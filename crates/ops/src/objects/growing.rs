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
//!
//! With `tile` the image is cut into tiles (the machinery of `tiles.rs`, as multiresolution does): each tile, with a
//! margin around it, is grown on its own, all tiles in parallel, with the scaling of the whole image and `minsize` 1;
//! the cores of the tiles are the pieces, and the ordinary merging over the whole image starts from them (the path of
//! `seeds`), joins the pieces of one seed, and applies `minsize`. The pieces that touch a tile border start as single
//! cells (see `tiles.rs`), so they grow across it and no seam is left.

use std::sync::Arc;

use rayon::prelude::*;
use smallvec::SmallVec;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;

use super::big::{Layered, Source};
use super::superpixels::prefetch;
use super::tiles::{Tile, margin_of, merge_tiled_with, tile_seed, tile_side};
use super::zones::{bits, zones};
use super::{NO_REGION, Objects, find};
use crate::raster_stats::Layers;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(RegionGrowing));
}

/// A region's nearest neighbour: its number and the distance to it.
#[derive(Clone, Copy)]
struct Edge {
    nb: u32,
    cost: f32,
}

/// `Regions::top` before it is known, and for a region with no neighbour.
const INVALID: u32 = u32::MAX;
const EMPTY: u32 = u32::MAX - 1;

/// Neighbours' numbers kept in the region itself (a cell has 4 or 8): no heap per region. The distances are not kept
/// in the lists (a merge changes a region's distance to every neighbour, and a list per neighbour would be written):
/// they are worked out from the means, which is cheap, and each region keeps its nearest.
type Adj = SmallVec<[u32; 8]>;

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
    /// The joined list of a merge, each neighbour with where it came from (1 the first region's list, 2 the second's,
    /// 3 both), kept to reuse its memory.
    scratch: Vec<(u32, u8)>,
    /// The regions the last merge leaves to be visited again, in the order to wake them.
    woken: Vec<u32>,
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
    ranges_of(&l.data).iter().map(|r| scale_of(*r)).collect()
}

/// The scaling of a band from its range `[lowest, highest]`.
fn scale_of([lo, hi]: [f64; 2]) -> (f64, f64) {
    if lo < hi { (lo, 1.0 / (hi - lo)) } else { (lo, 0.0) }
}

/// The range of each band, its lowest and highest value with a value (infinite and reversed for a band with none).
pub(super) fn ranges_of(data: &[Arc<Vec<f64>>]) -> Vec<[f64; 2]> {
    data.par_iter()
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
            [lo, hi]
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
            woken: Vec::new(),
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
                            a.push(lab[j] - 1);
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
                    g.adj[k].push(o - 1);
                    g.adj[o as usize - 1].push(a - 1);
                }
            }
        }
        for (s, (c, m)) in g.sum.iter().zip(g.cnt.iter().zip(g.mean.iter_mut())) {
            if *c > 0 {
                *m = s / *c as f64;
            }
        }
        for a in g.adj.iter_mut() {
            a.sort_unstable();
            a.dedup();
        }
        g
    }

    /// Every pair's distance worked out, and each region's least, the regions in parallel (a pair twice, to the same
    /// bits: the gaps are symmetric).
    fn eval_all(&mut self) {
        let (mean, nb, man) = (&self.mean, self.nb, self.manhattan);
        self.adj.par_iter_mut().zip(self.top.par_iter_mut()).enumerate().for_each(|(a, (l, top))| {
            *top = Edge { nb: EMPTY, cost: f32::INFINITY };
            for &x in l.iter() {
                let cost = distance(mean, nb, man, a, x as usize);
                if top.nb == EMPTY || cost < top.cost {
                    *top = Edge { nb: x, cost };
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

    /// The neighbour of `a` of the least distance over the whole list.
    #[inline]
    fn top_of(&mut self, a: usize) -> Edge {
        let t = self.top[a];
        if t.nb != INVALID {
            return t;
        }
        let mut top = Edge { nb: EMPTY, cost: f32::INFINITY };
        for &x in &self.adj[a] {
            let cost = distance(&self.mean, self.nb, self.manhattan, a, x as usize);
            if top.nb == EMPTY || cost < top.cost {
                top = Edge { nb: x, cost };
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
        for &x in &self.adj[a] {
            if stamp[x as usize] != pass {
                let cost = distance(&self.mean, self.nb, self.manhattan, a, x as usize);
                if best.is_none_or(|(_, g)| cost < g) {
                    best = Some((x, cost));
                }
            }
        }
        (best, true)
    }

    /// Merge region `b` into `a` (they touch). One pass over the joined list works out `a`'s distance to each
    /// neighbour and so its nearest; only the neighbours of `b` have their lists changed (b becomes a), and a neighbour's
    /// nearest changes only when it was `a` or `b`, or `a` is now nearer. The neighbours left to look again (their
    /// nearest within `limit`, and that nearest) are put in `woken`.
    fn merge(&mut self, a: usize, b: usize, limit: f32) {
        let nb = self.nb;
        self.cells[a] += self.cells[b];
        for j in 0..nb {
            let (x, y) = (a * nb + j, b * nb + j);
            self.sum[x] += self.sum[y];
            self.cnt[x] += self.cnt[y];
            self.mean[x] = if self.cnt[x] > 0 { self.sum[x] / self.cnt[x] as f64 } else { f64::NAN };
        }
        let lb = std::mem::take(&mut self.adj[b]);
        let la = std::mem::take(&mut self.adj[a]);
        let mut merged = std::mem::take(&mut self.scratch);
        merged.clear();
        let (a32, b32) = (a as u32, b as u32);
        // the lists joined, the two regions left out
        {
            let (mut i, mut j) = (0, 0);
            while i < la.len() || j < lb.len() {
                if i < la.len() && la[i] == b32 {
                    i += 1;
                } else if j < lb.len() && lb[j] == a32 {
                    j += 1;
                } else if j == lb.len() || (i < la.len() && la[i] < lb[j]) {
                    merged.push((la[i], 1));
                    i += 1;
                } else if i == la.len() || lb[j] < la[i] {
                    merged.push((lb[j], 2));
                    j += 1;
                } else {
                    merged.push((la[i], 3));
                    i += 1;
                    j += 1;
                }
            }
        }
        self.woken.clear();
        let mut least = Edge { nb: EMPTY, cost: f32::INFINITY };
        const AHEAD: usize = 8;
        for i in 0..merged.len() {
            if let Some(&(y, _)) = merged.get(i + 2 * AHEAD) {
                prefetch(&self.top[y as usize]);
                prefetch(&self.adj[y as usize]);
            }
            if let Some(&(y, _)) = merged.get(i + AHEAD) {
                let yu = y as usize;
                prefetch(self.mean.as_ptr().wrapping_add(yu * nb));
                // a neighbour whose nearest was `a` or `b` has its list read again (its nearest is worked out anew):
                // the means of what it touches are fetched together
                let t = self.top[yu].nb;
                if t == a32 || t == b32 {
                    for &z in &self.adj[yu] {
                        prefetch(self.mean.as_ptr().wrapping_add(z as usize * nb));
                    }
                }
            }
            let (x, tag) = merged[i];
            let xu = x as usize;
            let c = distance(&self.mean, nb, self.manhattan, a, xu);
            if least.nb == EMPTY || c < least.cost {
                least = Edge { nb: x, cost: c };
            }
            if tag & 2 != 0 {
                // a neighbour of b now points to a (b gone)
                let l = &mut self.adj[xu];
                if let Ok(j) = l.binary_search(&b32) {
                    l.remove(j);
                }
                if tag == 2 {
                    let at = l.binary_search(&a32).unwrap_or_else(|p| p);
                    l.insert(at, a32);
                }
            }
            let t = &mut self.top[xu];
            if t.nb == b32 || (t.nb == a32 && c > t.cost) {
                t.nb = INVALID;
            } else if t.nb != INVALID && (t.nb == EMPTY || c < t.cost || (c == t.cost && a32 < t.nb)) {
                *t = Edge { nb: a32, cost: c };
            } else if t.nb == a32 {
                t.cost = c;
            }
            let t = self.top_of(xu);
            if t.nb < EMPTY && t.cost <= limit {
                self.woken.push(x);
                self.woken.push(t.nb);
            }
        }
        self.top[a] = least;
        let mut la = la;
        la.clear();
        la.extend(merged.iter().map(|m| m.0));
        self.adj[a] = la;
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

/// A region's flags in the passes (one byte, so a visit reads one place): merged away, nothing to do until something
/// changes, in the list of the next pass.
const DEAD: u8 = 1;
const QUIET: u8 = 2;
const QUEUED: u8 = 4;

/// Region `x` is to be visited again: no longer quiet, and in the list of the next pass (once).
#[inline]
fn wake(x: u32, flags: &mut [u8], active: &mut Vec<u32>) {
    let f = &mut flags[x as usize];
    *f &= !QUIET;
    if *f & QUEUED == 0 {
        *f |= QUEUED;
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
    let mut flags = vec![QUEUED; n];
    let mut active: Vec<u32> = (0..n as u32).collect();
    let mut stamp = vec![0u32; n];
    g.eval_all();
    let mut rng = Rng(p.seed ^ 0xA5A5_5A5A_1234_5678);
    let mut pass = 0u32;
    while pass < p.max_passes {
        pass += 1;
        let mut order = std::mem::take(&mut active);
        for &r in &order {
            flags[r as usize] &= !QUEUED;
        }
        order.retain(|&r| flags[r as usize] & (DEAD | QUIET) == 0);
        if order.is_empty() {
            break;
        }
        for i in (1..order.len()).rev() {
            order.swap(i, (rng.next() % (i as u64 + 1)) as usize);
        }
        let mut merges = 0usize;
        for (at, &a) in order.iter().enumerate() {
            // the regions visited soon are fetched while this one is worked on: a visit reads the region's flags,
            // stamp and nearest, then the nearest's, then the stamp of the nearest's nearest (a shuffle: every read
            // would wait for memory)
            let soon = |k: usize| order.get(at + k * AHEAD).map(|&x| x as usize);
            if let Some(x) = soon(3) {
                prefetch(&g.top[x]);
                prefetch(&flags[x]);
                prefetch(&stamp[x]);
            }
            if let Some(x) = soon(2) {
                let b = g.top[x].nb;
                if b < EMPTY {
                    prefetch(&g.top[b as usize]);
                    prefetch(&stamp[b as usize]);
                }
            }
            if let Some(x) = soon(1) {
                let b = g.top[x].nb;
                if b < EMPTY {
                    let c = g.top[b as usize].nb;
                    if c < EMPTY {
                        prefetch(&stamp[c as usize]);
                    }
                    g.prefetch_region(b as usize);
                }
            }
            let ai = a as usize;
            if flags[ai] & DEAD != 0 || stamp[ai] == pass {
                continue;
            }
            let (best, left_out) = g.best(ai, &stamp, pass);
            let Some((b, _)) = best.filter(|&(_, f)| f <= p.limit) else {
                // nothing close enough, and nothing left out: it stays so until it or a neighbour changes
                if left_out {
                    wake(a, &mut flags, &mut active);
                } else {
                    flags[ai] |= QUIET;
                }
                continue;
            };
            let b = b as usize;
            if p.mutual {
                let (theirs, left_b) = g.best(b, &stamp, pass);
                if theirs.is_none_or(|(c, _)| c as usize != ai) {
                    // not mutual: the same next time unless a merge changes what either sees
                    if left_out || left_b {
                        wake(a, &mut flags, &mut active);
                    } else {
                        flags[ai] |= QUIET;
                    }
                    continue;
                }
            }
            g.merge(ai, b, p.limit);
            flags[b] |= DEAD;
            parent[b] = a;
            stamp[ai] = pass;
            stamp[b] = pass;
            // the merged region and its neighbours have changed, and so has each one's least distance. A neighbour
            // with a least distance over the limit has nothing to do whatever it is; one under it looks again, and so
            // does the one it names (which may wait on it: it is its least, and was not its own)
            wake(a, &mut flags, &mut active);
            let least = g.top[ai];
            if least.nb < EMPTY && least.cost <= p.limit {
                wake(least.nb, &mut flags, &mut active);
            }
            for k in 0..g.woken.len() {
                wake(g.woken[k], &mut flags, &mut active);
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
            (0..n as u32).filter(|&r| flags[r as usize] & DEAD == 0 && g.cells[r as usize] < p.minsize).collect();
        small.reverse();
        stamp.fill(0);
        while let Some(a) = small.pop() {
            let a = a as usize;
            if flags[a] & DEAD != 0 || g.cells[a] >= p.minsize {
                continue;
            }
            // (no pass is u32::MAX: nothing is left out)
            let Some((b, _)) = g.best(a, &stamp, u32::MAX).0.filter(|&(_, f)| f.is_finite()).or_else(|| {
                // no band in common with any neighbour: the neighbour with the most cells
                g.adj[a].iter().map(|&e| (e, 0.0f32)).max_by_key(|&(b, _)| (g.cells[b as usize], u32::MAX - b))
            }) else {
                continue;
            };
            let b = b as usize;
            g.merge(b, a, f32::NEG_INFINITY);
            flags[a] |= DEAD;
            parent[a] = b as u32;
            if g.cells[b] < p.minsize {
                small.push(b as u32);
            }
        }
    }
    (0..n as u32).map(|r| find(&mut parent, r)).collect()
}

/// The objects of the image `l`: the regions grown from the cells (or from the groups of connected cells of one seed,
/// `seeds`, a key per cell, `u64::MAX` for none) and merged as `plan` says, none crossing the `zone`s (empty for none).
fn grow(
    l: &Layers,
    sc: &[(f64, f64)],
    zone: &[u64],
    seeds: Option<&[u64]>,
    eight: bool,
    manhattan: bool,
    plan: &Plan,
) -> Objects {
    let nbs = if eight { 8 } else { 4 };
    let (mut g, labels) = match seeds {
        None => Regions::from_cells(l, sc, zone, eight, manhattan),
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
            let start = Objects::from_fn(l.grid.clone(), key, (u64::MAX, u64::MAX), nbs);
            let labels: Vec<u32> = start.labels[..].to_vec();
            (Regions::from_labels(l, sc, &labels, start.count, zone, eight, manhattan), labels)
        }
    };
    let root = merge_all(&mut g, plan);
    drop(g);
    from_roots(l, &labels, &root, nbs)
}

/// The objects of the regions' `labels` (0 none) with each region's root.
fn from_roots(l: &Layers, labels: &[u32], root: &[u32], nbs: usize) -> Objects {
    debug_assert_eq!(labels.len(), l.grid.width * l.grid.height);
    Objects::from_fn(
        l.grid.clone(),
        |i| match labels[i] {
            0 => NO_REGION,
            a => root[a as usize - 1] as u64,
        },
        NO_REGION,
        nbs,
    )
}

/// The merging over the whole image that follows the tiles: it starts from their `pieces` (each connected), which
/// are joined first when they hold cells of one seed (a seed cut by a tile border is whole again), and goes on as
/// `plan` says (the threshold, and `minsize`, which the tiles did not apply).
#[allow(clippy::too_many_arguments)]
fn stitch(
    l: &Layers,
    sc: &[(f64, f64)],
    zone: &[u64],
    seeds: Option<&[u64]>,
    pieces: Objects,
    eight: bool,
    manhattan: bool,
    plan: &Plan,
) -> Objects {
    let nbs = if eight { 8 } else { 4 };
    let mut labels: Vec<u32> = pieces.labels[..].to_vec();
    let mut count = pieces.count;
    if let Some(s) = seeds {
        let any = |i: usize| l.data.iter().any(|b| b[i].is_finite());
        let key = |i: usize| -> (u64, u64) {
            if s[i] == u64::MAX || !any(i) {
                (u64::MAX, u64::MAX)
            } else {
                (s[i], if zone.is_empty() { 0 } else { zone[i] })
            }
        };
        let groups = Objects::from_fn(l.grid.clone(), key, (u64::MAX, u64::MAX), nbs);
        // the pieces holding cells of one seed group are one (pieces numbered from 1, the lowest root)
        let mut parent: Vec<u32> = (0..=count as u32).collect();
        let mut first = vec![0u32; groups.count + 1];
        for (i, &p) in labels.iter().enumerate() {
            let g = groups.labels[i] as usize;
            if g == 0 || p == 0 {
                continue;
            }
            if first[g] == 0 {
                first[g] = p;
            } else {
                let (a, b) = (find(&mut parent, first[g]), find(&mut parent, p));
                parent[a.max(b) as usize] = a.min(b);
            }
        }
        let mut number = vec![0u32; count + 1];
        count = 0;
        for p in labels.iter_mut().filter(|p| **p != 0) {
            let r = find(&mut parent, *p) as usize;
            if number[r] == 0 {
                count += 1;
                number[r] = count as u32;
            }
            *p = number[r];
        }
    }
    let mut g = Regions::from_labels(l, sc, &labels, count, zone, eight, manhattan);
    let root = merge_all(&mut g, plan);
    drop(g);
    from_roots(l, &labels, &root, nbs)
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
             one such pass). A threshold of 0 joins only identical neighbours, 1 everything. `bounds`: a map (a \
             zone for each value), vectors (each feature a zone) or objects (each object a zone), that no object \
             crosses; `seeds`: objects or a map to start from, each group of \
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
            Type::Any,
            "a map of zones (cells of one value), vectors (each feature a zone) or objects (each object a zone): no \
             object crosses from one zone to another; cells without a value in the map, in no feature or in no \
             object are one zone together",
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
        .param(PortSpec::optional(
            "ranges",
            Type::Any,
            "a list of [lowest, highest], one for each band: the range the 0-1 scaling uses instead of the image's \
             own (a part of an image, or another date, scaled as the whole)",
        ))
        .param(PortSpec::optional(
            "tile",
            Type::Integer,
            "cells, the side of the tiles the image is cut into, grown in parallel and stitched at their borders (8 or \
             more; default 0: the whole image at once, one thread). No seam shows at the borders (the \
             pieces that touch one are grown again from single cells across it); the objects differ from the run \
             without tiles only as another seed's do",
        ))
        .param(PortSpec::optional(
            "margin",
            Type::Integer,
            "cells, the margin around each tile that is grown with it for the context of its border; 0 (default) is \
             automatic: 64 cells, at most half the tile",
        ))
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
        let tile: i64 = args.param("tile", 0)?;
        if tile < 0 || (tile > 0 && tile < 8) {
            return Err(bad(format!("tile {tile}: 0 (no tiles) or 8 cells or more")));
        }
        let margin: i64 = args.param("margin", 0)?;
        if margin < 0 {
            return Err(bad(format!("margin {margin}: 0 (automatic) or more")));
        }
        let l = crate::raster_stats::layers(args, "image")?;
        if l.grid.width * l.grid.height >= 1 << 31 {
            return Err(bad(
                "an image of 2^31 cells or more is more than segment.region_growing works in memory".into()
            ));
        }
        super::warnings::image(ctx, node, &l.grid, &l.names, &l.data, false);
        let eight = nbs == 8;
        let zone = zones(args, "bounds", &l.grid)?;
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
        let sc = match args.params.get("ranges") {
            None | Some(serde_json::Value::Null) => scales(&l),
            Some(v) => {
                let r: Vec<[f64; 2]> = serde_json::from_value(v.clone())
                    .map_err(|_| bad("ranges: a list of [lowest, highest], one for each band".into()))?;
                if r.len() != l.data.len() || r.iter().any(|b| b.iter().any(|x| !x.is_finite())) {
                    return Err(bad(format!(
                        "ranges: {} finite pairs [lowest, highest], one for each band",
                        l.data.len()
                    )));
                }
                r.into_iter().map(scale_of).collect()
            }
        };
        let plan = Plan {
            limit: if manhattan { threshold } else { threshold * threshold } as f32,
            mutual,
            seed: seed as u64,
            max_passes: iterations.map_or(u32::MAX, |k| k.min(u32::MAX as i64) as u32),
            minsize: minsize.min(u32::MAX as i64) as u32,
        };
        let (w, h) = (l.grid.width, l.grid.height);
        let sd = seeds.as_deref();
        let tiles = if tile > 0 { tile_side(w, h, tile as usize) } else { None };
        let o = match tiles {
            None => grow(&l, &sc, &zone, sd, eight, manhattan, &plan),
            Some(t) => {
                let m = margin_of(t, margin as usize);
                let src = Layered::mem(&l.data, &l.grid);
                let tile_plan = |k: usize| Plan { minsize: 1, seed: tile_seed(plan.seed, k), ..plan };
                let core = |k: usize| -> Result<Vec<u32>> {
                    let tl = Tile::new((w, h), t, m, k);
                    let win = Layers {
                        grid: Arc::new(Grid::new(tl.width(), tl.wr1 - tl.wr0)),
                        names: Vec::new(),
                        data: src.window(tl.wr0, tl.wr1, tl.wc0, tl.wc1)?,
                    };
                    let z = if zone.is_empty() { Vec::new() } else { tl.cut(&zone, w) };
                    let s = sd.map(|s| tl.cut(s, w));
                    let o = grow(&win, &sc, &z, s.as_deref(), eight, manhattan, &tile_plan(k));
                    Ok(tl.core(&o.labels[..]))
                };
                let per_cell = l.data.len() as u64 * 40 + 300;
                merge_tiled_with(&l.grid, t, m, per_cell, nbs as usize, 0, None, &core, |pieces| {
                    Ok(stitch(&l, &sc, &zone, sd, pieces, eight, manhattan, &plan))
                })?
            }
        };
        let t = super::stats_table(&o, &l.names, &l.data);
        let o = o.with_table(t.clone())?;
        Ok(outputs([("objects", Value::Objects(Arc::new(o))), ("table", Value::Table(t))]))
    }
}
