//! The model graph of Terraflux.
//!
//! A [`Model`] is a graph of operators: each [`Node`] runs one operator
//! (`op`) on its inputs, which come from the model's [`Input`]s (`$name`),
//! from the outputs of other nodes (`node.port`), from values carried along
//! a loop (`@name`), or are literal numbers; `params` hold the operator's
//! settings and `note` says why they were chosen (the wizards write it, so
//! that every automatic decision can be read and changed).
//!
//! Containers hold a [`Body`] of nodes: `control.repeat` runs it once per
//! step, carrying values from one step to the next and collecting others
//! into series.
//!
//! The same graph is written by every front-end (the wizards and the node
//! editor of the application, landschaft's Python API, a text editor) and
//! run by every engine (one machine or many). It is saved as TOML or JSON
//! ([`Model::load`], [`Model::save`]); [`check`] validates it against the
//! operators' signatures before anything runs.

mod check;
pub mod submodel;
mod types;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub use check::{Catalog, Issue, Severity, check};
pub use types::{Access, OpSignature, PortSpec, Type};

/// The container that runs its body once: a group of nodes with its own inputs (`@name` inside) and outputs
/// (`body.outputs`), as submodels become.
pub const GROUP: &str = "control.group";

/// A node that stands for another model file (`params.path`), expanded into a group before a run
/// ([`submodel::expand`]).
pub const SUBMODEL: &str = "control.submodel";

/// A WebAssembly plugin (`params.path`, relative to the model's folder until the model is expanded).
pub const PLUGIN: &str = "plugin.wasm";

/// The version of the model format this crate reads and writes.
pub const FORMAT: &str = "terraflux/1";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("TOML: {0}")]
    TomlWrite(#[from] toml::ser::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Model(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A land change model: its inputs, its graph of operators and what it writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    /// The format version (`terraflux/1`).
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The data the model reads, by name (`$name` in the nodes).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, Input>,
    /// The operators, in any order (they run in the order of their dependencies).
    #[serde(default)]
    pub nodes: Vec<Node>,
    /// What the model writes, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, Output>,
}

/// Data a model reads: a raster file (one band, or every band), a table, or a value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Input {
    /// One band of a raster file: a map.
    Map {
        path: String,
        /// The band, by name or 0-based index (default: the first).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        band: Option<String>,
        /// A no-data value replacing the file's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nodata: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// Every band of a raster file (or the bands named): a stack of maps, e.g. the drivers.
    Stack {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bands: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nodata: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// A number or any structured value (e.g. the seed, a table of demands).
    Value {
        value: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
}

/// Where a node input comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Source {
    /// `$input`, `@carried` or `node.port`.
    Ref(String),
    /// A literal value.
    Literal(serde_json::Value),
}

/// A reference parsed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ref {
    /// `$name`: an input of the model.
    Input(String),
    /// `@name`: a value carried along the loop of the enclosing container (or `@step`).
    Carried(String),
    /// `node.port`: an output of a node.
    Port { node: String, port: String },
}

impl Ref {
    pub fn parse(s: &str) -> Result<Ref> {
        let bad = || Error::Model(format!("{s:?} is not a reference ($input, @carried or node.port)"));
        if let Some(name) = s.strip_prefix('$') {
            return if valid_id(name) { Ok(Ref::Input(name.to_string())) } else { Err(bad()) };
        }
        if let Some(name) = s.strip_prefix('@') {
            return if valid_id(name) { Ok(Ref::Carried(name.to_string())) } else { Err(bad()) };
        }
        let (node, port) = s.split_once('.').ok_or_else(bad)?;
        if valid_id(node) && valid_id(port) {
            Ok(Ref::Port { node: node.to_string(), port: port.to_string() })
        } else {
            Err(bad())
        }
    }
}

impl std::fmt::Display for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ref::Input(n) => write!(f, "${n}"),
            Ref::Carried(n) => write!(f, "@{n}"),
            Ref::Port { node, port } => write!(f, "{node}.{port}"),
        }
    }
}

/// Names of inputs, nodes and ports: a letter or `_`, then letters, digits, `_` or `-`.
pub fn valid_id(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(x) if x.is_ascii_alphabetic() || x == '_')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '-')
}

/// One operator of the graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    /// The operator (`markov.fit`, `woe.probability`, `control.repeat`...).
    pub op: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, Source>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub params: serde_json::Map<String, serde_json::Value>,
    /// Why the node is there or why its settings (written by the wizards).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The nodes of a container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Body>,
    /// Where the node editor draws it (its scope's coordinates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<[f32; 2]>,
}

impl Node {
    pub fn new(id: &str, op: &str) -> Self {
        Node {
            id: id.to_string(),
            op: op.to_string(),
            inputs: BTreeMap::new(),
            params: serde_json::Map::new(),
            note: None,
            body: None,
            pos: None,
        }
    }

    /// The node with an input from a reference.
    pub fn input(mut self, port: &str, reference: &str) -> Self {
        self.inputs.insert(port.to_string(), Source::Ref(reference.to_string()));
        self
    }

    /// The node with a literal input.
    pub fn literal(mut self, port: &str, value: impl Into<serde_json::Value>) -> Self {
        self.inputs.insert(port.to_string(), Source::Literal(value.into()));
        self
    }

    pub fn param(mut self, name: &str, value: impl Into<serde_json::Value>) -> Self {
        self.params.insert(name.to_string(), value.into());
        self
    }

    pub fn note(mut self, note: &str) -> Self {
        self.note = Some(note.to_string());
        self
    }

    pub fn body(mut self, body: Body) -> Self {
        self.body = Some(body);
        self
    }
}

/// The nodes of a container and the values it carries along its loop.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Body {
    #[serde(default)]
    pub nodes: Vec<Node>,
    /// `@name` -> the reference giving its value for the next step (its first value is the container's input
    /// of the same name).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub carry: BTreeMap<String, String>,
    /// Output name -> the reference whose value at every step is collected into a series.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub collect: BTreeMap<String, String>,
    /// Output name -> the reference giving it (a group's outputs).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, String>,
}

/// A value the model writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Output {
    /// The reference written (`node.port`).
    pub from: String,
    /// The file (relative to the output folder; default: the output's name and the usual extension).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The cell type of rasters (`u8`, `i32`, `f32`...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dtype: Option<String>,
}

impl Model {
    pub fn new(name: &str) -> Self {
        Model {
            format: FORMAT.to_string(),
            name: Some(name.to_string()),
            description: None,
            inputs: BTreeMap::new(),
            nodes: Vec::new(),
            outputs: BTreeMap::new(),
        }
    }

    /// Read a model from TOML (`.toml`) or JSON (any other extension).
    pub fn load(path: impl AsRef<Path>) -> Result<Model> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)?;
        let model = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")) {
            Model::from_toml(&text)?
        } else {
            Model::from_json(&text)?
        };
        Ok(model)
    }

    /// Read a model and expand its submodels (to run it).
    pub fn load_expanded(path: impl AsRef<Path>) -> Result<Model> {
        let path = path.as_ref();
        let mut m = Model::load(path)?;
        submodel::expand(&mut m, path.parent().unwrap_or(Path::new("")))?;
        Ok(m)
    }

    /// Write a model as TOML (`.toml`) or JSON (any other extension).
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let text = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")) {
            self.to_toml()?
        } else {
            self.to_json()?
        };
        std::fs::write(path, text)?;
        Ok(())
    }

    pub fn from_toml(text: &str) -> Result<Model> {
        let m: Model = toml::from_str(text)?;
        m.check_format()?;
        Ok(m)
    }

    pub fn from_json(text: &str) -> Result<Model> {
        let m: Model = serde_json::from_str(text)?;
        m.check_format()?;
        Ok(m)
    }

    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    fn check_format(&self) -> Result<()> {
        if self.format != FORMAT {
            return Err(Error::Model(format!(
                "this model is in format {:?}; this version of Terraflux reads {FORMAT:?}",
                self.format
            )));
        }
        Ok(())
    }

    /// The node of an id at the top level.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// The nodes of a scope: the top level (an empty path) or the body of the container at a path of ids.
    pub fn scope(&self, path: &[String]) -> Option<&Vec<Node>> {
        let mut nodes = &self.nodes;
        for id in path {
            nodes = &nodes.iter().find(|n| &n.id == id)?.body.as_ref()?.nodes;
        }
        Some(nodes)
    }

    pub fn scope_mut(&mut self, path: &[String]) -> Option<&mut Vec<Node>> {
        let mut nodes = &mut self.nodes;
        for id in path {
            nodes = &mut nodes.iter_mut().find(|n| &n.id == id)?.body.as_mut()?.nodes;
        }
        Some(nodes)
    }

    /// The node of a path of ids (`simulation/allocate`: a node of a container's body).
    pub fn node_mut(&mut self, path: &str) -> Option<&mut Node> {
        let mut parts = path.split('/');
        let mut node = self.nodes.iter_mut().find(|n| Some(n.id.as_str()) == parts.clone().next())?;
        parts.next();
        for id in parts {
            node = node.body.as_mut()?.nodes.iter_mut().find(|n| n.id == id)?;
        }
        Some(node)
    }

    /// Change the model for one run of a batch: `$input` sets a value input (or the path of a file input);
    /// `node.name` (`node/inner.name` in a body) sets the literal input of that name, else the setting.
    pub fn apply(&mut self, overrides: &BTreeMap<String, serde_json::Value>) -> Result<()> {
        for (key, value) in overrides {
            if let Some(name) = key.strip_prefix('$') {
                let input = self.inputs.get_mut(name).ok_or_else(|| Error::Model(format!("no input ${name}")))?;
                match input {
                    Input::Value { value: v, .. } => *v = value.clone(),
                    Input::Map { path, .. } | Input::Stack { path, .. } => {
                        *path = value
                            .as_str()
                            .ok_or_else(|| Error::Model(format!("${name} is a file: give its path")))?
                            .to_string();
                    }
                }
                continue;
            }
            let (path, name) =
                key.rsplit_once('.').ok_or_else(|| Error::Model(format!("{key:?}: give $input or node.setting")))?;
            let node = self.node_mut(path).ok_or_else(|| Error::Model(format!("{key:?}: no node {path:?}")))?;
            match node.inputs.get_mut(name) {
                Some(Source::Literal(v)) => *v = value.clone(),
                Some(Source::Ref(r)) => {
                    return Err(Error::Model(format!("{key:?}: the input comes from {r}, not a value")));
                }
                None => {
                    node.params.insert(name.to_string(), value.clone());
                }
            }
        }
        Ok(())
    }
}

/// `path` relative to `dir` when they share a root (with `..` where needed), else as it is; with forward
/// slashes, as the model file stores paths. On Windows the comparison ignores case.
pub fn relative_path(path: &Path, dir: &Path) -> String {
    let p: Vec<_> = path.components().collect();
    let d: Vec<_> = dir.components().collect();
    let same = |a: &std::path::Component, b: &std::path::Component| {
        if cfg!(windows) {
            a.as_os_str().to_string_lossy().to_lowercase() == b.as_os_str().to_string_lossy().to_lowercase()
        } else {
            a == b
        }
    };
    let common = p.iter().zip(&d).take_while(|(a, b)| same(a, b)).count();
    let s = if common == 0 {
        path.to_string_lossy().into_owned()
    } else {
        let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), d.len() - common).collect();
        parts.extend(p[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
        parts.join("/")
    };
    s.replace('\\', "/")
}

/// The order to run nodes in: each after the nodes it depends on in the same scope (an error on a cycle).
///
/// `deps(node)` gives the ids, in the same scope, that the node depends on.
pub fn order(nodes: &[Node], deps: impl Fn(&Node) -> Vec<String>) -> Result<Vec<&Node>> {
    let index: BTreeMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let mut state = vec![0u8; nodes.len()]; // 0 new, 1 visiting, 2 done
    let mut out = Vec::with_capacity(nodes.len());
    fn visit<'a>(
        i: usize,
        nodes: &'a [Node],
        index: &BTreeMap<&str, usize>,
        deps: &dyn Fn(&Node) -> Vec<String>,
        state: &mut [u8],
        out: &mut Vec<&'a Node>,
    ) -> Result<()> {
        match state[i] {
            2 => return Ok(()),
            1 => return Err(Error::Model(format!("the graph has a cycle through node {:?}", nodes[i].id))),
            _ => {}
        }
        state[i] = 1;
        for d in deps(&nodes[i]) {
            if let Some(&j) = index.get(d.as_str()) {
                visit(j, nodes, index, deps, state, out)?;
            }
        }
        state[i] = 2;
        out.push(&nodes[i]);
        Ok(())
    }
    for i in 0..nodes.len() {
        visit(i, nodes, &index, &deps, &mut state, &mut out)?;
    }
    Ok(out)
}

/// Every reference a node uses, its body's included (those to the body's own nodes and carried values too).
pub fn references(node: &Node) -> Vec<String> {
    let mut out: Vec<String> = node
        .inputs
        .values()
        .filter_map(|s| match s {
            Source::Ref(r) => Some(r.clone()),
            Source::Literal(_) => None,
        })
        .collect();
    if let Some(b) = &node.body {
        for n in &b.nodes {
            out.extend(references(n));
        }
        out.extend(b.carry.values().cloned());
        out.extend(b.collect.values().cloned());
        out.extend(b.outputs.values().cloned());
    }
    out
}

/// The ids of the nodes of `scope` that a node depends on: the nodes its references (its body's
/// included) name, when they are in that scope and not in its own body.
pub fn dependencies(node: &Node, scope: &[Node]) -> Vec<String> {
    let own: Vec<&str> =
        node.body.as_ref().map(|b| b.nodes.iter().map(|n| n.id.as_str()).collect()).unwrap_or_default();
    let mut out = Vec::new();
    for r in references(node) {
        if let Ok(Ref::Port { node: id, .. }) = Ref::parse(&r)
            && !own.contains(&id.as_str())
            && scope.iter().any(|n| n.id == id)
            && !out.contains(&id)
        {
            out.push(id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Model {
        let mut m = Model::new("sample");
        m.inputs.insert(
            "t0".into(),
            Input::Map { path: "lu.tif".into(), band: Some("1997".into()), nodata: None, note: None },
        );
        m.inputs.insert("seed".into(), Input::Value { value: 7.into(), note: None });
        m.nodes.push(Node::new("b", "x.op").input("a", "a.out").literal("k", 3));
        m.nodes.push(Node::new("a", "x.op").input("t", "$t0").param("steps", 14).note("why"));
        m.nodes.push(Node::new("loop", "control.repeat").input("land", "$t0").literal("times", 3).body(Body {
            nodes: vec![Node::new("step", "x.op").input("land", "@land").input("p", "b.out")],
            carry: [("land".to_string(), "step.out".to_string())].into(),
            collect: [("maps".to_string(), "step.out".to_string())].into(),
            ..Default::default()
        }));
        m.outputs.insert("maps".into(), Output { from: "loop.maps".into(), path: None, dtype: Some("u8".into()) });
        m
    }

    #[test]
    fn references_parse() {
        assert_eq!(Ref::parse("$t0").unwrap(), Ref::Input("t0".into()));
        assert_eq!(Ref::parse("@land").unwrap(), Ref::Carried("land".into()));
        assert_eq!(Ref::parse("woe.model").unwrap(), Ref::Port { node: "woe".into(), port: "model".into() });
        for bad in ["", "$", "woe", "a.b.c", "1a.b", "a. b", "@x y"] {
            assert!(Ref::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn toml_and_json_round_trip() {
        let m = sample();
        let t = m.to_toml().unwrap();
        assert_eq!(Model::from_toml(&t).unwrap(), m, "{t}");
        let j = m.to_json().unwrap();
        assert_eq!(Model::from_json(&j).unwrap(), m);
    }

    #[test]
    fn literals_and_references() {
        let t = r#"
format = "terraflux/1"
[[nodes]]
id = "n"
op = "x.op"
inputs = { a = "m.out", k = 14, f = 0.5, flag = true }
"#;
        let m = Model::from_toml(t).unwrap();
        let n = &m.nodes[0];
        assert_eq!(n.inputs["a"], Source::Ref("m.out".into()));
        assert_eq!(n.inputs["k"], Source::Literal(14.into()));
        assert_eq!(n.inputs["flag"], Source::Literal(true.into()));
    }

    #[test]
    fn other_formats_are_refused() {
        assert!(Model::from_toml("format = \"terraflux/9\"").is_err());
    }

    #[test]
    fn order_follows_dependencies() {
        let m = sample();
        let o = order(&m.nodes, |n| dependencies(n, &m.nodes)).unwrap();
        let ids: Vec<&str> = o.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "loop"]);
    }

    #[test]
    fn overrides() {
        let mut m = sample();
        let o: BTreeMap<String, serde_json::Value> = [
            ("$seed".to_string(), serde_json::json!(9)),
            ("$t0".to_string(), serde_json::json!("other.tif")),
            ("a.steps".to_string(), serde_json::json!(3)),
            ("b.k".to_string(), serde_json::json!(5)),
            ("loop/step.new".to_string(), serde_json::json!(true)),
        ]
        .into();
        m.apply(&o).unwrap();
        assert!(matches!(&m.inputs["seed"], Input::Value { value, .. } if value == 9));
        assert!(matches!(&m.inputs["t0"], Input::Map { path, .. } if path == "other.tif"));
        assert_eq!(m.node("a").unwrap().params["steps"], 3);
        assert_eq!(m.node("b").unwrap().inputs["k"], Source::Literal(5.into()));
        assert_eq!(m.node("loop").unwrap().body.as_ref().unwrap().nodes[0].params["new"], true);
        for bad in ["$nope", "zz.x", "b.a", "plain"] {
            let o: BTreeMap<String, serde_json::Value> = [(bad.to_string(), serde_json::json!(1))].into();
            assert!(m.clone().apply(&o).is_err(), "{bad}");
        }
    }

    #[test]
    fn cycles_are_errors() {
        let nodes = vec![Node::new("a", "x").input("i", "b.out"), Node::new("b", "x").input("i", "a.out")];
        assert!(order(&nodes, |n| dependencies(n, &nodes)).is_err());
    }
}
