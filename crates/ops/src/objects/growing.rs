//! `segment.region_growing`: the region growing and merging of GRASS GIS's `i.segment` (Momsen & Metz), as its manual
//! describes it.
//!
//! Regions start as single cells (or the segments of a map of seeds) and merge, pass after pass. Each band is scaled to
//! 0-1 by its range over the whole image, and the distance of two regions is that of their means: the Euclidean one
//! over the square root of the bands, or the Manhattan one over the bands, so it lies in 0-1 and `threshold` has the
//! same meaning on any data. In a pass the regions are visited in a shuffled order (the seed's); a region merges with
//! its most similar neighbour when that is at the threshold or closer and (the default) the region is the neighbour's
//! most similar too; a region that merged in a pass is left alone by the rest of it (as in multiresolution.rs: a big
//! region would otherwise take every neighbour in one pass, and each merge costs its whole list). It stops when a pass
//! merges nothing. A last step joins each region under `minsize` cells to its most similar neighbour, whatever the
//! distance.
//!
//! The state is a region adjacency graph like `multiresolution.rs`'s: per region the cells, the band sums and means,
//! and a sorted list of (neighbour, distance), a distance worked out once and kept until either region changes. A merge
//! is O(bands + neighbours); nothing per pass costs O(cells). Each region keeps its nearest neighbour, updated by the
//! merges, so the mutual check never reads a long list; a region with nothing to do (no neighbour close enough, or its
//! nearest is not mutual) is left out of the passes until it, a neighbour, or the neighbour it waits on changes, which
//! is the same result as visiting every region every pass. The visits are a shuffle, so the regions of the visits to
//! come are prefetched. The merging is one thread (the order of the visits is the algorithm); the set up (the scaling, the
//! regions, their lists, the first distances) and the objects run in parallel, and every number is the same on any
//! number of threads.

use std::sync::Arc;

use rayon::prelude::*;
use smallvec::SmallVec;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::superpixels::prefetch;
use super::{NO_REGION, Objects, find};
use crate::raster_stats::Layers;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(RegionGrowing));
}

/// A neighbour in a region's list: its number and the distance to it.
#[derive(Clone, Copy)]
struct Edge {
    nb: u32,
    cost: f32,
}

/// `Regions::top` before it is known, and for a region with no neighbour.
const INVALID: u32 = u32::MAX;
const EMPTY: u32 = u32::MAX - 1;

/// Neighbours kept in the region itself (a cell has 4 or 8): no heap per region.
type Adj = SmallVec<[Edge; 8]>;

/// The regions' state.
struct Regions {
    nb: usize,
    manhattan: bool,
    /// Per region: its cells.
    cells: Vec<u32>,
    /// Per region, per band: the sum of the scaled values, the cells with a value, and the mean (NaN for none).
    sum: Vec<f64>,
    cnt: Vec<u32>,
    mean: Vec<f64>,
    /// Per region: the neighbours, sorted by number.
    adj: Vec<Adj>,
    /// Per region: its neighbour of the least distance over the whole list (the lowest number on a tie), `INVALID`
    /// while not known, `EMPTY` for no neighbour. Kept up to date through the merges, so the mutual check of a
    /// region with thousands of neighbours does not read them.
    top: Vec<Edge>,
    /// The joined list of a merge (kept to reuse its memory).
    scratch: Vec<Edge>,
}

/// The distance of two regions' means (to the power the similarity works in: the squares for Euclidean, so no root is
/// taken): the mean over the bands both have of the squared or of the absolute gaps; infinity when they share none.
#[inline(always)]
fn distance(mean: &[f64], nb: usize, manhattan: bool, a: usize, b: usize) -> f32 {
    let (x, y) = (&mean[a * nb..(a + 1) * nb], &mean[b * nb..(b + 1) * nb]);
    let (mut s, mut n) = (0.0f64, 0u32);
    for j in 0..nb {
        let d = x[j] - y[j];
        if !d.is_nan() {
            s += if manhattan { d.abs() } else { d * d };
            n += 1;
        }
    }
    if n == 0 { f32::INFINITY } else { (s / n as f64) as f32 }
}

/// How many visits ahead the regions are prefetched.
const AHEAD: usize = 4;

/// The scaling of each band to 0-1: its lowest value and 1 / range (0 for a band of one value or none).
fn scales(l: &Layers) -> Vec<(f64, f64)> {
    l.data
        .par_iter()
        .map(|b| {
            let (lo, hi) = b
                .par_chunks(1 << 16)
                .map(|c| {
                    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                    for &x in c {
                        if x.is_finite() {
                            lo = lo.min(x);
                            hi = hi.max(x);
                        }
                    }
                    (lo, hi)
                })
                .reduce(|| (f64::INFINITY, f64::NEG_INFINITY), |a, b| (a.0.min(b.0), a.1.max(b.1)));
            if lo < hi { (lo, 1.0 / (hi - lo)) } else { (lo, 0.0) }
        })
        .collect()
}

/// The scaled value of band `j` at cell `i` (NaN for none).
#[inline(always)]
fn value(l: &Layers, sc: &[(f64, f64)], j: usize, i: usize) -> f64 {
    let v = l.data[j][i];
    if v.is_finite() { (v - sc[j].0) * sc[j].1 } else { f64::NAN }
}

/// The way a cell's neighbours are walked: the offsets (row, column) of the 4 or the 8, in the order of the cells.
fn around(eight: bool) -> &'static [(i64, i64)] {
    if eight {
        &[(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]
    } else {
        &[(-1, 0), (0, -1), (0, 1), (1, 0)]
    }
}

impl Regions {
    fn empty(nb: usize, manhattan: bool, count: usize) -> Regions {
        Regions {
            nb,
            manhattan,
            cells: vec![0; count],
            sum: vec![0.0; count * nb],
            cnt: vec![0; count * nb],
            mean: vec![f64::NAN; count * nb],
            adj: vec![Adj::new(); count],
            top: vec![Edge { nb: INVALID, cost: 0.0 }; count],
            scratch: Vec::new(),
        }
    }

    /// Every cell with a value a region of its own, numbered in the order of the cells; the cells' labels with them.
    /// All in parallel but the numbering. `zone` (empty for none) keeps a region from crossing the zones.
    fn from_cells(l: &Layers, sc: &[(f64, f64)], zone: &[u64], eight: bool, manhattan: bool) -> (Regions, Vec<u32>) {
        let (w, h) = (l.grid.width, l.grid.height);
        let nb = l.data.len();
        let n = w * h;
        let valid: Vec<bool> = (0..n).into_par_iter().map(|i| l.data.iter().any(|b| b[i].is_finite())).collect();
        let mut labels = vec![0u32; n];
        let mut cell_of: Vec<u32> = Vec::new();
        for i in 0..n {
            if valid[i] {
                cell_of.push(i as u32);
                labels[i] = cell_of.len() as u32;
            }
        }
        let mut g = Regions::empty(nb, manhattan, cell_of.len());
        let nbm = nb.max(1);
        let lab = &labels;
        let offs = around(eight);
        {
            let Regions { cells, sum, cnt, mean, adj, .. } = &mut g;
            mean.par_chunks_mut(nbm)
                .zip(sum.par_chunks_mut(nbm))
                .zip(cnt.par_chunks_mut(nbm))
                .zip(adj.par_iter_mut().zip(cells.par_iter_mut()))
                .enumerate()
                .for_each(|(k, (((m, s), c), (a, cl)))| {
                    let i = cell_of[k] as usize;
                    let (r, col) = ((i / w) as i64, (i % w) as i64);
                    *cl = 1;
                    for j in 0..nb {
                        let v = value(l, sc, j, i);
                        if !v.is_nan() {
                            (m[j], s[j], c[j]) = (v, v, 1);
                        }
                    }
                    for &(dr, dc) in offs {
                        let (rr, cc) = (r + dr, col + dc);
                        if rr < 0 || cc < 0 || rr >= h as i64 || cc >= w as i64 {
                            continue;
                        }
                        let j = rr as usize * w + cc as usize;
                        if lab[j] != 0 && (zone.is_empty() || zone[i] == zone[j]) {
                            a.push(Edge { nb: lab[j] - 1, cost: 0.0 });
                        }
                    }
                });
        }
        (g, labels)
    }

    /// The regions of labels (0 none, 1..=count) given: the sums by one pass over the cells, the lists by the pairs of
    /// neighbouring cells of two regions.
    fn from_labels(
        l: &Layers,
        sc: &[(f64, f64)],
        labels: &[u32],
        count: usize,
        zone: &[u64],
        eight: bool,
        manhattan: bool,
    ) -> Regions {
        let (w, h) = (l.grid.width, l.grid.height);
        let nb = l.data.len();
        let mut g = Regions::empty(nb, manhattan, count);
        // the pairs to the right and below (and across, with 8): a list is sorted and made unique afterwards
        let fwd: &[(i64, i64)] = if eight { &[(0, 1), (1, -1), (1, 0), (1, 1)] } else { &[(0, 1), (1, 0)] };
        for i in 0..w * h {
            let a = labels[i];
            if a == 0 {
                continue;
            }
            let k = a as usize - 1;
            g.cells[k] += 1;
            for j in 0..nb {
                let v = value(l, sc, j, i);
                if !v.is_nan() {
                    g.sum[k * nb + j] += v;
                    g.cnt[k * nb + j] += 1;
                }
            }
            let (r, c) = ((i / w) as i64, (i % w) as i64);
            for &(dr, dc) in fwd {
                let (rr, cc) = (r + dr, c + dc);
                if rr >= h as i64 || cc < 0 || cc >= w as i64 {
                    continue;
                }
                let j = rr as usize * w + cc as usize;
                let o = labels[j];
                if o != 0 && o != a && (zone.is_empty() || zone[i] == zone[j]) {
                    g.adj[k].push(Edge { nb: o - 1, cost: 0.0 });
                    g.adj[o as usize - 1].push(Edge { nb: a - 1, cost: 0.0 });
                }
            }
        }
        for (s, (c, m)) in g.sum.iter().zip(g.cnt.iter().zip(g.mean.iter_mut())) {
            if *c > 0 {
                *m = s / *c as f64;
            }
        }
        for a in g.adj.iter_mut() {
            a.sort_unstable_by_key(|e| e.nb);
            a.dedup_by_key(|e| e.nb);
        }
        g
    }

    /// Every pair's distance worked out, and each region's least, the regions in parallel (a pair twice, to the same
    /// bits: the gaps are symmetric).
    fn eval_all(&mut self) {
        let (mean, nb, man) = (&self.mean, self.nb, self.manhattan);
        self.adj.par_iter_mut().zip(self.top.par_iter_mut()).enumerate().for_each(|(a, (l, top))| {
            *top = Edge { nb: EMPTY, cost: f32::INFINITY };
            for e in l.iter_mut() {
                e.cost = distance(mean, nb, man, a, e.nb as usize);
                if top.nb == EMPTY || e.cost < top.cost {
                    *top = *e;
                }
            }
        });
    }

    /// Prefetch what a visit to region `a` reads first: its list (the part in the region) and its means.
    #[inline]
    fn prefetch_region(&self, a: usize) {
        let l = &self.adj[a];
        prefetch(l as *const Adj);
        prefetch((l as *const Adj).wrapping_byte_add(64));
        prefetch(self.mean.as_ptr().wrapping_add(a * self.nb));
    }

    /// The part of the list of `a` that did not fit in the region.
    #[inline]
    fn prefetch_heap(&self, a: usize) {
        let l = &self.adj[a];
        if l.spilled() {
            prefetch(l.as_ptr());
        }
    }

    /// Prefetch what the distance to each neighbour of region `a` reads.
    #[inline]
    fn prefetch_neighbours(&self, a: usize, stamp: &[u32]) {
        for e in &self.adj[a] {
            let b = e.nb as usize;
            prefetch(self.mean.as_ptr().wrapping_add(b * self.nb));
            prefetch(&self.adj[b]);
            prefetch(&stamp[b]);
        }
    }

    #[inline]
    fn prefetch_neighbour_heaps(&self, a: usize) {
        for e in &self.adj[a] {
            self.prefetch_heap(e.nb as usize);
        }
    }

    /// The neighbour of `a` of the least distance over the whole list.
    #[inline]
    fn top_of(&mut self, a: usize) -> Edge {
        let t = self.top[a];
        if t.nb != INVALID {
            return t;
        }
        let mut top = Edge { nb: EMPTY, cost: f32::INFINITY };
        for e in &self.adj[a] {
            if top.nb == EMPTY || e.cost < top.cost {
                top = *e;
            }
        }
        self.top[a] = top;
        top
    }

    /// The neighbour of `a` of the least distance among those not merged in this pass (the lowest number on a tie):
    /// the neighbour and its distance, and whether any was left out (the least of all was, so the list was read).
    fn best(&mut self, a: usize, stamp: &[u32], pass: u32) -> (Option<(u32, f32)>, bool) {
        let t = self.top_of(a);
        if t.nb == EMPTY {
            return (None, false);
        }
        if stamp[t.nb as usize] != pass {
            return (Some((t.nb, t.cost)), false);
        }
        let mut best: Option<(u32, f32)> = None;
        for e in &self.adj[a] {
            if stamp[e.nb as usize] != pass && best.is_none_or(|(_, g)| e.cost < g) {
                best = Some((e.nb, e.cost));
            }
        }
        (best, true)
    }

    /// Merge region `b` into `a` (they touch).
    fn merge(&mut self, a: usize, b: usize) {
        let nb = self.nb;
        self.cells[a] += self.cells[b];
        for j in 0..nb {
            let (x, y) = (a * nb + j, b * nb + j);
            self.sum[x] += self.sum[y];
            self.cnt[x] += self.cnt[y];
            self.mean[x] = if self.cnt[x] > 0 { self.sum[x] / self.cnt[x] as f64 } else { f64::NAN };
        }
        let lb = std::mem::take(&mut self.adj[b]);
        let mut merged = std::mem::take(&mut self.scratch);
        merged.clear();
        let (a32, b32) = (a as u32, b as u32);
        // the lists joined, the two of them left out
        {
            let la = &self.adj[a];
            let (mut i, mut j) = (0, 0);
            while i < la.len() || j < lb.len() {
                if i < la.len() && la[i].nb == b32 {
                    i += 1;
                } else if j < lb.len() && lb[j].nb == a32 {
                    j += 1;
                } else if j == lb.len() || (i < la.len() && la[i].nb < lb[j].nb) {
                    merged.push(la[i]);
                    i += 1;
                } else if i == la.len() || lb[j].nb < la[i].nb {
                    merged.push(lb[j]);
                    j += 1;
                } else {
                    merged.push(la[i]);
                    i += 1;
                    j += 1;
                }
            }
        }
        // each neighbour of the two now points to a (b gone), at its new distance, which may change its least
        for e in merged.iter_mut() {
            let x = e.nb as usize;
            e.cost = distance(&self.mean, nb, self.manhattan, a, x);
            let c = e.cost;
            let l = &mut self.adj[x];
            match (l.binary_search_by_key(&a32, |y| y.nb), l.binary_search_by_key(&b32, |y| y.nb)) {
                (Ok(i), Ok(j)) => {
                    l[i].cost = c;
                    l.remove(j);
                }
                (Ok(i), Err(_)) => l[i].cost = c,
                (Err(_), Ok(j)) => {
                    l.remove(j);
                    let at = l.binary_search_by_key(&a32, |y| y.nb).unwrap_or_else(|p| p);
                    l.insert(at, Edge { nb: a32, cost: c });
                }
                (Err(_), Err(_)) => unreachable!("a neighbour of the merged regions lists neither"),
            }
            let t = &mut self.top[x];
            if t.nb == b32 || (t.nb == a32 && c > t.cost) {
                t.nb = INVALID;
            } else if t.nb != INVALID && (t.nb == EMPTY || c < t.cost || (c == t.cost && a32 < t.nb)) {
                *t = Edge { nb: a32, cost: c };
            } else if t.nb == a32 {
                t.cost = c;
            }
        }
        self.top[a].nb = INVALID;
        let la = &mut self.adj[a];
        la.clear();
        la.extend_from_slice(&merged);
        self.scratch = merged;
    }
}

/// splitmix64: the order of the visits.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// What the merging is told.
struct Plan {
    /// The distance (squared for Euclidean) at or under which regions merge.
    limit: f32,
    mutual: bool,
    seed: u64,
    max_passes: u32,
    minsize: u32,
}

/// Region `x` is to be visited again: no longer quiet, and in the list of the next pass (once).
#[inline]
fn wake(x: u32, quiet: &mut [bool], queued: &mut [bool], active: &mut Vec<u32>) {
    quiet[x as usize] = false;
    if !queued[x as usize] {
        queued[x as usize] = true;
        active.push(x);
    }
}

/// The regions merged: each region's root, the region it merged into. A pass visits only the regions that may do
/// something: one whose visit would be the same as the last (nothing close enough, or its best neighbour is not
/// mutual, with no neighbour left out by a merge of the pass) waits until it or a neighbour changes, or the best
/// neighbour it waits on gets another best.
fn merge_all(g: &mut Regions, p: &Plan) -> Vec<u32> {
    let n = g.cells.len();
    let mut parent: Vec<u32> = (0..n as u32).collect();
    let mut dead = vec![false; n];
    let mut quiet = vec![false; n];
    let mut queued = vec![true; n];
    let mut active: Vec<u32> = (0..n as u32).collect();
    let mut stamp = vec![0u32; n];
    g.eval_all();
    let mut rng = Rng(p.seed ^ 0xA5A5_5A5A_1234_5678);
    let mut pass = 0u32;
    while pass < p.max_passes {
        pass += 1;
        let mut order = std::mem::take(&mut active);
        for &r in &order {
            queued[r as usize] = false;
        }
        order.retain(|&r| !dead[r as usize] && !quiet[r as usize]);
        if order.is_empty() {
            break;
        }
        for i in (1..order.len()).rev() {
            order.swap(i, (rng.next() % (i as u64 + 1)) as usize);
        }
        let mut merges = 0usize;
        for (at, &a) in order.iter().enumerate() {
            // the regions visited soon are fetched while this one is worked on
            let soon = |k: usize| order.get(at + k * AHEAD).map(|&x| x as usize);
            if let Some(x) = soon(4) {
                g.prefetch_region(x);
            }
            if let Some(x) = soon(3) {
                g.prefetch_heap(x);
            }
            if let Some(x) = soon(2) {
                g.prefetch_neighbours(x, &stamp);
            }
            if let Some(x) = soon(1) {
                g.prefetch_neighbour_heaps(x);
            }
            let ai = a as usize;
            if dead[ai] || stamp[ai] == pass {
                continue;
            }
            let (best, left_out) = g.best(ai, &stamp, pass);
            let Some((b, _)) = best.filter(|&(_, f)| f <= p.limit) else {
                // nothing close enough, and nothing left out: it stays so until it or a neighbour changes
                if left_out {
                    wake(a, &mut quiet, &mut queued, &mut active);
                } else {
                    quiet[ai] = true;
                }
                continue;
            };
            let b = b as usize;
            if p.mutual {
                let (theirs, left_b) = g.best(b, &stamp, pass);
                if theirs.is_none_or(|(c, _)| c as usize != ai) {
                    // not mutual: the same next time unless a merge changes what either sees
                    if left_out || left_b {
                        wake(a, &mut quiet, &mut queued, &mut active);
                    } else {
                        quiet[ai] = true;
                    }
                    continue;
                }
            }
            g.merge(ai, b);
            dead[b] = true;
            parent[b] = a;
            stamp[ai] = pass;
            stamp[b] = pass;
            // the merged region and its neighbours have changed, and so has each one's least distance. A neighbour
            // with a least distance over the limit has nothing to do whatever it is; one under it looks again, and so
            // does the one it names (which may wait on it: it is its least, and was not its own)
            wake(a, &mut quiet, &mut queued, &mut active);
            let least = g.top_of(ai);
            if least.nb < EMPTY && least.cost <= p.limit {
                wake(least.nb, &mut quiet, &mut queued, &mut active);
            }
            for i in 0..g.adj[ai].len() {
                let x = g.adj[ai][i].nb;
                let least = g.top_of(x as usize);
                if least.nb < EMPTY && least.cost <= p.limit {
                    wake(x, &mut quiet, &mut queued, &mut active);
                    wake(least.nb, &mut quiet, &mut queued, &mut active);
                }
            }
            merges += 1;
        }
        if merges == 0 {
            break;
        }
    }
    if p.minsize > 1 {
        // the regions under minsize, in the order of the numbers, each into its most similar neighbour whatever the
        // distance (a region still under minsize after it takes its turn again)
        let mut small: Vec<u32> =
            (0..n as u32).filter(|&r| !dead[r as usize] && g.cells[r as usize] < p.minsize).collect();
        small.reverse();
        stamp.fill(0);
        while let Some(a) = small.pop() {
            let a = a as usize;
            if dead[a] || g.cells[a] >= p.minsize {
                continue;
            }
            // (no pass is u32::MAX: nothing is left out)
            let Some((b, _)) = g.best(a, &stamp, u32::MAX).0.filter(|&(_, f)| f.is_finite()).or_else(|| {
                // no band in common with any neighbour: the neighbour with the most cells
                g.adj[a].iter().map(|e| (e.nb, 0.0f32)).max_by_key(|&(b, _)| (g.cells[b as usize], u32::MAX - b))
            }) else {
                continue;
            };
            let b = b as usize;
            g.merge(b, a);
            dead[a] = true;
            parent[a] = b as u32;
            if g.cells[b] < p.minsize {
                small.push(b as u32);
            }
        }
    }
    (0..n as u32).map(|r| find(&mut parent, r)).collect()
}

/// The bits of a number as a zone or a seed: the same for equal numbers (-0 and 0, any NaN).
fn bits(v: f64) -> u64 {
    if v.is_nan() {
        0x7ff8_0000_0000_0000
    } else if v == 0.0 {
        0
    } else {
        v.to_bits()
    }
}

/// `segment.region_growing`.
struct RegionGrowing;

impl Operator for RegionGrowing {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.region_growing",
            "Objects by region growing and merging, the method of GRASS GIS's i.segment (Momsen & Metz). Regions start \
             as single cells (or as the segments of `seeds`) and merge, pass after pass. Each band is scaled to 0-1 by \
             its range over the whole image (raw values are not used: the threshold has the same meaning for any \
             data) and two regions are as far apart as their means: the Euclidean distance divided by the square \
             root of the bands, or the Manhattan one divided by the bands, so 0 is identical and 1 the widest apart \
             (a band where either has no value is left out). In a pass the regions are visited in a pseudo-random \
             order (the seed's); one merges with its most similar neighbour when the distance is at or under the \
             threshold and, by default, the region is that neighbour's most similar as well (`mutual`), and a \
             region that merged is left alone for the rest of the pass. It stops when a pass merges nothing (or at \
             `iterations`). Then each region under `minsize` cells joins its \
             most similar neighbour, whatever the distance, again and again until none is left under it (GRASS makes \
             one such pass). A threshold of 0 joins only identical neighbours, 1 everything. `bounds`: a map whose \
             zones (cells of one value) no object crosses; `seeds`: objects or a map to start from, each group of \
             connected cells of one value being one start region, every other cell a region of its own. Neighbours \
             are the 4 cells around (GRASS's default) or the 8. Differences from GRASS: the shape terms GRASS lists \
             as future work are not there; a distance equal to the threshold merges, so the endpoints 0 and 1 mean \
             what GRASS's manual says of them; a cell with a value in some bands only counts in those; the passes \
             run until none merges (no limit) unless `iterations` is given. A cell without a value in every band \
             is no object. The result is the same on every run, with the same seed, on any number of threads.",
            Access::Global,
        )
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands, the colour"))
        .input(PortSpec::optional(
            "seeds",
            Type::Any,
            "objects, or a map, to start from: a group of connected cells of one value (of one object) is one start \
             region; cells without a value in the map are single-cell regions",
        ))
        .input(PortSpec::optional(
            "bounds",
            Type::Map,
            "a map of zones (cells of one value): no object crosses from one zone to another; cells without a value \
             in it are one zone together",
        ))
        .output(PortSpec::new("objects", Type::Objects, "the segments, each one connected"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per object: id, cells, area (map units squared), and mean_<band> and sd_<band> for each band",
        ))
        .param(PortSpec::new(
            "threshold",
            Type::Number,
            "0 to 1: regions merge while their distance (of the means scaled to 0-1) is at or under it; 0.01 to 0.05 \
             is a reasonable start",
        ))
        .param(
            PortSpec::optional("similarity", Type::Text, "the distance: euclidean (default) or manhattan")
                .choices(["euclidean", "manhattan"]),
        )
        .param(PortSpec::optional(
            "mutual",
            Type::Bool,
            "merge only when each is the other's most similar neighbour (default true, as GRASS)",
        ))
        .param(PortSpec::optional(
            "minsize",
            Type::Integer,
            "the least cells of an object: smaller ones join their most similar neighbour at the end (default 1)",
        ))
        .param(PortSpec::optional(
            "iterations",
            Type::Integer,
            "the most passes (default: until a pass merges nothing)",
        ))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of the order of the visits (default 0)"))
        .param(PortSpec::optional("neighbours", Type::Integer, "which cells touch: 4 (default) or 8").choices([4, 8]))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let threshold =
            args.opt_param::<f64>("threshold")?.ok_or_else(|| bad("the setting threshold is needed".into()))?;
        if !(0.0..=1.0).contains(&threshold) {
            return Err(bad(format!("threshold {threshold}: 0 to 1")));
        }
        let similarity: String = args.param("similarity", "euclidean".to_string())?;
        let manhattan = match similarity.as_str() {
            "euclidean" => false,
            "manhattan" => true,
            s => return Err(bad(format!("similarity {s:?}: euclidean or manhattan"))),
        };
        let mutual: bool = args.param("mutual", true)?;
        let minsize: i64 = args.param("minsize", 1)?;
        if minsize < 1 {
            return Err(bad(format!("minsize {minsize}: 1 or more")));
        }
        let iterations = args.opt_param::<i64>("iterations")?;
        if let Some(k) = iterations
            && k < 1
        {
            return Err(bad(format!("iterations {k}: 1 or more")));
        }
        let seed: i64 = args.param("seed", 0)?;
        let nbs: i64 = args.param("neighbours", 4)?;
        if nbs != 4 && nbs != 8 {
            return Err(bad(format!("neighbours {nbs}: 4 or 8")));
        }
        let l = crate::raster_stats::layers(args, "image")?;
        if l.grid.width * l.grid.height >= 1 << 31 {
            return Err(bad(
                "an image of 2^31 cells or more is more than segment.region_growing works in memory".into()
            ));
        }
        super::warnings::image(ctx, node, &l.grid, &l.names, &l.data, false);
        let n = l.grid.width * l.grid.height;
        let eight = nbs == 8;
        let zone: Vec<u64> = match args.get("bounds") {
            None => Vec::new(),
            Some(Value::Map(m)) => {
                if !m.grid.same_as(&l.grid) {
                    return Err(bad("bounds: a map of another grid than the image".into()));
                }
                m.data.iter().map(|&v| bits(v)).collect()
            }
            Some(v) => return Err(bad(format!("bounds: a {:?}, not a map", v.ty()))),
        };
        // the key of the seed segment of each cell (none: u64::MAX)
        let seeds: Option<Vec<u64>> = match args.get("seeds") {
            None => None,
            Some(Value::Objects(o)) => {
                if !o.grid.same_as(&l.grid) {
                    return Err(bad("seeds: objects of another grid than the image".into()));
                }
                Some(o.labels.iter().map(|&id| if id == 0 { u64::MAX } else { id as u64 }).collect())
            }
            Some(Value::Map(m)) => {
                if !m.grid.same_as(&l.grid) {
                    return Err(bad("seeds: a map of another grid than the image".into()));
                }
                Some(m.data.iter().map(|&v| if v.is_nan() { u64::MAX } else { bits(v) }).collect())
            }
            Some(v) => return Err(bad(format!("seeds: a {:?}, not objects or a map", v.ty()))),
        };
        let sc = scales(&l);
        let (mut g, labels) = match &seeds {
            None => Regions::from_cells(&l, &sc, &zone, eight, manhattan),
            Some(s) => {
                // each group of connected cells of one seed (and one zone) a start region; the other cells one each
                let any = |i: usize| l.data.iter().any(|b| b[i].is_finite());
                let key = |i: usize| -> (u64, u64) {
                    if !any(i) {
                        (u64::MAX, u64::MAX)
                    } else if s[i] == u64::MAX {
                        (i as u64, u64::MAX)
                    } else {
                        (s[i], if zone.is_empty() { 0 } else { zone[i] })
                    }
                };
                let start = Objects::from_fn(l.grid.clone(), key, (u64::MAX, u64::MAX), nbs as usize);
                let labels: Vec<u32> = start.labels[..].to_vec();
                (Regions::from_labels(&l, &sc, &labels, start.count, &zone, eight, manhattan), labels)
            }
        };
        let limit = if manhattan { threshold } else { threshold * threshold } as f32;
        let plan = Plan {
            limit,
            mutual,
            seed: seed as u64,
            max_passes: iterations.map_or(u32::MAX, |k| k.min(u32::MAX as i64) as u32),
            minsize: minsize.min(u32::MAX as i64) as u32,
        };
        let root = merge_all(&mut g, &plan);
        drop(g);
        let o = Objects::from_fn(
            l.grid.clone(),
            |i| match labels[i] {
                0 => NO_REGION,
                a => root[a as usize - 1] as u64,
            },
            NO_REGION,
            nbs as usize,
        );
        debug_assert_eq!(labels.len(), n);
        let t = super::stats_table(&o, &l.names, &l.data);
        let o = o.with_table(t.clone())?;
        Ok(outputs([("objects", Value::Objects(Arc::new(o))), ("table", Value::Table(t))]))
    }
}
