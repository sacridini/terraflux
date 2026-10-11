//! Running a Terraflux model on one machine: its inputs read ([`load_inputs`]), its graph run node by node in
//! the order of their dependencies ([`Engine::run`]), its outputs written ([`write_outputs`]).
//!
//! This is the exact engine: whole maps in memory, the kernels in the order landschaft runs them, so a model
//! gives the same maps as `ls.simulate` with the same seed.
//!
//! With a [`Cache`] ([`Engine::run_with`]) the outputs of the top-level nodes are kept between runs, and a node
//! runs again only when it, a node it depends on, an input or a file it reads changed; a run may stop at a
//! node (its ancestors only run). The application runs the model as it is edited this way.

mod io;
pub mod lint;

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use terraflux_graph::{Catalog, Model, Node, Ref, Severity, Source, check, dependencies_among, ids, order};
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
    /// Whether to stop: checked before every node.
    fn cancelled(&mut self) -> bool {
        false
    }
}

/// An observer that ignores everything.
pub struct Quiet;

impl Observer for Quiet {}

/// The values of a run: every output of the top-level nodes, and the warnings.
pub struct Results {
    pub values: BTreeMap<String, Outputs>,
    pub warnings: Vec<String>,
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

/// The outputs of a top-level node kept, the fingerprint of what they came from and the node's warnings.
struct Kept {
    print: u64,
    outputs: Outputs,
    warnings: Vec<String>,
}

/// The outputs of the top-level nodes of a model kept between runs ([`Engine::run_with`]).
#[derive(Default)]
pub struct Cache {
    kept: BTreeMap<String, Kept>,
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
    }

    pub fn clear(&mut self) {
        self.kept.clear();
    }
}

/// Whether a value holds a random generator, whose state changes as it is used (not to be kept).
fn holds_rng(v: &Value) -> bool {
    match v {
        Value::Rng(_) => true,
        Value::Series(s) => s.iter().any(holds_rng),
        _ => false,
    }
}

/// A node as it counts for its outputs: without its place on the canvas and its note.
fn bare(n: &Node) -> Node {
    let mut n = n.clone();
    n.pos = None;
    n.note = None;
    if let Some(b) = &mut n.body {
        b.nodes = b.nodes.iter().map(bare).collect();
    }
    n
}

/// The size and time of the files a node reads (in its body too).
fn files_read(n: &Node, h: &mut impl Hasher) {
    if n.op == "raster.read"
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

/// The top-level node of a place (`node`, `node.port`, `container/inner...`).
fn head(at: &str) -> &str {
    at.split(['/', '.']).next().unwrap_or(at)
}

struct Env<'p> {
    parent: Option<&'p Env<'p>>,
    results: BTreeMap<String, Outputs>,
    carried: BTreeMap<String, Value>,
}

impl Env<'_> {
    fn port(&self, node: &str, port: &str) -> Option<&Value> {
        match self.results.get(node) {
            Some(o) => o.get(port),
            None => self.parent?.port(node, port),
        }
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
        terraflux_ops::threads::ensure();
        let among = ids(&model.nodes);
        let by_id: std::collections::HashMap<&str, &Node> = model.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        // the nodes to run: every one, or the target's ancestors
        let needed: Option<std::collections::BTreeSet<String>> = match target {
            None => None,
            Some(t) => {
                if !by_id.contains_key(t) {
                    return Err(Error::Model(format!("no node {t:?}")));
                }
                let mut set = std::collections::BTreeSet::new();
                let mut todo = vec![t.to_string()];
                while let Some(id) = todo.pop() {
                    if !set.contains(&id)
                        && let Some(n) = by_id.get(id.as_str())
                    {
                        todo.extend(dependencies_among(n, &among));
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
        let mut env = Env { parent: None, results: BTreeMap::new(), carried: BTreeMap::new() };
        let mut warnings = Vec::new();
        let ordered =
            order(&model.nodes, |n| dependencies_among(n, &among)).map_err(|e| Error::Model(e.to_string()))?;
        let mut prints: BTreeMap<String, Option<u64>> = BTreeMap::new();
        for node in ordered {
            if !runs(&node.id) {
                continue;
            }
            if obs.cancelled() {
                return Err(Error::Cancelled);
            }
            let at = node.id.as_str();
            let print = cache.and_then(|_| fingerprint(node, model, &among, inputs, &prints));
            let kept = match (cache, print) {
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
                Self::replay(node, &out, obs);
                obs.done(at, true);
                env.results.insert(node.id.clone(), out);
                prints.insert(node.id.clone(), print);
                continue;
            }
            obs.node(at);
            let before = warnings.len();
            let out = self.run_node(node, &env, inputs, at, obs, &mut warnings)?;
            let keep = print.is_some() && node.op != "raster.write" && !out.values().any(holds_rng);
            if let (Some(c), Some(p), true) = (cache, print, keep) {
                let prefix = format!("{at}: ");
                let ws = warnings[before..].iter().map(|w| w.strip_prefix(&prefix).unwrap_or(w).to_string()).collect();
                c.lock().unwrap().kept.insert(node.id.clone(), Kept { print: p, outputs: out.clone(), warnings: ws });
            }
            prints.insert(node.id.clone(), if keep { print } else { None });
            obs.done(at, false);
            env.results.insert(node.id.clone(), out);
        }
        if let Some(c) = cache {
            c.lock().unwrap().kept.retain(|id, _| by_id.contains_key(id.as_str()));
        }
        Ok(Results { values: env.results, warnings })
    }

    /// What a loop collected, step by step, from its kept outputs (as if it ran).
    fn replay(node: &Node, out: &Outputs, obs: &mut dyn Observer) {
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
            obs.collected(&node.id, step, steps, &now);
            obs.step(&node.id, step, steps);
        }
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
        v.cloned().ok_or_else(|| Error::Node { at: at.into(), message: format!("{r} has no value") })
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
            let out = self.run_node(node, env, inputs, &at, obs, warnings)?;
            env.results.insert(node.id.clone(), out);
        }
        Ok(())
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
        let mut values = BTreeMap::new();
        for (port, src) in &node.inputs {
            values.insert(port.clone(), self.resolve(src, env, inputs, &format!("{at}.{port}"))?);
        }
        // rasters on disk are read whole where a port takes a map or a stack (a container's values: maps)
        let sig = self.registry.sig(&node.op).cloned();
        for (port, v) in values.iter_mut() {
            if matches!(v, Value::Raster(_)) {
                let want = sig
                    .as_ref()
                    .and_then(|s| s.input_spec(port).map(|p| p.ty).or(s.extra))
                    .unwrap_or(terraflux_graph::Type::Map);
                let raster = std::mem::replace(v, Value::Bool(false));
                *v = terraflux_ops::files::as_port(raster, want)
                    .map_err(|e| Error::Node { at: format!("{at}.{port}"), message: e.to_string() })?;
            }
        }
        if node.op == "control.repeat" {
            return self.repeat(node, env, values, inputs, at, obs, warnings);
        }
        if node.op == terraflux_graph::GROUP {
            return self.group(node, env, values, inputs, at, obs, warnings);
        }
        let op = self
            .registry
            .get(&node.op)
            .ok_or_else(|| Error::Node { at: at.into(), message: format!("unknown operator {:?}", node.op) })?;
        let mut ctx = Ctx::default();
        let args = Args { node: at, inputs: values, params: &node.params };
        let out = op.run(&mut ctx, &args).map_err(|e| Error::Node { at: at.into(), message: e.to_string() })?;
        for w in ctx.warnings {
            obs.warning(at, &w);
            warnings.push(format!("{at}: {w}"));
        }
        let sig = self.registry.signature(&node.op).unwrap();
        // the optional outputs: given on request or for some inputs only (a reader of one missing has no value)
        for p in sig.outputs.iter().filter(|p| p.required) {
            if !out.contains_key(&p.name) {
                return Err(Error::Node { at: at.into(), message: format!("{} gave no output {:?}", node.op, p.name) });
            }
        }
        Ok(out)
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
        let mut child = Env { parent: Some(env), results: BTreeMap::new(), carried: values };
        self.run_scope(&body.nodes, &mut child, inputs, at, obs, warnings)?;
        let mut out = Outputs::new();
        for (name, r) in &body.outputs {
            out.insert(
                name.clone(),
                self.resolve(&Source::Ref(r.clone()), &child, inputs, &format!("{at}/outputs.{name}"))?,
            );
        }
        Ok(out)
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
            let mut child = Env { parent: Some(env), results: BTreeMap::new(), carried: c };
            self.run_scope(&body.nodes, &mut child, inputs, at, obs, warnings)?;
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

    fn engine() -> Engine {
        let mut r = Registry::standard();
        r.add(Arc::new(Add));
        Engine { registry: r }
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
            Node::new("calc", "map.calc")
                .input("lu", "$land")
                .literal("k", 10)
                .param("expression", "if lu >= 2 then lu * k else 0"),
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
        m.inputs.insert("x".into(), terraflux_graph::Input::Value { value: 1.into(), note: None });
        assert!(engine().run(&m, &BTreeMap::new(), &mut Quiet).is_err());
    }
}
