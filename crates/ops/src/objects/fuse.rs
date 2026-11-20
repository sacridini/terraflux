//! `objects.fuse`: eCognition's image object fusion. An object (a seed) joins a neighbour (a candidate) only when the
//! object they make is good by a measure: an `objects.calc` expression over the joined object (`compactness`,
//! `mean_nir`, `sd_red`), better than the seed's own, and/or beyond a threshold. The merging by condition
//! (`objects.merge`) looks at the two objects; this looks at the result.
//!
//! The joined object's attributes are not computed again from its cells. Each group keeps what adds up: its cells,
//! its perimeter in cell edges (the sum of the two less twice the border they share, from the neighbour lists), its
//! box, and per band the count, the sum and the sum of squares (of the values less the band's mean, as
//! `stats_table`), so the cells, area, perimeter, length and width (the sides of the box, an approximation of the
//! rotated rectangle `objects.features` uses), compactness, shape_index, border_index, and the means and deviations
//! of the bands come in constant time. What cannot be derived this way (texture, skeleton, context, anything over
//! the neighbours) is refused with the list of what can.
//!
//! A pass: the expression is evaluated in batch over a table with a row per pair of neighbours (and, for `improve`,
//! over the seeds alone); each seed proposes its best candidate (`mode` best) or all the candidates that qualify
//! (`mode` all); the proposals are taken best first, a pair only when neither of its objects has joined in the pass
//! (disjoint pairs, as the merging of `objects.merge`); the groups' neighbour lists are updated; and again, until
//! nothing joins or `max_passes`. Every step runs in the order of the ids, so the result is the same on any number of
//! threads (the batch evaluation and the band sums run in parallel and give the same numbers).

use std::collections::BTreeMap;
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::Column;

use super::levels::{class_numbers, regroup};
use super::{Objects, band_names, big, find, objects_in, stats_table};
use crate::algebra::{Expr, Script, Stmt, parse_script_objects};
use crate::vector_algebra::{Frame, run_frame, script_name};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Fuse));
}

/// What a joined object's attribute can be, derived from the sums the groups keep.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Feat {
    Cells,
    Area,
    Perimeter,
    Length,
    Width,
    LengthWidth,
    Compactness,
    ShapeIndex,
    BorderIndex,
    Mean(usize),
    Sd(usize),
}

const SHAPES: [(&str, Feat); 9] = [
    ("cells", Feat::Cells),
    ("area", Feat::Area),
    ("perimeter", Feat::Perimeter),
    ("length", Feat::Length),
    ("width", Feat::Width),
    ("length_width", Feat::LengthWidth),
    ("compactness", Feat::Compactness),
    ("shape_index", Feat::ShapeIndex),
    ("border_index", Feat::BorderIndex),
];

/// The attribute a name in the expression stands for, if it can be derived.
fn lookup(name: &str, bands: &[String]) -> Option<Feat> {
    if let Some((_, f)) = SHAPES.iter().find(|(n, _)| *n == name) {
        return Some(*f);
    }
    let band = |p: &str| bands.iter().position(|b| format!("{p}{}", script_name(b)) == name);
    match (band("mean_"), band("sd_")) {
        (Some(k), _) => Some(Feat::Mean(k)),
        (_, Some(k)) => Some(Feat::Sd(k)),
        _ => None,
    }
}

/// The names that can be derived, for the refusal.
fn derivable(bands: &[String]) -> String {
    let mut v: Vec<String> = SHAPES.iter().map(|(n, _)| n.to_string()).collect();
    for b in bands {
        v.push(format!("mean_{}", script_name(b)));
        v.push(format!("sd_{}", script_name(b)));
    }
    if bands.is_empty() {
        v.push("mean_<band> and sd_<band> (with the input image)".into());
    }
    v.join(", ")
}

/// The names an expression reads (not those it sets itself), refusing what is not about the joined object alone.
fn names_of(script: &Script, bands: &[String]) -> Result<Vec<(String, Feat)>> {
    fn walk(e: &Expr, out: &mut Vec<String>, bad: &mut Option<String>) {
        match e {
            Expr::Num(_) | Expr::Str(_) => {}
            Expr::Var(n) => out.push(n.clone()),
            Expr::Layer(a, b) => *bad = Some(format!("{a}.{b}")),
            Expr::Unary(_, x) => walk(x, out, bad),
            Expr::Binary(_, x, y) => {
                walk(x, out, bad);
                walk(y, out, bad);
            }
            Expr::If(c, x, y) => {
                walk(c, out, bad);
                walk(x, out, bad);
                walk(y, out, bad);
            }
            Expr::Call(f, args) => {
                if f.contains('.') || matches!(f.as_str(), "border_to" | "distance_to") {
                    *bad = Some(format!("{f}()"));
                }
                args.iter().for_each(|a| walk(a, out, bad));
            }
            Expr::Method(x, _, args) => {
                walk(x, out, bad);
                args.iter().for_each(|a| walk(a, out, bad));
            }
        }
    }
    let (mut used, mut set, mut bad) = (Vec::new(), Vec::<String>::new(), None);
    for (_, s) in &script.statements {
        match s {
            Stmt::Set { name, mask, value, .. } => {
                if let Some(m) = mask {
                    walk(m, &mut used, &mut bad);
                }
                walk(value, &mut used, &mut bad);
                set.push(name.clone());
            }
            Stmt::Value(e) => walk(e, &mut used, &mut bad),
            Stmt::Out(_) => {}
        }
    }
    let list = derivable(bands);
    if let Some(b) = bad {
        return Err(Error::msg(format!(
            "the fitting uses {b}, which is not an attribute of the joined object alone; it can use: {list}"
        )));
    }
    let mut out: Vec<(String, Feat)> = Vec::new();
    for n in used {
        if set.contains(&n) || out.iter().any(|(m, _)| *m == n) {
            continue;
        }
        match lookup(&n, bands) {
            Some(f) => out.push((n, f)),
            None => {
                let why = if bands.is_empty() && (n.starts_with("mean_") || n.starts_with("sd_")) {
                    "give the input image to have the bands' means and deviations".to_string()
                } else {
                    "it cannot be derived for a joined object without computing its features again (texture, \
                     skeleton, context and the neighbours are in that case)"
                        .to_string()
                };
                return Err(Error::msg(format!("the fitting uses {n:?}: {why}; it can use: {list}")));
            }
        }
    }
    Ok(out)
}

/// A group alone, or two groups joined (the second with the edges they share with the first).
type Item = (usize, Option<(usize, u32)>);

/// The groups of objects (the index of a group is the smallest object in it) and what adds up over them.
struct State {
    live: Vec<bool>,
    cells: Vec<f64>,
    /// The perimeter in cell edges: those that face another group, no object or the image's edge.
    edges: Vec<f64>,
    bbox: Vec<[u32; 4]>,
    /// Per group and band: the count, the sum and the sum of squares of the values less the band's centre.
    bands: Vec<f64>,
    nb: usize,
    centre: Vec<f64>,
    /// The neighbours (0-based id, edges shared), sorted by id.
    adj: Vec<Vec<(u32, u32)>>,
    /// The original object whose class and memberships the group has: that of its seed.
    head: Vec<u32>,
    /// The cell's sides in map units.
    rw: f64,
    rh: f64,
}

impl State {
    fn new(o: &Objects, data: &[Arc<Vec<f64>>]) -> State {
        let n = o.count;
        let (w, h) = (o.grid.width, o.grid.height);
        let (rw, rh) = o.grid.resolution();
        // the perimeter: the edges to other objects (the neighbour lists) and to no object or the image's edge
        let rows: Vec<Vec<u32>> = (0..h)
            .into_par_iter()
            .map(|r| {
                let mut v = Vec::new();
                for c in 0..w {
                    let i = r * w + c;
                    let a = o.labels[i];
                    if a == 0 {
                        continue;
                    }
                    let sides = [
                        (c == 0, i.wrapping_sub(1)),
                        (c + 1 == w, i + 1),
                        (r == 0, i.wrapping_sub(w)),
                        (r + 1 == h, i + w),
                    ];
                    for (out, j) in sides {
                        if out || o.labels[j] == 0 {
                            v.push(a);
                        }
                    }
                }
                v
            })
            .collect();
        let mut edges = vec![0.0f64; n];
        for a in rows.iter().flatten() {
            edges[*a as usize - 1] += 1.0;
        }
        let adj: Vec<Vec<(u32, u32)>> =
            o.adjacency.iter().map(|l| l.iter().map(|&(b, len)| (b - 1, len)).collect()).collect();
        for (k, l) in adj.iter().enumerate() {
            edges[k] += l.iter().map(|&(_, len)| len as f64).sum::<f64>();
        }
        // the bands: a centre, then the sums, a band on each thread (each in the order of the cells)
        let nb = data.len();
        let per: Vec<(f64, Vec<f64>)> = data
            .par_iter()
            .map(|b| {
                let (s, c) = b.iter().filter(|v| !v.is_nan()).fold((0.0, 0usize), |(s, c), v| (s + v, c + 1));
                let centre = if c > 0 { s / c as f64 } else { 0.0 };
                let mut st = vec![0.0f64; n * 3];
                for (&l, &v) in o.labels.iter().zip(b.iter()) {
                    if l != 0 && !v.is_nan() {
                        let (d, k) = (v - centre, (l as usize - 1) * 3);
                        st[k] += 1.0;
                        st[k + 1] += d;
                        st[k + 2] += d * d;
                    }
                }
                (centre, st)
            })
            .collect();
        let mut bands = vec![0.0f64; n * nb * 3];
        for (k, (_, st)) in per.iter().enumerate() {
            for g in 0..n {
                bands[(g * nb + k) * 3..(g * nb + k) * 3 + 3].copy_from_slice(&st[g * 3..g * 3 + 3]);
            }
        }
        State {
            live: vec![true; n],
            cells: o.cells.iter().map(|&c| c as f64).collect(),
            edges,
            bbox: o.boxes.clone(),
            bands,
            nb,
            centre: per.iter().map(|p| p.0).collect(),
            adj,
            head: (0..n as u32).collect(),
            rw,
            rh,
        }
    }

    /// One attribute of a group (`b` none) or of the group joined to another (with the edges they share).
    fn feat(&self, f: Feat, a: usize, b: Option<(usize, u32)>) -> f64 {
        if let Feat::Mean(k) | Feat::Sd(k) = f {
            let add = |j: usize| {
                let at = |g: usize| self.bands[(g * self.nb + k) * 3 + j];
                at(a) + b.map_or(0.0, |(x, _)| at(x))
            };
            let n = add(0);
            if n == 0.0 {
                return f64::NAN;
            }
            let m = add(1) / n;
            return if matches!(f, Feat::Mean(_)) { self.centre[k] + m } else { (add(2) / n - m * m).max(0.0).sqrt() };
        }
        let cells = self.cells[a] + b.map_or(0.0, |(x, _)| self.cells[x]);
        let area = cells * self.rw * self.rh;
        let edges = self.edges[a] + b.map_or(0.0, |(x, s)| self.edges[x] - 2.0 * s as f64);
        let perimeter = edges * (self.rw + self.rh) / 2.0;
        let mut bx = self.bbox[a];
        if let Some((x, _)) = b {
            let y = self.bbox[x];
            bx = [bx[0].min(y[0]), bx[1].min(y[1]), bx[2].max(y[2]), bx[3].max(y[3])];
        }
        let (bh, bw) = ((bx[2] - bx[0] + 1) as f64 * self.rh, (bx[3] - bx[1] + 1) as f64 * self.rw);
        let (length, width) = (bh.max(bw), bh.min(bw));
        match f {
            Feat::Cells => cells,
            Feat::Area => area,
            Feat::Perimeter => perimeter,
            Feat::Length => length,
            Feat::Width => width,
            Feat::LengthWidth => length / width,
            Feat::Compactness => length * width / area,
            Feat::ShapeIndex => perimeter / (4.0 * area.sqrt()),
            Feat::BorderIndex => perimeter / (2.0 * (length + width)),
            Feat::Mean(_) | Feat::Sd(_) => unreachable!("the bands are above"),
        }
    }

    /// The expression over the items, in batch: a value per item.
    fn evaluate(&self, script: &Script, used: &[(String, Feat)], items: &[Item]) -> Result<Vec<f64>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let cols: Vec<(String, Column)> = used
            .iter()
            .map(|(n, f)| {
                let x: Vec<f64> = items.par_iter().with_min_len(2048).map(|&(a, b)| self.feat(*f, a, b)).collect();
                (n.clone(), Column::Numbers(x))
            })
            .collect();
        let frame = Frame { n: items.len(), attributes: &cols, vector: None };
        let mut r = run_frame(&frame, &BTreeMap::new(), script)?;
        if r.is_empty() {
            return Err(Error::msg("the fitting gives no value"));
        }
        match r.remove(0).1 {
            Column::Numbers(x) if x.len() == items.len() => Ok(x),
            Column::Numbers(x) if x.len() == 1 => Ok(vec![x[0]; items.len()]),
            Column::Numbers(_) => Err(Error::msg("the fitting gives no value for each object")),
            Column::Texts(_) => Err(Error::msg("the fitting is a text where a number goes")),
        }
    }

    /// The group `seed` joined to `cand`: the smaller index stays the group. Returns (kept, gone).
    fn join(&mut self, seed: usize, cand: usize) -> (usize, usize) {
        let (to, from) = (seed.min(cand), seed.max(cand));
        let shared = self.adj[seed].iter().find(|&&(x, _)| x as usize == cand).map_or(0, |p| p.1);
        self.head[to] = self.head[seed];
        self.edges[to] += self.edges[from] - 2.0 * shared as f64;
        self.cells[to] += self.cells[from];
        let (p, q) = (self.bbox[to], self.bbox[from]);
        self.bbox[to] = [p[0].min(q[0]), p[1].min(q[1]), p[2].max(q[2]), p[3].max(q[3])];
        let w = self.nb * 3;
        for j in 0..w {
            self.bands[to * w + j] += self.bands[from * w + j];
        }
        // the neighbours of both, the shared borders added; each neighbour's list now names `to`
        let (a, b) = (std::mem::take(&mut self.adj[to]), std::mem::take(&mut self.adj[from]));
        let skip = |x: u32| x as usize == to || x as usize == from;
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(a.len() + b.len());
        let (mut i, mut j) = (0, 0);
        loop {
            while i < a.len() && skip(a[i].0) {
                i += 1;
            }
            while j < b.len() && skip(b[j].0) {
                j += 1;
            }
            match (a.get(i), b.get(j)) {
                (None, None) => break,
                (Some(&x), None) => {
                    merged.push(x);
                    i += 1;
                }
                (None, Some(&y)) => {
                    merged.push(y);
                    j += 1;
                }
                (Some(&x), Some(&y)) if x.0 < y.0 => {
                    merged.push(x);
                    i += 1;
                }
                (Some(&x), Some(&y)) if y.0 < x.0 => {
                    merged.push(y);
                    j += 1;
                }
                (Some(&x), Some(&y)) => {
                    merged.push((x.0, x.1 + y.1));
                    i += 1;
                    j += 1;
                }
            }
        }
        for &(n, len) in &merged {
            let l = &mut self.adj[n as usize];
            l.retain(|&(x, _)| x as usize != from && x as usize != to);
            let at = l.partition_point(|&(x, _)| x < to as u32);
            l.insert(at, (to as u32, len));
        }
        self.adj[to] = merged;
        self.live[from] = false;
        (to, from)
    }
}

/// What the settings ask.
struct Plan {
    seed_ok: Vec<bool>,
    cand_ok: Vec<bool>,
    /// 1 when more is better, -1 when less is.
    sign: f64,
    improve: bool,
    threshold: Option<f64>,
    all: bool,
    max_passes: usize,
}

/// A seed's wish to take a candidate, the better the higher its score.
struct Prop {
    score: f64,
    seed: u32,
    cand: u32,
}

/// The passes: the parent of each group (a group joined to a smaller one points to it) and the joins made.
fn passes(
    st: &mut State,
    o: &Objects,
    script: &Script,
    used: &[(String, Feat)],
    plan: &Plan,
) -> Result<(Vec<u32>, usize)> {
    let n = st.live.len();
    let mut parent: Vec<u32> = (0..n as u32).collect();
    let mut joined = 0;
    for _ in 0..plan.max_passes {
        let class = |st: &State, g: usize| o.class_of[st.head[g] as usize] as usize;
        // the neighbours where a seed meets a candidate
        let mut pairs: Vec<(usize, usize, u32)> = Vec::new();
        let mut want = vec![false; n];
        for a in (0..n).filter(|&a| st.live[a]) {
            let ca = class(st, a);
            for &(b, len) in &st.adj[a] {
                let (b, cb) = (b as usize, class(st, b as usize));
                if b <= a {
                    continue;
                }
                let (ab, ba) = (plan.seed_ok[ca] && plan.cand_ok[cb], plan.seed_ok[cb] && plan.cand_ok[ca]);
                if ab || ba {
                    pairs.push((a, b, len));
                    want[a] |= ab;
                    want[b] |= ba;
                }
            }
        }
        if pairs.is_empty() {
            break;
        }
        let items: Vec<Item> = pairs.iter().map(|&(a, b, l)| (a, Some((b, l)))).collect();
        let vals = st.evaluate(script, used, &items)?;
        // the seeds' own values, the bar a joined object has to clear
        let mut own = vec![f64::NAN; n];
        if plan.improve {
            let ids: Vec<usize> = (0..n).filter(|&g| want[g]).collect();
            let items: Vec<Item> = ids.iter().map(|&g| (g, None)).collect();
            for (g, v) in ids.iter().zip(st.evaluate(script, used, &items)?) {
                own[*g] = v;
            }
        }
        let mut props: Vec<Prop> = Vec::new();
        for (&(a, b, _), &v) in pairs.iter().zip(&vals) {
            if !v.is_finite()
                || plan.threshold.is_some_and(|t| (plan.sign > 0.0 && v < t) || (plan.sign < 0.0 && v > t))
            {
                continue;
            }
            let score = plan.sign * v;
            for (s, c) in [(a, b), (b, a)] {
                if !(plan.seed_ok[class(st, s)] && plan.cand_ok[class(st, c)]) {
                    continue;
                }
                if plan.improve {
                    // (as good as the seed: a tie joins, so that pieces of a rectangle make the rectangle)
                    let bar = plan.sign * own[s];
                    if !bar.is_finite() || score < bar - 1e-12 * bar.abs().max(1.0) {
                        continue;
                    }
                }
                props.push(Prop { score, seed: s as u32, cand: c as u32 });
            }
        }
        if !plan.all {
            // each seed with its best candidate (the lower id on a tie)
            props.par_sort_unstable_by(|x, y| {
                x.seed.cmp(&y.seed).then(y.score.total_cmp(&x.score)).then(x.cand.cmp(&y.cand))
            });
            props.dedup_by_key(|p| p.seed);
        }
        // the best first, the bigger seed then the lower ids on a tie; a pair only when both are free
        props.par_sort_unstable_by(|x, y| {
            y.score
                .total_cmp(&x.score)
                .then(st.cells[y.seed as usize].total_cmp(&st.cells[x.seed as usize]))
                .then(x.seed.cmp(&y.seed))
                .then(x.cand.cmp(&y.cand))
        });
        let mut taken = vec![false; n];
        let mut accepted: Vec<(usize, usize)> = Vec::new();
        for p in &props {
            let (s, c) = (p.seed as usize, p.cand as usize);
            if !taken[s] && !taken[c] {
                taken[s] = true;
                taken[c] = true;
                accepted.push((s, c));
            }
        }
        if accepted.is_empty() {
            break;
        }
        for (s, c) in accepted {
            let (to, from) = st.join(s, c);
            parent[from] = to as u32;
            joined += 1;
        }
    }
    Ok((parent, joined))
}

/// The classes named, as a flag per class number (0 unclassified, said as "unclassified"); all when none are named.
fn class_flags(o: &Objects, names: Option<Vec<String>>) -> Result<Vec<bool>> {
    let mut flags = vec![true; o.classes.len() + 1];
    if let Some(names) = names.filter(|n| !n.is_empty()) {
        let none = names.iter().any(|n| n == "unclassified");
        let rest: Vec<String> = names.into_iter().filter(|n| n != "unclassified").collect();
        flags = vec![false; o.classes.len() + 1];
        flags[0] = none;
        for k in class_numbers(o, &rest)? {
            flags[k as usize] = true;
        }
    }
    Ok(flags)
}

/// `objects.fuse`.
struct Fuse;

impl Operator for Fuse {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.fuse",
            "Joins an object to a neighbour only when the object they make is better by a measure, as eCognition's \
             image object fusion (objects.merge by condition looks at the two objects; this looks at the result). \
             `fitting` is an expression over the joined object, written as objects.calc's: compactness (length x \
             width over area: 1 for a rectangle, higher is less compact, so goal min), shape_index, border_index, \
             length, width, length_width, area, cells, perimeter, and with the image mean_<band> and sd_<band> \
             (mean_nir > 0.3, sd_red, compactness * 2 + sd_nir). Its attributes are derived from sums the groups keep, \
             not computed again from the cells: the box's sides stand for the length and the width (a close \
             approximation of the rotated rectangle objects.features measures), and anything that needs the cells or \
             the neighbours (texture, skeleton, context) is refused with the list of what works. `seeds` are the \
             classes that take (default all), `candidates` the classes they take from (default all, `unclassified` \
             counts as a class); the joined object has the seed's class. `goal` max (the default) or min says which \
             is better; `improve` (the default) joins only when the joined object is at least as good as the seed's \
             own value (a tie joins: two rectangles make a rectangle); `threshold` is the value the joined object \
             must reach (at least for max, at most for min). Per pass each seed takes its best candidate (`mode` \
             best, the default) or any that qualify (`all`), best first, a pair only when neither object has joined \
             in this pass; it repeats until nothing joins or `max_passes`. An object may be seed and candidate (all \
             classes): the pair's value is the same both ways and the bigger one gives the class. The level above is \
             dropped (with a warning), as are the other attributes: the table is made again from the `image`, else \
             the attributes are the means of the parts weighted by cells (objects.features again). `count` is the \
             number of objects, `done` is true when nothing joined, `report` has a row per joined object: its id \
             and its parts. The same result on every run, on any number of threads.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects, with their classes"))
        .input(PortSpec::optional("image", Type::Raster, "the image of the objects: the bands' means and deviations"))
        .output(PortSpec::new("objects", Type::Objects, "the objects after the fusion"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "id, cells, area (and the band means and deviations with an image)",
        ))
        .output(PortSpec::new("report", Type::Table, "a row per joined object: its id and the objects it joined"))
        .output(PortSpec::new("count", Type::Integer, "the number of objects after"))
        .output(PortSpec::new("done", Type::Bool, "true when nothing joined"))
        .param(PortSpec::new(
            "fitting",
            Type::Text,
            "an objects.calc expression over the joined object (compactness, mean_nir, sd_red)",
        ))
        .param(
            PortSpec::optional("goal", Type::Text, "max (the default: a higher value is better) or min")
                .choices(["max", "min"]),
        )
        .param(PortSpec::optional("seeds", Type::Any, "the class names whose objects take a neighbour (default: all)"))
        .param(PortSpec::optional("candidates", Type::Any, "the class names that may be taken (default: all)"))
        .param(PortSpec::optional(
            "improve",
            Type::Bool,
            "join only when the joined object is at least as good as the seed's (default true)",
        ))
        .param(PortSpec::optional(
            "threshold",
            Type::Number,
            "the value the joined object must reach (at least for max, at most for min)",
        ))
        .param(
            PortSpec::optional("mode", Type::Text, "best (the default: each seed its best candidate) or all")
                .choices(["best", "all"]),
        )
        .param(PortSpec::optional("max_passes", Type::Integer, "the most passes (default 100)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        let image = match args.get("image") {
            Some(_) => {
                let l = big::layers(args, "image")?;
                if !l.grid.same_as(&o.grid) {
                    return Err(bad(format!(
                        "the image is on a {}x{} grid, the objects' is {}x{}: put it on the objects' grid with \
                         raster.match",
                        l.grid.height, l.grid.width, o.grid.height, o.grid.width
                    )));
                }
                Some(l)
            }
            None => None,
        };
        let bands: Vec<String> = image.as_ref().map(|l| band_names(&l.names)).unwrap_or_default();
        let text: String = args.param("fitting", String::new())?;
        if text.trim().is_empty() {
            return Err(bad("give the fitting: an expression over the joined object (compactness, mean_nir)".into()));
        }
        let script = parse_script_objects(&text).map_err(|e| bad(format!("the fitting: {}", e.0)))?;
        let used = names_of(&script, &bands).map_err(|e| bad(e.0))?;
        let sign = match args.param("goal", "max".to_string())?.as_str() {
            "max" => 1.0,
            "min" => -1.0,
            other => return Err(bad(format!("goal {other:?}: max or min"))),
        };
        let all = match args.param("mode", "best".to_string())?.as_str() {
            "best" => false,
            "all" => true,
            other => return Err(bad(format!("mode {other:?}: best or all"))),
        };
        let threshold: Option<f64> = args.opt_param("threshold")?;
        if threshold.is_some_and(f64::is_nan) {
            return Err(bad("threshold is not a number".into()));
        }
        let max_passes: i64 = args.param("max_passes", 100)?;
        if max_passes < 1 {
            return Err(bad(format!("max_passes {max_passes}: 1 or more")));
        }
        let plan = Plan {
            seed_ok: class_flags(o, args.opt_param("seeds")?).map_err(|e| bad(format!("seeds: {}", e.0)))?,
            cand_ok: class_flags(o, args.opt_param("candidates")?).map_err(|e| bad(format!("candidates: {}", e.0)))?,
            sign,
            improve: args.param("improve", true)?,
            threshold,
            all,
            max_passes: max_passes as usize,
        };
        let data: &[Arc<Vec<f64>>] = image.as_ref().map_or(&[], |l| l.data.as_slice());
        let mut st = State::new(o, data);
        let (mut parent, joined) = passes(&mut st, o, &script, &used, &plan).map_err(|e| bad(e.0))?;
        let table_of = |x: &Objects| match &image {
            Some(l) => stats_table(x, &l.names, &l.data),
            None => x.table.clone(),
        };
        if joined == 0 {
            let report = Table::new(vec!["object".into(), "parts".into()]);
            let mut out = outputs([
                ("table", Value::Table(table_of(o))),
                ("report", Value::Table(report)),
                ("count", Value::Integer(o.count as i64)),
                ("done", Value::Bool(true)),
            ]);
            out.insert("objects".into(), Value::Objects(o.clone()));
            return Ok(out);
        }
        // the groups made one object each; the level above cannot hold
        let stripped;
        let base: &Objects = if o.parent.is_some() {
            ctx.warn(format!(
                "node {node:?}: the joined objects replace the objects, so the level above is dropped (objects.merge \
                 with level makes one again)"
            ));
            let mut s = o.as_ref().clone();
            s.parent = None;
            s.parent_of = Vec::new();
            stripped = s;
            &stripped
        } else {
            o
        };
        let groups: Vec<u32> = (0..o.count as u32).map(|k| find(&mut parent, k)).collect();
        let (cut, new_of) = regroup(base, &groups)?;
        // each joined object has the class and the memberships of its seed
        let mut head = vec![0usize; cut.count];
        let mut parts = vec![0u32; cut.count];
        for k in 0..o.count {
            let g = new_of[k] as usize - 1;
            head[g] = st.head[groups[k] as usize] as usize;
            parts[g] += 1;
        }
        let class_of: Vec<u32> = head.iter().map(|&h| o.class_of[h]).collect();
        let membership: Vec<Vec<f64>> = o.membership.iter().map(|m| head.iter().map(|&h| m[h]).collect()).collect();
        let mut cut = cut.with_classes(o.classes.clone(), class_of, membership)?;
        if image.is_some() {
            let t = table_of(&cut);
            cut = cut.with_table(t)?;
        }
        let mut report = Table::new(vec!["object".into(), "parts".into()]);
        for (g, &p) in parts.iter().enumerate().filter(|(_, p)| **p > 1) {
            report.rows.push(vec![(g + 1) as f64, p as f64]);
        }
        let table = cut.table.clone();
        let count = cut.count;
        let mut out = outputs([
            ("table", Value::Table(table)),
            ("report", Value::Table(report)),
            ("count", Value::Integer(count as i64)),
            ("done", Value::Bool(false)),
        ]);
        out.insert("objects".into(), Value::Objects(Arc::new(cut)));
        Ok(out)
    }
}
