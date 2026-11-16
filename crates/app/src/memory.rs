//! The memory, before a run: what the computer has (total and available), and an estimate of what a run needs, so
//! that a run that will not fit is told before it starts and not when the system kills it. The estimate comes from the
//! model and the headers of the rasters it reads (the data are not read): a map in Terraflux's memory is a number of
//! 8 bytes a cell ([`terraflux_exec::bytes_of`] counts so), so each node's maps are its grid's cells x 8 x their
//! layers; the grid of a node is that of its main input (`like`, `map`, `raster`, `stack`, the first it has), a
//! `raster.read`'s that of its file, an `nlm.*`'s its `rows` x `columns`; a table or a vector read from a file is its
//! size on disk (x 2 for a vector: its points take 16 bytes).
//!
//! What it adds up, an upper bound (the engine keeps the outputs of every top-level node in its cache, none is let
//! go while the model runs): the outputs of the top-level nodes (`kept`, what the runner measures as it goes); the
//! maps the model's inputs hold and those a `raster.read` gives to a node that reads it whole (`reads`; none when
//! every node that reads it works by blocks: `block_rows`, `landscape.mspa_tiled`, the metrics of a map of more than
//! 250 million cells); what the bodies of loops keep (`bodies`: the nodes of one iteration, times the iterations but
//! no more than the 1 GiB the cache keeps of them, the rest let go); and the largest node's output again (`scratch`:
//! an operator builds its result while it holds what it reads). What it does not see: nodes with no grid to size them
//! by, tables and vectors an operator makes (they count for nothing), the working memory inside an operator.
//!
//! A run of the same model before is a better measure: the journal keeps what the nodes' outputs held at its end
//! (`Entry::memory`), and it replaces the estimate of `kept` (the rest stays estimated).

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use egui::RichText;
use terraflux_exec::journal::{Entry, Kind, Status};
use terraflux_graph::{Input, Model, Node, Source, Type};
use terraflux_ops::Registry;

/// The bytes of a cell in memory.
const CELL: u64 = 8;

/// The most the nodes of loops' bodies kept per iteration take in the engine's cache ([`terraflux_exec::Cache`]).
const ITER_BYTES: u64 = 1 << 30;

/// The layers of a stack whose layers nothing says.
const STACK_LAYERS: u64 = 3;

/// The cells above which the metrics read a raster by blocks of rows whatever they are told.
const BLOCKED_CELLS: u64 = 250_000_000;

/// The share of the available memory above which a run is told to be tight.
const TIGHT: f64 = 0.8;

/// The memory of the computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ram {
    pub total: u64,
    /// What a program can use now (the system's cache counted as free); the total where the system does not tell.
    pub available: u64,
}

thread_local! {
    /// A memory made up for the thread (the tests of the window).
    static PRETEND: Cell<Option<Ram>> = const { Cell::new(None) };
}

/// Make the computer's memory (for this thread) what is given, or the real again with `None`: for tests.
pub fn pretend(ram: Option<Ram>) {
    PRETEND.with(|p| p.set(ram));
}

/// The memory of the computer now; none when the system does not tell.
pub fn ram() -> Option<Ram> {
    PRETEND.with(|p| p.get()).or_else(query)
}

#[cfg(windows)]
fn query() -> Option<Ram> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: `s` is zeroed memory of the right type with its length set, for the call to fill
    let mut s: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    s.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    let ok = unsafe { GlobalMemoryStatusEx(&mut s) };
    (ok != 0 && s.ullTotalPhys > 0).then_some(Ram { total: s.ullTotalPhys, available: s.ullAvailPhys })
}

/// The memory of `/proc/meminfo`'s text: `MemTotal` and `MemAvailable` (kilobytes), else `MemFree` and the caches.
pub fn parse_meminfo(text: &str) -> Option<Ram> {
    let kb = |key: &str| -> Option<u64> {
        let line = text.lines().find(|l| l.starts_with(key))?;
        line.split_whitespace().nth(1)?.parse::<u64>().ok().map(|k| k * 1024)
    };
    let total = kb("MemTotal:")?;
    let available = match kb("MemAvailable:") {
        Some(a) => a,
        None => kb("MemFree:")? + kb("Buffers:").unwrap_or(0) + kb("Cached:").unwrap_or(0),
    };
    (total > 0).then_some(Ram { total, available: available.min(total) })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn query() -> Option<Ram> {
    parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

#[cfg(target_os = "macos")]
fn query() -> Option<Ram> {
    // the total by sysctl; the system does not say what is available as Linux does (the file cache is counted as
    // free there): the free and speculative pages and the inactive ones are what a program can take
    fn sysctl(name: &str) -> Option<u64> {
        let c = std::ffi::CString::new(name).ok()?;
        let mut v: u64 = 0;
        let mut len = std::mem::size_of::<u64>();
        // SAFETY: `v` and `len` are there for the call to write, `c` is a nul-ended name
        let r =
            unsafe { libc::sysctlbyname(c.as_ptr(), (&mut v as *mut u64).cast(), &mut len, std::ptr::null_mut(), 0) };
        (r == 0).then_some(v)
    }
    let total = sysctl("hw.memsize")?;
    let page = sysctl("vm.pagesize").unwrap_or(16384);
    let pages = sysctl("vm.page_free_count").unwrap_or(0) + sysctl("vm.page_speculative_count").unwrap_or(0);
    let available = if pages > 0 { (pages * page).min(total) } else { total };
    Some(Ram { total, available })
}

#[cfg(not(any(windows, unix)))]
fn query() -> Option<Ram> {
    None
}

/// The grid of a node's maps: its cells and its layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct G {
    cells: u64,
    layers: u64,
}

impl G {
    fn bytes(self) -> u64 {
        self.cells * self.layers * CELL
    }
}

/// The grid of the raster in a file, from its header; none for an address on the web, a file that does not read.
fn header(path: &str, dir: &Path) -> Option<G> {
    if path.is_empty()
        || path.starts_with(['$', '@'])
        || terraflux_graph::is_sent(path)
        || terraflux_graph::is_remote(path)
    {
        return None;
    }
    let at = if Path::new(path).is_absolute() { PathBuf::from(path) } else { dir.join(path) };
    let first: Vec<PathBuf> = terraflux_ops::files::files_of(&at.to_string_lossy()).ok()?.into_iter().take(1).collect();
    let r = terraflux_ops::files::Rasters::open_files(&at.to_string_lossy(), first, None, None).ok()?;
    Some(G { cells: r.grid.width as u64 * r.grid.height as u64, layers: (r.layers.len() as u64).max(1) })
}

/// The bytes of a table or a vector file, as it holds them in memory (about): a table about its size on disk, a
/// vector twice.
fn file_bytes(path: &str, dir: &Path, times: u64) -> Option<u64> {
    if path.is_empty()
        || path.starts_with(['$', '@'])
        || terraflux_graph::is_sent(path)
        || terraflux_graph::is_remote(path)
    {
        return None;
    }
    let at = if Path::new(path).is_absolute() { PathBuf::from(path) } else { dir.join(path) };
    Some(std::fs::metadata(at).ok()?.len() * times)
}

/// What one value of a type takes: a map its cells, a stack its layers' (an `Any` one of a container's output as a
/// map), a table or a vector what was found for its file; the rest nothing (a raster on disk is read when a node
/// asks).
fn type_bytes(ty: Type, g: Option<G>, file: Option<u64>, any: bool) -> u64 {
    let g = g.unwrap_or(G { cells: 0, layers: 1 });
    match ty {
        Type::Map => g.cells * CELL,
        Type::Any if any => g.cells * CELL,
        Type::Stack => g.cells * if g.layers > 1 { g.layers } else { STACK_LAYERS } * CELL,
        Type::Table | Type::Vector => file.unwrap_or(0),
        _ => 0,
    }
}

/// The iterations of a loop (its `times` or `steps`: a number, or a list of dates), 1 when it says none.
fn iterations(n: &Node) -> u64 {
    let mut best = 1;
    for key in ["times", "steps"] {
        let listed = n.params.get(key).and_then(|v| v.as_array()).map(|a| a.len() as u64);
        let given = match n.inputs.get(key) {
            Some(Source::Literal(v)) => v.as_array().map(|a| a.len() as u64).or_else(|| v.as_u64()),
            _ => None,
        };
        best = best.max(listed.or(given).or_else(|| crate::disk::number(n, key)).unwrap_or(1));
    }
    best
}

/// What a run needs of memory, about, and what it is made of ([`estimate`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Estimate {
    /// The outputs of the top-level nodes kept.
    pub kept: u64,
    /// The maps the inputs hold and the rasters read whole.
    pub reads: u64,
    /// What the loops' bodies keep.
    pub bodies: u64,
    /// The largest node's output again.
    pub scratch: u64,
    /// What the nodes of a run before held at its end, in place of `kept`.
    pub measured: Option<u64>,
    /// The nodes that weigh, the heaviest first (their outputs, bodies and reads).
    pub nodes: Vec<(String, u64)>,
    /// The nodes that read a map whole and take `block_rows`, which they do not set.
    pub blockable: Vec<String>,
    /// Whether the model is one the tiled engine runs (Dinamica's: `woe.probability` and `dinamica.step`).
    pub tiled: bool,
    /// The cells of the largest grid.
    pub cells: u64,
}

impl Estimate {
    /// The memory the run needs, about.
    pub fn peak(&self) -> u64 {
        self.measured.unwrap_or(self.kept) + self.reads + self.bodies + self.scratch
    }

    /// The same with what a run before measured in place of the estimate of the nodes' outputs.
    pub fn with_measure(mut self, kept: u64) -> Estimate {
        self.measured = Some(kept);
        self
    }

    /// Where the number comes from, in words.
    pub fn source(&self) -> &'static str {
        if self.measured.is_some() { "from the run before" } else { "estimated from the model" }
    }

    /// What to do when it does not fit, from the model: the nodes that weigh, `block_rows` on those that take it,
    /// the tiled engine for the model it runs, a check run to try it first.
    pub fn tips(&self) -> Vec<String> {
        let mut out = Vec::new();
        let heavy: Vec<String> = self
            .nodes
            .iter()
            .take(3)
            .filter(|(_, b)| *b > 0)
            .map(|(id, b)| format!("{id} ({})", crate::disk::size_text(*b)))
            .collect();
        if !heavy.is_empty() {
            out.push(format!("the nodes that weigh most: {}", heavy.join(", ")));
        }
        if !self.blockable.is_empty() {
            out.push(format!(
                "set block_rows (a multiple of 64, 1024 say) on {}: the map is read by blocks of rows, the same values",
                self.blockable.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        if self.tiled {
            out.push(
                "this model runs on tiles, which do not hold a whole map: terraflux run model.toml --tiled (--disk \
                 folder keeps the layers on disk)"
                    .into(),
            );
        }
        out.push(
            "to try the model first: a check run at 1/4 resolution (Run, Check run; terraflux run --coarsen 4) takes a \
             sixteenth of the memory, or a raster.resample or a crop of the inputs"
                .into(),
        );
        out
    }
}

/// What the model's nodes are sized by.
struct Walk<'a> {
    dir: &'a Path,
    r: &'a Registry,
    /// The grids of the model's inputs of maps and stacks.
    inputs: BTreeMap<String, G>,
}

impl Walk<'_> {
    /// The grid of a node: that of its file, its `rows` x `columns`, or its main input's.
    fn grid(&self, n: &Node, grids: &BTreeMap<String, G>, fallback: Option<G>) -> Option<G> {
        let of = |s: &Source| -> Option<G> {
            let Source::Ref(r) = s else { return None };
            if let Some(name) = r.strip_prefix('$') {
                return self.inputs.get(name).copied();
            }
            if r.starts_with('@') {
                return fallback;
            }
            grids.get(r.split('.').next()?).copied()
        };
        match n.op.as_str() {
            "raster.read" => {
                let p = n.params.get("path")?.as_str()?;
                let g = match p.strip_prefix('$') {
                    Some(name) => self.inputs.get(name).copied(),
                    None => header(p, self.dir),
                }?;
                Some(if n.params.contains_key("band") { G { layers: 1, ..g } } else { g })
            }
            op if op.starts_with("nlm.") => match n.inputs.get("like") {
                Some(s) => of(s),
                None => {
                    let rows = crate::disk::number(n, "rows").unwrap_or(256);
                    Some(G { cells: rows * crate::disk::number(n, "columns").unwrap_or(256), layers: 1 })
                }
            },
            _ => ["like", "map", "raster", "stack"]
                .iter()
                .find_map(|k| n.inputs.get(*k).and_then(of))
                .or_else(|| n.inputs.values().find_map(of)),
        }
    }

    /// The bytes of the table or vector a node holds: from its file, or (a vector) from the vector it is made of.
    fn file(&self, n: &Node, files: &BTreeMap<String, u64>) -> Option<u64> {
        let times = match n.op.as_str() {
            "table.read" => 1,
            "vector.read" => 2,
            _ => {
                let vector = crate::editor::output_ports(n, self.r).iter().any(|(_, t)| *t == Type::Vector);
                return vector
                    .then(|| {
                        n.inputs.values().find_map(|s| match s {
                            Source::Ref(r) => files.get(r.split('.').next()?).copied(),
                            _ => None,
                        })
                    })
                    .flatten();
            }
        };
        file_bytes(n.params.get("path")?.as_str()?, self.dir, times)
    }

    /// The grids and the files of the nodes of a scope, the nodes that give them first (a few passes: the order of
    /// the nodes in the file is not the order of the wires). `fallback`: the grid `@name` reads (a body's).
    fn resolve(&self, nodes: &[Node], fallback: Option<G>) -> (BTreeMap<String, G>, BTreeMap<String, u64>) {
        let (mut grids, mut files) = (BTreeMap::new(), BTreeMap::new());
        for _ in 0..=nodes.len() {
            let mut changed = false;
            for n in nodes {
                if !grids.contains_key(&n.id)
                    && let Some(g) = self.grid(n, &grids, fallback)
                {
                    grids.insert(n.id.clone(), g);
                    changed = true;
                }
                if !files.contains_key(&n.id)
                    && let Some(b) = self.file(n, &files)
                {
                    files.insert(n.id.clone(), b);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        (grids, files)
    }

    /// The bytes of the value a reference names in a body (`node.port`; `@name` and `$name` a map of the grid).
    fn value(
        &self,
        nodes: &[Node],
        grids: &BTreeMap<String, G>,
        files: &BTreeMap<String, u64>,
        from: &str,
        g: Option<G>,
    ) -> u64 {
        let Some((id, port)) = from.split_once('.') else { return type_bytes(Type::Map, g, None, false) };
        let Some(c) = nodes.iter().find(|c| c.id == id) else { return 0 };
        let Some((_, ty)) = crate::editor::output_ports(c, self.r).into_iter().find(|(p, _)| p == port) else {
            return 0;
        };
        type_bytes(ty, grids.get(id).copied(), files.get(id).copied(), true)
    }

    /// What a node's outputs take, and what its body keeps (a loop's).
    fn node(&self, n: &Node, g: Option<G>, file: Option<u64>) -> (u64, u64) {
        let Some(b) = &n.body else {
            // (the table a vector.read also gives is its attributes, already counted with its features)
            let kept = crate::editor::output_ports(n, self.r)
                .into_iter()
                .filter(|(_, t)| *t != Type::Table || n.op == "table.read")
                .map(|(_, t)| match t {
                    // a raster an operator gives (not one read from a file, which is read when a node asks) is its
                    // maps in memory: a map per metric of a window's, else the layers of the grid
                    Type::Raster if n.op != "raster.read" => {
                        let layers = n.params.get("metrics").and_then(|v| v.as_array()).map_or(1, |a| a.len() as u64);
                        g.map_or(0, |g| g.cells * layers.max(g.layers) * CELL)
                    }
                    t => type_bytes(t, g, file, false),
                })
                .sum();
            return (kept, 0);
        };
        let iters = iterations(n);
        let (grids, files) = self.resolve(&b.nodes, g);
        let one: u64 = b
            .nodes
            .iter()
            .map(|c| {
                let (k, inner) = self.node(c, grids.get(&c.id).copied(), files.get(&c.id).copied());
                k + inner
            })
            .sum();
        // the nodes of each iteration are kept for a loop to run again with one more step, up to the cache's limit
        let body = if iters > 1 { one.saturating_mul(iters).min(ITER_BYTES) } else { one };
        let value = |from: &String| self.value(&b.nodes, &grids, &files, from, g);
        let kept = b.collect.values().map(|f| value(f) * iters).sum::<u64>()
            + b.outputs.values().map(value).sum::<u64>()
            + b.carry.values().map(value).sum::<u64>();
        (kept, body)
    }
}

/// Whether a node reads the maps it is given by blocks of rows (what it holds of them at once is a block).
fn blockwise(n: &Node, cells: u64) -> bool {
    n.params.contains_key("block_rows")
        || matches!(n.op.as_str(), "landscape.mspa_tiled" | "vector.zonal" | "vector.extract" | "vector.rasterize")
        || (n.op.starts_with("metrics.") && cells > BLOCKED_CELLS)
}

/// The estimate of the memory a whole run of a model needs, from the model and the headers of its files. `dir` is the
/// model's folder.
pub fn estimate(m: &Model, r: &Registry, dir: &Path) -> Estimate {
    let mut w = Walk { dir, r, inputs: BTreeMap::new() };
    let mut e = Estimate::default();
    for (name, input) in &m.inputs {
        let g = match input {
            Input::Map { path, .. } => header(path, dir).map(|g| G { layers: 1, ..g }),
            Input::Stack { path, bands, .. } => {
                header(path, dir).map(|g| G { layers: bands.as_ref().map_or(g.layers, |b| b.len() as u64), ..g })
            }
            _ => None,
        };
        if let Some(g) = g {
            // (the engine loads the inputs whole before it runs)
            e.reads += g.bytes();
            e.cells = e.cells.max(g.cells);
            w.inputs.insert(name.clone(), g);
        }
    }
    let (grids, files) = w.resolve(&m.nodes, None);
    let all = crate::disk::nodes(&m.nodes);
    let mut weights = Vec::new();
    for n in &m.nodes {
        let g = grids.get(&n.id).copied();
        let (kept, body) = w.node(n, g, files.get(&n.id).copied());
        let mut weight = kept + body;
        let mut scratch = kept;
        if let Some(g) = g {
            e.cells = e.cells.max(g.cells);
            if n.op == "raster.read" {
                // read whole by a node that does not work by blocks (a node that reads nothing of it, none)
                let wire = format!("{}.", n.id);
                let readers: Vec<&&Node> = all
                    .iter()
                    .filter(|c| c.inputs.values().any(|s| matches!(s, Source::Ref(r) if r.starts_with(&wire))))
                    .collect();
                let written = m.outputs.values().any(|o| o.from.starts_with(&wire));
                if written || readers.iter().any(|c| !blockwise(c, g.cells)) {
                    e.reads += g.bytes();
                    weight += g.bytes();
                    scratch = scratch.max(g.bytes());
                }
            }
            // a node that reads a map whole and could read it by blocks
            let takes = r.sig(&n.op).is_some_and(|s| s.params.iter().any(|p| p.name == "block_rows"));
            if takes && !blockwise(n, g.cells) && g.cells <= BLOCKED_CELLS && g.cells > 0 {
                e.blockable.push(n.id.clone());
            }
        }
        e.kept += kept;
        e.bodies += body;
        e.scratch = e.scratch.max(scratch);
        if weight > 0 {
            weights.push((n.id.clone(), weight));
        }
    }
    weights.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    e.nodes = weights;
    e.tiled = all.iter().any(|n| n.op == "dinamica.step") && all.iter().any(|n| n.op == "woe.probability");
    e
}

/// What the nodes' outputs held at the end of the latest whole run of the same model (its `hash`), from the journal's
/// entries (the oldest first). None when no whole run of it measured.
pub fn from_journal(entries: &[Entry], hash: Option<&str>) -> Option<u64> {
    let h = hash?;
    entries
        .iter()
        .rev()
        .find(|e| matches!(e.kind, Kind::Run) && e.status == Status::Done && e.memory > 0 && e.model == h)
        .map(|e| e.memory)
}

/// How what a run needs fits the memory available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// With room to spare.
    Fits,
    /// More than four fifths of what is available.
    Tight,
    /// More than what is available.
    Over,
}

/// How `bytes` fit the memory `available`.
pub fn fit(bytes: u64, available: u64) -> Fit {
    if bytes == 0 || available == 0 {
        Fit::Fits
    } else if bytes > available {
        Fit::Over
    } else if bytes as f64 > available as f64 * TIGHT {
        Fit::Tight
    } else {
        Fit::Fits
    }
}

/// "about 12 GB of memory (estimated from the model); 8.0 GB available (of 16.0 GB)".
fn said(e: &Estimate, need: u64, ram: Ram) -> String {
    format!(
        "about {} of memory ({}); {} available (of {})",
        crate::disk::size_text(need),
        e.source(),
        crate::disk::size_text(ram.available),
        crate::disk::size_text(ram.total)
    )
}

/// A run held back for the memory: what is to be told, and what to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub text: String,
    pub tips: Vec<String>,
}

/// What to do before a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Run.
    Go,
    /// Run, and tell this (it fits but leaves little).
    Warn(String),
    /// Ask first (it may not fit).
    Ask(Ask),
}

/// The plan for a run that needs `e`, `at_once` of them at the same time (a batch's workers), on a computer of the
/// memory `ram` (none: it is not known, run).
pub fn plan(e: &Estimate, ram: Option<Ram>, at_once: usize) -> Plan {
    let Some(ram) = ram else { return Plan::Go };
    let at_once = at_once.max(1) as u64;
    let need = e.peak().saturating_mul(at_once);
    let fits = workers_that_fit(e.peak(), ram.available);
    let mut tips = Vec::new();
    if at_once > 1 {
        tips.push(if fits < at_once as usize {
            format!("{fits} at once fit: set the workers to {fits} or fewer")
        } else {
            "fewer workers take less memory".to_string()
        });
    }
    if at_once == 1 || fits <= 1 {
        tips.extend(e.tips());
    }
    let head = if at_once > 1 { format!("{at_once} runs at once need ") } else { "This run needs ".to_string() };
    match fit(need, ram.available) {
        Fit::Fits => Plan::Go,
        Fit::Tight => Plan::Warn(format!(
            "{head}{}: that leaves little{}",
            said(e, need, ram),
            tips.first().map(|t| format!(". {t}")).unwrap_or_default()
        )),
        Fit::Over => Plan::Ask(Ask { text: format!("{head}{}.", said(e, need, ram)), tips }),
    }
}

/// How many runs of `bytes` each fit at once in the memory available (one at least), leaving a fifth of it.
pub fn workers_that_fit(bytes: u64, available: u64) -> usize {
    if bytes == 0 {
        return usize::MAX;
    }
    ((available as f64 * TIGHT / bytes as f64) as usize).max(1)
}

/// The end of the cost line of a batch run `at_once` at a time: ", about 12 GB of memory at once (8.0 GB available)",
/// and whether it does not fit. None when it fits with room, or nothing is known.
pub fn cost_note(e: &Estimate, ram: Option<Ram>, at_once: usize) -> Option<(String, bool)> {
    let ram = ram?;
    let need = e.peak().saturating_mul(at_once.max(1) as u64);
    let f = fit(need, ram.available);
    (f != Fit::Fits).then(|| {
        (
            format!(
                ", about {} of memory at once ({} available)",
                crate::disk::size_text(need),
                crate::disk::size_text(ram.available)
            ),
            f == Fit::Over,
        )
    })
}

/// The advice for check_model and the Problems pane: a warning when a run (estimated from the model: the journal is
/// not at hand here) will not fit the memory available, or leaves little, with what to do. None otherwise.
pub fn advice(m: &Model, r: &Registry, dir: &Path) -> Option<crate::advice::Advice> {
    advice_on(m, r, dir, ram()?)
}

/// [`advice`] for a computer of the memory given.
pub fn advice_on(m: &Model, r: &Registry, dir: &Path, ram: Ram) -> Option<crate::advice::Advice> {
    advice_for(&estimate(m, r, dir), ram)
}

/// The advice for an estimate made already.
pub fn advice_for(e: &Estimate, ram: Ram) -> Option<crate::advice::Advice> {
    let f = fit(e.peak(), ram.available);
    if f == Fit::Fits {
        return None;
    }
    let verdict = if f == Fit::Over { "will not fit" } else { "leaves little" };
    Some(crate::advice::Advice {
        level: crate::advice::Level::Warning,
        at: e.nodes.iter().take(2).map(|(id, _)| id.clone()).collect(),
        message: format!("a run needs {} and that {verdict}: {}", said(e, e.peak(), ram), e.tips().join("; ")),
        guide: "data-preparation",
    })
}

/// The window of a run that may not fit the memory, while `ask` holds one: whether to run anyway (the window closed),
/// none while it waits.
pub fn window(ctx: &egui::Context, ask: &mut Option<Ask>) -> Option<bool> {
    let a = ask.as_ref()?;
    let mut answer = None;
    let mut open = true;
    egui::Window::new("Not much memory").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        ui.set_min_width(420.0);
        ui.label(&a.text);
        ui.label(
            RichText::new("An estimate: a run that needs more than the computer has is stopped by the system.")
                .small()
                .color(crate::theme::weak()),
        );
        for t in &a.tips {
            ui.label(format!("- {t}"));
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.add(crate::app::accent_button("Run anyway", false)).clicked() {
                answer = Some(true);
            }
            if ui.button("Cancel").clicked() {
                answer = Some(false);
            }
        });
    });
    if !open {
        answer = Some(false);
    }
    if answer.is_some() {
        *ask = None;
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1 << 30;
    use terraflux_exec::journal::Written;
    use terraflux_raster::tiff::{self, WriteOptions};
    use terraflux_raster::{Band, DataType, Grid, Raster};

    /// A GeoTIFF of `w` x `h` cells and `bands` bands in a folder of its own.
    fn tif(dir: &Path, name: &str, w: usize, h: usize, bands: usize) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        let band = || Band { name: None, data: vec![1.0; w * h] };
        let r = Raster {
            grid: Grid::new(w, h),
            bands: (0..bands).map(|_| band()).collect(),
            dtype: DataType::U8,
            nodata: None,
        };
        tiff::write(&r, &path, &WriteOptions::default()).unwrap();
        path
    }

    fn folder(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tf-memory-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn entry(model: &str, status: Status, memory: u64) -> Entry {
        let outputs = vec![Written { size: 1, ..Default::default() }];
        Entry { model: model.into(), status, memory, outputs, ..Default::default() }
    }

    /// A raster.read and two focal windows after it: 200 x 100 cells.
    fn chain(dir: &Path) -> Model {
        let mut m = Model::new("chain");
        m.nodes
            .push(Node::new("land", "raster.read").param("path", tif(dir, "land.tif", 200, 100, 1).to_string_lossy()));
        m.nodes.push(Node::new("a", "raster.focal").input("map", "land.raster"));
        m.nodes.push(Node::new("b", "raster.focal").input("map", "a.map"));
        m
    }

    /// The estimate of what the nodes' outputs hold against what the runner measures (the sum of `bytes_of` of the
    /// top-level nodes' outputs) on templates of the application that run as they are.
    #[test]
    fn the_estimate_stays_near_what_templates_hold_when_they_run() {
        let r = Registry::standard();
        let engine = terraflux_exec::Engine::default();
        let mut seen = 0;
        for t in
            crate::templates::TEMPLATES.iter().filter(|t| t.group == "Landscape metrics" || t.group == "Land change")
        {
            let m = t.model();
            if !m.inputs.is_empty() {
                continue;
            }
            let Ok(done) = engine.run(&m, &BTreeMap::new(), &mut terraflux_exec::Quiet) else { continue };
            let held: usize = done.values.values().flat_map(|o| o.values()).map(terraflux_exec::bytes_of).sum();
            let e = estimate(&m, &r, Path::new("."));
            eprintln!("{:<40} held {held:>10}  estimated {:>10}", t.name, e.kept);
            for (id, o) in &done.values {
                let b: usize = o.values().map(terraflux_exec::bytes_of).sum();
                let ports: Vec<String> =
                    o.iter().map(|(p, v)| format!("{p}={}", terraflux_exec::bytes_of(v))).collect();
                eprintln!("    {id:<20} {b:>9} {ports:?} est {:?}", e.nodes.iter().find(|n| n.0 == *id).map(|n| n.1));
            }
            if held > 100_000 {
                seen += 1;
                let ratio = e.kept as f64 / held as f64;
                assert!((0.4..=2.5).contains(&ratio), "{}: held {held}, estimated {}", t.name, e.kept);
            }
        }
        assert!(seen >= 3, "templates that run and hold maps: {seen}");
    }

    /// The estimate reads one header of a pattern of files, not each: the time of a template and of a thousand files.
    #[test]
    fn the_estimate_is_quick_for_a_template_and_for_a_thousand_files() {
        let r = Registry::standard();
        let t = crate::templates::TEMPLATES.iter().find(|t| t.group == "Landscape metrics").unwrap().model();
        let start = std::time::Instant::now();
        let e = estimate(&t, &r, Path::new("."));
        let template = start.elapsed();
        assert!(e.kept > 0);
        let dir = folder("thousand");
        for i in 0..1000 {
            tif(&dir, &format!("f{i:04}.tif"), 20, 10, 1);
        }
        let mut m = Model::new("many");
        m.nodes.push(Node::new("all", "raster.read").param("path", format!("{}/*.tif", dir.to_string_lossy())));
        m.nodes.push(Node::new("a", "raster.focal").input("map", "all.raster"));
        let start = std::time::Instant::now();
        let e = estimate(&m, &r, &dir);
        let many = start.elapsed();
        eprintln!("estimate: a template {template:?}, a pattern of 1000 files {many:?}");
        assert_eq!(e.cells, 200);
        assert!(template.as_millis() < 500 && many.as_millis() < 3000, "{template:?} {many:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_computer_tells_its_memory() {
        let r = ram().expect("this computer tells its memory");
        assert!(r.total > 0 && r.available > 0 && r.available <= r.total, "{r:?}");
    }

    #[test]
    fn proc_meminfo_is_read_with_or_without_the_available_line() {
        let with =
            "MemTotal:       16000000 kB\nMemFree:         1000000 kB\nMemAvailable:    8000000 kB\nCached: 5 kB\n";
        assert_eq!(parse_meminfo(with), Some(Ram { total: 16_000_000 * 1024, available: 8_000_000 * 1024 }));
        let old = "MemTotal: 1000 kB\nMemFree: 100 kB\nBuffers: 50 kB\nCached: 150 kB\n";
        assert_eq!(parse_meminfo(old), Some(Ram { total: 1000 * 1024, available: 300 * 1024 }));
        assert_eq!(parse_meminfo("nothing"), None);
    }

    #[test]
    fn a_made_up_memory_stands_for_the_thread_only() {
        let fake = Ram { total: 4 * GB, available: GB };
        pretend(Some(fake));
        assert_eq!(ram(), Some(fake));
        std::thread::spawn(|| assert_ne!(ram(), Some(Ram { total: 4 * GB, available: GB }))).join().unwrap();
        pretend(None);
        assert_ne!(ram(), Some(fake));
    }

    #[test]
    fn a_model_of_three_nodes_is_sized_by_the_header_of_its_raster() {
        let dir = folder("three");
        let m = chain(&dir);
        let e = estimate(&m, &Registry::standard(), &dir);
        let map = 200 * 100 * 8;
        // two maps kept; the raster read whole by the first window; the largest node's output again
        assert_eq!((e.kept, e.reads, e.bodies, e.scratch), (2 * map, map, 0, map), "{e:?}");
        assert_eq!(e.peak(), 4 * map);
        assert_eq!(e.cells, 20_000);
        assert_eq!(
            e.nodes.iter().map(|(id, b)| (id.as_str(), *b)).collect::<Vec<_>>(),
            [("a", map), ("b", map), ("land", map)]
        );
        // the heaviest nodes and the way out, in words
        let tips = e.tips().join("\n");
        assert!(tips.contains("the nodes that weigh most: ") && tips.contains("check run at 1/4 resolution"), "{tips}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_layers_of_a_file_and_of_an_input_stack_count() {
        let dir = folder("layers");
        let mut m = Model::new("layers");
        m.nodes.push(Node::new("bands", "raster.read").param("path", tif(&dir, "b.tif", 100, 50, 3).to_string_lossy()));
        m.nodes.push(
            Node::new("one", "raster.read")
                .param("path", tif(&dir, "b.tif", 100, 50, 3).to_string_lossy())
                .param("band", 1),
        );
        let e = estimate(&m, &Registry::standard(), &dir);
        // the first reads three layers whole, the second one: no node reads them, the raster is not read (the output
        // ports of a raster.read are lazy)
        assert_eq!(e.kept, 0);
        m.nodes.push(Node::new("f", "raster.focal").input("map", "bands.raster"));
        let e = estimate(&m, &Registry::standard(), &dir);
        assert_eq!(e.reads, 100 * 50 * 3 * 8);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_blocks_do_not_hold_the_whole_map() {
        let dir = folder("blocks");
        let mut m = Model::new("blocks");
        m.nodes
            .push(Node::new("land", "raster.read").param("path", tif(&dir, "land.tif", 200, 100, 1).to_string_lossy()));
        m.nodes.push(Node::new("met", "metrics.class").input("map", "land.raster"));
        let r = Registry::standard();
        let whole = estimate(&m, &r, &dir);
        assert_eq!(whole.reads, 200 * 100 * 8);
        assert_eq!(whole.blockable, ["met"], "it reads the map whole and takes block_rows");
        m.nodes[1].params.insert("block_rows".into(), serde_json::json!(64));
        let blocks = estimate(&m, &r, &dir);
        assert_eq!(blocks.reads, 0, "read by blocks of rows");
        assert!(blocks.blockable.is_empty());
        let tips = whole.tips().join("\n");
        assert!(tips.contains("set block_rows") && tips.contains("met"), "{tips}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_loop_keeps_one_iteration_of_its_body_times_its_steps_up_to_the_cache() {
        let r = Registry::standard();
        let mut body = terraflux_graph::Body::default();
        body.nodes.push(Node::new("w", "nlm.random").param("rows", 1000).param("columns", 1000));
        body.collect.insert("maps".into(), "w.map".into());
        let mut lp = Node::new("loop", "control.repeat").literal("times", 10);
        lp.body = Some(body);
        let mut m = Model::new("loop");
        m.nodes.push(lp);
        let map = 1_000_000 * 8;
        let e = estimate(&m, &r, Path::new("."));
        // the collected series of maps: a map a step; the nodes of the ten iterations kept: ten maps
        assert_eq!(e.kept, 10 * map);
        assert_eq!(e.bodies, 10 * map);
        // a hundred steps would be 800 MB kept, but the cache lets go past a GiB; a thousand
        m.nodes[0].inputs.insert("times".into(), Source::Literal(serde_json::json!(1000)));
        let e = estimate(&m, &r, Path::new("."));
        assert_eq!(e.bodies, ITER_BYTES, "the cache keeps no more than a GiB of the iterations");
        assert_eq!(e.kept, 1000 * map, "what the loop gives is kept whole");
    }

    #[test]
    fn a_table_and_a_vector_are_about_their_file() {
        let dir = folder("files");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("t.csv"), vec![b'1'; 1000]).unwrap();
        std::fs::write(dir.join("v.geojson"), vec![b' '; 3000]).unwrap();
        let r = Registry::standard();
        let mut m = Model::new("files");
        m.nodes.push(Node::new("t", "table.read").param("path", "t.csv"));
        m.nodes.push(Node::new("v", "vector.read").param("path", "v.geojson"));
        m.nodes.push(Node::new("buf", "vector.buffer").input("vector", "v.vector"));
        let e = estimate(&m, &r, &dir);
        // the table as its file; the vector twice; the buffer made of the vector the same
        assert_eq!(e.kept, 1000 + 6000 + 6000, "{e:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_run_before_is_preferred_to_the_estimate_and_says_so() {
        let entries = [
            entry("a", Status::Done, 700),
            entry("b", Status::Done, 900),
            entry("a", Status::Failed("x".into()), 5000),
            entry("a", Status::Done, 0),
            Entry { kind: Kind::To("n".into()), ..entry("a", Status::Done, 9000) },
        ];
        assert_eq!(from_journal(&entries, Some("a")), Some(700));
        assert_eq!(from_journal(&entries, Some("zz")), None, "another model's run says nothing of this one");
        assert_eq!(from_journal(&entries, None), None);
        let dir = folder("measured");
        let e = estimate(&chain(&dir), &Registry::standard(), &dir);
        assert_eq!(e.source(), "estimated from the model");
        let kept = e.kept;
        let m = e.with_measure(kept * 3);
        assert_eq!(m.source(), "from the run before");
        assert_eq!(m.peak(), kept * 3 + m.reads + m.scratch);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_run_fits_leaves_little_or_does_not_fit() {
        assert_eq!(fit(0, GB), Fit::Fits);
        assert_eq!(fit(79, 100), Fit::Fits);
        assert_eq!(fit(81, 100), Fit::Tight);
        assert_eq!(fit(100, 100), Fit::Tight);
        assert_eq!(fit(101, 100), Fit::Over);
        // nothing known of the memory: it fits
        assert_eq!(fit(GB, 0), Fit::Fits);
        let e = Estimate { kept: 10 * GB, ..Default::default() };
        let ram = |a| Some(Ram { total: 16 * GB, available: a });
        assert_eq!(plan(&e, ram(32 * GB), 1), Plan::Go);
        assert_eq!(plan(&e, None, 1), Plan::Go);
        let Plan::Warn(t) = plan(&e, ram(12 * GB), 1) else { panic!() };
        assert!(
            t.contains("about 10.0 GB of memory") && t.contains("12.0 GB available") && t.contains("little"),
            "{t}"
        );
        let Plan::Ask(a) = plan(&e, ram(8 * GB), 1) else { panic!() };
        assert!(
            a.text.contains("about 10.0 GB of memory (estimated from the model); 8.0 GB available (of 16.0 GB)"),
            "{}",
            a.text
        );
        assert!(a.tips.iter().any(|t| t.contains("check run")), "{:?}", a.tips);
    }

    #[test]
    fn the_workers_of_a_batch_that_fit_are_told() {
        let e = Estimate { kept: 3 * GB, ..Default::default() };
        let ram = Some(Ram { total: 16 * GB, available: 12 * GB });
        // 12 GB x 0.8 leaves room for three at once
        assert_eq!(workers_that_fit(3 * GB, 12 * GB), 3);
        assert_eq!(workers_that_fit(30 * GB, 12 * GB), 1, "one at least");
        assert_eq!(plan(&e, ram, 3), Plan::Go);
        let Plan::Ask(a) = plan(&e, ram, 6) else { panic!() };
        assert!(a.text.contains("6 runs at once need about 18.0 GB"), "{}", a.text);
        assert!(a.tips[0].contains("3 at once fit: set the workers to 3 or fewer"), "{:?}", a.tips);
        let (note, over) = cost_note(&e, ram, 6).unwrap();
        assert!(over && note.contains("about 18.0 GB of memory at once (12.0 GB available)"), "{note}");
        assert!(cost_note(&e, ram, 1).is_none());
    }

    #[test]
    fn a_model_that_may_not_fit_the_memory_is_warned_about() {
        let r = Registry::standard();
        let mut m = Model::new("big");
        // 20000 x 20000 cells of 8 bytes: 3 GB a map
        m.nodes.push(Node::new("a", "nlm.random").param("rows", 20000).param("columns", 20000));
        m.nodes.push(Node::new("f", "raster.focal").input("map", "a.map"));
        let dir = std::env::temp_dir();
        let small = Ram { total: 8 * GB, available: 4 * GB };
        let a = advice_on(&m, &r, &dir, small).unwrap();
        assert_eq!(a.level, crate::advice::Level::Warning);
        assert!(a.message.contains("will not fit") && a.message.contains("4.0 GB available"), "{}", a.message);
        assert!(a.message.contains("set block_rows") || a.message.contains("check run"), "{}", a.message);
        assert!(advice_on(&m, &r, &dir, Ram { total: 64 * GB, available: 32 * GB }).is_none());
        let mut tiled = m.clone();
        tiled.nodes.push(Node::new("p", "woe.probability"));
        tiled.nodes.push(Node::new("s", "dinamica.step"));
        assert!(advice_on(&tiled, &r, &dir, small).unwrap().message.contains("--tiled"));
    }
}
