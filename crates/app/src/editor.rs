//! Editing a model's graph, as the node editor does: nodes added, moved, renamed and removed, connections
//! made and undone, literal inputs and settings changed, in the top level or in a container's body (a scope:
//! the path of the containers' ids). A change that would break the model (a connection of the wrong type, a
//! cycle, a port the operator does not have) is refused with the checker's reason, and the model stays as it
//! was.

use std::collections::{HashMap, HashSet};

use terraflux_graph::{Body, GROUP, Issue, Model, Node, Ref, Severity, Source, Type, UserNode, Uses, check, valid_id};
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

/// A fresh id in a scope from an operator's whole name, the dot an underscore (an id has no dot: `node.port` is
/// how a node's output is read): `raster_calc`, `raster_calc_2`...; `stack_of`, `raster_read`, not `of` nor `read`.
fn fresh_id(nodes: &[Node], op: &str) -> String {
    let base: String = op.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
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

/// The input of a node a folder goes to, to run it once per file: its first that takes one map.
pub fn folder_port(n: &Node, r: &Registry) -> Option<String> {
    let sig = r.sig(&n.op)?;
    sig.inputs.iter().find(|p| sig.per_layer(&p.name)).map(|p| p.name.clone())
}

/// Run a node on every raster of a folder: the folder read where it takes its first map, so that it (and the nodes
/// after it that take its maps) runs once per file. The `raster.read` that feeds that input reads the folder in
/// place of its file; else a `raster.read` of the folder is added at the node's left. The id of the node reading
/// the folder.
pub fn run_on_folder(m: &mut Model, r: &Registry, path: &[String], node: &str, folder: &str) -> Result<String> {
    let n = scope_mut(m, path)?.iter().find(|n| n.id == node).cloned().ok_or_else(|| format!("no node {node:?}"))?;
    let port = folder_port(&n, r).ok_or_else(|| format!("{} takes no map: it cannot run on a folder", n.op))?;
    let reader = match n.inputs.get(&port) {
        Some(Source::Ref(reference)) => match Ref::parse(reference) {
            Ok(Ref::Port { node: from, .. }) => {
                scope_mut(m, path)?.iter().find(|x| x.id == from && x.op == "raster.read").map(|x| x.id.clone())
            }
            _ => None,
        },
        _ => None,
    };
    if let Some(id) = reader {
        set_text_param(m, path, &id, "path", folder)?;
        return Ok(id);
    }
    let [x, y] = n.pos.unwrap_or([0.0, 0.0]);
    let id = add_node(m, r, path, "raster.read", [x - 260.0, y])?;
    let read = node_mut(m, path, &id)?;
    read.params.insert("path".into(), folder.into());
    read.note = Some(format!("every raster of the folder, {node} run on each, file by file"));
    let reference = format!("{id}.raster");
    if let Err(e) = connect(m, r, path, &reference, node, &port) {
        scope_mut(m, path)?.retain(|x| x.id != id);
        return Err(e);
    }
    Ok(id)
}

/// Add a node of the user's library to a scope: a group holding a copy of that version's body; its id (from the
/// node's name).
pub fn add_user_node(m: &mut Model, path: &[String], u: &UserNode, pos: [f32; 2]) -> Result<String> {
    let nodes = scope_mut(m, path)?;
    let id = fresh_id(nodes, &u.name);
    let mut n = u.instance(&id);
    n.pos = Some(pos);
    nodes.push(n);
    Ok(id)
}

/// A map algebra node made a group of that one node, to be saved as a node of the user's: what it read, the
/// group's inputs (`@name` inside); each of its outputs, the group's of the same name. The group keeps its id and
/// place, so what read the node reads the group.
pub fn calc_to_group(m: &mut Model, r: &Registry, path: &[String], id: &str) -> Result<()> {
    let calc = scope_mut(m, path)?.iter().find(|n| n.id == id).cloned().ok_or_else(|| format!("no node {id:?}"))?;
    if calc.op != "raster.calc" {
        return Err(format!("{id:?} is not a map algebra node"));
    }
    if let Some(k) = calc.inputs.keys().find(|k| !valid_id(k)) {
        return Err(format!("the input {k:?} is not a name a group's input can have: rename it first"));
    }
    let outputs: std::collections::BTreeMap<String, String> = r
        .sig("raster.calc")
        .map(|s| s.outputs_of(&calc))
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.name.clone(), format!("calc.{}", p.name)))
        .collect();
    let mut inner = calc.unrecorded();
    inner.id = "calc".into();
    inner.pos = Some([0.0, 0.0]);
    inner.width = None;
    inner.inputs = calc.inputs.keys().map(|k| (k.clone(), Source::Ref(format!("@{k}")))).collect();
    let mut g = Node::new(id, GROUP).body(Body { nodes: vec![inner], outputs, ..Default::default() });
    g.inputs = calc.inputs.clone();
    g.pos = calc.pos;
    g.note = calc.note.clone();
    g.why = calc.why.clone();
    g.refs = calc.refs.clone();
    g.assumptions = calc.assumptions.clone();
    g.alternatives = calc.alternatives.clone();
    guarded(m, r, &[prefix(path) + id], |c| {
        let n = node_mut(c, path, id)?;
        *n = g;
        Ok(())
    })
}

/// Make a group a node of the user's library, named and typed by `uses` (its body as that version's), or a plain
/// group again (`None`), checked.
pub fn set_uses(m: &mut Model, r: &Registry, path: &[String], id: &str, uses: Option<Uses>) -> Result<()> {
    if node_mut(m, path, id)?.op != GROUP {
        return Err(format!("{id:?} is not a group"));
    }
    guarded(m, r, &[prefix(path) + id], |c| {
        node_mut(c, path, id)?.uses = uses;
        Ok(())
    })
}

/// Bring a node of the user's to another version of it (its body, what it takes and gives), the inputs it was
/// given kept where the version has them: the inputs taken away. Refused when the model would break (a node
/// reading an output the version does not give, an input given what the version does not take); an input the
/// version needs and is not given is left to give.
pub fn update_user_node(m: &mut Model, r: &Registry, path: &[String], id: &str, u: &UserNode) -> Result<Vec<String>> {
    let before: Vec<String> = errors(m, r).into_iter().map(|i| i.to_string()).collect();
    let mut copy = m.clone();
    let gone = terraflux_graph::user_node::update(node_mut(&mut copy, path, id)?, u);
    let at = prefix(path) + id;
    if let Some(i) = errors(&copy, r).into_iter().find(|i| i.at != at && !before.contains(&i.to_string())) {
        return Err(format!("{}: {}", u.label(), i));
    }
    *m = copy;
    Ok(gone)
}

pub fn move_node(m: &mut Model, path: &[String], id: &str, pos: [f32; 2]) -> Result<()> {
    node_mut(m, path, id)?.pos = Some(pos);
    Ok(())
}

/// Give a node's box a width in the node editor (none: as wide as its texts need).
pub fn resize_node(m: &mut Model, path: &[String], id: &str, width: Option<f32>) -> Result<()> {
    node_mut(m, path, id)?.width = width;
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
        n.params.values_mut().filter_map(terraflux_graph::setting_ref_mut).for_each(&mut *f);
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
    n.params
        .retain(|_, v| terraflux_graph::setting_ref_mut(v).is_none_or(|r| relinked(r, new, shadow, outside, inner)));
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
        // (a setting that read it goes back to its default)
        n.params.retain(|_, v| !terraflux_graph::setting_ref(v).is_some_and(|r| r.starts_with(&from)));
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

/// Set why a node was chosen (empty text: none).
pub fn set_why(m: &mut Model, path: &[String], node: &str, text: &str) -> Result<()> {
    let n = node_mut(m, path, node)?;
    let t = text.trim();
    n.why = if t.is_empty() { None } else { Some(t.to_string()) };
    Ok(())
}

/// Set a list of a node's record (`refs`, `assumptions`, `alternatives`) from a text of an item a line.
pub fn set_list(m: &mut Model, path: &[String], node: &str, list: &str, text: &str) -> Result<()> {
    let n = node_mut(m, path, node)?;
    let items: Vec<String> = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
    match list {
        "refs" => n.refs = items,
        "assumptions" => n.assumptions = items,
        "alternatives" => n.alternatives = items,
        other => return Err(format!("a node has no list {other:?}")),
    }
    Ok(())
}

/// Turn nodes off (bypassed: their inputs passed on) or on again: on when all of them were off.
pub fn toggle_bypass(m: &mut Model, path: &[String], ids: &[String]) -> Result<()> {
    let all_off = ids.iter().all(|id| m.scope(path).is_some_and(|ns| ns.iter().any(|n| &n.id == id && n.bypass)));
    for id in ids {
        node_mut(m, path, id)?.bypass = !all_off;
    }
    Ok(())
}

/// Freeze a top-level node's result or thaw it (the flag; its outputs kept on disk by the session).
pub fn set_frozen(m: &mut Model, node: &str, on: bool) -> Result<()> {
    node_mut(m, &[], node)?.frozen = on;
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

/// Set a setting to a text as it is (a script: not read as a number or as JSON).
pub fn set_text_param(m: &mut Model, path: &[String], node: &str, name: &str, text: &str) -> Result<()> {
    let n = node_mut(m, path, node)?;
    n.params.insert(name.to_string(), serde_json::Value::String(text.to_string()));
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

/// Put nodes of a scope into a new group (`control.group`) of that scope, where the first of them was: what they
/// read stays as it is (a body reads the nodes around it), what read them from outside reads the group's outputs,
/// one per port read (`node`, or `node_port` when several of its ports are). Refused when it would make a cycle (a
/// node outside both reading one of them and read by another).
pub fn group_nodes(
    m: &mut Model,
    r: &Registry,
    path: &[String],
    ids: &[String],
    id: &str,
    note: Option<&str>,
) -> Result<()> {
    if ids.is_empty() {
        return Err("no nodes to group".into());
    }
    if !valid_id(id) {
        return Err(format!("{id:?} is not a valid id (a letter, then letters, digits, _ or -)"));
    }
    let scope = m.scope(path).ok_or_else(|| format!("no container {}", path.join("/")))?;
    if scope.iter().any(|n| n.id == id) {
        return Err(format!("a node is already named {id:?}"));
    }
    for i in ids {
        if !scope.iter().any(|n| &n.id == i) {
            return Err(format!("no node {i:?}"));
        }
    }
    let moved: HashSet<&str> = ids.iter().map(String::as_str).collect();
    // where the group goes: the corner of its nodes' places (the canvas's when they have none)
    let auto = crate::editor_view::auto_positions(scope);
    let place: HashMap<String, [f32; 2]> = scope
        .iter()
        .map(|n| (n.id.clone(), n.pos.or_else(|| auto.get(&n.id).copied()).unwrap_or([0.0, 0.0])))
        .collect();
    let corner = ids.iter().map(|i| place[i]).fold([f32::MAX, f32::MAX], |c, p| [c[0].min(p[0]), c[1].min(p[1])]);
    guarded(m, r, &[prefix(path) + id], |c| {
        let nodes = scope_mut(c, path)?;
        let first = nodes.iter().position(|n| moved.contains(n.id.as_str())).unwrap_or(nodes.len());
        let (mut body, kept): (Vec<Node>, Vec<Node>) = nodes.drain(..).partition(|n| moved.contains(n.id.as_str()));
        *nodes = kept;
        // the ports of the grouped nodes read from outside, and the outputs they become
        let mut read: Vec<(String, String)> = Vec::new();
        for_each_ref(c, path, &mut |rf| {
            if let Ok(Ref::Port { node, port }) = Ref::parse(rf)
                && moved.contains(node.as_str())
                && !read.contains(&(node.clone(), port.clone()))
            {
                read.push((node, port));
            }
        });
        let name = |node: &str, port: &str| {
            if read.iter().filter(|(n, _)| n == node).count() == 1 {
                node.to_string()
            } else {
                format!("{node}_{port}")
            }
        };
        let outputs: std::collections::BTreeMap<String, String> =
            read.iter().map(|(n, p)| (name(n, p), format!("{n}.{p}"))).collect();
        let renamed: HashMap<String, String> =
            read.iter().map(|(n, p)| (format!("{n}.{p}"), format!("{id}.{}", name(n, p)))).collect();
        for_each_ref(c, path, &mut |rf| {
            if let Some(new) = renamed.get(rf.as_str()) {
                *rf = new.clone();
            }
        });
        for n in body.iter_mut() {
            let p = place[&n.id];
            n.pos = Some([p[0] - corner[0], p[1] - corner[1]]);
        }
        let mut g = Node::new(id, GROUP).body(Body { nodes: body, outputs, ..Default::default() });
        g.note = note.map(String::from);
        g.pos = Some(corner);
        let nodes = scope_mut(c, path)?;
        nodes.insert(first.min(nodes.len()), g);
        Ok(())
    })
}

/// What a setting holds to read a source: its value, or the reference (`"$input"`, `"@carried"`, `{ from =
/// "node.port" }`).
pub fn setting_of(src: &Source) -> serde_json::Value {
    match src {
        Source::Literal(v) => v.clone(),
        Source::Ref(r) if r.starts_with(['$', '@']) => r.clone().into(),
        Source::Ref(r) => serde_json::json!({ "from": r }),
    }
}

/// Take the nodes of a group out into its scope, where it was: what read the group's outputs reads the nodes that
/// gave them, and its inputs (`@name` inside) are what they were given. Refused when a node of its body has the
/// name of one around it.
pub fn ungroup(m: &mut Model, r: &Registry, path: &[String], id: &str) -> Result<()> {
    let scope = m.scope(path).ok_or_else(|| format!("no container {}", path.join("/")))?;
    let g = scope.iter().find(|n| n.id == id).ok_or_else(|| format!("no node {id:?}"))?;
    if g.op != GROUP {
        return Err(format!("{id:?} is not a group"));
    }
    let body = g.body.clone().unwrap_or_default();
    if let Some(n) = body.nodes.iter().find(|n| scope.iter().any(|o| o.id == n.id)) {
        return Err(format!("a node outside the group is named {:?} too: rename one first", n.id));
    }
    let given = g.inputs.clone();
    let corner = g.pos.unwrap_or([0.0, 0.0]);
    let moved: Vec<String> = body.nodes.iter().map(|n| prefix(path) + &n.id).collect();
    /// The group's inputs given in place of `@name`, in the body's own containers too unless one gives its own.
    fn give(n: &mut Node, given: &std::collections::BTreeMap<String, Source>) {
        for s in n.inputs.values_mut() {
            if let Source::Ref(rf) = s
                && let Some(v) = rf.strip_prefix('@').and_then(|name| given.get(name))
            {
                *s = v.clone();
            }
        }
        for v in n.params.values_mut() {
            if let Some(src) =
                terraflux_graph::setting_ref(v).and_then(|r| r.strip_prefix('@')).and_then(|name| given.get(name))
            {
                *v = setting_of(src);
            }
        }
        if let Some(b) = &mut n.body {
            let own: HashSet<&String> =
                if n.op == GROUP { n.inputs.keys().collect() } else { b.carry.keys().collect() };
            let inner: std::collections::BTreeMap<String, Source> = given
                .iter()
                .filter(|(k, _)| !own.contains(k) && (n.op == GROUP || k.as_str() != "step"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            for x in b.nodes.iter_mut() {
                give(x, &inner);
            }
        }
    }
    guarded(m, r, &moved, |c| {
        let nodes = scope_mut(c, path)?;
        let at = nodes.iter().position(|n| n.id == id).ok_or_else(|| format!("no node {id:?}"))?;
        nodes.remove(at);
        let mut out = body.nodes.clone();
        for n in out.iter_mut() {
            give(n, &given);
            let p = n.pos.unwrap_or([0.0, 0.0]);
            n.pos = Some([p[0] + corner[0], p[1] + corner[1]]);
        }
        nodes.splice(at..at, out);
        let from = format!("{id}.");
        for_each_ref(c, path, &mut |rf| {
            if let Some(inner) = rf.strip_prefix(&from).and_then(|name| body.outputs.get(name)) {
                *rf = inner.clone();
            }
        });
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
        if let (true, Some(u)) = (c.op == GROUP, &c.uses) {
            // a node of the user's: the inputs it declares, of their types
            out.extend(u.inputs.iter().map(|(k, i)| (format!("@{k}"), i.ty)));
        } else if c.op == GROUP {
            out.extend(c.inputs.keys().map(|k| (format!("@{k}"), Type::Any)));
        } else if c.op == terraflux_graph::SWITCH {
            out.extend(c.inputs.keys().filter(|k| *k != "case").map(|k| (format!("@{k}"), Type::Any)));
        } else if c.op == terraflux_graph::EACH {
            out.extend([("@item".into(), Type::Any), ("@index".into(), Type::Integer), ("@name".into(), Type::Text)]);
            out.extend(c.inputs.keys().filter(|k| *k != "over").map(|k| (format!("@{k}"), Type::Any)));
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
    // a node of the user's: the inputs it declares (needed unless they have a value of their own)
    if let Some(u) = n.uses.as_ref().filter(|_| n.op == GROUP) {
        out.extend(u.inputs.iter().map(|(k, i)| (k.clone(), i.ty, i.required())));
        out.extend(n.inputs.keys().filter(|k| !u.inputs.contains_key(*k)).map(|k| (k.clone(), Type::Any, false)));
        return out;
    }
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
    if let Some(u) = n.uses.as_ref().filter(|_| n.op == GROUP) {
        return u.outputs.iter().map(|(k, o)| (k.clone(), o.ty)).collect();
    }
    if let Some(sig) = r.sig(&n.op) {
        out.extend(sig.outputs_of(n).into_iter().map(|p| (p.name, p.ty)));
    }
    if let Some(b) = &n.body {
        let collected = if n.op == terraflux_graph::EACH { Type::Any } else { Type::Series };
        out.extend(b.carry.keys().map(|k| (k.clone(), Type::Any)));
        out.extend(b.collect.keys().map(|k| (k.clone(), collected)));
        out.extend(b.outputs.keys().map(|k| (k.clone(), Type::Any)));
        // a switch: its branches' outputs (the first's names)
        if n.op == terraflux_graph::SWITCH
            && let Some(first) = b.nodes.first().and_then(|g| g.body.as_ref())
        {
            out.extend(first.outputs.keys().map(|k| (k.clone(), Type::Any)));
        }
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
    fn a_node_is_run_on_a_folder() {
        let r = Registry::standard();
        let mut m = Model::new("t");
        m.nodes.push(Node::new("mspa", "landscape.mspa"));
        m.nodes[0].pos = Some([400.0, 100.0]);
        // a folder read where it takes its map, at its left
        let id = run_on_folder(&mut m, &r, &[], "mspa", "tiles").unwrap();
        assert_eq!(id, "raster_read");
        let read = m.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(read.params["path"], "tiles");
        assert_eq!(read.pos, Some([140.0, 100.0]));
        assert_eq!(m.nodes[0].inputs["map"], Source::Ref("raster_read.raster".into()));
        // fed by a raster.read already: that one reads the folder
        assert_eq!(run_on_folder(&mut m, &r, &[], "mspa", "other").unwrap(), "raster_read");
        assert_eq!(m.nodes.len(), 2);
        assert_eq!(m.nodes[1].params["path"], "other");
        // an operator of rasters on disk that takes one map: also; one of no map: refused
        m.nodes.push(Node::new("metrics", "metrics.class"));
        assert_eq!(folder_port(&m.nodes[2], &r).as_deref(), Some("map"));
        m.nodes.push(Node::new("t", "table.filter"));
        assert!(run_on_folder(&mut m, &r, &[], "t", "tiles").unwrap_err().contains("takes no map"));
    }

    #[test]
    fn nodes_are_added_connected_and_checked() {
        let r = Registry::standard();
        let mut m = model();
        let id = add_node(&mut m, &r, &[], "markov.fit", [10.0, 20.0]).unwrap();
        assert_eq!(id, "markov_fit");
        assert_eq!(add_node(&mut m, &r, &[], "markov.fit", [0.0, 0.0]).unwrap(), "markov_fit_2");
        connect(&mut m, &r, &[], "$t0", "markov_fit", "t0").unwrap();
        connect(&mut m, &r, &[], "$t1", "markov_fit", "t1").unwrap();
        set_literal(&mut m, &r, &[], "markov_fit", "steps", "14").unwrap();
        assert_eq!(m.nodes[0].inputs["steps"], Source::Literal(14.into()));
        // a map into a Markov chain's port: refused, the model unchanged
        let calc = add_node(&mut m, &r, &[], "raster.calc", [0.0, 0.0]).unwrap();
        let before = m.clone();
        let e = add_and_fail(&mut m, &r, "$t0", "markov_fit_2", "steps");
        assert!(e.contains("takes a"), "{e}");
        assert_eq!(m, before);
        // a port the operator does not have; a reference to nothing
        assert!(connect(&mut m, &r, &[], "$t0", "markov_fit", "nope").is_err());
        assert!(connect(&mut m, &r, &[], "zz.out", "markov_fit", "t0").is_err());
        // map algebra takes inputs of any name
        connect(&mut m, &r, &[], "$t0", &calc, "land").unwrap();
        set_param(&mut m, &[], &calc, "expression", "land * 2").unwrap();
        assert_eq!(m.node(&calc).unwrap().params["expression"], "land * 2");
        // a cycle is refused
        connect(&mut m, &r, &[], "raster_calc.map", "markov_fit_2", "t0").unwrap();
        let e = connect(&mut m, &r, &[], "markov_fit_2.markov", &calc, "x").err().unwrap();
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
        add_node(&mut m, &r, &[], "raster.calc", [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], "$t0", "raster_calc", "a").unwrap();
        connect(&mut m, &r, &[], "raster_calc.map", "markov_fit", "t0").unwrap();
        m.outputs.insert(
            "out".into(),
            terraflux_graph::Output { from: "raster_calc.map".into(), path: None, dtype: None, layer: None },
        );
        // a setting that reads a node's output follows it too
        set_param(&mut m, &[], "markov_fit", "steps", r#"{"from": "raster_calc.map"}"#).unwrap();
        rename_node(&mut m, &[], "raster_calc", "algebra").unwrap();
        assert_eq!(m.node("markov_fit").unwrap().inputs["t0"], Source::Ref("algebra.map".into()));
        assert_eq!(m.node("markov_fit").unwrap().params["steps"], serde_json::json!({ "from": "algebra.map" }));
        assert_eq!(m.outputs["out"].from, "algebra.map");
        assert!(rename_node(&mut m, &[], "algebra", "markov_fit").is_err());
        assert!(rename_node(&mut m, &[], "algebra", "1bad").is_err());
        remove_node(&mut m, &[], "algebra").unwrap();
        assert!(!m.node("markov_fit").unwrap().inputs.contains_key("t0"));
        assert!(!m.node("markov_fit").unwrap().params.contains_key("steps"), "back to its default");
        assert!(m.outputs.is_empty());
    }

    #[test]
    fn nodes_are_pasted_as_new_nodes() {
        let r = Registry::standard();
        let mut m = model();
        add_node(&mut m, &r, &[], "raster.calc", [0.0, 0.0]).unwrap();
        add_node(&mut m, &r, &[], "markov.fit", [200.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], "$t0", "raster_calc", "a").unwrap();
        connect(&mut m, &r, &[], "raster_calc.map", "markov_fit", "t0").unwrap();
        connect(&mut m, &r, &[], "$t1", "markov_fit", "t1").unwrap();
        set_literal(&mut m, &r, &[], "markov_fit", "steps", "14").unwrap();
        // a duplicate of `fit` alone: a new id, what it read kept
        let copies = vec![m.node("markov_fit").unwrap().clone()];
        assert_eq!(paste_nodes(&mut m, &[], &copies, [40.0, 40.0], true).unwrap(), vec!["markov_fit_2"]);
        let d = m.node("markov_fit_2").unwrap();
        assert_eq!(d.inputs["t0"], Source::Ref("raster_calc.map".into()));
        assert_eq!(d.inputs["steps"], Source::Literal(14.into()));
        assert_eq!(d.pos, Some([240.0, 40.0]));
        // both pasted through the clipboard's text: the copies connected to each other, nothing from outside them
        let text = clip_text(&[m.node("raster_calc").unwrap().clone(), m.node("markov_fit_2").unwrap().clone()]);
        assert!(clip_nodes("raster_calc").is_none() && clip_nodes("{\"a\": 1}").is_none());
        let copies = clip_nodes(&text).unwrap();
        assert_eq!(
            paste_nodes(&mut m, &[], &copies, [0.0, 300.0], false).unwrap(),
            vec!["raster_calc_2", "markov_fit_3"]
        );
        let (c, f) = (m.node("raster_calc_2").unwrap(), m.node("markov_fit_3").unwrap());
        assert!(c.inputs.is_empty());
        assert_eq!(f.inputs["t0"], Source::Ref("raster_calc_2.map".into()));
        assert!(!f.inputs.contains_key("t1"));
        assert_eq!(f.inputs["steps"], Source::Literal(14.into()));
        assert_eq!(m.nodes.len(), 5);
        assert!(errors(&m, &r).iter().all(|i| !i.at.starts_with("raster_calc_2") && !i.message.contains("cycle")));
        // a container's body keeps its own connections
        let g = add_node(&mut m, &r, &[], GROUP, [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], "$t0", &g, "land").unwrap();
        let path = vec![g.clone()];
        let inner = add_node(&mut m, &r, &path, "raster.calc", [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &path, "@land", &inner, "x").unwrap();
        set_body_map(&mut m, &r, &path, "outputs", "plus", "raster_calc.map").unwrap();
        let copies = vec![m.node(&g).unwrap().clone()];
        let id = paste_nodes(&mut m, &[], &copies, [0.0, 0.0], false).unwrap().remove(0);
        let b = m.node(&id).unwrap().body.as_ref().unwrap();
        assert_eq!(b.nodes[0].inputs["x"], Source::Ref("@land".into()));
        assert_eq!(b.outputs["plus"], "raster_calc.map");
        assert!(m.node(&id).unwrap().inputs.is_empty());
    }

    /// A calculator made a group, the group a node of the user's library, a copy of it added, and both brought to
    /// a new version.
    #[test]
    fn a_calculator_becomes_a_node_of_the_library() {
        use terraflux_graph::{UserInput, UserOutput};
        let r = Registry::standard();
        let mut m = model();
        let calc = add_node(&mut m, &r, &[], "raster.calc", [50.0, 60.0]).unwrap();
        connect(&mut m, &r, &[], "$t0", &calc, "img").unwrap();
        set_literal(&mut m, &r, &[], &calc, "k", "2").unwrap();
        set_param(
            &mut m,
            &[],
            &calc,
            "expression",
            "out twice = img * k
out big = img > 5",
        )
        .unwrap();
        set_note(&mut m, &[], &calc, "doubled").unwrap();
        add_node(&mut m, &r, &[], "markov.fit", [300.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], &format!("{calc}.twice"), "markov_fit", "t0").unwrap();
        connect(&mut m, &r, &[], "$t1", "markov_fit", "t1").unwrap();
        set_literal(&mut m, &r, &[], "markov_fit", "steps", "14").unwrap();
        calc_to_group(&mut m, &r, &[], &calc).unwrap();
        let g = m.node(&calc).unwrap();
        assert_eq!((g.op.as_str(), g.pos, g.note.as_deref()), (GROUP, Some([50.0, 60.0]), Some("doubled")));
        assert_eq!(g.inputs["img"], Source::Ref("$t0".into()));
        let b = g.body.as_ref().unwrap();
        assert_eq!(b.nodes[0].inputs["k"], Source::Ref("@k".into()));
        assert_eq!(b.outputs.keys().collect::<Vec<_>>(), ["big", "twice"]);
        assert!(errors(&m, &r).is_empty(), "{:?}", errors(&m, &r));
        assert!(calc_to_group(&mut m, &r, &[], "markov_fit").is_err());
        // named and typed: a node of the library
        let uses = Uses {
            name: "double".into(),
            version: 1,
            description: None,
            inputs: [
                ("img".to_string(), UserInput::new(Type::Map)),
                ("k".to_string(), UserInput { default: Some(2.into()), ..UserInput::new(Type::Number) }),
            ]
            .into(),
            outputs: [
                ("twice".to_string(), UserOutput::new(Type::Map)),
                ("big".to_string(), UserOutput::new(Type::Map)),
            ]
            .into(),
        };
        set_uses(&mut m, &r, &[], &calc, Some(uses.clone())).unwrap();
        let g = m.node(&calc).unwrap().clone();
        assert_eq!(input_ports(&g, &r), [("img".into(), Type::Map, true), ("k".into(), Type::Number, false)]);
        assert_eq!(output_ports(&g, &r), [("big".into(), Type::Map), ("twice".into(), Type::Map)]);
        // a copy of it from the library
        let u = UserNode::of(&g).unwrap();
        let copy = add_user_node(&mut m, &[], &u, [0.0, 300.0]).unwrap();
        assert_eq!(copy, "double");
        assert_eq!(m.node(&copy).unwrap().inputs["k"], Source::Literal(2.into()));
        // version 2 gives no `big`: the copy follows; the first does not while what it gives is read... only twice
        let mut v2 = u.clone();
        v2.version = 2;
        v2.outputs.remove("big");
        v2.body.outputs.remove("big");
        assert!(update_user_node(&mut m, &r, &[], &copy, &v2).unwrap().is_empty());
        assert_eq!(m.node(&copy).unwrap().uses.as_ref().unwrap().version, 2);
        let mut v3 = v2.clone();
        v3.version = 3;
        v3.outputs = [("other".to_string(), UserOutput::new(Type::Map))].into();
        v3.body.outputs = [("other".to_string(), "calc.twice".to_string())].into();
        let e = update_user_node(&mut m, &r, &[], &calc, &v3).unwrap_err();
        assert!(e.contains("no output \"twice\""), "{e}");
        assert_eq!(m.node(&calc).unwrap().uses.as_ref().unwrap().version, 1);
        // a plain group again
        set_uses(&mut m, &r, &[], &calc, None).unwrap();
        assert!(m.node(&calc).unwrap().uses.is_none());
    }

    #[test]
    fn nodes_are_grouped_and_ungrouped() {
        let r = Registry::standard();
        let mut m = model();
        for (k, op) in ["raster.calc", "raster.calc", "markov.fit"].iter().enumerate() {
            add_node(&mut m, &r, &[], op, [k as f32 * 200.0, 0.0]).unwrap();
        }
        connect(&mut m, &r, &[], "$t0", "raster_calc", "a").unwrap();
        connect(&mut m, &r, &[], "raster_calc.map", "raster_calc_2", "b").unwrap();
        connect(&mut m, &r, &[], "raster_calc_2.map", "markov_fit", "t0").unwrap();
        connect(&mut m, &r, &[], "$t1", "markov_fit", "t1").unwrap();
        set_param(&mut m, &[], "raster_calc", "expression", "a + 1").unwrap();
        set_param(&mut m, &[], "raster_calc_2", "expression", "b * 2").unwrap();
        set_literal(&mut m, &r, &[], "markov_fit", "steps", "14").unwrap();
        m.outputs.insert(
            "calc".into(),
            terraflux_graph::Output { from: "raster_calc_2.map".into(), path: None, dtype: None, layer: None },
        );
        let before = m.clone();
        let ids = vec!["raster_calc".to_string(), "raster_calc_2".to_string()];
        group_nodes(&mut m, &r, &[], &ids, "prepare", Some("The maps prepared.")).unwrap();
        let g = m.node("prepare").unwrap();
        assert_eq!(g.op, GROUP);
        assert_eq!(g.note.as_deref(), Some("The maps prepared."));
        assert_eq!(g.pos, Some([0.0, 0.0]));
        let b = g.body.as_ref().unwrap();
        assert_eq!(b.nodes.len(), 2);
        assert_eq!(b.nodes[1].pos, Some([200.0, 0.0]));
        assert_eq!(b.outputs["raster_calc_2"], "raster_calc_2.map");
        assert_eq!(m.node("markov_fit").unwrap().inputs["t0"], Source::Ref("prepare.raster_calc_2".into()));
        assert_eq!(m.outputs["calc"].from, "prepare.raster_calc_2");
        assert!(errors(&before, &r).is_empty(), "{:?}", errors(&before, &r));
        assert!(errors(&m, &r).is_empty(), "{:?}", errors(&m, &r));
        // a name taken: refused, the model as it was
        let grouped = m.clone();
        assert!(group_nodes(&mut m, &r, &[], &["markov_fit".to_string()], "prepare", None).is_err());
        assert_eq!(m, grouped);
        // and back
        ungroup(&mut m, &r, &[], "prepare").unwrap();
        let ids: Vec<&str> = m.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["raster_calc", "raster_calc_2", "markov_fit"]);
        assert_eq!(m.nodes[..2], before.nodes[..2]);
        assert_eq!(m.node("markov_fit").unwrap().inputs["t0"], Source::Ref("raster_calc_2.map".into()));
        assert_eq!(m.outputs["calc"].from, "raster_calc_2.map");
        assert!(ungroup(&mut m, &r, &[], "markov_fit").is_err(), "not a group");
        // the ends of a chain grouped, its middle left out: a cycle
        let mut c = before.clone();
        let ends = vec!["raster_calc".to_string(), "markov_fit".to_string()];
        let e = group_nodes(&mut c, &r, &[], &ends, "ends", None).unwrap_err();
        assert!(e.contains("cycle"), "{e}");
        assert_eq!(c, before);
    }

    #[test]
    fn bodies_are_edited_in_their_scope() {
        let r = Registry::standard();
        let mut m = model();
        let g = add_node(&mut m, &r, &[], GROUP, [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], "$t0", &g, "land").unwrap();
        let path = vec![g.clone()];
        assert!(sources(&m, &r, &path).iter().any(|(s, _)| s == "@land"));
        let c = add_node(&mut m, &r, &path, "raster.calc", [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &path, "@land", &c, "x").unwrap();
        set_param(&mut m, &path, &c, "expression", "x + 1").unwrap();
        set_body_map(&mut m, &r, &path, "outputs", "plus", "raster_calc.map").unwrap();
        assert!(output_ports(m.node(&g).unwrap(), &r).iter().any(|(p, _)| p == "plus"));
        // outside, the group's output is a port
        let f = add_node(&mut m, &r, &[], "markov.fit", [0.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], &format!("{g}.plus"), &f, "t0").unwrap();
        assert!(set_body_map(&mut m, &r, &path, "outputs", "bad", "nothing.here").is_err());
    }
}
