//! Submodels: a node `control.submodel` stands for another model file (`params.path`, relative to the folder
//! of the model that uses it). Before a run, [`expand`] puts the other model's nodes in the node's body and
//! makes the node a `control.group`:
//!
//! - the submodel's inputs `$name` become `@name`, the node's inputs of the same names; an input the node does
//!   not give keeps its value (a value input) or becomes an input of the outer model (a file, its path made
//!   relative to the outer model's folder, named `node__name`);
//! - the submodel's outputs (`name -> node.port`) become the group's outputs, `node.name` outside.
//!
//! Submodels may use submodels (up to 16 deep, and never themselves). The paths of plugins and of the rasters
//! read and written by nodes become absolute (each relative to the folder of the model that names it).

use std::path::{Path, PathBuf};

use crate::{Body, Error, GROUP, Input, Model, Node, Result, SUBMODEL, Source, Type, relative_path};

const DEPTH: usize = 16;

/// What a submodel file says about itself, to show beside its node: what it is (the model's `summary`, else its
/// `description`), what it takes (its inputs, typed by what they hold) and what it gives (its outputs and where
/// they come from). A model file has no version of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct About {
    pub description: Option<String>,
    /// Its inputs by name: the type, the value it has when the node gives none (a file input: none, it becomes an
    /// input of the outer model) and its note.
    pub inputs: Vec<AboutInput>,
    /// Its outputs: the name and the reference of its body that gives it.
    pub outputs: Vec<(String, String)>,
}

/// An input of a submodel.
#[derive(Debug, Clone, PartialEq)]
pub struct AboutInput {
    pub name: String,
    pub ty: Type,
    pub value: Option<serde_json::Value>,
    pub note: Option<String>,
}

/// What a submodel file says about itself ([`About`]); its nodes are not expanded.
pub fn about(file: &Path) -> Result<About> {
    let m = Model::load(file)?;
    let inputs = m
        .inputs
        .iter()
        .map(|(name, i)| {
            let (ty, value, note) = match i {
                Input::Map { note, .. } => (Type::Map, None, note.clone()),
                Input::Stack { note, .. } => (Type::Stack, None, note.clone()),
                Input::Value { value, note } => (value_type(value), Some(value.clone()), note.clone()),
            };
            AboutInput { name: name.clone(), ty, value, note }
        })
        .collect();
    Ok(About {
        description: m.summary.or(m.description).filter(|d| !d.trim().is_empty()),
        inputs,
        outputs: m.outputs.into_iter().map(|(k, o)| (k, o.from)).collect(),
    })
}

fn value_type(v: &serde_json::Value) -> Type {
    match v {
        serde_json::Value::Bool(_) => Type::Bool,
        serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => Type::Integer,
        serde_json::Value::Number(_) => Type::Number,
        serde_json::Value::String(_) => Type::Text,
        _ => Type::Any,
    }
}

/// Expand every submodel of a model (`dir`: the model's folder).
pub fn expand(model: &mut Model, dir: &Path) -> Result<()> {
    let mut extra = Vec::new();
    expand_nodes(&mut model.nodes, dir, dir, &mut extra, &mut Vec::new())?;
    for (name, input) in extra {
        model.inputs.insert(name, input);
    }
    Ok(())
}

/// Whether a model uses submodels (anywhere).
pub fn has_submodels(model: &Model) -> bool {
    fn any(nodes: &[Node]) -> bool {
        nodes.iter().any(|n| n.op == SUBMODEL || n.body.as_ref().is_some_and(|b| any(&b.nodes)))
    }
    any(&model.nodes)
}

fn expand_nodes(
    nodes: &mut [Node],
    dir: &Path,
    top: &Path,
    extra: &mut Vec<(String, Input)>,
    stack: &mut Vec<PathBuf>,
) -> Result<()> {
    for n in nodes.iter_mut() {
        for p in crate::file_settings(n) {
            if Path::new(p.as_str()).is_relative() {
                *p = dir.join(p.as_str()).to_string_lossy().into_owned();
            }
        }
        if n.op == SUBMODEL {
            expand_node(n, dir, top, extra, stack)?;
        } else if let Some(b) = &mut n.body {
            expand_nodes(&mut b.nodes, dir, top, extra, stack)?;
        }
    }
    Ok(())
}

fn expand_node(
    n: &mut Node,
    dir: &Path,
    top: &Path,
    extra: &mut Vec<(String, Input)>,
    stack: &mut Vec<PathBuf>,
) -> Result<()> {
    let path = n
        .params
        .get("path")
        .and_then(|p| p.as_str())
        .map(str::to_string)
        .ok_or_else(|| Error::Model(format!("the submodel {:?} has no path", n.id)))?;
    let file = if Path::new(&path).is_absolute() { PathBuf::from(&path) } else { dir.join(&path) };
    let canon = file.canonicalize().unwrap_or_else(|_| file.clone());
    if stack.contains(&canon) {
        return Err(Error::Model(format!("the submodel {:?} uses itself ({})", n.id, file.display())));
    }
    if stack.len() >= DEPTH {
        return Err(Error::Model(format!("submodels nested more than {DEPTH} deep at {:?}", n.id)));
    }
    let mut sub =
        Model::load(&file).map_err(|e| Error::Model(format!("the submodel {:?}: {}: {e}", n.id, file.display())))?;
    let sub_dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
    stack.push(canon);
    let mut inner_extra = Vec::new();
    expand_nodes(&mut sub.nodes, &sub_dir, top, &mut inner_extra, stack)?;
    stack.pop();
    for (name, input) in inner_extra {
        sub.inputs.insert(name, input);
    }
    // the submodel's inputs: the node's, else their own values, else inputs of the outer model
    for (name, input) in &sub.inputs {
        if n.inputs.contains_key(name) {
            continue;
        }
        match input {
            Input::Value { value, .. } => {
                n.inputs.insert(name.clone(), Source::Literal(value.clone()));
            }
            Input::Map { path, .. } | Input::Stack { path, .. } => {
                let outer = format!("{}__{name}", n.id);
                let mut i = input.clone();
                let full = if Path::new(path).is_absolute() { PathBuf::from(path) } else { sub_dir.join(path) };
                let rel = relative_path(&full, top);
                if let Input::Map { path, .. } | Input::Stack { path, .. } = &mut i {
                    *path = rel;
                }
                extra.push((outer.clone(), i));
                n.inputs.insert(name.clone(), Source::Ref(format!("${outer}")));
            }
        }
    }
    for name in n.inputs.keys() {
        if !sub.inputs.contains_key(name) {
            return Err(Error::Model(format!("the submodel {:?} has no input {name:?}", n.id)));
        }
    }
    let mut nodes = sub.nodes;
    for m in nodes.iter_mut() {
        rename_inputs(m);
    }
    n.body = Some(Body {
        nodes,
        outputs: sub.outputs.into_iter().map(|(k, o)| (k, o.from)).collect(),
        ..Default::default()
    });
    n.op = GROUP.to_string();
    n.params.remove("path");
    let from = format!("from {path}");
    n.note = Some(match &n.note {
        Some(x) => format!("{x} ({from})"),
        None => from,
    });
    Ok(())
}

/// `$name` -> `@name` in a node and its body.
fn rename_inputs(n: &mut Node) {
    let fix = |r: &mut String| {
        if let Some(name) = r.strip_prefix('$') {
            *r = format!("@{name}");
        }
    };
    for s in n.inputs.values_mut() {
        if let Source::Ref(r) = s {
            fix(r);
        }
    }
    // (the settings that read the submodel's inputs too: its settings given from outside)
    n.params.values_mut().filter_map(crate::setting_ref_mut).for_each(fix);
    if let Some(b) = &mut n.body {
        for m in b.nodes.iter_mut() {
            rename_inputs(m);
        }
        b.carry.values_mut().for_each(fix);
        b.collect.values_mut().for_each(fix);
        b.outputs.values_mut().for_each(fix);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("terraflux-submodel-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn submodels_become_groups() {
        let d = tmp("groups");
        std::fs::create_dir_all(d.join("lib")).unwrap();
        std::fs::write(
            d.join("lib/scale.toml"),
            r#"format = "terraflux/1"
[inputs.x]
kind = "value"
value = 1
[inputs.k]
kind = "value"
value = 10
[inputs.mask]
kind = "map"
path = "mask.tif"
[[nodes]]
id = "times"
op = "test.mul"
inputs = { a = "$x", b = "$k", m = "$mask" }
[outputs.y]
from = "times.out"
"#,
        )
        .unwrap();
        let mut m = Model::new("outer");
        m.inputs.insert("v".into(), Input::Value { value: 3.into(), note: None });
        m.nodes.push(Node::new("s", SUBMODEL).input("x", "$v").param("path", "lib/scale.toml"));
        m.nodes.push(Node::new("after", "test.mul").input("a", "s.y").literal("b", 2));
        assert!(has_submodels(&m));
        expand(&mut m, &d).unwrap();
        assert!(!has_submodels(&m));
        let s = m.node("s").unwrap();
        assert_eq!(s.op, GROUP);
        let b = s.body.as_ref().unwrap();
        assert_eq!(b.nodes[0].inputs["a"], Source::Ref("@x".into()));
        assert_eq!(b.outputs["y"], "times.out");
        // the value input kept its value, the file became an input of the outer model
        assert_eq!(s.inputs["k"], Source::Literal(10.into()));
        assert_eq!(s.inputs["mask"], Source::Ref("$s__mask".into()));
        assert!(matches!(&m.inputs["s__mask"], Input::Map { path, .. } if path == "lib/mask.tif"));
    }

    #[test]
    fn a_submodel_using_itself_is_refused() {
        let d = tmp("self");
        std::fs::write(
            d.join("me.toml"),
            "format = \"terraflux/1\"\n[[nodes]]\nid = \"again\"\nop = \"control.submodel\"\nparams = { path = \"me.toml\" }\n",
        )
        .unwrap();
        let mut m = Model::new("outer");
        m.nodes.push(Node::new("s", SUBMODEL).param("path", "me.toml"));
        let e = expand(&mut m, &d).err().unwrap().to_string();
        assert!(e.contains("uses itself"), "{e}");
        let mut m = Model::new("outer");
        m.nodes.push(Node::new("s", SUBMODEL).param("path", "me.toml").literal("nope", 1));
        assert!(expand(&mut m, &d).is_err());
    }

    #[test]
    fn files_named_by_settings_follow_the_model() {
        let d = tmp("files");
        let mut m = Model::new("m");
        m.nodes.push(Node::new("hq", "services.habitat_quality").param("threats", "threats.csv"));
        m.nodes.push(
            Node::new("hq2", "services.habitat_quality").param("threats", serde_json::json!([{"threat": "roads"}])),
        );
        expand(&mut m, &d).unwrap();
        // a file named by a text is the model's folder's; a list of threats is left alone
        assert_eq!(m.node("hq").unwrap().params["threats"], d.join("threats.csv").to_string_lossy().as_ref());
        assert!(m.node("hq2").unwrap().params["threats"].is_array());
        crate::relative_paths(&mut m.nodes, &d);
        assert_eq!(m.node("hq").unwrap().params["threats"], "threats.csv");
    }

    #[test]
    fn a_submodels_settings_read_its_inputs() {
        let d = tmp("settings");
        std::fs::write(
            d.join("inner.toml"),
            r#"format = "terraflux/1"
[inputs.k]
kind = "value"
value = 2
[[nodes]]
id = "t"
op = "test.scale"
params = { factor = "$k", other = { from = "$k" } }
[outputs.y]
from = "t.map"
"#,
        )
        .unwrap();
        std::fs::write(
            d.join("outer.toml"),
            r#"format = "terraflux/1"
summary = "scales twice"
[inputs.f]
kind = "value"
value = 5
[inputs.m]
kind = "map"
path = "m.tif"
[[nodes]]
id = "in"
op = "control.submodel"
inputs = { k = "$f" }
params = { path = "inner.toml" }
[outputs.y]
from = "in.y"
"#,
        )
        .unwrap();
        // a setting of a node of the submodel reads the submodel's input: the node's, the submodel's own value
        for (given, want) in [(Some(7), Source::Literal(7.into())), (None, Source::Literal(2.into()))] {
            let mut m = Model::new("m");
            let mut n = Node::new("s", SUBMODEL).param("path", "inner.toml");
            if let Some(g) = given {
                n = n.literal("k", g);
            }
            m.nodes.push(n);
            expand(&mut m, &d).unwrap();
            let s = m.node("s").unwrap();
            assert_eq!(s.inputs["k"], want);
            let t = &s.body.as_ref().unwrap().nodes[0];
            assert_eq!(t.params["factor"], "@k");
            assert_eq!(t.params["other"], serde_json::json!({ "from": "@k" }));
            // what the body reads is what the node has
            assert_eq!(crate::user_node::read_inputs(s.body.as_ref().unwrap()), vec!["k".to_string()]);
        }
        // given from outside by a reference, through a submodel inside a submodel
        let mut m = Model::new("m");
        m.nodes.push(Node::new("o", SUBMODEL).param("path", "outer.toml"));
        expand(&mut m, &d).unwrap();
        let inner = &m.node("o").unwrap().body.as_ref().unwrap().nodes[0];
        assert_eq!(inner.op, GROUP);
        assert_eq!(inner.inputs["k"], Source::Ref("@f".into()));
        assert_eq!(inner.body.as_ref().unwrap().nodes[0].params["factor"], "@k");
        // a setting the submodel gives no input for: it is refused
        let mut m = Model::new("m");
        m.nodes.push(Node::new("s", SUBMODEL).param("path", "inner.toml").literal("factor", 3));
        let e = expand(&mut m, &d).err().unwrap().to_string();
        assert!(e.contains("has no input \"factor\""), "{e}");
    }

    #[test]
    fn a_submodel_describes_itself() {
        let d = tmp("about");
        std::fs::write(
            d.join("s.toml"),
            r#"format = "terraflux/1"
summary = "scales a map"
[inputs.k]
kind = "value"
value = 2.5
note = "the factor"
[inputs.n]
kind = "value"
value = 3
[inputs.m]
kind = "map"
path = "m.tif"
[outputs.y]
from = "t.map"
"#,
        )
        .unwrap();
        let a = about(&d.join("s.toml")).unwrap();
        assert_eq!(a.description.as_deref(), Some("scales a map"));
        let kinds: Vec<_> = a.inputs.iter().map(|i| (i.name.as_str(), i.ty)).collect();
        assert_eq!(kinds, [("k", Type::Number), ("m", Type::Map), ("n", Type::Integer)]);
        assert_eq!(a.inputs[0].note.as_deref(), Some("the factor"));
        assert_eq!(a.outputs, [("y".to_string(), "t.map".to_string())]);
        assert!(about(&d.join("none.toml")).is_err());
    }
}
