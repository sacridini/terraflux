//! `segment.slic`, `segment.snic` and `segment.felzenszwalb`: superpixels and graph-based segments.
//!
//! All three work on the bands standardised (z-score over the cells with a value) unless told otherwise: the distance
//! between two cells sums the squared differences of the bands, so a band of range 0-10000 would drown one of range
//! 0-1; on z-scores every band counts alike. A cell with a value in some bands only takes the band's mean (0 after the
//! z-score) in the others. A cell without a value in every band is no object. The same result on every run, on any number of threads.

use std::cmp::Reverse;
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::{NO_REGION, Objects, find};
use crate::raster_stats::Layers;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

/// A hint to the processor to bring the memory at `p` into the cache (nothing where there is no such instruction).
#[inline(always)]
pub(super) fn prefetch<T>(p: *const T) {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: a prefetch only hints: it reads nothing and cannot fault, whatever the address.
    unsafe {
        std::arch::x86_64::_mm_prefetch(p as *const i8, std::arch::x86_64::_MM_HINT_T0)
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = p;
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Slic));
    r.add(Arc::new(Snic));
    r.add(Arc::new(Felzenszwalb));
}

/// A cell that is in an object (has a value) but in no superpixel yet: the regions of these split by connection.
const LOOSE: u64 = u64::MAX - 1;

/// The image as the algorithms see it: the cells with a value and the bands of each, standardised.
pub(super) struct Prep {
    pub(super) w: usize,
    pub(super) h: usize,
    pub(super) nb: usize,
    pub(super) valid: Vec<bool>,
    /// The bands of cell i at `i * nb..(i + 1) * nb`.
    pub(super) x: Vec<f64>,
}

pub(super) fn prep(l: &Layers, standardise: bool) -> Prep {
    let (w, h, nb) = (l.grid.width, l.grid.height, l.data.len());
    let n = w * h;
    let valid: Vec<bool> = (0..n).into_par_iter().map(|i| l.data.iter().any(|b| !b[i].is_nan())).collect();
    // each band's mean and scale, the bands in parallel (a band's sums stay in the order of the cells)
    let st: Vec<(f64, f64)> = l
        .data
        .par_iter()
        .map(|b| {
            let (mut s, mut c) = (0.0, 0usize);
            for &v in b.iter().filter(|v| !v.is_nan()) {
                s += v;
                c += 1;
            }
            let mean = if c > 0 { s / c as f64 } else { 0.0 };
            let mut s2 = 0.0;
            for &v in b.iter().filter(|v| !v.is_nan()) {
                s2 += (v - mean) * (v - mean);
            }
            let sd = if c > 0 { (s2 / c as f64).sqrt() } else { 0.0 };
            // a band with no spread has nothing to tell cells apart: its z-scores are 0
            (mean, if standardise && sd > 0.0 { 1.0 / sd } else { 1.0 })
        })
        .collect();
    let mut x = vec![0.0; n * nb];
    const CHUNK: usize = 4096;
    x.par_chunks_mut(CHUNK * nb.max(1)).enumerate().for_each(|(ch, xs)| {
        for (q, cell) in xs.chunks_mut(nb.max(1)).enumerate() {
            let i = ch * CHUNK + q;
            for (j, (b, &(mean, scale))) in l.data.iter().zip(&st).enumerate() {
                let v = b[i];
                if !v.is_nan() {
                    cell[j] = if standardise { (v - mean) * scale } else { v };
                } else if !standardise {
                    cell[j] = mean;
                }
            }
        }
    });
    Prep { w, h, nb, valid, x }
}

impl Prep {
    pub(super) fn feat(&self, i: usize) -> &[f64] {
        &self.x[i * self.nb..(i + 1) * self.nb]
    }

    /// The squared difference of two cells' bands.
    fn gap(&self, i: usize, j: usize) -> f64 {
        self.feat(i).iter().zip(self.feat(j)).map(|(a, b)| (a - b) * (a - b)).sum()
    }
}

/// The settings every segmenter of the lattice shares.
fn lattice_params(args: &Args, cells: usize) -> Result<(f64, f64)> {
    let node = args.node;
    let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
    let size = args.opt_param::<i64>("size")?;
    let count = args.opt_param::<i64>("count")?;
    let s = match (size, count) {
        (Some(_), Some(_)) => return Err(bad("give size or count, not both".into())),
        (Some(s), None) if s >= 1 => s as f64,
        (Some(s), None) => return Err(bad(format!("size {s}: 1 cell or more"))),
        (None, Some(k)) if k >= 1 => (cells.max(1) as f64 / k as f64).sqrt().max(1.0),
        (None, Some(k)) => return Err(bad(format!("count {k}: 1 or more"))),
        (None, None) => 10.0,
    };
    let m: f64 = args.param("compactness", 1.0)?;
    if !m.is_finite() || m < 0.0 {
        return Err(bad(format!("compactness {m}: 0 or more")));
    }
    Ok((s, m))
}

/// The seeds on a regular lattice of about `s` cells a side: the cell nearest to each lattice point that has a value
/// in its 3 x 3 (with `gradient`, the one of the lowest gradient there). Returns the cells and the side the lattice
/// really has.
fn seeds(p: &Prep, s: f64, gradient: bool) -> (Vec<usize>, f64) {
    let (w, h) = (p.w, p.h);
    let ny = ((h as f64 / s).round() as usize).clamp(1, h.max(1));
    let nx = ((w as f64 / s).round() as usize).clamp(1, w.max(1));
    let side = ((w * h) as f64 / (nx * ny) as f64).sqrt();
    // the gradient of a cell: its neighbours across, an edge or a cell with no value counted as the cell itself
    let grad = |r: usize, c: usize| -> f64 {
        let i = r * w + c;
        let at = |rr: usize, cc: usize, ok: bool| if ok && p.valid[rr * w + cc] { rr * w + cc } else { i };
        let (up, down) = (at(r.wrapping_sub(1), c, r > 0), at(r + 1, c, r + 1 < h));
        let (left, right) = (at(r, c.wrapping_sub(1), c > 0), at(r, c + 1, c + 1 < w));
        p.gap(up, down) + p.gap(left, right)
    };
    let mut seen = vec![false; w * h];
    let mut out = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        for k in 0..nx {
            let (r0, c0) = (
                ((j as f64 + 0.5) * h as f64 / ny as f64) as usize,
                ((k as f64 + 0.5) * w as f64 / nx as f64) as usize,
            );
            let (r0, c0) = (r0.min(h - 1), c0.min(w - 1));
            let mut best: Option<(usize, f64)> = None;
            // the lattice point first, then its 3 x 3 in scan order
            let around = std::iter::once((0i64, 0i64))
                .chain((-1..=1).flat_map(|a| (-1..=1).map(move |b| (a, b))).filter(|&(a, b)| (a, b) != (0, 0)));
            for (a, b) in around {
                let (r, c) = (r0 as i64 + a, c0 as i64 + b);
                if r < 0 || c < 0 || r >= h as i64 || c >= w as i64 {
                    continue;
                }
                let (r, c) = (r as usize, c as usize);
                let i = r * w + c;
                if !p.valid[i] {
                    continue;
                }
                let g = if gradient { grad(r, c) } else { 0.0 };
                if best.is_none_or(|(_, bg)| g < bg) {
                    best = Some((i, g));
                }
                if !gradient {
                    break;
                }
            }
            if let Some((i, _)) = best
                && !seen[i]
            {
                seen[i] = true;
                out.push(i);
            }
        }
    }
    (out, side)
}

/// The squared difference of the bands of a cell and of a centre; `N` the bands when known (0: `nb`), so the loop unrolls.
#[inline(always)]
fn colour_gap<const N: usize>(f: &[f64], c: &[f64], nb: usize) -> f64 {
    let n = if N == 0 { nb } else { N };
    let mut s = 0.0;
    for b in 0..n {
        let d = f[b] - c[b];
        s += d * d;
    }
    s
}

/// One assignment of SLIC: each cell takes the centre of the least distance among those whose window holds it (the
/// lowest centre on a tie), rows in parallel. `ks[start[r]..start[r + 1]]` are the centres whose window holds row `r`,
/// in order. Returns whether some cell keeps a label from an earlier pass that no window holds now.
#[allow(clippy::too_many_arguments)]
fn assign<const N: usize>(
    p: &Prep,
    cen: &[f64],
    win: &[[usize; 4]],
    start: &[usize],
    ks: &[u32],
    wt: f64,
    lab: &mut [u32],
) -> bool {
    let (w, nb) = (p.w, p.nb);
    let stride = nb + 2;
    lab.par_chunks_mut(w.max(1))
        .enumerate()
        .map_init(
            || vec![f64::INFINITY; w],
            |dist, (r, row)| {
                dist.fill(f64::INFINITY);
                let valid = &p.valid[r * w..(r + 1) * w];
                let x = &p.x[r * w * nb..(r + 1) * w * nb];
                for &k in &ks[start[r]..start[r + 1]] {
                    let k = k as usize;
                    let c = &cen[k * stride..(k + 1) * stride];
                    let dr = r as f64 - c[0];
                    let dr2 = dr * dr;
                    for q in win[k][2]..=win[k][3] {
                        if !valid[q] {
                            continue;
                        }
                        let dq = q as f64 - c[1];
                        let sp = (dr2 + dq * dq) * wt;
                        // (the colour part is never negative: the cell cannot win with the space part alone not enough)
                        if sp >= dist[q] {
                            continue;
                        }
                        let d = colour_gap::<N>(&x[q * nb..(q + 1) * nb], &c[2..], nb) + sp;
                        if d < dist[q] {
                            dist[q] = d;
                            row[q] = k as u32 + 1;
                        }
                    }
                }
                (0..w).any(|q| dist[q] == f64::INFINITY && row[q] != 0)
            },
        )
        .reduce(|| false, |a, b| a || b)
}

/// SLIC (Achanta et al. 2012): k-means on the bands and the position, each centre looking only at a window of 2S x 2S
/// around it; DÂ² = d_colourÂ² + (d_space / S)Â² mÂ² (d_colourÂ² the mean over the bands of the squared gaps). Returns the superpixel of each cell (0 none).
fn slic(p: &Prep, seeds: &[usize], s: f64, m: f64, iterations: usize) -> Vec<u32> {
    let (w, h, nb) = (p.w, p.h, p.nb);
    let stride = nb + 2;
    let nk = seeds.len();
    let mut cen = vec![0.0f64; nk * stride];
    for (k, &i) in seeds.iter().enumerate() {
        cen[k * stride] = (i / w) as f64;
        cen[k * stride + 1] = (i % w) as f64;
        cen[k * stride + 2..(k + 1) * stride].copy_from_slice(p.feat(i));
    }
    let wt = m * m * nb as f64 / (s * s);
    let mut lab = vec![0u32; w * h];
    // the window of each centre (row min, row max, column min, column max) and, by row, the centres whose window holds it
    let mut win = vec![[0usize; 4]; nk];
    let mut start = vec![0usize; h + 1];
    let mut fill = vec![0usize; h];
    let mut ks: Vec<u32> = Vec::new();
    let mut sum = vec![0.0f64; nk * stride];
    for _ in 0..iterations {
        for (k, c) in cen.chunks(stride).enumerate() {
            win[k] = [
                (c[0] - s).floor().max(0.0) as usize,
                ((c[0] + s).ceil() as usize).min(h - 1),
                (c[1] - s).floor().max(0.0) as usize,
                ((c[1] + s).ceil() as usize).min(w - 1),
            ];
        }
        start.fill(0);
        for wk in &win {
            for r in wk[0]..=wk[1] {
                start[r + 1] += 1;
            }
        }
        for r in 0..h {
            start[r + 1] += start[r];
        }
        ks.resize(start[h], 0);
        fill.copy_from_slice(&start[..h]);
        for (k, wk) in win.iter().enumerate() {
            for r in wk[0]..=wk[1] {
                ks[fill[r]] = k as u32;
                fill[r] += 1;
            }
        }
        let stale = match nb {
            1 => assign::<1>(p, &cen, &win, &start, &ks, wt, &mut lab),
            2 => assign::<2>(p, &cen, &win, &start, &ks, wt, &mut lab),
            3 => assign::<3>(p, &cen, &win, &start, &ks, wt, &mut lab),
            4 => assign::<4>(p, &cen, &win, &start, &ks, wt, &mut lab),
            _ => assign::<0>(p, &cen, &win, &start, &ks, wt, &mut lab),
        };
        // the centres move to the mean of their cells
        if stale {
            // some cell kept the label of an earlier pass, maybe outside the window: all the cells, in order
            let mut cnt = vec![0u32; nk];
            sum.fill(0.0);
            for (i, &l) in lab.iter().enumerate() {
                if l == 0 {
                    continue;
                }
                let k = l as usize - 1;
                cnt[k] += 1;
                let t = &mut sum[k * stride..(k + 1) * stride];
                t[0] += (i / w) as f64;
                t[1] += (i % w) as f64;
                for (a, b) in t[2..].iter_mut().zip(p.feat(i)) {
                    *a += b;
                }
            }
            for k in 0..nk {
                if cnt[k] > 0 {
                    for q in 0..stride {
                        cen[k * stride + q] = sum[k * stride + q] / cnt[k] as f64;
                    }
                }
            }
        } else {
            // every cell of a centre is in its window: the sums of each centre in parallel, its cells in the same order
            // as over the whole image, so the same floats
            let lab = &lab;
            let win = &win;
            sum.par_chunks_mut(stride).zip(cen.par_chunks_mut(stride)).enumerate().for_each(|(k, (t, c))| {
                t.fill(0.0);
                let mut cnt = 0u32;
                let id = k as u32 + 1;
                for r in win[k][0]..=win[k][1] {
                    for q in win[k][2]..=win[k][3] {
                        let i = r * w + q;
                        if lab[i] != id {
                            continue;
                        }
                        cnt += 1;
                        t[0] += r as f64;
                        t[1] += q as f64;
                        for (a, b) in t[2..].iter_mut().zip(p.feat(i)) {
                            *a += b;
                        }
                    }
                }
                if cnt > 0 {
                    for q in 0..stride {
                        c[q] = t[q] / cnt as f64;
                    }
                }
            });
        }
    }
    lab
}

/// SNIC (Achanta & Susstrunk 2017): the superpixels grow from the seeds by a queue of cells ordered by their distance to
/// the superpixel's centroid, which is updated as each cell joins; 4 neighbours; the same distance as SLIC. Ties go to
/// the cell queued first. Returns the superpixel of each cell (0 none).
fn snic(p: &Prep, seeds: &[usize], s: f64, m: f64) -> Vec<u32> {
    let (w, h, nb) = (p.w, p.h, p.nb);
    // the queue numbers its pushes in 32 bits, at most 4 a cell (`run` refuses bigger images)
    assert!(w * h < 1 << 28, "segment.snic: an image of more than 2^28 cells");
    let stride = nb + 3;
    // per superpixel: count, row sum, column sum, band sums
    let mut sum = vec![0.0f64; seeds.len() * stride];
    let wt = m * m * nb as f64 / (s * s);
    // The queue holds (distance bits, push number << 32 | cell): the distance is never negative, so its bits sort as the
    // values, and the push number breaks ties (the cell queued first). The superpixel of push number q is `owner[q]`.
    // A cell is queued only when its distance is below every one queued for it so far: a later push with a distance
    // no lower would come out after the first, when the cell is labelled already.
    let mut cells = cells_of(p);
    let mut heap = Queue::with_capacity(w * h / 4);
    let mut owner: Vec<u32> = Vec::with_capacity(w * h / 4);
    for (k, &i) in seeds.iter().enumerate() {
        heap.push(Queue::key(0.0, owner.len(), i));
        owner.push(k as u32);
        cells[i].low = 0.0;
    }
    let mut mean = vec![0.0f64; nb];
    while let Some(q) = heap.pop() {
        let i = Queue::cell(q);
        if cells[i].lab != 0 {
            continue;
        }
        // (the next cell to come out is likely this one's neighbour work after: its memory is fetched meanwhile)
        if let Some(nx) = heap.peek() {
            let j = Queue::cell(nx);
            prefetch(cells.as_ptr().wrapping_add(j));
            prefetch(cells.as_ptr().wrapping_add(j).wrapping_sub(w));
            prefetch(cells.as_ptr().wrapping_add(j + w));
            prefetch(p.x.as_ptr().wrapping_add(j * nb));
            prefetch(p.x.as_ptr().wrapping_add(j.wrapping_sub(w).wrapping_mul(nb)));
            prefetch(p.x.as_ptr().wrapping_add((j + w) * nb));
            prefetch(owner.as_ptr().wrapping_add(Queue::push_of(nx)));
        }
        let k = owner[Queue::push_of(q)] as usize;
        cells[i].lab = k as u32 + 1;
        let (r, c) = (i / w, i % w);
        let t = &mut sum[k * stride..(k + 1) * stride];
        t[0] += 1.0;
        t[1] += r as f64;
        t[2] += c as f64;
        for (a, b) in t[3..].iter_mut().zip(p.feat(i)) {
            *a += b;
        }
        let (mr, mc) = (t[1] / t[0], t[2] / t[0]);
        for (a, b) in mean.iter_mut().zip(&t[3..]) {
            *a = b / t[0];
        }
        let mut go = |j: usize, rj: usize, cj: usize| {
            let cell = &mut cells[j];
            if cell.lab != 0 {
                return;
            }
            let (dr, dq) = (rj as f64 - mr, cj as f64 - mc);
            let sp = (dr * dr + dq * dq) * wt;
            // (the colour part is never negative)
            if sp >= cell.low {
                return;
            }
            let mut dc = 0.0;
            for (a, b) in p.feat(j).iter().zip(&mean) {
                dc += (a - b) * (a - b);
            }
            let d = dc + sp;
            if d < cell.low {
                cell.low = d;
                heap.push(Queue::key(d, owner.len(), j));
                owner.push(k as u32);
            }
        };
        if r > 0 {
            go(i - w, r - 1, c);
        }
        if c > 0 {
            go(i - 1, r, c - 1);
        }
        if c + 1 < w {
            go(i + 1, r, c + 1);
        }
        if r + 1 < h {
            go(i + w, r + 1, c);
        }
    }
    labels_of(&cells)
}

/// A cell of the queue-driven growth of `snic` and the watershed's flood: the lowest priority queued for it so far
/// (infinity none) and its label (0 none yet, `u32::MAX` a cell without a value), side by side for one cache miss.
#[derive(Clone, Copy)]
pub(super) struct Cell {
    pub(super) low: f64,
    pub(super) lab: u32,
}

pub(super) fn cells_of(p: &Prep) -> Vec<Cell> {
    (0..p.w * p.h)
        .into_par_iter()
        .map(|i| Cell { low: f64::INFINITY, lab: if p.valid[i] { 0 } else { u32::MAX } })
        .collect()
}

/// The label of each cell (0 for none or no value).
pub(super) fn labels_of(cells: &[Cell]) -> Vec<u32> {
    cells.par_iter().map(|c| if c.lab == u32::MAX { 0 } else { c.lab }).collect()
}

/// A min-heap of 128-bit keys with four children to a node: fewer levels than a binary heap, and the children side by
/// side. A key is the priority's bits (the priority is never negative, so they sort as the values), then the number of
/// the push (so equal priorities come out in the order they went in) and the cell.
pub(super) struct Queue {
    v: Vec<u128>,
}

impl Queue {
    pub(super) fn with_capacity(n: usize) -> Queue {
        Queue { v: Vec::with_capacity(n) }
    }

    pub(super) fn key(pri: f64, push: usize, cell: usize) -> u128 {
        ((pri.to_bits() as u128) << 64) | ((push as u128) << 32) | cell as u128
    }

    pub(super) fn cell(key: u128) -> usize {
        (key as u64 & 0xFFFF_FFFF) as usize
    }

    pub(super) fn push_of(key: u128) -> usize {
        ((key as u64) >> 32) as usize
    }

    /// The least key, left in.
    pub(super) fn peek(&self) -> Option<u128> {
        self.v.first().copied()
    }

    pub(super) fn push(&mut self, x: u128) {
        let mut i = self.v.len();
        self.v.push(x);
        while i > 0 {
            let up = (i - 1) / 4;
            if self.v[up] <= x {
                break;
            }
            self.v[i] = self.v[up];
            i = up;
        }
        self.v[i] = x;
    }

    pub(super) fn pop(&mut self) -> Option<u128> {
        let last = self.v.pop()?;
        let n = self.v.len();
        if n == 0 {
            return Some(last);
        }
        let top = self.v[0];
        let mut i = 0;
        loop {
            let first = 4 * i + 1;
            if first >= n {
                break;
            }
            let (mut at, mut least) = (first, self.v[first]);
            for q in first + 1..(first + 4).min(n) {
                if self.v[q] < least {
                    (at, least) = (q, self.v[q]);
                }
            }
            if least >= last {
                break;
            }
            self.v[i] = least;
            i = at;
        }
        self.v[i] = last;
        Some(top)
    }
}

/// The regions of a labelling (0 where a cell with a value has none: those are `LOOSE`).
fn regions_of(p: &Prep, lab: &[u32]) -> Vec<u64> {
    (0..p.w * p.h)
        .into_par_iter()
        .map(|i| match (p.valid[i], lab[i]) {
            (false, _) => NO_REGION,
            (true, 0) => LOOSE,
            (true, l) => l as u64,
        })
        .collect()
}

/// Each object under `min` cells joined to its largest neighbour (cells, then the lowest id), in the order of the ids,
/// as the reference SLIC does to enforce connectivity; the neighbour's own growth counts for the next ones. The
/// regions to give to `Objects::from_regions`.
fn absorb(o: &Objects, min: u32) -> Vec<u64> {
    let mut parent: Vec<u32> = (0..=o.count as u32).collect();
    let mut size: Vec<u32> = std::iter::once(0).chain(o.cells.iter().copied()).collect();
    for id in 1..=o.count as u32 {
        let me = find(&mut parent, id);
        if me != id || size[me as usize] >= min {
            continue;
        }
        let mut best: Option<u32> = None;
        for &(nb, _) in o.neighbours_of(id) {
            let r = find(&mut parent, nb);
            if r == me {
                continue;
            }
            if best.is_none_or(|b| (size[r as usize], Reverse(r)) > (size[b as usize], Reverse(b))) {
                best = Some(r);
            }
        }
        if let Some(b) = best {
            parent[me as usize] = b;
            size[b as usize] += size[me as usize];
        }
    }
    // the root of each id, then the cells in parallel
    let root: Vec<u32> = (0..=o.count as u32).map(|id| find(&mut parent, id)).collect();
    o.labels[..].par_iter().map(|&l| if l == 0 { NO_REGION } else { root[l as usize] as u64 }).collect()
}

/// The objects and the table of a segmenter's regions.
pub(super) fn finish(l: &Layers, region: &[u64]) -> Result<Outputs> {
    let o = Objects::from_regions(l.grid.clone(), region, 4);
    let t = super::stats_table(&o, &l.names, &l.data);
    let o = o.with_table(t.clone())?;
    Ok(outputs([("objects", Value::Objects(Arc::new(o))), ("table", Value::Table(t))]))
}

/// The ports every segmenter here has.
fn signature(name: &str, about: &str) -> OpSignature {
    OpSignature::new(name, about, Access::Global)
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands, the colour"))
        .output(PortSpec::new("objects", Type::Objects, "the superpixels, each one connected"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per object: id, cells, area (map units squared), and mean_<band> and sd_<band> for each band",
        ))
        .param(PortSpec::optional("standardise", Type::Bool, "standardise each band (z-score) first (default true)"))
}

const STANDARD: &str = "The bands are standardised (z-score over the cells with a value) by default, so that bands \
    of different ranges (reflectance 0-1 beside 0-10000) weigh alike in the distance; standardise: false uses the \
    raw values. A cell without a value in every band is no object. The same result on every run, on any number of threads.";

fn lattice_ports(sig: OpSignature) -> OpSignature {
    sig.param(PortSpec::optional(
        "size",
        Type::Integer,
        "the approximate side of a superpixel in cells (default 10): about cells / size² superpixels",
    ))
    .param(PortSpec::optional("count", Type::Integer, "instead of size: the approximate number of superpixels"))
    .param(PortSpec::optional(
        "compactness",
        Type::Number,
        "m, the weight of space against colour (default 1): the colour distance is the mean over the bands of the \
         squared gap (in standard deviations when standardised), the space one the squared distance over the \
         side S; m = 1 makes a distance of S cost as much as a gap of one standard deviation in every band. Higher \
         gives more regular superpixels, lower ones that follow the edges",
    ))
}

/// `segment.slic`.
struct Slic;

impl Operator for Slic {
    fn signature(&self) -> OpSignature {
        let about = format!(
            "Objects as SLIC superpixels (Achanta et al. 2012): seeds on a regular lattice, each moved to the lowest \
             gradient of its 3 x 3, then k-means in the bands and the position, each centre taking the cells in a \
             window of 2S x 2S around it, with the distance sqrt(d_colour² + (d_space / S)² m²), d_colour² being the mean over the bands of the squared \
             gaps, so m means the same for any number of bands. Fragments smaller \
             than a quarter of the expected size are joined to their largest neighbour, so each superpixel is \
             connected. {STANDARD}"
        );
        lattice_ports(signature("segment.slic", &about)).param(PortSpec::optional(
            "iterations",
            Type::Integer,
            "the iterations of the k-means (default 10)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let l = crate::raster_stats::layers(args, "image")?;
        let standardise: bool = args.param("standardise", true)?;
        super::warnings::image(ctx, args.node, &l.grid, &l.names, &l.data, standardise);
        let p = prep(&l, standardise);
        let (s, m) = lattice_params(args, p.valid.iter().filter(|&&v| v).count())?;
        let iterations: i64 = args.param("iterations", 10)?;
        if iterations < 1 {
            return Err(Error::msg(format!("node {:?}: iterations {iterations}: 1 or more", args.node)));
        }
        let (seeds, side) = seeds(&p, s, true);
        let lab = slic(&p, &seeds, side, m, iterations as usize);
        let first = Objects::from_regions(l.grid.clone(), &regions_of(&p, &lab), 4);
        let min = (side * side / 4.0).ceil() as u32;
        finish(&l, &absorb(&first, min))
    }
}

/// `segment.snic`.
struct Snic;

impl Operator for Snic {
    fn signature(&self) -> OpSignature {
        let about = format!(
            "Objects as SNIC superpixels (Achanta & Susstrunk 2017): the superpixels grow from seeds on a regular \
             lattice, no iterations: a queue gives the next cell, the one nearest (in the bands and the position, \
             as SLIC's distance) to the centroid of the superpixel it would join, and the centroid is updated at \
             each cell. 4 neighbours, so a superpixel grows connected. {STANDARD}"
        );
        lattice_ports(signature("segment.snic", &about))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let l = crate::raster_stats::layers(args, "image")?;
        if l.grid.width * l.grid.height >= 1 << 28 {
            return Err(Error::msg(format!(
                "node {:?}: an image of {} × {} cells, more than segment.snic works in memory",
                args.node, l.grid.width, l.grid.height
            )));
        }
        let standardise: bool = args.param("standardise", true)?;
        super::warnings::image(ctx, args.node, &l.grid, &l.names, &l.data, standardise);
        let p = prep(&l, standardise);
        let (s, m) = lattice_params(args, p.valid.iter().filter(|&&v| v).count())?;
        let (seeds, side) = seeds(&p, s, false);
        let lab = snic(&p, &seeds, side, m);
        finish(&l, &regions_of(&p, &lab))
    }
}

/// The bands smoothed by a Gaussian of `sigma` cells (to 4 sigma), over the cells with a value only: a cell's smooth is
/// the weighted mean of those around it. Both passes run by rows in parallel, each cell summing its line in the same
/// order.
pub(super) fn smooth(p: &mut Prep, sigma: f64) {
    if p.w == 0 || p.h == 0 || p.nb == 0 {
        return;
    }
    let rad = (4.0 * sigma).ceil() as i64;
    let ker: Vec<f64> = (-rad..=rad).map(|d| (-((d * d) as f64) / (2.0 * sigma * sigma)).exp()).collect();
    match p.nb {
        1 => smooth_n::<1>(p, &ker, rad),
        2 => smooth_n::<2>(p, &ker, rad),
        3 => smooth_n::<3>(p, &ker, rad),
        4 => smooth_n::<4>(p, &ker, rad),
        _ => smooth_n::<0>(p, &ker, rad),
    }
}

/// [`smooth`] with `N` bands when known (0: as many as `p.nb`), so the loops over the bands unroll.
fn smooth_n<const N: usize>(p: &mut Prep, ker: &[f64], rad: i64) {
    let (w, h, nb) = (p.w, p.h, p.nb);
    let n = if N == 0 { nb } else { N };
    let valid = &p.valid;
    // along the rows
    let mut rows = p.x.clone();
    rows.par_chunks_mut(w * nb).zip(p.x.par_chunks(w * nb)).enumerate().for_each_init(
        || vec![0.0f64; nb],
        |acc, (r, (dst, src))| {
            let vr = &valid[r * w..(r + 1) * w];
            let acc = &mut acc[..n];
            for pos in 0..w {
                if !vr[pos] {
                    continue;
                }
                acc.fill(0.0);
                let mut wsum = 0.0;
                for (q, &kw) in ker.iter().enumerate() {
                    let at = pos as i64 + q as i64 - rad;
                    if at < 0 || at >= w as i64 {
                        continue;
                    }
                    let j = at as usize;
                    if vr[j] {
                        wsum += kw;
                        let s = &src[j * nb..j * nb + n];
                        for b in 0..n {
                            acc[b] += kw * s[b];
                        }
                    }
                }
                let d = &mut dst[pos * nb..pos * nb + n];
                for b in 0..n {
                    d[b] = acc[b] / wsum;
                }
            }
        },
    );
    // along the columns: an output row sums the rows around it, cell by cell
    let mut out = rows.clone();
    out.par_chunks_mut(w * nb).enumerate().for_each_init(
        || (vec![0.0f64; w * nb], vec![0.0f64; w]),
        |(acc, wsum), (r, dst)| {
            acc.fill(0.0);
            wsum.fill(0.0);
            for (q, &kw) in ker.iter().enumerate() {
                let at = r as i64 + q as i64 - rad;
                if at < 0 || at >= h as i64 {
                    continue;
                }
                let at = at as usize;
                let vr = &valid[at * w..(at + 1) * w];
                let src = &rows[at * w * nb..(at + 1) * w * nb];
                for c in 0..w {
                    if vr[c] {
                        wsum[c] += kw;
                        let (a, s) = (&mut acc[c * nb..c * nb + n], &src[c * nb..c * nb + n]);
                        for b in 0..n {
                            a[b] += kw * s[b];
                        }
                    }
                }
            }
            for c in 0..w {
                if valid[r * w + c] {
                    let (d, a) = (&mut dst[c * nb..c * nb + n], &acc[c * nb..c * nb + n]);
                    for b in 0..n {
                        d[b] = a[b] / wsum[c];
                    }
                }
            }
        },
    );
    p.x = out;
}

/// Felzenszwalb & Huttenlocher's (2004) segments: returns the region of each cell.
fn felzenszwalb(p: &Prep, eight: bool, k: f64, min_size: usize) -> Vec<u64> {
    let (w, h, nb) = (p.w, p.h, p.nb);
    // the edges as sort keys: the weight's bits (never negative, so they sort as the values), then the cell and the
    // kind of edge (0 right, 1 down, 2 down-right, 3 down-left), which is the order they are made in: equal weights
    // keep it. Rows in parallel.
    let rows: Vec<Vec<u128>> = (0..h)
        .into_par_iter()
        .map(|r| {
            let mut row: Vec<u128> = Vec::with_capacity(w * if eight { 4 } else { 2 });
            for c in 0..w {
                let i = r * w + c;
                if !p.valid[i] {
                    continue;
                }
                let f = p.feat(i);
                let mut add = |j: usize, kind: u128| {
                    if p.valid[j] {
                        let g = p.feat(j);
                        let mut s = 0.0;
                        for b in 0..nb {
                            let d = f[b] - g[b];
                            s += d * d;
                        }
                        row.push(((s.sqrt().to_bits() as u128) << 64) | ((i as u128) << 2) | kind);
                    }
                };
                if c + 1 < w {
                    add(i + 1, 0);
                }
                if r + 1 < h {
                    add(i + w, 1);
                    if eight && c + 1 < w {
                        add(i + w + 1, 2);
                    }
                    if eight && c > 0 {
                        add(i + w - 1, 3);
                    }
                }
            }
            row
        })
        .collect();
    let mut edges: Vec<u128> = Vec::with_capacity(rows.iter().map(Vec::len).sum());
    for row in rows {
        edges.extend_from_slice(&row);
    }
    edges.par_sort_unstable();
    let n = w * h;
    let other = |e: u128| -> (u32, u32) {
        let i = ((e as u64) >> 2) as usize;
        let j = match e as u64 & 3 {
            0 => i + 1,
            1 => i + w,
            2 => i + w + 1,
            _ => i + w - 1,
        };
        (i as u32, j as u32)
    };
    // each cell's set: the root, the cells in it, the largest edge inside it. Which cell is the root of a set does not
    // matter (only the sets do), so the larger one stays the root, for short paths.
    let mut set: Vec<Set> = (0..n as u32).map(|i| Set { parent: i, size: 1, inner: 0.0 }).collect();
    // (the sets of the edges to come are fetched ahead, and, nearer, the sets their cells point to: the edges are in the
    // order of their weights, so far apart)
    const AHEAD: usize = 16;
    for (at, &e) in edges.iter().enumerate() {
        if let Some(&f) = edges.get(at + AHEAD) {
            let (a, b) = other(f);
            prefetch(set.as_ptr().wrapping_add(a as usize));
            prefetch(set.as_ptr().wrapping_add(b as usize));
        }
        if let Some(&f) = edges.get(at + AHEAD / 2) {
            let (a, b) = other(f);
            prefetch(set.as_ptr().wrapping_add(set[a as usize].parent as usize));
            prefetch(set.as_ptr().wrapping_add(set[b as usize].parent as usize));
        }
        let (a, b) = other(e);
        let wt = f64::from_bits((e >> 64) as u64);
        let (ra, rb) = (set_find(&mut set, a), set_find(&mut set, b));
        if ra == rb {
            continue;
        }
        let (sa, sb) = (set[ra as usize], set[rb as usize]);
        if wt <= (sa.inner + k / sa.size as f64).min(sb.inner + k / sb.size as f64) {
            let (big, little) = if sa.size >= sb.size { (ra, rb) } else { (rb, ra) };
            set[little as usize].parent = big;
            set[big as usize].size = sa.size + sb.size;
            set[big as usize].inner = wt;
        }
    }
    // the second pass joins only sets under min_size, and a set that has reached it only grows: the cells already in
    // sets that large are left out of it
    let small: Vec<bool> = (0..n as u32)
        .map(|i| p.valid[i as usize] && {
            let r = set_find(&mut set, i);
            set[r as usize].size < min_size as u32
        })
        .collect();
    for (at, &e) in edges.iter().enumerate() {
        let (a, b) = other(e);
        if !small[a as usize] && !small[b as usize] {
            continue;
        }
        if let Some(&f) = edges.get(at + AHEAD) {
            let (a, b) = other(f);
            prefetch(set.as_ptr().wrapping_add(a as usize));
            prefetch(set.as_ptr().wrapping_add(b as usize));
        }
        let (ra, rb) = (set_find(&mut set, a), set_find(&mut set, b));
        if ra != rb && (set[ra as usize].size < min_size as u32 || set[rb as usize].size < min_size as u32) {
            let (big, little) = if set[ra as usize].size >= set[rb as usize].size { (ra, rb) } else { (rb, ra) };
            set[little as usize].parent = big;
            set[big as usize].size += set[little as usize].size;
        }
    }
    (0..n).map(|i| if p.valid[i] { set_find(&mut set, i as u32) as u64 } else { NO_REGION }).collect()
}

/// A set of cells of `felzenszwalb`.
#[derive(Clone, Copy)]
struct Set {
    parent: u32,
    size: u32,
    inner: f64,
}

/// The root of a cell's set (its path halved on the way).
fn set_find(set: &mut [Set], mut x: u32) -> u32 {
    while set[x as usize].parent != x {
        let g = set[set[x as usize].parent as usize].parent;
        set[x as usize].parent = g;
        x = g;
    }
    x
}

/// `segment.felzenszwalb`.
struct Felzenszwalb;

impl Operator for Felzenszwalb {
    fn signature(&self) -> OpSignature {
        let about = format!(
            "Objects as graph-based segments (Felzenszwalb & Huttenlocher 2004, as skimage's felzenszwalb): the cells \
             are nodes, each joined to its 4 or 8 neighbours by an edge weighted with the distance of their bands (after \
             a Gaussian smoothing of sigma cells). The edges are taken from the lowest weight up, and two components \
             join when the edge is no heavier than the least of their inner weight (the heaviest edge in them) plus \
             k / size: a small component is easy to join, a large one needs a closer match. Then components under \
             min_size cells join, in the order of the edges. A constant area is one object. The objects are \
             4-connected: a link only by a corner splits. {STANDARD}"
        );
        signature("segment.felzenszwalb", &about)
            .param(PortSpec::optional(
                "k",
                Type::Number,
                "the scale (default 1): higher gives larger segments. It is in the distance of the bands: with \
                 standardised bands one unit is a standard deviation of a band (skimage's default 100 is for values \
                 0-255, about 0.4 of the range)",
            ))
            .param(PortSpec::optional(
                "sigma",
                Type::Number,
                "the smoothing's Gaussian, in cells (default 0.8; 0 none)",
            ))
            .param(PortSpec::optional("min_size", Type::Integer, "the least cells of a segment (default 20)"))
            .param(
                PortSpec::optional("neighbours", Type::Integer, "which cells are joined: 4 or 8 (default 8)")
                    .choices([4, 8]),
            )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let k: f64 = args.param("k", 1.0)?;
        if !k.is_finite() || k < 0.0 {
            return Err(bad(format!("k {k}: 0 or more")));
        }
        let sigma: f64 = args.param("sigma", 0.8)?;
        if !sigma.is_finite() || sigma < 0.0 {
            return Err(bad(format!("sigma {sigma}: 0 or more")));
        }
        let min_size: i64 = args.param("min_size", 20)?;
        if min_size < 1 {
            return Err(bad(format!("min_size {min_size}: 1 or more")));
        }
        let nbs: i64 = args.param("neighbours", 8)?;
        if nbs != 4 && nbs != 8 {
            return Err(bad(format!("neighbours {nbs}: 4 or 8")));
        }
        let l = crate::raster_stats::layers(args, "image")?;
        let standardise: bool = args.param("standardise", true)?;
        super::warnings::image(ctx, args.node, &l.grid, &l.names, &l.data, standardise);
        let mut p = prep(&l, standardise);
        if sigma > 0.0 {
            smooth(&mut p, sigma);
        }
        finish(&l, &felzenszwalb(&p, nbs == 8, k, min_size as usize))
    }
}
