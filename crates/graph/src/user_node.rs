//! The user's own nodes: a group of nodes (a calculator's script, most often) saved in the user's library and
//! used as one operator, with a description, typed ports and what each input needs (a map or a stack, the layers
//! it reads).
//!
//! In a model such a node is a group (`control.group`) with [`Uses`]: the name and version it was made from and
//! what it takes and gives. Its body is a copy of that version's: the model runs the version it was made with, on
//! any computer, with or without the library, and a newer version saved later changes nothing until the node is
//! updated to it ([`update`]).
//!
//! The library is a folder of files, `name@version.toml`, each a [`UserNode`]; a version once saved is never
//! written again ([`Library::save`] gives a new one).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Body, Error, FORMAT, GROUP, Node, Result, Source, Type, valid_id};

/// What a group is when it is a node of the user's library: its name and version, what it does, and what it takes
/// and gives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Uses {
    pub name: String,
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Its inputs by name (`@name` in its body).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, UserInput>,
    /// Its outputs by name (the body's outputs).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, UserOutput>,
}

impl Uses {
    /// How it is written: `ndvi@3`.
    pub fn label(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

/// An input of a node of the user's: its type, the layers it needs (of a stack, by the names a script reads them:
/// `nir`, `_1997`), its value when none is given (then it is not needed), what it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserInput {
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bands: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

impl UserInput {
    pub fn new(ty: Type) -> UserInput {
        UserInput { ty, bands: Vec::new(), default: None, doc: None }
    }

    /// Whether a node must be given it (it has no value of its own).
    pub fn required(&self) -> bool {
        self.default.is_none()
    }

    /// The type, its layers and its value as a line: `stack (nir, red)`, `number = 0.5`.
    pub fn summary(&self) -> String {
        let mut s = self.ty.name().to_string();
        if !self.bands.is_empty() {
            s.push_str(&format!(" ({})", self.bands.join(", ")));
        }
        if let Some(d) = &self.default {
            s.push_str(&format!(" = {d}"));
        }
        s
    }
}

/// An output of a node of the user's: its type and what it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserOutput {
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

impl UserOutput {
    pub fn new(ty: Type) -> UserOutput {
        UserOutput { ty, doc: None }
    }
}

/// A version of a node of the user's library, as its file holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserNode {
    /// The format version (`terraflux/1`, a model's).
    pub format: String,
    pub name: String,
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, UserInput>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, UserOutput>,
    /// Its nodes (reading its inputs as `@name`) and the references giving its outputs.
    pub body: Body,
}

impl UserNode {
    /// A node of what it takes and gives (`uses`, its version kept) and a body.
    pub fn new(uses: Uses, body: Body) -> UserNode {
        UserNode {
            format: FORMAT.to_string(),
            name: uses.name,
            version: uses.version,
            description: uses.description,
            inputs: uses.inputs,
            outputs: uses.outputs,
            body,
        }
    }

    /// The node a group of a model is (none: a plain group).
    pub fn of(n: &Node) -> Option<UserNode> {
        Some(UserNode::new(n.uses.clone()?, n.body.clone().unwrap_or_default()))
    }

    pub fn uses(&self) -> Uses {
        Uses {
            name: self.name.clone(),
            version: self.version,
            description: self.description.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
        }
    }

    pub fn label(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<UserNode> {
        UserNode::from_toml(&std::fs::read_to_string(path)?)
    }

    pub fn from_toml(text: &str) -> Result<UserNode> {
        let u: UserNode = toml::from_str(text)?;
        if u.format != FORMAT {
            return Err(Error::Model(format!(
                "this node is in format {:?}; this version of Terraflux reads {FORMAT:?}",
                u.format
            )));
        }
        Ok(u)
    }

    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// What is wrong with it, as a node to save: a name that is no id, an output of the body not declared or one
    /// declared the body does not give, an input its body reads and it does not declare.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !valid_id(&self.name) {
            out.push(format!("{:?} is not a valid name (a letter, then letters, digits, _ or -)", self.name));
        }
        for k in self.body.outputs.keys().filter(|k| !self.outputs.contains_key(*k)) {
            out.push(format!("the output {k:?} has no type"));
        }
        for k in self.outputs.keys().filter(|k| !self.body.outputs.contains_key(*k)) {
            out.push(format!("the body gives no output {k:?}"));
        }
        if self.outputs.is_empty() {
            out.push("it gives nothing: no output".into());
        }
        for k in read_inputs(&self.body).into_iter().filter(|k| !self.inputs.contains_key(k)) {
            out.push(format!("the body reads @{k}, not an input"));
        }
        out
    }

    /// A node of a model made of it: a group with its body and [`Uses`], its inputs that have a value of their own
    /// given it.
    pub fn instance(&self, id: &str) -> Node {
        let mut n = Node::new(id, GROUP).body(self.body.clone());
        n.uses = Some(self.uses());
        for (k, i) in &self.inputs {
            if let Some(v) = &i.default {
                n.inputs.insert(k.clone(), Source::Literal(v.clone()));
            }
        }
        n
    }

    /// Whether a node of a model made of this version was changed since (its body or what it takes and gives;
    /// where its nodes are drawn aside).
    pub fn differs(&self, n: &Node) -> bool {
        let Some(u) = &n.uses else { return true };
        let mine = self.uses();
        let (a, b) = (bare(n.body.as_ref().unwrap_or(&Body::default())), bare(&self.body));
        a != b || u.inputs != mine.inputs || u.outputs != mine.outputs || u.description != mine.description
    }
}

/// A body without where its nodes are drawn.
fn bare(b: &Body) -> Body {
    let mut b = b.clone();
    fn strip(nodes: &mut [Node]) {
        for n in nodes {
            n.pos = None;
            n.width = None;
            if let Some(b) = &mut n.body {
                strip(&mut b.nodes);
            }
        }
    }
    strip(&mut b.nodes);
    b
}

/// The names a body reads as `@name` that its own containers do not give: the inputs of the group it is.
pub fn read_inputs(b: &Body) -> Vec<String> {
    fn walk(nodes: &[Node], own: &[String], out: &mut Vec<String>) {
        for n in nodes {
            let refs = n
                .inputs
                .values()
                .filter_map(|s| match s {
                    Source::Ref(r) => Some(r.as_str()),
                    Source::Literal(_) => None,
                })
                .chain(n.params.values().filter_map(crate::setting_ref));
            for r in refs {
                if let Some(name) = r.strip_prefix('@')
                    && !own.iter().any(|o| o == name)
                    && !out.iter().any(|o| o == name)
                {
                    out.push(name.to_string());
                }
            }
            if let Some(b) = &n.body {
                // what the container gives its body: a group's inputs, a loop's step and carried values, an each's
                // element
                let mut inner = own.to_vec();
                inner.extend(n.inputs.keys().cloned());
                inner.extend(b.carry.keys().cloned());
                if n.op != GROUP {
                    inner.extend(["step", "item", "index", "name"].map(String::from));
                }
                walk(&b.nodes, &inner, out);
                let outer: Vec<&String> =
                    b.carry.values().chain(b.collect.values()).chain(b.outputs.values()).collect();
                for r in outer {
                    if let Some(name) = r.strip_prefix('@')
                        && !inner.iter().any(|o| o == name)
                        && !out.iter().any(|o| o == name)
                    {
                        out.push(name.to_string());
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(&b.nodes, &[], &mut out);
    for r in b.outputs.values() {
        if let Some(name) = r.strip_prefix('@')
            && !out.iter().any(|o| o == name)
        {
            out.push(name.to_string());
        }
    }
    out
}

/// A node of a model brought to another version of what it was made of: its body and what it takes and gives that
/// version's, the inputs it was given kept where the version still has them, the version's own values for the
/// new ones. The inputs it no longer has, taken away.
pub fn update(n: &mut Node, u: &UserNode) -> Vec<String> {
    let gone: Vec<String> = n.inputs.keys().filter(|k| !u.inputs.contains_key(*k)).cloned().collect();
    for k in &gone {
        n.inputs.remove(k);
    }
    for (k, i) in &u.inputs {
        if let (false, Some(v)) = (n.inputs.contains_key(k), &i.default) {
            n.inputs.insert(k.clone(), Source::Literal(v.clone()));
        }
    }
    n.op = GROUP.to_string();
    n.body = Some(u.body.clone());
    n.uses = Some(u.uses());
    gone
}

/// The user's library of nodes: a folder of `name@version.toml` files.
#[derive(Debug, Clone)]
pub struct Library {
    pub dir: PathBuf,
}

impl Library {
    pub fn new(dir: impl Into<PathBuf>) -> Library {
        Library { dir: dir.into() }
    }

    fn file(&self, name: &str, version: u32) -> PathBuf {
        self.dir.join(format!("{name}@{version}.toml"))
    }

    /// Every version of every node in the folder, by name and version (files that do not read left out).
    fn all(&self) -> BTreeMap<String, Vec<u32>> {
        let mut out: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return out };
        for e in entries.flatten() {
            let p = e.path();
            if !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("toml")) {
                continue;
            }
            let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
            if let Some((name, v)) = stem.rsplit_once('@')
                && let Ok(v) = v.parse::<u32>()
                && valid_id(name)
            {
                out.entry(name.to_string()).or_default().push(v);
            }
        }
        for v in out.values_mut() {
            v.sort_unstable();
        }
        out
    }

    /// The last version of each node, by name.
    pub fn list(&self) -> Vec<UserNode> {
        self.all()
            .into_iter()
            .filter_map(|(name, v)| v.iter().rev().find_map(|&k| UserNode::load(self.file(&name, k)).ok()))
            .collect()
    }

    /// The versions of a node, the first first.
    pub fn versions(&self, name: &str) -> Vec<u32> {
        self.all().remove(name).unwrap_or_default()
    }

    /// A version of a node (none: the last).
    pub fn get(&self, name: &str, version: Option<u32>) -> Result<UserNode> {
        let v = match version {
            Some(v) => v,
            None => *self
                .versions(name)
                .last()
                .ok_or_else(|| Error::Model(format!("no node {name:?} in the library ({})", self.dir.display())))?,
        };
        UserNode::load(self.file(name, v)).map_err(|e| Error::Model(format!("{name}@{v}: {e}")))
    }

    /// Save a node as the next version of its name (the first: 1), unless it is the same as the last version: the
    /// node as saved (its version set). Refused when it has [`UserNode::problems`].
    pub fn save(&self, node: &UserNode) -> Result<UserNode> {
        if let Some(p) = node.problems().into_iter().next() {
            return Err(Error::Model(p));
        }
        let mut u = node.clone();
        u.format = FORMAT.to_string();
        if let Some(&last) = self.versions(&u.name).last() {
            if let Ok(prev) = self.get(&u.name, Some(last))
                && !prev.differs(&u.instance("x"))
            {
                return Ok(prev);
            }
            u.version = last + 1;
        } else {
            u.version = 1;
        }
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.file(&u.name, u.version), u.to_toml()?)?;
        Ok(u)
    }

    /// Take a node and all its versions out of the library (the models using it keep their copies).
    pub fn remove(&self, name: &str) -> Result<()> {
        for v in self.versions(name) {
            std::fs::remove_file(self.file(name, v))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Model;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("terraflux-user-node-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn ndvi() -> UserNode {
        let mut inputs = BTreeMap::new();
        inputs.insert(
            "img".to_string(),
            UserInput { bands: vec!["nir".into(), "red".into()], ..UserInput::new(Type::Stack) },
        );
        inputs.insert("k".to_string(), UserInput { default: Some(0.5.into()), ..UserInput::new(Type::Number) });
        UserNode::new(
            Uses {
                name: "ndvi".into(),
                version: 0,
                description: Some("the vegetation index".into()),
                inputs,
                outputs: [("ndvi".to_string(), UserOutput::new(Type::Map))].into(),
            },
            Body {
                nodes: vec![
                    Node::new("calc", "map.calc")
                        .input("img", "@img")
                        .input("k", "@k")
                        .param("expression", "out ndvi = (img.nir - img.red) / (img.nir + img.red) * k"),
                ],
                outputs: [("ndvi".to_string(), "calc.ndvi".to_string())].into(),
                ..Default::default()
            },
        )
    }

    #[test]
    fn versions_are_saved_once_each() {
        let lib = Library::new(tmp("versions"));
        let one = lib.save(&ndvi()).unwrap();
        assert_eq!(one.version, 1);
        // the same again: no new version
        assert_eq!(lib.save(&ndvi()).unwrap().version, 1);
        let mut changed = ndvi();
        changed.body.nodes[0].params.insert("expression".into(), "out ndvi = img.nir - img.red".into());
        assert_eq!(lib.save(&changed).unwrap().version, 2);
        assert_eq!(lib.versions("ndvi"), vec![1, 2]);
        assert_eq!(lib.get("ndvi", None).unwrap().version, 2);
        assert_eq!(lib.get("ndvi", Some(1)).unwrap().body, ndvi().body);
        let listed = lib.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].label(), "ndvi@2");
        lib.remove("ndvi").unwrap();
        assert!(lib.list().is_empty());
    }

    #[test]
    fn a_node_with_problems_is_not_saved() {
        let lib = Library::new(tmp("problems"));
        let mut u = ndvi();
        u.inputs.remove("img");
        assert!(lib.save(&u).unwrap_err().to_string().contains("@img"));
        let mut u = ndvi();
        u.outputs.insert("other".into(), UserOutput::new(Type::Map));
        assert!(lib.save(&u).is_err());
        let mut u = ndvi();
        u.name = "two words".into();
        assert!(lib.save(&u).is_err());
    }

    #[test]
    fn a_model_keeps_its_copy_and_round_trips() {
        let mut u = ndvi();
        u.version = 3;
        let n = u.instance("veg");
        assert_eq!(n.op, GROUP);
        assert_eq!(n.uses.as_ref().unwrap().label(), "ndvi@3");
        // the input of a value of its own given it; the stack not
        assert_eq!(n.inputs["k"], Source::Literal(0.5.into()));
        assert!(!n.inputs.contains_key("img"));
        let mut m = Model::new("m");
        m.nodes.push(n.input("img", "$s2"));
        let back = Model::from_toml(&m.to_toml().unwrap()).unwrap();
        assert_eq!(back, m);
        let n = &back.nodes[0];
        assert!(!u.differs(n));
        assert_eq!(UserNode::of(n).unwrap().body, u.body);
        // moved inside: the same; its script changed: not
        let mut moved = n.clone();
        moved.body.as_mut().unwrap().nodes[0].pos = Some([10.0, 20.0]);
        assert!(!u.differs(&moved));
        moved.body.as_mut().unwrap().nodes[0].params.insert("expression".into(), "out ndvi = 1".into());
        assert!(u.differs(&moved));
        // the file of a version
        assert_eq!(UserNode::from_toml(&u.to_toml().unwrap()).unwrap(), u);
    }

    #[test]
    fn updating_keeps_what_was_given() {
        let mut u = ndvi();
        u.version = 1;
        let mut n = u.instance("veg").input("img", "$s2").literal("k", 2.0).literal("old", 1);
        let mut v2 = ndvi();
        v2.version = 2;
        v2.inputs.remove("k");
        v2.inputs.insert("scale".into(), UserInput { default: Some(10.into()), ..UserInput::new(Type::Number) });
        v2.body.nodes[0].inputs.remove("k");
        let gone = update(&mut n, &v2);
        assert_eq!(gone, vec!["k".to_string(), "old".to_string()]);
        assert_eq!(n.inputs["img"], Source::Ref("$s2".into()));
        assert_eq!(n.inputs["scale"], Source::Literal(10.into()));
        assert_eq!(n.uses.as_ref().unwrap().version, 2);
    }

    #[test]
    fn the_inputs_a_body_reads() {
        let b = Body {
            nodes: vec![
                Node::new("a", "x").input("m", "@img").param("t", "@k"),
                Node::new("loop", "control.repeat").input("land", "@start").body(Body {
                    nodes: vec![Node::new("in", "x").input("l", "@land").input("s", "@step").input("o", "@other")],
                    carry: [("land".to_string(), "in.out".to_string())].into(),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        assert_eq!(read_inputs(&b), vec!["img", "k", "start", "other"]);
    }
}
