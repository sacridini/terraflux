//! Editing a model's graph, as the node editor does: nodes added, moved, renamed and removed, connections
//! made and undone, literal inputs and settings changed, in the top level or in a container's body (a scope:
//! the path of the containers' ids). A change that would break the model (a connection of the wrong type, a
//! cycle, a port the operator does not have) is refused with the checker's reason, and the model stays as it
//! was.

use std::collections::{HashMap, HashSet};

use terraflux_graph::{Body, GROUP, Issue, Model, Node, Ref, Severity, Source, Type, check, valid_id};
use terraflux_ops::Registry;

pub type Result<T> = std::result::Result<T, String>;

fn scope_mut<'a>(m: &'a mut Model, path: &[String]) -> Result<&'a mut Vec<Node>> {
    m.scope_mut(path).ok_or_else(|| format!("no container {}", path.join("/")))
}

fn node_mut<'a>(m: &'a mut Model, path: &[String], id: &str) -> Result<&'a mut Node> {
    scope_mut(m, path)?.iter_mut().find(|n| n.id == id).ok_or_else(|| format!("no node {id:?}"))
}

/// The prefix the checker gives the issues of a scope's nodes (`a/b/`).
fn prefix(path: &[String]) -> String {
    if path.is_empty() { String::new() } else { format!("{}/", path.join("/")) }
}

fn errors(m: &Model, r: &Registry) -> Vec<Issue> {
    check(m, r).into_iter().filter(|i| i.severity == Severity::Error).collect()
}

/// Apply a change to a copy of the model; keep it unless it brings an error about `at` (or a new cycle).
fn guarded(m: &mut Model, r: &Registry, at: &[String], change: impl FnOnce(&mut Model) -> Result<()>) -> Result<()> {
    let before: Vec<String> = errors(m, r).into_iter().map(|i| i.to_string()).collect();
    let mut copy = m.clone();
    change(&mut copy)?;
    let new: Vec<Issue> = errors(&copy, r).into_iter().filter(|i| !before.contains(&i.to_string())).collect();
    if let Some(i) = new.iter().find(|i| at.contains(&i.at) || i.message.contains("cycle")) {
        return Err(i.message.clone());
    }
    *m = copy;
    Ok(())
}

/// A fresh id in a scope from an operator's name (`calc`, `calc_2`...).
fn fresh_id(nodes: &[Node], op: &str) -> String {
    let base: String =
        op.rsplit('.').next().unwrap_or(op).chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let base = if valid_id(&base) { base } else { format!("n_{base}") };
    if !nodes.iter().any(|n| n.id == base) {
        return base;
    }
    (2..).map(|k| format!("{base}_{k}")).find(|id| !nodes.iter().any(|n| &n.id == id)).unwrap()
}

/// Add a node of an operator to a scope (containers with an empty body); its id.
pub fn add_node(m: &mut Model, r: &Registry, path: &[String], op: &str, pos: [f32; 2]) -> Result<String> {
    let sig = r.sig(op).ok_or_else(|| format!("no operator {op:?}"))?;
    let nodes = scope_mut(m, path)?;
    let id = fresh_id(nodes, op);
    let mut n = Node::new(&id, op);
    n.pos = Some(pos);
    if sig.container {
        n.body = Some(Body::default());
    }
    nodes.push(n);
    Ok(id)
}

pub fn move_node(m: &mut Model, path: &[String], id: &str, pos: [f32; 2]) -> Result<()> {
    node_mut(m, path, id)?.pos = Some(pos);
    Ok(())
}

/// The references of a scope's nodes and of what reads their results (the enclosing body's maps, the model's
/// outputs at the top level), to be changed together.
fn for_each_ref(m: &mut Model, path: &[String], f: &mut dyn FnMut(&mut String)) {
    fn in_node(n: &mut Node, f: &mut dyn FnMut(&mut String)) {
        for s in n.inputs.values_mut() {
            if let Source::Ref(r) = s {
                f(r);
            }
        }
        if let Some(b) = &mut n.body {
            for x in b.nodes.iter_mut() {
                in_node(x, f);
            }
            b.carry.values_mut().for_each(&mut *f);
            b.collect.values_mut().for_each(&mut *f);
            b.outputs.values_mut().for_each(&mut *f);
        }
    }
    if path.is_empty() {
        for o in m.outputs.values_mut() {
            f(&mut o.from);
        }
    } else if let Some(container) = m.node_mut(&path.join("/"))
        && let Some(b) = &mut container.body
    {
        b.carry.values_mut().for_each(&mut *f);
        b.collect.values_mut().for_each(&mut *f);
        b.outputs.values_mut().for_each(&mut *f);
    }
    if let Some(nodes) = m.scope_mut(path) {
        for n in nodes.iter_mut() {
            in_node(n, f);
        }
    }
}

/// The text of nodes copied, for the clipboard: JSON, its key telling it from other texts.
pub fn clip_text(nodes: &[Node]) -> String {
    serde_json::json!({ "terraflux_nodes": nodes }).to_string()
}

/// The nodes in a text of the clipboard ([`clip_text`]'s); none in another text.
pub fn clip_nodes(text: &str) -> Option<Vec<Node>> {
    let mut v: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let nodes: Vec<Node> = serde_json::from_value(v.get_mut("terraflux_nodes")?.take()).ok()?;
    (!nodes.is_empty()).then_some(nodes)
}

/// A reference of a node pasted, followed: to a node pasted with it, its new id; to the nodes of a body pasted
/// (`shadow`), the same; to anything else (another node, the model's input, outside a body the value of the
/// container of the scope), kept when `outside`, else let go (false).
fn relinked(
    r: &mut String,
    new: &HashMap<String, String>,
    shadow: &HashSet<String>,
    outside: bool,
    inner: bool,
) -> bool {
    match Ref::parse(r) {
        Ok(Ref::Port { node, port }) if !shadow.contains(&node) => match new.get(&node) {
            Some(id) => {
                *r = format!("{id}.{port}");
                true
            }
            None => outside,
        },
        Ok(Ref::Input(_)) => outside,
        Ok(Ref::Carried(_)) => inner || outside,
        _ => true,
    }
}

fn relink(n: &mut Node, new: &HashMap<String, String>, shadow: &HashSet<String>, outside: bool, inner: bool) {
    n.inputs.retain(|_, s| match s {
        Source::Ref(r) => relinked(r, new, shadow, outside, inner),
        Source::Literal(_) => true,
    });
    if let Some(b) = &mut n.body {
        let mut shadow = shadow.clone();
        shadow.extend(b.nodes.iter().map(|x| x.id.clone()));
        for x in &mut b.nodes {
            relink(x, new, &shadow, outside, true);
        }
        for map in [&mut b.carry, &mut b.collect, &mut b.outputs] {
            map.retain(|_, r| relinked(r, new, &shadow, outside, true));
        }
        if let Some(u) = &mut b.until
            && !relinked(u, new, &shadow, outside, true)
        {
            b.until = None;
        }
    }
}

/// Add nodes to a scope as new nodes (copied, duplicated): each a fresh id from its own (`calc`: `calc_2`), the
/// connections among them following them, moved by `offset`. Their connections from outside them are kept when
/// `outside` (a duplicate, in the scope of the originals), else taken away (pasted, maybe in another scope or
/// model). Their ids, in the order given.
pub fn paste_nodes(
    m: &mut Model,
    path: &[String],
    copies: &[Node],
    offset: [f32; 2],
    outside: bool,
) -> Result<Vec<String>> {
    let nodes = scope_mut(m, path)?;
    let mut taken: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let mut new = HashMap::new();
    let mut ids = Vec::new();
    for n in copies {
        // (`calc_2` copied: `calc_3`, not `calc_2_2`)
        let base = match n.id.rsplit_once('_') {
            Some((b, k)) if valid_id(b) && !k.is_empty() && k.chars().all(|c| c.is_ascii_digit()) => b,
            _ => n.id.as_str(),
        };
        let id = std::iter::once(base.to_string())
            .chain((2..).map(|k| format!("{base}_{k}")))
            .find(|id| !taken.contains(id))
            .unwrap();
        taken.insert(id.clone());
        new.insert(n.id.clone(), id.clone());
        ids.push(id);
    }
    for (n, id) in copies.iter().zip(&ids) {
        let mut c = n.clone();
        c.id = id.clone();
        c.pos = n.pos.map(|p| [p[0] + offset[0], p[1] + offset[1]]);
        relink(&mut c, &new, &HashSet::new(), outside, false);
        nodes.push(c);
    }
    Ok(ids)
}

/// Rename a node, and every reference to it.
pub fn rename_node(m: &mut Model, path: &[String], old: &str, new: &str) -> Result<()> {
    if old == new {
        return Ok(());
    }
    if !valid_id(new) {
        return Err(format!("{new:?} is not a valid id (a letter, then letters, digits, _ or -)"));
    }
    if scope_mut(m, path)?.iter().any(|n| n.id == new) {
        return Err(format!("a node is already named {new:?}"));
    }
    node_mut(m, path, old)?.id = new.to_string();
    let from = format!("{old}.");
    for_each_ref(m, path, &mut |r| {
        if let Some(port) = r.strip_prefix(&from) {
            *r = format!("{new}.{port}");
        }
    });
    Ok(())
}

/// Remove a node: what read it reads nothing (its inputs, outputs and carried values go).
pub fn remove_node(m: &mut Model, path: &[String], id: &str) -> Result<()> {
    let nodes = scope_mut(m, path)?;
    let before = nodes.len();
    nodes.retain(|n| n.id != id);
    if nodes.len() == before {
        return Err(format!("no node {id:?}"));
    }
    let from = format!("{id}.");
    let reads = |s: &Source| matches!(s, Source::Ref(r) if r.starts_with(&from));
    for n in scope_mut(m, path)?.iter_mut() {
        n.inputs.retain(|_, s| !reads(s));
    }
    if path.is_empty() {
        m.outputs.retain(|_, o| !o.from.starts_with(&from));
    } else if let Some(c) = m.node_mut(&path.join("/"))
        && let Some(b) = &mut c.body
    {
        b.carry.retain(|_, r| !r.starts_with(&from));
        b.collect.retain(|_, r| !r.starts_with(&from));
        b.outputs.retain(|_, r| !r.starts_with(&from));
    }
    Ok(())
}

/// Connect a reference (`$input`, `@carried`, `node.port`) to a node's input port, checked.
pub fn connect(m: &mut Model, r: &Registry, path: &[String], reference: &str, node: &str, port: &str) -> Result<()> {
    Ref::parse(reference).map_err(|e| e.to_string())?;
    if !terraflux_graph::valid_port(port) {
        return Err(format!("{port:?} is not a valid port name"));
    }
    let at = format!("{}{node}", prefix(path));
    guarded(m, r, &[format!("{at}.{port}"), at.clone()], |c| {
        node_mut(c, path, node)?.inputs.insert(port.to_string(), Source::Ref(reference.to_string()));
        Ok(())
    })
}

/// Remove what a node's input port reads.
pub fn disconnect(m: &mut Model, path: &[String], node: &str, port: &str) -> Result<()> {
    node_mut(m, path, node)?.inputs.remove(port);
    Ok(())
}

/// A value from what was typed: JSON when it reads as JSON, else text.
pub fn value_of(text: &str) -> serde_json::Value {
    serde_json::from_str(text.trim()).unwrap_or_else(|_| serde_json::Value::String(text.trim().to_string()))
}

/// Set a literal input (empty text: remove it), checked.
pub fn set_literal(m: &mut Model, r: &Registry, path: &[String], node: &str, port: &str, text: &str) -> Result<()> {
    if text.trim().is_empty() {
        return disconnect(m, path, node, port);
    }
    let at = format!("{}{node}", prefix(path));
    guarded(m, r, &[format!("{at}.{port}"), at.clone()], |c| {
        node_mut(c, path, node)?.inputs.insert(port.to_string(), Source::Literal(value_of(text)));
        Ok(())
    })
}

/// Set a node's note, why it is there (empty text: no note).
pub fn set_note(m: &mut Model, path: &[String], node: &str, text: &str) -> Result<()> {
    let n = node_mut(m, path, node)?;
    let t = text.trim();
    n.note = if t.is_empty() { None } else { Some(t.to_string()) };
    Ok(())
}

/// Set a setting (empty text: remove it).
pub fn set_param(m: &mut Model, path: &[String], node: &str, name: &str, text: &str) -> Result<()> {
    let n = node_mut(m, path, node)?;
    if text.trim().is_empty() {
        n.params.remove(name);
    } else {
        n.params.insert(name.to_string(), value_of(text));
    }
    Ok(())
}

/// Make the output `name` of a container read a reference of its body: a group's output, or a loop's
/// collected series (`collect`) or carried value (`carry`).
pub fn set_body_map(
    m: &mut Model,
    r: &Registry,
    path: &[String],
    which: &str,
    name: &str,
    reference: &str,
) -> Result<()> {
    if !valid_id(name) {
        return Err(format!("{name:?} is not a valid name"));
    }
    let at = path.join("/");
    guarded(m, r, &[format!("{at}/{which}.{name}"), at.clone()], |c| {
        let n = c.node_mut(&at).ok_or_else(|| format!("no container {at}"))?;
        let b = n.body.as_mut().ok_or("not a container")?;
        let map = match which {
            "outputs" => &mut b.outputs,
            "collect" => &mut b.collect,
            "carry" => &mut b.carry,
            _ => return Err(format!("no {which}")),
        };
        map.insert(name.to_string(), reference.to_string());
        Ok(())
    })
}

/// What a node may read in a scope besides the scope's nodes: the model's inputs, and in a body the values
/// its containers give (a loop's step and carried values, a group's inputs), with their types when known.
pub fn sources(m: &Model, r: &Registry, path: &[String]) -> Vec<(String, Type)> {
    let mut out: Vec<(String, Type)> = m
        .inputs
        .iter()
        .map(|(k, i)| {
            let t = match i {
                terraflux_graph::Input::Map { .. } => Type::Map,
                terraflux_graph::Input::Stack { .. } => Type::Stack,
                terraflux_graph::Input::Value { .. } => Type::Any,
            };
            (format!("${k}"), t)
        })
        .collect();
    for depth in 1..=path.len() {
        let Some(c) = m.node_mut_ref(&path[..depth]) else { continue };
        let Some(b) = &c.body else { continue };
        if c.op == GROUP {
            out.extend(c.inputs.keys().map(|k| (format!("@{k}"), Type::Any)));
        } else {
            out.push(("@step".into(), Type::Integer));
            out.extend(b.carry.keys().map(|k| (format!("@{k}"), Type::Any)));
        }
    }
    let _ = r;
    out
}

/// The input ports of a node: its operator's, then the other inputs it has (an operator's extra inputs, a
/// container's carried values).
pub fn input_ports(n: &Node, r: &Registry) -> Vec<(String, Type, bool)> {
    let mut out: Vec<(String, Type, bool)> = Vec::new();
    if let Some(sig) = r.sig(&n.op) {
        out.extend(sig.inputs.iter().map(|p| (p.name.clone(), p.ty, p.required)));
        for k in n.inputs.keys() {
            if !out.iter().any(|(x, _, _)| x == k) {
                out.push((k.clone(), sig.extra.unwrap_or(Type::Any), false));
            }
        }
    }
    out
}

/// The output ports of a node: its operator's, or a container's (carried, collected, a group's outputs).
pub fn output_ports(n: &Node, r: &Registry) -> Vec<(String, Type)> {
    let mut out: Vec<(String, Type)> = Vec::new();
    if let Some(sig) = r.sig(&n.op) {
        out.extend(sig.outputs.iter().map(|p| (p.name.clone(), p.ty)));
    }
    if let Some(b) = &n.body {
        out.extend(b.carry.keys().map(|k| (k.clone(), Type::Any)));
        out.extend(b.collect.keys().map(|k| (k.clone(), Type::Series)));
        out.extend(b.outputs.keys().map(|k| (k.clone(), Type::Any)));
    }
    out
}

/// A node by a path of ids (immutable).
trait NodeAt {
    fn node_mut_ref(&self, path: &[String]) -> Option<&Node>;
}

impl NodeAt for Model {
    fn node_mut_ref(&self, path: &[String]) -> Option<&Node> {
        let (last, up) = path.split_last()?;
        self.scope(up)?.iter().find(|n| &n.id == last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_graph::Input;

    fn model() -> Model {
        let mut m = Model::new("t");
        m.inputs.insert("t0".into(), Input::Map { path: "a.tif".into(), band: None, nodata: None, note: None });
        m.inputs.insert("t1".into(), Input::Map { path: "b.tif".into(), band: None, nodata: None, note: None });
        m
    }

    #[test]
    fn nodes_are_added_connected_and_checked() {
        let r = Registry::standard();
        let mut m = model();
        let id = add_node(&mut m, &r, &[], "markov.fit", [10.0, 20.0]).unwrap();
        assert_eq!(id, "fit");
        assert_eq!(add_node(&mut m, &r, &[], "markov.fit", [0.0, 0.0]).unwrap(), "fit_2");
        connect(&mut m, &r, &[], "$t0", "fit", "t0").unwrap();
        connect(&mut m, &r, &[], "$t1", "fit", "t1").unwrap();
        set_literal(&mut m, &r, &[], "fit", "steps", "14").unwrap();
        assert_eq!(m.nodes[0].inputs["steps"], Source::Literal(14.into()));
        // a map into a Markov chain's port: refused, the model unchanged
        let calc = add_node(&mut m, &r, &[], "map.calc", [0.0, 0.0]).unwrap();
        let before = m.clone();
        let e = add_and_fail(&mut m, &r, "$t0", "fit_2", "steps");
        assert!(e.contains("takes a"), "{e}");
        assert_eq!(m, before);
        // a port the operator does not have; a reference to nothing
        assert!(connect(&mut m, &r, &[], "$t0", "fit", "nope").is_err());
        assert!(connect(&mut m, &r, &[], "zz.out", "fit", "t0").is_err());
        // map algebra takes inputs of any name
        connect(&mut m, &r, &[], "$t0", &calc, "land").unwrap();
        set_param(&mut m, &[], &calc, "expression", "land * 2").unwrap();
        assert_eq!(m.node(&calc).unwrap().params["expression"], "land * 2");
        // a cycle is refused
        connect(&mut m, &r, &[], "calc.map", "fit_2", "t0").unwrap();
        let e = connect(&mut m, &r, &[], "fit_2.markov", &calc, "x").err().unwrap();
        assert!(e.contains("cycle"), "{e}");
    }

    fn add_and_fail(m: &mut Model, r: &Registry, reference: &str, node: &str, port: &str) -> String {
        connect(m, r, &[], reference, node, port).err().unwrap()
    }

    #[test]
    fn renaming_and_removing_follow_the_references() {
        let r = Registry::standard();
        let mut m = model();
        add_node(&mut m, &r, &[], "markov.fit", [0.0, 0.0]).unwrap();
        add_node(&mut m, &r, &[], "map.calc", [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], "$t0", "calc", "a").unwrap();
        connect(&mut m, &r, &[], "calc.map", "fit", "t0").unwrap();
        m.outputs.insert("out".into(), terraflux_graph::Output { from: "calc.map".into(), path: None, dtype: None });
        rename_node(&mut m, &[], "calc", "algebra").unwrap();
        assert_eq!(m.node("fit").unwrap().inputs["t0"], Source::Ref("algebra.map".into()));
        assert_eq!(m.outputs["out"].from, "algebra.map");
        assert!(rename_node(&mut m, &[], "algebra", "fit").is_err());
        assert!(rename_node(&mut m, &[], "algebra", "1bad").is_err());
        remove_node(&mut m, &[], "algebra").unwrap();
        assert!(!m.node("fit").unwrap().inputs.contains_key("t0"));
        assert!(m.outputs.is_empty());
    }

    #[test]
    fn nodes_are_pasted_as_new_nodes() {
        let r = Registry::standard();
        let mut m = model();
        add_node(&mut m, &r, &[], "map.calc", [0.0, 0.0]).unwrap();
        add_node(&mut m, &r, &[], "markov.fit", [200.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], "$t0", "calc", "a").unwrap();
        connect(&mut m, &r, &[], "calc.map", "fit", "t0").unwrap();
        connect(&mut m, &r, &[], "$t1", "fit", "t1").unwrap();
        set_literal(&mut m, &r, &[], "fit", "steps", "14").unwrap();
        // a duplicate of `fit` alone: a new id, what it read kept
        let copies = vec![m.node("fit").unwrap().clone()];
        assert_eq!(paste_nodes(&mut m, &[], &copies, [40.0, 40.0], true).unwrap(), vec!["fit_2"]);
        let d = m.node("fit_2").unwrap();
        assert_eq!(d.inputs["t0"], Source::Ref("calc.map".into()));
        assert_eq!(d.inputs["steps"], Source::Literal(14.into()));
        assert_eq!(d.pos, Some([240.0, 40.0]));
        // both pasted through the clipboard's text: the copies connected to each other, nothing from outside them
        let text = clip_text(&[m.node("calc").unwrap().clone(), m.node("fit_2").unwrap().clone()]);
        assert!(clip_nodes("calc").is_none() && clip_nodes("{\"a\": 1}").is_none());
        let copies = clip_nodes(&text).unwrap();
        assert_eq!(paste_nodes(&mut m, &[], &copies, [0.0, 300.0], false).unwrap(), vec!["calc_2", "fit_3"]);
        let (c, f) = (m.node("calc_2").unwrap(), m.node("fit_3").unwrap());
        assert!(c.inputs.is_empty());
        assert_eq!(f.inputs["t0"], Source::Ref("calc_2.map".into()));
        assert!(!f.inputs.contains_key("t1"));
        assert_eq!(f.inputs["steps"], Source::Literal(14.into()));
        assert_eq!(m.nodes.len(), 5);
        assert!(errors(&m, &r).iter().all(|i| !i.at.starts_with("calc_2") && !i.message.contains("cycle")));
        // a container's body keeps its own connections
        let g = add_node(&mut m, &r, &[], GROUP, [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], "$t0", &g, "land").unwrap();
        let path = vec![g.clone()];
        let inner = add_node(&mut m, &r, &path, "map.calc", [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &path, "@land", &inner, "x").unwrap();
        set_body_map(&mut m, &r, &path, "outputs", "plus", "calc.map").unwrap();
        let copies = vec![m.node(&g).unwrap().clone()];
        let id = paste_nodes(&mut m, &[], &copies, [0.0, 0.0], false).unwrap().remove(0);
        let b = m.node(&id).unwrap().body.as_ref().unwrap();
        assert_eq!(b.nodes[0].inputs["x"], Source::Ref("@land".into()));
        assert_eq!(b.outputs["plus"], "calc.map");
        assert!(m.node(&id).unwrap().inputs.is_empty());
    }

    #[test]
    fn bodies_are_edited_in_their_scope() {
        let r = Registry::standard();
        let mut m = model();
        let g = add_node(&mut m, &r, &[], GROUP, [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], "$t0", &g, "land").unwrap();
        let path = vec![g.clone()];
        assert!(sources(&m, &r, &path).iter().any(|(s, _)| s == "@land"));
        let c = add_node(&mut m, &r, &path, "map.calc", [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &path, "@land", &c, "x").unwrap();
        set_param(&mut m, &path, &c, "expression", "x + 1").unwrap();
        set_body_map(&mut m, &r, &path, "outputs", "plus", "calc.map").unwrap();
        assert!(output_ports(m.node(&g).unwrap(), &r).iter().any(|(p, _)| p == "plus"));
        // outside, the group's output is a port
        let f = add_node(&mut m, &r, &[], "markov.fit", [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], &format!("{g}.plus"), &f, "t0").unwrap();
        assert!(set_body_map(&mut m, &r, &path, "outputs", "bad", "nothing.here").is_err());
    }
}
