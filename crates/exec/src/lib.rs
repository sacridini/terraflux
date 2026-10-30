//! Running a Terraflux model on one machine: its inputs read ([`load_inputs`]), its graph run node by node in
//! the order of their dependencies ([`Engine::run`]), its outputs written ([`write_outputs`]).
//!
//! This is the exact engine: whole maps in memory, the kernels in the order landschaft runs them, so a model
//! gives the same maps as `ls.simulate` with the same seed.
//!
//! With a [`Cache`] ([`Engine::run_with`]) the outputs of the top-level nodes, and of the nodes of their groups,
//! are kept between runs, and a node runs again only when it, a node it depends on, an input or a file it reads
//! changed; a run may stop at a node (its ancestors only run). The application runs the model as it is edited
//! this way.

pub mod frozen;
mod io;
pub mod lint;
pub mod testing;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use terraflux_graph::{Model, Node, Ref, Severity, Source, check, dependencies_among, ids, order};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Value};

pub use io::{load_inputs, write_outputs, write_value};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the run was stopped")]
    Cancelled,
    #[error("{0}")]
    Model(String),
    #[error("{at}: {message}")]
    Node { at: String, message: String },
    #[error("{0}")]
    Raster(#[from] terraflux_raster::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// What the engine reports while it runs (the application shows it).
pub trait Observer {
    /// A node is about to run (`at`: its path, e.g. `simulation/allocate`).
    fn node(&mut self, _at: &str) {}
    /// A container finished a step.
    fn step(&mut self, _at: &str, _step: usize, _of: usize) {}
    /// A warning of an operator.
    fn warning(&mut self, _at: &str, _message: &str) {}
    /// The values a container collected at a step (the application draws the maps as they come).
    fn collected(&mut self, _at: &str, _step: usize, _of: usize, _values: &BTreeMap<String, Value>) {}
    /// A top-level node is done: run, or its outputs taken from the cache.
    fn done(&mut self, _at: &str, _cached: bool) {}
    /// What the nodes of a loop's body gave at a step, by node (a cell's trace keeps its values).
    fn body(&mut self, _at: &str, _step: usize, _results: &BTreeMap<String, Outputs>) {}
    /// A node ran (not taken from the cache): how long it took and about how much memory what it gave holds.
    fn timed(&mut self, _at: &str, _seconds: f64, _bytes: usize) {}
    /// How far a node running is, as its operator tells it (a solver's gap, say).
    fn progress(&mut self, _at: &str, _progress: &terraflux_ops::Progress) {}
    /// Whether to stop: checked before every node, and by the operators that can stop while they run.
    fn cancelled(&mut self) -> bool {
        false
    }
}

/// An observer that ignores everything.
pub struct Quiet;

impl Observer for Quiet {}

/// The values of a run: every output of the top-level nodes, the warnings, and how long each node took.
pub struct Results {
    pub values: BTreeMap<String, Outputs>,
    pub warnings: Vec<String>,
    pub timings: Vec<Timing>,
}

/// A node's run: its place (`node`, `container/inner`), its time and about how much memory what it gave holds
/// (its maps', stacks', tables' and vectors' numbers).
#[derive(Debug, Clone, PartialEq)]
pub struct Timing {
    pub at: String,
    pub seconds: f64,
    pub bytes: usize,
}

/// About how much memory a value holds: its numbers (8 bytes each) and its vectors' coordinates.
pub fn bytes_of(v: &Value) -> usize {
    match v {
        Value::Map(m) => m.data.len() * 8,
        Value::Stack(s) => s.layers.iter().map(|l| l.len() * 8).sum(),
        Value::Table(t) => t.rows.iter().map(|r| r.len() * 8).sum(),
        Value::Series(s) => s.iter().map(bytes_of).sum(),
        Value::Vector(v) => {
            let mut n = 0;
            for g in &v.geometries {
                g.points(&mut |_| n += 1);
            }
            n * 16
        }
        _ => 0,
    }
}

/// The most memory the maps of a chain run file by file only written to files may take before they are no longer
/// kept ([`Engine::run_chain`]).
const KEPT_BYTES: usize = 2 << 30;

/// A chain of nodes run file by file ([`Engine::chain`]).
struct Chain {
    /// What gives the files (`node.port`, `$input`, `@value`).
    source: String,
    /// The files (each a layer of rasters on disk), by name.
    files: Vec<(String, Value)>,
    /// Its nodes, in order: the first takes the files.
    nodes: Vec<Node>,
    /// How many files at the same time: the `workers` of the `raster.read` of the folder, else the threads.
    workers: usize,
}

/// Whether a value is the files of a folder (rasters on disk of many files, no band chosen).
fn of_files(v: &Value) -> bool {
    matches!(v, Value::Raster(r) if r.images.len() > 1 && r.layers.len() > 1 && !r.picked)
}

/// The outputs of a chain's nodes that only its `raster.write`s read (not the model's outputs, `wanted`, nor any
/// other node): by node (its place in the chain) and port.
fn droppable(nodes: &[Node], chain: &Chain, wanted: &BTreeSet<String>) -> Vec<(usize, String)> {
    let writes: HashSet<&str> = chain.nodes.iter().filter(|m| m.op == "raster.write").map(|m| m.id.as_str()).collect();
    let mut readers: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for n in nodes {
        for r in terraflux_graph::references(n) {
            readers.entry(r).or_default().push(n.id.as_str());
        }
    }
    let mut out = Vec::new();
    for (j, m) in chain.nodes.iter().enumerate() {
        let from = format!("{}.", m.id);
        for (r, by) in &readers {
            if let Some(port) = r.strip_prefix(&from)
                && !wanted.contains(r)
                && by.iter().all(|b| writes.contains(b))
            {
                out.push((j, port.to_string()));
            }
        }
    }
    out
}

/// A `raster.write` of a chain, for a file: its path that file's ([`terraflux_ops::files::layer_path`]).
fn written_as<'a>(node: &std::borrow::Cow<'a, Node>, name: &str) -> std::borrow::Cow<'a, Node> {
    let mut n = node.clone().into_owned();
    if let Some(p) = n.params.get("path").and_then(|p| p.as_str()).filter(|p| !p.starts_with(['$', '@'])) {
        let p = terraflux_ops::files::layer_path(p, name);
        n.params.insert("path".into(), p.into());
    }
    std::borrow::Cow::Owned(n)
}

/// Some names, the first few: `files a, b, c and 4 more`.
fn listed(names: &[String]) -> String {
    let shown = names.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
    let more = if names.len() > 5 { format!(" and {} more", names.len() - 5) } else { String::new() };
    format!("file{} {shown}{more}", if names.len() > 1 { "s" } else { "" })
}

/// An observer around another that keeps the warnings, to tell them once for many files.
struct Muffled<'a> {
    inner: &'a mut dyn Observer,
    warnings: Vec<(String, String)>,
}

impl Observer for Muffled<'_> {
    fn node(&mut self, at: &str) {
        self.inner.node(at)
    }
    fn step(&mut self, at: &str, step: usize, of: usize) {
        self.inner.step(at, step, of)
    }
    fn warning(&mut self, at: &str, message: &str) {
        self.warnings.push((at.to_string(), message.to_string()))
    }
    fn collected(&mut self, at: &str, step: usize, of: usize, values: &BTreeMap<String, Value>) {
        self.inner.collected(at, step, of, values)
    }
    fn done(&mut self, at: &str, cached: bool) {
        self.inner.done(at, cached)
    }
    fn body(&mut self, at: &str, step: usize, results: &BTreeMap<String, Outputs>) {
        self.inner.body(at, step, results)
    }
    fn timed(&mut self, at: &str, seconds: f64, bytes: usize) {
        self.inner.timed(at, seconds, bytes)
    }
    fn progress(&mut self, at: &str, progress: &terraflux_ops::Progress) {
        self.inner.progress(at, progress)
    }
    fn cancelled(&mut self) -> bool {
        self.inner.cancelled()
    }
}

/// A file run through a chain: what each of its nodes gave, by its place in the chain (or the error), the warnings
/// told, and whether it was taken from its record ([`done_before`]).
struct Done {
    outputs: Result<Vec<Outputs>>,
    warnings: Vec<(String, String)>,
    skipped: bool,
}

/// The observer of a file run on a thread of a chain's pool: only whether to stop (the warnings are kept by
/// [`Muffled`] around it, the progress told by the chain).
struct Relay<'a> {
    stop: &'a AtomicBool,
}

impl Observer for Relay<'_> {
    fn cancelled(&mut self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// Whether a chain's files done before need not run again: it writes files, and nothing but its `raster.write`s
/// reads what its other nodes give (no node after it, not the model's outputs).
fn resumable(nodes: &[Node], chain: &Chain, wanted: &BTreeSet<String>) -> bool {
    let writes: HashSet<&str> = chain.nodes.iter().filter(|m| m.op == "raster.write").map(|m| m.id.as_str()).collect();
    if writes.is_empty() {
        return false;
    }
    let others: HashSet<&str> = chain.nodes.iter().map(|m| m.id.as_str()).filter(|id| !writes.contains(id)).collect();
    let read = |r: &str| matches!(Ref::parse(r), Ok(Ref::Port { node, .. }) if others.contains(node.as_str()));
    !wanted.iter().any(|r| read(r))
        && nodes
            .iter()
            .filter(|n| !writes.contains(n.id.as_str()))
            .all(|n| others.contains(n.id.as_str()) || !terraflux_graph::references(n).iter().any(|r| read(r)))
}

/// The file on disk a chain's file is (the file of its layer).
fn file_path(file: &Value) -> Option<std::path::PathBuf> {
    match file {
        Value::Raster(r) => r.images.get(r.layers.get(r.chosen)?.image).map(|i| i.path()),
        _ => None,
    }
}

/// What a file run through a chain depends on, as a hex SHA-256: the file (its path, size and time) and the chain's
/// nodes (operators, inputs, settings). None for a chain that writes no file, or a file not on disk.
fn print_of(chain: &Chain, file: &Value) -> Option<String> {
    use sha2::{Digest, Sha256};
    chain.nodes.iter().find(|m| m.op == "raster.write")?;
    let path = file_path(file)?;
    let meta = std::fs::metadata(&path).ok()?;
    let time = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    let nodes: Vec<serde_json::Value> = chain
        .nodes
        .iter()
        .map(|m| serde_json::json!({"id": m.id, "op": m.op, "inputs": m.inputs, "params": m.params}))
        .collect();
    let mut h = Sha256::new();
    h.update(path.to_string_lossy().as_bytes());
    h.update(meta.len().to_le_bytes());
    h.update(time.to_le_bytes());
    h.update(serde_json::to_string(&nodes).ok()?.as_bytes());
    Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The record of a file run through a chain: next to the first file its first `raster.write` writes, `.json`
/// (`mspa/clc_r0c0_mspa.json`).
fn record_path(chain: &Chain, name: &str) -> Option<std::path::PathBuf> {
    let w = chain.nodes.iter().find(|m| m.op == "raster.write")?;
    let p = w.params.get("path")?.as_str().filter(|p| !p.starts_with(['$', '@']))?;
    Some(std::path::Path::new(&terraflux_ops::files::layer_path(p, name)).with_extension("json"))
}

/// Record a file run through a chain (as `batch.run` does): the file read, the files written, the nodes and
/// their settings, how long it took, Terraflux's version, and what it depends on (`print`, [`print_of`]).
fn record(chain: &Chain, name: &str, file: &Value, print: &str, outs: &[Outputs], seconds: f64) {
    let Some(path) = record_path(chain, name) else { return };
    let written: serde_json::Map<String, serde_json::Value> = chain
        .nodes
        .iter()
        .zip(outs)
        .filter(|(m, _)| m.op == "raster.write")
        .filter_map(|(m, o)| match o.get("path") {
            Some(Value::Text(p)) => Some((m.id.clone(), p.clone().into())),
            _ => None,
        })
        .collect();
    let nodes: Vec<serde_json::Value> =
        chain.nodes.iter().map(|m| serde_json::json!({"id": m.id, "op": m.op, "params": m.params})).collect();
    let r = serde_json::json!({
        "input": file_path(file).map(|p| p.to_string_lossy().into_owned()),
        "name": name,
        "outputs": written,
        "nodes": nodes,
        "seconds": seconds,
        "terraflux": env!("CARGO_PKG_VERSION"),
        "print": print,
    });
    let _ = std::fs::write(path, serde_json::to_string_pretty(&r).unwrap_or_default());
}

/// A file a chain ran on before with the same nodes and settings, its record saying so and its files there: what
/// its `raster.write`s gave (the files), nothing for its other nodes.
fn done_before(chain: &Chain, name: &str, print: &str) -> Option<Vec<Outputs>> {
    let text = std::fs::read_to_string(record_path(chain, name)?).ok()?;
    let r: serde_json::Value = serde_json::from_str(&text).ok()?;
    if r.get("print")?.as_str()? != print {
        return None;
    }
    let written = r.get("outputs")?.as_object()?;
    chain
        .nodes
        .iter()
        .map(|m| {
            if m.op != "raster.write" {
                return Some(Outputs::new());
            }
            let p = written.get(&m.id)?.as_str()?;
            std::path::Path::new(p).exists().then(|| [("path".to_string(), Value::Text(p.to_string()))].into())
        })
        .collect()
}

/// Records the time of every node as it runs (an observer around another).
struct Timed<'a> {
    inner: &'a mut dyn Observer,
    timings: Vec<Timing>,
}

impl Observer for Timed<'_> {
    fn node(&mut self, at: &str) {
        self.inner.node(at)
    }
    fn step(&mut self, at: &str, step: usize, of: usize) {
        self.inner.step(at, step, of)
    }
    fn warning(&mut self, at: &str, message: &str) {
        self.inner.warning(at, message)
    }
    fn collected(&mut self, at: &str, step: usize, of: usize, values: &BTreeMap<String, Value>) {
        self.inner.collected(at, step, of, values)
    }
    fn done(&mut self, at: &str, cached: bool) {
        self.inner.done(at, cached)
    }
    fn body(&mut self, at: &str, step: usize, results: &BTreeMap<String, Outputs>) {
        self.inner.body(at, step, results)
    }
    fn timed(&mut self, at: &str, seconds: f64, bytes: usize) {
        // (a node of a loop's body each step: its time summed)
        match self.timings.iter_mut().find(|t| t.at == at) {
            Some(t) => {
                t.seconds += seconds;
                t.bytes = t.bytes.max(bytes);
            }
            None => self.timings.push(Timing { at: at.to_string(), seconds, bytes }),
        }
        self.inner.timed(at, seconds, bytes)
    }
    fn progress(&mut self, at: &str, progress: &terraflux_ops::Progress) {
        self.inner.progress(at, progress)
    }
    fn cancelled(&mut self) -> bool {
        self.inner.cancelled()
    }
}

impl Results {
    /// The value of a reference (`node.port`).
    pub fn get(&self, reference: &str) -> Option<&Value> {
        match Ref::parse(reference).ok()? {
            Ref::Port { node, port } => self.values.get(&node)?.get(&port),
            _ => None,
        }
    }
}

/// The outputs of a top-level node kept, the fingerprint of what they came from and the node's warnings; `held`:
/// its result frozen ([`Cache::hold`]), used while the node is frozen whatever it came from.
struct Kept {
    print: u64,
    outputs: Outputs,
    warnings: Vec<String>,
    held: bool,
}

/// The outputs of the top-level nodes of a model kept between runs ([`Engine::run_with`]), and those of the nodes
/// of their groups by place (`group/node`: a group runs again only the nodes of its body that changed).
#[derive(Default)]
pub struct Cache {
    kept: BTreeMap<String, Kept>,
    inner: BTreeMap<String, Kept>,
}

impl Cache {
    /// The outputs kept of a node.
    pub fn outputs(&self, node: &str) -> Option<&Outputs> {
        self.kept.get(node).map(|k| &k.outputs)
    }

    /// The nodes with outputs kept.
    pub fn nodes(&self) -> impl Iterator<Item = &str> {
        self.kept.keys().map(String::as_str)
    }

    pub fn forget(&mut self, node: &str) {
        self.kept.remove(node);
        let within = format!("{node}/");
        self.inner.retain(|at, _| !at.starts_with(&within));
    }

    /// The places of the nodes of groups with outputs kept (`group/node`).
    pub fn inner_nodes(&self) -> impl Iterator<Item = &str> {
        self.inner.keys().map(String::as_str)
    }

    /// Hold a node's frozen result (read from disk, or the outputs it gave): used instead of running it while it
    /// is frozen, those of its own run kept when it has them already. `print` tells what it was (its downstream
    /// run again when it changes).
    pub fn hold(&mut self, node: &str, outputs: Option<Outputs>, print: u64) -> bool {
        match (self.kept.get_mut(node), outputs) {
            (Some(k), _) => k.held = true,
            (None, Some(outputs)) => {
                self.kept.insert(node.to_string(), Kept { print, outputs, warnings: Vec::new(), held: true });
            }
            (None, None) => return false,
        }
        true
    }

    /// Whether a node's frozen result is held.
    pub fn holds(&self, node: &str) -> bool {
        self.kept.get(node).is_some_and(|k| k.held)
    }

    /// Let go of a node's frozen result (it runs again when its fingerprint differs).
    pub fn release(&mut self, node: &str) {
        if let Some(k) = self.kept.get_mut(node) {
            k.held = false;
        }
    }

    pub fn clear(&mut self) {
        self.kept.clear();
        self.inner.clear();
    }
}

/// Whether a value holds a random generator, whose state changes as it is used (not to be kept).
fn holds_rng(v: &Value) -> bool {
    match v {
        Value::Rng(_) | Value::NumpyRng(_) => true,
        Value::Series(s) => s.iter().any(holds_rng),
        _ => false,
    }
}

/// A node as it counts for its outputs: without its place and width on the canvas and its record (note, why...).
/// The inputs of a node of the user's library as it declares them: those not given their values of their own, the
/// layers it reads there (the error says which are missing), rasters on disk read as the map or stack it takes.
fn user_inputs(u: &terraflux_graph::Uses, values: &mut BTreeMap<String, Value>, at: &str) -> Result<()> {
    for (k, i) in &u.inputs {
        if !values.contains_key(k)
            && let Some(d) = &i.default
        {
            values.insert(k.clone(), terraflux_ops::Value::from_json(d));
        }
        let Some(v) = values.get_mut(k) else { continue };
        if !i.bands.is_empty()
            && let Some(why) = terraflux_ops::algebra_tools::lacks_layers(v, &i.bands)
        {
            return Err(Error::Node {
                at: format!("{at}.{k}"),
                message: format!("{} reads {k}'s layers: {why}", u.label()),
            });
        }
        if matches!(i.ty, terraflux_graph::Type::Map | terraflux_graph::Type::Stack) && matches!(v, Value::Raster(_)) {
            let raster = std::mem::replace(v, Value::Bool(false));
            *v = terraflux_ops::files::as_port(raster, i.ty)
                .map_err(|e| Error::Node { at: format!("{at}.{k}"), message: e.to_string() })?;
        }
    }
    Ok(())
}

fn bare(n: &Node) -> Node {
    let mut n = n.unrecorded();
    n.pos = None;
    n.width = None;
    n.frozen = false;
    if let Some(b) = &mut n.body {
        b.nodes = b.nodes.iter().map(bare).collect();
    }
    n
}

/// The size and time of the files a node reads (in its body too).
fn files_read(n: &Node, h: &mut impl Hasher) {
    if (n.op == "raster.read" || n.op == "vector.read")
        && let Some(serde_json::Value::String(p)) = n.params.get("path")
    {
        match std::fs::metadata(p) {
            Ok(m) => (m.len(), m.modified().ok()).hash(h),
            Err(_) => "missing".hash(h),
        }
    }
    for b in n.body.iter().flat_map(|b| &b.nodes) {
        files_read(b, h);
    }
}

/// The fingerprint of a top-level node's outputs: the node, the fingerprints of the nodes it depends on, the
/// inputs it reads and the files; none when a node it depends on is not kept.
fn fingerprint(
    n: &Node,
    model: &Model,
    among: &std::collections::HashSet<&str>,
    inputs: &BTreeMap<String, Value>,
    prints: &BTreeMap<String, Option<u64>>,
) -> Option<u64> {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(&bare(n)).unwrap_or_default().hash(&mut h);
    for d in dependencies_among(n, among) {
        (d.as_str(), (*prints.get(&d)?)?).hash(&mut h);
    }
    let mut read: Vec<String> =
        terraflux_graph::references(n).iter().filter_map(|r| r.strip_prefix('$').map(String::from)).collect();
    read.sort();
    read.dedup();
    for name in read {
        name.hash(&mut h);
        serde_json::to_string(&model.inputs.get(&name)).unwrap_or_default().hash(&mut h);
        match inputs.get(&name) {
            Some(Value::Map(m)) => (std::sync::Arc::as_ptr(&m.data) as usize).hash(&mut h),
            Some(Value::Stack(s)) => s.layers.iter().for_each(|l| (std::sync::Arc::as_ptr(l) as usize).hash(&mut h)),
            Some(v) => format!("{v:?}").hash(&mut h),
            None => {}
        }
    }
    files_read(n, &mut h);
    Some(h.finish())
}

/// The ids of a node's body, its bodies' within too.
fn own_ids<'a>(n: &'a Node, out: &mut std::collections::HashSet<&'a str>) {
    for b in n.body.iter().flat_map(|b| &b.nodes) {
        out.insert(&b.id);
        own_ids(b, out);
    }
}

/// The fingerprint of a node of a scope whose outputs are kept ([`Scope`]): the scope's and the node's as a
/// top-level node's; none when it reads a node of another scope (an enclosing one) or one not kept.
fn scoped_print(n: &Node, env: &Env, scope: &Scope, inputs: &BTreeMap<String, Value>) -> Option<u64> {
    let among: std::collections::HashSet<&str> = env.prints.keys().map(String::as_str).collect();
    let mut own = std::collections::HashSet::new();
    own_ids(n, &mut own);
    for r in terraflux_graph::references(n) {
        if let Ok(Ref::Port { node, .. }) = Ref::parse(&r)
            && !among.contains(node.as_str())
            && !own.contains(node.as_str())
        {
            return None;
        }
    }
    let p = fingerprint(n, scope.model, &among, inputs, &env.prints)?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (scope.print, p).hash(&mut h);
    Some(h.finish())
}

/// The outputs of a node bypassed: each the first of its inputs (in its operator's order, then the others) of a
/// type that fits it; the outputs no input fits are missing (what reads them is told).
fn bypassed(sig: Option<&terraflux_graph::OpSignature>, values: BTreeMap<String, Value>) -> Outputs {
    let Some(sig) = sig else { return Outputs::new() };
    let mut order: Vec<&String> =
        sig.inputs.iter().filter_map(|p| values.get_key_value(&p.name).map(|(k, _)| k)).collect();
    let rest: Vec<&String> = values.keys().filter(|k| !order.contains(k)).collect();
    order.extend(rest);
    let mut out = Outputs::new();
    for o in &sig.outputs {
        let fits = |v: &&Value| v.ty() == o.ty || (o.ty != terraflux_graph::Type::Any && v.ty().fits(o.ty));
        if let Some(v) = order.iter().map(|k| &values[*k]).find(fits) {
            out.insert(o.name.clone(), v.clone());
        }
    }
    out
}

/// A value as a setting holds it: a number, a whole number, true or false, a text (none for maps, tables...).
fn setting_value(v: &Value) -> Option<serde_json::Value> {
    Some(match v {
        // (a whole number as a whole number: a setting of whole numbers takes it)
        Value::Number(x) if x.fract() == 0.0 && x.abs() < 9e15 => (*x as i64).into(),
        Value::Number(x) => serde_json::json!(x),
        Value::Integer(i) => (*i).into(),
        Value::Bool(b) => (*b).into(),
        Value::Text(t) => t.clone().into(),
        Value::Json(j) => j.clone(),
        _ => return None,
    })
}

/// The top-level node of a place (`node`, `node.port`, `container/inner...`).
fn head(at: &str) -> &str {
    at.split(['/', '.']).next().unwrap_or(at)
}

/// A scope whose nodes' outputs are kept between runs: the top level, a group's body there or in another such
/// group (not a loop's, whose nodes give a value per step); `print`, what its values carried in came from.
#[derive(Clone, Copy)]
struct Scope<'p> {
    cache: &'p Mutex<Cache>,
    model: &'p Model,
    print: u64,
}

struct Env<'p> {
    parent: Option<&'p Env<'p>>,
    results: BTreeMap<String, Outputs>,
    carried: BTreeMap<String, Value>,
    kept: Option<Scope<'p>>,
    /// The fingerprints of the scope's nodes run (none: not kept, nor what depends on them).
    prints: BTreeMap<String, Option<u64>>,
    /// The outputs of the scope's nodes run ahead, in a chain run file by file ([`Engine::chain`]).
    fused: BTreeMap<String, Outputs>,
    /// The scope's nodes whose outputs were not all kept (a chain's maps only written to files): not cached.
    partial: BTreeSet<String>,
}

impl<'p> Env<'p> {
    /// The scope of a container's body, under this one: not kept.
    fn child(&'p self, carried: BTreeMap<String, Value>) -> Env<'p> {
        Env {
            parent: Some(self),
            results: BTreeMap::new(),
            carried,
            kept: None,
            prints: BTreeMap::new(),
            fused: BTreeMap::new(),
            partial: BTreeSet::new(),
        }
    }
}

impl Env<'_> {
    fn port(&self, node: &str, port: &str) -> Option<&Value> {
        match self.results.get(node) {
            Some(o) => o.get(port),
            None => self.parent?.port(node, port),
        }
    }

    /// All the outputs of a node of this scope or an enclosing one.
    fn outputs(&self, node: &str) -> Option<&Outputs> {
        match self.results.get(node) {
            Some(o) => Some(o),
            None => self.parent?.outputs(node),
        }
    }

    /// Whether a node of this scope (or an enclosing one) has run.
    fn ran(&self, node: &str) -> bool {
        self.results.contains_key(node) || self.parent.is_some_and(|p| p.ran(node))
    }

    fn carried(&self, name: &str) -> Option<&Value> {
        match self.carried.get(name) {
            Some(v) => Some(v),
            None => self.parent?.carried(name),
        }
    }
}

/// Runs models with a registry of operators.
pub struct Engine {
    pub registry: Registry,
}

impl Default for Engine {
    fn default() -> Self {
        Engine { registry: Registry::standard() }
    }
}

impl Engine {
    /// Check the model (errors stop it), then run it on the inputs.
    pub fn run(&self, model: &Model, inputs: &BTreeMap<String, Value>, obs: &mut dyn Observer) -> Result<Results> {
        self.run_with(model, inputs, obs, None, None)
    }

    /// Run the model, or only what a top-level node needs (`target`: the node and the nodes it depends on),
    /// taking the outputs of the nodes that did not change from `cache` and keeping the new ones there. Only
    /// the errors of the nodes run stop it. Nodes whose outputs hold a random generator, the nodes that depend
    /// on them, and `raster.write` (a file to write) always run.
    pub fn run_with(
        &self,
        model: &Model,
        inputs: &BTreeMap<String, Value>,
        obs: &mut dyn Observer,
        cache: Option<&Mutex<Cache>>,
        target: Option<&str>,
    ) -> Result<Results> {
        let mut timed = Timed { inner: obs, timings: Vec::new() };
        let mut r = self.run_timed(model, inputs, &mut timed, cache, target)?;
        r.timings = timed.timings;
        Ok(r)
    }

    fn run_timed(
        &self,
        model: &Model,
        inputs: &BTreeMap<String, Value>,
        obs: &mut dyn Observer,
        cache: Option<&Mutex<Cache>>,
        target: Option<&str>,
    ) -> Result<Results> {
        // the solvers and kernels of the nodes with the process's threads, on whatever thread the run is
        terraflux_ops::threads::for_this_thread();
        // an input of a value not given: the model's own value
        let defaults: Vec<(String, Value)> = model
            .inputs
            .iter()
            .filter(|(k, _)| !inputs.contains_key(*k))
            .filter_map(|(k, i)| match i {
                terraflux_graph::Input::Value { value, .. } => Some((k.clone(), Value::from_json(value))),
                _ => None,
            })
            .collect();
        let with_defaults;
        let inputs = if defaults.is_empty() {
            inputs
        } else {
            with_defaults = inputs.clone().into_iter().chain(defaults).collect::<BTreeMap<_, _>>();
            &with_defaults
        };
        let among = ids(&model.nodes);
        let by_id: std::collections::HashMap<&str, &Node> = model.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        // the nodes frozen whose result is held: what only they read does not run
        let held: std::collections::HashSet<&str> = match cache {
            Some(c) => {
                let c = c.lock().unwrap();
                model.nodes.iter().filter(|n| n.frozen && c.holds(&n.id)).map(|n| n.id.as_str()).collect()
            }
            None => Default::default(),
        };
        // the nodes to run: every one, or the target's ancestors (not those of a node held)
        let needed: Option<std::collections::BTreeSet<String>> = match target {
            None if held.is_empty() => None,
            _ => {
                let todo: Vec<String> = match target {
                    Some(t) => {
                        if !by_id.contains_key(t) {
                            return Err(Error::Model(format!("no node {t:?}")));
                        }
                        vec![t.to_string()]
                    }
                    // a whole run: from the nodes nothing reads
                    None => {
                        let read: std::collections::HashSet<String> =
                            model.nodes.iter().flat_map(|n| dependencies_among(n, &among)).collect();
                        model.nodes.iter().filter(|n| !read.contains(&n.id)).map(|n| n.id.clone()).collect()
                    }
                };
                let mut todo = todo;
                let mut set = std::collections::BTreeSet::new();
                while let Some(id) = todo.pop() {
                    if !set.contains(&id)
                        && let Some(n) = by_id.get(id.as_str())
                    {
                        if !held.contains(id.as_str()) {
                            todo.extend(dependencies_among(n, &among));
                        }
                        set.insert(id);
                    }
                }
                Some(set)
            }
        };
        let runs = |id: &str| needed.as_ref().is_none_or(|s| s.contains(id));
        let errors: Vec<String> = check(model, &self.registry)
            .into_iter()
            .filter(|i| i.severity == Severity::Error && (i.at.is_empty() || runs(head(&i.at))))
            .map(|i| i.to_string())
            .collect();
        if !errors.is_empty() {
            return Err(Error::Model(format!("the model has errors:\n  {}", errors.join("\n  "))));
        }
        let read: Vec<String> = match &needed {
            None => model.inputs.keys().cloned().collect(),
            Some(set) => model
                .nodes
                .iter()
                .filter(|n| set.contains(&n.id))
                .flat_map(terraflux_graph::references)
                .filter_map(|r| r.strip_prefix('$').map(String::from))
                .collect(),
        };
        for name in read {
            if !inputs.contains_key(&name) {
                return Err(Error::Model(format!("no value for the input ${name}")));
            }
        }
        let mut env = Env {
            parent: None,
            results: BTreeMap::new(),
            carried: BTreeMap::new(),
            kept: cache.map(|cache| Scope { cache, model, print: 0 }),
            prints: BTreeMap::new(),
            fused: BTreeMap::new(),
            partial: BTreeSet::new(),
        };
        let wanted: BTreeSet<String> = model.outputs.values().map(|o| o.from.clone()).collect();
        let mut warnings = Vec::new();
        let ordered =
            order(&model.nodes, |n| dependencies_among(n, &among)).map_err(|e| Error::Model(e.to_string()))?;
        for node in ordered {
            if !runs(&node.id) {
                continue;
            }
            if obs.cancelled() {
                return Err(Error::Cancelled);
            }
            let at = node.id.as_str();
            let mut print = cache.and_then(|_| fingerprint(node, model, &among, inputs, &env.prints));
            let kept = match (cache, print) {
                // frozen: its result held, whatever it came from
                (Some(c), _) if held.contains(at) => c.lock().unwrap().kept.get(&node.id).map(|k| {
                    print = Some(k.print);
                    (k.outputs.clone(), k.warnings.clone())
                }),
                (Some(c), Some(p)) => c
                    .lock()
                    .unwrap()
                    .kept
                    .get(&node.id)
                    .filter(|k| k.print == p)
                    .map(|k| (k.outputs.clone(), k.warnings.clone())),
                _ => None,
            };
            if let Some((out, ws)) = kept {
                obs.node(at);
                for w in &ws {
                    obs.warning(at, w);
                    warnings.push(format!("{at}: {w}"));
                }
                Self::replay(node, at, &out, obs);
                obs.done(at, true);
                env.results.insert(node.id.clone(), out);
                env.prints.insert(node.id.clone(), print);
                continue;
            }
            obs.node(at);
            let before = warnings.len();
            let node = &with_product(node, &model.nodes);
            let started = std::time::Instant::now();
            let out =
                self.produce(&model.nodes, node, &mut env, inputs, at, obs, &mut warnings, &runs, Some(&wanted))?;
            obs.timed(at, started.elapsed().as_secs_f64(), out.values().map(bytes_of).sum());
            let keep = print.is_some()
                && node.op != "raster.write"
                && !out.values().any(holds_rng)
                && !env.partial.contains(&node.id);
            if let (Some(c), Some(p), true) = (cache, print, keep) {
                let prefix = format!("{at}: ");
                let ws = warnings[before..].iter().map(|w| w.strip_prefix(&prefix).unwrap_or(w).to_string()).collect();
                let k = Kept { print: p, outputs: out.clone(), warnings: ws, held: false };
                c.lock().unwrap().kept.insert(node.id.clone(), k);
            }
            env.prints.insert(node.id.clone(), if keep { print } else { None });
            obs.done(at, false);
            env.results.insert(node.id.clone(), out);
        }
        if let Some(c) = cache {
            let mut c = c.lock().unwrap();
            c.kept.retain(|id, _| by_id.contains_key(id.as_str()));
            c.inner.retain(|at, _| by_id.contains_key(head(at)));
        }
        Ok(Results { values: env.results, warnings, timings: Vec::new() })
    }

    /// What a loop collected, step by step, from its kept outputs (as if it ran).
    fn replay(node: &Node, at: &str, out: &Outputs, obs: &mut dyn Observer) {
        let Some(body) = node.body.as_ref().filter(|_| node.op == "control.repeat") else { return };
        let series: BTreeMap<&String, &Vec<Value>> = body
            .collect
            .keys()
            .filter_map(|k| match out.get(k) {
                Some(Value::Series(s)) => Some((k, s)),
                _ => None,
            })
            .collect();
        let steps = series.values().map(|s| s.len()).max().unwrap_or(0);
        for step in 1..=steps {
            let now: BTreeMap<String, Value> =
                series.iter().filter_map(|(k, s)| s.get(step - 1).map(|v| ((*k).clone(), v.clone()))).collect();
            obs.collected(at, step, steps, &now);
            obs.step(at, step, steps);
        }
    }

    /// A node whose settings that read values (`"$input"`, `"@carried"`, `{ from = "node.port" }`) hold them.
    fn with_settings(&self, node: &Node, env: &Env, inputs: &BTreeMap<String, Value>, at: &str) -> Result<Node> {
        let mut n = node.clone();
        for (k, v) in n.params.iter_mut() {
            let Some(r) = terraflux_graph::setting_ref(v).map(String::from) else { continue };
            let place = format!("{at}.{k}");
            let value = self.resolve(&Source::Ref(r.clone()), env, inputs, &place)?;
            *v = setting_value(&value).ok_or_else(|| Error::Node {
                at: place,
                message: format!("{r} gives a {:?}: a setting takes a number, true or false, or a text", value.ty()),
            })?;
        }
        Ok(n)
    }

    fn resolve(&self, src: &Source, env: &Env, inputs: &BTreeMap<String, Value>, at: &str) -> Result<Value> {
        let r = match src {
            Source::Literal(v) => return Ok(Value::from_json(v)),
            Source::Ref(r) => Ref::parse(r).map_err(|e| Error::Node { at: at.into(), message: e.to_string() })?,
        };
        let v = match &r {
            Ref::Input(n) => inputs.get(n),
            Ref::Carried(n) => env.carried(n),
            Ref::Port { node, port } => env.port(node, port),
        };
        v.cloned().ok_or_else(|| {
            let message = match &r {
                Ref::Port { node, port } if env.ran(node) => format!(
                    "{node} gave no output {port:?}: some outputs are given only with some settings or inputs (see \
                     its operator's description)"
                ),
                _ => format!("{r} has no value"),
            };
            Error::Node { at: at.into(), message }
        })
    }

    fn run_scope(
        &self,
        nodes: &[Node],
        env: &mut Env,
        inputs: &BTreeMap<String, Value>,
        prefix: &str,
        obs: &mut dyn Observer,
        warnings: &mut Vec<String>,
    ) -> Result<()> {
        let among = ids(nodes);
        let ordered = order(nodes, |n| dependencies_among(n, &among)).map_err(|e| Error::Model(e.to_string()))?;
        for node in ordered {
            let at = if prefix.is_empty() { node.id.clone() } else { format!("{prefix}/{}", node.id) };
            if obs.cancelled() {
                return Err(Error::Cancelled);
            }
            obs.node(&at);
            // in a group's body kept: what did not change from the cache (a group inside it runs, its own body's
            // nodes kept in turn)
            let kept = env.kept;
            let print = kept.and_then(|k| scoped_print(node, env, &k, inputs));
            if let (Some(k), Some(p)) = (kept, print)
                && node.op != terraflux_graph::GROUP
            {
                let found = k
                    .cache
                    .lock()
                    .unwrap()
                    .inner
                    .get(&at)
                    .filter(|c| c.print == p)
                    .map(|c| (c.outputs.clone(), c.warnings.clone()));
                if let Some((out, ws)) = found {
                    for w in &ws {
                        obs.warning(&at, w);
                        warnings.push(format!("{at}: {w}"));
                    }
                    Self::replay(node, &at, &out, obs);
                    env.results.insert(node.id.clone(), out);
                    env.prints.insert(node.id.clone(), print);
                    continue;
                }
            }
            let before = warnings.len();
            let node = &with_product(node, nodes);
            let started = std::time::Instant::now();
            let out = self.produce(nodes, node, env, inputs, &at, obs, warnings, &|_| true, None)?;
            obs.timed(&at, started.elapsed().as_secs_f64(), out.values().map(bytes_of).sum());
            let keep = print.is_some()
                && node.op != "raster.write"
                && !out.values().any(holds_rng)
                && !env.partial.contains(&node.id);
            if let (Some(k), Some(p), true) = (kept, print, keep)
                && node.op != terraflux_graph::GROUP
            {
                let prefix = format!("{at}: ");
                let ws = warnings[before..].iter().map(|w| w.strip_prefix(&prefix).unwrap_or(w).to_string()).collect();
                let c = Kept { print: p, outputs: out.clone(), warnings: ws, held: false };
                k.cache.lock().unwrap().inner.insert(at.clone(), c);
            }
            if kept.is_some() {
                env.prints.insert(node.id.clone(), if keep { print } else { None });
            }
            env.results.insert(node.id.clone(), out);
        }
        Ok(())
    }

    /// A node's outputs: those of a chain run file by file it is part of ([`Engine::chain`]), else it run. `runs`:
    /// whether a node of the scope runs at all (a run that stops at a node); `wanted`: what the model's outputs read
    /// (at the top level; none in a body, whose chains keep every map).
    #[allow(clippy::too_many_arguments)]
    fn produce(
        &self,
        nodes: &[Node],
        node: &Node,
        env: &mut Env,
        inputs: &BTreeMap<String, Value>,
        at: &str,
        obs: &mut dyn Observer,
        warnings: &mut Vec<String>,
        runs: &dyn Fn(&str) -> bool,
        wanted: Option<&BTreeSet<String>>,
    ) -> Result<Outputs> {
        if let Some(out) = env.fused.remove(&node.id) {
            return Ok(out);
        }
        if let Some(chain) = self.chain(nodes, node, env, inputs, at, runs) {
            let (mut outs, partial) = self.run_chain(nodes, &chain, env, inputs, at, obs, warnings, wanted)?;
            let mine = outs.remove(&node.id).unwrap_or_default();
            env.fused.extend(outs);
            env.partial.extend(partial);
            return Ok(mine);
        }
        self.run_node(node, env, inputs, at, obs, warnings)
    }

    /// The chain a node begins when it is given the files of a folder (rasters on disk of many files) where it
    /// takes one map: it and the nodes after it that take, where they take one map, what it gives (and so on),
    /// with all else they read already given, and the `raster.write`s of their maps that write a file per file. Run
    /// file by file ([`Engine::run_chain`]), not each node on all the files before the next. None when the node is
    /// given another value of many layers where it takes one map (then paired with the files, layer by layer).
    fn chain(
        &self,
        nodes: &[Node],
        node: &Node,
        env: &Env,
        inputs: &BTreeMap<String, Value>,
        at: &str,
        runs: &dyn Fn(&str) -> bool,
    ) -> Option<Chain> {
        let sig = self.registry.sig(&node.op)?;
        if node.bypass || sig.container {
            return None;
        }
        let mut source: Option<(String, Value)> = None;
        for (port, src) in &node.inputs {
            let Source::Ref(r) = src else { continue };
            if !sig.per_layer(port) {
                continue;
            }
            let v = self.resolve(src, env, inputs, at).ok()?;
            if of_files(&v) {
                match &source {
                    Some((s, _)) if s != r => return None,
                    Some(_) => {}
                    None => source = Some((r.clone(), v)),
                }
            } else if terraflux_ops::files::layers(&v).is_some() {
                return None;
            }
        }
        let (source, value) = source?;
        let files = terraflux_ops::files::layers(&value)?;
        let grids = matches!(&value, Value::Raster(r) if r.mixed);
        let among = ids(nodes);
        let ordered = order(nodes, |n| dependencies_among(n, &among)).ok()?;
        let after = ordered.iter().position(|n| n.id == node.id)? + 1;
        let mut members = vec![node.clone()];
        let mut inside: HashSet<String> = HashSet::from([node.id.clone()]);
        for m in &ordered[after..] {
            if env.results.contains_key(&m.id) || !runs(&m.id) || m.bypass {
                continue;
            }
            let Some(ms) = self.registry.sig(&m.op) else { continue };
            if ms.container {
                continue;
            }
            let reads =
                |r: &str| r == source || matches!(Ref::parse(r), Ok(Ref::Port { node: n, .. }) if inside.contains(&n));
            // a file per file: {name} in its path, or maps of different grids (no file of them all)
            let writes = m.op == "raster.write"
                && (grids || m.params.get("path").and_then(|p| p.as_str()).is_some_and(|p| p.contains("{name}")));
            let (mut each, mut whole) = (false, false);
            for (p, src) in &m.inputs {
                let Source::Ref(r) = src else { continue };
                if reads(r) {
                    if ms.per_layer(p) || (writes && p == "value") {
                        each = true;
                    } else {
                        whole = true;
                    }
                } else if ms.per_layer(p)
                    && self
                        .resolve(src, env, inputs, at)
                        .ok()
                        .is_some_and(|v| terraflux_ops::files::layers(&v).is_some())
                {
                    whole = true;
                }
            }
            // a setting that reads the chain reads what it gathered; what was not run yet, after it
            whole |= m.params.values().filter_map(terraflux_graph::setting_ref).any(reads);
            let ready = terraflux_graph::references(m).iter().all(|r| match Ref::parse(r) {
                Ok(Ref::Port { node: n, .. }) => inside.contains(&n) || env.ran(&n),
                _ => true,
            });
            if each && !whole && ready {
                inside.insert(m.id.clone());
                members.push((*m).clone());
            }
        }
        // the files at the same time: as the folder's raster.read says, else as many as the threads
        let workers = match Ref::parse(&source) {
            Ok(Ref::Port { node: from, .. }) => nodes
                .iter()
                .find(|n| n.id == from && n.op == "raster.read")
                .and_then(|n| n.params.get("workers"))
                .and_then(|w| w.as_u64())
                .map(|w| w as usize),
            _ => None,
        };
        let workers = workers.unwrap_or_else(rayon::current_num_threads);
        Some(Chain { source, files, nodes: members, workers })
    }

    /// A chain run file by file: for each file, its nodes in order on it (the file read, passed along, written,
    /// let go); what they give gathered as a node run once per layer gathers it ([`gather`]), by the files' names.
    /// Several files at the same time (`Chain::workers`, in a pool of as many threads, which the operators share).
    /// A file that fails does not stop the others (told in a warning; an error when all fail). A chain that writes
    /// files leaves a record of each file ([`record_path`]); when nothing else reads what it gives, a file whose
    /// record says it was done with the same nodes, settings and file, its files there, is not run again. At the
    /// top level (`wanted`), the maps only written to files are not kept when they would take more than
    /// [`KEPT_BYTES`]: their nodes are given back as partial, not to be kept in the cache.
    #[allow(clippy::too_many_arguments)]
    fn run_chain(
        &self,
        nodes: &[Node],
        chain: &Chain,
        env: &Env,
        inputs: &BTreeMap<String, Value>,
        at: &str,
        obs: &mut dyn Observer,
        warnings: &mut Vec<String>,
        wanted: Option<&BTreeSet<String>>,
    ) -> Result<(BTreeMap<String, Outputs>, BTreeSet<String>)> {
        let prefix = at.strip_suffix(chain.nodes[0].id.as_str()).unwrap_or("");
        let source = Ref::parse(&chain.source).map_err(|e| Error::Node { at: at.into(), message: e.to_string() })?;
        let n = chain.files.len();
        let droppable = wanted.map_or_else(Vec::new, |w| droppable(nodes, chain, w));
        // what it gives read by nothing but its files: a file done before need not run again
        let resumable = wanted.is_some_and(|w| resumable(nodes, chain, w));
        let mut drop: Option<bool> = None;
        let mut slots: Vec<Option<Done>> = (0..n).map(|_| None).collect();
        // a file done: its maps only written not kept when they would take too much memory (told by the first)
        let mut settle = |i: usize, mut d: Done| {
            if let (Ok(outs), false) = (&mut d.outputs, d.skipped) {
                let drop = *drop.get_or_insert_with(|| {
                    let bytes: usize = droppable.iter().map(|(j, p)| outs[*j].get(p).map_or(0, bytes_of)).sum();
                    bytes.saturating_mul(n) > KEPT_BYTES
                });
                if drop {
                    for (j, p) in &droppable {
                        outs[*j].remove(p);
                    }
                }
            }
            slots[i] = Some(d);
        };
        let workers = chain.workers.min(n).max(1);
        if workers == 1 {
            for (i, (name, file)) in chain.files.iter().enumerate() {
                if obs.cancelled() {
                    return Err(Error::Cancelled);
                }
                obs.progress(
                    at,
                    &terraflux_ops::Progress {
                        fraction: Some(i as f64 / n as f64),
                        text: format!("{name} ({} of {n})", i + 1),
                    },
                );
                let d = self.run_file(nodes, chain, &source, env, inputs, prefix, name, file, resumable, obs);
                if matches!(d.outputs, Err(Error::Cancelled)) {
                    return Err(Error::Cancelled);
                }
                settle(i, d);
            }
        } else {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .map_err(|e| Error::Node { at: at.into(), message: e.to_string() })?;
            let stop = AtomicBool::new(false);
            let (tx, rx) = std::sync::mpsc::channel::<(usize, Done)>();
            std::thread::scope(|scope| {
                let (stop, source) = (&stop, &source);
                scope.spawn(move || {
                    pool.install(|| {
                        (0..n).into_par_iter().for_each_with(tx, |tx, i| {
                            if stop.load(Ordering::Relaxed) {
                                return;
                            }
                            let (name, file) = &chain.files[i];
                            let mut relay = Relay { stop };
                            let d = self
                                .run_file(nodes, chain, source, env, inputs, prefix, name, file, resumable, &mut relay);
                            let _ = tx.send((i, d));
                        })
                    })
                });
                let mut done = 0;
                loop {
                    match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                        Ok((i, d)) => {
                            done += 1;
                            obs.progress(
                                at,
                                &terraflux_ops::Progress {
                                    fraction: Some(done as f64 / n as f64),
                                    text: format!("{done} of {n} files ({} done)", chain.files[i].0),
                                },
                            );
                            settle(i, d);
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            if obs.cancelled() {
                                stop.store(true, Ordering::Relaxed);
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
            if stop.load(Ordering::Relaxed)
                || slots.iter().flatten().any(|d| matches!(d.outputs, Err(Error::Cancelled)))
            {
                return Err(Error::Cancelled);
            }
        }
        let drop = drop.unwrap_or(false);
        let mut kept: Vec<Vec<Outputs>> = vec![Vec::new(); chain.nodes.len()];
        let mut names: Vec<String> = Vec::with_capacity(n);
        let mut failed: Vec<(String, Error)> = Vec::new();
        let mut told: Vec<(String, String, Vec<String>)> = Vec::new();
        let mut skipped = 0;
        for ((name, _), d) in chain.files.iter().zip(slots) {
            let Some(d) = d else { continue };
            for (w_at, w) in d.warnings {
                match told.iter_mut().find(|(a, t, _)| *a == w_at && *t == w) {
                    Some((_, _, whose)) => whose.push(name.clone()),
                    None => told.push((w_at, w, vec![name.clone()])),
                }
            }
            match d.outputs {
                Ok(outs) => {
                    skipped += usize::from(d.skipped);
                    for (j, o) in outs.into_iter().enumerate() {
                        kept[j].push(o);
                    }
                    names.push(name.clone());
                }
                Err(e) => failed.push((name.clone(), e)),
            }
        }
        for (w_at, w, whose) in told {
            let w = format!("{w} ({})", if whose.len() == n { "every file".into() } else { listed(&whose) });
            obs.warning(&w_at, &w);
            warnings.push(format!("{w_at}: {w}"));
        }
        if names.is_empty()
            && let Some((name, e)) = failed.drain(..).next()
        {
            return Err(match e {
                Error::Node { at, message } => {
                    Error::Node { at, message: format!("{name:?} (and every other of the {n} files): {message}") }
                }
                e => e,
            });
        }
        let mut tell = |w: String| {
            obs.warning(at, &w);
            warnings.push(format!("{at}: {w}"));
        };
        if !failed.is_empty() {
            let shown: Vec<String> = failed.iter().take(5).map(|(name, e)| format!("{name} ({e})")).collect();
            let more = if failed.len() > 5 { format!("; and {} more", failed.len() - 5) } else { String::new() };
            tell(format!("{} of the {n} files failed, the others went on: {}{more}", failed.len(), shown.join("; ")));
        }
        if skipped > 0 {
            tell(format!(
                "{skipped} of the {n} files were done before with the same nodes and settings (their records): not \
                 run again (delete a file's record, .json, to run it again)"
            ));
        }
        let mut partial = BTreeSet::new();
        if drop {
            tell(format!(
                "the maps of the {n} files written by the chain were not kept in memory (they would take more than \
                 {} GB): see them in the files written",
                KEPT_BYTES >> 30
            ));
        }
        if drop || skipped > 0 {
            partial.extend(droppable.iter().map(|(j, _)| chain.nodes[*j].id.clone()));
        }
        let out = chain
            .nodes
            .iter()
            .zip(kept)
            .map(|(m, outs)| (m.id.clone(), gather(outs, &names, false, "layer")))
            .collect();
        Ok((out, partial))
    }

    /// One file through a chain, in place of the folder: its nodes in order on it. What each node gave (by its
    /// place in the chain) or the error, and the warnings told. `resumable`: taken from its record when it was
    /// done before ([`done_before`]); a chain that writes files records it ([`record`]).
    #[allow(clippy::too_many_arguments)]
    fn run_file(
        &self,
        nodes: &[Node],
        chain: &Chain,
        source: &Ref,
        env: &Env,
        inputs: &BTreeMap<String, Value>,
        prefix: &str,
        name: &str,
        file: &Value,
        resumable: bool,
        obs: &mut dyn Observer,
    ) -> Done {
        let print = print_of(chain, file);
        if resumable && let Some(outs) = print.as_deref().and_then(|p| done_before(chain, name, p)) {
            return Done { outputs: Ok(outs), warnings: Vec::new(), skipped: true };
        }
        let started = std::time::Instant::now();
        let mut child = env.child(BTreeMap::new());
        let mut given = None;
        match source {
            Ref::Port { node, port } => {
                let mut o = env.outputs(node).cloned().unwrap_or_default();
                o.insert(port.clone(), file.clone());
                child.results.insert(node.clone(), o);
            }
            Ref::Input(k) => {
                let mut x = inputs.clone();
                x.insert(k.clone(), file.clone());
                given = Some(x);
            }
            Ref::Carried(k) => {
                child.carried.insert(k.clone(), file.clone());
            }
        }
        let given = given.as_ref().unwrap_or(inputs);
        let mut muffled = Muffled { inner: obs, warnings: Vec::new() };
        for m in &chain.nodes {
            let at_m = format!("{prefix}{}", m.id);
            let m = with_product(m, nodes);
            let m = if m.op == "raster.write" { written_as(&m, name) } else { m };
            let mut ws = Vec::new();
            match self.run_node(&m, &child, given, &at_m, &mut muffled, &mut ws) {
                Ok(out) => {
                    child.results.insert(m.id.clone(), out);
                }
                Err(e) => return Done { outputs: Err(e), warnings: muffled.warnings, skipped: false },
            }
        }
        let outs: Vec<Outputs> = chain.nodes.iter().map(|m| child.results.remove(&m.id).unwrap_or_default()).collect();
        if let Some(p) = &print {
            record(chain, name, file, p, &outs, started.elapsed().as_secs_f64());
        }
        Done { outputs: Ok(outs), warnings: muffled.warnings, skipped: false }
    }

    fn run_node(
        &self,
        node: &Node,
        env: &Env,
        inputs: &BTreeMap<String, Value>,
        at: &str,
        obs: &mut dyn Observer,
        warnings: &mut Vec<String>,
    ) -> Result<Outputs> {
        let given;
        let node = if node.params.values().any(|v| terraflux_graph::setting_ref(v).is_some()) {
            given = self.with_settings(node, env, inputs, at)?;
            &given
        } else {
            node
        };
        let mut values = BTreeMap::new();
        for (port, src) in &node.inputs {
            values.insert(port.clone(), self.resolve(src, env, inputs, &format!("{at}.{port}"))?);
        }
        let sig = self.registry.sig(&node.op).cloned();
        if node.bypass {
            return Ok(bypassed(sig.as_deref(), values));
        }
        // many layers where a port takes one map (a stack's, a series of maps', the bands of rasters on disk): the
        // node runs once per layer
        if let Some(s) = &sig {
            let many: Vec<(String, Vec<(String, Value)>)> = values
                .iter()
                .filter(|(p, _)| s.per_layer(p))
                .filter_map(|(p, v)| terraflux_ops::files::layers(v).map(|l| (p.clone(), l)))
                .collect();
            if !many.is_empty() {
                return self.per_layer(node, s, values, many, at, obs, warnings);
            }
        }
        if node.op == terraflux_graph::EACH {
            return self.each(node, env, values, inputs, at, obs, warnings);
        }
        if node.op == terraflux_graph::SWITCH {
            return self.switch(node, env, values, inputs, at, obs, warnings);
        }
        if let Some(u) = node.uses.as_ref().filter(|_| node.op == terraflux_graph::GROUP) {
            user_inputs(u, &mut values, at)?;
        }
        self.read_rasters(sig.as_deref(), &mut values, at, obs, warnings)?;
        if node.op == "control.repeat" {
            return self.repeat(node, env, values, inputs, at, obs, warnings);
        }
        if node.op == terraflux_graph::GROUP {
            return self.group(node, env, values, inputs, at, obs, warnings);
        }
        let (out, warned) = self.run_op(node, values, at, obs, None)?;
        for w in warned {
            obs.warning(at, &w);
            warnings.push(format!("{at}: {w}"));
        }
        // an output missing matters only to what reads it (some are given only with some settings or inputs): its
        // reader is told, by `resolve`
        Ok(out)
    }

    /// Rasters on disk read whole where a port takes a map or a stack (a container's values: maps); told when a
    /// port of one map is given many bands and takes only the one chosen.
    fn read_rasters(
        &self,
        sig: Option<&terraflux_graph::OpSignature>,
        values: &mut BTreeMap<String, Value>,
        at: &str,
        obs: &mut dyn Observer,
        warnings: &mut Vec<String>,
    ) -> Result<()> {
        for (port, v) in values.iter_mut() {
            let Value::Raster(r) = v else { continue };
            let want = sig.and_then(|s| s.input_type(port)).unwrap_or(terraflux_graph::Type::Map);
            if want == terraflux_graph::Type::Map && r.layers.len() > 1 && !r.picked {
                let w = format!(
                    "{port} takes one map: of the {} layers given, only {:?} is used (raster.read's band chooses it)",
                    r.layers.len(),
                    r.layers[r.chosen].name
                );
                obs.warning(at, &w);
                warnings.push(format!("{at}: {w}"));
            }
            let raster = std::mem::replace(v, Value::Bool(false));
            *v = terraflux_ops::files::as_port(raster, want)
                .map_err(|e| Error::Node { at: format!("{at}.{port}"), message: e.to_string() })?;
        }
        Ok(())
    }

    /// An operator run on its values, with its warnings; `part`: the layer it runs on (its place, the number of
    /// layers, its name), which its progress tells.
    fn run_op(
        &self,
        node: &Node,
        values: BTreeMap<String, Value>,
        at: &str,
        obs: &mut dyn Observer,
        part: Option<(usize, usize, &str)>,
    ) -> Result<(Outputs, Vec<String>)> {
        let op = self
            .registry
            .get(&node.op)
            .ok_or_else(|| Error::Node { at: at.into(), message: format!("unknown operator {:?}", node.op) })?;
        let args = Args { node: at, inputs: values, params: &node.params };
        // the operator told to report how far it is, and whether the run was stopped
        let (out, warned, stopped) = {
            let mut watch = |p: Option<&terraflux_ops::Progress>| {
                match (p, part) {
                    (Some(p), None) => obs.progress(at, p),
                    (Some(p), Some((i, n, name))) => obs.progress(
                        at,
                        &terraflux_ops::Progress {
                            fraction: Some((i as f64 + p.fraction.unwrap_or(0.0)) / n as f64),
                            text: format!("{name} ({} of {n}): {}", i + 1, p.text),
                        },
                    ),
                    (None, _) => {}
                }
                !obs.cancelled()
            };
            let mut ctx = Ctx::watched(&mut watch);
            let out = op.run(&mut ctx, &args);
            (out, std::mem::take(&mut ctx.warnings), ctx.was_stopped())
        };
        let out = out.map_err(|e| {
            if stopped { Error::Cancelled } else { Error::Node { at: at.into(), message: e.to_string() } }
        })?;
        Ok((out, warned))
    }

    /// A node given many layers where it takes one map, run once per layer: the layers of its ports given many
    /// paired (as many on each), the maps given alone taken by every run. Its outputs gathered ([`gather`]); a
    /// warning told once, with the layers that gave it.
    #[allow(clippy::too_many_arguments)]
    fn per_layer(
        &self,
        node: &Node,
        sig: &terraflux_graph::OpSignature,
        values: BTreeMap<String, Value>,
        many: Vec<(String, Vec<(String, Value)>)>,
        at: &str,
        obs: &mut dyn Observer,
        warnings: &mut Vec<String>,
    ) -> Result<Outputs> {
        let n = many[0].1.len();
        if let Some((p, l)) = many.iter().find(|(_, l)| l.len() != n) {
            return Err(Error::Node {
                at: at.into(),
                message: format!(
                    "it runs once per layer, pairing the layers of its inputs: {} has {n}, {p} has {} (give them as \
                     many layers, or one map, which every layer takes)",
                    many[0].0,
                    l.len()
                ),
            });
        }
        let names: Vec<String> = many[0].1.iter().map(|(name, _)| name.clone()).collect();
        // a loop's series (not maps of different grids by their names) gives a series
        let series = many.iter().any(|(p, _)| {
            matches!(values.get(p), Some(Value::Series(s)) if s.first().and_then(terraflux_ops::files::layer_name).is_none())
        });
        let mut outs = Vec::with_capacity(n);
        let mut told: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, name) in names.iter().enumerate() {
            if obs.cancelled() {
                return Err(Error::Cancelled);
            }
            obs.progress(
                at,
                &terraflux_ops::Progress {
                    fraction: Some(i as f64 / n as f64),
                    text: format!("{name} ({} of {n})", i + 1),
                },
            );
            let mut v = values.clone();
            for (p, l) in &many {
                v.insert(p.clone(), l[i].1.clone());
            }
            let layer = |e: Error| match e {
                Error::Node { at, message } => {
                    Error::Node { at, message: format!("layer {name:?} ({} of {n}): {message}", i + 1) }
                }
                e => e,
            };
            self.read_rasters(Some(sig), &mut v, at, obs, warnings).map_err(layer)?;
            let (out, warned) = self.run_op(node, v, at, obs, Some((i, n, name))).map_err(layer)?;
            for w in warned {
                match told.iter_mut().find(|(t, _)| *t == w) {
                    Some((_, ls)) => ls.push(i),
                    None => told.push((w, vec![i])),
                }
            }
            outs.push(out);
        }
        for (w, ls) in told {
            let whose = if ls.len() == n {
                "every layer".to_string()
            } else {
                let some: Vec<&str> = ls.iter().map(|&i| names[i].as_str()).collect();
                format!("layer{} {}", if some.len() > 1 { "s" } else { "" }, some.join(", "))
            };
            let w = format!("{w} ({whose})");
            obs.warning(at, &w);
            warnings.push(format!("{at}: {w}"));
        }
        Ok(gather(outs, &names, series, "layer"))
    }

    /// A group: its body once, its inputs carried inside, its outputs from `body.outputs`.
    #[allow(clippy::too_many_arguments)]
    fn group(
        &self,
        node: &Node,
        env: &Env,
        values: BTreeMap<String, Value>,
        inputs: &BTreeMap<String, Value>,
        at: &str,
        obs: &mut dyn Observer,
        warnings: &mut Vec<String>,
    ) -> Result<Outputs> {
        let body = node.body.as_ref().ok_or_else(|| Error::Node { at: at.into(), message: "no body".into() })?;
        let mut child = env.child(values);
        // in a scope kept: its body's nodes kept too, by what its inputs came from (the group without its body)
        child.kept = env.kept.and_then(|k| {
            let outer = Node { body: None, ..node.clone() };
            scoped_print(&outer, env, &k, inputs).map(|print| Scope { print, ..k })
        });
        self.run_scope(&body.nodes, &mut child, inputs, at, obs, warnings)?;
        if let Some(k) = child.kept {
            // (what was kept of nodes no longer in its body: let go)
            let within = format!("{at}/");
            k.cache.lock().unwrap().inner.retain(|p, _| {
                p.strip_prefix(&within)
                    .is_none_or(|rest| body.nodes.iter().any(|n| rest.split('/').next() == Some(n.id.as_str())))
            });
        }
        let mut out = Outputs::new();
        for (name, r) in &body.outputs {
            out.insert(
                name.clone(),
                self.resolve(&Source::Ref(r.clone()), &child, inputs, &format!("{at}/outputs.{name}"))?,
            );
        }
        // what the body's nodes gave too, as `node.port` (no reference names them: the application shows a group's
        // steps with them), its random generators aside (a group holding one would never be kept)
        for (id, outs) in &child.results {
            for (port, v) in outs {
                if !holds_rng(v) {
                    out.entry(format!("{id}.{port}")).or_insert_with(|| v.clone());
                }
            }
        }
        Ok(out)
    }

    /// A switch: the branch of its case run (a group of its body: the one named as the case, `case_<n>` for a
    /// number, else `otherwise`), its outputs the switch's, and the id of the branch.
    #[allow(clippy::too_many_arguments)]
    fn switch(
        &self,
        node: &Node,
        env: &Env,
        mut values: BTreeMap<String, Value>,
        inputs: &BTreeMap<String, Value>,
        at: &str,
        obs: &mut dyn Observer,
        warnings: &mut Vec<String>,
    ) -> Result<Outputs> {
        let body = node.body.as_ref().ok_or_else(|| Error::Node { at: at.into(), message: "no body".into() })?;
        let case = values.remove("case").ok_or_else(|| Error::Node { at: at.into(), message: "no case".into() })?;
        let text = match &case {
            Value::Text(t) => t.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Json(serde_json::Value::String(s)) => s.clone(),
            v => match v.as_number() {
                Some(x) => terraflux_ops::values::number_text(x),
                None => {
                    return Err(Error::Node {
                        at: format!("{at}.case"),
                        message: format!("the case is a text, a number or true or false, not a {:?}", v.ty()),
                    });
                }
            },
        };
        let wanted = [text.clone(), format!("case_{text}")];
        let branch = body
            .nodes
            .iter()
            .find(|n| wanted.contains(&n.id))
            .or_else(|| body.nodes.iter().find(|n| n.id == "otherwise"))
            .ok_or_else(|| {
                let ids: Vec<&str> = body.nodes.iter().map(|n| n.id.as_str()).collect();
                Error::Node {
                    at: at.into(),
                    message: format!(
                        "no branch for the case {text:?} (its branches: {}; otherwise for the rest)",
                        ids.join(", ")
                    ),
                }
            })?;
        self.read_rasters(None, &mut values, at, obs, warnings)?;
        let child = env.child(values);
        let inner = format!("{at}/{}", branch.id);
        obs.node(&inner);
        let mut out = self.run_node(branch, &child, inputs, &inner, obs, warnings)?;
        out.insert("branch".into(), Value::Text(branch.id.clone()));
        Ok(out)
    }

    /// A loop over a collection: the body once per element (`@item`, `@index`, `@name`; the node's other inputs as
    /// a group's), what it collects gathered as a node run once per layer gathers its outputs ([`gather`]).
    #[allow(clippy::too_many_arguments)]
    fn each(
        &self,
        node: &Node,
        env: &Env,
        mut values: BTreeMap<String, Value>,
        inputs: &BTreeMap<String, Value>,
        at: &str,
        obs: &mut dyn Observer,
        warnings: &mut Vec<String>,
    ) -> Result<Outputs> {
        let body = node.body.as_ref().ok_or_else(|| Error::Node { at: at.into(), message: "no body".into() })?;
        let over = values
            .remove("over")
            .ok_or_else(|| Error::Node { at: at.into(), message: "nothing to go over (over)".into() })?;
        let kind = over.ty();
        let items = elements(over).ok_or_else(|| Error::Node {
            at: format!("{at}.over"),
            message: format!("control.each goes over a stack, a series, a table, vectors or a list, not a {kind:?}"),
        })?;
        self.read_rasters(None, &mut values, at, obs, warnings)?;
        let n = items.len();
        let names: Vec<String> = items.iter().map(|(name, _)| name.clone()).collect();
        let mut gathered = Vec::with_capacity(n);
        for (i, (name, item)) in items.into_iter().enumerate() {
            if obs.cancelled() {
                return Err(Error::Cancelled);
            }
            let item = match item {
                Value::Raster(r) => {
                    Value::Map(r.map().map_err(|e| Error::Node { at: at.into(), message: e.to_string() })?)
                }
                v => v,
            };
            let mut c = values.clone();
            c.insert("item".into(), item);
            c.insert("index".into(), Value::Integer(i as i64 + 1));
            c.insert("name".into(), Value::Text(name.clone()));
            let mut child = env.child(c);
            let element = |e: Error| match e {
                Error::Node { at, message } => {
                    Error::Node { at, message: format!("{name:?} ({} of {n}): {message}", i + 1) }
                }
                e => e,
            };
            self.run_scope(&body.nodes, &mut child, inputs, at, obs, warnings).map_err(element)?;
            obs.body(at, i + 1, &child.results);
            let mut now = Outputs::new();
            for (k, r) in &body.collect {
                let v = self
                    .resolve(&Source::Ref(r.clone()), &child, inputs, &format!("{at}/collect.{k}"))
                    .map_err(element)?;
                now.insert(k.clone(), v);
            }
            obs.collected(at, i + 1, n, &now);
            gathered.push(now);
            obs.step(at, i + 1, n);
        }
        Ok(gather(gathered, &names, false, "item"))
    }

    #[allow(clippy::too_many_arguments)]
    fn repeat(
        &self,
        node: &Node,
        env: &Env,
        mut values: BTreeMap<String, Value>,
        inputs: &BTreeMap<String, Value>,
        at: &str,
        obs: &mut dyn Observer,
        warnings: &mut Vec<String>,
    ) -> Result<Outputs> {
        let body = node.body.as_ref().ok_or_else(|| Error::Node { at: at.into(), message: "no body".into() })?;
        let times = values
            .remove("times")
            .and_then(|v| v.as_number())
            .filter(|t| t.fract() == 0.0 && *t >= 0.0)
            .ok_or_else(|| Error::Node { at: at.into(), message: "times must be a whole number >= 0".into() })?
            as usize;
        let mut carried: BTreeMap<String, Value> = BTreeMap::new();
        for name in body.carry.keys() {
            let v = values
                .remove(name)
                .ok_or_else(|| Error::Node { at: at.into(), message: format!("no first value for @{name}") })?;
            carried.insert(name.clone(), v);
        }
        let mut series: BTreeMap<String, Vec<Value>> = body.collect.keys().map(|k| (k.clone(), Vec::new())).collect();
        for step in 1..=times {
            let mut c = carried.clone();
            c.insert("step".into(), Value::Integer(step as i64));
            let mut child = env.child(c);
            self.run_scope(&body.nodes, &mut child, inputs, at, obs, warnings)?;
            obs.body(at, step, &child.results);
            if let Some(r) = &body.until {
                let stop = self.resolve(&Source::Ref(r.clone()), &child, inputs, &format!("{at}/until"))?;
                match stop {
                    Value::Bool(true) => break,
                    Value::Bool(false) => {}
                    v => {
                        return Err(Error::Node {
                            at: at.into(),
                            message: format!("until must be a yes or no, not a {:?}", v.ty()),
                        });
                    }
                }
            }
            let mut now = BTreeMap::new();
            for (name, r) in &body.collect {
                let v = self.resolve(&Source::Ref(r.clone()), &child, inputs, &format!("{at}/collect.{name}"))?;
                series.get_mut(name).unwrap().push(v.clone());
                now.insert(name.clone(), v);
            }
            obs.collected(at, step, times, &now);
            let mut next = BTreeMap::new();
            for (name, r) in &body.carry {
                next.insert(
                    name.clone(),
                    self.resolve(&Source::Ref(r.clone()), &child, inputs, &format!("{at}/carry.{name}"))?,
                );
            }
            carried = next;
            obs.step(at, step, times);
        }
        let mut out: Outputs = carried;
        for (name, s) in series {
            out.insert(name, Value::Series(s));
        }
        Ok(out)
    }
}

/// The elements of a collection, by name: the layers of a stack or of rasters on disk, the maps of a series, the
/// items of any other series (named 1, 2...), the rows of a table (each a table of one row, named by its first
/// column), vectors' features (each alone, named by their attribute `name` when it is a text, else 1, 2...) and
/// the values of a list (named by them); none for anything else.
fn elements(v: Value) -> Option<Vec<(String, Value)>> {
    if let Some(l) = terraflux_ops::files::layers(&v) {
        return Some(l);
    }
    let number = |x: f64| if x.fract() == 0.0 && x.abs() < 1e15 { format!("{}", x as i64) } else { x.to_string() };
    Some(match v {
        Value::Series(items) => items.into_iter().enumerate().map(|(i, v)| ((i + 1).to_string(), v)).collect(),
        Value::Table(t) => t
            .rows
            .iter()
            .map(|row| {
                let name = row.first().map_or(String::new(), |x| number(*x));
                (name, Value::Table(terraflux_ops::Table { columns: t.columns.clone(), rows: vec![row.clone()] }))
            })
            .collect(),
        Value::Vector(v) => {
            let names = v.attributes.iter().find_map(|(n, c)| match c {
                terraflux_raster::vector::Column::Texts(t) if n == "name" => Some(t.clone()),
                _ => None,
            });
            (0..v.len())
                .map(|i| {
                    let name = names.as_ref().map_or_else(|| (i + 1).to_string(), |t| t[i].clone());
                    (name, Value::Vector(std::sync::Arc::new(v.select(&[i]))))
                })
                .collect()
        }
        Value::Json(serde_json::Value::Array(a)) => a
            .iter()
            .map(|x| {
                let name = match x {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), number),
                    x => x.to_string(),
                };
                (name, Value::from_json(x))
            })
            .collect(),
        _ => return None,
    })
}

/// The outputs of a node run once per layer, by output: its maps a stack of the layers' names (a series when it
/// was given a loop's series, or when they are of different grids, each map then named,
/// [`terraflux_ops::files::named`]), its tables one table ([`tables`]), its other
/// values a series. An output some layers did not give is left out.
fn gather(outs: Vec<Outputs>, names: &[String], series: bool, column: &str) -> Outputs {
    let mut gathered = Outputs::new();
    let Some(first) = outs.first() else { return gathered };
    for k in first.keys().filter(|k| outs.iter().all(|o| o.contains_key(*k))) {
        let all: Vec<&Value> = outs.iter().map(|o| &o[k]).collect();
        let maps: Option<Vec<&terraflux_ops::Map>> =
            all.iter().map(|v| if let Value::Map(m) = v { Some(m) } else { None }).collect();
        let v = match maps {
            Some(maps) if !series && maps.iter().all(|m| m.grid.same_as(&maps[0].grid)) => {
                Value::Stack(terraflux_ops::Stack {
                    grid: maps[0].grid.clone(),
                    keys: names.iter().map(|n| terraflux_ops::Key::Name(n.clone())).collect(),
                    layers: maps.iter().map(|m| m.data.clone()).collect(),
                })
            }
            // of different grids (the files of a folder of tiles, of regions): a series, each map with its name
            Some(maps) if !series => Value::Series(
                maps.iter().zip(names).map(|(m, n)| Value::Map(terraflux_ops::files::named(m, n))).collect(),
            ),
            _ if all.iter().all(|v| matches!(v, Value::Table(_))) => Value::Table(tables(&all, names, column)),
            _ => Value::Series(all.into_iter().cloned().collect()),
        };
        gathered.insert(k.clone(), v);
    }
    gathered
}

/// The tables of the layers as one, each row after a column `name` (`layer`, or `item` of control.each; `input_<name>` when they have one): the
/// layers' names when they are numbers, else the number each ends with when they all end with a different one
/// (the year of `forest_2010`), else their places (1, 2...).
fn tables(all: &[&Value], names: &[String], name: &str) -> terraflux_ops::Table {
    let trailing = |n: &str| {
        let digits: String = n.chars().rev().take_while(char::is_ascii_digit).collect();
        digits.chars().rev().collect::<String>().parse::<f64>().ok()
    };
    let distinct = |v: &Vec<f64>| v.iter().enumerate().all(|(i, x)| !v[..i].contains(x));
    let at: Vec<f64> = names
        .iter()
        .map(|n| n.trim().parse::<f64>().ok())
        .collect::<Option<Vec<f64>>>()
        .or_else(|| names.iter().map(|n| trailing(n)).collect::<Option<Vec<f64>>>().filter(distinct))
        .unwrap_or_else(|| (1..=names.len()).map(|i| i as f64).collect());
    let parts: Vec<terraflux_ops::Table> = all
        .iter()
        .zip(&at)
        .filter_map(|(v, &x)| {
            let Value::Table(t) = v else { return None };
            let column = if t.columns.iter().any(|c| c == name) { format!("input_{name}") } else { name.to_string() };
            Some(terraflux_ops::Table {
                columns: std::iter::once(column.to_string()).chain(t.columns.iter().cloned()).collect(),
                rows: t.rows.iter().map(|r| std::iter::once(x).chain(r.iter().copied()).collect()).collect(),
            })
        })
        .collect();
    terraflux_ops::Table::concat(&parts)
}

/// raster.write of a map an operator made as a product of GuidosToolbox (MSPA, FAD...): the product's tag given to
/// it (written in the GeoTIFF as landschaft_product, as landschaft does), unless the node gives one.
fn with_product<'a>(node: &'a Node, nodes: &[Node]) -> std::borrow::Cow<'a, Node> {
    use std::borrow::Cow;
    if node.op != "raster.write" || node.params.contains_key("product") {
        return Cow::Borrowed(node);
    }
    let Some(terraflux_graph::Source::Ref(r)) = node.inputs.get("value") else { return Cow::Borrowed(node) };
    let Ok(terraflux_graph::Ref::Port { node: from, port }) = terraflux_graph::Ref::parse(r) else {
        return Cow::Borrowed(node);
    };
    let tag = nodes
        .iter()
        .find(|n| n.id == from)
        .and_then(|p| terraflux_ops::landscape::Product::of(&p.op, &port, &p.params))
        .and_then(|p| p.tag());
    match tag {
        Some(t) => {
            let mut n = node.clone();
            n.params.insert("product".into(), t.into());
            Cow::Owned(n)
        }
        None => Cow::Borrowed(node),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terraflux_graph::{Access, Body, OpSignature, PortSpec, Type};
    use terraflux_ops::{Operator, Result as OpResult};

    /// `test.add`: x + y.
    struct Add;

    impl Operator for Add {
        fn signature(&self) -> OpSignature {
            OpSignature::new("test.add", "", Access::None)
                .input(PortSpec::new("x", Type::Number, ""))
                .input(PortSpec::new("y", Type::Number, ""))
                .output(PortSpec::new("sum", Type::Number, ""))
        }
        fn run(&self, _ctx: &mut Ctx, a: &Args) -> OpResult<Outputs> {
            Ok([("sum".to_string(), Value::Number(a.number("x")? + a.number("y")?))].into())
        }
    }

    /// `test.over`: x > limit.
    struct Over;

    impl Operator for Over {
        fn signature(&self) -> OpSignature {
            OpSignature::new("test.over", "", Access::None)
                .input(PortSpec::new("x", Type::Number, ""))
                .input(PortSpec::new("limit", Type::Number, ""))
                .output(PortSpec::new("over", Type::Bool, ""))
        }
        fn run(&self, _ctx: &mut Ctx, a: &Args) -> OpResult<Outputs> {
            Ok([("over".to_string(), Value::Bool(a.number("x")? > a.number("limit")?))].into())
        }
    }

    /// `test.wait`: says how far it is, then waits until the run is stopped (or 10 s).
    struct Wait;

    impl Operator for Wait {
        fn signature(&self) -> OpSignature {
            OpSignature::new("test.wait", "", Access::None).output(PortSpec::new("done", Type::Bool, ""))
        }
        fn run(&self, ctx: &mut Ctx, _a: &Args) -> OpResult<Outputs> {
            let t = std::time::Instant::now();
            ctx.progress(Some(0.5), "half way");
            while t.elapsed() < std::time::Duration::from_secs(10) {
                if ctx.stopped() {
                    return Err(terraflux_ops::Error::msg("stopped"));
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Ok([("done".to_string(), Value::Bool(true))].into())
        }
    }

    fn engine() -> Engine {
        let mut r = Registry::standard();
        r.add(Arc::new(Add));
        r.add(Arc::new(Over));
        r.add(Arc::new(Wait));
        Engine { registry: r }
    }

    /// A model run on a thread of its own (as the application and the command line run it) has the process's
    /// threads for its kernels and solvers, not OpenMP's one of a new thread.
    #[test]
    fn a_run_in_its_own_thread_has_the_threads() {
        let n = terraflux_ops::threads::ensure();
        let seen = std::thread::spawn(|| {
            let mut m = Model::new("t");
            m.nodes.push(Node::new("a", "test.add").literal("x", 1).literal("y", 2));
            engine().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
            terraflux_ops::threads::current()
        })
        .join()
        .unwrap();
        assert_eq!(seen, n);
    }

    /// An operator's progress reaches the observer; stopped while it runs, the run ends stopped, not failed.
    #[test]
    fn a_node_reports_and_stops_in_the_middle() {
        struct Stopping {
            told: Vec<(String, terraflux_ops::Progress)>,
        }
        impl Observer for Stopping {
            fn progress(&mut self, at: &str, p: &terraflux_ops::Progress) {
                self.told.push((at.to_string(), p.clone()));
            }
            // stopped once the node said how far it is
            fn cancelled(&mut self) -> bool {
                !self.told.is_empty()
            }
        }
        let mut m = Model::new("t");
        m.nodes.push(Node::new("w", "test.wait"));
        let mut obs = Stopping { told: Vec::new() };
        let t = std::time::Instant::now();
        let r = engine().run(&m, &BTreeMap::new(), &mut obs);
        assert!(matches!(r, Err(Error::Cancelled)), "{:?}", r.err());
        assert!(t.elapsed() < std::time::Duration::from_secs(5));
        assert_eq!(obs.told[0].0, "w");
        assert_eq!(obs.told[0].1.fraction, Some(0.5));
        assert_eq!(obs.told[0].1.text, "half way");
    }

    #[test]
    fn a_loop_ends_where_until_is_true() {
        // total = sum of steps, ending at the step that would take it over 5: steps 1 and 2 kept (3), step 3 not
        let mut m = Model::new("t");
        m.nodes.push(Node::new("loop", "control.repeat").literal("times", 10).literal("total", 0).body(Body {
            nodes: vec![
                Node::new("acc", "test.add").input("x", "@total").input("y", "@step"),
                Node::new("stop", "test.over").input("x", "acc.sum").literal("limit", 5),
            ],
            carry: [("total".to_string(), "acc.sum".to_string())].into(),
            collect: [("each".to_string(), "acc.sum".to_string())].into(),
            until: Some("stop.over".into()),
            ..Default::default()
        }));
        let r = engine().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
        assert_eq!(r.get("loop.total").unwrap().as_number(), Some(3.0));
        let Value::Series(s) = r.get("loop.each").unwrap() else { panic!() };
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn runs_in_dependency_order_with_loops() {
        // total = 10 + sum of (step + 1) over 4 steps, carried; each step's value collected
        let mut m = Model::new("t");
        m.inputs.insert("start".into(), terraflux_graph::Input::Value { value: 10.into(), note: None });
        m.nodes.push(Node::new("loop", "control.repeat").input("times", "four.sum").input("total", "$start").body(
            Body {
                nodes: vec![
                    Node::new("inc", "test.add").input("x", "@step").literal("y", 1),
                    Node::new("acc", "test.add").input("x", "@total").input("y", "inc.sum"),
                ],
                carry: [("total".to_string(), "acc.sum".to_string())].into(),
                collect: [("each".to_string(), "acc.sum".to_string())].into(),
                ..Default::default()
            },
        ));
        m.nodes.push(Node::new("four", "test.add").literal("x", 3).literal("y", 1));
        let inputs = [("start".to_string(), Value::Integer(10))].into();
        let r = engine().run(&m, &inputs, &mut Quiet).unwrap();
        assert_eq!(r.get("loop.total").unwrap().as_number(), Some(10.0 + 2.0 + 3.0 + 4.0 + 5.0));
        let Value::Series(s) = r.get("loop.each").unwrap() else { panic!() };
        let v: Vec<f64> = s.iter().map(|x| x.as_number().unwrap()).collect();
        assert_eq!(v, vec![12.0, 15.0, 19.0, 24.0]);
    }

    #[test]
    fn groups_and_submodels() {
        // a group: 3 + 1, then + 10; inside a loop, a group sees the loop's step
        let mut m = Model::new("t");
        m.nodes.push(Node::new("g", terraflux_graph::GROUP).literal("x", 3).body(Body {
            nodes: vec![Node::new("inc", "test.add").input("x", "@x").literal("y", 1)],
            outputs: [("y".to_string(), "inc.sum".to_string())].into(),
            ..Default::default()
        }));
        m.nodes.push(Node::new("after", "test.add").input("x", "g.y").literal("y", 10));
        m.nodes.push(Node::new("loop", "control.repeat").literal("times", 2).literal("total", 0).body(Body {
            nodes: vec![Node::new("in", terraflux_graph::GROUP).input("x", "@total").body(Body {
                nodes: vec![Node::new("add", "test.add").input("x", "@x").input("y", "@step")],
                outputs: [("v".to_string(), "add.sum".to_string())].into(),
                ..Default::default()
            })],
            carry: [("total".to_string(), "in.v".to_string())].into(),
            ..Default::default()
        }));
        let r = engine().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
        assert_eq!(r.get("after.sum").unwrap().as_number(), Some(14.0));
        assert_eq!(r.get("loop.total").unwrap().as_number(), Some(3.0));
        // what the group's nodes gave, kept with its outputs
        assert_eq!(r.values["g"]["inc.sum"].as_number(), Some(4.0));

        // the same group as a submodel file
        let d = std::env::temp_dir().join(format!("terraflux-exec-sub-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("inc.toml"),
            "format = \"terraflux/1\"
[inputs.x]
kind = \"value\"
value = 0
[[nodes]]
id = \"inc\"
op = \"test.add\"
inputs = { x = \"$x\", y = 1 }
[outputs.y]
from = \"inc.sum\"
",
        )
        .unwrap();
        let mut m = Model::new("t");
        m.nodes.push(Node::new("g", terraflux_graph::SUBMODEL).literal("x", 3).param("path", "inc.toml"));
        m.nodes.push(Node::new("after", "test.add").input("x", "g.y").literal("y", 10));
        assert!(engine().run(&m, &BTreeMap::new(), &mut Quiet).is_err(), "submodels are expanded first");
        terraflux_graph::submodel::expand(&mut m, &d).unwrap();
        let r = engine().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
        assert_eq!(r.get("after.sum").unwrap().as_number(), Some(14.0));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn map_algebra() {
        let g = Arc::new(terraflux_raster::Grid::new(2, 2));
        let land = Value::Map(terraflux_ops::Map::new(g, vec![1.0, 2.0, 3.0, f64::NAN]));
        let mut m = Model::new("t");
        m.inputs.insert(
            "land".into(),
            terraflux_graph::Input::Map { path: "x.tif".into(), band: None, nodata: None, note: None },
        );
        m.nodes.push(
            Node::new("calc", "raster.calc")
                .input("lu", "$land")
                .literal("k", 10)
                .param("expression", "lu * k if lu >= 2 else 0"),
        );
        let r = engine().run(&m, &[("land".to_string(), land)].into(), &mut Quiet).unwrap();
        let Value::Map(out) = r.get("calc.map").unwrap() else { panic!() };
        assert_eq!(out.data[..3], [0.0, 20.0, 30.0]);
        assert!(out.data[3].is_nan());
        // a bad expression names the node
        m.nodes[0].params.insert("expression".into(), "lu +".into());
        let e = engine().run(&m, &BTreeMap::new(), &mut Quiet).err().unwrap().to_string();
        assert!(e.contains("calc") || e.contains("no value"), "{e}");
    }

    #[test]
    fn errors_name_the_node() {
        let mut m = Model::new("t");
        m.nodes.push(Node::new("bad", "markov.fit").literal("t0", 1).literal("t1", 1).literal("steps", 1));
        let e = engine().run(&m, &BTreeMap::new(), &mut Quiet).err().unwrap().to_string();
        assert!(e.contains("bad"), "{e}");
    }

    #[test]
    fn missing_inputs_are_refused() {
        let mut m = Model::new("t");
        m.inputs.insert(
            "x".into(),
            terraflux_graph::Input::Map { path: "x.tif".into(), band: None, nodata: None, note: None },
        );
        assert!(engine().run(&m, &BTreeMap::new(), &mut Quiet).is_err());
        // an input of a value not given: its own value in the model
        let mut m = Model::new("t");
        m.inputs.insert("k".into(), terraflux_graph::Input::Value { value: 1.into(), note: None });
        assert!(engine().run(&m, &BTreeMap::new(), &mut Quiet).is_ok());
    }
}
