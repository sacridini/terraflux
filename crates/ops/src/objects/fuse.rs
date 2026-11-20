//! `objects.fuse`: eCognition's image object fusion. An object (a seed) joins a neighbour (a candidate) only when the
//! object they make is good by a measure: an `objects.calc` expression over the joined object (`compactness`,
//! `mean_nir`, `sd_red`), better than the seed's own, and/or beyond a threshold. The merging by condition
//! (`objects.merge`) looks at the two objects; this looks at the result.
//!
//! The joined object's attributes are not computed again from its cells. Each group keeps what adds up: its cells,
//! its perimeter in cell edges (the sum of the two less twice the border they share, from the neighbour lists), the
//! convex hull of its cells' corners, and per band the count, the sum and the sum of squares (of the values less the
//! band's mean, as `stats_table`), so the cells, area, perimeter, and the means and deviations of the bands come in
//! constant time, and the length and width (the sides of the minimum-area rectangle, by rotating calipers on the hull
//! of the two groups' hulls, the code and so the numbers of `objects.features`), compactness, shape_index and
//! border_index in the time of the hulls' points. What cannot be derived this way (texture, skeleton, context, anything over
//! the neighbours) is refused with the list of what can.
//!
//! A pass: the expression is evaluated in batch over a table with a row per pair of neighbours (and, for `improve`,
//! over the seeds alone); each seed proposes its best candidate (`mode` best) or all the candidates that qualify
//! (`mode` all); the proposals are taken best first, a pair only when neither of its objects has joined in the pass
//! (disjoint pairs, as the merging of `objects.merge`); the groups' neighbour lists are updated; and again, until
//! nothing joins or `max_passes`. Every step runs in the order of the ids, so the result is the same on any number of
//! threads (the batch evaluation and the band sums run in parallel and give the same numbers).
//!
//! A pass only looks again at what the last one changed. A pair's value is kept in the neighbour lists, a seed's own
//! value and proposals beside them; a join leaves stale the pairs of the group it makes (and the proposals of that
//! group and of its neighbours), and only those are evaluated and proposed again. The values are those a full
//! evaluation gives (the expression is evaluated row by row), so the joins are the same, in the same order.

use std::collections::BTreeMap;
use std::sync::Arc;

use rayon::prelude::*;
use smallvec::SmallVec;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::Column;

use super::features::{HullScratch, hull_rect, merge_hulls, sorted_hull};
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

impl Feat {
    /// Whether the attribute comes from the rotated rectangle (the hull).
    fn needs_rect(self) -> bool {
        matches!(self, Feat::Length | Feat::Width | Feat::LengthWidth | Feat::Compactness | Feat::BorderIndex)
    }
}

/// Whether a group of `cells` cells fills its box (row and column of the first and last cell).
fn fills(cells: f64, bx: [u32; 4]) -> bool {
    cells == (bx[2] - bx[0] + 1) as f64 * (bx[3] - bx[1] + 1) as f64
}

/// The corners of a box, sorted: the hull of a group that fills it (such a group keeps no hull).
fn corners(bx: [u32; 4]) -> [(i64, i64); 4] {
    let (r0, c0, r1, c1) = (bx[0] as i64, bx[1] as i64, bx[2] as i64 + 1, bx[3] as i64 + 1);
    [(c0, r0), (c0, r1), (c1, r0), (c1, r1)]
}

/// The convex hull of the cells' corners of each object (but those that fill their box, which have none, see
/// [`State::hull`]), as `objects.features` takes it: from the runs of each row.
fn hulls_of(o: &Objects) -> Vec<Vec<(i64, i64)>> {
    let (w, h) = (o.grid.width, o.grid.height);
    // (the runs of each row: object, row, first and last column)
    let rows: Vec<Vec<(u32, u32, u32, u32)>> = (0..h)
        .into_par_iter()
        .map(|r| {
            let mut v: Vec<(u32, u32, u32, u32)> = Vec::new();
            let mut c = 0;
            while c < w {
                let l = o.labels[r * w + c];
                let a = c;
                while c + 1 < w && o.labels[r * w + c + 1] == l {
                    c += 1;
                }
                if l != 0 && !fills(o.cells[l as usize - 1] as f64, o.boxes[l as usize - 1]) {
                    v.push((l, r as u32, a as u32, c as u32));
                }
                c += 1;
            }
            v
        })
        .collect();
    let mut pts: Vec<Vec<(i64, i64)>> = vec![Vec::new(); o.count];
    for &(l, r, a, b) in rows.iter().flatten() {
        let (r, a, b) = (r as i64, a as i64, b as i64 + 1);
        pts[l as usize - 1].extend([(a, r), (a, r + 1), (b, r), (b, r + 1)]);
    }
    pts.into_par_iter().map(sorted_hull).collect()
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

/// A neighbour: its id, the edges shared, and the value of the pair joined (NaN when not known yet).
type Nb = (u32, u32, f64);

/// The groups of objects (the index of a group is the smallest object in it) and what adds up over them.
struct State {
    live: Vec<bool>,
    cells: Vec<f64>,
    /// The perimeter in cell edges: those that face another group, no object or the image's edge.
    edges: Vec<f64>,
    /// The convex hull of each group's cell corners, counter-clockwise (the joined object's is the hull of two).
    hulls: Vec<Vec<(i64, i64)>>,
    /// The box of each group (row and column of the first cell, then of the last), kept with the hulls.
    bbox: Vec<[u32; 4]>,
    /// Per group and band: the count, the sum and the sum of squares of the values less the band's centre.
    bands: Vec<f64>,
    nb: usize,
    centre: Vec<f64>,
    /// The neighbours (0-based id, edges shared, value of the pair), sorted by id.
    adj: Vec<Vec<Nb>>,
    /// The original object whose class and memberships the group has: that of its seed.
    head: Vec<u32>,
    /// The class number of each group (that of its head).
    class: Vec<u32>,
    /// A list kept to make the next group's neighbours in.
    spare: Vec<Nb>,
    /// The cell's sides in map units.
    rw: f64,
    rh: f64,
}

impl State {
    /// `shapes`: whether the length and width are needed (the hulls are kept only then).
    fn new(o: &Objects, data: &[Arc<Vec<f64>>], shapes: bool) -> State {
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
        let adj: Vec<Vec<Nb>> =
            o.adjacency.iter().map(|l| l.iter().map(|&(b, len)| (b - 1, len, f64::NAN)).collect()).collect();
        for (k, l) in adj.iter().enumerate() {
            edges[k] += l.iter().map(|&(_, len, _)| len as f64).sum::<f64>();
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
        let hulls = if shapes { hulls_of(o) } else { Vec::new() };
        State {
            live: vec![true; n],
            cells: o.cells.iter().map(|&c| c as f64).collect(),
            edges,
            hulls,
            bbox: if shapes { o.boxes.clone() } else { Vec::new() },
            bands,
            nb,
            centre: per.iter().map(|p| p.0).collect(),
            adj,
            head: (0..n as u32).collect(),
            class: o.class_of.clone(),
            spare: Vec::new(),
            rw,
            rh,
        }
    }

    /// One attribute of a group (`b` none) or of the group joined to another (with the edges they share).
    ///
    /// `rect` is the (length, width) of the minimum-area rectangle around it (see [`State::rect`]).
    fn feat(&self, f: Feat, a: usize, b: Option<(usize, u32)>, rect: (f64, f64)) -> f64 {
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
        let (length, width) = rect;
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

    /// The sorted hull of a group: the one kept, or the corners of its box (put in `buf`) when it fills it.
    fn hull<'a>(&'a self, g: usize, buf: &'a mut [(i64, i64); 4]) -> &'a [(i64, i64)] {
        if fills(self.cells[g], self.bbox[g]) {
            *buf = corners(self.bbox[g]);
            buf
        } else {
            &self.hulls[g]
        }
    }

    /// The length and the width of a group or of two joined, as `objects.features` measures them: the sides of the
    /// minimum-area rectangle around the hull of the cells' corners.
    fn rect(&self, a: usize, b: Option<(usize, u32)>, sc: &mut HullScratch) -> (f64, f64) {
        // (a group that fills its box is that rectangle: the calipers would give the same sides, to the bit)
        let (cells, mut bx) = (self.cells[a], self.bbox[a]);
        let (mut ca, mut cb) = ([(0, 0); 4], [(0, 0); 4]);
        if let Some((x, _)) = b {
            let y = self.bbox[x];
            bx = [bx[0].min(y[0]), bx[1].min(y[1]), bx[2].max(y[2]), bx[3].max(y[3])];
        }
        let cells = cells + b.map_or(0.0, |(x, _)| self.cells[x]);
        let (rows, cols) = ((bx[2] - bx[0] + 1) as f64, (bx[3] - bx[1] + 1) as f64);
        if cells == rows * cols {
            let side = |lo: u32, n: f64, d: f64| ((lo as f64 + n) * d) - (lo as f64 * d);
            let (h, w) = (side(bx[0], rows, self.rh), side(bx[1], cols, self.rw));
            return (h.max(w), h.min(w));
        }
        let ha = self.hull(a, &mut ca);
        match b {
            None => hull_rect(ha, None, self.rw, self.rh, sc),
            Some((x, _)) => hull_rect(ha, Some(self.hull(x, &mut cb)), self.rw, self.rh, sc),
        }
    }

    /// The expression over the items, in batch: a value per item.
    fn evaluate(&self, script: &Script, used: &[(String, Feat)], items: &[Item]) -> Result<Vec<f64>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        // (the rectangles once per item, when an attribute needs them)
        let needs = used.iter().any(|(_, f)| f.needs_rect());
        let rects: Vec<(f64, f64)> = if needs {
            items
                .par_iter()
                .with_min_len(512)
                .map_init(HullScratch::default, |sc, &(a, b)| self.rect(a, b, sc))
                .collect()
        } else {
            vec![(f64::NAN, f64::NAN); items.len()]
        };
        let cols: Vec<(String, Column)> = used
            .iter()
            .map(|(n, f)| {
                let x: Vec<f64> = items
                    .par_iter()
                    .zip(rects.par_iter())
                    .with_min_len(2048)
                    .map(|(&(a, b), &r)| self.feat(*f, a, b, r))
                    .collect();
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
        let shared = self.adj[seed].iter().find(|&&(x, ..)| x as usize == cand).map_or(0, |p| p.1);
        self.head[to] = self.head[seed];
        self.class[to] = self.class[seed];
        self.edges[to] += self.edges[from] - 2.0 * shared as f64;
        if !self.hulls.is_empty() {
            let (mut ca, mut cb) = ([(0, 0); 4], [(0, 0); 4]);
            let merged = merge_hulls(self.hull(to, &mut ca), self.hull(from, &mut cb), &mut HullScratch::default());
            let (p, q) = (self.bbox[to], self.bbox[from]);
            self.bbox[to] = [p[0].min(q[0]), p[1].min(q[1]), p[2].max(q[2]), p[3].max(q[3])];
            self.hulls[from] = Vec::new();
            // (a group that fills its box keeps no hull)
            let full = fills(self.cells[to] + self.cells[from], self.bbox[to]);
            self.hulls[to] = if full { Vec::new() } else { merged };
        }
        self.cells[to] += self.cells[from];
        let w = self.nb * 3;
        for j in 0..w {
            self.bands[to * w + j] += self.bands[from * w + j];
        }
        // the neighbours of both, the shared borders added; each neighbour's list now names `to`
        let (mut a, b) = (std::mem::take(&mut self.adj[to]), std::mem::take(&mut self.adj[from]));
        let skip = |x: u32| x as usize == to || x as usize == from;
        let mut merged = std::mem::take(&mut self.spare);
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
                    merged.push((x.0, x.1 + y.1, f64::NAN));
                    i += 1;
                    j += 1;
                }
            }
        }
        // (the pairs of the group made are stale: their values are not known until evaluated again)
        for m in &mut merged {
            m.2 = f64::NAN;
            let l = &mut self.adj[m.0 as usize];
            l.retain(|&(x, ..)| x as usize != from && x as usize != to);
            let at = l.partition_point(|&(x, ..)| x < to as u32);
            l.insert(at, (to as u32, m.1, f64::NAN));
        }
        a.clear();
        self.spare = a;
        self.adj[to] = merged;
        self.live[from] = false;
        (to, from)
    }
}

/// A seed's proposals: (score, candidate), one in `best` mode.
type Proposals = SmallVec<[(f64, u32); 1]>;

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
    /// Evaluate everything again in each pass (the passes must give the same: for the tests).
    full: bool,
}

/// A seed's wish to take a candidate, the better the higher its score.
struct Prop {
    score: f64,
    /// The seed's cells (the bigger seed goes first on a tie).
    cells: f64,
    seed: u32,
    cand: u32,
}

/// The passes: the parent of each group (a group joined to a smaller one points to it) and the joins made.
fn passes(st: &mut State, script: &Script, used: &[(String, Feat)], plan: &Plan) -> Result<(Vec<u32>, usize)> {
    let n = st.live.len();
    let mut parent: Vec<u32> = (0..n as u32).collect();
    let mut joined = 0;
    // a seed's wish for a candidate, whatever their values
    let may = |st: &State, s: usize, c: usize| plan.seed_ok[st.class[s] as usize] && plan.cand_ok[st.class[c] as usize];
    // the seeds' own values (the bar a joined object has to clear) and each seed's proposals (score, candidate)
    let mut own = vec![f64::NAN; n];
    let mut cache: Vec<Proposals> = vec![Proposals::new(); n];
    // the groups whose pairs, own value or neighbours changed in the last pass: all at first
    let mut dirty: Vec<usize> = (0..n).collect();
    let mut is_dirty = vec![true; n];
    // the seeds that have a proposal (maybe stale: the ones that lost it or joined are dropped when read)
    let mut active: Vec<usize> = Vec::new();
    let mut has = vec![false; n];
    let mut taken = vec![0usize; n];
    for pass in 1..=plan.max_passes {
        if plan.full {
            for l in st.adj.iter_mut().flatten() {
                l.2 = f64::NAN;
            }
            own.fill(f64::NAN);
            dirty = (0..n).collect();
            is_dirty.fill(true);
        }
        dirty.retain(|&g| st.live[g]);
        dirty.sort_unstable();
        // the pairs left stale by the last joins (each once: from the smaller id, or from the only dirty side)
        let pairs: Vec<(usize, usize, u32)> = dirty
            .par_iter()
            .with_min_len(1024)
            .flat_map_iter(|&a| {
                let (is_dirty, st) = (&is_dirty, &*st);
                st.adj[a].iter().filter_map(move |&(b, len, v)| {
                    let b = b as usize;
                    let (x, y) = (a.min(b), a.max(b));
                    (v.is_nan() && (b > a || !is_dirty[b]) && (may(st, x, y) || may(st, y, x))).then_some((x, y, len))
                })
            })
            .collect();
        let items: Vec<Item> = pairs.iter().map(|&(a, b, l)| (a, Some((b, l)))).collect();
        for (&(a, b, _), v) in pairs.iter().zip(st.evaluate(script, used, &items)?) {
            for (x, y) in [(a, b), (b, a)] {
                let at = st.adj[x].partition_point(|&(z, ..)| (z as usize) < y);
                st.adj[x][at].2 = v;
            }
        }
        if plan.improve {
            let ids: Vec<usize> = dirty
                .iter()
                .copied()
                .filter(|&g| own[g].is_nan() && st.adj[g].iter().any(|&(c, ..)| may(st, g, c as usize)))
                .collect();
            let items: Vec<Item> = ids.iter().map(|&g| (g, None)).collect();
            for (&g, v) in ids.iter().zip(st.evaluate(script, used, &items)?) {
                own[g] = v;
            }
        }
        // each dirty seed proposes again: its best candidate (the lower id on a tie) or all that qualify
        let proposals: Vec<Proposals> = dirty
            .par_iter()
            .with_min_len(1024)
            .map(|&s| {
                let mut best = Proposals::new();
                for &(c, _, v) in &st.adj[s] {
                    if !may(st, s, c as usize) {
                        continue;
                    }
                    if !v.is_finite()
                        || plan.threshold.is_some_and(|t| (plan.sign > 0.0 && v < t) || (plan.sign < 0.0 && v > t))
                    {
                        continue;
                    }
                    let score = plan.sign * v;
                    if plan.improve {
                        // (as good as the seed: a tie joins, so that pieces of a rectangle make the rectangle)
                        let bar = plan.sign * own[s];
                        if !bar.is_finite() || score < bar - 1e-12 * bar.abs().max(1.0) {
                            continue;
                        }
                    }
                    // (ascending candidates: a tie keeps the lower id)
                    match best.first_mut() {
                        Some(b) if !plan.all => {
                            if score.total_cmp(&b.0).is_gt() {
                                *b = (score, c);
                            }
                        }
                        _ => best.push((score, c)),
                    }
                }
                best
            })
            .collect();
        for (&s, best) in dirty.iter().zip(proposals) {
            cache[s] = best;
            is_dirty[s] = false;
            if !cache[s].is_empty() && !has[s] {
                has[s] = true;
                active.push(s);
            }
        }
        dirty.clear();
        active.retain(|&s| {
            has[s] = st.live[s] && !cache[s].is_empty();
            has[s]
        });
        let mut props: Vec<Prop> = Vec::new();
        for &s in &active {
            let cells = st.cells[s];
            props.extend(cache[s].iter().map(|&(score, c)| Prop { score, cells, seed: s as u32, cand: c }));
        }
        // the best first, the bigger seed then the lower ids on a tie; a pair only when both are free
        props.par_sort_unstable_by(|x, y| {
            y.score
                .total_cmp(&x.score)
                .then(y.cells.total_cmp(&x.cells))
                .then(x.seed.cmp(&y.seed))
                .then(x.cand.cmp(&y.cand))
        });
        let mut accepted: Vec<(usize, usize)> = Vec::new();
        for p in &props {
            let (s, c) = (p.seed as usize, p.cand as usize);
            if taken[s] != pass && taken[c] != pass {
                taken[s] = pass;
                taken[c] = pass;
                accepted.push((s, c));
            }
        }
        if accepted.is_empty() {
            break;
        }
        // (the pairs are disjoint, so the joins give the same in any order: by id they touch memory nearby)
        accepted.sort_unstable_by_key(|&(s, c)| s.min(c));
        for (s, c) in accepted {
            let (to, from) = st.join(s, c);
            parent[from] = to as u32;
            joined += 1;
            cache[from] = Proposals::new();
            own[to] = f64::NAN;
            // the group made and its neighbours: their pairs and proposals change
            for g in std::iter::once(to).chain(st.adj[to].iter().map(|&(g, ..)| g as usize)) {
                if !is_dirty[g] {
                    is_dirty[g] = true;
                    dirty.push(g);
                }
            }
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
             not computed again from the cells: the length and the width are the sides of the rotated \
             minimum-area rectangle, from the convex hull the groups keep (the numbers objects.features gives for \
             the result), and anything that needs the cells or the neighbours (texture, skeleton, context) is refused with the list of what works. `seeds` are the \
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
        let improve: bool = args.param("improve", true)?;
        if !improve && threshold.is_none() {
            // (a warning, not a refusal: joining every pair is what some want, to rebuild the clumps of a class)
            ctx.warn(format!(
                "node {node:?}: improve is false and there is no threshold, so nothing limits the joins: every pair \
                 of neighbours qualifies and the objects are joined into the clumps of their neighbours (of the \
                 candidates' classes); give a threshold, or leave improve true"
            ));
        }
        let max_passes: i64 = args.param("max_passes", 100)?;
        if max_passes < 1 {
            return Err(bad(format!("max_passes {max_passes}: 1 or more")));
        }
        let plan = Plan {
            seed_ok: class_flags(o, args.opt_param("seeds")?).map_err(|e| bad(format!("seeds: {}", e.0)))?,
            cand_ok: class_flags(o, args.opt_param("candidates")?).map_err(|e| bad(format!("candidates: {}", e.0)))?,
            sign,
            improve,
            threshold,
            all,
            max_passes: max_passes as usize,
            full: false,
        };
        let data: &[Arc<Vec<f64>>] = image.as_ref().map_or(&[], |l| l.data.as_slice());
        let needs = used.iter().any(|(_, f)| f.needs_rect());
        let mut st = State::new(o, data, needs);
        let (mut parent, joined) = passes(&mut st, &script, &used, &plan).map_err(|e| bad(e.0))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::ObjectClass;
    use terraflux_raster::Grid;

    fn lcg(s: &mut u64) -> f64 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*s >> 11) as f64) / (1u64 << 53) as f64
    }

    /// The incremental passes against passes that evaluate every pair and seed again each time, on a random image.
    #[test]
    fn the_incremental_passes_join_as_a_full_evaluation_does() {
        let (w, h) = (48, 40);
        let mut s = 7u64;
        // blocks of 3 x 2 cells, each its own region, and two bands of noise over a trend
        let region: Vec<u64> = (0..w * h).map(|i| ((i / w / 2) * (w / 3) + (i % w) / 3 + 1) as u64).collect();
        let o = Objects::from_regions(Arc::new(Grid::new(w, h)), &region, 4);
        let of: Vec<u32> = (0..o.count as u32).map(|k| k % 3).collect();
        let list = ["a", "b"].iter().map(|n| ObjectClass { name: n.to_string(), colour: None }).collect();
        let o = o.with_classes(list, of, Vec::new()).unwrap();
        let data: Vec<Arc<Vec<f64>>> = (0..2)
            .map(|b| {
                Arc::new((0..w * h).map(|i| (i % w) as f64 * 0.5 + b as f64 * 10.0 + lcg(&mut s) * 20.0).collect())
            })
            .collect();
        let bands = vec!["b1".to_string(), "b2".to_string()];
        let cases = [
            ("sd_b1 + sd_b2", -1.0, true, None, false),
            ("sd_b1 + sd_b2", -1.0, false, Some(9.0), false),
            ("compactness", -1.0, true, None, false),
            ("compactness", -1.0, false, Some(1.5), true),
            ("mean_b1 - mean_b2 + cells * 0.01", 1.0, true, None, true),
            ("shape_index + border_index", -1.0, false, Some(2.3), false),
        ];
        for (text, sign, improve, threshold, all) in cases {
            let script = parse_script_objects(text).unwrap();
            let used = names_of(&script, &bands).unwrap();
            let run = |full: bool| {
                let mut st = State::new(&o, &data, true);
                let plan = Plan {
                    seed_ok: vec![true; 3],
                    cand_ok: vec![true, true, false],
                    sign,
                    improve,
                    threshold,
                    all,
                    max_passes: 100,
                    full,
                };
                let (parent, joined) = passes(&mut st, &script, &used, &plan).unwrap();
                (parent, joined, st.live, st.head)
            };
            let (a, b) = (run(false), run(true));
            assert!(a.1 > 0, "{text}: nothing joined, the case tests nothing");
            assert!(a == b, "{text}: the incremental passes differ from the full ones");
        }
    }

    /// The length, width and compactness of the joined objects against `objects.features` on the result.
    #[test]
    fn the_joined_rectangle_is_the_one_objects_features_measures() {
        let (w, h) = (64, 48);
        let mut s = 11u64;
        // 40 Voronoi cells of random seeds: convex, so their unions are ragged
        let seeds: Vec<(f64, f64)> = (0..40).map(|_| (lcg(&mut s) * w as f64, lcg(&mut s) * h as f64)).collect();
        let region: Vec<u64> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f64, (i / w) as f64);
                let d = |p: &(f64, f64)| (p.0 - x).powi(2) + (p.1 - y).powi(2);
                let best = (0..seeds.len()).min_by(|&a, &b| d(&seeds[a]).total_cmp(&d(&seeds[b]))).unwrap();
                best as u64 + 1
            })
            .collect();
        let mut g = Grid::new(w, h);
        g.transform = [0.0, 30.0, 0.0, 0.0, 0.0, -20.0];
        let o = Objects::from_regions(Arc::new(g), &region, 4);
        let script = parse_script_objects("compactness").unwrap();
        let used = names_of(&script, &[]).unwrap();
        let mut st = State::new(&o, &[], true);
        let plan = Plan {
            seed_ok: vec![true],
            cand_ok: vec![true],
            sign: -1.0,
            improve: false,
            threshold: Some(1.4),
            all: false,
            max_passes: 2,
            full: false,
        };
        let (mut parent, joined) = passes(&mut st, &script, &used, &plan).unwrap();
        assert!(joined > 5, "{joined} joins");
        let groups: Vec<u32> = (0..o.count as u32).map(|k| find(&mut parent, k)).collect();
        let (cut, new_of) = regroup(&o, &groups).unwrap();
        assert!(cut.count < o.count && cut.count > 1);
        let image = Value::Map(crate::Map::new(cut.grid.clone(), vec![1.0; cut.grid.cells()]));
        let inputs = [("objects".to_string(), Value::Objects(Arc::new(cut))), ("image".to_string(), image)];
        let params = serde_json::json!({ "features": ["length", "width", "compactness"] });
        let params = params.as_object().cloned().unwrap();
        let args = Args { node: "n", inputs: inputs.into_iter().collect(), params: &params };
        let reg = Registry::standard();
        let out = reg.get("objects.features").unwrap().run(&mut Ctx::default(), &args).unwrap();
        let Value::Table(t) = &out["table"] else { panic!("table") };
        let col = |n: &str| t.column(n).unwrap().to_vec();
        let (len, wid, comp) = (col("length"), col("width"), col("compactness"));
        let mut multi = 0;
        for g in (0..o.count).filter(|&g| st.live[g]) {
            let at = new_of[g] as usize - 1;
            let (l, wd) = st.rect(g, None, &mut HullScratch::default());
            let c = st.feat(Feat::Compactness, g, None, (l, wd));
            for (a, b, what) in [(l, len[at], "length"), (wd, wid[at], "width"), (c, comp[at], "compactness")] {
                assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "group {g}, {what}: {a} against {b}");
            }
            multi += groups.iter().filter(|&&r| r as usize == g).count().min(2) - 1;
        }
        assert!(multi > 5, "{multi} joined objects checked");
    }
}
