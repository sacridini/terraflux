//! The trace of a cell: the chain of values that made what a node gives at a place (IDEAS, phase 7). From the node
//! shown, back through the graph: at every node, the value its output has at the cell and the values its inputs
//! have there, with how the operator made one of the others: map algebra's expression with the values in place of
//! its names, the window of cells a neighbourhood's operator reads, the layers a model's operator weighs (a class's
//! suitability), a loop's value step by step. The values are those of the last run (a top-level node's, a group's
//! body's: a group keeps them); inside a loop, those of a run that kept them ([`Kept`]). Built from the model and a
//! way to read the values: the Trace pane shows it, the assistant explains it (its words, [`Trace::text`]), the tool
//! `trace_cell` gives it to an assistant outside.

use std::collections::{BTreeMap, HashSet};

use terraflux_graph::{Access, GROUP, Model, Node, Ref, Source};
use terraflux_ops::{Registry, Value};

use crate::document::words;
use crate::report::cell;

/// Nodes traced at most (a graph of thousands traced to its sources would say nothing).
const MOST: usize = 60;
/// The side of the window of cells shown of a neighbourhood's input, at most (an odd number).
const WINDOW: usize = 5;

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
    /// A loop: the value at every step (its body's nodes traced when a run kept them).
    Loop,
    /// A group: what its body gave.
    Group,
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

/// The values of a run inside loops, at the cell: by the loop's path and the body node's `id.port`, a value per
/// step ([`crate::trace`]'s run that keeps them).
pub type Kept = BTreeMap<(String, String), Vec<f64>>;

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

/// Where a reference read at a scope points: the node (its path and id) and port, looked for in the scope, then
/// in those around it (a body reads the nodes around it); a group's output, at the node of its body giving it.
fn resolve(m: &Model, path: &[String], reference: &str) -> Option<(Vec<String>, String, String)> {
    let Ok(Ref::Port { node, port }) = Ref::parse(reference) else { return None };
    let mut scope: Vec<String> = path.to_vec();
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
/// node's, or the top-level group's (`inner.port`, through groups); none in a loop.
pub fn key(m: &Model, path: &[String], node: &str, port: &str) -> Option<(String, String)> {
    if path.is_empty() {
        return Some((node.to_string(), port.to_string()));
    }
    // every container on the way a group
    for k in 0..path.len() {
        let n = m.scope(&path[..k])?.iter().find(|n| n.id == path[k])?;
        if n.op != GROUP {
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
    kept.get(&(path.join("/"), format!("{node}.{port}"))).map(|s| At::Steps(s.clone()))
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
                None => (rf.clone(), None, None),
            },
        };
        if let Some(v) = &value {
            at_inputs.insert(p.clone(), v.clone());
        }
        inputs.push(Input { port: p.clone(), from, value, step: traced });
    }
    // what a container's body reads around it: traced as its inputs are (a loop's allocation reads the
    // suitability made before it)
    if let Some(b) = &n.body {
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
    let how = if n.op == "control.repeat" {
        How::Loop
    } else if n.op == GROUP {
        How::Group
    } else if n.op == "raster.calc" {
        let text = n.params.get("expression").map(words).unwrap_or_default();
        How::Expression { with_values: substituted(&text, &at_inputs), text }
    } else if !reads_nodes {
        How::Data
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
        for ((loop_path, what), s) in kept {
            if *loop_path == at {
                body.push((what.clone(), s.clone()));
            }
        }
    }
    let step = &mut t.steps[k];
    step.inputs = inputs;
    step.how = how;
    step.body = body;
    Some(k)
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
                let Some(x) = self::at(v, self.x, self.y).and_then(|a| a.number()) else { continue };
                let s = self.kept.entry((at.to_string(), format!("{node}.{port}"))).or_default();
                // (a step missed, as a loop's first steps before a node of it gave a map: no value)
                s.resize(step - 1, f64::NAN);
                s.push(x);
            }
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
                How::Loop => out.push_str("   a loop: its value at every step above\n"),
                How::Group => out.push_str("   a group: what its body gave\n"),
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
}
