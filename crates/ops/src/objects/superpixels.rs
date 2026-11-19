//! `segment.slic`, `segment.snic` and `segment.felzenszwalb`: superpixels and graph-based segments.
//!
//! All three work on the bands standardised (z-score over the cells with a value) unless told otherwise: the distance
//! between two cells sums the squared differences of the bands, so a band of range 0-10000 would drown one of range
//! 0-1; on z-scores every band counts alike. A cell with a value in some bands only takes the band's mean (0 after the
//! z-score) in the others. A cell without a value in every band is no object. One thread: the same result on every run.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::{NO_REGION, Objects, find};
use crate::raster_stats::Layers;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

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
    let valid: Vec<bool> = (0..n).map(|i| l.data.iter().any(|b| !b[i].is_nan())).collect();
    let mut x = vec![0.0; n * nb];
    for (j, b) in l.data.iter().enumerate() {
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
        let scale = if standardise && sd > 0.0 { 1.0 / sd } else { 1.0 };
        for i in 0..n {
            let v = b[i];
            if !v.is_nan() {
                x[i * nb + j] = if standardise { (v - mean) * scale } else { v };
            } else if !standardise {
                x[i * nb + j] = mean;
            }
        }
    }
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

/// SLIC (Achanta et al. 2012): k-means on the bands and the position, each centre looking only at a window of 2S x 2S
/// around it; D² = d_colour² + (d_space / S)² m² (d_colour² the mean over the bands of the squared gaps). Returns the superpixel of each cell (0 none).
fn slic(p: &Prep, seeds: &[usize], s: f64, m: f64, iterations: usize) -> Vec<u32> {
    let (w, h, nb) = (p.w, p.h, p.nb);
    let stride = nb + 2;
    let mut cen = vec![0.0f64; seeds.len() * stride];
    for (k, &i) in seeds.iter().enumerate() {
        cen[k * stride] = (i / w) as f64;
        cen[k * stride + 1] = (i % w) as f64;
        cen[k * stride + 2..(k + 1) * stride].copy_from_slice(p.feat(i));
    }
    let wt = m * m * nb as f64 / (s * s);
    let mut lab = vec![0u32; w * h];
    for _ in 0..iterations {
        let mut dist = vec![f64::INFINITY; w * h];
        for k in 0..seeds.len() {
            let c = &cen[k * stride..(k + 1) * stride];
            let r0 = (c[0] - s).floor().max(0.0) as usize;
            let r1 = ((c[0] + s).ceil() as usize).min(h - 1);
            let c0 = (c[1] - s).floor().max(0.0) as usize;
            let c1 = ((c[1] + s).ceil() as usize).min(w - 1);
            for r in r0..=r1 {
                for q in c0..=c1 {
                    let i = r * w + q;
                    if !p.valid[i] {
                        continue;
                    }
                    let dc: f64 = p.feat(i).iter().zip(&c[2..]).map(|(a, b)| (a - b) * (a - b)).sum();
                    let (dr, dq) = (r as f64 - c[0], q as f64 - c[1]);
                    let d = dc + (dr * dr + dq * dq) * wt;
                    if d < dist[i] {
                        dist[i] = d;
                        lab[i] = k as u32 + 1;
                    }
                }
            }
        }
        // the centres move to the mean of their cells
        let mut sum = vec![0.0f64; seeds.len() * stride];
        let mut cnt = vec![0u32; seeds.len()];
        for (i, &l) in lab.iter().enumerate() {
            let l = l as usize;
            if l == 0 {
                continue;
            }
            let k = l - 1;
            cnt[k] += 1;
            let t = &mut sum[k * stride..(k + 1) * stride];
            t[0] += (i / w) as f64;
            t[1] += (i % w) as f64;
            for (a, b) in t[2..].iter_mut().zip(p.feat(i)) {
                *a += b;
            }
        }
        for k in 0..seeds.len() {
            if cnt[k] > 0 {
                for q in 0..stride {
                    cen[k * stride + q] = sum[k * stride + q] / cnt[k] as f64;
                }
            }
        }
    }
    lab
}

/// SNIC (Achanta & Susstrunk 2017): the superpixels grow from the seeds by a queue of cells ordered by their distance to
/// the superpixel's centroid, which is updated as each cell joins; 4 neighbours; the same distance as SLIC. Ties go to
/// the cell queued first. Returns the superpixel of each cell (0 none).
fn snic(p: &Prep, seeds: &[usize], s: f64, m: f64) -> Vec<u32> {
    let (w, h, nb) = (p.w, p.h, p.nb);
    let stride = nb + 3;
    // per superpixel: count, row sum, column sum, band sums
    let mut sum = vec![0.0f64; seeds.len() * stride];
    let wt = m * m * nb as f64 / (s * s);
    let mut lab = vec![0u32; w * h];
    let mut heap: BinaryHeap<Reverse<(u64, u64, u32, u32)>> = BinaryHeap::new();
    let mut seq = 0u64;
    for (k, &i) in seeds.iter().enumerate() {
        heap.push(Reverse((0f64.to_bits(), seq, i as u32, k as u32)));
        seq += 1;
    }
    while let Some(Reverse((_, _, i, k))) = heap.pop() {
        let i = i as usize;
        if lab[i] != 0 {
            continue;
        }
        lab[i] = k + 1;
        let t = &mut sum[k as usize * stride..(k as usize + 1) * stride];
        t[0] += 1.0;
        t[1] += (i / w) as f64;
        t[2] += (i % w) as f64;
        for (a, b) in t[3..].iter_mut().zip(p.feat(i)) {
            *a += b;
        }
        let t = &sum[k as usize * stride..(k as usize + 1) * stride];
        let (r, c) = (i / w, i % w);
        let (mr, mc) = (t[1] / t[0], t[2] / t[0]);
        let mut go = |j: usize| {
            if !p.valid[j] || lab[j] != 0 {
                return;
            }
            let dc: f64 = p.feat(j).iter().zip(&t[3..]).map(|(a, b)| (a - b / t[0]) * (a - b / t[0])).sum();
            let (dr, dq) = ((j / w) as f64 - mr, (j % w) as f64 - mc);
            let d = dc + (dr * dr + dq * dq) * wt;
            heap.push(Reverse((d.to_bits(), seq, j as u32, k)));
            seq += 1;
        };
        if r > 0 {
            go(i - w);
        }
        if c > 0 {
            go(i - 1);
        }
        if c + 1 < w {
            go(i + 1);
        }
        if r + 1 < h {
            go(i + w);
        }
    }
    lab
}

/// The regions of a labelling (0 where a cell with a value has none: those are `LOOSE`).
fn regions_of(p: &Prep, lab: &[u32]) -> Vec<u64> {
    (0..p.w * p.h)
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
    o.labels.iter().map(|&l| if l == 0 { NO_REGION } else { find(&mut parent, l) as u64 }).collect()
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
    raw values. A cell without a value in every band is no object. One thread: the same result on every run.";

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
/// the weighted mean of those around it.
pub(super) fn smooth(p: &mut Prep, sigma: f64) {
    let (w, h, nb) = (p.w, p.h, p.nb);
    let rad = (4.0 * sigma).ceil() as i64;
    let ker: Vec<f64> = (-rad..=rad).map(|d| (-((d * d) as f64) / (2.0 * sigma * sigma)).exp()).collect();
    // one pass along lines of `len` cells, `step` apart, the lines starting `line_step` apart
    let pass = |valid: &[bool], src: &[f64], len: usize, step: usize, lines: usize, line_step: usize| -> Vec<f64> {
        let mut dst = src.to_vec();
        let mut acc = vec![0.0f64; nb];
        for line in 0..lines {
            for pos in 0..len {
                let i = line * line_step + pos * step;
                if !valid[i] {
                    continue;
                }
                acc.iter_mut().for_each(|a| *a = 0.0);
                let mut wsum = 0.0;
                for (q, &kw) in ker.iter().enumerate() {
                    let at = pos as i64 + q as i64 - rad;
                    if at < 0 || at >= len as i64 {
                        continue;
                    }
                    let j = line * line_step + at as usize * step;
                    if valid[j] {
                        wsum += kw;
                        for b in 0..nb {
                            acc[b] += kw * src[j * nb + b];
                        }
                    }
                }
                for b in 0..nb {
                    dst[i * nb + b] = acc[b] / wsum;
                }
            }
        }
        dst
    };
    let rows = pass(&p.valid, &p.x, w, 1, h, w);
    p.x = pass(&p.valid, &rows, h, w, w, 1);
}

/// Felzenszwalb & Huttenlocher's (2004) segments: returns the region of each cell.
fn felzenszwalb(p: &Prep, eight: bool, k: f64, min_size: usize) -> Vec<u64> {
    let (w, h) = (p.w, p.h);
    let mut edges: Vec<(f64, u32, u32)> = Vec::new();
    for r in 0..h {
        for c in 0..w {
            let i = r * w + c;
            if !p.valid[i] {
                continue;
            }
            let mut add = |j: usize| {
                if p.valid[j] {
                    edges.push((p.gap(i, j).sqrt(), i as u32, j as u32));
                }
            };
            if c + 1 < w {
                add(i + 1);
            }
            if r + 1 < h {
                add(i + w);
                if eight && c + 1 < w {
                    add(i + w + 1);
                }
                if eight && c > 0 {
                    add(i + w - 1);
                }
            }
        }
    }
    // stable: equal weights stay in the order the edges were made
    edges.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = w * h;
    let mut parent: Vec<u32> = (0..n as u32).collect();
    let mut size = vec![1u32; n];
    // the largest edge inside each component
    let mut inner = vec![0.0f64; n];
    for &(wt, a, b) in &edges {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra == rb {
            continue;
        }
        let (ia, ib) = (ra as usize, rb as usize);
        if wt <= (inner[ia] + k / size[ia] as f64).min(inner[ib] + k / size[ib] as f64) {
            parent[ib] = ra;
            size[ia] += size[ib];
            inner[ia] = wt;
        }
    }
    for &(_, a, b) in &edges {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb && (size[ra as usize] < min_size as u32 || size[rb as usize] < min_size as u32) {
            parent[rb as usize] = ra;
            size[ra as usize] += size[rb as usize];
        }
    }
    (0..n).map(|i| if p.valid[i] { find(&mut parent, i as u32) as u64 } else { NO_REGION }).collect()
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
