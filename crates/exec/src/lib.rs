//! Running a Terraflux model on one machine: its inputs read ([`load_inputs`]), its graph run node by node in
//! the order of their dependencies ([`Engine::run`]), its outputs written ([`write_outputs`]).
//!
//! This is the exact engine: whole maps in memory, the kernels in the order landschaft runs them, so a model
//! gives the same maps as `ls.simulate` with the same seed.

mod io;
pub mod lint;

use std::collections::BTreeMap;

use terraflux_graph::{Catalog, Model, Node, Ref, Severity, Source, check, dependencies, order};
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
        let errors: Vec<String> = check(model, &self.registry)
            .into_iter()
            .filter(|i| i.severity == Severity::Error)
            .map(|i| i.to_string())
            .collect();
        if !errors.is_empty() {
            return Err(Error::Model(format!("the model has errors:\n  {}", errors.join("\n  "))));
        }
        for name in model.inputs.keys() {
            if !inputs.contains_key(name) {
                return Err(Error::Model(format!("no value for the input ${name}")));
            }
        }
        let mut env = Env { parent: None, results: BTreeMap::new(), carried: BTreeMap::new() };
        let mut warnings = Vec::new();
        self.run_scope(&model.nodes, &mut env, inputs, "", obs, &mut warnings)?;
        Ok(Results { values: env.results, warnings })
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
        let ordered = order(nodes, |n| dependencies(n, nodes)).map_err(|e| Error::Model(e.to_string()))?;
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
        if node.op == "control.repeat" {
            return self.repeat(node, env, values, inputs, at, obs, warnings);
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
        for p in &sig.outputs {
            if !out.contains_key(&p.name) {
                return Err(Error::Node { at: at.into(), message: format!("{} gave no output {:?}", node.op, p.name) });
            }
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
