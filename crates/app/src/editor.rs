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

/// The name of a node that reads a file, by the file's: `landuse_1997.tif` `landuse_1997`, a folder its name, a
/// pattern without its `*` (`landuse_*.tif` `landuse`); the letters without their accents, any other character a
/// `_`; a digit first after `map_` (`map_1997`). None when no letter nor digit is left.
pub fn id_of_file(path: &str) -> Option<String> {
    let p = std::path::Path::new(path);
    let name = if p.is_dir() { p.file_name() } else { p.file_stem() }?.to_string_lossy();
    let plain = |c: char| match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ñ' => 'n',
        'Ñ' => 'N',
        c if c.is_ascii_alphanumeric() => c,
        _ => '_',
    };
    let mut id = String::new();
    for c in name.chars().map(plain) {
        // (one `_` between words, none at the ends)
        if c != '_' || !id.is_empty() && !id.ends_with('_') {
            id.push(c);
        }
    }
    let id = id.trim_end_matches('_').to_string();
    if !id.chars().any(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let id = if valid_id(&id) { id } else { format!("map_{id}") };
    valid_id(&id).then_some(id)
}

/// Add a node of an operator to a scope (containers with an empty body); its id.
pub fn add_node(m: &mut Model, r: &Registry, path: &[String], op: &str, pos: [f32; 2]) -> Result<String> {
    add_node_named(m, r, path, op, None, pos)
}

/// [`add_node`], named `name` (made unique: `name_2`...) when given, else by its operator.
pub fn add_node_named(
    m: &mut Model,
    r: &Registry,
    path: &[String],
    op: &str,
    name: Option<&str>,
    pos: [f32; 2],
) -> Result<String> {
    let sig = r.sig(op).ok_or_else(|| format!("no operator {op:?}"))?;
    let nodes = scope_mut(m, path)?;
    let id = fresh_id(nodes, name.filter(|n| valid_id(n)).unwrap_or(op));
    let mut n = Node::new(&id, op);
    n.pos = Some(pos);
    if sig.container {
        n.body = Some(Body::default());
    }
    nodes.push(n);
    Ok(id)
}

/// Whether a box of the canvas is of what a scope reads besides its nodes (a reference: a model's input `$name`, a
/// container's value `@name`, a node outside it `node.port`), not a node: a node's id has no `.`, `$` nor `@`.
pub fn is_source(key: &str) -> bool {
    key.starts_with(['$', '@']) || key.contains('.')
}

/// Put the box of something a scope reads somewhere on the canvas ([`Model::places`]).
pub fn place_source(m: &mut Model, path: &[String], reference: &str, pos: [f32; 2]) {
    m.places.insert(terraflux_graph::place_key(path, reference), pos);
}

/// The references to `$name` taken away: the inputs that read it read nothing, the settings that read it their
/// default, a container's values and the model's outputs of it gone; the parameters and scenarios that change
/// it, and its boxes' places, too.
fn forget_input(m: &mut Model, name: &str) {
    fn in_nodes(nodes: &mut [Node], gone: &str) {
        for n in nodes {
            n.inputs.retain(|_, s| !matches!(s, Source::Ref(r) if r == gone));
            n.params.retain(|_, v| terraflux_graph::setting_ref(v) != Some(gone));
            if let Some(b) = &mut n.body {
                in_nodes(&mut b.nodes, gone);
                for map in [&mut b.carry, &mut b.collect, &mut b.outputs] {
                    map.retain(|_, r| r != gone);
                }
            }
        }
    }
    let gone = format!("${name}");
    in_nodes(&mut m.nodes, &gone);
    m.outputs.retain(|_, o| o.from != gone);
    m.parameters.retain(|p| p.key != gone);
    for s in &mut m.scenarios {
        s.values.remove(&gone);
    }
    let place = format!("|{gone}");
    m.places.retain(|k, _| !k.ends_with(&place));
}

/// Rename an input of the model: what reads it, the parameters and scenarios that change it and its boxes' places
/// follow.
pub fn rename_input(m: &mut Model, old: &str, new: &str) -> Result<()> {
    if old == new {
        return Ok(());
    }
    if !valid_id(new) {
        return Err(format!("{new:?} is not a valid name (letters, digits and _, not first a digit)"));
    }
    if m.inputs.contains_key(new) {
        return Err(format!("the model has an input ${new} already"));
    }
    let input = m.inputs.remove(old).ok_or_else(|| format!("the model has no input ${old}"))?;
    m.inputs.insert(new.to_string(), input);
    let (from, to) = (format!("${old}"), format!("${new}"));
    for_each_ref(m, &[], &mut |r| {
        if *r == from {
            *r = to.clone();
        }
    });
    for p in &mut m.parameters {
        if p.key == from {
            p.key = to.clone();
        }
    }
    for s in &mut m.scenarios {
        if let Some(v) = s.values.remove(&from) {
            s.values.insert(to.clone(), v);
        }
    }
    let place = format!("|{from}");
    let moved: Vec<(String, [f32; 2])> =
        m.places.iter().filter(|(k, _)| k.ends_with(&place)).map(|(k, p)| (k.clone(), *p)).collect();
    for (k, p) in moved {
        m.places.remove(&k);
        m.places.insert(format!("{}{to}", &k[..k.len() - from.len()]), p);
    }
    Ok(())
}

/// Take an input of the model away, and what reads it with it ([`forget_input`]).
pub fn remove_input(m: &mut Model, name: &str) -> Result<()> {
    m.inputs.remove(name).ok_or_else(|| format!("the model has no input ${name}"))?;
    forget_input(m, name);
    Ok(())
}

/// Make an input of the model of a raster file a node of the top level: a `raster.read` of its file (its band
/// chosen when it was a map), read by what read it. A stack of some bands of its file, or a value (what the
/// forms and scenarios change), stays an input. The node's id: the input's name when no node has it.
pub fn input_to_node(m: &mut Model, r: &Registry, name: &str) -> Result<String> {
    let input = m.inputs.get(name).cloned().ok_or_else(|| format!("the model has no input ${name}"))?;
    let (path, band, nodata, note) = match input {
        terraflux_graph::Input::Map { path, band, nodata, note } => {
            (path, Some(band.unwrap_or_else(|| "0".into())), nodata, note)
        }
        terraflux_graph::Input::Stack { path, bands: None, nodata, note } => (path, None, nodata, note),
        terraflux_graph::Input::Stack { .. } => {
            return Err(format!("${name} is some bands of its file: it stays an input"));
        }
        terraflux_graph::Input::Value { .. } => {
            return Err(format!("${name} is a value, which the forms and scenarios change: it stays an input"));
        }
    };
    let reference = format!("${name}");
    let id = if valid_id(name) && !m.nodes.iter().any(|n| n.id == name) {
        name.to_string()
    } else {
        fresh_id(&m.nodes, "raster.read")
    };
    let pos = m.places.get(&terraflux_graph::place_key(&[], &reference)).copied().unwrap_or_else(|| {
        let reads = |n: &&Node| terraflux_graph::references(n).contains(&reference);
        m.nodes.iter().find(reads).and_then(|n| n.pos).map_or([0.0, 0.0], |[x, y]| [x - 260.0, y])
    });
    guarded(m, r, std::slice::from_ref(&id), |c| {
        let mut n = Node::new(&id, "raster.read").param("path", path.clone());
        if let Some(b) = &band {
            n = n.param("band", b.clone());
        }
        if let Some(v) = nodata {
            n = n.param("nodata", v);
        }
        n.note = note.clone().or_else(|| Some(format!("the file of the model's input ${name}")));
        n.pos = Some(pos);
        let to = format!("{id}.raster");
        for_each_ref(c, &[], &mut |r| {
            if *r == reference {
                *r = to.clone();
            }
        });
        c.inputs.remove(name);
        forget_input(c, name);
        c.nodes.push(n);
        Ok(())
    })?;
    Ok(id)
}

/// Make a `raster.read` of a file, of the top level, an input of the model named `name` (what the command line,
/// Many runs and the scenarios change): a map of its band when it chose one or its file has one band (`bands`:
/// its file's, when known), else a stack of every band. A folder stays a node.
pub fn node_to_input(m: &mut Model, r: &Registry, id: &str, name: &str, bands: Option<usize>) -> Result<()> {
    let n = m.nodes.iter().find(|n| n.id == id).cloned().ok_or_else(|| format!("no node {id:?} at the top level"))?;
    if n.op != "raster.read" {
        return Err(format!("{id} is a {}: only a raster.read becomes an input of the model", n.op));
    }
    let path = n.params.get("path").and_then(|p| p.as_str()).filter(|p| !p.is_empty()).map(String::from);
    let path = path.ok_or_else(|| format!("{id} reads no file yet"))?;
    if path.contains(['*', '?']) || std::path::Path::new(&path).is_dir() {
        return Err(format!("{id} reads a folder: a model's input is one file"));
    }
    if !valid_id(name) {
        return Err(format!("{name:?} is not a valid name (letters, digits and _, not first a digit)"));
    }
    if m.inputs.contains_key(name) {
        return Err(format!("the model has an input ${name} already"));
    }
    let band = match n.params.get("band") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(b)) => Some(b.clone()),
        Some(v) => Some(v.to_string()),
    };
    let nodata = n.params.get("nodata").and_then(|v| v.as_f64());
    let input = if band.is_some() || bands.is_none_or(|b| b <= 1) {
        terraflux_graph::Input::Map { path, band, nodata, note: n.note.clone() }
    } else {
        terraflux_graph::Input::Stack { path, bands: None, nodata, note: n.note.clone() }
    };
    let (from, to) = (format!("{id}.raster"), format!("${name}"));
    guarded(m, r, &[], |c| {
        for_each_ref(c, &[], &mut |r| {
            if *r == from {
                *r = to.clone();
            }
        });
        c.nodes.retain(|x| x.id != id);
        c.inputs.insert(name.to_string(), input);
        if let Some(p) = n.pos {
            c.places.insert(terraflux_graph::place_key(&[], &to), p);
        }
        Ok(())
    })
}

/// A box of the canvas taken away: a node, or an input of the model (with what read it). A container's value or a
/// node outside the scope is not taken away here: what reads it is.
pub fn remove_box(m: &mut Model, path: &[String], key: &str) -> Result<()> {
    match key.strip_prefix('$') {
        Some(name) => remove_input(m, name),
        None if is_source(key) => {
            Err(format!("{}: not taken away here (disconnect what reads it)", crate::editor_view::source_help(key)))
        }
        None => remove_node(m, path, key),
    }
}

/// The name of the input of the model a node becomes ([`node_to_input`]): its id, unless an input has it.
pub fn input_name(m: &Model, id: &str) -> String {
    if !m.inputs.contains_key(id) {
        return id.to_string();
    }
    (2..).map(|k| format!("{id}_{k}")).find(|n| !m.inputs.contains_key(n)).unwrap()
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

/// Process every raster of a folder by an operator (Process a folder...): a `raster.read` of the folder, the
/// operator on it (run once per file) and what writes what it gives beside the folder: its first map a file per
/// raster (`<folder>_<op>/{name}_<op>.tif`), its first table one table of all of them (`<folder>_<op>.csv`), at `at`
/// and to its right. The id of the operator's node.
pub fn process_folder(
    m: &mut Model,
    r: &Registry,
    path: &[String],
    folder: &str,
    op: &str,
    at: [f32; 2],
) -> Result<String> {
    let sig = r.sig(op).ok_or_else(|| format!("no operator {op:?}"))?.clone();
    let before = scope_mut(m, path)?.len();
    let built = (|| {
        let id = add_node(m, r, path, op, [at[0] + 260.0, at[1]])?;
        run_on_folder(m, r, path, &id, folder)?;
        let short = op.rsplit('.').next().unwrap_or(op);
        let dir = std::path::Path::new(folder.trim_end_matches(['/', '\\']));
        let base = dir.file_name().map_or("folder".into(), |n| n.to_string_lossy().into_owned());
        let beside = |name: String| dir.with_file_name(name).to_string_lossy().into_owned();
        let mut y = at[1];
        if let Some(p) =
            sig.outputs.iter().find(|p| p.required && matches!(p.ty, Type::Map | Type::Stack | Type::Raster))
        {
            let w = add_node(m, r, path, "raster.write", [at[0] + 520.0, y])?;
            let file = std::path::Path::new(&beside(format!("{base}_{short}"))).join(format!("{{name}}_{short}.tif"));
            node_mut(m, path, &w)?.params.insert("path".into(), file.to_string_lossy().into_owned().into());
            connect(m, r, path, &format!("{id}.{}", p.name), &w, "value")?;
            y += 120.0;
        }
        if let Some(p) = sig.outputs.iter().find(|p| p.required && p.ty == Type::Table) {
            let w = add_node(m, r, path, "table.write", [at[0] + 520.0, y])?;
            node_mut(m, path, &w)?.params.insert("path".into(), beside(format!("{base}_{short}.csv")).into());
            connect(m, r, path, &format!("{id}.{}", p.name), &w, "table")?;
        }
        Ok(id)
    })();
    if built.is_err() {
        scope_mut(m, path)?.truncate(before);
    }
    built
}

/// A `batch.run` made a chain of nodes (Make it a chain of nodes): a `raster.read` of its folder (or pattern), its
/// operator with its parameters and a `raster.write` of each map it gives to its output folder
/// (`{name}_<op>[_<output>].tif`), its table gathered in one (`<op>.csv`); run file by file as it ran, done files
/// not run again. Refused when something reads what it gives (its table, its report), it reads a list of rasters
/// or the sub-folders too. The id of the operator's node.
pub fn batch_to_chain(m: &mut Model, r: &Registry, path: &[String], id: &str) -> Result<String> {
    let n = scope_mut(m, path)?.iter().find(|n| n.id == id).cloned().ok_or_else(|| format!("no node {id:?}"))?;
    if n.op != "batch.run" {
        return Err(format!("{id} is not a batch.run"));
    }
    let reads = |r: &str| matches!(Ref::parse(r), Ok(Ref::Port { node, .. }) if node == id);
    let mut readers: Vec<String> = scope_mut(m, path)?
        .iter()
        .filter(|x| terraflux_graph::references(x).iter().any(|r| reads(r)))
        .map(|x| x.id.clone())
        .collect();
    if path.is_empty() {
        readers.extend(m.outputs.iter().filter(|(_, o)| reads(&o.from)).map(|(k, _)| format!("the output {k}")));
    }
    if !readers.is_empty() {
        return Err(format!("{} read its table or report: not made a chain", readers.join(", ")));
    }
    let text = |k: &str| n.params.get(k).and_then(|v| v.as_str()).map(str::trim).filter(|v| !v.is_empty());
    let op = text("operator").ok_or("batch.run: give its operator first")?.to_string();
    let sig = r.sig(&op).ok_or_else(|| format!("no operator {op:?}"))?.clone();
    let folder =
        text("inputs").ok_or("batch.run: its inputs are a list of rasters (or none): give a folder or a pattern")?;
    if n.params.get("recursive").and_then(|v| v.as_bool()) == Some(true) {
        return Err("batch.run reads the sub-folders too: a chain reads one folder".into());
    }
    let out_dir = text("output_dir").unwrap_or(".");
    let at = n.pos.unwrap_or([0.0, 0.0]);
    let before = scope_mut(m, path)?.clone();
    let built = (|| {
        let id = add_node(m, r, path, &op, [at[0] + 260.0, at[1]])?;
        if let Some(serde_json::Value::Object(p)) = n.params.get("params") {
            node_mut(m, path, &id)?.params.extend(p.clone());
        }
        let port = match text("input") {
            Some(p) => p.to_string(),
            None => folder_port(&Node::new("x", &op), r).ok_or_else(|| format!("{op} takes no map"))?,
        };
        let read = add_node(m, r, path, "raster.read", at)?;
        let reader = node_mut(m, path, &read)?;
        reader.params.insert("path".into(), folder.into());
        if let Some(w) = n.params.get("workers").filter(|w| w.is_number()) {
            reader.params.insert("workers".into(), w.clone());
        }
        reader.note = Some(format!("every raster of the folder, {id} run on each, file by file"));
        connect(m, r, path, &format!("{read}.raster"), &id, &port)?;
        let short = op.rsplit('.').next().unwrap_or(&op);
        let maps: Vec<&String> = sig
            .outputs
            .iter()
            .filter(|p| p.required && matches!(p.ty, Type::Map | Type::Stack | Type::Raster))
            .map(|p| &p.name)
            .collect();
        let mut y = at[1];
        for p in &maps {
            let w = add_node(m, r, path, "raster.write", [at[0] + 520.0, y])?;
            let file =
                if maps.len() > 1 { format!("{{name}}_{short}_{p}.tif") } else { format!("{{name}}_{short}.tif") };
            let file = std::path::Path::new(out_dir).join(file);
            node_mut(m, path, &w)?.params.insert("path".into(), file.to_string_lossy().into_owned().into());
            connect(m, r, path, &format!("{id}.{p}"), &w, "value")?;
            y += 120.0;
        }
        if let Some(p) = sig.outputs.iter().find(|p| p.required && p.ty == Type::Table) {
            let w = add_node(m, r, path, "table.write", [at[0] + 520.0, y])?;
            let file = std::path::Path::new(out_dir).join(format!("{short}.csv"));
            node_mut(m, path, &w)?.params.insert("path".into(), file.to_string_lossy().into_owned().into());
            connect(m, r, path, &format!("{id}.{}", p.name), &w, "table")?;
        }
        scope_mut(m, path)?.retain(|x| x.id != n.id);
        Ok(id)
    })();
    if built.is_err() {
        *scope_mut(m, path)? = before;
    }
    built
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
        // (what the Plots show)
        for p in m.plots.iter_mut() {
            for l in p.layers.iter_mut() {
                f(&mut l.from);
                if let Some(m) = &mut l.minus {
                    f(m);
                }
            }
            if let Some(c) = &mut p.compare {
                c.each_ref(&mut *f);
            }
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
    prune_wires(n);
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
        // (the reroute points of its wires go with it)
        for p in c.wires.values_mut().flatten() {
            *p = [p[0] + offset[0], p[1] + offset[1]];
        }
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

/// The names a pattern gives the nodes `ids` of a scope, in the order of the canvas (top to bottom, then left to
/// right): `{n}` their number from 1, `{id}` the id now, `{op}` the operator's whole name as the ids made by default
/// have it (`raster_read`), `{file}` the name [`id_of_file`] gives the file of the node's `path` setting. Each one is
/// the new id or why it cannot be: not a valid id, no file, the name of a node outside the selection, or of another
/// one of the selection.
pub fn batch_names(nodes: &[Node], ids: &[String], pattern: &str) -> Vec<(String, Result<String>)> {
    let auto = crate::editor_view::auto_positions(nodes);
    let at = |n: &Node| n.pos.or_else(|| auto.get(&n.id).copied()).unwrap_or([0.0, 0.0]);
    let mut chosen: Vec<&Node> = nodes.iter().filter(|n| ids.contains(&n.id)).collect();
    chosen.sort_by(|a, b| {
        let (a, b) = (at(a), at(b));
        a[1].total_cmp(&b[1]).then(a[0].total_cmp(&b[0]))
    });
    let mut out: Vec<(String, Result<String>)> = chosen
        .iter()
        .enumerate()
        .map(|(k, n)| {
            let name = if pattern.contains("{file}") {
                match n.params.get("path").and_then(|p| p.as_str()).and_then(id_of_file) {
                    Some(f) => Ok(pattern.replace("{file}", &f)),
                    None => Err("it reads no file for {file}".to_string()),
                }
            } else {
                Ok(pattern.to_string())
            };
            let op: String = n.op.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
            let name = name.map(|t| t.replace("{n}", &(k + 1).to_string()).replace("{id}", &n.id).replace("{op}", &op));
            (n.id.clone(), name)
        })
        .collect();
    let outside: HashSet<&str> = nodes.iter().filter(|n| !ids.contains(&n.id)).map(|n| n.id.as_str()).collect();
    let mut count: HashMap<String, usize> = HashMap::new();
    for name in out.iter().filter_map(|(_, r)| r.as_ref().ok()) {
        *count.entry(name.clone()).or_default() += 1;
    }
    for (_, r) in out.iter_mut() {
        if let Ok(name) = r.as_ref() {
            *r = if !valid_id(name) {
                Err(format!("{name:?} is not a valid id (a letter, then letters, digits, _ or -)"))
            } else if outside.contains(name.as_str()) {
                Err(format!("a node is already named {name:?}"))
            } else if count[name] > 1 {
                Err(format!("{name:?} is the name of another node of the selection too"))
            } else {
                Ok(name.clone())
            };
        }
    }
    out
}

/// Rename several nodes of a scope at once, (old, new) each, references to them following: through temporary ids,
/// so a chain (`a` to `b`, `b` to `c`) or a swap never meets a name taken. None renamed when one fails. The
/// renames made one after the other, as steps (old, new) of what the layers shown must follow.
pub fn rename_many(m: &mut Model, path: &[String], pairs: &[(String, String)]) -> Result<Vec<(String, String)>> {
    let before = m.clone();
    let mut taken: HashSet<String> = scope_mut(m, path)?.iter().map(|n| n.id.clone()).collect();
    let mut steps = Vec::new();
    let mut then = Vec::new();
    let mut run = || -> Result<()> {
        for (k, (old, new)) in pairs.iter().filter(|(o, n)| o != n).enumerate() {
            let tmp = (k..).map(|j| format!("tf_tmp_{j}")).find(|t| !taken.contains(t)).unwrap();
            taken.insert(tmp.clone());
            rename_node(m, path, old, &tmp)?;
            steps.push((old.clone(), tmp.clone()));
            then.push((tmp, new.clone()));
        }
        for (tmp, new) in &then {
            rename_node(m, path, tmp, new)?;
            steps.push((tmp.clone(), new.clone()));
        }
        Ok(())
    };
    match run() {
        Ok(()) => Ok(steps),
        Err(e) => {
            *m = before;
            Err(e)
        }
    }
}

/// The id a read node takes when its file changes from `old` to `new`: only when its id is what [`add_node_named`]
/// gives a node that reads `old` (a name chosen by hand stays), the name of the new file made unique among the other
/// nodes. None: it stays.
pub fn id_after_file(nodes: &[Node], id: &str, old: &str, new: &str) -> Option<String> {
    if id_of_file(old).as_deref() != Some(id) {
        return None;
    }
    let base = id_of_file(new)?;
    let others: Vec<Node> = nodes.iter().filter(|n| n.id != id).cloned().collect();
    Some(fresh_id(&others, &base)).filter(|n| n != id)
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
        prune_wires(n);
        // (a setting that read it goes back to its default)
        n.params.retain(|_, v| !terraflux_graph::setting_ref(v).is_some_and(|r| r.starts_with(&from)));
    }
    if path.is_empty() {
        m.outputs.retain(|_, o| !o.from.starts_with(&from));
        for p in m.plots.iter_mut() {
            p.layers.retain(|l| !l.from.starts_with(&from) && !l.minus.as_ref().is_some_and(|m| m.starts_with(&from)));
            if p.compare.as_ref().is_some_and(|c| c.with.starts_with(&from)) {
                p.compare = None;
            }
            if let Some(c) = &mut p.compare {
                c.retain_refs(|r| !r.starts_with(&from));
            }
        }
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
        let n = node_mut(c, path, node)?;
        n.inputs.insert(port.to_string(), Source::Ref(reference.to_string()));
        // (a wire moved here comes without its points)
        n.wires.remove(port);
        Ok(())
    })
}

/// Connect several references to a node's input ports at once, (reference, port) each: one change (one step of
/// undo), checked; none made when one fails.
pub fn connect_many(
    m: &mut Model,
    r: &Registry,
    path: &[String],
    node: &str,
    links: &[(String, String)],
) -> Result<()> {
    for (reference, port) in links {
        Ref::parse(reference).map_err(|e| e.to_string())?;
        if !terraflux_graph::valid_port(port) {
            return Err(format!("{port:?} is not a valid port name"));
        }
    }
    let at = format!("{}{node}", prefix(path));
    let checked: Vec<String> =
        links.iter().map(|(_, port)| format!("{at}.{port}")).chain(std::iter::once(at.clone())).collect();
    guarded(m, r, &checked, |c| {
        let n = node_mut(c, path, node)?;
        for (reference, port) in links {
            n.inputs.insert(port.clone(), Source::Ref(reference.clone()));
            n.wires.remove(port);
        }
        Ok(())
    })
}

/// Put a node in the middle of a connection, as one change (one step of undo), checked: `from` (the reference the
/// connection carried) into the node's `input`, the node's `out` into `to`'s `to_port`, which read `from` before.
/// Nothing is made when it brings an error or a cycle.
#[allow(clippy::too_many_arguments)]
pub fn insert_between(
    m: &mut Model,
    r: &Registry,
    path: &[String],
    node: &str,
    from: &str,
    input: &str,
    out: &str,
    to: &str,
    to_port: &str,
) -> Result<()> {
    Ref::parse(from).map_err(|e| e.to_string())?;
    for port in [input, to_port] {
        if !terraflux_graph::valid_port(port) {
            return Err(format!("{port:?} is not a valid port name"));
        }
    }
    let at = |id: &str, port: &str| format!("{}{id}.{port}", prefix(path));
    let checked =
        [at(node, input), at(to, to_port), format!("{}{node}", prefix(path)), format!("{}{to}", prefix(path))];
    guarded(m, r, &checked, |c| {
        let n = node_mut(c, path, node)?;
        n.inputs.insert(input.to_string(), Source::Ref(from.to_string()));
        n.wires.remove(input);
        let t = node_mut(c, path, to)?;
        t.inputs.insert(to_port.to_string(), Source::Ref(format!("{node}.{out}")));
        t.wires.remove(to_port);
        Ok(())
    })
}

/// Take a node out of the connections it is in, as one change (one step of undo): its own connections removed,
/// and each `(node, port, source)` of `rewire` read `source` now (nothing: disconnected). The node stays.
pub fn lift_out(m: &mut Model, path: &[String], node: &str, rewire: &[(String, String, Option<String>)]) -> Result<()> {
    // (nothing to check: the sources were chosen to fit, and a node out of the chain makes no cycle; what the
    // node lacks now is the checks' to tell)
    let mut copy = m.clone();
    let own = node_mut(&mut copy, path, node)?;
    own.inputs.retain(|_, s| !matches!(s, Source::Ref(_)));
    own.wires.clear();
    for (n, port, source) in rewire {
        let n = node_mut(&mut copy, path, n)?;
        match source {
            Some(s) => n.inputs.insert(port.clone(), Source::Ref(s.clone())),
            None => n.inputs.remove(port),
        };
        n.wires.remove(port);
    }
    *m = copy;
    Ok(())
}

/// Whether an operator has an input a value of type `ty` may feed (a new input of one that takes any, too).
pub fn takes(r: &Registry, op: &str, ty: Type) -> bool {
    r.sig(op).is_some_and(|s| s.inputs.iter().any(|p| ty.fits(p.ty)) || s.extra.is_some_and(|e| ty.fits(e)))
}

/// Where outputs dragged to a node just added go, as (reference, port): a new input of each (named as what it
/// reads) in a node that takes any, else each its best free input that fits (the type itself, then those the node
/// needs, then the order of its inputs), the first reference first. Those that fit nothing are left out.
pub fn wire_links(n: &Node, r: &Registry, refs: &[(String, Option<Type>)]) -> Vec<(String, String)> {
    let extra = r.sig(&n.op).filter(|_| n.uses.is_none()).and_then(|s| s.extra);
    let mut ports = input_ports(n, r);
    let mut names: Vec<String> = ports.iter().map(|(k, _, _)| k.clone()).collect();
    let mut links = Vec::new();
    for (reference, ty) in refs {
        let fits = |t: Type| ty.is_none_or(|ty| ty.fits(t));
        if extra.is_some_and(fits) {
            let taken: Vec<&str> = names.iter().map(String::as_str).collect();
            let port = crate::editor_view::new_input(reference, &taken);
            names.push(port.clone());
            links.push((reference.clone(), port));
            continue;
        }
        let best = ports
            .iter()
            .enumerate()
            .filter(|(_, (_, t, _))| fits(*t))
            .min_by_key(|(_, (_, t, required))| (Some(*t) != *ty, !*required))
            .map(|(k, _)| k);
        if let Some(k) = best {
            links.push((reference.clone(), ports.remove(k).0));
        }
    }
    links
}

/// Exchange two connections, checked, as one change: `reference` (what the connection carried off the input `from`
/// read) goes into `node`'s `port`, and what that port read goes into `from`. Nothing is made when a type does
/// not fit or a cycle would be made, or when `port` reads no node.
pub fn swap_wires(
    m: &mut Model,
    r: &Registry,
    path: &[String],
    reference: &str,
    from: (&str, &str),
    (node, port): (&str, &str),
) -> Result<()> {
    Ref::parse(reference).map_err(|e| e.to_string())?;
    let Some(Source::Ref(other)) = node_mut(m, path, node)?.inputs.get(port).cloned() else {
        return Err(format!("{node}.{port} reads no node"));
    };
    let at = |id: &str, port: &str| format!("{}{id}.{port}", prefix(path));
    let checked =
        [at(from.0, from.1), at(node, port), format!("{}{}", prefix(path), from.0), format!("{}{node}", prefix(path))];
    guarded(m, r, &checked, |c| {
        let n = node_mut(c, path, node)?;
        n.inputs.insert(port.to_string(), Source::Ref(reference.to_string()));
        n.wires.remove(port);
        let f = node_mut(c, path, from.0)?;
        f.inputs.insert(from.1.to_string(), Source::Ref(other));
        f.wires.remove(from.1);
        Ok(())
    })
}

/// Let go the reroute points of the wires a node no longer reads (a port that reads no node has none).
fn prune_wires(n: &mut Node) {
    let Node { inputs, wires, .. } = n;
    wires.retain(|port, pts| !pts.is_empty() && matches!(inputs.get(port), Some(Source::Ref(_))));
}

/// The reroute points of a node's wires moved by `by` (a node taken to another scope's coordinates), those of the
/// wires that read a node `keep` says only: the others read from another scope, where the points mean nothing.
fn shift_wires(n: &mut Node, by: [f32; 2], keep: impl Fn(&str) -> bool) {
    let Node { inputs, wires, .. } = n;
    wires.retain(|port, _| match inputs.get(port) {
        Some(Source::Ref(rf)) => matches!(Ref::parse(rf), Ok(Ref::Port { node, .. }) if keep(&node)),
        _ => false,
    });
    for p in wires.values_mut().flatten() {
        *p = [p[0] + by[0], p[1] + by[1]];
    }
}

/// The points of a node's wire into `port` (a connection that is there), to change.
fn wire_points<'a>(m: &'a mut Model, path: &[String], node: &str, port: &str) -> Result<&'a mut Vec<[f32; 2]>> {
    let n = node_mut(m, path, node)?;
    if !matches!(n.inputs.get(port), Some(Source::Ref(_))) {
        return Err(format!("{node}.{port} reads no node"));
    }
    Ok(n.wires.entry(port.to_string()).or_default())
}

/// A reroute point added to the wire into a node's input, as the `index`th (from the output): drawing only.
pub fn add_wire_point(
    m: &mut Model,
    path: &[String],
    node: &str,
    port: &str,
    index: usize,
    at: [f32; 2],
) -> Result<()> {
    let pts = wire_points(m, path, node, port)?;
    pts.insert(index.min(pts.len()), at);
    Ok(())
}

/// The `index`th reroute point of the wire into a node's input moved.
pub fn move_wire_point(
    m: &mut Model,
    path: &[String],
    node: &str,
    port: &str,
    index: usize,
    to: [f32; 2],
) -> Result<()> {
    let p = wire_points(m, path, node, port)?
        .get_mut(index)
        .ok_or_else(|| format!("{node}.{port} has no point {index}"))?;
    *p = to;
    Ok(())
}

/// The `index`th reroute point of the wire into a node's input taken away (the wire's last: it is straight again).
pub fn remove_wire_point(m: &mut Model, path: &[String], node: &str, port: &str, index: usize) -> Result<()> {
    let pts = wire_points(m, path, node, port)?;
    if index >= pts.len() {
        return Err(format!("{node}.{port} has no point {index}"));
    }
    pts.remove(index);
    prune_wires(node_mut(m, path, node)?);
    Ok(())
}

/// Remove what a node's input port reads (and the reroute points of its wire).
pub fn disconnect(m: &mut Model, path: &[String], node: &str, port: &str) -> Result<()> {
    let n = node_mut(m, path, node)?;
    n.inputs.remove(port);
    n.wires.remove(port);
    Ok(())
}

/// Remove every connection into the nodes `ids` (what they read from other nodes, inputs of the model or a group's
/// carried values; their literal values stay). How many were taken away.
pub fn disconnect_all(m: &mut Model, path: &[String], ids: &[String]) -> Result<usize> {
    let mut gone = 0;
    for id in ids {
        let n = node_mut(m, path, id)?;
        let before = n.inputs.len();
        n.inputs.retain(|_, s| !matches!(s, Source::Ref(_)));
        n.wires.clear();
        gone += before - n.inputs.len();
    }
    Ok(gone)
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
        let n = node_mut(c, path, node)?;
        n.inputs.insert(port.to_string(), Source::Literal(value_of(text)));
        n.wires.remove(port);
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

/// Freeze a node's result or thaw it (the flag; its outputs kept on disk by the session): a top-level node's or
/// one of a group's body (`path`).
pub fn set_frozen(m: &mut Model, path: &[String], node: &str, on: bool) -> Result<()> {
    node_mut(m, path, node)?.frozen = on;
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

/// Set a setting of several nodes of a scope at once (empty text: remove it from each), checked: when the checker
/// refuses it on any of them, on none, and the model stays as it was.
pub fn set_param_many(
    m: &mut Model,
    r: &Registry,
    path: &[String],
    nodes: &[String],
    name: &str,
    text: &str,
) -> Result<()> {
    let pre = prefix(path);
    let at: Vec<String> = nodes.iter().flat_map(|id| [format!("{pre}{id}.{name}"), format!("{pre}{id}")]).collect();
    guarded(m, r, &at, |c| nodes.iter().try_for_each(|id| set_param(c, path, id, name, text)))
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
        // and the results of its last nodes (those none of the others reads: rasters grouped alone, the end of a
        // chain), so that the group can be connected on
        for (node, port) in last_outputs(r, &body) {
            if !read.iter().any(|(n, _)| *n == node) {
                read.push((node, port));
            }
        }
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
            // (the points of the wires among the grouped nodes follow; those of wires from outside are let go)
            shift_wires(n, [-corner[0], -corner[1]], |k| moved.contains(k));
        }
        let mut g = Node::new(id, GROUP).body(Body { nodes: body, outputs, ..Default::default() });
        g.note = note.map(String::from);
        g.pos = Some(corner);
        let nodes = scope_mut(c, path)?;
        nodes.insert(first.min(nodes.len()), g);
        Ok(())
    })
}

/// The first output of each of a body's last nodes, those whose outputs none of its other nodes reads, as (node,
/// port), in their order; the nodes without outputs (a file written, a check) left out.
fn last_outputs(r: &Registry, body: &[Node]) -> Vec<(String, String)> {
    let read: HashSet<String> = body
        .iter()
        .flat_map(terraflux_graph::references)
        .filter_map(|rf| match Ref::parse(&rf) {
            Ok(Ref::Port { node, .. }) => Some(node),
            _ => None,
        })
        .collect();
    body.iter()
        .filter(|n| !read.contains(&n.id))
        .filter_map(|n| {
            let port = r
                .sig(&n.op)
                .and_then(|s| s.outputs_of(n).first().map(|p| p.name.clone()))
                .or_else(|| n.body.as_ref().and_then(|b| b.outputs.keys().next().cloned()))?;
            Some((n.id.clone(), port))
        })
        .collect()
}

/// A group given the results of its last nodes as outputs (those it has not already), to connect outside: a group
/// made of nodes nothing read had none. How many it was given.
pub fn expose_last(m: &mut Model, r: &Registry, path: &[String], id: &str) -> Result<usize> {
    let mut added = 0;
    let at = prefix(path) + id;
    guarded(m, r, std::slice::from_ref(&at), |c| {
        let g = node_mut(c, path, id)?;
        let b = g.body.as_mut().ok_or_else(|| format!("{id} has no nodes"))?;
        for (node, port) in last_outputs(r, &b.nodes) {
            let reference = format!("{node}.{port}");
            if b.outputs.values().any(|v| *v == reference) {
                continue;
            }
            let name = [node.clone(), format!("{node}_{port}")]
                .into_iter()
                .chain((2..).map(|k| format!("{node}_{k}")))
                .find(|n| !b.outputs.contains_key(n))
                .unwrap();
            b.outputs.insert(name, reference);
            added += 1;
        }
        Ok(())
    })?;
    Ok(added)
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
    let own: HashSet<String> = body.nodes.iter().map(|n| n.id.clone()).collect();
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
            // (the points of the wires among the nodes of the body follow; those of wires from outside the body,
            // read through the group's inputs, are let go)
            shift_wires(n, corner, |k| own.contains(k));
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
    fn a_node_that_reads_a_file_is_named_by_it() {
        assert_eq!(id_of_file("C:/data/landuse_1997.tif").as_deref(), Some("landuse_1997"));
        assert_eq!(id_of_file("C:/data/landuse_*.tif").as_deref(), Some("landuse"));
        assert_eq!(id_of_file("C:/data/Uso do Solo (São Paulo).gpkg").as_deref(), Some("Uso_do_Solo_Sao_Paulo"));
        assert_eq!(id_of_file("C:/data/1997.tif").as_deref(), Some("map_1997"));
        assert_eq!(id_of_file("C:/data/land-use.v2.tif").as_deref(), Some("land_use_v2"));
        assert_eq!(id_of_file("C:/data/__.tif"), None);
        let r = Registry::standard();
        let mut m = Model::new("t");
        let a = add_node_named(&mut m, &r, &[], "raster.read", Some("landuse_1997"), [0.0, 0.0]).unwrap();
        let b = add_node_named(&mut m, &r, &[], "raster.read", Some("landuse_1997"), [0.0, 0.0]).unwrap();
        assert_eq!((a.as_str(), b.as_str()), ("landuse_1997", "landuse_1997_2"));
    }

    #[test]
    fn inputs_of_the_model_are_moved_renamed_made_nodes_and_taken_away() {
        let r = Registry::standard();
        let mut m = model();
        m.nodes.push(Node::new("mspa", "landscape.mspa").input("map", "$t0"));
        m.nodes[0].pos = Some([400.0, 50.0]);
        m.nodes.push(Node::new("calc", "raster.calc").input("a", "$t1").param("expression", "out x = a"));
        m.parameters.push(terraflux_graph::Parameter { key: "$t1".into(), ..Default::default() });
        // moved: its place kept
        assert!(is_source("$t0") && is_source("@step") && is_source("a.map") && !is_source("mspa"));
        place_source(&mut m, &[], "$t0", [10.0, 20.0]);
        assert_eq!(m.places["|$t0"], [10.0, 20.0]);
        // renamed: what reads it, its parameter and its place follow
        rename_input(&mut m, "t0", "land").unwrap();
        assert_eq!(m.nodes[0].inputs["map"], Source::Ref("$land".into()));
        assert_eq!(m.places["|$land"], [10.0, 20.0]);
        assert!(rename_input(&mut m, "land", "t1").unwrap_err().contains("already"));
        assert!(rename_input(&mut m, "land", "2x").is_err());
        // made a node: a raster.read of its file at its place, its band chosen (it was a map)
        let id = input_to_node(&mut m, &r, "land").unwrap();
        assert_eq!(id, "land");
        let n = m.nodes.iter().find(|n| n.id == "land").unwrap();
        assert_eq!(
            (n.op.as_str(), n.params["path"].as_str(), n.params["band"].as_str()),
            ("raster.read", Some("a.tif"), Some("0"))
        );
        assert_eq!(n.pos, Some([10.0, 20.0]));
        assert_eq!(m.nodes[0].inputs["map"], Source::Ref("land.raster".into()));
        assert!(!m.inputs.contains_key("land") && !m.places.contains_key("|$land"));
        // and back: an input of one band, read by what read the node
        node_to_input(&mut m, &r, "land", "land", Some(1)).unwrap();
        assert!(matches!(&m.inputs["land"], Input::Map { path, band: Some(b), .. } if path == "a.tif" && b == "0"));
        assert_eq!(m.nodes[0].inputs["map"], Source::Ref("$land".into()));
        assert!(!m.nodes.iter().any(|n| n.id == "land"));
        // a file of many bands, no band chosen: a stack
        m.nodes.push(Node::new("bands", "raster.read").param("path", "s.tif"));
        let name = input_name(&m, "bands");
        node_to_input(&mut m, &r, "bands", &name, Some(3)).unwrap();
        assert!(matches!(m.inputs["bands"], Input::Stack { .. }));
        // a folder stays a node
        m.nodes.push(Node::new("tiles", "raster.read").param("path", "tiles/*.tif"));
        assert!(node_to_input(&mut m, &r, "tiles", "tiles", None).unwrap_err().contains("folder"));
        // taken away: what read it reads nothing, its parameter gone
        remove_box(&mut m, &[], "$t1").unwrap();
        assert!(!m.nodes[1].inputs.contains_key("a") && m.parameters.is_empty());
        assert!(remove_box(&mut m, &[], "@step").is_err());
        remove_box(&mut m, &[], "tiles").unwrap();
        assert!(!m.nodes.iter().any(|n| n.id == "tiles"));
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
    fn a_folder_is_processed_by_an_operator() {
        let r = Registry::standard();
        let mut m = Model::new("t");
        let folder = std::path::Path::new("data").join("tiles");
        let folder = folder.to_string_lossy();
        // a map: written a file per raster, beside the folder
        let id = process_folder(&mut m, &r, &[], &folder, "landscape.mspa", [0.0, 0.0]).unwrap();
        let ops: Vec<&str> = m.nodes.iter().map(|n| n.op.as_str()).collect();
        assert_eq!(ops, ["landscape.mspa", "raster.read", "raster.write"]);
        assert_eq!(m.nodes[1].params["path"], *folder);
        assert_eq!(m.nodes[0].inputs["map"], Source::Ref("raster_read.raster".into()));
        let out = std::path::Path::new("data").join("tiles_mspa").join("{name}_mspa.tif");
        assert_eq!(m.nodes[2].params["path"], *out.to_string_lossy());
        assert_eq!(m.nodes[2].inputs["value"], Source::Ref(format!("{id}.map")));
        assert!(check(&m, &r).iter().all(|i| i.severity != Severity::Error), "{:?}", check(&m, &r));
        // a table: one of all of them
        let mut m = Model::new("t");
        process_folder(&mut m, &r, &[], &folder, "metrics.class", [0.0, 0.0]).unwrap();
        let w = m.nodes.iter().find(|n| n.op == "table.write").unwrap();
        assert_eq!(w.params["path"], *std::path::Path::new("data").join("tiles_class.csv").to_string_lossy());
        // an operator of no map: refused, nothing left
        assert!(process_folder(&mut m, &r, &[], &folder, "table.filter", [0.0, 0.0]).is_err());
        assert_eq!(m.nodes.len(), 3);
    }

    #[test]
    fn a_batch_run_is_made_a_chain_of_nodes() {
        let r = Registry::standard();
        let mut m = Model::new("t");
        let mut b = Node::new("batch", "batch.run")
            .param("operator", "landscape.mspa")
            .param("inputs", "tiles/*.tif")
            .param("output_dir", "out")
            .param("params", serde_json::json!({"edge_width": 2}))
            .param("workers", 4);
        b.pos = Some([100.0, 50.0]);
        m.nodes.push(b.clone());
        // its table read: refused, the model as it was
        m.nodes.push(Node::new("w", "table.write").input("table", "batch.table").param("path", "runs.csv"));
        assert!(batch_to_chain(&mut m, &r, &[], "batch").unwrap_err().contains("w read its table"));
        assert_eq!(m.nodes.len(), 2);
        m.nodes.pop();
        let id = batch_to_chain(&mut m, &r, &[], "batch").unwrap();
        let ops: Vec<&str> = m.nodes.iter().map(|n| n.op.as_str()).collect();
        assert_eq!(ops, ["landscape.mspa", "raster.read", "raster.write"]);
        let (mspa, read, write) = (&m.nodes[0], &m.nodes[1], &m.nodes[2]);
        assert_eq!(mspa.id, id);
        assert_eq!(mspa.params["edge_width"], 2);
        assert_eq!((read.params["path"].as_str(), read.params["workers"].as_i64()), (Some("tiles/*.tif"), Some(4)));
        assert_eq!(read.pos, Some([100.0, 50.0]));
        assert_eq!(mspa.inputs["map"], Source::Ref(format!("{}.raster", read.id)));
        let out = std::path::Path::new("out").join("{name}_mspa.tif");
        assert_eq!(write.params["path"], *out.to_string_lossy());
        // a list of rasters: refused
        let mut m = Model::new("t");
        m.nodes.push(b.param("inputs", serde_json::json!(["a.tif", "b.tif"])));
        assert!(batch_to_chain(&mut m, &r, &[], "batch").unwrap_err().contains("list of rasters"));
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

    /// The reroute points of the wires among nodes grouped go with them (into the group's coordinates and out again);
    /// those of wires from outside are let go; a disconnected input has none.
    #[test]
    fn reroute_points_follow_a_group_and_a_connection() {
        let r = Registry::standard();
        let mut m = model();
        add_node(&mut m, &r, &[], "raster.calc", [300.0, 100.0]).unwrap();
        add_node(&mut m, &r, &[], "raster.calc", [500.0, 100.0]).unwrap();
        connect(&mut m, &r, &[], "$t0", "raster_calc", "a").unwrap();
        connect(&mut m, &r, &[], "raster_calc.map", "raster_calc_2", "b").unwrap();
        add_wire_point(&mut m, &[], "raster_calc_2", "b", 0, [400.0, 60.0]).unwrap();
        add_wire_point(&mut m, &[], "raster_calc_2", "b", 1, [450.0, 70.0]).unwrap();
        add_wire_point(&mut m, &[], "raster_calc", "a", 0, [100.0, 100.0]).unwrap();
        assert!(add_wire_point(&mut m, &[], "raster_calc_2", "a", 0, [0.0, 0.0]).is_err(), "no connection there");
        move_wire_point(&mut m, &[], "raster_calc_2", "b", 1, [460.0, 80.0]).unwrap();
        remove_wire_point(&mut m, &[], "raster_calc_2", "b", 0).unwrap();
        assert_eq!(m.node("raster_calc_2").unwrap().wires["b"], vec![[460.0, 80.0]]);
        add_wire_point(&mut m, &[], "raster_calc_2", "b", 0, [400.0, 60.0]).unwrap();
        let ids = vec!["raster_calc".to_string(), "raster_calc_2".to_string()];
        group_nodes(&mut m, &r, &[], &ids, "prepare", None).unwrap();
        let body = m.node("prepare").unwrap().body.as_ref().unwrap();
        assert_eq!(body.nodes[1].wires["b"], vec![[100.0, -40.0], [160.0, -20.0]]);
        assert!(body.nodes[0].wires.is_empty(), "the wire from the model's input is out of the group's scope");
        ungroup(&mut m, &r, &[], "prepare").unwrap();
        assert_eq!(m.node("raster_calc_2").unwrap().wires["b"], vec![[400.0, 60.0], [460.0, 80.0]]);
        assert!(m.node("raster_calc").unwrap().wires.is_empty());
        // taken off its input, the wire has no points
        disconnect(&mut m, &[], "raster_calc_2", "b").unwrap();
        assert!(m.node("raster_calc_2").unwrap().wires.is_empty());
    }

    /// Rasters grouped that nothing read yet: the group gives each as an output, to connect on (a reclassify, a
    /// stack); a group made without them is given them from its menu.
    #[test]
    fn a_group_gives_the_results_of_its_last_nodes() {
        let r = Registry::standard();
        let mut m = model();
        for k in 0..3 {
            add_node(&mut m, &r, &[], "raster.read", [0.0, k as f32 * 150.0]).unwrap();
        }
        let ids: Vec<String> = m.nodes.iter().map(|n| n.id.clone()).collect();
        group_nodes(&mut m, &r, &[], &ids, "inputs", None).unwrap();
        let b = m.node("inputs").unwrap().body.as_ref().unwrap();
        let outs: Vec<(&str, &str)> = b.outputs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(
            outs,
            [
                ("raster_read", "raster_read.raster"),
                ("raster_read_2", "raster_read_2.raster"),
                ("raster_read_3", "raster_read_3.raster")
            ]
        );
        let reclass = add_node(&mut m, &r, &[], "raster.reclassify", [400.0, 0.0]).unwrap();
        connect(&mut m, &r, &[], "inputs.raster_read_2", &reclass, "map").unwrap();
        // a chain grouped: only its end
        let mut c = model();
        let a = add_node(&mut c, &r, &[], "raster.calc", [0.0, 0.0]).unwrap();
        let b = add_node(&mut c, &r, &[], "raster.calc", [200.0, 0.0]).unwrap();
        connect(&mut c, &r, &[], &format!("{a}.map"), &b, "x").unwrap();
        group_nodes(&mut c, &r, &[], &[a, b.clone()], "chain", None).unwrap();
        let outs: Vec<&String> = c.node("chain").unwrap().body.as_ref().unwrap().outputs.values().collect();
        assert_eq!(outs, [&format!("{b}.map")]);
        // a group without them: given them, once
        c.node_mut("chain").unwrap().body.as_mut().unwrap().outputs.clear();
        assert_eq!(expose_last(&mut c, &r, &[], "chain").unwrap(), 1);
        assert_eq!(expose_last(&mut c, &r, &[], "chain").unwrap(), 0);
        assert_eq!(c.node("chain").unwrap().body.as_ref().unwrap().outputs[&b], format!("{b}.map"));
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
