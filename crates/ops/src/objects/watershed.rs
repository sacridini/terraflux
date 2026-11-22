//! `segment.watershed`: the watershed of the gradient of an image, with markers given or found.
//!
//! The image is standardised (z-score per band, unless told otherwise) and smoothed by a Gaussian of `sigma` cells
//! (superpixels.rs's, which ignores the cells without a value). The gradient of a cell is the Sobel magnitude of each
//! band (per cell, so the units are standard deviations per cell when standardised), and the largest over the bands:
//! an edge in any one band counts in full, and adding bands does not dilute it as the Euclidean norm would. Edges and
//! cells without a value count as the cell itself in the Sobel window. The same result on every run, on any number of threads.

use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::superpixels::{Cell, Prep, Queue, cells_of, finish, labels_of, prefetch, prep, smooth};
use super::{NO_REGION, find};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Watershed));
}

/// A cell with a value that no marker reached (cut off from them by cells without a value).
const LOOSE: u64 = u64::MAX - 1;

/// The Sobel gradient: the largest over the bands of the magnitude, divided by 8 (a slope per cell).
fn gradient(p: &Prep) -> Vec<f64> {
    let (w, h, nb) = (p.w, p.h, p.nb);
    let mut g = vec![0.0; w * h];
    g.par_chunks_mut(w.max(1)).enumerate().for_each(|(r, grow)| {
        for (c, out) in grow.iter_mut().enumerate() {
            let i = r * w + c;
            if !p.valid[i] {
                continue;
            }
            // the cell at an offset: the cell itself past an edge or on a cell without a value
            let at = |dr: i64, dc: i64| -> usize {
                let (rr, cc) = (r as i64 + dr, c as i64 + dc);
                if rr < 0 || cc < 0 || rr >= h as i64 || cc >= w as i64 {
                    return i;
                }
                let j = rr as usize * w + cc as usize;
                if p.valid[j] { j } else { i }
            };
            let n: [[usize; 3]; 3] = std::array::from_fn(|a| std::array::from_fn(|b| at(a as i64 - 1, b as i64 - 1)));
            let mut best = 0.0f64;
            for b in 0..nb {
                let v = |a: usize, k: usize| p.x[n[a][k] * nb + b];
                let gx = (v(0, 2) + 2.0 * v(1, 2) + v(2, 2)) - (v(0, 0) + 2.0 * v(1, 0) + v(2, 0));
                let gy = (v(2, 0) + 2.0 * v(2, 1) + v(2, 2)) - (v(0, 0) + 2.0 * v(0, 1) + v(0, 2));
                best = best.max((gx * gx + gy * gy).sqrt() / 8.0);
            }
            *out = best;
        }
    });
    g
}

/// The neighbours of a cell (4 or 8) in a grid of `w` x `h`: written to `out`, how many there are. In the order of
/// the rows above, the row, the rows below (left to right).
#[inline]
fn around(i: usize, w: usize, h: usize, eight: bool, out: &mut [usize; 8]) -> usize {
    let (r, c) = (i / w, i % w);
    around_at(i, r, c, w, h, eight, out)
}

/// [`around`] for a cell whose row and column are known.
#[inline]
fn around_at(i: usize, r: usize, c: usize, w: usize, h: usize, eight: bool, out: &mut [usize; 8]) -> usize {
    let (up, down, left, right) = (r > 0, r + 1 < h, c > 0, c + 1 < w);
    let mut n = 0;
    let mut put = |ok: bool, j: usize| {
        if ok {
            out[n] = j;
            n += 1;
        }
    };
    put(eight && up && left, i.wrapping_sub(w + 1));
    put(up, i.wrapping_sub(w));
    put(eight && up && right, i.wrapping_sub(w).wrapping_add(1));
    put(left, i.wrapping_sub(1));
    put(right, i + 1);
    put(eight && down && left, (i + w).wrapping_sub(1));
    put(down, i + w);
    put(eight && down && right, i + w + 1);
    n
}

/// The markers found by the gradient: the cells of its minima that stand `h` or more above the lowest cell they
/// join (their dynamics, the h-minima). The cells go from the lowest up, and a cell joins the components of the cells
/// before it that touch it; when two meet the one with the higher minimum (the younger) ends, its depth being the level
/// of the meeting less its minimum. Each area cut off by cells without a value keeps its lowest minimum whatever its
/// depth. The marker of a minimum is its cells at its lowest value (a plateau is one marker).
pub(super) fn minima(p: &Prep, grad: &[f64], eight: bool, h: f64) -> Vec<Vec<usize>> {
    minima_in(p, &order_of(p, grad), eight, h)
}

/// The cells with a value, from the lowest gradient up (the ties by index): (the gradient's bits, the cell).
pub(super) fn order_of(p: &Prep, grad: &[f64]) -> Vec<(u64, u32)> {
    let mut order: Vec<(u64, u32)> =
        (0..p.w * p.h).into_par_iter().filter(|&i| p.valid[i]).map(|i| (grad[i].to_bits(), i as u32)).collect();
    // (the gradient is never negative, so its bits sort as the values)
    order.par_sort_unstable();
    order
}

/// [`minima`] with the cells in the order of [`order_of`] already.
pub(super) fn minima_in(p: &Prep, order: &[(u64, u32)], eight: bool, h: f64) -> Vec<Vec<usize>> {
    let (w, hh) = (p.w, p.h);
    let n = w * hh;
    let mut parent: Vec<u32> = Vec::new();
    let mut low: Vec<f64> = Vec::new();
    let mut depth: Vec<f64> = Vec::new();
    // per cell: the component it joined and, if it is at the lowest value of its minimum, the minimum (side by side:
    // a cell's turn reads the components around it and writes its own)
    let mut cell = vec![[u32::MAX; 2]; n];
    let mut nb = [0usize; 8];
    let mut roots: Vec<u32> = Vec::with_capacity(8);
    const AHEAD: usize = 12;
    for (at, &(bits, i)) in order.iter().enumerate() {
        // (the cells to come are anywhere in the image: their surroundings are fetched ahead)
        if let Some(&(_, f)) = order.get(at + AHEAD) {
            let f = f as usize;
            prefetch(cell.as_ptr().wrapping_add(f));
            prefetch(cell.as_ptr().wrapping_add(f).wrapping_sub(w));
            prefetch(cell.as_ptr().wrapping_add(f + w));
        }
        let i = i as usize;
        let v = f64::from_bits(bits);
        roots.clear();
        let k = around(i, w, hh, eight, &mut nb);
        for &j in &nb[..k] {
            if cell[j][0] != u32::MAX {
                let r = find(&mut parent, cell[j][0]);
                if !roots.contains(&r) {
                    roots.push(r);
                }
            }
        }
        if roots.is_empty() {
            let id = parent.len() as u32;
            parent.push(id);
            low.push(v);
            depth.push(f64::INFINITY);
            cell[i] = [id, id];
            continue;
        }
        // the elder: the lowest minimum, then the first made
        let elder =
            *roots.iter().min_by(|&&a, &&b| low[a as usize].total_cmp(&low[b as usize]).then(a.cmp(&b))).unwrap();
        for &r in &roots {
            if r != elder {
                depth[r as usize] = v - low[r as usize];
                parent[r as usize] = elder;
            }
        }
        cell[i][0] = elder;
        if v == low[elder as usize] {
            cell[i][1] = elder;
        }
    }
    let mut index = vec![u32::MAX; parent.len()];
    let mut out: Vec<Vec<usize>> = Vec::new();
    for (i, &[_, k]) in cell.iter().enumerate() {
        if k == u32::MAX {
            continue;
        }
        let d = depth[k as usize];
        if d.is_finite() && !(d >= h && d > 0.0) {
            continue;
        }
        if index[k as usize] == u32::MAX {
            index[k as usize] = out.len() as u32;
            out.push(Vec::new());
        }
        out[index[k as usize] as usize].push(i);
    }
    out
}

/// The markers given: the cells of the map with a value (and in the image), a group of 4-connected cells of one value
/// being one marker; in the order of the first cell of each.
fn given(p: &Prep, m: &[f64]) -> Vec<Vec<usize>> {
    let (w, h) = (p.w, p.h);
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    let mut nb = [0usize; 8];
    for s in 0..w * h {
        if seen[s] || m[s].is_nan() || !p.valid[s] {
            continue;
        }
        let mut cells = vec![s];
        seen[s] = true;
        let mut at = 0;
        while at < cells.len() {
            let k = around(cells[at], w, h, false, &mut nb);
            for &j in &nb[..k] {
                if !seen[j] && p.valid[j] && m[j] == m[s] {
                    seen[j] = true;
                    cells.push(j);
                }
            }
            at += 1;
        }
        out.push(cells);
    }
    out
}

/// The flooding: the markers' cells labelled (marker k with k + 1), then the cell of the lowest priority among those
/// that touch a labelled one, the gradient plus `compact` times its distance to the centre of the marker it would
/// join, ties to the cell queued first. Returns the label of each cell (0 none).
pub(super) fn flood(p: &Prep, grad: &[f64], markers: &[Vec<usize>], eight: bool, compact: f64) -> Vec<u32> {
    if compact > 0.0 {
        flood_heap(p, grad, markers, eight, compact)
    } else {
        flood_buckets(p, &order_of(p, grad), markers, eight)
    }
}

/// [`flood`] with the cells in the order of [`order_of`] already.
pub(super) fn flood_in(
    p: &Prep,
    grad: &[f64],
    order: &[(u64, u32)],
    markers: &[Vec<usize>],
    eight: bool,
    compact: f64,
) -> Vec<u32> {
    if compact > 0.0 { flood_heap(p, grad, markers, eight, compact) } else { flood_buckets(p, order, markers, eight) }
}

const NONE: u32 = u32::MAX;

/// What the bucket flooding keeps of a cell, side by side (it reads them together, a cell at a time): its label (0 not
/// yet, `NONE` no value), the bucket it goes in and the cell after it in that bucket.
#[derive(Clone, Copy)]
struct Slot {
    lab: u32,
    rank: u32,
    next: u32,
}

/// A queue of cells in buckets, one a distinct priority (in order), each bucket a list in the order the cells went in;
/// a cell goes in once. `marks` tell the buckets that are not empty (a bit each, and above them a bit for each word
/// that has one set, and so on up to one word), so that the lowest is found in a few steps; `at` is at or below it.
struct Buckets {
    // per bucket: the first cell and the last
    ends: Vec<[u32; 2]>,
    marks: Vec<Vec<u64>>,
    at: usize,
}

impl Buckets {
    fn new(buckets: usize) -> Buckets {
        let mut marks = Vec::new();
        let mut n = buckets;
        loop {
            let words = n.div_ceil(64).max(1);
            marks.push(vec![0u64; words]);
            if words == 1 {
                break;
            }
            n = words;
        }
        Buckets { ends: vec![[NONE; 2]; buckets], marks, at: usize::MAX }
    }

    #[inline]
    fn push(&mut self, slots: &mut [Slot], cell: u32) {
        let b = slots[cell as usize].rank as usize;
        let e = &mut self.ends[b];
        if e[1] == NONE {
            *e = [cell, cell];
            self.mark(b);
        } else {
            slots[e[1] as usize].next = cell;
            e[1] = cell;
        }
        // (a priority lower than the last one out is fine: the search starts again from it)
        self.at = self.at.min(b);
    }

    #[inline]
    fn mark(&mut self, mut b: usize) {
        for level in self.marks.iter_mut() {
            let was = level[b / 64];
            level[b / 64] = was | 1 << (b % 64);
            if was != 0 {
                break;
            }
            b /= 64;
        }
    }

    #[inline]
    fn unmark(&mut self, mut b: usize) {
        for level in self.marks.iter_mut() {
            level[b / 64] &= !(1 << (b % 64));
            if level[b / 64] != 0 {
                break;
            }
            b /= 64;
        }
    }

    /// The lowest bucket not empty at or after `from`.
    #[inline]
    fn lowest(&self, from: usize) -> Option<usize> {
        let (mut l, mut pos) = (0, from);
        // up to a level with a bit set at or after the position
        loop {
            let m = *self.marks[l].get(pos / 64)? & (u64::MAX << (pos % 64));
            if m != 0 {
                pos = pos / 64 * 64 + m.trailing_zeros() as usize;
                break;
            }
            pos = pos / 64 + 1;
            l += 1;
            if l == self.marks.len() {
                return None;
            }
        }
        // and down to the bucket
        while l > 0 {
            l -= 1;
            pos = pos * 64 + self.marks[l][pos].trailing_zeros() as usize;
        }
        Some(pos)
    }

    /// The first cell of the lowest bucket, taken out.
    #[inline]
    fn pop(&mut self, slots: &[Slot]) -> Option<u32> {
        let b = self.lowest(self.at)?;
        self.at = b;
        let e = &mut self.ends[b];
        let cell = e[0];
        let nx = slots[cell as usize].next;
        if nx == NONE {
            *e = [NONE; 2];
            self.unmark(b);
        } else {
            e[0] = nx;
        }
        Some(cell)
    }

    /// The cell that comes out next if nothing goes in lower (a guess, to fetch its memory).
    #[inline]
    fn peek(&self) -> Option<u32> {
        let b = self.lowest(self.at)?;
        Some(self.ends[b][0])
    }
}

/// [`flood`] with no compactness: the priority of a cell is its gradient, whichever marker reaches it, and it is
/// queued once, so the label goes on at the queueing and the queue is one bucket a distinct gradient (no sorting of
/// the queue, no heap). The same labels as the heap's, as a cell comes out by (gradient, the order it went in).
fn flood_buckets(p: &Prep, order: &[(u64, u32)], markers: &[Vec<usize>], eight: bool) -> Vec<u32> {
    let (w, h) = (p.w, p.h);
    let n = w * h;
    assert!(n < 1 << 28, "segment.watershed: an image of more than 2^28 cells");
    let mut slots: Vec<Slot> =
        (0..n).into_par_iter().map(|i| Slot { lab: if p.valid[i] { 0 } else { NONE }, rank: 0, next: NONE }).collect();
    // the bucket of each cell with a value: the rank of its gradient among the distinct ones
    let mut count = 0usize;
    let mut last = u64::MAX;
    for &(bits, i) in order {
        if bits != last {
            count += 1;
            last = bits;
        }
        slots[i as usize].rank = count as u32 - 1;
    }
    for (k, m) in markers.iter().enumerate() {
        for &i in m {
            slots[i].lab = k as u32 + 1;
        }
    }
    let mut q = Buckets::new(count);
    let mut nb = [0usize; 8];
    let mut put = |i: usize, k: u32, slots: &mut [Slot], q: &mut Buckets| {
        let k8 = around_at(i, i / w, i % w, w, h, eight, &mut nb);
        for &j in &nb[..k8] {
            if slots[j].lab == 0 {
                slots[j].lab = k;
                q.push(slots, j as u32);
            }
        }
    };
    for (k, m) in markers.iter().enumerate() {
        for &i in m {
            put(i, k as u32 + 1, &mut slots, &mut q);
        }
    }
    while let Some(j) = q.pop(&slots) {
        // (the cell to come out after this one: its surroundings are fetched meanwhile)
        if let Some(x) = q.peek() {
            let x = x as usize;
            prefetch(slots.as_ptr().wrapping_add(x));
            prefetch(slots.as_ptr().wrapping_add(x).wrapping_sub(w));
            prefetch(slots.as_ptr().wrapping_add(x + w));
        }
        let j = j as usize;
        put(j, slots[j].lab, &mut slots, &mut q);
    }
    slots.par_iter().map(|s| if s.lab == NONE { 0 } else { s.lab }).collect()
}

/// [`flood`] by the heap (any compactness; the buckets do for 0).
pub(super) fn flood_heap(p: &Prep, grad: &[f64], markers: &[Vec<usize>], eight: bool, compact: f64) -> Vec<u32> {
    let (w, h) = (p.w, p.h);
    // the queue numbers its pushes in 32 bits, at most 8 a cell (`run` refuses bigger images)
    assert!(w * h < 1 << 28, "segment.watershed: an image of more than 2^28 cells");
    let mut cells = cells_of(p);
    let centre: Vec<(f64, f64)> = markers
        .iter()
        .map(|m| {
            let n = m.len() as f64;
            (m.iter().map(|&i| (i / w) as f64).sum::<f64>() / n, m.iter().map(|&i| (i % w) as f64).sum::<f64>() / n)
        })
        .collect();
    for (k, m) in markers.iter().enumerate() {
        for &i in m {
            cells[i].lab = k as u32 + 1;
        }
    }
    // The queue holds (priority bits, push number << 32 | cell), see `Queue`. The label of push number q is `owner[q]`.
    // A cell is queued only when its priority is below every one queued for it so far: a later push with a priority no
    // lower would come out after the first, when the cell is labelled already.
    let mut heap = Queue::with_capacity(w * h / 4);
    let mut owner: Vec<u32> = Vec::with_capacity(w * h / 4);
    let mut nb = [0usize; 8];
    let mut push = |i: usize, k: u32, cells: &mut [Cell], heap: &mut Queue, owner: &mut Vec<u32>| {
        let k8 = around_at(i, i / w, i % w, w, h, eight, &mut nb);
        for &j in &nb[..k8] {
            let cell = &mut cells[j];
            if cell.lab != 0 {
                continue;
            }
            let mut pri;
            if compact > 0.0 {
                pri = grad[j];
                let (cr, cc) = centre[k as usize - 1];
                pri += compact * (((j / w) as f64 - cr).powi(2) + ((j % w) as f64 - cc).powi(2)).sqrt();
            } else if cell.low == f64::INFINITY {
                // (the priority is the gradient whichever label comes: the first push is the only one)
                pri = grad[j];
            } else {
                continue;
            }
            if pri < cell.low {
                cell.low = pri;
                heap.push(Queue::key(pri, owner.len(), j));
                owner.push(k);
            }
        }
    };
    for (k, m) in markers.iter().enumerate() {
        for &i in m {
            push(i, k as u32 + 1, &mut cells, &mut heap, &mut owner);
        }
    }
    while let Some(q) = heap.pop() {
        let j = Queue::cell(q);
        if cells[j].lab != 0 {
            continue;
        }
        // (the next cell to come out is likely this one's neighbour work after: its memory is fetched meanwhile)
        if let Some(nx) = heap.peek() {
            let x = Queue::cell(nx);
            prefetch(cells.as_ptr().wrapping_add(x));
            prefetch(cells.as_ptr().wrapping_add(x).wrapping_sub(w));
            prefetch(cells.as_ptr().wrapping_add(x + w));
            prefetch(grad.as_ptr().wrapping_add(x));
            prefetch(grad.as_ptr().wrapping_add(x).wrapping_sub(w));
            prefetch(grad.as_ptr().wrapping_add(x + w));
            prefetch(owner.as_ptr().wrapping_add(Queue::push_of(nx)));
        }
        let k = owner[Queue::push_of(q)];
        cells[j].lab = k;
        push(j, k, &mut cells, &mut heap, &mut owner);
    }
    labels_of(&cells)
}

/// `segment.watershed`.
struct Watershed;

impl Operator for Watershed {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.watershed",
            "Objects as the watershed of the image's gradient (Vincent & Soille 1991, as skimage's watershed), \
             marker-controlled or not. The bands are standardised (z-score over the cells with a value) by default, so \
             that bands of different ranges weigh alike, and smoothed by a Gaussian of sigma cells; the gradient is \
             the Sobel magnitude of each band, the largest over the bands. Without markers, they are the minima of \
             the gradient that stand h or more above the lowest cell they would join (h-minima: a minimum shallower \
             than h is noise and floods with its neighbour); with a map of markers, the cells with a value are the \
             seeds, a group of connected cells of one value being one marker. The markers flood by a queue of the \
             lowest gradient first (a cell takes the marker that reaches it first, ties to the cell queued first), \
             4 or 8 neighbours, with no watershed lines: every cell with a value gets a marker's label (areas cut off \
             from every marker by cells without a value stay apart as objects of their own). compactness above 0 \
             makes the watershed compact (Neubert & Protzel 2014): the queue's priority is the gradient plus \
             compactness times the distance in cells to the marker's centre. The objects are made 4-connected \
             whatever the neighbours: a marker's area linked only by a corner splits. A cell without a value in \
             every band is no object. The same result on every run, on any number of threads.",
            Access::Global,
        )
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands, the colour"))
        .input(PortSpec::optional(
            "markers",
            Type::Map,
            "a map of the seeds: cells with a value are markers (a group of connected cells of one value is one); \
             missing elsewhere. Without it the markers are found by the gradient's h-minima",
        ))
        .input(super::zones::port(false))
        .output(PortSpec::new("objects", Type::Objects, "the basins, each one connected"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per object: id, cells, area (map units squared), and mean_<band> and sd_<band> for each band",
        ))
        .param(PortSpec::optional("standardise", Type::Bool, "standardise each band (z-score) first (default true)"))
        .param(PortSpec::optional("sigma", Type::Number, "the smoothing's Gaussian, in cells (default 1; 0 none)"))
        .param(PortSpec::optional(
            "h",
            Type::Number,
            "without markers, the least depth of a minimum of the gradient to be a marker, in the gradient's units \
             (default 0.05 of the gradient's range: a minimum under a twentieth of the strongest edge is noise; 0 \
             makes every minimum a marker, with a plateau one)",
        ))
        .param(
            PortSpec::optional(
                "neighbours",
                Type::Integer,
                "which cells the flooding passes between: 4 or 8 (default 8)",
            )
            .choices([4, 8]),
        )
        .param(PortSpec::optional(
            "compactness",
            Type::Number,
            "0 (default) is the plain watershed; above it the basins are more regular. The gradient is in standard \
             deviations per cell when standardised, so values near 0.01 to 0.1 matter",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let sigma: f64 = args.param("sigma", 1.0)?;
        if !sigma.is_finite() || sigma < 0.0 {
            return Err(bad(format!("sigma {sigma}: 0 or more")));
        }
        let compact: f64 = args.param("compactness", 0.0)?;
        if !compact.is_finite() || compact < 0.0 {
            return Err(bad(format!("compactness {compact}: 0 or more")));
        }
        let hp = args.opt_param::<f64>("h")?;
        if let Some(h) = hp
            && (!h.is_finite() || h < 0.0)
        {
            return Err(bad(format!("h {h}: 0 or more")));
        }
        let nbs: i64 = args.param("neighbours", 8)?;
        if nbs != 4 && nbs != 8 {
            return Err(bad(format!("neighbours {nbs}: 4 or 8")));
        }
        let l = crate::raster_stats::layers(args, "image")?;
        if l.grid.width * l.grid.height >= 1 << 28 {
            return Err(bad(format!(
                "an image of {} × {} cells, more than segment.watershed works in memory",
                l.grid.width, l.grid.height
            )));
        }
        let standardise: bool = args.param("standardise", true)?;
        super::warnings::image(ctx, args.node, &l.grid, &l.names, &l.data, standardise);
        let mut p = prep(&l, standardise);
        if sigma > 0.0 {
            smooth(&mut p, sigma);
        }
        let grad = gradient(&p);
        let eight = nbs == 8;
        // (the cells by gradient: the markers found and the flooding without compactness both go by it)
        let order = (compact == 0.0 || args.get("markers").is_none()).then(|| order_of(&p, &grad));
        let markers = match args.get("markers") {
            Some(Value::Map(m)) => {
                if !m.grid.same_as(&l.grid) {
                    return Err(bad("the markers are not on the grid of the image".into()));
                }
                given(&p, &m.data)
            }
            Some(v) => return Err(bad(format!("the markers are a {:?}, not a map", v.ty()))),
            None => {
                let (lo, hi) = (0..p.w * p.h)
                    .filter(|&i| p.valid[i])
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), i| (a.min(grad[i]), b.max(grad[i])));
                let h = hp.unwrap_or(if lo <= hi { 0.05 * (hi - lo) } else { 0.0 });
                minima_in(&p, order.as_deref().unwrap_or_default(), eight, h)
            }
        };
        let lab = match order.as_deref() {
            Some(o) => flood_in(&p, &grad, o, &markers, eight, compact),
            None => flood_heap(&p, &grad, &markers, eight, compact),
        };
        let region: Vec<u64> = (0..p.w * p.h)
            .map(|i| match (p.valid[i], lab[i]) {
                (false, _) => NO_REGION,
                (true, 0) => LOOSE,
                (true, k) => k as u64,
            })
            .collect();
        finish(args, &l, &region)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bucket queue gives the heap's labels, on gradients with many ties and cells without a value.
    #[test]
    fn the_buckets_flood_as_the_heap_does() {
        for (seed, levels) in [(1u64, 3u64), (2, 6), (3, 40), (4, 1_000_000)] {
            let (w, h) = (53, 41);
            let mut s = seed;
            let mut rnd = move || {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                s >> 33
            };
            let valid: Vec<bool> = (0..w * h).map(|_| rnd() % 11 != 0).collect();
            let grad: Vec<f64> = (0..w * h).map(|_| (rnd() % levels) as f64 / 4.0).collect();
            let p = Prep { w, h, nb: 1, valid, x: Vec::new() };
            for eight in [false, true] {
                let order = order_of(&p, &grad);
                for hh in [0.0, 0.5] {
                    let markers = minima_in(&p, &order, eight, hh);
                    assert!(!markers.is_empty());
                    let a = flood_heap(&p, &grad, &markers, eight, 0.0);
                    let b = flood_buckets(&p, &order, &markers, eight);
                    assert_eq!(a, b, "seed {seed}, levels {levels}, eight {eight}, h {hh}");
                    assert_eq!(a, flood(&p, &grad, &markers, eight, 0.0));
                }
            }
        }
    }
}
