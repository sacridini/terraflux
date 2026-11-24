//! The trace of a cell: the chain of values that made what a node gives at a place (IDEAS, phase 7). From the node
//! shown, back through the graph: at every node, the value its output has at the cell and the values its inputs
//! have there, with how the operator made one of the others: map algebra's expression with the values in place of
//! its names, the window of cells a neighbourhood's operator reads, the layers a model's operator weighs (a class's
//! suitability), a loop's value step by step. The values are those of the last run (a top-level node's, a group's
//! body's: a group keeps them); inside a loop, those of a run that kept them ([`Kept`]). Built from the model and a
//! way to read the values: the Trace pane shows it, the assistant explains it (its words, [`Trace::text`]), the tool
//! `trace_cell` gives it to an assistant outside.

use std::collections::{BTreeMap, HashSet};

use serde_json::Value as Json;
use terraflux_graph::{Access, GROUP, Model, Node, Ref, Source};
use terraflux_ops::{Registry, Value};

use crate::document::words;
use crate::report::cell;

/// Nodes traced at most (a graph of thousands traced to its sources would say nothing).
const MOST: usize = 60;
/// The side of the window of cells shown of a neighbourhood's input, at most (an odd number).
const WINDOW: usize = 5;
/// The operators that work patch by patch: their cell explained by its whole patch.
const PATCHES: &[&str] =
    &["raster.label", "raster.sieve", "landscape.mspa", "landscape.mspa_tiled", "metrics.patch", "patches.observe"];
/// The cells of a patch counted at most.
const PATCH_MOST: usize = 4_000_000;

/// The cells of a patch kept to draw it on the map, at most (a larger one is drawn as the rows and columns it spans).
pub const PATCH_DRAWN: usize = 200_000;
/// The operators that allocate a step's demand: their cell explained by the demand of its changes.
const ALLOCATIONS: &[&str] = &["lulcc.step", "dinamica.step"];

/// A value at the cell.
#[derive(Debug, Clone, PartialEq)]
pub enum At {
    /// A number (NaN: no value there).
    Number(f64),
    /// A stack's (or rasters') layers there, by name.
    Layers(Vec<(String, f64)>),
    /// A loop's series: the value there at every step.
    Steps(Vec<f64>),
    /// Not a map: the value in a few words (a table, a model, a text).
    Other(String),
}

impl At {
    /// The value in words.
    pub fn text(&self) -> String {
        match self {
            At::Number(x) => cell(*x),
            At::Layers(l) => l.iter().map(|(k, v)| format!("{k} {}", cell(*v))).collect::<Vec<_>>().join(", "),
            At::Steps(s) => steps_text(s),
            At::Other(t) => t.clone(),
        }
    }

    /// The single number it is (a series: its last step).
    pub fn number(&self) -> Option<f64> {
        match self {
            At::Number(x) => Some(*x),
            At::Steps(s) => s.last().copied(),
            At::Layers(l) if l.len() == 1 => Some(l[0].1),
            _ => None,
        }
    }
}

/// A series in words: its first value, then each change ("1 at the start, 2 from step 3").
pub fn steps_text(s: &[f64]) -> String {
    let Some(first) = s.first() else { return "no steps".into() };
    let mut parts = vec![format!("{} at step 1", cell(*first))];
    let mut last = *first;
    for (k, v) in s.iter().enumerate().skip(1) {
        let same = (v.is_nan() && last.is_nan()) || *v == last;
        if !same {
            parts.push(format!("{} from step {}", cell(*v), k + 1));
            last = *v;
        }
    }
    if parts.len() == 1 {
        parts[0] = format!("{} at every step ({})", cell(*first), s.len());
    } else {
        parts.push(format!("({} steps)", s.len()));
    }
    parts.join(", ")
}

/// How a node made its value from its inputs'.
#[derive(Debug, Clone, PartialEq)]
pub enum How {
    /// Data read or drawn (it reads no other node).
    Data,
    /// Map algebra: its expression, and the same with the values at the cell in place of its names.
    Expression { text: String, with_values: String },
    /// From the same cell of its inputs.
    Local,
    /// From the cells around: the windows of its inputs that are maps, by input (rows of values, the cell at the
    /// centre).
    Window { radius: u32, windows: Vec<(String, Vec<Vec<f64>>)> },
    /// From the whole map (a fit, an allocation, a sort): what the operator does, in a sentence.
    Whole(String),
    /// From the patch of cells it is in (a patch's operator: labels, sieves, MSPA...): the cells of its input's
    /// class joined to it by `neighbours` (8 or 4), how many (counted up to [`PATCH_MOST`]: `more` beyond) and the
    /// rows and columns they span.
    Patch {
        port: String,
        class: f64,
        cells: usize,
        more: bool,
        neighbours: u8,
        rows: (usize, usize),
        cols: (usize, usize),
        /// Its cells (row, column), to draw it on the map; none beyond [`PATCH_DRAWN`].
        members: Vec<(usize, usize)>,
    },
    /// An allocation's step: what the operator does, and in sentences the cell's class before and after and the
    /// demand (or the rates) of the steps it changed at.
    Allocation { what: String, lines: Vec<String> },
    /// A loop: the value at every step (its body's nodes traced when a run kept them).
    Loop,
    /// A group: what its body gave.
    Group,
    /// A switch (`control.switch`): what its case was and the branch it ran (the group named as the case,
    /// `case_<n>` for a number, else `otherwise`; none when not known); in a loop, the branch of every step (empty
    /// where it did not run), when a run kept them.
    Branch { case: String, branch: Option<String>, steps: Vec<String> },
}

/// The branches a switch ran step by step, in words ("dry at steps 1 to 4, wet at steps 5 to 9").
pub fn branches_text(steps: &[String]) -> String {
    let mut runs: Vec<(&str, usize, usize)> = Vec::new();
    for (k, b) in steps.iter().enumerate() {
        match runs.last_mut() {
            Some((last, _, end)) if *last == b.as_str() => *end = k + 1,
            _ => runs.push((b.as_str(), k + 1, k + 1)),
        }
    }
    if let [(b, _, _)] = runs.as_slice() {
        return format!("{b} at every step ({})", steps.len());
    }
    runs.iter()
        .map(|(b, from, to)| {
            let b = if b.is_empty() { "no branch" } else { b };
            if from == to { format!("{b} at step {from}") } else { format!("{b} at steps {from} to {to}") }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

impl How {
    /// A switch's branch in words ("the case is dry: its branch dry ran, the others did not"); empty for the others.
    pub fn branch_text(&self) -> String {
        let How::Branch { case, branch, steps } = self else { return String::new() };
        if !steps.is_empty() {
            return format!("a switch, its case {case}: the branch of each step, {}", branches_text(steps));
        }
        match branch {
            Some(b) => format!("a switch, its case {case}: the branch {b} ran, the others did not"),
            None => format!("a switch, its case {case}: the branch that ran is not known (not run)"),
        }
    }

    /// A patch in words ("from the patch of map it is in: class 2, 1204 cells joined by 8 neighbours, rows 40 to
    /// 80, columns 10 to 55"); empty for the others.
    pub fn patch_text(&self) -> String {
        let How::Patch { port, class, cells, more, neighbours, rows, cols, .. } = self else { return String::new() };
        if class.is_nan() {
            return format!("on no patch of {port}: the cell has no value there");
        }
        format!(
            "from the patch of {port} it is in: class {}, {}{cells} cells joined by {neighbours} neighbours, rows {} to \
             {}, columns {} to {}",
            cell(*class),
            if *more { "more than " } else { "" },
            rows.0,
            rows.1,
            cols.0,
            cols.1
        )
    }
}

/// An input of a traced node: its port, where it comes from, its value at the cell, and the node traced from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub port: String,
    pub from: String,
    pub value: Option<At>,
    /// The step traced from it (its index in [`Trace::steps`]).
    pub step: Option<usize>,
}

/// A node traced.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// The containers it is in, its id, operator and the port traced.
    pub path: Vec<String>,
    pub node: String,
    pub op: String,
    pub port: String,
    pub note: Option<String>,
    pub value: Option<At>,
    pub how: How,
    pub inputs: Vec<Input>,
    /// A loop's body traced step by step (a run that kept it): each body node's value at every step.
    pub body: Vec<(String, Vec<f64>)>,
}

/// What a run kept inside loops at the cell ([`keep_loops`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Kept {
    /// By the loop's path and the body node's `id.port` (through a group `g.inner.port`, through a switch
    /// `sw.branch.inner.port`), a value per step (NaN at a step it did not give one).
    pub values: BTreeMap<(String, String), Vec<f64>>,
    /// The branch a switch of a body ran at every step (empty at a step it did not run), by the loop's path and
    /// `sw.branch` (`g.sw.branch` through a group).
    pub branches: BTreeMap<(String, String), Vec<String>>,
}

impl Kept {
    pub fn new() -> Self {
        Self::default()
    }

    /// The values of a body node's port, step by step.
    pub fn get(&self, key: &(String, String)) -> Option<&Vec<f64>> {
        self.values.get(key)
    }
}

/// A cell's trace.
#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    /// The place: its coordinates (the cell's centre), and the row and column of the grid it was picked on.
    pub x: f64,
    pub y: f64,
    pub cell: (usize, usize),
    /// The steps, the first the node asked about; each traced once (a node read twice is told once).
    pub steps: Vec<Step>,
    /// Nodes left out (the trace stops at [`MOST`]).
    pub cut: bool,
}

/// What a value is at a place given by its coordinates: the cell of its grid there.
pub fn cell_of(grid: &terraflux_raster::Grid, x: f64, y: f64) -> Option<(usize, usize)> {
    let t = grid.transform;
    if t[1] == 0.0 || t[5] == 0.0 {
        return None;
    }
    let (c, r) = ((x - t[0]) / t[1], (y - t[3]) / t[5]);
    (c >= 0.0 && r >= 0.0 && (c as usize) < grid.width && (r as usize) < grid.height)
        .then_some((r as usize, c as usize))
}

/// The coordinates of a cell's centre.
pub fn centre(grid: &terraflux_raster::Grid, row: usize, col: usize) -> (f64, f64) {
    let t = grid.transform;
    let (c, r) = (col as f64 + 0.5, row as f64 + 0.5);
    (t[0] + c * t[1] + r * t[2], t[3] + c * t[4] + r * t[5])
}

/// The grid of a value of maps.
pub fn grid_of(v: &Value) -> Option<std::sync::Arc<terraflux_raster::Grid>> {
    match v {
        Value::Map(m) => Some(m.grid.clone()),
        Value::Stack(s) => Some(s.grid.clone()),
        Value::Raster(r) => Some(r.grid.clone()),
        Value::Objects(o) => Some(o.grid.clone()),
        Value::Series(items) => items.last().and_then(grid_of),
        _ => None,
    }
}

/// A value at the place (x, y).
pub fn at(v: &Value, x: f64, y: f64) -> Option<At> {
    match v {
        Value::Map(m) => {
            let (r, c) = cell_of(&m.grid, x, y)?;
            Some(At::Number(m.data[r * m.grid.width + c]))
        }
        Value::Stack(s) => {
            let (r, c) = cell_of(&s.grid, x, y)?;
            let k = r * s.grid.width + c;
            Some(At::Layers(s.keys.iter().zip(&s.layers).map(|(key, l)| (key.to_string(), l[k])).collect()))
        }
        Value::Raster(rs) => {
            let (r, c) = cell_of(&rs.grid, x, y)?;
            let win = terraflux_raster::tiff::Window { x0: c as i64, y0: r as i64, width: 1, height: 1 };
            let values: Vec<(String, f64)> = (0..rs.layers.len())
                .filter_map(|l| Some((rs.layers[l].name.clone(), *rs.window(l, win).ok()?.first()?)))
                .collect();
            match values.as_slice() {
                [(_, one)] => Some(At::Number(*one)),
                [] => None,
                _ => Some(At::Layers(values)),
            }
        }
        Value::Series(items) if items.iter().all(|i| matches!(i, Value::Map(_) | Value::Raster(_))) => {
            Some(At::Steps(items.iter().map(|i| at(i, x, y).and_then(|a| a.number()).unwrap_or(f64::NAN)).collect()))
        }
        Value::Objects(o) => {
            let (r, c) = cell_of(&o.grid, x, y)?;
            let id = o.id_at(r, c);
            if id == 0 {
                return Some(At::Other("no object".into()));
            }
            let k = id as usize - 1;
            let mut parts = vec![format!("object {id}")];
            parts.push(o.class_name(id).map_or("unclassified".to_string(), |n| format!("class {n}")));
            if let Some(p) = o.parent_id(id) {
                parts.push(format!("parent {p}"));
            }
            parts.push(format!("{} neighbours", o.neighbours_of(id).len()));
            // (the membership to each class, and the margin: the largest minus the next)
            if !o.membership.is_empty() {
                let said: Vec<String> =
                    o.classes.iter().zip(&o.membership).map(|(c, m)| format!("{} {}", c.name, cell(m[k]))).collect();
                parts.push(format!("membership {}", said.join(" ")));
                parts.push(format!("margin {}", cell(crate::objects_inspect::margin_in(o, id))));
            }
            for (name, v) in o.table.columns.iter().zip(&o.table.rows[k]).skip(1) {
                parts.push(format!("{name} {}", cell(*v)));
            }
            Some(At::Other(parts.join(", ")))
        }
        Value::Number(n) => Some(At::Number(*n)),
        Value::Integer(n) => Some(At::Number(*n as f64)),
        Value::Bool(b) => Some(At::Number(f64::from(u8::from(*b)))),
        other => Some(At::Other(crate::tools::summarise(other).lines().next().unwrap_or_default().to_string())),
    }
}

/// The window of cells (rows) of a map around a place, `radius` cells on each side (NaN outside the map).
fn window(v: &Value, x: f64, y: f64, radius: usize) -> Option<Vec<Vec<f64>>> {
    let m = match v {
        Value::Map(m) => m,
        Value::Series(items) => match items.last()? {
            Value::Map(m) => m,
            _ => return None,
        },
        _ => return None,
    };
    let (r, c) = cell_of(&m.grid, x, y)?;
    let (w, h) = (m.grid.width as i64, m.grid.height as i64);
    let rad = radius as i64;
    Some(
        (-rad..=rad)
            .map(|dr| {
                (-rad..=rad)
                    .map(|dc| {
                        let (rr, cc) = (r as i64 + dr, c as i64 + dc);
                        if rr < 0 || cc < 0 || rr >= h || cc >= w { f64::NAN } else { m.data[(rr * w + cc) as usize] }
                    })
                    .collect()
            })
            .collect(),
    )
}

/// Map algebra's expression with the values at the cell in place of its names: `t0` (a map or a number) and
/// `stack.layer` (a stack's layer).
fn substituted(expr: &str, values: &BTreeMap<String, At>) -> String {
    let chars: Vec<char> = expr.chars().collect();
    let mut out = String::new();
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        if c.is_alphabetic() || c == '_' {
            let start = k;
            while k < chars.len() && (chars[k].is_alphanumeric() || chars[k] == '_') {
                k += 1;
            }
            let name: String = chars[start..k].iter().collect();
            // a stack's layer: name.layer
            if k + 1 < chars.len() && chars[k] == '.' && (chars[k + 1].is_alphanumeric() || chars[k + 1] == '_') {
                let mut e = k + 1;
                while e < chars.len() && (chars[e].is_alphanumeric() || chars[e] == '_') {
                    e += 1;
                }
                let layer: String = chars[k + 1..e].iter().collect();
                if let Some(At::Layers(l)) = values.get(&name)
                    && let Some((_, v)) = l.iter().find(|(n, _)| *n == layer)
                {
                    out.push_str(&cell(*v));
                    k = e;
                    continue;
                }
            }
            // a function's name stays (followed by "(")
            let call = chars[k..].iter().find(|c| !c.is_whitespace()) == Some(&'(');
            match values.get(&name).and_then(At::number) {
                Some(v) if !call => out.push_str(&cell(v)),
                _ => out.push_str(&name),
            }
            continue;
        }
        out.push(c);
        k += 1;
    }
    out
}

/// What a reference read at a scope is given by: an input of a group or a switch around it (`@name`) followed to
/// the reference that container's input reads, at the container's own scope (groups in groups); the others as
/// they are. None for what a loop carries (its step's) and for a literal given to the container.
fn followed(m: &Model, path: &[String], reference: &str) -> Option<(Vec<String>, String)> {
    let Ok(Ref::Carried(name)) = Ref::parse(reference) else { return Some((path.to_vec(), reference.to_string())) };
    for k in (0..path.len()).rev() {
        let c = m.scope(&path[..k])?.iter().find(|n| n.id == path[k])?;
        if c.op != GROUP && c.op != terraflux_graph::SWITCH {
            // (a loop's carried values and step: what its step was)
            return None;
        }
        // (a switch's case is not read by its branches)
        if c.op == terraflux_graph::SWITCH && name == "case" {
            continue;
        }
        match c.inputs.get(&name) {
            Some(Source::Ref(r)) => return followed(m, &path[..k], r),
            Some(Source::Literal(_)) => return None,
            None => continue,
        }
    }
    None
}

/// The value at the cell of a reference that is no node's: a model's input of a value (`$weight`), through the
/// groups and switches that pass it on.
fn input_value(m: &Model, path: &[String], reference: &str) -> Option<At> {
    let (_, r) = followed(m, path, reference)?;
    let Ok(Ref::Input(name)) = Ref::parse(&r) else { return None };
    match m.inputs.get(&name)? {
        terraflux_graph::Input::Value { value, .. } => Some(match value.as_f64() {
            Some(x) => At::Number(x),
            None => At::Other(words(value)),
        }),
        _ => None,
    }
}

/// Where a reference read at a scope points: the node (its path and id) and port, looked for in the scope, then
/// in those around it (a body reads the nodes around it); a group's output, at the node of its body giving it; a
/// group's or a switch's input (`@name`), at what it is given.
fn resolve(m: &Model, path: &[String], reference: &str) -> Option<(Vec<String>, String, String)> {
    let (path, reference) = followed(m, path, reference)?;
    let Ok(Ref::Port { node, port }) = Ref::parse(&reference) else { return None };
    let mut scope: Vec<String> = path;
    loop {
        if m.scope(&scope).is_some_and(|ns| ns.iter().any(|n| n.id == node)) {
            break;
        }
        scope.pop()?;
    }
    let (mut path, mut node, mut port) = (scope, node, port);
    // through groups to what gives their outputs
    while let Some(n) = m.scope(&path).and_then(|ns| ns.iter().find(|n| n.id == node)) {
        let inner =
            (n.op == GROUP).then(|| n.body.as_ref()?.outputs.get(&port)).flatten().and_then(|r| match Ref::parse(r) {
                Ok(Ref::Port { node, port }) => Some((node, port)),
                _ => None,
            });
        let Some((i, p)) = inner else { break };
        path.push(node);
        (node, port) = (i, p);
    }
    Some((path, node, port))
}

/// Where the value of a node's port is kept after a run (as [`crate::document::Step::value_key`]): the top-level
/// node's, or the top-level container's (`inner.port`, through groups and the branches of switches: a switch keeps
/// what the branch that ran gave, `branch.node.port`); none in a loop.
pub fn key(m: &Model, path: &[String], node: &str, port: &str) -> Option<(String, String)> {
    kept_at(m, path, node, port, false)
}

/// As [`key`], and in a loop's body too: the value its last step gave (`control.repeat` keeps it, as a group what
/// its body gave; `control.each` its last element's); what the viewer shows of a node of a body.
pub fn shown_key(m: &Model, path: &[String], node: &str, port: &str) -> Option<(String, String)> {
    kept_at(m, path, node, port, true)
}

/// Whether a place goes through a loop's body (`control.repeat`, `control.each`): what is shown there is its last
/// step's (its last element's).
pub fn in_loop(m: &Model, path: &[String]) -> bool {
    loop_on(m, path).is_some()
}

/// The first loop a place goes through: its operator (`control.repeat` or `control.each`).
pub fn loop_on(m: &Model, path: &[String]) -> Option<&'static str> {
    (0..path.len()).find_map(|k| {
        let n = m.scope(&path[..k])?.iter().find(|n| n.id == path[k])?;
        [REPEAT, EACH].into_iter().find(|l| n.op == *l)
    })
}

/// Whether a place is the top level or the body of a group (groups in groups) or of a switch's branch (a group):
/// where a node's result can be frozen.
pub fn in_groups(m: &Model, path: &[String]) -> bool {
    (0..path.len()).all(|k| {
        m.scope(&path[..k])
            .and_then(|ns| ns.iter().find(|n| n.id == path[k]))
            .is_some_and(|n| n.op == GROUP || n.op == terraflux_graph::SWITCH)
    })
}

/// Whether a node at a place can be frozen: [`in_groups`], and not a switch's branch itself (its result is held
/// through the nodes inside it; the branch runs as a whole).
pub fn can_freeze(m: &Model, path: &[String]) -> bool {
    in_groups(m, path)
        && path.last().is_none_or(|_| {
            m.scope(&path[..path.len() - 1])
                .and_then(|ns| ns.iter().find(|n| n.id == path[path.len() - 1]))
                .is_some_and(|n| n.op != terraflux_graph::SWITCH)
        })
}

const REPEAT: &str = "control.repeat";
const EACH: &str = "control.each";

fn kept_at(m: &Model, path: &[String], node: &str, port: &str, loops: bool) -> Option<(String, String)> {
    if path.is_empty() {
        return Some((node.to_string(), port.to_string()));
    }
    // every container on the way a group, a switch (its branches are groups) or, when asked, a loop
    for k in 0..path.len() {
        let n = m.scope(&path[..k])?.iter().find(|n| n.id == path[k])?;
        if !(n.op == GROUP || n.op == terraflux_graph::SWITCH || loops && (n.op == REPEAT || n.op == EACH)) {
            return None;
        }
    }
    let inner: Vec<&str> = path[1..].iter().map(String::as_str).chain([node, port]).collect();
    Some((path[0].clone(), inner.join(".")))
}

/// The values of a run, by node and port (a top-level node's, a group's `inner.port`).
pub type Values<'a> = &'a dyn Fn(&str, &str) -> Option<Value>;

/// The trace of a node's output at the cell (row, col) of the grid it was picked on, back through the graph.
#[allow(clippy::too_many_arguments)]
pub fn trace(
    m: &Model,
    r: &Registry,
    values: Values,
    kept: &Kept,
    node: &str,
    port: &str,
    grid: &terraflux_raster::Grid,
    cell: (usize, usize),
) -> Trace {
    let (x, y) = centre(grid, cell.0, cell.1);
    let mut t = Trace { x, y, cell, steps: Vec::new(), cut: false };
    let mut seen: HashSet<(Vec<String>, String)> = HashSet::new();
    let start = match port.rsplit_once('.') {
        // a node of a group's body (the top-level group's `inner.node.port`)
        Some((place, port)) => {
            let mut path: Vec<String> = std::iter::once(node).chain(place.split('.')).map(String::from).collect();
            let inner = path.pop().unwrap_or_default();
            (path, inner, port.to_string())
        }
        None => resolve(m, &[], &format!("{node}.{port}")).unwrap_or((Vec::new(), node.into(), port.into())),
    };
    walk(m, r, values, kept, &start.0, &start.1, &start.2, &mut t, &mut seen);
    t
}

/// The value of a node's port at the place: kept by the run (top level, groups), or step by step in a loop.
#[allow(clippy::too_many_arguments)]
fn value_at(
    m: &Model,
    values: Values,
    kept: &Kept,
    path: &[String],
    node: &str,
    port: &str,
    x: f64,
    y: f64,
) -> Option<At> {
    if let Some((n, p)) = key(m, path, node, port) {
        return values(&n, &p).and_then(|v| at(&v, x, y));
    }
    kept.get(&kept_key(m, path, node, port)).map(|s| At::Steps(s.clone()))
}

/// Where a run inside loops keeps a node's port ([`Kept`]): by the innermost loop on the way (its path), and the
/// rest of the way to the node with the node and port, dotted (a group's and a switch's body nodes are among their
/// outputs, `g.inner.port`, `sw.branch.inner.port`).
fn kept_key(m: &Model, path: &[String], node: &str, port: &str) -> (String, String) {
    let innermost = (0..path.len()).rev().find(|&k| {
        m.scope(&path[..k])
            .and_then(|ns| ns.iter().find(|n| n.id == path[k]))
            .is_some_and(|n| n.op == REPEAT || n.op == EACH)
    });
    match innermost {
        Some(k) => {
            let rest: Vec<&str> = path[k + 1..].iter().map(String::as_str).chain([node, port]).collect();
            (path[..=k].join("/"), rest.join("."))
        }
        None => (path.join("/"), format!("{node}.{port}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn walk(
    m: &Model,
    r: &Registry,
    values: Values,
    kept: &Kept,
    path: &[String],
    node: &str,
    port: &str,
    t: &mut Trace,
    seen: &mut HashSet<(Vec<String>, String)>,
) -> Option<usize> {
    if let Some(k) = t.steps.iter().position(|s| s.path == path && s.node == node) {
        return Some(k);
    }
    if t.steps.len() >= MOST {
        t.cut = true;
        return None;
    }
    if !seen.insert((path.to_vec(), node.to_string())) {
        return None;
    }
    let n: &Node = m.scope(path)?.iter().find(|n| n.id == node)?;
    let (x, y) = (t.x, t.y);
    let sig = r.sig(&n.op);
    let k = t.steps.len();
    t.steps.push(Step {
        path: path.to_vec(),
        node: node.to_string(),
        op: n.op.clone(),
        port: port.to_string(),
        note: n.note.clone(),
        value: value_at(m, values, kept, path, node, port, x, y),
        how: How::Local,
        inputs: Vec::new(),
        body: Vec::new(),
    });
    // the inputs: their values at the cell, and the nodes they come from traced in turn
    let mut inputs = Vec::new();
    let mut at_inputs: BTreeMap<String, At> = BTreeMap::new();
    for (p, src) in &n.inputs {
        let (from, value, traced) = match src {
            Source::Literal(v) => (words(v), Some(At::Other(words(v))), None),
            Source::Ref(rf) => match resolve(m, path, rf) {
                Some((q, id, op_port)) => {
                    let v = value_at(m, values, kept, &q, &id, &op_port, x, y);
                    let step = walk(m, r, values, kept, &q, &id, &op_port, t, seen);
                    (rf.clone(), v, step)
                }
                None => (rf.clone(), input_value(m, path, rf), None),
            },
        };
        if let Some(v) = &value {
            at_inputs.insert(p.clone(), v.clone());
        }
        inputs.push(Input { port: p.clone(), from, value, step: traced });
    }
    // a switch: the branch that ran (in a loop, those of its steps), traced from the node of its body giving the
    // port; the branches that did not run are not
    let mut switched = None;
    if n.op == terraflux_graph::SWITCH
        && let Some(b) = &n.body
    {
        let (branch, steps) = match key(m, path, node, "branch").and_then(|(kn, kp)| values(&kn, &kp)) {
            Some(Value::Text(t)) => (Some(t), Vec::new()),
            _ => match kept.branches.get(&kept_key(m, path, node, "branch")) {
                Some(s) => (s.iter().rev().find(|b| !b.is_empty()).cloned(), s.clone()),
                None => (None, Vec::new()),
            },
        };
        let mut ran: Vec<&str> = Vec::new();
        for b in steps.iter().map(String::as_str).chain(branch.as_deref()) {
            if !b.is_empty() && !ran.contains(&b) {
                ran.push(b);
            }
        }
        let inner: Vec<String> = path.iter().cloned().chain([node.to_string()]).collect();
        for id in ran {
            let Some(rf) = b.nodes.iter().find(|g| g.id == id).and_then(|g| g.body.as_ref()?.outputs.get(port)) else {
                continue;
            };
            let at_branch: Vec<String> = inner.iter().cloned().chain([id.to_string()]).collect();
            let Some((q, nid, p)) = resolve(m, &at_branch, rf) else { continue };
            let value = value_at(m, values, kept, &q, &nid, &p, x, y);
            let step = walk(m, r, values, kept, &q, &nid, &p, t, seen);
            inputs.push(Input { port: format!("(branch {id}) {port}"), from: format!("{id}: {rf}"), value, step });
        }
        let case = match (at_inputs.get("case"), n.inputs.get("case")) {
            (Some(v), _) => v.text(),
            (None, Some(Source::Ref(rf))) => rf.clone(),
            (None, Some(Source::Literal(v))) => words(v),
            (None, None) => "not given".into(),
        };
        switched = Some(How::Branch { case, branch, steps });
    }
    // what a container's body reads around it: traced as its inputs are (a loop's allocation reads the
    // suitability made before it); a switch's branches read through the branch that ran
    if let Some(b) = n.body.as_ref().filter(|_| switched.is_none()) {
        let inner: Vec<String> = path.iter().cloned().chain([node.to_string()]).collect();
        let own: HashSet<&str> = b.nodes.iter().map(|x| x.id.as_str()).collect();
        let mut read: Vec<(String, String)> = Vec::new();
        for x in &b.nodes {
            for (p, src) in &x.inputs {
                if let Source::Ref(rf) = src
                    && let Ok(Ref::Port { node: id, .. }) = Ref::parse(rf)
                    && !own.contains(id.as_str())
                    && !read.iter().any(|(_, f)| f == rf)
                {
                    read.push((format!("{}.{p}", x.id), rf.clone()));
                }
            }
        }
        for (p, rf) in read {
            let Some((q, id, op_port)) = resolve(m, &inner, &rf) else { continue };
            let value = value_at(m, values, kept, &q, &id, &op_port, x, y);
            let step = walk(m, r, values, kept, &q, &id, &op_port, t, seen);
            inputs.push(Input { port: format!("(in its body) {p}"), from: rf, value, step });
        }
    }
    let reads_nodes =
        n.inputs.values().any(|s| matches!(s, Source::Ref(r) if matches!(Ref::parse(r), Ok(Ref::Port { .. }))));
    let how = if let Some(h) = switched {
        h
    } else if n.op == "control.repeat" {
        How::Loop
    } else if n.op == GROUP {
        How::Group
    } else if n.op == "raster.calc" {
        let text = n.params.get("expression").map(words).unwrap_or_default();
        How::Expression { with_values: substituted(&text, &at_inputs), text }
    } else if n.op == "objects.resolve"
        && let Some(h) = resolve_how(n, &at_inputs)
    {
        h
    } else if !reads_nodes {
        How::Data
    } else if let Some(p) = PATCHES.contains(&n.op.as_str()).then(|| patch_of(m, r, path, n, values, x, y)).flatten() {
        p
    } else if ALLOCATIONS.contains(&n.op.as_str()) {
        let value = t.steps[k].value.clone();
        How::Allocation {
            what: format!(
                "from the whole map: {}",
                sig.map(|s| crate::node_view::first_sentence(&s.doc).0).unwrap_or_default()
            ),
            lines: allocation(m, path, n, values, value.as_ref(), &at_inputs, (x, y)),
        }
    } else {
        match sig.map(|s| s.access) {
            Some(Access::Halo(rad)) => {
                let radius = (rad as usize).min(WINDOW / 2);
                let windows = n
                    .inputs
                    .iter()
                    .filter_map(|(p, src)| {
                        let Source::Ref(rf) = src else { return None };
                        let (q, id, op_port) = resolve(m, path, rf)?;
                        let (kn, kp) = key(m, &q, &id, &op_port)?;
                        Some((p.clone(), window(&values(&kn, &kp)?, x, y, radius)?))
                    })
                    .collect();
                How::Window { radius: rad, windows }
            }
            Some(Access::Global) | Some(Access::None) => {
                How::Whole(sig.map(|s| crate::node_view::first_sentence(&s.doc).0).unwrap_or_default())
            }
            _ => How::Local,
        }
    };
    // a loop's body, step by step, when a run kept it
    let mut body = Vec::new();
    if n.body.is_some() {
        let at = path.iter().cloned().chain([node.to_string()]).collect::<Vec<_>>().join("/");
        for ((loop_path, what), s) in &kept.values {
            if *loop_path == at {
                body.push((what.clone(), s.clone()));
            }
        }
    }
    // a loop that allocates (its body kept): the demand of the steps the cell changed at
    let mut how = how;
    if how == How::Loop
        && let Some(b) = &n.body
    {
        let inner: Vec<String> = path.iter().cloned().chain([node.to_string()]).collect();
        for x in b.nodes.iter().filter(|x| ALLOCATIONS.contains(&x.op.as_str())) {
            let Some((_, after)) = body.iter().find(|(w, _)| *w == format!("{}.landscape", x.id)) else { continue };
            // the map each step starts from: the loop's own at the first, then what the step before made
            let carried = match x.inputs.get("landscape") {
                Some(Source::Ref(rf)) => rf.strip_prefix('@').and_then(|c| at_inputs.get(c)).and_then(At::number),
                _ => None,
            };
            let Some(start) = carried else { continue };
            let before: Vec<f64> = std::iter::once(start).chain(after.iter().copied()).take(after.len()).collect();
            let inputs: BTreeMap<String, At> = [("landscape".to_string(), At::Steps(before))].into();
            how = How::Allocation {
                what: format!("a loop: its value at every step, {} ({}) allocating each step", x.id, x.op),
                lines: allocation(m, &inner, x, values, Some(&At::Steps(after.clone())), &inputs, (t.x, t.y)),
            };
            break;
        }
    }
    let step = &mut t.steps[k];
    step.inputs = inputs;
    step.how = how;
    step.body = body;
    Some(k)
}

/// What an `objects.resolve` decided at the cell: the memberships of each class, the winner, the doubt, the conflict
/// (the sentence of `terraflux_ops::objects::explain_cell`); none when the memberships at the cell are not known.
fn resolve_how(n: &Node, at_inputs: &BTreeMap<String, At>) -> Option<How> {
    let Some(At::Layers(layers)) = at_inputs.get("memberships") else { return None };
    let names: Vec<String> = layers.iter().map(|(k, _)| k.clone()).collect();
    let values: Vec<f64> = layers.iter().map(|(_, v)| *v).collect();
    let min = n.params.get("min").and_then(|v| v.as_f64()).unwrap_or(0.1);
    let order: Option<Vec<String>> = n
        .params
        .get("order")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect());
    Some(How::Whole(format!(
        "the class of the greatest membership wins a cell that reaches {min}: {}",
        terraflux_ops::objects::explain_cell(&names, &values, order.as_deref(), min)
    )))
}

/// The map an input of a node reads, by its source (a series: its last step).
fn input_map(m: &Model, path: &[String], src: &Source, values: Values) -> Option<terraflux_ops::Map> {
    let Source::Ref(rf) = src else { return None };
    let (q, id, op_port) = resolve(m, path, rf)?;
    let (kn, kp) = key(m, &q, &id, &op_port)?;
    match values(&kn, &kp)? {
        Value::Map(map) => Some(map),
        Value::Series(s) => match s.last() {
            Some(Value::Map(map)) => Some(map.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// The patch of a patch operator's input the place is in: the cells of its class joined to it.
fn patch_of(m: &Model, r: &Registry, path: &[String], n: &Node, values: Values, x: f64, y: f64) -> Option<How> {
    // the neighbours that join a patch: the node's setting, else its operator's default (8 unless it says 4)
    let sig = r.sig(&n.op);
    let neighbours =
        ["connectivity", "directions", "neighbours", "neighbourhood"].iter().find_map(|k| match n.params.get(*k) {
            Some(v) => v.as_f64(),
            None => {
                let doc = &sig?.params.iter().find(|p| p.name == *k)?.doc;
                (doc.contains("4 (default)") || doc.contains("default 4")).then_some(4.0)
            }
        });
    let neighbours: u8 = if neighbours == Some(4.0) { 4 } else { 8 };
    let (port, map) = n.inputs.iter().find_map(|(p, src)| Some((p.clone(), input_map(m, path, src, values)?)))?;
    let (r, c) = cell_of(&map.grid, x, y)?;
    let (w, h) = (map.grid.width, map.grid.height);
    let class = map.data[r * w + c];
    let (mut rows, mut cols) = ((r, r), (c, c));
    if class.is_nan() {
        return Some(How::Patch { port, class, cells: 0, more: false, neighbours, rows, cols, members: Vec::new() });
    }
    let mut seen = vec![false; w * h];
    let mut todo = vec![(r, c)];
    seen[r * w + c] = true;
    let mut cells = 0;
    let mut members = Vec::new();
    while let Some((i, j)) = todo.pop() {
        cells += 1;
        rows = (rows.0.min(i), rows.1.max(i));
        cols = (cols.0.min(j), cols.1.max(j));
        if cells <= PATCH_DRAWN {
            members.push((i, j));
        } else {
            members = Vec::new();
        }
        if cells >= PATCH_MOST {
            return Some(How::Patch { port, class, cells, more: true, neighbours, rows, cols, members });
        }
        for (di, dj) in
            [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)].iter().take(neighbours as usize)
        {
            let (a, b) = (i as i64 + di, j as i64 + dj);
            if a < 0 || b < 0 || a >= h as i64 || b >= w as i64 {
                continue;
            }
            let k = a as usize * w + b as usize;
            if !seen[k] && map.data[k] == class {
                seen[k] = true;
                todo.push((a as usize, b as usize));
            }
        }
    }
    Some(How::Patch { port, class, cells, more: false, neighbours, rows, cols, members })
}

/// What of a patch is drawn on the map: the sides of its cells that border a cell not in it, each from a corner to
/// another (x a column, y a row, the cells' corners), or none when it has too many cells to keep (its rows and
/// columns then, as a rectangle).
pub fn patch_outline(how: &How) -> Option<crate::map_view::Outline> {
    let How::Patch { members, rows, cols, cells, .. } = how else { return None };
    if *cells == 0 {
        return None;
    }
    let bounds = (cols.0 as f32, rows.0 as f32, cols.1 as f32 + 1.0, rows.1 as f32 + 1.0);
    if members.is_empty() {
        return Some(crate::map_view::Outline { edges: Vec::new(), bounds });
    }
    let set: HashSet<(usize, usize)> = members.iter().copied().collect();
    let has = |r: i64, c: i64| r >= 0 && c >= 0 && set.contains(&(r as usize, c as usize));
    let mut edges = Vec::new();
    for &(r, c) in members {
        let (x, y) = (c as f32, r as f32);
        let (ri, ci) = (r as i64, c as i64);
        if !has(ri - 1, ci) {
            edges.push([(x, y), (x + 1.0, y)]);
        }
        if !has(ri + 1, ci) {
            edges.push([(x, y + 1.0), (x + 1.0, y + 1.0)]);
        }
        if !has(ri, ci - 1) {
            edges.push([(x, y), (x, y + 1.0)]);
        }
        if !has(ri, ci + 1) {
            edges.push([(x + 1.0, y), (x + 1.0, y + 1.0)]);
        }
    }
    Some(crate::map_view::Outline { edges, bounds })
}

/// An allocation's step at the cell in sentences: its class before and after, and at each step it changed, the
/// demand of that step (the cells of each class, and how many more or fewer than the step before) or the rate of
/// its change (Dinamica's Markov chain).
fn allocation(
    m: &Model,
    path: &[String],
    n: &Node,
    values: Values,
    value: Option<&At>,
    inputs: &BTreeMap<String, At>,
    place: (f64, f64),
) -> Vec<String> {
    let mut lines = Vec::new();
    // the steps it changed at: (step, from, to)
    let same = |a: f64, b: f64| a == b || (a.is_nan() && b.is_nan());
    let changes: Vec<(usize, f64, f64)> = match (inputs.get("landscape"), value) {
        (Some(At::Steps(before)), Some(At::Steps(after))) => after
            .iter()
            .enumerate()
            .filter_map(|(k, &to)| before.get(k).filter(|&&from| !same(from, to)).map(|&from| (k + 1, from, to)))
            .collect(),
        (Some(before), Some(after)) => {
            let (Some(from), Some(to)) = (before.number(), after.number()) else { return lines };
            let step = inputs.get("step").and_then(At::number).map_or(1, |s| s as usize);
            if same(from, to) {
                lines.push(format!("here the class stays {}", cell(from)));
                return lines;
            }
            vec![(step, from, to)]
        }
        _ => return lines,
    };
    if changes.is_empty() {
        let kept = inputs.get("landscape").and_then(At::number).map(cell).unwrap_or_default();
        lines.push(format!("here the class stays {kept} at every step"));
        return lines;
    }
    let read = |port: &str| {
        n.inputs.get(port).and_then(|src| {
            let Source::Ref(rf) = src else { return None };
            let (q, id, p) = resolve(m, path, rf)?;
            let (kn, kp) = key(m, &q, &id, &p)?;
            values(&kn, &kp)
        })
    };
    let demand = read("demand");
    let rates = read("rates");
    for (step, from, to) in changes.iter().take(5) {
        let mut line = format!("step {step}: from class {} to {}", cell(*from), cell(*to));
        match (&demand, &rates) {
            (Some(Value::Demand(d)), _) => {
                let (Some(now), Some(before)) = (d.table.get(*step), d.table.get(step - 1)) else {
                    lines.push(line);
                    continue;
                };
                let classes: Vec<String> = d
                    .classes
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let delta = now[i] - before[i];
                        let sign = if delta > 0.0 { "+" } else { "" };
                        format!("{} {} ({sign}{})", cell(*c), cell(now[i]), cell(delta))
                    })
                    .collect();
                line.push_str(&format!("; the step's demand, in cells of each class: {}", classes.join(", ")));
            }
            (_, Some(Value::Markov(mk))) => {
                let k = mk.k();
                let (i, j) = (mk.classes.iter().position(|c| c == from), mk.classes.iter().position(|c| c == to));
                if let (Some(i), Some(j)) = (i, j)
                    && let Some(rate) = mk.multi.get(i * k + j)
                {
                    line.push_str(&format!(
                        "; the rate of {} to {} each step: {} of the cells of {}",
                        cell(*from),
                        cell(*to),
                        cell(*rate),
                        cell(*from)
                    ));
                }
            }
            _ => {}
        }
        lines.push(line);
    }
    if changes.len() > 5 {
        lines.push(format!("and {} more changes", changes.len() - 5));
    }
    // why the cell: once per change (from, to) it made
    let mut told: Vec<(f64, f64)> = Vec::new();
    for (_, from, to) in changes.iter().take(5) {
        if told.iter().any(|(a, b)| same(*a, *from) && same(*b, *to)) {
            continue;
        }
        told.push((*from, *to));
        let why = match n.op.as_str() {
            "lulcc.step" => read("suitability").and_then(|s| clue_why(n, &s, demand.as_ref(), place, *from, *to)),
            "dinamica.step" => read("probability").and_then(|p| patcher_why(&p, place, *from, *to)),
            _ => None,
        };
        lines.extend(why);
    }
    lines
}

/// The share of a layer's cells with a value below `v` (of those with a value), in percent.
fn below(layer: &[f64], v: f64) -> f64 {
    let (n, under) =
        layer.iter().filter(|x| x.is_finite()).fold((0usize, 0usize), |(n, u), x| (n + 1, u + usize::from(*x < v)));
    100.0 * under as f64 / n.max(1) as f64
}

/// Why CLUE's step gave the cell the class `to`: its suitability for each class there (the layers by class, else in
/// the demand's order), how it ranks among the cells for `to`, the elasticity its class `from` held it with; the
/// iteration variable CLUE adds to each class until the demand is met is not kept.
fn clue_why(
    n: &Node,
    suitability: &Value,
    demand: Option<&Value>,
    (x, y): (f64, f64),
    from: f64,
    to: f64,
) -> Option<String> {
    let Value::Stack(s) = suitability else { return None };
    let (r, c) = cell_of(&s.grid, x, y)?;
    let k = r * s.grid.width + c;
    let order: Vec<f64> = match demand {
        Some(Value::Demand(d)) => d.classes.clone(),
        _ => Vec::new(),
    };
    // each layer's class
    let classes: Vec<Option<f64>> = s
        .keys
        .iter()
        .enumerate()
        .map(|(i, key)| match key {
            terraflux_ops::Key::Class(c) => Some(*c),
            _ => order.get(i).copied(),
        })
        .collect();
    let here: Vec<(Option<f64>, String, f64)> = s
        .keys
        .iter()
        .zip(&s.layers)
        .zip(&classes)
        .map(|((key, l), class)| (*class, class.map_or_else(|| key.to_string(), cell), l[k]))
        .collect();
    let mine = here.iter().position(|(c, _, _)| *c == Some(to))?;
    let v = here[mine].2;
    let highest = here.iter().all(|(_, _, w)| w.is_nan() || *w <= v);
    let list: Vec<String> = here.iter().map(|(_, name, w)| format!("{name} {}", cell(*w))).collect();
    let mut line = format!(
        "why {} → {}: its suitability here, by class: {}; for class {}: {} ({}), above {:.0}% of the cells",
        cell(from),
        cell(to),
        list.join(", "),
        cell(to),
        cell(v),
        if highest { "its highest" } else { "not its highest" },
        below(&s.layers[mine], v)
    );
    // the elasticity of the class it had: added to its suitability for that class, so it keeps it
    let at = classes.iter().position(|c| *c == Some(from));
    let elasticity = n.params.get("elasticity").and_then(|e| match e {
        Json::Number(x) => x.as_f64(),
        Json::Array(a) => at.and_then(|i| a.get(i)).and_then(Json::as_f64),
        Json::Object(o) => {
            o.iter().find(|(k, _)| k.trim().parse::<f64>().ok() == Some(from)).and_then(|(_, v)| v.as_f64())
        }
        _ => None,
    });
    if let Some(e) = elasticity.filter(|e| *e > 0.0) {
        line.push_str(&format!("; its class {} held it with an elasticity of {}", cell(from), cell(e)));
    }
    line.push_str(
        "; CLUE adds to each class a value of its own until the demand is met (the iteration variable, not kept by \
         the step), and the cell takes the class of the highest sum",
    );
    Some(line)
}

/// Why Dinamica's step changed the cell from `from` to `to`: its probability of that transition there and how it
/// ranks among the cells; the Expander and the Patcher draw the cells of each change among the most probable.
fn patcher_why(probability: &Value, (x, y): (f64, f64), from: f64, to: f64) -> Option<String> {
    let Value::Stack(s) = probability else { return None };
    let (r, c) = cell_of(&s.grid, x, y)?;
    let i = s.keys.iter().position(|k| *k == terraflux_ops::Key::Transition(from, to))?;
    let v = s.layers[i][r * s.grid.width + c];
    Some(format!(
        "why {} → {}: its probability of that change here {}, above {:.0}% of the cells; the Expander and the \
         Patcher draw the cells of each change among the most probable ones (prune_factor times the cells it needs)",
        cell(from),
        cell(to),
        cell(v),
        below(&s.layers[i], v)
    ))
}

/// What keeps the values of the loops' bodies at a place, step by step, while the model runs.
struct Keeper {
    x: f64,
    y: f64,
    kept: Kept,
}

impl terraflux_exec::Observer for Keeper {
    fn body(&mut self, at: &str, step: usize, results: &BTreeMap<String, terraflux_ops::Outputs>) {
        for (node, outs) in results {
            for (port, v) in outs {
                // the branch a switch ran (its `branch`, a group's `sw.branch`)
                if let Value::Text(b) = v
                    && (port == "branch" || port.ends_with(".branch"))
                {
                    let s = self.kept.branches.entry((at.to_string(), format!("{node}.{port}"))).or_default();
                    s.resize(step - 1, String::new());
                    s.push(b.clone());
                    continue;
                }
                let Some(x) = self::at(v, self.x, self.y).and_then(|a| a.number()) else { continue };
                let s = self.kept.values.entry((at.to_string(), format!("{node}.{port}"))).or_default();
                // (a step missed, as a loop's first steps before a node of it gave a map, or a switch's branch
                // that did not run: no value)
                s.resize(step - 1, f64::NAN);
                s.push(x);
            }
        }
    }
}

impl Keeper {
    /// Every series of a loop as long as its longest (a node that gave nothing at the last steps: no value there,
    /// as a switch's branch that ran only at the first).
    fn even(&mut self) {
        let mut most: BTreeMap<String, usize> = BTreeMap::new();
        for ((at, _), s) in &self.kept.values {
            let n = most.entry(at.clone()).or_default();
            *n = (*n).max(s.len());
        }
        for ((at, _), s) in &self.kept.branches {
            let n = most.entry(at.clone()).or_default();
            *n = (*n).max(s.len());
        }
        for ((at, _), s) in self.kept.values.iter_mut() {
            s.resize(most[at], f64::NAN);
        }
        for ((at, _), s) in self.kept.branches.iter_mut() {
            s.resize(most[at], String::new());
        }
    }
}

/// Run the model again keeping what the nodes of its loops give at the place (x, y) at every step: the trace
/// then goes into the loops ([`trace`]'s `kept`). The model expanded, its data read from `base`.
pub fn keep_loops(m: &Model, base: &std::path::Path, x: f64, y: f64) -> Result<Kept, String> {
    let mut m = m.clone();
    terraflux_graph::submodel::expand(&mut m, base).map_err(|e| e.to_string())?;
    let inputs = terraflux_exec::load_inputs(&m, base, &BTreeMap::new()).map_err(|e| e.to_string())?;
    let mut keeper = Keeper { x, y, kept: Kept::new() };
    terraflux_exec::Engine::default().run(&m, &inputs, &mut keeper).map_err(|e| e.to_string())?;
    keeper.even();
    Ok(keeper.kept)
}

/// The file the application keeps the last trace in (beside its settings), for an assistant through `terraflux
/// mcp` ([`last`]).
pub fn last_file() -> Option<std::path::PathBuf> {
    Some(crate::settings::file()?.parent()?.join("last_trace.txt"))
}

/// Keep a trace as the last one made: the model's file, then its words.
pub fn save_last(model: Option<&std::path::Path>, t: &Trace) {
    let Some(f) = last_file() else { return };
    let model = model.map_or("a model not saved".to_string(), |p| p.display().to_string());
    let _ = std::fs::write(f, format!("model: {model}\n\n{}\n\n{}", t.sentence(), t.text()));
}

/// The last trace made in the application (the model's file first), if any.
pub fn last() -> Option<String> {
    std::fs::read_to_string(last_file()?).ok()
}

impl Trace {
    /// The trace in a few lines of words: where, what the node asked about is there, and every step with how it
    /// made its value (for the pane's summary, a report, an assistant).
    pub fn text(&self) -> String {
        let mut out = format!(
            "The cell at row {}, column {} (x {}, y {}).\n",
            self.cell.0,
            self.cell.1,
            cell(self.x),
            cell(self.y)
        );
        for (k, s) in self.steps.iter().enumerate() {
            let at = if s.path.is_empty() { String::new() } else { format!(" (in {})", s.path.join("/")) };
            let value = s.value.as_ref().map_or("no value kept".to_string(), At::text);
            out.push_str(&format!("{}. {}.{}{at} [{}] = {value}\n", k + 1, s.node, s.port, s.op));
            if let Some(n) = &s.note {
                out.push_str(&format!("   note: {n}\n"));
            }
            match &s.how {
                How::Data => out.push_str("   data: read or drawn, from no other node\n"),
                How::Expression { text, with_values } => {
                    out.push_str(&format!("   expression: {text}\n   with the values here: {with_values}\n"))
                }
                How::Window { radius, windows } => {
                    out.push_str(&format!("   from the cells within {radius} of this one\n"));
                    for (p, w) in windows {
                        let rows: Vec<String> =
                            w.iter().map(|r| r.iter().map(|v| cell(*v)).collect::<Vec<_>>().join(" ")).collect();
                        out.push_str(&format!("   {p} around the cell: [{}]\n", rows.join(" | ")));
                    }
                }
                How::Whole(what) => out.push_str(&format!("   from the whole map: {what}\n")),
                How::Patch { .. } => out.push_str(&format!("   {}\n", s.how.patch_text())),
                How::Allocation { what, lines } => {
                    out.push_str(&format!("   {what}\n"));
                    for l in lines {
                        out.push_str(&format!("   {l}\n"));
                    }
                }
                How::Loop => out.push_str("   a loop: its value at every step above\n"),
                How::Group => out.push_str("   a group: what its body gave\n"),
                How::Branch { .. } => out.push_str(&format!("   {}\n", s.how.branch_text())),
                How::Local => {}
            }
            for i in &s.inputs {
                let v = i.value.as_ref().map_or("?".to_string(), At::text);
                let traced = i.step.map(|k| format!(" (step {})", k + 1)).unwrap_or_default();
                out.push_str(&format!("   input {} <- {} = {v}{traced}\n", i.port, i.from));
            }
            for (what, values) in &s.body {
                out.push_str(&format!("   in the loop, {what}: {}\n", steps_text(values)));
            }
        }
        if self.cut {
            out.push_str(&format!("(the trace stops at {MOST} nodes)\n"));
        }
        out
    }

    /// The answer in a sentence: what the node asked about is at the cell, and from what.
    pub fn sentence(&self) -> String {
        let Some(first) = self.steps.first() else { return "Nothing to trace.".into() };
        let value = first.value.as_ref().map_or("unknown (not run)".to_string(), At::text);
        let mut s = format!("Here {}.{} is {value}", first.node, first.port);
        let from: Vec<String> = first
            .inputs
            .iter()
            .filter(|i| i.step.is_some() && matches!(i.value, Some(At::Number(_) | At::Layers(_) | At::Steps(_))))
            .take(4)
            .map(|i| format!("{} = {}", i.from, i.value.as_ref().map_or("?".into(), At::text)))
            .collect();
        match &first.how {
            How::Expression { with_values, .. } => s.push_str(&format!(": {with_values}")),
            How::Whole(what) if !what.is_empty() => s.push_str(&format!(", made from the whole map ({what})")),
            How::Window { radius, .. } => s.push_str(&format!(", from the cells within {radius} around it")),
            How::Patch { .. } => s.push_str(&format!(", {}", first.how.patch_text())),
            How::Allocation { lines, .. } if !lines.is_empty() => s.push_str(&format!(" ({})", lines[0])),
            How::Branch { .. } => s.push_str(&format!(", from {}", first.how.branch_text())),
            _ => {}
        }
        if !from.is_empty() {
            s.push_str(&format!(", where {}", from.join("; ")));
        }
        s.push('.');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resolve_step_tells_the_memberships_the_winner_and_the_doubt() {
        let n = Node::new("res", "objects.resolve").param("min", 0.1);
        let at: BTreeMap<String, At> =
            [("memberships".to_string(), At::Layers(vec![("Grass".into(), 0.64), ("Trees".into(), 0.82)]))].into();
        let How::Whole(text) = resolve_how(&n, &at).expect("memberships known") else { panic!("a sentence") };
        assert!(text.contains("Grass 0.64, Trees 0.82"), "{text}");
        assert!(text.contains("Trees 0.82 won over Grass 0.64 (conflict"), "{text}");
        // (no memberships at the cell: the step is explained as any whole-map operator is)
        assert!(resolve_how(&n, &BTreeMap::new()).is_none());
    }

    #[test]
    fn expressions_take_the_values_at_the_cell() {
        let v: BTreeMap<String, At> = [
            ("t0".to_string(), At::Number(1.0)),
            ("suit".to_string(), At::Number(0.71)),
            ("s".to_string(), At::Layers(vec![("forest".into(), 3.0)])),
        ]
        .into();
        assert_eq!(
            substituted("2 if t0 == 1 and nbsum(t0 == 2, 1) >= 2 and suit > 0.45 else t0", &v),
            "2 if 1 == 1 and nbsum(1 == 2, 1) >= 2 and 0.71 > 0.45 else 1"
        );
        assert_eq!(substituted("s.forest * 2 + other", &v), "3 * 2 + other");
        assert_eq!(steps_text(&[1.0, 1.0, 2.0, 2.0]), "1 at step 1, 2 from step 3, (4 steps)");
        assert_eq!(steps_text(&[3.0, 3.0]), "3 at every step (2)");
    }

    #[test]
    fn a_cell_is_traced_back_through_the_graph() {
        let r = Registry::standard();
        let m = crate::templates::TEMPLATES.iter().find(|t| t.name == "CLUE-S").unwrap().model();
        let res = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
        let values = |n: &str, p: &str| res.values.get(n)?.get(p).cloned();
        let maps = values("simulation", "maps").unwrap();
        let grid = grid_of(&maps).unwrap();
        let t = trace(&m, &r, &values, &Kept::new(), "simulation", "maps", &grid, (40, 60));
        let first = &t.steps[0];
        assert_eq!((first.node.as_str(), first.how.clone()), ("simulation", How::Loop));
        assert!(matches!(&first.value, Some(At::Steps(s)) if s.len() == 3), "{:?}", first.value);
        // map algebra: its expression with the values at the cell
        let calc = t.steps.iter().find(|s| s.node == "land_2010").expect("land_2010 traced");
        let How::Expression { with_values, .. } = &calc.how else { panic!("{:?}", calc.how) };
        assert!(with_values.contains(" if ") && with_values.contains(" else "), "{with_values}");
        assert!(!with_values.contains("suit"), "the names replaced: {with_values}");
        // a model's operator from the whole map; the data at the end
        let fit = t.steps.iter().find(|s| s.node == "regression").expect("the regression traced");
        assert!(matches!(&fit.how, How::Whole(_)));
        let data = t.steps.iter().find(|s| s.node == "land_2000").expect("the data traced");
        assert_eq!(data.how, How::Data);
        assert!(matches!(data.value, Some(At::Number(_))));
        // each node once, the text and sentence tell them
        let ids: HashSet<&str> = t.steps.iter().map(|s| s.node.as_str()).collect();
        assert_eq!(ids.len(), t.steps.len());
        assert!(t.text().contains("with the values here: 2 if "));
        assert!(t.sentence().starts_with("Here simulation.maps is "), "{}", t.sentence());
        assert!(!t.sentence().contains("a value of type"), "{}", t.sentence());
        // into the loop: a run that kept its body's values at the cell, step by step
        let kept = keep_loops(&m, std::path::Path::new(""), t.x, t.y).unwrap();
        let allocate = kept.get(&("simulation".to_string(), "allocate.landscape".to_string())).expect("kept");
        assert_eq!(allocate.len(), 3);
        let Some(At::Steps(maps)) = &t.steps[0].value else { panic!() };
        assert_eq!(allocate, maps, "the same values as the series collected");
        let deep = trace(&m, &r, &values, &kept, "simulation", "maps", &grid, (40, 60));
        assert!(deep.steps[0].body.iter().any(|(what, s)| what == "allocate.landscape" && s.len() == 3));
        assert!(deep.text().contains("in the loop, allocate.landscape"));
    }

    #[test]
    fn groups_are_traced_through() {
        let r = Registry::standard();
        let mut m = crate::templates::TEMPLATES.iter().find(|t| t.name == "CLUE-S").unwrap().model();
        crate::document::group_by_stage(&mut m, &r).unwrap();
        let res = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
        let values = |n: &str, p: &str| res.values.get(n)?.get(p).cloned();
        let grid = grid_of(&values("model", "simulation_maps").unwrap()).unwrap();
        let t = trace(&m, &r, &values, &Kept::new(), "model", "simulation_maps", &grid, (10, 10));
        assert_eq!(t.steps[0].node, "simulation", "the group's output: the node of its body giving it");
        assert_eq!(t.steps[0].path, ["model"]);
        let calc = t.steps.iter().find(|s| s.node == "land_2010").unwrap();
        assert!(calc.value.is_some(), "a group's body's values kept by the run");
    }

    /// A model of the format, from the rest of its TOML.
    fn model(text: &str) -> Model {
        Model::from_toml(&format!("format = \"terraflux/1\"\nname = \"t\"\n{text}")).unwrap()
    }

    #[test]
    fn a_switch_is_traced_into_the_branch_that_ran() {
        let r = Registry::standard();
        let m = crate::templates::TEMPLATES.iter().find(|t| t.name.starts_with("Choose the method")).unwrap().model();
        for (scenario, ran, not) in
            [("by distance", "distance", "neighbourhood"), ("by neighbourhood", "neighbourhood", "distance")]
        {
            let m = m.with_scenario(scenario).unwrap();
            let res =
                terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
            let values = |n: &str, p: &str| res.values.get(n)?.get(p).cloned();
            let grid = grid_of(&values("combined", "map").unwrap()).unwrap();
            let t = trace(&m, &r, &values, &Kept::new(), "combined", "map", &grid, (30, 40));
            // the model's input of a value: its value
            let combined = &t.steps[0];
            let w = combined.inputs.iter().find(|i| i.port == "w").unwrap();
            assert_eq!(w.value, Some(At::Number(0.8)));
            let sw = t.steps.iter().find(|s| s.node == "suitability").expect("the switch traced");
            assert_eq!(sw.how, How::Branch { case: ran.into(), branch: Some(ran.into()), steps: Vec::new() });
            assert!(sw.how.branch_text().contains(&format!("the branch {ran} ran")), "{}", sw.how.branch_text());
            // into the branch: the node of its body giving the port, with its value kept by the run
            let into = sw.inputs.iter().find(|i| i.port == format!("(branch {ran}) map")).expect("the branch");
            let inner = &t.steps[into.step.expect("traced")];
            assert_eq!(inner.path, ["suitability", ran]);
            assert_eq!(inner.value, sw.value, "what the switch gave is what its branch gave");
            assert!(matches!(inner.value, Some(At::Number(v)) if v.is_finite()));
            // and on through the branch's inputs (`@f`, the switch's `@farm`) to the node outside
            assert!(t.steps.iter().any(|s| s.node == "farmland" && s.path.is_empty()), "{}", t.text());
            assert!(!t.steps.iter().any(|s| s.path.iter().any(|p| p == not)), "the other branch did not run");
            assert!(t.text().contains(&format!("the branch {ran} ran")));
        }
    }

    #[test]
    fn a_switch_in_a_loop_is_traced_into_the_branch_of_each_step() {
        let r = Registry::standard();
        let m = model(
            r#"
[[nodes]]
id = "land"
op = "nlm.fbm"
params = { rows = 6, columns = 6, dimension = 1.4, seed = 3, resolution = 30 }

[[nodes]]
id = "loop"
op = "control.repeat"
inputs = { times = 5, m = "land.map" }
[nodes.body]
carry = { m = "sw.map" }
collect = { maps = "sw.map" }
[[nodes.body.nodes]]
id = "sw"
op = "control.switch"
inputs = { case = "@step", m = "@m" }
[[nodes.body.nodes.body.nodes]]
id = "case_2"
op = "control.group"
inputs = { x = "@m" }
[nodes.body.nodes.body.nodes.body]
outputs = { map = "up.map" }
[[nodes.body.nodes.body.nodes.body.nodes]]
id = "up"
op = "raster.calc"
inputs = { x = "@x" }
params = { expression = "x + 10" }
[[nodes.body.nodes.body.nodes]]
id = "otherwise"
op = "control.group"
inputs = { x = "@m" }
[nodes.body.nodes.body.nodes.body]
outputs = { map = "down.map" }
[[nodes.body.nodes.body.nodes.body.nodes]]
id = "down"
op = "raster.calc"
inputs = { x = "@x" }
params = { expression = "x - 1" }
"#,
        );
        let res = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
        let values = |n: &str, p: &str| res.values.get(n)?.get(p).cloned();
        let grid = grid_of(&values("loop", "maps").unwrap()).unwrap();
        let (x, y) = centre(&grid, 2, 3);
        let kept = keep_loops(&m, std::path::Path::new(""), x, y).unwrap();
        let steps: Vec<String> =
            ["otherwise", "case_2", "otherwise", "otherwise", "otherwise"].map(String::from).into();
        assert_eq!(kept.branches.get(&("loop".to_string(), "sw.branch".to_string())), Some(&steps));
        // the switch of the loop's body, traced
        let t = trace(&m, &r, &values, &kept, "loop", "sw.map", &grid, (2, 3));
        let sw = &t.steps[0];
        assert_eq!((sw.node.as_str(), sw.path.as_slice()), ("sw", ["loop".to_string()].as_slice()));
        let How::Branch { steps: told, branch, .. } = &sw.how else { panic!("{:?}", sw.how) };
        assert_eq!((told, branch.as_deref()), (&steps, Some("otherwise")));
        assert!(
            sw.how.branch_text().contains("otherwise at step 1, case_2 at step 2, otherwise at steps 3 to 5"),
            "{}",
            sw.how.branch_text()
        );
        // each branch traced, with the steps it ran at (no value at the others)
        let series = |id: &str| -> Vec<f64> {
            let s = t.steps.iter().find(|s| s.node == id).unwrap_or_else(|| panic!("{id}: {}", t.text()));
            let Some(At::Steps(v)) = &s.value else { panic!("{:?}", s.value) };
            v.clone()
        };
        let (up, down) = (series("up"), series("down"));
        let Some(At::Steps(all)) = &sw.value else { panic!() };
        assert_eq!((up.len(), down.len(), all.len()), (5, 5, 5));
        assert!(up[1] == all[1] && [0, 2, 3, 4].iter().all(|&k| up[k].is_nan()), "{up:?}");
        assert!(down[1].is_nan() && [0, 2, 3, 4].iter().all(|&k| down[k] == all[k]), "{down:?}");
        assert!((all[1] - all[0] - 10.0).abs() < 1e-9, "{all:?}");
        // without the run that kept them: the branch is not known
        let flat = trace(&m, &r, &values, &Kept::new(), "loop", "sw.map", &grid, (2, 3));
        assert_eq!(flat.steps[0].how, How::Branch { case: "@step".into(), branch: None, steps: Vec::new() });
    }

    #[test]
    fn a_switch_in_a_group_is_traced_through() {
        let r = Registry::standard();
        let m = model(
            r#"
[inputs.method]
kind = "value"
value = "double"

[[nodes]]
id = "land"
op = "nlm.fbm"
params = { rows = 6, columns = 6, dimension = 1.4, seed = 3, resolution = 30 }

[[nodes]]
id = "g"
op = "control.group"
inputs = { land = "land.map" }
[nodes.body]
outputs = { map = "sw.map" }
[[nodes.body.nodes]]
id = "sw"
op = "control.switch"
inputs = { case = "$method", map = "@land" }
[[nodes.body.nodes.body.nodes]]
id = "double"
op = "control.group"
inputs = { x = "@map" }
[nodes.body.nodes.body.nodes.body]
outputs = { map = "d.map" }
[[nodes.body.nodes.body.nodes.body.nodes]]
id = "d"
op = "raster.calc"
inputs = { x = "@x" }
params = { expression = "x * 2" }
[[nodes.body.nodes.body.nodes]]
id = "otherwise"
op = "control.group"
inputs = { x = "@map" }
[nodes.body.nodes.body.nodes.body]
outputs = { map = "o.map" }
[[nodes.body.nodes.body.nodes.body.nodes]]
id = "o"
op = "raster.calc"
inputs = { x = "@x" }
params = { expression = "x" }
"#,
        );
        let res = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
        let values = |n: &str, p: &str| res.values.get(n)?.get(p).cloned();
        let grid = grid_of(&values("g", "map").unwrap()).unwrap();
        let t = trace(&m, &r, &values, &Kept::new(), "g", "map", &grid, (1, 4));
        let sw = &t.steps[0];
        assert_eq!((sw.node.as_str(), sw.path.as_slice()), ("sw", ["g".to_string()].as_slice()));
        assert_eq!(sw.how, How::Branch { case: "double".into(), branch: Some("double".into()), steps: Vec::new() });
        let d = t.steps.iter().find(|s| s.node == "d").expect("the branch's node");
        assert_eq!(d.path, ["g", "sw", "double"]);
        let land = t.steps.iter().find(|s| s.node == "land").expect("through @x, @map and @land to the node outside");
        assert_eq!(land.how, How::Data);
        let (Some(At::Number(dv)), Some(At::Number(lv))) = (&d.value, &land.value) else { panic!("{}", t.text()) };
        assert!((dv - 2.0 * lv).abs() < 1e-12, "{dv} {lv}");
        assert!(!t.steps.iter().any(|s| s.node == "o"));
    }

    #[test]
    fn a_node_of_a_group_or_a_switch_branch_can_be_frozen() {
        use terraflux_graph::Body;
        let mut m = Model::new("t");
        m.nodes.push(
            Node::new("g", GROUP).body(Body { nodes: vec![Node::new("in", "raster.calc")], ..Default::default() }),
        );
        m.nodes.push(Node::new("sw", terraflux_graph::SWITCH).body(Body {
            nodes: vec![Node::new("double", GROUP).body(Body {
                nodes: vec![Node::new("d", "raster.calc")],
                ..Default::default()
            })],
            ..Default::default()
        }));
        m.nodes.push(
            Node::new("each", EACH).body(Body { nodes: vec![Node::new("e", "raster.calc")], ..Default::default() }),
        );
        let p = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(in_groups(&m, &[]) && in_groups(&m, &p(&["g"])));
        assert!(in_groups(&m, &p(&["sw"])) && in_groups(&m, &p(&["sw", "double"])));
        assert!(!in_groups(&m, &p(&["each"])), "a loop's body gives a value per step");
        // the branch itself is not freezable, a node inside it is (as a group's)
        assert!(can_freeze(&m, &[]) && can_freeze(&m, &p(&["g"])) && can_freeze(&m, &p(&["sw", "double"])));
        assert!(!can_freeze(&m, &p(&["sw"])) && !can_freeze(&m, &p(&["each"])));
    }

    #[test]
    fn a_patch_is_traced_whole() {
        let r = Registry::standard();
        // two patches of class 1 on a ground of 0: an L of 4 cells (joined by sides) and one cell alone, touching
        // the L by a corner
        let mut m = Model::new("m");
        m.inputs.insert(
            "x".into(),
            terraflux_graph::Input::Map { path: "x.tif".into(), band: None, nodata: None, note: None, data: None },
        );
        m.nodes.push(Node::new("classes", "raster.calc").param("expression", "x").input("x", "$x"));
        m.nodes.push(Node::new("patches", "raster.label").input("map", "classes.map"));
        let grid = std::sync::Arc::new(terraflux_raster::Grid::new(4, 4));
        #[rustfmt::skip]
        let cells = vec![
            1.0, 0.0, 0.0, 0.0,
            1.0, 0.0, 0.0, 0.0,
            1.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
        ];
        let x = Value::Map(terraflux_ops::Map::new(grid.clone(), cells));
        let inputs = [("x".to_string(), x)].into();
        let res = terraflux_exec::Engine::default().run(&m, &inputs, &mut terraflux_exec::Quiet).unwrap();
        let values = |n: &str, p: &str| res.values.get(n)?.get(p).cloned();
        let t = trace(&m, &r, &values, &Kept::new(), "patches", "map", &grid, (0, 0));
        let How::Patch { port, class, cells, more, neighbours, rows, cols, members } = t.steps[0].how.clone() else {
            panic!("{:?}", t.steps[0].how)
        };
        assert_eq!((port.as_str(), class, cells, more), ("map", 1.0, 4, false));
        assert_eq!(members.len(), 4);
        // drawn: the sides of its cells on its border (an L of 4 cells: 10 sides), within its rows and columns
        let o = patch_outline(&t.steps[0].how).unwrap();
        assert_eq!(o.edges.len(), 10);
        assert_eq!(o.bounds, (0.0, 0.0, 2.0, 3.0));
        assert_eq!((neighbours, rows, cols), (4, (0, 2), (0, 1)), "raster.label joins by 4 neighbours by default");
        assert!(t.sentence().contains("from the patch of map it is in: class 1, 4 cells"), "{}", t.sentence());
        // by 8 neighbours the cell alone joins the L
        m.nodes[1].params.insert("neighbours".into(), 8.into());
        let res = terraflux_exec::Engine::default().run(&m, &inputs, &mut terraflux_exec::Quiet).unwrap();
        let values = |n: &str, p: &str| res.values.get(n)?.get(p).cloned();
        let t = trace(&m, &r, &values, &Kept::new(), "patches", "map", &grid, (3, 2));
        assert!(matches!(t.steps[0].how, How::Patch { cells: 5, neighbours: 8, .. }), "{:?}", t.steps[0].how);
    }

    #[test]
    fn an_allocation_tells_the_demand_of_its_steps() {
        let r = Registry::standard();
        let m = crate::templates::TEMPLATES.iter().find(|t| t.name == "CLUE-S").unwrap().model();
        let res = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
        let values = |n: &str, p: &str| res.values.get(n)?.get(p).cloned();
        let Some(Value::Series(maps)) = values("simulation", "maps") else { panic!() };
        let (Value::Map(first), Value::Map(last)) = (&maps[0], &maps[maps.len() - 1]) else { panic!() };
        // a cell that changed class along the steps
        let i = (0..first.data.len()).find(|&i| first.data[i] != last.data[i] && last.data[i].is_finite()).unwrap();
        let w = first.grid.width;
        let grid = first.grid.clone();
        let (x, y) = centre(&grid, i / w, i % w);
        let kept = keep_loops(&m, std::path::Path::new(""), x, y).unwrap();
        let t = trace(&m, &r, &values, &kept, "simulation", "maps", &grid, (i / w, i % w));
        let How::Allocation { what, lines } = &t.steps[0].how else { panic!("{:?}", t.steps[0].how) };
        assert!(what.contains("allocate (lulcc.step)"), "{what}");
        assert!(lines.iter().any(|l| l.starts_with("step ") && l.contains("the step's demand")), "{lines:?}");
        // why the cell: its suitability for each class there, the class it took ranked among the cells
        let why = lines.iter().find(|l| l.starts_with("why ")).unwrap_or_else(|| panic!("{lines:?}"));
        assert!(why.contains("its suitability here, by class") && why.contains("% of the cells"), "{why}");
        assert!(why.contains("iteration variable"), "{why}");
        assert!(t.text().contains("the step's demand, in cells of each class"));
    }
}
