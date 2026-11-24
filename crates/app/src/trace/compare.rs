//! Two traces compared: why this cell and not that one (IDEAS, the compared Trace). The nodes of the two traces
//! aligned by their place, id and port, each the same at both, different, or traced in one only; and where the
//! difference begins: a node whose values differ while every input it reads has the same values at both (or that
//! reads no node), with why, from how it made its value: the data differ there, the cells around, the whole map, the
//! patches, the place itself, a switch's branch, an input that is no node's. Deterministic, as the trace is: the
//! assistant only puts it in the words of the field.

use std::collections::HashMap;

use super::{At, How, MOST, Step, Trace};

/// How a node traced stands at the two cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The same value at both.
    Same,
    /// Another value.
    Different,
    /// Traced at A only (a branch of a switch that ran there alone).
    OnlyA,
    /// Traced at B only.
    OnlyB,
}

/// A node of the traces, aligned by its place, id and port: its step in each trace.
#[derive(Debug, Clone, PartialEq)]
pub struct Pair {
    pub path: Vec<String>,
    pub node: String,
    pub port: String,
    pub op: String,
    pub a: Option<usize>,
    pub b: Option<usize>,
    pub kind: Kind,
}

/// Why the values of a node differ where the difference begins.
#[derive(Debug, Clone, PartialEq)]
pub enum Why {
    /// It reads no node: the data (read or drawn) have other values at the two cells.
    Data,
    /// The same inputs at the cell, other cells around: the windows of its inputs at each.
    Window { radius: u32, a: Vec<(String, Vec<Vec<f64>>)>, b: Vec<(String, Vec<Vec<f64>>)> },
    /// Made from the whole map (a fit, an allocation): what the operator does, and the lines of each (an
    /// allocation's demand and the cell's place among the cells).
    Whole { what: String, a: Vec<String>, b: Vec<String> },
    /// Other patches: each in words.
    Patch { a: String, b: String },
    /// The same inputs, another value: it depends on where the cell is (a position, a draw at random): its
    /// operator.
    Place(String),
    /// A switch that ran other branches.
    Branch { a: String, b: String },
    /// An input that is no node's (a model's input, a literal) with other values.
    Input { port: String, a: String, b: String },
    /// Not known from here: the trace stops, or an input has no value kept.
    Unknown(String),
}

/// Where the difference begins: a pair of [`Comparison::pairs`] and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Root {
    pub pair: usize,
    pub why: Why,
}

/// Two traces of the same node compared.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// What the two sides are called ("A" and "B", or two scenarios).
    pub names: [String; 2],
    pub a: Trace,
    pub b: Trace,
    /// Every node traced, in the order of A's trace, then those of B's only.
    pub pairs: Vec<Pair>,
    /// Where the difference begins.
    pub roots: Vec<Root>,
    /// The step where the two series part (from 0), when what is compared is a loop's series that differs: where the
    /// difference begins is looked for at that step (until then everything was the same, what the step before
    /// carried too).
    pub step: Option<usize>,
}

/// The step where two series part (from 0).
fn parting(a: &Option<At>, b: &Option<At>) -> Option<usize> {
    let (Some(At::Steps(p)), Some(At::Steps(q))) = (a, b) else { return None };
    (0..p.len().min(q.len())).find(|&k| !same_number(p[k], q[k]))
}

/// Two values the same at a step: a series at that step, the others whole.
fn same_at(a: &Option<At>, b: &Option<At>, step: Option<usize>) -> bool {
    match (step, a, b) {
        (Some(s), Some(At::Steps(p)), Some(At::Steps(q))) if s < p.len() && s < q.len() => same_number(p[s], q[s]),
        _ => same(a, b),
    }
}

/// A value in words at a step: a series' value there ("1 at step 7"), the others whole.
fn told_at(v: &Option<At>, step: Option<usize>) -> String {
    match (step, v) {
        (Some(s), Some(At::Steps(p))) if s < p.len() => format!("{} at step {}", crate::report::cell(p[s]), s + 1),
        _ => told(v),
    }
}

/// Two numbers the same: the same bits a calculation gives (NaN as NaN).
fn same_number(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

/// Two values at a cell the same: numbers, layers by layer, series step by step, the rest by its words.
pub fn same(a: &Option<At>, b: &Option<At>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(At::Number(p)), Some(At::Number(q))) => same_number(*p, *q),
        (Some(At::Layers(p)), Some(At::Layers(q))) => {
            p.len() == q.len() && p.iter().zip(q).all(|((k, v), (l, w))| k == l && same_number(*v, *w))
        }
        (Some(At::Steps(p)), Some(At::Steps(q))) => {
            p.len() == q.len() && p.iter().zip(q).all(|(v, w)| same_number(*v, *w))
        }
        (Some(At::Other(p)), Some(At::Other(q))) => p == q,
        _ => false,
    }
}

/// Whether a step is data: it reads no node (data read or drawn, a map algebra of a model's input), nor what a
/// container gives its body (`@m`, a loop's carried value).
fn reads_no_node(s: &Step) -> bool {
    s.how == How::Data || s.inputs.iter().all(|i| !i.node && !i.from.starts_with('@'))
}

/// A value in words, or that none was kept.
fn told(v: &Option<At>) -> String {
    v.as_ref().map_or("no value kept".to_string(), At::text)
}

/// Two traces compared, A and B.
pub fn compare(a: &Trace, b: &Trace) -> Comparison {
    compare_named(a, b, ["A", "B"])
}

/// Two traces compared, the sides by their names (two scenarios).
pub fn compare_named(a: &Trace, b: &Trace, names: [&str; 2]) -> Comparison {
    let key = |s: &Step| (s.path.clone(), s.node.clone(), s.port.clone());
    let in_b: HashMap<_, usize> = b.steps.iter().enumerate().map(|(k, s)| (key(s), k)).collect();
    let mut pairs: Vec<Pair> = Vec::new();
    let (mut of_a, mut of_b) = (vec![None; a.steps.len()], vec![None; b.steps.len()]);
    let pair = |s: &Step, a: Option<usize>, b: Option<usize>, kind: Kind| Pair {
        path: s.path.clone(),
        node: s.node.clone(),
        port: s.port.clone(),
        op: s.op.clone(),
        a,
        b,
        kind,
    };
    for (i, s) in a.steps.iter().enumerate() {
        let j = in_b.get(&key(s)).copied();
        let kind = match j {
            Some(j) if same(&s.value, &b.steps[j].value) => Kind::Same,
            Some(_) => Kind::Different,
            None => Kind::OnlyA,
        };
        of_a[i] = Some(pairs.len());
        if let Some(j) = j {
            of_b[j] = Some(pairs.len());
        }
        pairs.push(pair(s, Some(i), j, kind));
    }
    for (j, s) in b.steps.iter().enumerate() {
        if of_b[j].is_none() {
            of_b[j] = Some(pairs.len());
            pairs.push(pair(s, None, Some(j), Kind::OnlyB));
        }
    }
    // (series that part at a step: each node compared at that step)
    let step = parting(&a.steps.first().and_then(|s| s.value.clone()), &b.steps.first().and_then(|s| s.value.clone()));
    let at_step: Vec<Kind> = pairs
        .iter()
        .map(|p| match (p.a, p.b) {
            (Some(i), Some(j)) if same_at(&a.steps[i].value, &b.steps[j].value, step) => Kind::Same,
            (Some(_), Some(_)) => Kind::Different,
            _ => p.kind,
        })
        .collect();
    let mut roots = Vec::new();
    for (k, p) in pairs.iter().enumerate() {
        let (Some(i), Some(j)) = (p.a, p.b) else { continue };
        let (sa, sb) = (&a.steps[i], &b.steps[j]);
        // a switch that ran other branches: the difference begins there, whatever its inputs (in a loop, at the
        // step the series part)
        if let (How::Branch { branch: ba, steps: ta, .. }, How::Branch { branch: bb, steps: tb, .. }) =
            (&sa.how, &sb.how)
        {
            let apart = match step {
                Some(s) if !ta.is_empty() && !tb.is_empty() => ta.get(s) != tb.get(s),
                _ => ba != bb || ta != tb,
            };
            if apart {
                let said = |b: &Option<String>, t: &[String]| match (step.and_then(|s| t.get(s)), t.is_empty()) {
                    (Some(at), _) => format!("{at} at step {}", step.unwrap_or_default() + 1),
                    (None, true) => b.clone().unwrap_or_else(|| "not known".into()),
                    (None, false) => super::branches_text(t),
                };
                roots.push(Root { pair: k, why: Why::Branch { a: said(ba, ta), b: said(bb, tb) } });
                continue;
            }
        }
        if at_step[k] != Kind::Different {
            continue;
        }
        // every input traced the same at both: the difference begins here
        let traced = sa
            .inputs
            .iter()
            .filter_map(|x| x.step.and_then(|s| of_a[s]))
            .chain(sb.inputs.iter().filter_map(|x| x.step.and_then(|s| of_b[s])));
        if traced.map(|q| at_step[q]).any(|k| k != Kind::Same) {
            continue;
        }
        roots.push(Root { pair: k, why: why(sa, sb, step) });
    }
    Comparison { names: names.map(String::from), a: a.clone(), b: b.clone(), pairs, roots, step }
}

/// Why a node's values differ (at the step the series part) when the inputs traced are the same at both.
fn why(sa: &Step, sb: &Step, step: Option<usize>) -> Why {
    let untraced: Vec<_> = sa
        .inputs
        .iter()
        .filter(|x| x.step.is_none())
        .map(|x| (x, sb.inputs.iter().find(|y| y.port == x.port)))
        .collect();
    // an input that is no node's with other values: a model's input, a literal (another scenario)
    for (x, y) in &untraced {
        if let Some(y) = y
            && x.value.is_some()
            && y.value.is_some()
            && !same_at(&x.value, &y.value, step)
        {
            return Why::Input { port: x.port.clone(), a: told_at(&x.value, step), b: told_at(&y.value, step) };
        }
    }
    // a node the trace did not reach
    if let Some((x, _)) = untraced.iter().find(|(x, _)| x.node) {
        return Why::Unknown(format!("the trace stops before {} (its input {}), at {MOST} nodes", x.from, x.port));
    }
    if reads_no_node(sa) {
        return Why::Data;
    }
    // an input without a value kept at the cell (a loop's carried value): it may be what differs
    if let Some((x, _)) = untraced.iter().find(|(x, y)| x.value.is_none() || y.is_none_or(|y| y.value.is_none())) {
        return Why::Unknown(format!("its input {} ({}) has no value kept here", x.port, x.from));
    }
    match (&sa.how, &sb.how) {
        (How::Window { radius, windows: a }, How::Window { windows: b, .. }) => {
            Why::Window { radius: *radius, a: a.clone(), b: b.clone() }
        }
        (How::Whole(what), _) => Why::Whole { what: what.clone(), a: Vec::new(), b: Vec::new() },
        (How::Allocation { what, lines: a }, How::Allocation { lines: b, .. }) => {
            Why::Whole { what: what.clone(), a: a.clone(), b: b.clone() }
        }
        (How::Patch { .. }, How::Patch { .. }) => Why::Patch { a: sa.how.patch_text(), b: sb.how.patch_text() },
        (How::Loop, _) => {
            Why::Unknown("decided inside the loop: Trace inside the loops follows the difference into its body".into())
        }
        (How::Expression { .. } | How::Local, _) => Why::Place(sa.op.clone()),
        _ => Why::Unknown(format!("{} gives other values from the same inputs", sa.op)),
    }
}

/// The rows of a window in words ("1 2 3 | 4 5 6 | 7 8 9").
fn rows(w: &[Vec<f64>]) -> String {
    w.iter()
        .map(|r| r.iter().map(|v| crate::report::cell(*v)).collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join(" | ")
}

impl Why {
    /// Why in words, the sides by their names.
    pub fn text(&self, names: &[String; 2]) -> String {
        let [a_, b_] = names;
        match self {
            Why::Data => "it reads no node: the data have other values at the two cells".into(),
            Why::Window { radius, .. } => {
                format!("the same inputs at the cell, but the cells within {radius} around differ")
            }
            Why::Whole { what, .. } => format!("decided by the whole map: {what}"),
            Why::Patch { a, b } => format!("other patches: at {a_} {a}; at {b_} {b}"),
            Why::Place(op) => format!(
                "the same inputs, another value: {op} gives what depends on where the cell is (a position, a draw at \
                 random)"
            ),
            Why::Branch { a, b } => format!("the switch ran another branch: at {a_} {a}, at {b_} {b}"),
            Why::Input { port, a, b } => format!("its input {port} differs: {a} at {a_}, {b} at {b_}"),
            Why::Unknown(what) => format!("not known from here: {what}"),
        }
    }

    /// What shows it, line by line: the windows of each side, an allocation's lines.
    pub fn details(&self, names: &[String; 2]) -> Vec<String> {
        let [a_, b_] = names;
        match self {
            Why::Window { a, b, .. } => {
                let mut out = Vec::new();
                for (port, w) in a {
                    out.push(format!("{port} around {a_}: [{}]", rows(w)));
                    if let Some((_, v)) = b.iter().find(|(p, _)| p == port) {
                        out.push(format!("{port} around {b_}: [{}]", rows(v)));
                    }
                }
                out
            }
            Why::Whole { a, b, .. } => {
                a.iter().map(|l| format!("{a_}: {l}")).chain(b.iter().map(|l| format!("{b_}: {l}"))).collect()
            }
            _ => Vec::new(),
        }
    }
}

impl Comparison {
    /// Whether anything differs between the two traces.
    pub fn differs(&self) -> bool {
        self.pairs.iter().any(|p| p.kind != Kind::Same) || !self.roots.is_empty()
    }

    /// A pair's step at A and at B.
    pub fn steps(&self, p: &Pair) -> (Option<&Step>, Option<&Step>) {
        (p.a.map(|i| &self.a.steps[i]), p.b.map(|j| &self.b.steps[j]))
    }

    /// The value of a pair at each side, in words.
    fn values(&self, p: &Pair) -> (String, String) {
        let (sa, sb) = self.steps(p);
        (told(&sa.and_then(|s| s.value.clone())), told(&sb.and_then(|s| s.value.clone())))
    }

    /// The value of a pair at each side at the step the series part (a series' value there), in words.
    pub fn values_at_step(&self, p: &Pair) -> (String, String) {
        let (sa, sb) = self.steps(p);
        (told_at(&sa.and_then(|s| s.value.clone()), self.step), told_at(&sb.and_then(|s| s.value.clone()), self.step))
    }

    /// A side's place in words ("A (row 40, column 12) = 2").
    fn side(&self, k: usize) -> String {
        let t = if k == 0 { &self.a } else { &self.b };
        let v = told(&t.steps.first().and_then(|s| s.value.clone()));
        format!("{} (row {}, column {}) = {v}", self.names[k], t.cell.0, t.cell.1)
    }

    /// The answer in a sentence: the two values, the data both read alike, and where the difference begins.
    pub fn sentence(&self) -> String {
        let mut s = format!("{} and {}.", self.side(0), self.side(1));
        if !self.differs() {
            s.push_str(&format!(
                " Nothing differs: the {} nodes traced give the same values at both.",
                self.pairs.len()
            ));
            return s;
        }
        let mut parts = Vec::new();
        if self.pairs.first().is_some_and(|p| p.kind == Kind::Same) {
            parts.push("the same value, made in other ways".to_string());
        }
        if let Some(s) = self.step {
            parts.push(if s == 0 {
                "the series differ from the first step".to_string()
            } else {
                format!("the series are the same up to step {s} and part at step {}", s + 1)
            });
        }
        let alike: Vec<String> = self
            .pairs
            .iter()
            .filter(|p| {
                p.kind == Kind::Same
                    && self.steps(p).0.is_some_and(|s| {
                        reads_no_node(s) && matches!(s.value, Some(At::Number(_) | At::Layers(_) | At::Steps(_)))
                    })
            })
            .take(3)
            .map(|p| format!("{} ({})", p.node, self.values(p).0))
            .collect();
        if !alike.is_empty() {
            parts.push(format!("they read the same {}", alike.join(", ")));
        }
        let roots: Vec<String> = self
            .roots
            .iter()
            .take(4)
            .map(|r| {
                let p = &self.pairs[r.pair];
                match &r.why {
                    Why::Branch { a, b } => format!("{} (branch {a} against {b})", p.node),
                    _ => {
                        let (a, b) = self.values_at_step(p);
                        format!("{} ({a} against {b})", p.node)
                    }
                }
            })
            .collect();
        if roots.is_empty() {
            parts.push(
                "where the difference begins is not known from here (the trace stops, or a loop's values were not \
                 kept)"
                    .into(),
            );
        } else {
            let more = if self.roots.len() > 4 { format!(" and {} more", self.roots.len() - 4) } else { String::new() };
            parts.push(format!("they differ from {}{more}", roots.join(", ")));
        }
        let mut body = parts.join("; ");
        if let Some(c) = body.get(..1) {
            body = c.to_uppercase() + &body[1..];
        }
        s.push_str(&format!(" {body}."));
        s
    }

    /// The comparison in lines of words: the two cells, where the difference begins and why, then every node
    /// traced (for the Copy, an assistant, the tool trace_cell).
    pub fn text(&self) -> String {
        let [a_, b_] = &self.names;
        let mut out = String::new();
        for (name, t) in [(a_, &self.a), (b_, &self.b)] {
            out.push_str(&format!(
                "{name}: the cell at row {}, column {} (x {}, y {}).\n",
                t.cell.0,
                t.cell.1,
                crate::report::cell(t.x),
                crate::report::cell(t.y)
            ));
        }
        let at = |p: &Pair| if p.path.is_empty() { String::new() } else { format!(" (in {})", p.path.join("/")) };
        if !self.differs() {
            out.push_str("Nothing differs.\n");
        } else if self.roots.is_empty() {
            out.push_str("Where the difference begins is not known from here.\n");
        } else {
            if let Some(s) = self.step {
                out.push_str(&format!("The series part at step {}: the nodes compared at that step.\n", s + 1));
            }
            out.push_str("Where the difference begins:\n");
            for r in &self.roots {
                let p = &self.pairs[r.pair];
                let (va, vb) = self.values_at_step(p);
                out.push_str(&format!(
                    "- {}.{} [{}]{}: {a_} {va}, {b_} {vb}: {}\n",
                    p.node,
                    p.port,
                    p.op,
                    at(p),
                    r.why.text(&self.names)
                ));
                for l in r.why.details(&self.names) {
                    out.push_str(&format!("  {l}\n"));
                }
                if let (Some(sa), Some(sb)) = self.steps(p)
                    && let (How::Expression { with_values: ea, .. }, How::Expression { with_values: eb, .. }) =
                        (&sa.how, &sb.how)
                {
                    out.push_str(&format!("  {a_} here: {ea}\n  {b_} here: {eb}\n"));
                }
            }
        }
        out.push_str("Every node traced:\n");
        for (k, p) in self.pairs.iter().enumerate() {
            let (va, vb) = self.values(p);
            let how = match p.kind {
                Kind::Same => format!("the same, {va}"),
                Kind::Different => format!("{a_} {va}, {b_} {vb}"),
                Kind::OnlyA => format!("only at {a_}: {va}"),
                Kind::OnlyB => format!("only at {b_}: {vb}"),
            };
            out.push_str(&format!("{}. {}.{} [{}]{}: {how}\n", k + 1, p.node, p.port, p.op, at(p)));
        }
        if self.a.cut || self.b.cut {
            out.push_str(&format!("(a trace stops at {MOST} nodes)\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use terraflux_graph::{Model, Node};
    use terraflux_ops::{Registry, Value};

    use super::*;
    use crate::trace::{Input, Kept, grid_of, trace};

    /// A model reading the map `x` (4 x 4 cells, given), run: its values.
    fn run(m: &Model, cells: Vec<f64>) -> (terraflux_exec::Results, std::sync::Arc<terraflux_raster::Grid>) {
        let grid = std::sync::Arc::new(terraflux_raster::Grid::new(4, 4));
        let x = Value::Map(terraflux_ops::Map::new(grid.clone(), cells));
        let inputs = [("x".to_string(), x)].into();
        (terraflux_exec::Engine::default().run(m, &inputs, &mut terraflux_exec::Quiet).unwrap(), grid)
    }

    fn reading_x(nodes: Vec<Node>) -> Model {
        let mut m = Model::new("m");
        m.inputs.insert(
            "x".into(),
            terraflux_graph::Input::Map { path: "x.tif".into(), band: None, nodata: None, note: None, data: None },
        );
        m.nodes.push(Node::new("classes", "raster.calc").param("expression", "x").input("x", "$x"));
        m.nodes.extend(nodes);
        m
    }

    /// The comparison of two cells of a node's map.
    fn compared(
        m: &Model,
        cells: Vec<f64>,
        node: &str,
        port: &str,
        a: (usize, usize),
        b: (usize, usize),
    ) -> Comparison {
        let r = Registry::standard();
        let (res, grid) = run(m, cells);
        let values = |n: &str, p: &str| res.values.get(n)?.get(p).cloned();
        let ta = trace(m, &r, &values, &Kept::new(), node, port, &grid, a);
        let tb = trace(m, &r, &values, &Kept::new(), node, port, &grid, b);
        compare(&ta, &tb)
    }

    /// The root's node and why.
    fn roots(c: &Comparison) -> Vec<(&str, &Why)> {
        c.roots.iter().map(|r| (c.pairs[r.pair].node.as_str(), &r.why)).collect()
    }

    #[rustfmt::skip]
    const CELLS: [f64; 16] = [
        1.0, 0.0, 0.0, 1.0,
        1.0, 0.0, 0.0, 0.0,
        1.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
    ];

    #[test]
    fn the_difference_begins_where_the_data_differ() {
        let m =
            reading_x(vec![Node::new("double", "raster.calc").param("expression", "c * 2").input("c", "classes.map")]);
        let c = compared(&m, CELLS.to_vec(), "double", "map", (0, 0), (0, 1));
        assert_eq!(roots(&c), [("classes", &Why::Data)]);
        assert!(c.pairs.iter().all(|p| p.kind == Kind::Different), "{:?}", c.pairs);
        let s = c.sentence();
        assert_eq!(s, "A (row 0, column 0) = 2 and B (row 0, column 1) = 0. They differ from classes (1 against 0).");
        let text = c.text();
        assert!(text.contains("- classes.map [raster.calc]: A 1, B 0: it reads no node"), "{text}");
        assert!(text.contains("1. double.map [raster.calc]: A 2, B 0\n"), "{text}");
    }

    #[test]
    fn the_same_cell_twice_differs_nowhere() {
        let m =
            reading_x(vec![Node::new("double", "raster.calc").param("expression", "c * 2").input("c", "classes.map")]);
        let c = compared(&m, CELLS.to_vec(), "double", "map", (2, 1), (2, 1));
        assert!(!c.differs() && c.roots.is_empty());
        assert!(
            c.sentence().ends_with("Nothing differs: the 2 nodes traced give the same values at both."),
            "{}",
            c.sentence()
        );
        // two cells alike: nothing differs either
        let c = compared(&m, CELLS.to_vec(), "double", "map", (0, 1), (1, 2));
        assert!(!c.differs(), "{}", c.text());
    }

    #[test]
    fn the_difference_begins_in_the_cells_around() {
        let m = reading_x(vec![
            Node::new("share", "dynamic.share_of").input("landscape", "classes.map").param("classes", 1),
        ]);
        // two cells of class 0 with other neighbours
        let c = compared(&m, CELLS.to_vec(), "share", "share", (1, 1), (1, 2));
        let [(node, Why::Window { radius, a, b })] = roots(&c)[..] else { panic!("{}", c.text()) };
        assert_eq!((node, *radius), ("share", 1));
        assert_ne!(a, b, "the windows differ");
        assert!(c.pairs.iter().any(|p| p.node == "classes" && p.kind == Kind::Same));
        assert!(c.sentence().contains("They read the same classes (0); they differ from share ("), "{}", c.sentence());
        assert!(c.text().contains("landscape around A: ["), "{}", c.text());
    }

    #[test]
    fn the_difference_begins_in_the_patches() {
        let m = reading_x(vec![Node::new("patches", "raster.label").input("map", "classes.map")]);
        // two cells of class 1: one in the L of four cells, the other alone (a corner away, four neighbours)
        let c = compared(&m, CELLS.to_vec(), "patches", "map", (0, 0), (3, 2));
        let [(node, Why::Patch { a, b })] = roots(&c)[..] else { panic!("{}", c.text()) };
        assert_eq!(node, "patches");
        assert!(a.contains("4 cells") && b.contains("1 cells"), "{a} / {b}");
    }

    #[test]
    fn two_cells_of_a_land_change_model_differ_from_a_node_whose_inputs_are_alike() {
        let r = Registry::standard();
        let m = crate::templates::TEMPLATES.iter().find(|t| t.name == "CLUE-S").unwrap().model();
        let res = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
        let values = |n: &str, p: &str| res.values.get(n)?.get(p).cloned();
        let grid = grid_of(&values("simulation", "maps").unwrap()).unwrap();
        let ta = trace(&m, &r, &values, &Kept::new(), "simulation", "maps", &grid, (40, 60));
        // a cell whose series differs
        let tb = (0..grid.height)
            .flat_map(|row| (0..grid.width).map(move |col| (row, col)))
            .map(|cell| trace(&m, &r, &values, &Kept::new(), "simulation", "maps", &grid, cell))
            .find(|t| !same(&t.steps[0].value, &ta.steps[0].value))
            .expect("another series");
        let c = compare(&ta, &tb);
        assert!(!c.sentence().contains("random generator"), "only the maps read alike: {}", c.sentence());
        assert!(!c.roots.is_empty(), "{}", c.text());
        for root in &c.roots {
            let p = &c.pairs[root.pair];
            assert_eq!(p.kind, Kind::Different);
            let (sa, _) = c.steps(p);
            for i in sa.unwrap().inputs.iter().filter_map(|i| i.step) {
                let q = c.pairs.iter().find(|q| q.a == Some(i)).unwrap();
                assert_eq!(q.kind, Kind::Same, "{}: {} differs", p.node, q.node);
            }
        }
        assert!(c.text().contains("Where the difference begins:\n- "), "{}", c.text());
        assert!(c.sentence().contains("they differ from") || c.sentence().contains("They differ from"));
    }

    #[test]
    fn two_cells_of_a_loop_part_at_a_step_and_the_difference_is_looked_for_there() {
        let r = Registry::standard();
        let m = crate::templates::TEMPLATES.iter().find(|t| t.name == "CLUE-S").unwrap().model();
        let res = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
        let values = |n: &str, p: &str| res.values.get(n)?.get(p).cloned();
        let maps = values("simulation", "maps").unwrap();
        let grid = grid_of(&maps).unwrap();
        let series = |(row, col): (usize, usize)| -> Vec<f64> {
            let (x, y) = crate::trace::centre(&grid, row, col);
            let Some(At::Steps(s)) = crate::trace::at(&maps, x, y) else { panic!("a series") };
            s
        };
        let cells: Vec<(usize, usize)> =
            (0..grid.height).flat_map(|row| (0..grid.width).map(move |col| (row, col))).collect();
        // A, a cell that changes; B, the same at the first step, not after; C, the same series as A
        let a = *cells.iter().find(|&&c| series(c).windows(2).any(|w| w[0] != w[1])).expect("a cell that changes");
        let sa = series(a);
        let b = *cells.iter().find(|&&c| series(c)[0] == sa[0] && series(c) != sa).expect("B");
        let c_ = *cells.iter().find(|&&c| c != a && series(c) == sa).expect("C");
        let place = |(row, col)| crate::trace::centre(&grid, row, col);
        let kept = crate::trace::keep_loops_at(&m, std::path::Path::new(""), &[place(a), place(b), place(c_)]).unwrap();
        let traced = |cell, k: usize| trace(&m, &r, &values, &kept[k], "simulation", "maps", &grid, cell);
        let (ta, tb, tc) = (traced(a, 0), traced(b, 1), traced(c_, 2));
        // into the loop's body: its allocation, with what each step began from
        let allocate = ta.steps.iter().find(|s| s.node == "allocate").expect("the body traced");
        assert_eq!(allocate.path, ["simulation"]);
        assert!(matches!(allocate.how, How::Allocation { .. }), "{:?}", allocate.how);
        let start = allocate.inputs.iter().find(|i| i.port == "landscape").unwrap();
        assert!(matches!(&start.value, Some(At::Steps(v)) if v.len() == sa.len()), "{:?}", start.value);
        let c = compare(&ta, &tb);
        let s = c.step.expect("they part");
        assert!(s > 0, "the same at the first step");
        assert!(c.sentence().contains(&format!("part at step {}", s + 1)), "{}", c.sentence());
        assert!(!c.roots.is_empty(), "{}", c.text());
        // each root differs at that step, the inputs it reads the same there
        for root in &c.roots {
            let p = &c.pairs[root.pair];
            let (sa, sb) = c.steps(p);
            let (sa, sb) = (sa.unwrap(), sb.unwrap());
            assert!(!same_at(&sa.value, &sb.value, Some(s)), "{}", p.node);
            for i in sa.inputs.iter().filter_map(|i| i.step) {
                let q = c.pairs.iter().find(|q| q.a == Some(i)).unwrap();
                let (qa, qb) = c.steps(q);
                assert!(same_at(&qa.unwrap().value, &qb.unwrap().value, Some(s)), "{}: {} differs", p.node, q.node);
            }
        }
        // two cells whose series never part
        let c = compare(&ta, &tc);
        assert_eq!(c.step, None);
        assert!(!c.sentence().contains("part at step"), "{}", c.sentence());
    }

    /// A step made by hand: a node at the top level, its value and inputs.
    fn step(node: &str, op: &str, value: f64, how: How, inputs: Vec<Input>) -> Step {
        Step {
            path: Vec::new(),
            node: node.into(),
            op: op.into(),
            port: "map".into(),
            note: None,
            value: Some(At::Number(value)),
            how,
            inputs,
            body: Vec::new(),
        }
    }

    fn input(port: &str, from: &str, value: Option<f64>, step: Option<usize>, node: bool) -> Input {
        Input { port: port.into(), from: from.into(), value: value.map(At::Number), step, node }
    }

    fn traced(steps: Vec<Step>, cut: bool) -> Trace {
        Trace { x: 0.0, y: 0.0, cell: (0, 0), steps, cut }
    }

    #[test]
    fn a_root_says_the_place_an_input_or_what_is_not_known() {
        let data = |v| step("d", "nlm.fbm", v, How::Data, Vec::new());
        let calc = |v, rain: Option<f64>| {
            step(
                "c",
                "raster.calc",
                v,
                How::Expression { text: "d + rain".into(), with_values: String::new() },
                vec![input("d", "d.map", Some(1.0), Some(1), true), input("rain", "$rain", rain, None, false)],
            )
        };
        // the same inputs, another value: the place
        let c = compare(
            &traced(vec![calc(3.0, Some(2.0)), data(1.0)], false),
            &traced(vec![calc(4.0, Some(2.0)), data(1.0)], false),
        );
        assert_eq!(roots(&c), [("c", &Why::Place("raster.calc".into()))]);
        // an input of the model with another value
        let c = compare(
            &traced(vec![calc(3.0, Some(2.0)), data(1.0)], false),
            &traced(vec![calc(4.0, Some(3.0)), data(1.0)], false),
        );
        assert_eq!(roots(&c), [("c", &Why::Input { port: "rain".into(), a: "2".into(), b: "3".into() })]);
        assert!(c.text().contains("its input rain differs: 2 at A, 3 at B"), "{}", c.text());
        // no value kept of an input: not known
        let c =
            compare(&traced(vec![calc(3.0, None), data(1.0)], false), &traced(vec![calc(4.0, None), data(1.0)], false));
        assert!(matches!(roots(&c)[..], [("c", Why::Unknown(w))] if w.contains("rain")), "{:?}", c.roots);
        // the trace cut before a node
        let cut = |v| step("c", "raster.calc", v, How::Local, vec![input("d", "d.map", Some(1.0), None, true)]);
        let c = compare(&traced(vec![cut(3.0)], true), &traced(vec![cut(4.0)], true));
        assert!(matches!(roots(&c)[..], [("c", Why::Unknown(w))] if w.contains("stops before d.map")), "{:?}", c.roots);
        // a node in one trace only: told so
        let c = compare(&traced(vec![data(1.0)], false), &traced(vec![calc(4.0, Some(2.0)), data(1.0)], false));
        assert_eq!(c.pairs.iter().map(|p| p.kind).collect::<Vec<_>>(), [Kind::Same, Kind::OnlyB]);
    }

    #[test]
    fn a_switch_that_ran_other_branches_is_where_the_difference_begins() {
        let sw = |b: &str, v| {
            step(
                "sw",
                "control.switch",
                v,
                How::Branch { case: b.into(), branch: Some(b.into()), steps: Vec::new() },
                vec![input(&format!("(branch {b}) map"), &format!("{b}: x.map"), Some(v), Some(1), true)],
            )
        };
        let inner = |b: &str, v| Step {
            path: vec!["sw".into(), b.into()],
            ..step("x", "raster.calc", v, How::Local, Vec::new())
        };
        let c = compare(
            &traced(vec![sw("dry", 1.0), inner("dry", 1.0)], false),
            &traced(vec![sw("wet", 2.0), inner("wet", 2.0)], false),
        );
        assert_eq!(roots(&c), [("sw", &Why::Branch { a: "dry".into(), b: "wet".into() })]);
        assert_eq!(c.pairs.iter().map(|p| p.kind).collect::<Vec<_>>(), [Kind::Different, Kind::OnlyA, Kind::OnlyB]);
        assert!(c.sentence().contains("They differ from sw (branch dry against wet)."), "{}", c.sentence());
    }
}
