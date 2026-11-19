//! `objects.temporal` and `objects.transition_matrix`: the classes of two dates bound by a fuzzy transition matrix
//! (Feitosa et al. 2009; InterIMAGE, Costa et al. 2010).

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::{ObjectClass, Objects, objects_in};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Temporal));
    r.add(Arc::new(TransitionMatrix));
}

/// The name shown for no class in the change table.
const NONE: &str = "(none)";

/// A fuzzy transition matrix: the class names of the origins and destinations and the possibilities (row: origin).
struct Matrix {
    from: Vec<String>,
    to: Vec<String>,
    m: Vec<Vec<f64>>,
}

impl Matrix {
    fn of(node: &str, t: &Table) -> Result<Matrix> {
        let bad = |s: String| Error::msg(format!("node {node:?}: matrix: {s}"));
        if t.columns.first().map(String::as_str) != Some("class") || !t.is_text("class") {
            return Err(bad("the first column is `class`, the names of the classes of origin".into()));
        }
        let to: Vec<String> = t.columns[1..].to_vec();
        let mut from = Vec::new();
        let mut m = Vec::new();
        for r in &t.rows {
            let name = t.label("class", r[0]).ok_or_else(|| bad("a row without a class".into()))?.to_string();
            if from.contains(&name) {
                return Err(bad(format!("the class {name:?} has two rows")));
            }
            if let Some(v) = r[1..].iter().find(|v| !(0.0..=1.0).contains(*v)) {
                return Err(bad(format!("the possibilities are from 0 to 1, not {v} (row {name:?})")));
            }
            from.push(name);
            m.push(r[1..].to_vec());
        }
        Ok(Matrix { from, to, m })
    }

    /// The matrix as the table [`Matrix::of`] reads.
    fn table(names: &[String], m: &[Vec<f64>]) -> Table {
        let mut t = Table::new(std::iter::once("class".to_string()).chain(names.iter().cloned()).collect());
        t.labels.insert("class".into(), names.to_vec());
        t.rows = m
            .iter()
            .enumerate()
            .map(|(k, r)| std::iter::once((k + 1) as f64).chain(r.iter().copied()).collect())
            .collect();
        t
    }
}

/// The index of each of `wanted` among the matrix's `names` (an error listing the ones it lacks).
fn find_all(node: &str, what: &str, wanted: &[ObjectClass], names: &[String]) -> Result<Vec<usize>> {
    let mut at = Vec::new();
    let mut missing = Vec::new();
    for c in wanted {
        match names.iter().position(|n| *n == c.name) {
            Some(k) => at.push(k),
            None => missing.push(c.name.clone()),
        }
    }
    if !missing.is_empty() {
        return Err(Error::msg(format!(
            "node {node:?}: the matrix has no {what} for the classes {} (it has: {})",
            missing.join(", "),
            names.join(", ")
        )));
    }
    Ok(at)
}

/// The memberships of an object set: those it holds, or crisp ones (1 for its class) when it has none.
fn memberships(node: &str, which: &str, o: &Objects) -> Result<Vec<Vec<f64>>> {
    if o.classes.is_empty() {
        return Err(Error::msg(format!("node {node:?}: {which} has no classes (classify the objects first)")));
    }
    if !o.membership.is_empty() {
        return Ok(o.membership.clone());
    }
    Ok((0..o.classes.len())
        .map(|k| o.class_of.iter().map(|&c| if c as usize == k + 1 { 1.0 } else { 0.0 }).collect())
        .collect())
}

/// For each object of `a`, the cell-weighted mean of the memberships `mem` (class, object of `b`) of the objects of
/// `b` its cells fall in (the cells with no object of `b` do not count); None when none of its cells has one.
fn link(a: &Objects, b: &Objects, mem: &[Vec<f64>]) -> Vec<Option<Vec<f64>>> {
    let nk = mem.len();
    let mut sum = vec![0.0f64; a.count * nk];
    let mut w = vec![0u32; a.count];
    for (&la, &lb) in a.labels.iter().zip(b.labels.iter()) {
        if la == 0 || lb == 0 {
            continue;
        }
        let (j, i) = (la as usize - 1, lb as usize - 1);
        w[j] += 1;
        for k in 0..nk {
            sum[j * nk + k] += mem[k][i];
        }
    }
    (0..a.count).map(|j| (w[j] > 0).then(|| (0..nk).map(|k| sum[j * nk + k] / w[j] as f64).collect())).collect()
}

/// Max-min composition: for each destination `c`, min(own[c], max over k of min(other[k], p(k, c))) where `p` reads
/// the possibility of the pair; None (not linked) leaves the memberships as they are.
fn adjust(own: &[Vec<f64>], other: &[Option<Vec<f64>>], p: impl Fn(usize, usize) -> f64) -> Vec<Vec<f64>> {
    let mut out = own.to_vec();
    for (c, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            if let Some(o) = &other[j] {
                let reach = (0..o.len()).map(|k| o[k].min(p(k, c))).fold(0.0, f64::max);
                *v = v.min(reach);
            }
        }
    }
    out
}

/// The class of each object: the highest membership, 0 below `min` (or at 0).
fn classes_of(mem: &[Vec<f64>], count: usize, min: f64) -> Vec<u32> {
    (0..count)
        .map(|j| {
            let mut best = (0u32, 0.0f64);
            for (c, m) in mem.iter().enumerate() {
                if m[j] > best.1 {
                    best = (c as u32 + 1, m[j]);
                }
            }
            if best.1 < min { 0 } else { best.0 }
        })
        .collect()
}

struct Temporal;

impl Operator for Temporal {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.temporal",
            "The classes of two dates bound by a fuzzy transition matrix (Feitosa et al. 2009, as InterIMAGE's \
             multi-temporal analysis, Costa et al. 2010): a transition the matrix calls impossible cannot be what \
             the objects of the second date are, whatever their spectra say. The first date's memberships are \
             linked to each object of the second (the mean over its cells of the memberships of the objects of the \
             first date they fall in; the segmentations may differ) and each membership of the second date becomes \
             min(membership, max over the classes of origin of min(membership before, possibility of the \
             transition)), the max-min composition. With `both`, the first date is adjusted the same way from the \
             second. A class is the highest adjusted membership, below `min` none. The classes are matched by name \
             with the matrix. The change table (the before and after class of each object of the second date) \
             enters the land-change models (change.transitions, the Markov and CA models) as the observed dates. \
             The matrix is estimated by objects.transition_matrix or written by hand with table.of (a first \
             column `class` of text, then a column per destination).",
            Access::Global,
        )
        .input(PortSpec::new("before", Type::Objects, "the objects of the first date, with classes and memberships"))
        .input(PortSpec::new(
            "after",
            Type::Objects,
            "the objects of the second date, on the same grid, with classes and memberships",
        ))
        .input(PortSpec::new(
            "matrix",
            Type::Table,
            "the possibility (0 to 1) of each transition: a row per class of origin, the column `class` (text) then \
             a column per destination class",
        ))
        .output(PortSpec::new("objects", Type::Objects, "the second date, classified with the adjusted memberships"))
        .output(PortSpec::new("before", Type::Objects, "the first date: adjusted with `both`, else as given"))
        .output(PortSpec::new("change", Type::Table, "from, to, objects, cells, area: the transitions found"))
        .output(PortSpec::new("map", Type::Map, "the class number of the second date in each cell"))
        .param(PortSpec::optional("both", Type::Bool, "also adjust the first date from the second (default false)"))
        .param(PortSpec::optional(
            "min",
            Type::Number,
            "the lowest adjusted membership that gives a class, below it the object has none (default 0.1)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let before = objects_in(args, "before")?;
        let after = objects_in(args, "after")?;
        let Some(Value::Table(mt)) = args.get("matrix") else {
            return Err(Error::msg(format!("node {node:?}: connect the table matrix")));
        };
        let both: bool = args.param("both", false)?;
        let min: f64 = args.param("min", 0.1)?;
        if !(0.0..=1.0).contains(&min) {
            return Err(Error::msg(format!("node {node:?}: min {min}: 0 to 1")));
        }
        if before.grid.width != after.grid.width || before.grid.height != after.grid.height {
            return Err(Error::msg(format!(
                "node {node:?}: the dates are on grids of {}x{} and {}x{} cells",
                before.grid.height, before.grid.width, after.grid.height, after.grid.width
            )));
        }
        let mx = Matrix::of(node, mt)?;
        let rows = find_all(node, "row (origin)", &before.classes, &mx.from)?;
        let cols = find_all(node, "column (destination)", &after.classes, &mx.to)?;
        // the possibility of the class k of the first date becoming the class c of the second
        let poss = |k: usize, c: usize| mx.m[rows[k]][cols[c]];
        let mb = memberships(node, "before", before)?;
        let ma = memberships(node, "after", after)?;
        let adj_after = adjust(&ma, &link(after, before, &mb), poss);
        let class_after = classes_of(&adj_after, after.count, min);
        let new_after = Objects::clone(after).with_classes(after.classes.clone(), class_after, adj_after)?;
        let new_before = if both { adj_before(before, &mb, &ma, after, &poss, min)? } else { (**before).clone() };
        // the change table: the class of the first date of an object of the second is the majority of its cells
        let nb = new_before.classes.len();
        let mut votes = vec![0u32; after.count * (nb + 1)];
        for (&la, &lb) in after.labels.iter().zip(new_before.labels.iter()) {
            if la != 0 && lb != 0 {
                votes[(la as usize - 1) * (nb + 1) + new_before.class_of[lb as usize - 1] as usize] += 1;
            }
        }
        let mut counts: BTreeMap<(u32, u32), (u32, u64)> = BTreeMap::new();
        for j in 0..after.count {
            let v = &votes[j * (nb + 1)..(j + 1) * (nb + 1)];
            // the most voted class (a class before none), the lowest on a tie
            let mut from = 0u32;
            let mut top = 0;
            for (k, &n) in v.iter().enumerate().skip(1) {
                if n > top {
                    (from, top) = (k as u32, n);
                }
            }
            let e = counts.entry((from, new_after.class_of[j])).or_default();
            e.0 += 1;
            e.1 += after.cells[j] as u64;
        }
        let mut from_names: Vec<String> = before.classes.iter().map(|c| c.name.clone()).collect();
        from_names.push(NONE.into());
        let mut to_names: Vec<String> = after.classes.iter().map(|c| c.name.clone()).collect();
        to_names.push(NONE.into());
        let code = |k: u32, n: usize| if k == 0 { (n + 1) as f64 } else { k as f64 };
        let area = after.cell_area();
        let mut change = Table::new(["from", "to", "objects", "cells", "area"].map(String::from).to_vec());
        change.labels.insert("from".into(), from_names);
        change.labels.insert("to".into(), to_names);
        for (&(f, t), &(n, cells)) in &counts {
            change.rows.push(vec![
                code(f, nb),
                code(t, after.classes.len()),
                n as f64,
                cells as f64,
                cells as f64 * area,
            ]);
        }
        let map: Vec<f64> = after
            .labels
            .iter()
            .map(|&l| match l {
                0 => f64::NAN,
                l => match new_after.class_of[l as usize - 1] {
                    0 => f64::NAN,
                    k => k as f64,
                },
            })
            .collect();
        let map = Map::new(after.grid.clone(), map);
        Ok(outputs([
            ("objects", Value::Objects(Arc::new(new_after))),
            ("before", Value::Objects(Arc::new(new_before))),
            ("change", Value::Table(change)),
            ("map", Value::Map(map)),
        ]))
    }
}

/// The first date adjusted from the second: μ_before(k) = min(μ_before(k), max_c min(μ_after(c), M[k][c])), the
/// memberships of the second date linked to each object of the first.
fn adj_before(
    before: &Objects,
    mb: &[Vec<f64>],
    ma: &[Vec<f64>],
    after: &Objects,
    poss: &impl Fn(usize, usize) -> f64,
    min: f64,
) -> Result<Objects> {
    let linked = link(before, after, ma);
    let mut adj = mb.to_vec();
    for (k, row) in adj.iter_mut().enumerate() {
        for (i, v) in row.iter_mut().enumerate() {
            if let Some(o) = &linked[i] {
                let reach = (0..o.len()).map(|c| o[c].min(poss(k, c))).fold(0.0, f64::max);
                *v = v.min(reach);
            }
        }
    }
    let class_of = classes_of(&adj, before.count, min);
    Objects::clone(before).with_classes(before.classes.clone(), class_of, adj)
}

struct TransitionMatrix;

/// The possibilities from counts: each row over its largest count (the most frequent transition of an origin is 1),
/// the unseen ones `floor`; an origin never seen is all 1 (nothing is known against any transition).
fn from_counts(counts: &[Vec<f64>], floor: f64) -> Vec<Vec<f64>> {
    counts
        .iter()
        .map(|r| {
            let top = r.iter().copied().fold(0.0, f64::max);
            if top <= 0.0 {
                return vec![1.0; r.len()];
            }
            r.iter().map(|&c| if c > 0.0 { c / top } else { floor }).collect()
        })
        .collect()
}

impl Operator for TransitionMatrix {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.transition_matrix",
            "Estimate the fuzzy transition matrix of objects.temporal (Feitosa et al. 2009; InterIMAGE, Costa et al. \
             2010) from what was seen: `samples`, a table with the class of each sample at the two dates, or \
             `transitions`, the table of change.transitions. A row per class of origin: its count of each \
             transition over the count of its most frequent one, so that one is 1 and a transition never seen is \
             0 (or `floor`); an origin never seen is all 1. (InterIMAGE searches the matrix with a genetic \
             algorithm; this one reads it straight from the samples.) To write the matrix by hand use table.of, \
             with the columns `class` (text) and a column per class, and rows of the class of origin then its \
             possibilities.",
            Access::None,
        )
        .input(PortSpec::optional(
            "samples",
            Type::Table,
            "a row per sample with the class at the first date and at the second (text)",
        ))
        .input(PortSpec::optional(
            "transitions",
            Type::Table,
            "the table of change.transitions: from, to and the cells (a count of each pair)",
        ))
        .output(PortSpec::new("matrix", Type::Table, "the possibilities: `class`, then a column per class"))
        .param(PortSpec::optional("from_column", Type::Text, "the column of the first date's class (default before)"))
        .param(PortSpec::optional("to_column", Type::Text, "the column of the second date's class (default after)"))
        .param(PortSpec::optional("floor", Type::Number, "the possibility of a transition never seen (default 0)"))
        .param(PortSpec::optional(
            "column",
            Type::Text,
            "with `transitions`, the column that counts each pair (default cells; km2 works too)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |s: String| Error::msg(format!("node {node:?}: {s}"));
        let floor: f64 = args.param("floor", 0.0)?;
        if !(0.0..=1.0).contains(&floor) {
            return Err(bad(format!("floor {floor}: 0 to 1")));
        }
        let table = |port: &str| match args.get(port) {
            None => Ok(None),
            Some(Value::Table(t)) => Ok(Some(t)),
            Some(v) => Err(bad(format!("{port} is a {:?}, not a table", v.ty()))),
        };
        // the pairs (origin, destination, count) with the names in order of first appearance
        let mut names: Vec<String> = Vec::new();
        let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
        let mut at = |n: String| match names.iter().position(|x| *x == n) {
            Some(k) => k,
            None => {
                names.push(n);
                names.len() - 1
            }
        };
        match (table("samples")?, table("transitions")?) {
            (Some(_), Some(_)) => return Err(bad("give samples or transitions, not both".into())),
            (None, None) => return Err(bad("connect samples or transitions".into())),
            (Some(t), None) => {
                let fc: String = args.param("from_column", "before".to_string())?;
                let tc: String = args.param("to_column", "after".to_string())?;
                let col = |c: &str| {
                    t.columns.iter().position(|x| x == c).ok_or_else(|| {
                        bad(format!("no column {c:?} in the samples (it has: {})", t.columns.join(", ")))
                    })
                };
                let (fj, tj) = (col(&fc)?, col(&tc)?);
                for r in &t.rows {
                    if r[fj].is_nan() || r[tj].is_nan() {
                        continue;
                    }
                    let (f, d) = (t.cell(fj, r[fj]), t.cell(tj, r[tj]));
                    pairs.push((at(f), at(d), 1.0));
                }
            }
            (None, Some(t)) => {
                let cc: String = args.param("column", "cells".to_string())?;
                let col = |c: &str| {
                    t.columns.iter().position(|x| x == c).ok_or_else(|| {
                        bad(format!("no column {c:?} in the transitions (it has: {})", t.columns.join(", ")))
                    })
                };
                let (fj, tj, nj) = (col("from")?, col("to")?, col(&cc)?);
                for r in &t.rows {
                    if r[nj].is_nan() || r[nj] < 0.0 {
                        continue;
                    }
                    let (f, d) = (t.cell(fj, r[fj]), t.cell(tj, r[tj]));
                    pairs.push((at(f), at(d), r[nj]));
                }
            }
        }
        if pairs.is_empty() {
            return Err(bad("no transitions to count".into()));
        }
        let n = names.len();
        let mut counts = vec![vec![0.0; n]; n];
        for (f, d, c) in pairs {
            counts[f][d] += c;
        }
        let m = from_counts(&counts, floor);
        Ok(outputs([("matrix", Value::Table(Matrix::table(&names, &m)))]))
    }
}
