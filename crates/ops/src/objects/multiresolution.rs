//! `segment.multiresolution`: Baatz & Schäpe's (2000) multiresolution segmentation, region merging.
//!
//! Regions start as single cells (or the objects of a level below) and merge, pass after pass, while the fusion cost
//! stays under scale². The state is a region adjacency graph: per region its cells, perimeter, box, and per band
//! (count, sum, sum of squares) of the values, and a sorted list of (neighbour, shared border, cost of the merge). A
//! cost is worked out once and kept until either region changes (from terms cached per region: the n·sd of each band
//! and the shape terms of the region alone, so a cost takes one square root per band for the merged region only). A
//! merge is O(bands) for the cost and O(neighbours) for the lists; nothing per pass costs O(cells). The visits are a
//! shuffle, so every one would wait for memory: the regions of the visits to come are prefetched. A region whose
//! every neighbour costs more than scale² is left alone until it or a neighbour changes (its cost depends on nothing
//! else), so the late passes only visit what can still merge.
//!
//! One thread. Happ et al. (2010) run it in parallel by blocks of the image, with the merges across blocks made
//! apart: that is `tiles.rs`, which uses this for each tile and for the stitching.

use std::sync::Arc;

use smallvec::SmallVec;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;

use super::big::{Layered, Source, strips};
use super::{Labels, NO_REGION, Objects, find};
use crate::{Args, Ctx, Error, Operator, Outputs, Result, Table, Value, outputs};

/// The first pass of a run on rasters on disk as a job for another machine: none for a part of a file (a check run).
#[allow(clippy::too_many_arguments)]
fn tile_job(
    d: &super::big::Image,
    cost: &Cost,
    scale: f64,
    mutual: bool,
    seed: u64,
    max_passes: u32,
    tile: usize,
    margin: usize,
    zones: &[u64],
) -> Option<super::tiles::TileJob> {
    let super::big::Part::Disk(r, _) = d.src.parts.first()? else { return None };
    let layers = d
        .src
        .parts
        .iter()
        .map(|p| match p {
            super::big::Part::Disk(_, l) => Some(*l),
            super::big::Part::Mem(_) => None,
        })
        .collect::<Option<Vec<usize>>>()?;
    Some(super::tiles::TileJob {
        files: r.paths()?,
        layers,
        nodata: r.layers.iter().map(|l| l.nodata).collect(),
        width: d.grid.width,
        height: d.grid.height,
        colour: cost.colour,
        compact: cost.compact,
        weights: cost.weights.clone(),
        scale,
        mutual,
        seed,
        max_passes,
        tile,
        margin,
        zones: super::zones::compact(zones),
    })
}

/// What the cost of a fusion needs besides the regions.
pub(crate) struct Cost {
    /// The weight of colour: 1 - shape.
    pub colour: f64,
    /// The weight of compactness against smoothness in the shape heterogeneity.
    pub compact: f64,
    /// The weight of each band.
    pub weights: Vec<f64>,
}

/// A region as the cost sees it: cells, perimeter (in cell edges), box (row min, column min, row max, column max) and
/// per band `[count, sum, sum of squares]`. The reference of [`Regions::cost`], which works from cached terms.
#[cfg(test)]
#[derive(Clone, Copy)]
pub(crate) struct View<'a> {
    pub cells: u32,
    pub perim: u32,
    pub bbox: [u32; 4],
    pub bands: &'a [f64],
}

/// The standard deviation of the values `[count, sum, sum of squares]` (0 for none).
fn sd(s: &[f64]) -> f64 {
    if s[0] <= 0.0 {
        return 0.0;
    }
    let m = s[1] / s[0];
    (s[2] / s[0] - m * m).max(0.0).sqrt()
}

/// The cost of merging two regions with `shared` edges of border: `f = w h_colour + (1 - w) h_shape`, the colour
/// heterogeneity the growth of n·sd per band (weighted), the shape one a mix of compactness and smoothness.
#[cfg(test)]
pub(crate) fn fusion_cost(p: &Cost, a: View, b: View, shared: u32) -> f64 {
    let mut hc = 0.0;
    for (k, w) in p.weights.iter().enumerate() {
        let (x, y) = (&a.bands[3 * k..3 * k + 3], &b.bands[3 * k..3 * k + 3]);
        let m = [x[0] + y[0], x[1] + y[1], x[2] + y[2]];
        hc += w * (m[0] * sd(&m) - (x[0] * sd(x) + y[0] * sd(y)));
    }
    if p.colour >= 1.0 {
        return hc;
    }
    let bbox = [a.bbox[0].min(b.bbox[0]), a.bbox[1].min(b.bbox[1]), a.bbox[2].max(b.bbox[2]), a.bbox[3].max(b.bbox[3])];
    let (n1, n2) = (a.cells as f64, b.cells as f64);
    let nm = n1 + n2;
    let (l1, l2) = (a.perim as f64, b.perim as f64);
    let lm = l1 + l2 - 2.0 * shared as f64;
    let cmpct = nm * lm / nm.sqrt() - (n1 * l1 / n1.sqrt() + n2 * l2 / n2.sqrt());
    let smooth = nm * lm / around(bbox) - (n1 * l1 / around(a.bbox) + n2 * l2 / around(b.bbox));
    let hs = p.compact * cmpct + (1.0 - p.compact) * smooth;
    p.colour * hc + (1.0 - p.colour) * hs
}

/// The perimeter of a box (row min, column min, row max, column max), in cell edges.
fn around(x: [u32; 4]) -> f64 {
    2.0 * ((x[2] - x[0] + 1) + (x[3] - x[1] + 1)) as f64
}

/// A neighbour in a region's list: its number, the edges of border shared, and the cost of merging the two (NaN
/// while not known: a cost lives until either region changes).
#[derive(Clone, Copy)]
struct Edge {
    nb: u32,
    shared: u32,
    cost: f64,
}

/// Neighbours kept in the region itself (a cell has 4, a region of the middle passes about 6): no heap per region.
type Adj = SmallVec<[Edge; 6]>;

/// What the cost reads of a region besides its sums, with the terms of the shape that depend on the region only.
#[derive(Clone, Copy)]
struct Reg {
    cells: u32,
    perim: u32,
    bbox: [u32; 4],
    /// n·l/√n and n·l/around(box): the region's own terms of compactness and smoothness.
    cmpct: f64,
    smooth: f64,
}

/// The regions' state.
pub(crate) struct Regions {
    /// Numbers per region in `st`: per band (count, sum, sum of squares, count · standard deviation).
    stride: usize,
    reg: Vec<Reg>,
    st: Vec<f64>,
    /// Per region: the neighbours, sorted by number.
    adj: Vec<Adj>,
    /// The joined list of a merge (kept to reuse its memory).
    scratch: Vec<Edge>,
}

impl Regions {
    /// The regions of labels (0 none, 1..=count) over the bands; values are taken from `centre` for the sums (the
    /// deviation does not change, the rounding error does).
    #[cfg(test)]
    fn new(labels: &[u32], count: usize, w: usize, h: usize, bands: &[Arc<Vec<f64>>], centre: &[f64]) -> Regions {
        let src = Layered::new(bands.iter().cloned().map(super::big::Part::Mem).collect(), w, h);
        Regions::from_source(labels, count, w, h, &src, centre, &[]).expect("memory is read without failing")
    }

    /// The same with the values read by strips of rows from `src` (an image on disk is never whole in memory).
    /// `zones` (one number a cell, empty for none) keeps the cells of different zones from being neighbours.
    fn from_source(
        labels: &[u32],
        count: usize,
        w: usize,
        h: usize,
        src: &dyn Source,
        centre: &[f64],
        zones: &[u64],
    ) -> Result<Regions> {
        let nb = src.nb();
        let stride = 4 * nb;
        let empty = Reg { cells: 0, perim: 0, bbox: [u32::MAX, u32::MAX, 0, 0], cmpct: 0.0, smooth: 0.0 };
        let mut g = Regions {
            stride,
            reg: vec![empty; count],
            st: vec![0.0; count * stride],
            adj: vec![Adj::new(); count],
            scratch: Vec::new(),
        };
        let all: Vec<usize> = (0..nb).collect();
        strips(src, h, &all, |r0, r1, rows| {
            for r in r0..r1 {
                for c in 0..w {
                    let i = r * w + c;
                    let a = labels[i];
                    if a == 0 {
                        continue;
                    }
                    let k = a as usize - 1;
                    let at = |ok: bool, j: usize| if ok { labels[j] } else { 0 };
                    let (left, up) = (at(c > 0, i.wrapping_sub(1)), at(r > 0, i.wrapping_sub(w)));
                    let (right, down) = (at(c + 1 < w, i + 1), at(r + 1 < h, i + w));
                    let reg = &mut g.reg[k];
                    reg.cells += 1;
                    let b = &mut reg.bbox;
                    *b = [b[0].min(r as u32), b[1].min(c as u32), b[2].max(r as u32), b[3].max(c as u32)];
                    reg.perim += [left, up, right, down].iter().filter(|&&o| o != a).count() as u32;
                    for (j, band) in rows.iter().enumerate() {
                        let v = band[(r - r0) * w + c];
                        if !v.is_nan() {
                            let d = v - centre[j];
                            let s = &mut g.st[k * stride + 4 * j..k * stride + 4 * j + 3];
                            s[0] += 1.0;
                            s[1] += d;
                            s[2] += d * d;
                        }
                    }
                    // (cells of two zones are not neighbours; their edge still counts in the perimeter above)
                    for (o, j) in [(right, i + 1), (down, i + w)] {
                        if o != 0 && o != a && (zones.is_empty() || zones[i] == zones[j]) {
                            g.adj[k].push(Edge { nb: o - 1, shared: 1, cost: f64::NAN });
                            g.adj[o as usize - 1].push(Edge { nb: a - 1, shared: 1, cost: f64::NAN });
                        }
                    }
                }
            }
        })?;
        for l in g.adj.iter_mut() {
            l.sort_unstable_by_key(|e| e.nb);
            l.dedup_by(|b, a| {
                if a.nb == b.nb {
                    a.shared += b.shared;
                    true
                } else {
                    false
                }
            });
        }
        for r in 0..count {
            g.refresh(r);
        }
        Ok(g)
    }

    /// Prefetch what a visit to region `a` reads first: its list (the part in the region), its numbers and its sums.
    #[inline]
    fn prefetch_region(&self, a: usize) {
        let l = &self.adj[a];
        prefetch(l as *const Adj);
        prefetch((l as *const Adj).wrapping_byte_add(64));
        prefetch(&self.reg[a]);
        prefetch(self.st[a * self.stride..].as_ptr());
    }

    /// The part of the list of `a` that did not fit in the region (the first part is in the cache by now).
    #[inline]
    fn prefetch_heap(&self, a: usize) {
        let l = &self.adj[a];
        if l.spilled() {
            prefetch(l.as_ptr());
            prefetch(l.as_ptr().wrapping_add(4));
        }
    }

    /// Prefetch what the cost to each neighbour of region `a` reads (its whole list is in the cache by now).
    #[inline]
    fn prefetch_neighbours(&self, a: usize, stamp: &[u32]) {
        for e in &self.adj[a] {
            let b = e.nb as usize;
            prefetch(&self.reg[b]);
            prefetch(self.st[b * self.stride..].as_ptr());
            prefetch(&self.adj[b]);
            prefetch(&stamp[b]);
        }
    }

    /// The lists of the neighbours of `a` that spilled out of their regions.
    #[inline]
    fn prefetch_neighbour_heaps(&self, a: usize) {
        for e in &self.adj[a] {
            self.prefetch_heap(e.nb as usize);
        }
    }

    /// The cost of every pair of neighbours worked out, in the order of the regions (what the first pass would do at
    /// random: read in a row, memory keeps up).
    fn eval_all(&mut self, p: &Cost) {
        for a in 0..self.reg.len() {
            for i in 0..self.adj[a].len() {
                let e = self.adj[a][i];
                if (e.nb as usize) > a && e.cost.is_nan() {
                    let f = self.cost(p, a, e.nb as usize, e.shared);
                    self.adj[a][i].cost = f;
                    let l = &mut self.adj[e.nb as usize];
                    if let Ok(j) = l.binary_search_by_key(&(a as u32), |x| x.nb) {
                        l[j].cost = f;
                    }
                }
            }
        }
    }

    /// The terms cached for region `r` worked out again from its state (after its sums or its shape changed).
    fn refresh(&mut self, r: usize) {
        let st = &mut self.st[r * self.stride..(r + 1) * self.stride];
        for j in (0..st.len()).step_by(4) {
            st[j + 3] = st[j] * sd(&st[j..j + 3]);
        }
        let g = &mut self.reg[r];
        let (n, l) = (g.cells as f64, g.perim as f64);
        g.cmpct = n * l / n.sqrt();
        g.smooth = n * l / around(g.bbox);
    }

    /// The statistics of region `r`: per band (count, sum, sum of squares, count · standard deviation).
    fn stats(&self, r: usize) -> &[f64] {
        &self.st[r * self.stride..(r + 1) * self.stride]
    }

    /// The cost of merging regions `a` and `b` that touch along `shared` edges (as `fusion_cost`, the same number,
    /// bit for bit): from the regions' cached terms, so only the merged region's own terms are worked out.
    fn cost(&self, p: &Cost, a: usize, b: usize, shared: u32) -> f64 {
        let (sa, sb) = (self.stats(a), self.stats(b));
        let mut hc = 0.0;
        for (k, w) in p.weights.iter().enumerate() {
            let (x, y) = (&sa[4 * k..4 * k + 4], &sb[4 * k..4 * k + 4]);
            let m = [x[0] + y[0], x[1] + y[1], x[2] + y[2]];
            hc += w * (m[0] * sd(&m) - (x[3] + y[3]));
        }
        if p.colour >= 1.0 {
            return hc;
        }
        let (ra, rb) = (&self.reg[a], &self.reg[b]);
        let bbox = [
            ra.bbox[0].min(rb.bbox[0]),
            ra.bbox[1].min(rb.bbox[1]),
            ra.bbox[2].max(rb.bbox[2]),
            ra.bbox[3].max(rb.bbox[3]),
        ];
        let nm = ra.cells as f64 + rb.cells as f64;
        let lm = ra.perim as f64 + rb.perim as f64 - 2.0 * shared as f64;
        let cmpct = nm * lm / nm.sqrt() - (ra.cmpct + rb.cmpct);
        let smooth = nm * lm / around(bbox) - (ra.smooth + rb.smooth);
        let hs = p.compact * cmpct + (1.0 - p.compact) * smooth;
        p.colour * hc + (1.0 - p.colour) * hs
    }

    /// The neighbour of `a` of the lowest cost among those not merged in this pass (the lowest id on a tie): the
    /// neighbour, its cost, the border, and whether any was left out. The costs worked out are kept in both lists.
    fn best(&mut self, p: &Cost, a: usize, stamp: &[u32], pass: u32) -> (Option<(u32, f64, u32)>, bool) {
        let mut best: Option<(u32, f64, u32)> = None;
        let mut left_out = false;
        for i in 0..self.adj[a].len() {
            let e = self.adj[a][i];
            if stamp[e.nb as usize] == pass {
                left_out = true;
                continue;
            }
            let mut f = e.cost;
            if f.is_nan() {
                f = self.cost(p, a, e.nb as usize, e.shared);
                self.adj[a][i].cost = f;
                let l = &mut self.adj[e.nb as usize];
                if let Ok(j) = l.binary_search_by_key(&(a as u32), |x| x.nb) {
                    l[j].cost = f;
                }
            }
            if best.is_none_or(|(_, g, _)| f < g) {
                best = Some((e.nb, f, e.shared));
            }
        }
        (best, left_out)
    }

    /// Merge region `b` into `a` (they touch along `shared` edges).
    fn merge(&mut self, a: usize, b: usize, shared: u32) {
        let s = self.stride;
        for k in 0..s {
            self.st[a * s + k] += self.st[b * s + k];
        }
        let (x, y) = (self.reg[a], self.reg[b]);
        let ra = &mut self.reg[a];
        ra.cells = x.cells + y.cells;
        ra.perim = x.perim + y.perim - 2 * shared;
        ra.bbox = [x.bbox[0].min(y.bbox[0]), x.bbox[1].min(y.bbox[1]), x.bbox[2].max(y.bbox[2]), x.bbox[3].max(y.bbox[3])];
        self.refresh(a);
        let lb = std::mem::take(&mut self.adj[b]);
        let mut merged = std::mem::take(&mut self.scratch);
        merged.clear();
        let (a32, b32) = (a as u32, b as u32);
        // the lists joined, the two of them left out, the costs unknown
        {
            let la = &self.adj[a];
            let (mut i, mut j) = (0, 0);
            while i < la.len() || j < lb.len() {
                if i < la.len() && la[i].nb == b32 {
                    i += 1;
                } else if j < lb.len() && lb[j].nb == a32 {
                    j += 1;
                } else if j == lb.len() || (i < la.len() && la[i].nb < lb[j].nb) {
                    merged.push(Edge { cost: f64::NAN, ..la[i] });
                    i += 1;
                } else if i == la.len() || lb[j].nb < la[i].nb {
                    merged.push(Edge { cost: f64::NAN, ..lb[j] });
                    j += 1;
                } else {
                    merged.push(Edge { nb: la[i].nb, shared: la[i].shared + lb[j].shared, cost: f64::NAN });
                    i += 1;
                    j += 1;
                }
            }
        }
        // each neighbour of the two now points to a (b gone), the cost of that pair unknown
        for e in &merged {
            let l = &mut self.adj[e.nb as usize];
            let pa = l.binary_search_by_key(&a32, |x| x.nb);
            let pb = l.binary_search_by_key(&b32, |x| x.nb);
            match (pa, pb) {
                (Ok(i), Ok(j)) => {
                    l[i] = Edge { nb: a32, shared: e.shared, cost: f64::NAN };
                    l.remove(j);
                }
                (Ok(i), Err(_)) => l[i].cost = f64::NAN,
                (Err(_), Ok(j)) => {
                    l.remove(j);
                    let at = l.binary_search_by_key(&a32, |x| x.nb).unwrap_or_else(|p| p);
                    l.insert(at, Edge { nb: a32, shared: e.shared, cost: f64::NAN });
                }
                (Err(_), Err(_)) => unreachable!("a neighbour of the merged regions lists neither"),
            }
        }
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

/// How many visits ahead the regions are prefetched.
const AHEAD: usize = 4;

/// A hint to the processor to bring the memory at `p` into the cache (nothing where there is no such instruction).
#[inline(always)]
fn prefetch<T>(p: *const T) {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: a prefetch only hints: it reads nothing and cannot fault, whatever the address.
    unsafe {
        std::arch::x86_64::_mm_prefetch(p as *const i8, std::arch::x86_64::_MM_HINT_T0)
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = p;
}

/// The regions of an image merged: each cell's final region (a region of the start, merged), by the cell's start
/// label (0 none). `labels` has the start regions 1..=count.
fn merge_all(g: &mut Regions, p: &Cost, scale: f64, mutual: bool, seed: u64, max_passes: u32) -> Vec<u32> {
    let n = g.reg.len();
    let scale2 = scale * scale;
    let mut parent: Vec<u32> = (0..n as u32).collect();
    let mut alive: Vec<u32> = (0..n as u32).collect();
    let mut dead = vec![false; n];
    let mut quiet = vec![false; n];
    let mut stamp = vec![0u32; n];
    // the cost of every pair at the start, read in a row, as the first pass would work them out at random
    g.eval_all(p);
    let mut rng = Rng(seed ^ 0xA5A5_5A5A_1234_5678);
    for pass in 1..=max_passes {
        alive.retain(|&r| !dead[r as usize]);
        let mut order: Vec<u32> = alive.iter().copied().filter(|&r| !quiet[r as usize]).collect();
        if order.is_empty() {
            break;
        }
        for i in (1..order.len()).rev() {
            order.swap(i, (rng.next() % (i as u64 + 1)) as usize);
        }
        let mut merges = 0usize;
        for (at, &a) in order.iter().enumerate() {
            // the regions visited soon are fetched while this one is worked on: the order is a shuffle, so every
            // visit would wait for memory (the lists first, then the neighbours they name)
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
            let a = a as usize;
            if dead[a] || stamp[a] == pass {
                continue;
            }
            let (best, left_out) = g.best(p, a, &stamp, pass);
            let Some((b, _, shared)) = best.filter(|&(_, f, _)| f < scale2) else {
                // nothing to merge with, and nothing left out: it stays so until it or a neighbour changes
                quiet[a] |= !left_out;
                continue;
            };
            let b = b as usize;
            if mutual && g.best(p, b, &stamp, pass).0.is_none_or(|(c, _, _)| c as usize != a) {
                continue;
            }
            g.merge(a, b, shared);
            dead[b] = true;
            parent[b] = a as u32;
            stamp[a] = pass;
            stamp[b] = pass;
            quiet[a] = false;
            for e in &g.adj[a] {
                quiet[e.nb as usize] = false;
            }
            merges += 1;
        }
        if merges == 0 {
            break;
        }
    }
    (0..n as u32).map(|r| find(&mut parent, r)).collect()
}

/// A level of objects as the merging leaves it, with the regions' state to read their statistics from.
pub(crate) struct Level {
    pub objects: Objects,
    /// Each cell's start region (0 none) and each start region's root.
    pub labels: Labels,
    pub root: Vec<u32>,
    pub g: Regions,
    /// The centre of each band: the sums of `g` are of the values less it.
    pub centre: Vec<f64>,
}

/// One level of multiresolution segmentation: the cells (or the objects of `from`, the level below) merged at a scale.
/// The objects carry no table.
#[allow(clippy::too_many_arguments)]
pub(crate) fn merge_level(
    grid: &Arc<Grid>,
    data: &[Arc<Vec<f64>>],
    cost: &Cost,
    scale: f64,
    mutual: bool,
    seed: u64,
    max_passes: u32,
    from: Option<&Objects>,
    zones: &[u64],
) -> Level {
    let src = Layered::mem(data, grid);
    merge_level_src(grid, &src, cost, scale, mutual, seed, max_passes, from, zones)
        .expect("memory is read without failing")
}

/// [`merge_level`] over an image read by strips of rows from `src` (on disk, say): the same regions, the sums made
/// strip after strip in the order of the cells, so the result is the same as from memory.
#[allow(clippy::too_many_arguments)]
pub(crate) fn merge_level_src(
    grid: &Arc<Grid>,
    src: &dyn Source,
    cost: &Cost,
    scale: f64,
    mutual: bool,
    seed: u64,
    max_passes: u32,
    from: Option<&Objects>,
    zones: &[u64],
) -> Result<Level> {
    let (w, h) = (grid.width, grid.height);
    // the start: a label for each cell that has a value (and, with `from`, is in an object); the centre of each
    // band: sums are of the values less it
    let n = w * h;
    let nb = src.nb();
    let all: Vec<usize> = (0..nb).collect();
    let mut valid = vec![false; n];
    let mut sums = vec![(0.0f64, 0usize); nb];
    strips(src, h, &all, |r0, _, rows| {
        for (j, b) in rows.iter().enumerate() {
            for (k, &v) in b.iter().enumerate() {
                if !v.is_nan() {
                    valid[r0 * w + k] = true;
                    sums[j].0 += v;
                    sums[j].1 += 1;
                }
            }
        }
    })?;
    let centre: Vec<f64> = sums.iter().map(|&(s, c)| if c > 0 { s / c as f64 } else { 0.0 }).collect();
    let (labels, count) = match from {
        None => {
            let mut v = Labels::zeros(n);
            let mut next = 0u32;
            for (to, &ok) in v.iter_mut().zip(&valid) {
                if ok {
                    next += 1;
                    *to = next;
                }
            }
            (v, next as usize)
        }
        Some(f) if !zones.is_empty() => {
            // an object of `from` cut by the zones: a region for each (object, zone), in the order of the first cell
            // (one zone: the same regions as without zones)
            let mut id = std::collections::HashMap::new();
            let mut v = Labels::zeros(n);
            for i in 0..n {
                let a = f.labels[i];
                if valid[i] && a != 0 {
                    let next = id.len() as u32 + 1;
                    v[i] = *id.entry((a, zones[i])).or_insert(next);
                }
            }
            (v, id.len())
        }
        Some(f) => {
            // from-objects with no valid cell are no region: the ids closed up
            let mut id = vec![0u32; f.count + 1];
            let mut next = 0u32;
            let mut v = Labels::zeros(n);
            for i in 0..n {
                let a = f.labels[i] as usize;
                if valid[i] && a != 0 {
                    if id[a] == 0 {
                        next += 1;
                        id[a] = next;
                    }
                    v[i] = id[a];
                }
            }
            (v, next as usize)
        }
    };
    drop(valid);
    let mut g = Regions::from_source(&labels, count, w, h, src, &centre, zones)?;
    let root = merge_all(&mut g, cost, scale, mutual, seed, max_passes);
    // (the region of each cell is read through the start labels, never made as an array)
    let objects = Objects::from_fn(
        grid.clone(),
        |i| match labels[i] {
            0 => NO_REGION,
            a => root[a as usize - 1] as u64,
        },
        NO_REGION,
        4,
    );
    Ok(Level { objects, labels, root, g, centre })
}

impl Level {
    /// Each object's regions' view: the root region of object `k` (id k + 1).
    pub fn roots(&self) -> Vec<usize> {
        let mut at = vec![0usize; self.objects.count];
        let mut seen = vec![false; self.objects.count];
        for (&a, &id) in self.labels.iter().zip(self.objects.labels.iter()) {
            if id != 0 && !seen[id as usize - 1] {
                seen[id as usize - 1] = true;
                at[id as usize - 1] = self.root[a as usize - 1] as usize;
            }
        }
        at
    }

    /// The standard deviation of object `r`'s values in each band (NaN where it has none), `r` from [`Level::roots`].
    pub fn sds(&self, r: usize) -> Vec<f64> {
        let s = self.g.stats(r);
        (0..s.len() / 4).map(|j| if s[4 * j] > 0.0 { sd(&s[4 * j..4 * j + 3]) } else { f64::NAN }).collect()
    }
}

/// `segment.multiresolution`.
pub struct Multiresolution;

impl Operator for Multiresolution {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.multiresolution",
            "Objects by multiresolution segmentation (Baatz & Schäpe 2000, eCognition's): regions start as single \
             cells and merge, pass after pass. In each pass the regions are visited in a pseudo-random order (the \
             seed's); a region merges with the neighbour that costs least when the cost is under scale squared and \
             the region is that neighbour's best too (local mutual best fit; mutual: false merges with the best \
             neighbour whatever its own best). The cost is (1 - shape) times the colour heterogeneity (the growth of \
             cells times standard deviation, summed over the bands with their weights) plus shape times the shape \
             heterogeneity (compactness against smoothness of the outline). It stops when a pass merges nothing \
             (or at max_passes). The scale is in the units of the image's values and of its weights: the same \
             scale gives other objects on reflectance 0-1 than on 0-10000, so choose it for the data (segment.scale \
             suggests one). Raw values are used, not standardised. A cell without a value in every band is no object \
             and never merges; a cell with a value in some bands counts in those bands only. With `from`, the \
             objects of a level below are the start regions, and the result is the level above them: every \
             object of `from` ends inside exactly one new object (output below). With `bounds` no object crosses \
             from one zone to another (a lot, a block, a field); the shape cost still counts the zone's edge as the \
             object's border. A run is one thread, except a big \
             image (or `tile`), which is cut into tiles segmented in parallel and stitched at their borders; the \
             result is the same on every run with the same seed.",
            Access::Global,
        )
        .input(PortSpec::new(
            "image",
            Type::Raster,
            "a map, or a stack of bands, the colour; rasters on disk (raster.read) are read by tiles when the image \
             is tiled, never whole",
        ))
        .input(PortSpec::optional(
            "from",
            Type::Objects,
            "objects of the same grid to start from (the level below); cells in none of them are no object",
        ))
        .input(PortSpec::optional(
            "bounds",
            Type::Any,
            "a thematic layer whose zones no object crosses: a map (a zone for each value, the cells without a value \
             one zone together), vectors (each feature a zone, the cells in none one zone) or objects (each object a \
             zone, id 0 one zone); with `from`, its objects are cut by the zones first",
        ))
        .output(PortSpec::new("objects", Type::Objects, "the new level"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per object: id, cells, area (map units squared), and mean_<band> and sd_<band> for each band",
        ))
        .output(PortSpec::new(
            "scale",
            Type::Number,
            "the scale used (the one given, or the one segment.scale suggests)",
        ))
        .output(PortSpec::optional(
            "below",
            Type::Objects,
            "with `from`: those objects inside the new level (their parent is the new object)",
        ))
        .param(PortSpec::optional(
            "scale",
            Type::Number,
            "the scale, above 0: objects merge while their cost is under its square (the image's units); without \
             it, the first scale segment.scale suggests for the image (noted in the run's warnings, and the \
             `scale` output)",
        ))
        .param(PortSpec::optional("shape", Type::Number, "the weight of shape against colour, 0 to 0.9 (default 0.1)"))
        .param(PortSpec::optional(
            "compactness",
            Type::Number,
            "in the shape, the weight of compactness against smoothness, 0 to 1 (default 0.5)",
        ))
        .param(PortSpec::optional("weights", Type::Any, "a list, the weight of each band (default 1 for all)"))
        .param(PortSpec::optional(
            "mutual",
            Type::Bool,
            "merge only when each is the other's best neighbour (default true)",
        ))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of the order of the visits (default 0)"))
        .param(PortSpec::optional("max_passes", Type::Integer, "the most passes over the objects (default 100)"))
        .param(PortSpec::optional(
            "tile",
            Type::Integer,
            "cells, the side of the tiles segmented in parallel and stitched at their borders (Happ et al. 2010); \
             0 (default) is automatic: the whole image at once up to 4 million cells, tiles of 1024 above; a number \
             forces tiles of that side. Ignored with `from`. The tiles change the objects a little (not the \
             criterion); the result is the same for a seed at any number of threads",
        ))
        .param(PortSpec::optional(
            "margin",
            Type::Integer,
            "cells, the margin around each tile that is segmented with it for the context of its border; 0 \
             (default) is automatic: 64 cells, at most half the tile",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let given = args.opt_param::<f64>("scale")?;
        if let Some(s) = given
            && (!s.is_finite() || s <= 0.0)
        {
            return Err(bad(format!("scale {s}: above 0")));
        }
        let shape: f64 = args.param("shape", 0.1)?;
        if !(0.0..=0.9).contains(&shape) {
            return Err(bad(format!("shape {shape}: 0 to 0.9")));
        }
        let compact: f64 = args.param("compactness", 0.5)?;
        if !(0.0..=1.0).contains(&compact) {
            return Err(bad(format!("compactness {compact}: 0 to 1")));
        }
        let mutual: bool = args.param("mutual", true)?;
        let seed: i64 = args.param("seed", 0)?;
        let max_passes: i64 = args.param("max_passes", 100)?;
        if max_passes < 1 {
            return Err(bad(format!("max_passes {max_passes}: 1 or more")));
        }
        let tile: i64 = args.param("tile", 0)?;
        if tile < 0 || (tile > 0 && tile < 8) {
            return Err(bad(format!("tile {tile}: 0 (automatic) or 8 cells or more")));
        }
        let margin: i64 = args.param("margin", 0)?;
        if margin < 0 {
            return Err(bad(format!("margin {margin}: 0 (automatic) or more")));
        }
        let weights_of = |nb: usize| -> Result<Vec<f64>> {
            let weights: Vec<f64> = match args.opt_param::<Vec<f64>>("weights")? {
                None => vec![1.0; nb],
                Some(v) => v,
            };
            if weights.len() != nb || weights.iter().any(|x| !x.is_finite() || *x < 0.0) {
                return Err(bad(format!("weights: {nb} numbers, 0 or more (one for each band)")));
            }
            Ok(weights)
        };
        // rasters on disk cut in tiles: each tile's window is read when its turn comes, never the whole image
        if args.get("from").is_none()
            && let Some(d) = super::big::disk_image(args, "image")
            && let Some(t) = super::tiles::tile_side(d.grid.width, d.grid.height, tile as usize)
        {
            let cost = Cost { colour: 1.0 - shape, compact, weights: weights_of(d.names.len())? };
            let Some(scale) = given else {
                return Err(bad(
                    "scale: give the scale (the one segment.scale suggests needs the image in memory: run it on a \
                     part of the image)"
                        .into(),
                ));
            };
            let m = super::tiles::margin_of(t, margin as usize);
            let zone = super::zones::zones(args, "bounds", &d.grid)?;
            let job = tile_job(&d, &cost, scale, mutual, seed as u64, max_passes as u32, t, m, &zone);
            let runner = ctx.tile_runner().cloned();
            let remote = runner.as_ref().zip(job.as_ref()).map(|(r, j)| (&**r, j.clone()));
            let on_cluster = remote.is_some();
            if on_cluster {
                ctx.progress(None, "tiles on other computers");
            }
            let mut o = super::tiles::merge_tiled(
                &d.grid,
                &d.src,
                &cost,
                scale,
                mutual,
                seed as u64,
                max_passes as u32,
                t,
                m,
                remote,
                &zone,
            );
            // a cluster that fails (a computer left) never fails the run: the tiles run here
            if let (true, Err(e)) = (on_cluster, &o) {
                ctx.warn(format!("node {node:?}: the tiles could not run on the cluster, they ran here: {e}"));
                o = super::tiles::merge_tiled(
                    &d.grid,
                    &d.src,
                    &cost,
                    scale,
                    mutual,
                    seed as u64,
                    max_passes as u32,
                    t,
                    m,
                    None,
                    &zone,
                );
            }
            let o = o?;
            let table = super::stats_table_src(&o, &d.names, &d.src)?;
            let o = Arc::new(o.with_table(table.clone())?);
            return Ok(outputs([
                ("objects", Value::Objects(o)),
                ("table", Value::Table(table)),
                ("scale", Value::Number(scale)),
            ]));
        }
        let l = super::big::layers(args, "image")?;
        super::warnings::image(ctx, args.node, &l.grid, &l.names, &l.data, false);
        let (w, h) = (l.grid.width, l.grid.height);
        let nb = l.data.len();
        let weights = weights_of(nb)?;
        let from = match args.get("from") {
            None => None,
            Some(_) => Some(super::objects_in(args, "from")?),
        };
        if let Some(f) = from
            && (f.grid.width, f.grid.height) != (w, h)
        {
            return Err(bad("from: objects of another grid than the image".into()));
        }
        // no scale given: the one of segment.scale's ESP (said in the `scale` output and as a note)
        let scale = match given {
            Some(s) => s,
            None => {
                let s = super::esp::suggest_scale(&l, shape, compact, &weights, seed as u64)
                    .map_err(|e| bad(format!("no scale to suggest for the image, give a scale ({e})")))?;
                ctx.warn(format!(
                    "node {node:?}: no scale given, used {s}, the first one segment.scale suggests for the image; set \
                     scale to choose it"
                ));
                s
            }
        };
        let cost = Cost { colour: 1.0 - shape, compact, weights };
        let zone = super::zones::zones(args, "bounds", &l.grid)?;
        // by tiles (an image without `from` that is big, or when `tile` says so): the objects, their table at once
        if from.is_none()
            && let Some(t) = super::tiles::tile_side(w, h, tile as usize)
        {
            let m = super::tiles::margin_of(t, margin as usize);
            let src = Layered::mem(&l.data, &l.grid);
            if ctx.tile_runner().is_some() {
                ctx.warn(format!(
                    "node {node:?}: the cluster reads rasters on disk: read the image with raster.read to segment it \
                     there"
                ));
            }
            let o = super::tiles::merge_tiled(
                &l.grid,
                &src,
                &cost,
                scale,
                mutual,
                seed as u64,
                max_passes as u32,
                t,
                m,
                None,
                &zone,
            )?;
            let table = super::stats_table(&o, &l.names, &l.data);
            let o = Arc::new(o.with_table(table.clone())?);
            return Ok(outputs([
                ("objects", Value::Objects(o)),
                ("table", Value::Table(table)),
                ("scale", Value::Number(scale)),
            ]));
        }
        let Level { objects: o, labels, root, g, centre } = merge_level(
            &l.grid,
            &l.data,
            &cost,
            scale,
            mutual,
            seed as u64,
            max_passes as u32,
            from.map(|f| &**f),
            &zone,
        );
        // the table: the statistics of each object from its root region
        let names = super::band_names(&l.names);
        let mut columns: Vec<String> = ["id", "cells", "area"].map(String::from).to_vec();
        columns.extend(names.iter().map(|s| format!("mean_{s}")));
        columns.extend(names.iter().map(|s| format!("sd_{s}")));
        let area = o.cell_area();
        let mut t = Table::new(columns);
        let mut at = vec![0usize; o.count];
        let mut seen = vec![false; o.count];
        for (&a, &id) in labels.iter().zip(o.labels.iter()) {
            if id != 0 && !seen[id as usize - 1] {
                seen[id as usize - 1] = true;
                at[id as usize - 1] = root[a as usize - 1] as usize;
            }
        }
        t.rows = (0..o.count)
            .map(|k| {
                let s = g.stats(at[k]);
                let mut row = vec![(k + 1) as f64, o.cells[k] as f64, o.cells[k] as f64 * area];
                row.extend(
                    (0..nb).map(|j| if s[4 * j] > 0.0 { centre[j] + s[4 * j + 1] / s[4 * j] } else { f64::NAN }),
                );
                row.extend((0..nb).map(|j| if s[4 * j] > 0.0 { sd(&s[4 * j..4 * j + 3]) } else { f64::NAN }));
                row
            })
            .collect();
        let o = Arc::new(o.with_table(t.clone())?);
        let mut out = outputs([
            ("objects", Value::Objects(o.clone())),
            ("table", Value::Table(t)),
            ("scale", Value::Number(scale)),
        ]);
        if let Some(f) = from {
            // each from-object in the object of its first cell that is one
            let mut parent_of = vec![0u32; f.count];
            for (&a, &id) in f.labels.iter().zip(o.labels.iter()) {
                if a != 0 && id != 0 && parent_of[a as usize - 1] == 0 {
                    parent_of[a as usize - 1] = id;
                }
            }
            let below = (**f).clone().with_parent(o, parent_of)?;
            out.insert("below".into(), Value::Objects(Arc::new(below)));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two cells side by side, one band with the values 0 and 1, worked by hand.
    #[test]
    fn the_cost_of_two_cells_by_hand() {
        let p = Cost { colour: 0.9, compact: 0.5, weights: vec![1.0] };
        let (sa, sb) = ([1.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let a = View { cells: 1, perim: 4, bbox: [0, 0, 0, 0], bands: &sa };
        let b = View { cells: 1, perim: 4, bbox: [0, 1, 0, 1], bands: &sb };
        let f = fusion_cost(&p, a, b, 1);
        // colour: n sd of the pair is 2 x 0.5, the cells' own are 0
        let h_colour = 2.0 * 0.5;
        // shape: the pair has perimeter 6 and a box of 1 x 2 (6 around)
        let h_cmpct = 2.0 * 6.0 / 2f64.sqrt() - 2.0 * 4.0;
        let h_smooth = 2.0 * 6.0 / 6.0 - 2.0 * (4.0 / 4.0);
        let want = 0.9 * h_colour + 0.1 * (0.5 * h_cmpct + 0.5 * h_smooth);
        assert!((f - want).abs() < 1e-12, "{f} against {want}");
        // two equal cells have no colour cost, only the shape's
        let sc = [1.0, 1.0, 1.0];
        let c1 = View { cells: 1, perim: 4, bbox: [0, 0, 0, 0], bands: &sc };
        let c2 = View { cells: 1, perim: 4, bbox: [0, 1, 0, 1], bands: &sc };
        let f0 = fusion_cost(&p, c1, c2, 1);
        assert!((f0 - 0.1 * 0.5 * h_cmpct).abs() < 1e-12, "{f0}");
    }

    /// Merging keeps the lists, the perimeter and the box right: three cells in a row.
    #[test]
    fn merging_joins_the_lists_and_the_perimeter() {
        let band = Arc::new(vec![0.0, 1.0, 2.0]);
        let mut g = Regions::new(&[1, 2, 3], 3, 3, 1, &[band], &[0.0]);
        let list = |g: &Regions, r: usize| g.adj[r].iter().map(|e| (e.nb, e.shared)).collect::<Vec<_>>();
        assert_eq!(list(&g, 1), vec![(0, 1), (2, 1)]);
        g.merge(0, 1, 1);
        assert_eq!(g.reg[0].perim, 6);
        assert_eq!(g.reg[0].bbox, [0, 0, 0, 1]);
        assert_eq!(list(&g, 0), vec![(2, 1)]);
        assert_eq!(list(&g, 2), vec![(0, 1)]);
        g.merge(0, 2, 1);
        assert_eq!((g.reg[0].cells, g.reg[0].perim), (3, 8));
    }

    /// The cost from the cached terms is the reference's number bit for bit, before and after merges, and the costs
    /// kept in the lists are the ones worked out again.
    #[test]
    fn the_cached_cost_is_the_reference_cost() {
        let (w, h) = (12, 9);
        let mut s = 7u64;
        let mut next = || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 33) as f64 / (1u64 << 31) as f64
        };
        let bands: Vec<Arc<Vec<f64>>> =
            (0..3).map(|_| Arc::new((0..w * h).map(|_| 100.0 * next()).collect())).collect();
        let labels: Vec<u32> = (1..=(w * h) as u32).collect();
        for colour in [1.0, 0.9] {
            let mut g = Regions::new(&labels, w * h, w, h, &bands, &[50.0; 3]);
            let p = Cost { colour, compact: 0.4, weights: vec![1.0, 2.0, 0.5] };
            let view = |g: &Regions, r: usize| {
                let (reg, st) = (g.reg[r], g.stats(r));
                let bands: Vec<f64> = st.chunks(4).flat_map(|c| c[..3].to_vec()).collect();
                (reg, bands)
            };
            let mut alive: Vec<usize> = (0..w * h).collect();
            let mut stamp = vec![0u32; w * h];
            for step in 0..60 {
                for &a in &alive {
                    let (ra, ba) = view(&g, a);
                    let es: Vec<Edge> = g.adj[a].to_vec();
                    for e in es {
                        let (rb, bb) = view(&g, e.nb as usize);
                        let va = View { cells: ra.cells, perim: ra.perim, bbox: ra.bbox, bands: &ba };
                        let vb = View { cells: rb.cells, perim: rb.perim, bbox: rb.bbox, bands: &bb };
                        let want = fusion_cost(&p, va, vb, e.shared);
                        assert_eq!(g.cost(&p, a, e.nb as usize, e.shared).to_bits(), want.to_bits());
                        if !e.cost.is_nan() {
                            assert_eq!(e.cost.to_bits(), want.to_bits(), "a stale cost kept");
                        }
                    }
                    g.best(&p, a, &stamp, u32::MAX);
                }
                let a = alive[(step * 7) % alive.len()];
                if let Some((b, _, shared)) = g.best(&p, a, &stamp, u32::MAX).0 {
                    g.merge(a, b as usize, shared);
                    alive.retain(|&r| r != b as usize);
                    stamp[b as usize] = 1;
                }
            }
        }
    }
}
