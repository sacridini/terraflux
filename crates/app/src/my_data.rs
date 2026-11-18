//! "Use my data", the engine: a model of any template, its drawn data (the nodes that stand for the user's: `data`,
//! the `nlm.*`) replaced by the user's files, as one change of the model and no window (the window is another
//! stage).
//!
//! [`sources`] lists what a model draws that a file can replace; [`plan`] makes a copy of the model with the files
//! chosen: each a model's input (so [`check`] reads it as the data of any model is read: the same findings), a
//! `raster.match` to the reference grid where its grid differs, a `raster.reclassify` of the table of classes
//! (the user's class -> the template's), a `raster.legend` with the template's names; the drawn node taken away
//! and what read it reading the end of the chain; the `nlm.*` left drawn on the reference grid.
//!
//! Decisions (see the report of the stage in IDEAS.md):
//! - The grids are compared by the files' headers: a `raster.match` is put only where the grid or the CRS differs
//!   from the reference's (`near` for classes and masks, `bilinear` for values, `auto` for the rest).
//! - `data` goes to the LAST node of the chain (what the rest of the model reads), so a second run of Use my data
//!   finds the data again. Where the chain has no node (a reference read straight as an input) there is none to
//!   carry it, and the input's note says what it is.
//! - Classes the table does not map, and a class mapped to "no data", become no data (`* -> nan` ends the rules).
//! - An `nlm.*` that is left is put on the reference grid (`like`; rows, columns, resolution and crs taken away);
//!   `nlm.curds` has no `like` and stays on its own grid, named in the notes.
//! - A stack is one file's bands (`Choice::bands`); a stack of several files is not offered yet.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use terraflux_exec::lint::{Finding, Level};
use terraflux_graph::{Data, DataKind, Input, Model, Node, Ref, Severity, Source as Wire, Type};
use terraflux_ops::Registry;
use terraflux_raster::Grid;

use crate::editor;

/// The `op` of a [`Source`] that is an input of the model (`$name`) with no file.
pub const INPUT: &str = "input";

/// What a model draws or leaves empty that a file of the user's can take the place of.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    /// The node (or the input, for [`INPUT`]).
    pub node: String,
    pub op: String,
    /// What it stands for, when the node says it.
    pub data: Option<Data>,
    /// What it is in a phrase: the data's, else the node's note, else its operator.
    pub what: String,
    /// The output port the rest of the model reads (an input: empty).
    pub port: String,
    /// A map or a stack, from what reads it.
    pub needs: Type,
}

/// What the user chose for a source.
#[derive(Debug, Clone, Default)]
pub struct Choice {
    pub file: PathBuf,
    /// The band of a map, by name or 0-based index (none: the first).
    pub band: Option<String>,
    /// The bands of a stack of one file (none: all of them).
    pub bands: Option<Vec<String>>,
    /// The user's class -> the template's (none: no data), for the data of classes and masks.
    pub crosswalk: BTreeMap<i64, Option<i64>>,
    pub nodata: Option<f64>,
}

/// The model with the user's data, what was done, and what was not.
#[derive(Debug, Clone)]
pub struct Plan {
    pub model: Model,
    /// What was done, a line each.
    pub notes: Vec<String>,
    /// The choices that were not made, with the reason.
    pub refused: Vec<String>,
    /// The inputs made for the files.
    pub inputs: Vec<String>,
    /// The input whose grid is the reference.
    pub reference: String,
}

/// The references a node reads (its inputs, settings and body).
fn reads(n: &Node) -> Vec<String> {
    let mut t = Model::new("t");
    t.nodes.push(n.clone());
    let mut out = Vec::new();
    editor::for_each_ref(&mut t, &[], &mut |r| out.push(r.clone()));
    out
}

/// What the model's outputs and Plots read.
fn outside_reads(m: &Model) -> Vec<String> {
    let mut t = m.clone();
    t.nodes.clear();
    let mut out = Vec::new();
    editor::for_each_ref(&mut t, &[], &mut |r| out.push(r.clone()));
    out
}

/// Who reads a node: the nodes (with the input port), the output ports they read, and whether the outputs or the
/// Plots do.
struct Readers {
    nodes: Vec<(String, Option<String>)>,
    ports: BTreeSet<String>,
    outside: bool,
}

fn readers(m: &Model, id: &str) -> Readers {
    let port_of = |r: &str| match Ref::parse(r) {
        Ok(Ref::Port { node, port }) if node == id => Some(port),
        _ => None,
    };
    let mut out = Readers { nodes: Vec::new(), ports: BTreeSet::new(), outside: false };
    for n in m.nodes.iter().filter(|n| n.id != id) {
        for r in reads(n) {
            if let Some(p) = port_of(&r) {
                out.ports.insert(p);
                let at = n.inputs.iter().find(|(_, s)| matches!(s, Wire::Ref(x) if *x == r)).map(|(k, _)| k.clone());
                out.nodes.push((n.id.clone(), at));
            }
        }
    }
    for r in outside_reads(m) {
        if let Some(p) = port_of(&r) {
            out.ports.insert(p);
            out.outside = true;
        }
    }
    out
}

fn source_of(m: &Model, r: &Registry, n: &Node) -> Source {
    let rd = readers(m, &n.id);
    let sig = r.sig(&n.op);
    let port = rd
        .ports
        .iter()
        .next()
        .cloned()
        .or_else(|| sig.and_then(|s| s.outputs.first().map(|o| o.name.clone())))
        .unwrap_or_else(|| "map".to_string());
    let stack_out = sig.and_then(|s| s.outputs.iter().find(|o| o.name == port)).is_some_and(|o| o.ty == Type::Stack);
    let stack_in = rd.nodes.iter().any(|(c, p)| {
        let Some(p) = p else { return false };
        let sig = m.node(c).and_then(|c| r.sig(&c.op));
        sig.and_then(|s| s.inputs.iter().find(|i| &i.name == p)).is_some_and(|i| i.ty == Type::Stack)
    });
    let stack = stack_out || stack_in || n.data.as_ref().is_some_and(|d| d.kind == DataKind::Stack);
    let what = n
        .data
        .as_ref()
        .map(|d| d.what.clone())
        .or_else(|| n.note.clone())
        .unwrap_or_else(|| format!("a map drawn by {}", n.op));
    Source {
        node: n.id.clone(),
        op: n.op.clone(),
        data: n.data.clone(),
        what,
        port,
        needs: if stack { Type::Stack } else { Type::Map },
    }
}

/// The data of the model a file can replace, at its top level: the nodes that stand for the user's data (`data`) in
/// the model's order, then the `nlm.*` without the mark that something other than a node of `data` reads (the
/// ones only such nodes read are inside a drawn map, as the suitability of a simulated change), the `raster.read`
/// without a file, and the model's inputs without a path.
pub fn sources(m: &Model, r: &Registry) -> Vec<Source> {
    let mut out: Vec<Source> = m.nodes.iter().filter(|n| n.data.is_some()).map(|n| source_of(m, r, n)).collect();
    for n in m.nodes.iter().filter(|n| n.data.is_none()) {
        let loose = if n.op.starts_with("nlm.") {
            let rd = readers(m, &n.id);
            let inside = !rd.outside
                && !rd.nodes.is_empty()
                && rd.nodes.iter().all(|(c, _)| m.node(c).is_some_and(|c| c.data.is_some()));
            !inside
        } else {
            n.op == "raster.read" && n.params.get("path").and_then(|p| p.as_str()).is_none_or(|p| p.trim().is_empty())
        };
        if loose {
            out.push(source_of(m, r, n));
        }
    }
    for (name, input) in &m.inputs {
        let (empty, needs, note) = match input {
            Input::Map { path, note, .. } => (path.trim().is_empty(), Type::Map, note),
            Input::Stack { path, note, .. } => (path.trim().is_empty(), Type::Stack, note),
            Input::Value { .. } => continue,
        };
        if empty {
            let what = note.clone().unwrap_or_else(|| format!("the input ${name}"));
            out.push(Source {
                node: name.clone(),
                op: INPUT.to_string(),
                data: None,
                what,
                port: String::new(),
                needs,
            });
        }
    }
    out
}

/// A name not taken (`base`, else `base_2`...), taken from now on.
fn fresh(taken: &mut BTreeSet<String>, base: &str) -> String {
    let id = if !taken.contains(base) {
        base.to_string()
    } else {
        (2..).map(|k| format!("{base}_{k}")).find(|id| !taken.contains(id)).unwrap()
    };
    taken.insert(id.clone());
    id
}

/// The header of a file: its grid and band names.
fn header(path: &Path) -> Result<(Grid, Vec<String>), String> {
    let f = terraflux_ops::files::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let meta = f.meta();
    Ok((meta.grid.clone(), meta.band_names()))
}

fn same_grid(a: &Grid, b: &Grid) -> bool {
    let near = |x: &f64, y: &f64| (x - y).abs() <= 1e-9 * (1.0 + x.abs().max(y.abs()));
    let crs = match (a.geo.epsg(), b.geo.epsg()) {
        (Some(x), Some(y)) => x == y,
        _ => match (&a.geo.wkt, &b.geo.wkt) {
            (Some(x), Some(y)) => x == y,
            _ => true,
        },
    };
    a.width == b.width && a.height == b.height && a.transform.iter().zip(&b.transform).all(|(x, y)| near(x, y)) && crs
}

/// Whether a choice can be made for a source: the grid of its file, or why not.
fn vet(m: &Model, s: &Source, c: &Choice) -> Result<Grid, String> {
    if s.needs == Type::Map && c.bands.is_some() {
        return Err("a stack of bands for a map: choose one band".into());
    }
    if s.op != INPUT && readers(m, &s.node).ports.len() > 1 {
        return Err("the model reads more than one of its outputs: it cannot be replaced by a file".into());
    }
    let (grid, names) = header(&c.file)?;
    let known = |b: &String| names.contains(b) || b.parse::<usize>().is_ok_and(|i| i < names.len());
    if let Some(b) = c.band.iter().chain(c.bands.iter().flatten()).find(|b| !known(b)) {
        return Err(format!("{} has no band {b:?} (its bands are {names:?})", c.file.display()));
    }
    if !c.crosswalk.is_empty() {
        let d = s.data.as_ref().ok_or("it takes no table of classes (it is not data of classes)")?;
        if !matches!(d.kind, DataKind::Classes | DataKind::Mask) {
            return Err(format!("it is data of {}: a table of classes is for classes and masks", d.kind.name()));
        }
        let targets: Vec<i64> = c.crosswalk.values().flatten().copied().collect();
        if targets.is_empty() {
            return Err("the table of classes maps no class".into());
        }
        if d.classes.is_empty() {
            if d.kind == DataKind::Classes {
                return Err("it takes any classes: they are kept as they are, no table".into());
            }
            if let Some(t) = targets.iter().find(|t| !matches!(t, 0 | 1)) {
                return Err(format!("class {t} is not 0 or 1, what a mask takes"));
            }
        } else if let Some(t) = targets.iter().find(|t| !d.classes.contains_key(t)) {
            return Err(format!("class {t} is not among the template's classes ({})", d.classes_line()));
        }
    }
    Ok(grid)
}

/// A file's path as the model writes it: relative to its folder when under it, else absolute.
fn path_text(file: &Path, base: &Path) -> String {
    let p = if base.as_os_str().is_empty() { None } else { file.strip_prefix(base).ok() };
    match p {
        Some(p) => p.to_string_lossy().replace('\\', "/"),
        None => file.to_string_lossy().into_owned(),
    }
}

/// The rules of a table of classes: the user's classes of each class of the template's a line, those mapped to
/// nothing (and the ones not in the table) no data.
fn rules_of(crosswalk: &BTreeMap<i64, Option<i64>>) -> String {
    let mut by: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    let mut none = Vec::new();
    for (user, to) in crosswalk {
        match to {
            Some(t) => by.entry(*t).or_default().push(*user),
            None => none.push(*user),
        }
    }
    let list = |v: &[i64]| v.iter().map(i64::to_string).collect::<Vec<_>>().join(" ");
    let mut lines: Vec<String> = by.iter().map(|(t, v)| format!("{} -> {t}", list(v))).collect();
    if !none.is_empty() {
        lines.push(format!("{} -> nan", list(&none)));
    }
    lines.push("* -> nan".to_string());
    lines.join("\n")
}

/// Every reference `old` read by the model's nodes, outputs and Plots read `new` instead (and the reroute points of
/// the wires changed gone).
fn repoint(m: &mut Model, old: &str, new: &str) {
    for n in m.nodes.iter_mut() {
        let ports: Vec<String> = n
            .inputs
            .iter_mut()
            .filter_map(|(k, s)| match s {
                Wire::Ref(r) if r == old => {
                    *r = new.to_string();
                    Some(k.clone())
                }
                _ => None,
            })
            .collect();
        for p in ports {
            n.wires.remove(&p);
        }
    }
    editor::for_each_ref(m, &[], &mut |r| {
        if r == old {
            *r = new.to_string();
        }
    });
}

/// The node or input a reference reads.
fn target_of(r: &str) -> Option<String> {
    match Ref::parse(r).ok()? {
        Ref::Port { node, .. } => Some(node),
        Ref::Input(name) => Some(format!("${name}")),
        Ref::Carried(_) => None,
    }
}

/// The model with the files chosen in place of the data they stand for (a copy: the model given is not changed).
/// `choices` is by the source's node; `reference` is the source whose grid the others are put on (none: the first
/// map chosen); `base` is the model's folder. The choices that cannot be made are left out and told in
/// [`Plan::refused`]; none that can: an error.
pub fn plan(
    m: &Model,
    r: &Registry,
    choices: &BTreeMap<String, Choice>,
    reference: Option<&str>,
    base: &Path,
) -> Result<Plan, String> {
    let srcs = sources(m, r);
    let mut refused = Vec::new();
    for k in choices.keys().filter(|k| !srcs.iter().any(|s| &s.node == *k)) {
        refused.push(format!("{k}: the model has no such data to replace"));
    }
    let mut ok: Vec<(&Source, &Choice, Grid)> = Vec::new();
    for s in &srcs {
        let Some(c) = choices.get(&s.node) else { continue };
        match vet(m, s, c) {
            Ok(g) => ok.push((s, c, g)),
            Err(e) => refused.push(format!("{}: {e}", s.node)),
        }
    }
    if ok.is_empty() {
        return Err(if refused.is_empty() {
            "choose the file of at least one data".into()
        } else {
            refused.join("; ")
        });
    }
    let (ref_node, ref_grid, ref_stack) = match reference {
        Some(k) => ok.iter().find(|(s, ..)| s.node == k).map(|(s, _, g)| (s.node.clone(), g.clone(), s.needs)),
        None => ok
            .iter()
            .find(|(s, ..)| s.needs == Type::Map)
            .or(ok.first())
            .map(|(s, _, g)| (s.node.clone(), g.clone(), s.needs)),
    }
    .map(|(n, g, t)| (n, g, t == Type::Stack))
    .ok_or_else(|| format!("the reference {:?} is not among the data chosen", reference.unwrap_or("")))?;

    let mut out = m.clone();
    let mut notes = Vec::new();
    let mut taken_in: BTreeSet<String> = out.inputs.keys().cloned().collect();
    let mut taken: BTreeSet<String> = out.nodes.iter().map(|n| n.id.clone()).collect();
    let mut names: HashMap<String, String> = HashMap::new();
    for (s, ..) in &ok {
        let name = if s.op == INPUT { s.node.clone() } else { fresh(&mut taken_in, &s.node) };
        names.insert(s.node.clone(), name);
    }
    let ref_in = names[&ref_node].clone();
    let mut inputs = Vec::new();
    // what the drawn nodes read, to take away what only they read
    let mut candidates: Vec<String> = Vec::new();
    for (s, c, grid) in &ok {
        let name = names[&s.node].clone();
        let file = path_text(&c.file, base);
        let band = match (&c.band, &c.bands) {
            (Some(b), _) => format!(", band {b}"),
            (None, Some(b)) => format!(", bands {}", b.join(" ")),
            _ => String::new(),
        };
        let kind = s.data.as_ref().map(|d| d.kind);
        let note = format!("{}: your file {file}{band}", s.what);
        let input = if s.needs == Type::Stack {
            Input::Stack { path: file.clone(), bands: c.bands.clone(), nodata: c.nodata, note: Some(note) }
        } else {
            Input::Map { path: file.clone(), band: c.band.clone(), nodata: c.nodata, note: Some(note) }
        };
        out.inputs.insert(name.clone(), input);
        inputs.push(name.clone());
        notes.push(format!("{}: reads your file {file}{band} as the input ${name}", s.node));
        let why = if s.op == INPUT {
            format!("for the input ${}: your file {file}{band}", s.node)
        } else {
            format!("in place of the drawn {}: your file {file}{band}", s.node)
        };
        let mut chain: Vec<(Node, &str)> = Vec::new();
        let mut cur = format!("${name}");
        if s.node != ref_node {
            if same_grid(grid, &ref_grid) {
                notes.push(format!("{}: already on the grid of ${ref_in}: no raster.match", s.node));
            } else {
                let how = match kind {
                    Some(DataKind::Classes | DataKind::Mask) => "near",
                    Some(DataKind::Values) => "bilinear",
                    _ => "auto",
                };
                let id = fresh(&mut taken, &format!("{}_match", s.node));
                let n = Node::new(&id, "raster.match")
                    .input("raster", &cur)
                    .input("like", &format!("${ref_in}"))
                    .param("resampling", how)
                    .note(&format!(
                        "Your file put on the grid of ${ref_in} (its cells, extent and CRS), resampled by {how}."
                    ))
                    .why(&why);
                notes.push(format!(
                    "{}: put on the grid of ${ref_in} by raster.match ({how}): its grid differs",
                    s.node
                ));
                cur = format!("{id}.raster");
                chain.push((n, "raster"));
            }
        }
        if !c.crosswalk.is_empty() {
            let rules = rules_of(&c.crosswalk);
            let id = fresh(&mut taken, &format!("{}_classes", s.node));
            let n = Node::new(&id, "raster.reclassify")
                .input("map", &cur)
                .param("rules", rules.clone())
                .note("Your classes into the ones the model reads (the rules are your class -> the model's); a class not in them has no data.")
                .why(&why);
            notes.push(format!(
                "{}: your classes into the model's by raster.reclassify: {}",
                s.node,
                rules.replace('\n', "; ")
            ));
            cur = format!("{id}.map");
            chain.push((n, "map"));
            if let Some(d) = s.data.as_ref().filter(|d| !d.classes.is_empty()) {
                let id = fresh(&mut taken, &format!("{}_legend", s.node));
                let classes: Vec<serde_json::Value> =
                    d.classes.iter().map(|(k, v)| serde_json::json!([k, v])).collect();
                let n = Node::new(&id, "raster.legend")
                    .input("map", &cur)
                    .param("classes", classes)
                    .note("The names of the model's classes, for the maps to show.")
                    .why(&why);
                cur = format!("{id}.map");
                chain.push((n, "map"));
            }
        }
        let end = chain.last().map(|(n, p)| format!("{}.{p}", n.id)).unwrap_or_else(|| format!("${name}"));
        let mut chain: Vec<Node> = chain.into_iter().map(|(n, _)| n).collect();
        if let (Some(d), Some(last)) = (&s.data, chain.last_mut()) {
            last.data = Some(d.clone());
        }
        let old = if s.op == INPUT { format!("${}", s.node) } else { format!("{}.{}", s.node, s.port) };
        if old != end {
            repoint(&mut out, &old, &end);
        }
        if s.op == INPUT {
            out.nodes.extend(chain);
            continue;
        }
        // the drawn node goes, its chain in its place
        let at = out.nodes.iter().position(|n| n.id == s.node).expect("a source is a node of the model");
        let old_node = out.nodes.remove(at);
        candidates.extend(reads(&old_node).iter().filter_map(|r| target_of(r)));
        if let Some([x, y]) = old_node.pos {
            for (k, n) in chain.iter_mut().enumerate() {
                n.pos = Some([x, y + 140.0 * k as f32]);
            }
        }
        let ids: Vec<String> = chain.iter().map(|n| n.id.clone()).collect();
        for f in out.notes.iter_mut().filter(|f| f.nodes.contains(&s.node)) {
            f.nodes.retain(|k| k != &s.node);
            f.nodes.extend(ids.clone());
        }
        let prefix = format!("{}.", s.node);
        out.parameters.retain(|p| !p.key.starts_with(&prefix));
        for sc in out.scenarios.iter_mut() {
            sc.values.retain(|k, _| !k.starts_with(&prefix));
        }
        out.nodes.splice(at..at, chain);
        notes.push(format!("{}: the drawn node taken away; what read it reads {end}", s.node));
    }

    // what only the nodes taken away read goes too
    loop {
        let mut read: BTreeSet<String> = BTreeSet::new();
        for n in &out.nodes {
            read.extend(reads(n).iter().filter_map(|r| target_of(r)));
        }
        read.extend(outside_reads(&out).iter().filter_map(|r| target_of(r)));
        let Some(k) = candidates.iter().position(|c| {
            !read.contains(c)
                && match c.strip_prefix('$') {
                    Some(name) => out.inputs.contains_key(name) && !inputs.iter().any(|i| i == name),
                    None => out.nodes.iter().any(|n| &n.id == c),
                }
        }) else {
            break;
        };
        let c = candidates.remove(k);
        match c.strip_prefix('$') {
            Some(name) => {
                out.inputs.remove(name);
                notes.push(format!("${name}: taken away, nothing reads it any more"));
            }
            None => {
                let at = out.nodes.iter().position(|n| n.id == c).unwrap();
                let n = out.nodes.remove(at);
                candidates.extend(reads(&n).iter().filter_map(|r| target_of(r)));
                notes.push(format!("{c}: taken away, only the drawn data read it"));
            }
        }
    }

    // the drawn maps left go on the reference grid
    for n in out.nodes.iter_mut().filter(|n| n.op.starts_with("nlm.")) {
        let like = r.sig(&n.op).is_some_and(|s| s.inputs.iter().any(|i| i.name == "like"));
        if !like {
            notes.push(format!(
                "{}: drawn on its own grid ({} has no `like`): it does not follow your data",
                n.id, n.op
            ));
        } else if n.inputs.contains_key("like") {
            continue;
        } else if ref_stack {
            notes.push(format!("{}: drawn on its own grid (the reference is a stack, not a map)", n.id));
        } else {
            n.inputs.insert("like".into(), Wire::Ref(format!("${ref_in}")));
            for k in ["rows", "columns", "resolution", "crs"] {
                n.params.remove(k);
            }
            notes.push(format!("{}: still drawn, on the grid of ${ref_in}", n.id));
        }
    }
    Ok(Plan { model: out, notes, refused, inputs, reference: ref_in })
}

/// The problems of the planned model: those of the model's check, and of its data as the data of any model is
/// read ([`terraflux_exec::load_inputs`] and [`terraflux_exec::lint::lint`]), without those of grids of the inputs
/// that a `raster.match` puts right.
pub fn check(plan: &Plan, base: &Path) -> Vec<Finding> {
    let r = Registry::standard();
    let mut out: Vec<Finding> = terraflux_graph::check(&plan.model, &r)
        .into_iter()
        .map(|i| Finding {
            level: if i.severity == Severity::Error { Level::Error } else { Level::Warning },
            at: i.at,
            message: i.message,
            fix: None,
            action: None,
        })
        .collect();
    match terraflux_exec::load_inputs(&plan.model, base, &Default::default()) {
        Err(e) => out.push(Finding {
            level: Level::Error,
            at: String::new(),
            message: e.to_string(),
            fix: None,
            action: None,
        }),
        Ok(inputs) => {
            let matched: BTreeSet<&str> = plan
                .model
                .nodes
                .iter()
                .filter(|n| n.op == "raster.match")
                .filter_map(|n| match n.inputs.get("raster") {
                    Some(Wire::Ref(r)) => r.strip_prefix('$'),
                    _ => None,
                })
                .collect();
            out.extend(terraflux_exec::lint::lint(&plan.model, &inputs).into_iter().filter(|f| {
                let name = f.at.trim_start_matches('$').split('/').next().unwrap_or("");
                !(f.message.contains("reference grid")
                    && (matched.contains(name) || plan.inputs.iter().any(|i| i == name)))
            }));
        }
    }
    out
}

/// The classes of a file's band (the first when none is given): each class's code, its name from the file's
/// legend, and its cells. An error when the band is not a map of classes: fractional values, or more than 256.
pub fn classes_of(path: &Path, band: Option<&str>) -> Result<Vec<(i64, Option<String>, usize)>, String> {
    let r = terraflux_ops::files::read_raster(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let i =
        match band {
            None => 0,
            Some(b) => match r.bands.iter().position(|x| x.name.as_deref() == Some(b)) {
                Some(i) => i,
                None => b.parse::<usize>().ok().filter(|i| *i < r.bands.len()).ok_or_else(|| {
                    format!("{} has no band {b:?} (its bands are {:?})", path.display(), r.band_names())
                })?,
            },
        };
    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    for v in r.bands[i].masked(r.nodata) {
        if v.is_nan() {
            continue;
        }
        if v.fract() != 0.0 {
            return Err("not a map of classes: it holds fractional values".into());
        }
        *counts.entry(v as i64).or_default() += 1;
        if counts.len() > 256 {
            return Err("not a map of classes: more than 256 values".into());
        }
    }
    let names: BTreeMap<i64, String> = terraflux_ops::legend::of_grid(&r.grid)
        .unwrap_or_default()
        .into_iter()
        .filter(|c| !c.name.trim().is_empty())
        .map(|c| (c.value as i64, c.name))
        .collect();
    Ok(counts.into_iter().map(|(k, n)| (k, names.get(&k).cloned(), n)).collect())
}

/// A first table of classes: the user's class of the template's name (letters of any case) is that class; when the
/// user's file names none of its classes, the same code is the same class; else no class (to be chosen).
pub fn suggest(template: &BTreeMap<i64, String>, user: &[(i64, Option<String>, usize)]) -> BTreeMap<i64, Option<i64>> {
    let named = user.iter().any(|(_, n, _)| n.is_some());
    user.iter()
        .map(|(code, name, _)| {
            let to = match name {
                Some(n) => template.iter().find(|(_, t)| t.trim().eq_ignore_ascii_case(n.trim())).map(|(k, _)| *k),
                None if !named => template.contains_key(code).then_some(*code),
                None => None,
            };
            (*code, to)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::TEMPLATES;
    use terraflux_graph::DataKind;
    use terraflux_ops::legend::Class;
    use terraflux_raster::{DataType, Raster};

    fn test_file(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name)
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("terraflux-my-data-{name}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn template(name: &str) -> Model {
        TEMPLATES.iter().find(|t| t.name == name).unwrap().model()
    }

    /// A map of w x h cells of side `cell`: the value of each cell by its row and column.
    fn write(d: &Path, name: &str, size: (usize, usize), cell: f64, f: impl Fn(usize, usize) -> f64) -> PathBuf {
        let mut g = terraflux_raster::Grid::new(size.0, size.1);
        g.transform = [1000.0, cell, 0.0, 1000.0 + cell * size.1 as f64, 0.0, -cell];
        let data = (0..size.0 * size.1).map(|i| f(i / size.0, i % size.0)).collect();
        let p = d.join(name);
        Raster::single(g, data, DataType::F32, None).write(&p).unwrap();
        p
    }

    fn choice(file: &Path) -> Choice {
        Choice { file: file.to_path_buf(), ..Default::default() }
    }

    fn one(node: &str, c: Choice) -> BTreeMap<String, Choice> {
        BTreeMap::from([(node.to_string(), c)])
    }

    fn run(p: &Plan, base: &Path) -> terraflux_exec::Results {
        let inputs = terraflux_exec::load_inputs(&p.model, base, &Default::default()).unwrap();
        terraflux_exec::Engine::default().run(&p.model, &inputs, &mut terraflux_exec::Quiet).unwrap()
    }

    fn errors(p: &Plan, base: &Path) -> Vec<String> {
        check(p, base).into_iter().filter(|f| f.level == Level::Error).map(|f| f.to_string()).collect()
    }

    fn ids(s: &[Source]) -> Vec<&str> {
        s.iter().map(|s| s.node.as_str()).collect()
    }

    /// Sibuyan's land use of 1997 as the user's classes of the forest (1 2 -> 1, 3 4 -> 3, 5 no data).
    fn forest_choice() -> Choice {
        let mut c = choice(&test_file("sibuyan_landuse.tif"));
        c.band = Some("1997".into());
        c.crosswalk = BTreeMap::from([(1, Some(1)), (2, Some(1)), (3, Some(3)), (4, Some(3)), (5, None)]);
        c
    }

    #[test]
    fn the_sources_of_the_templates() {
        let r = Registry::standard();
        assert_eq!(ids(&sources(&template("MSPA of a forest"), &r)), ["landscape"]);
        assert_eq!(ids(&sources(&template("Circuit connectivity"), &r)), ["quality", "resistance"]);
        assert_eq!(ids(&sources(&template("Species distribution model"), &r)), ["temperature", "rainfall", "relief"]);
        // the suitability is inside the drawn maps: not offered
        let m = template("Land change analysis");
        assert!(m.node("aptitude").is_some_and(|n| n.data.is_none()));
        assert_eq!(ids(&sources(&m, &r)), ["land_2000", "land_2010", "land_2020"]);
        let m = template("Dinamica EGO");
        let s = sources(&m, &r);
        assert_eq!(ids(&s), ["land_2000", "land_2010", "drivers"]);
        assert_eq!((s[0].needs, s[2].needs, s[2].port.as_str()), (Type::Map, Type::Stack, "stack"));
        assert_eq!(s[0].data.as_ref().unwrap().kind, DataKind::Classes);
    }

    #[test]
    fn the_loose_nodes_and_inputs_are_offered_after_the_marked() {
        let r = Registry::standard();
        let mut m = Model::new("t");
        let empty = Input::Map { path: String::new(), band: None, nodata: None, note: Some("the x".into()) };
        m.inputs.insert("x".into(), empty);
        m.inputs.insert("y".into(), Input::Map { path: "y.tif".into(), band: None, nodata: None, note: None });
        m.nodes.push(Node::new("loose", "nlm.fbm").note("a loose one"));
        m.nodes.push(Node::new("read", "raster.read"));
        m.nodes.push(Node::new("done", "raster.read").param("path", "a.tif"));
        m.nodes.push(Node::new("inner", "nlm.fbm"));
        let marked = Node::new("marked", "raster.calc").input("a", "inner.map");
        m.nodes.push(marked.data(Data::new("a map", DataKind::Values)));
        let s = sources(&m, &r);
        assert_eq!(ids(&s), ["marked", "loose", "read", "x"]);
        assert_eq!(s[1].what, "a loose one");
        assert_eq!((s[3].op.as_str(), s[3].what.as_str()), (INPUT, "the x"));
    }

    #[test]
    fn mspa_of_a_forest_on_a_map_of_classes() {
        let r = Registry::standard();
        let m = template("MSPA of a forest");
        let d = dir("mspa");
        let p = plan(&m, &r, &one("landscape", forest_choice()), None, &d).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        let md = &p.model;
        assert!(md.node("landscape").is_none() && md.node("landscape_match").is_none());
        let Some(Input::Map { path, band, note, .. }) = md.inputs.get("landscape") else { panic!("{:?}", md.inputs) };
        assert!(path.ends_with("sibuyan_landuse.tif") && band.as_deref() == Some("1997"));
        assert!(note.as_ref().unwrap().contains("a land cover map with a forest class"));
        let rc = md.node("landscape_classes").unwrap();
        assert_eq!(rc.op, "raster.reclassify");
        assert_eq!(rc.params["rules"], "1 2 -> 1\n3 4 -> 3\n5 -> nan\n* -> nan");
        assert_eq!(rc.inputs["map"], Wire::Ref("$landscape".into()));
        assert!(rc.why.as_ref().unwrap().contains("in place of the drawn landscape"));
        // the template's names, on the last node, which carries the data
        let lg = md.node("landscape_legend").unwrap();
        assert_eq!(lg.op, "raster.legend");
        assert_eq!(lg.data, m.node("landscape").unwrap().data);
        assert_eq!(lg.params["classes"], serde_json::json!([[1, "farming"], [2, "water"], [3, "forest"]]));
        // what read the drawn map reads the end of the chain
        assert_eq!(md.node("forest").unwrap().inputs["map"], Wire::Ref("landscape_legend.map".into()));
        assert_eq!(p.reference, "landscape");
        assert!(p.notes.iter().any(|n| n.contains("raster.reclassify")), "{:?}", p.notes);
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
        // it runs, on the user's grid
        let out = run(&p, &d);
        let Some(terraflux_ops::Value::Map(mspa)) = out.get("mspa.map") else { panic!("no MSPA") };
        assert_eq!((mspa.grid.width, mspa.grid.height), (128, 108));
        assert!(mspa.data.iter().any(|v| !v.is_nan()));
        // a second Use my data finds the data again, and takes the first chain away
        assert_eq!(ids(&sources(&p.model, &r)), ["landscape_legend"]);
        let mut c = forest_choice();
        c.band = Some("2011".into());
        let p2 = plan(&p.model, &r, &one("landscape_legend", c), None, &d).unwrap();
        assert_eq!(p2.model.nodes.iter().filter(|n| n.op == "raster.legend").count(), 1);
        assert!(errors(&p2, &d).is_empty(), "{:?}", errors(&p2, &d));
        assert_eq!(p2.model.inputs.len(), 1, "{:?}", p2.model.inputs.keys().collect::<Vec<_>>());
    }

    #[test]
    fn a_file_on_another_grid_is_matched_and_the_drawn_maps_left_follow_the_reference() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        let r = Registry::standard();
        let m = template("Land change analysis");
        let d = dir("another-grid");
        // the second date on a grid twice as fine
        let src = Raster::read(test_file("sibuyan_landuse.tif")).unwrap();
        let mut fine = src.grid.clone();
        fine.width *= 2;
        fine.height *= 2;
        fine.transform[1] /= 2.0;
        fine.transform[5] /= 2.0;
        let (moved, _) =
            terraflux_raster::resample::resample(&src, &fine, terraflux_raster::resample::Method::Nearest).unwrap();
        let fine_file = d.join("landuse_fine.tif");
        moved.write(&fine_file).unwrap();
        let mut first = forest_choice();
        first.crosswalk = BTreeMap::from([(1, Some(1)), (2, Some(2)), (3, Some(2)), (4, Some(3)), (5, Some(3))]);
        let mut second = first.clone();
        second.file = fine_file;
        second.band = Some("2011".into());
        let choices = BTreeMap::from([("land_2000".to_string(), first), ("land_2010".to_string(), second)]);
        let p = plan(&m, &r, &choices, None, &d).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        let md = &p.model;
        // the first is the reference: no match; the second is matched by near (classes), then reclassified
        assert!(md.node("land_2000_match").is_none());
        let mt = md.node("land_2010_match").unwrap();
        assert_eq!((mt.op.as_str(), mt.params["resampling"].as_str()), ("raster.match", Some("near")));
        assert_eq!(mt.inputs["raster"], Wire::Ref("$land_2010".into()));
        assert_eq!(mt.inputs["like"], Wire::Ref("$land_2000".into()));
        assert_eq!(md.node("land_2010_classes").unwrap().inputs["map"], Wire::Ref("land_2010_match.raster".into()));
        // the drawn nodes left follow the reference grid
        let ap = md.node("aptitude").unwrap();
        assert_eq!(ap.inputs["like"], Wire::Ref("$land_2000".into()));
        assert!(!ap.params.contains_key("rows") && !ap.params.contains_key("columns"));
        // (the third date, still drawn, reads the second)
        assert!(md.node("land_2020").is_some());
        // the findings of the grids of the inputs are not told: the match puts them right
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
        let out = run(&p, &d);
        let Some(terraflux_ops::Value::Map(last)) = out.get("land_2020.map") else { panic!("no third date") };
        assert_eq!((last.grid.width, last.grid.height), (128, 108));
    }

    #[test]
    fn a_stack_replaces_the_drawn_drivers_and_what_only_they_read() {
        let r = Registry::standard();
        let m = template("Dinamica EGO");
        let d = dir("drivers");
        let choices = BTreeMap::from([
            ("land_2000".to_string(), forest_choice()),
            ("land_2010".to_string(), {
                let mut c = forest_choice();
                c.band = Some("2011".into());
                c
            }),
            ("drivers".to_string(), choice(&test_file("sibuyan_drivers.tif"))),
        ]);
        let p = plan(&m, &r, &choices, None, &d).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        assert!(matches!(p.model.inputs.get("drivers"), Some(Input::Stack { bands: None, .. })));
        // the suitability and the roughness fed the drawn maps only
        assert!(p.model.node("aptitude").is_none() && p.model.node("roughness").is_none());
        assert!(p.notes.iter().any(|n| n.starts_with("aptitude: taken away")), "{:?}", p.notes);
        assert!(p.model.nodes.iter().all(|n| n.op != "raster.match"));
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
    }

    #[test]
    fn values_are_matched_by_bilinear() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        let r = Registry::standard();
        let m = template("Species distribution model");
        let d = dir("bilinear");
        let a = write(&d, "temperature.tif", (50, 50), 100.0, |r, c| (r + c) as f64 / 100.0);
        let b = write(&d, "rainfall.tif", (100, 100), 50.0, |r, c| r as f64 / 100.0 + c as f64 / 200.0);
        let choices = BTreeMap::from([("temperature".to_string(), choice(&a)), ("rainfall".to_string(), choice(&b))]);
        let p = plan(&m, &r, &choices, None, &d).unwrap();
        assert_eq!(p.reference, "temperature");
        let mt = p.model.node("rainfall_match").unwrap();
        assert_eq!(mt.params["resampling"], "bilinear");
        assert!(p.model.node("temperature_match").is_none());
        // the relief left drawn on the temperature's grid; the stack of the predictors reads the three
        assert_eq!(p.model.node("relief").unwrap().inputs["like"], Wire::Ref("$temperature".into()));
        let preds = p.model.node("predictors").unwrap();
        assert_eq!(preds.inputs["rainfall"], Wire::Ref("rainfall_match.raster".into()));
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
        // naming the reference: the rainfall's grid
        let q = plan(&m, &r, &choices, Some("rainfall"), &d).unwrap();
        assert!(q.model.node("rainfall_match").is_none() && q.model.node("temperature_match").is_some());
        assert_eq!(q.model.node("relief").unwrap().inputs["like"], Wire::Ref("$rainfall".into()));
        assert!(plan(&m, &r, &choices, Some("relief"), &d).is_err());
    }

    #[test]
    fn circuit_connectivity_on_a_map_of_values_runs() {
        let r = Registry::standard();
        let m = template("Circuit connectivity");
        let d = dir("circuit");
        let f = write(&d, "resistance.tif", (60, 60), 30.0, |r, c| 1.0 + ((r / 6 + c / 6) % 10) as f64 * 5.0);
        let p = plan(&m, &r, &one("resistance", choice(&f)), None, &d).unwrap();
        assert!(p.model.nodes.iter().all(|n| n.op != "raster.reclassify" && n.op != "raster.match"));
        assert!(p.model.node("resistance").is_none());
        assert_eq!(p.model.node("omniscape").unwrap().inputs["resistance"], Wire::Ref("$resistance".into()));
        // the quality (drawn, it makes the cores) is on the file's grid
        assert_eq!(p.model.node("quality").unwrap().inputs["like"], Wire::Ref("$resistance".into()));
        // (no node to carry the data: the input's note says what it is)
        let Some(Input::Map { note, .. }) = p.model.inputs.get("resistance") else { panic!() };
        assert!(note.as_ref().unwrap().contains("a resistance"));
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
        let out = run(&p, &d);
        let Some(terraflux_ops::Value::Map(o)) = out.get("omniscape.normalized") else { panic!("no omniscape") };
        assert_eq!((o.grid.width, o.grid.height), (60, 60));
    }

    #[test]
    fn data_of_any_classes_keeps_the_users_classes() {
        let r = Registry::standard();
        let m = template("Landscape metrics");
        assert!(m.node("cover").unwrap().data.as_ref().unwrap().classes.is_empty());
        let d = dir("any-classes");
        let mut c = choice(&test_file("sibuyan_landuse.tif"));
        c.band = Some("2011".into());
        let p = plan(&m, &r, &one("cover", c.clone()), None, &d).unwrap();
        assert!(p.model.nodes.iter().all(|n| n.op != "raster.reclassify" && n.op != "raster.legend"));
        assert_eq!(p.model.node("classes").unwrap().inputs["map"], Wire::Ref("$cover".into()));
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
        let out = run(&p, &d);
        assert!(out.get("classes.table").is_some());
        // a table of classes for it is refused
        c.crosswalk = BTreeMap::from([(1, Some(1))]);
        let e = plan(&m, &r, &one("cover", c), None, &d).unwrap_err();
        assert!(e.contains("any classes"), "{e}");
    }

    #[test]
    fn what_cannot_be_done_is_refused() {
        let r = Registry::standard();
        let m = template("MSPA of a forest");
        let d = dir("refused");
        let file = test_file("sibuyan_landuse.tif");
        // no choice
        assert!(plan(&m, &r, &BTreeMap::new(), None, &d).unwrap_err().contains("at least one"));
        // a stack for a map
        let mut c = choice(&file);
        c.bands = Some(vec!["1997".into(), "2011".into()]);
        let e = plan(&m, &r, &one("landscape", c), None, &d).unwrap_err();
        assert!(e.contains("a stack of bands for a map"), "{e}");
        // a class that is not the template's
        let mut c = forest_choice();
        c.crosswalk.insert(2, Some(9));
        let e = plan(&m, &r, &one("landscape", c), None, &d).unwrap_err();
        assert!(e.contains("class 9 is not among the template's classes"), "{e}");
        // a band the file has not, a file that is not there
        let mut c = choice(&file);
        c.band = Some("1900".into());
        let e = plan(&m, &r, &one("landscape", c), None, &d).unwrap_err();
        assert!(e.contains("has no band"), "{e}");
        assert!(plan(&m, &r, &one("landscape", choice(&d.join("none.tif"))), None, &d).is_err());
        // a table that maps nothing
        let mut c = choice(&file);
        c.crosswalk = BTreeMap::from([(1, None)]);
        assert!(plan(&m, &r, &one("landscape", c), None, &d).is_err());
        // a choice for what the model has not is told; the one that can be made is made
        let choices =
            BTreeMap::from([("landscape".to_string(), forest_choice()), ("nothing".to_string(), choice(&file))]);
        let p = plan(&m, &r, &choices, None, &d).unwrap();
        assert_eq!(p.refused.len(), 1);
        assert!(p.refused[0].starts_with("nothing:"), "{:?}", p.refused);
    }

    #[test]
    fn the_classes_of_a_file_and_a_first_table() {
        let d = dir("classes");
        let names = [(1.0, "Forest"), (2.0, "Water"), (7.0, "Crops")];
        let classes: Vec<Class> =
            names.iter().map(|(v, n)| Class { value: *v, name: n.to_string(), colour: String::new() }).collect();
        let g = terraflux_ops::legend::with_legend(&terraflux_raster::Grid::new(4, 3), Some(&classes));
        let data = vec![1.0, 1.0, 1.0, 2.0, 2.0, 7.0, 7.0, 7.0, 7.0, f64::NAN, 7.0, 7.0];
        let p = d.join("named.tif");
        Raster::single(g, data, DataType::F32, None).write(&p).unwrap();
        let cls = classes_of(&p, None).unwrap();
        let want = vec![
            (1, Some("Forest".to_string()), 3),
            (2, Some("Water".to_string()), 2),
            (7, Some("Crops".to_string()), 6),
        ];
        assert_eq!(cls, want);
        let template =
            BTreeMap::from([(1, "farming".to_string()), (2, "water".to_string()), (3, "forest".to_string())]);
        // by name, the case of letters apart; a name the template lacks, no class (Crops)
        assert_eq!(suggest(&template, &cls), BTreeMap::from([(1, Some(3)), (2, Some(2)), (7, None)]));
        let cls = vec![(3, Some(" FOREST".to_string()), 1), (4, Some("farming".to_string()), 1)];
        assert_eq!(suggest(&template, &cls), BTreeMap::from([(3, Some(3)), (4, Some(1))]));
        // without names, the same codes are the same classes
        let cls = vec![(1, None, 5), (2, None, 5), (5, None, 5)];
        assert_eq!(suggest(&template, &cls), BTreeMap::from([(1, Some(1)), (2, Some(2)), (5, None)]));
        // a map of values is not a map of classes; nor is one of many values
        let f = write(&d, "values.tif", (4, 4), 1.0, |r, c| (r * 4 + c) as f64 / 3.0);
        assert!(classes_of(&f, None).unwrap_err().contains("fractional"));
        let f = write(&d, "many.tif", (20, 20), 1.0, |r, c| (r * 20 + c) as f64);
        assert_eq!(classes_of(&f, None).unwrap_err(), "not a map of classes: more than 256 values");
        // a band of a file with bands
        let c = classes_of(&test_file("sibuyan_landuse.tif"), Some("2011")).unwrap();
        assert_eq!(c.iter().map(|c| c.2).sum::<usize>(), 4160 + 1404 + 1090 + 288 + 284);
        assert!(classes_of(&test_file("sibuyan_landuse.tif"), Some("nope")).is_err());
    }
}
