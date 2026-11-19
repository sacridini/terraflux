//! `segment.watershed`: the watershed of the gradient of an image, with markers given or found.
//!
//! The image is standardised (z-score per band, unless told otherwise) and smoothed by a Gaussian of `sigma` cells
//! (superpixels.rs's, which ignores the cells without a value). The gradient of a cell is the Sobel magnitude of each
//! band (per cell, so the units are standard deviations per cell when standardised), and the largest over the bands:
//! an edge in any one band counts in full, and adding bands does not dilute it as the Euclidean norm would. Edges and
//! cells without a value count as the cell itself in the Sobel window. One thread: the same result on every run.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::superpixels::{Prep, finish, prep, smooth};
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
    for r in 0..h {
        for c in 0..w {
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
            g[i] = best;
        }
    }
    g
}

/// The neighbours of a cell (4 or 8) in a grid of `w` x `h`: written to `out`, how many there are.
fn around(i: usize, w: usize, h: usize, eight: bool, out: &mut [usize; 8]) -> usize {
    let (r, c) = (i / w, i % w);
    let mut n = 0;
    for dr in -1i64..=1 {
        for dc in -1i64..=1 {
            if (dr, dc) == (0, 0) || (!eight && dr != 0 && dc != 0) {
                continue;
            }
            let (rr, cc) = (r as i64 + dr, c as i64 + dc);
            if rr >= 0 && cc >= 0 && rr < h as i64 && cc < w as i64 {
                out[n] = rr as usize * w + cc as usize;
                n += 1;
            }
        }
    }
    n
}

/// The markers found by the gradient: the cells of its minima that stand `h` or more above the lowest cell they
/// join (their dynamics, the h-minima). The cells go from the lowest up, and a cell joins the components of the cells
/// before it that touch it; when two meet the one with the higher minimum (the younger) ends, its depth being the level
/// of the meeting less its minimum. Each area cut off by cells without a value keeps its lowest minimum whatever its
/// depth. The marker of a minimum is its cells at its lowest value (a plateau is one marker).
fn minima(p: &Prep, grad: &[f64], eight: bool, h: f64) -> Vec<Vec<usize>> {
    let (w, hh) = (p.w, p.h);
    let n = w * hh;
    let mut order: Vec<(u64, u32)> = (0..n).filter(|&i| p.valid[i]).map(|i| (grad[i].to_bits(), i as u32)).collect();
    // (the gradient is never negative, so its bits sort as the values)
    order.sort_unstable();
    let mut parent: Vec<u32> = Vec::new();
    let mut low: Vec<f64> = Vec::new();
    let mut depth: Vec<f64> = Vec::new();
    let mut comp = vec![u32::MAX; n];
    let mut origin = vec![u32::MAX; n];
    let mut nb = [0usize; 8];
    let mut roots: Vec<u32> = Vec::with_capacity(8);
    for &(_, i) in &order {
        let i = i as usize;
        let v = grad[i];
        roots.clear();
        let k = around(i, w, hh, eight, &mut nb);
        for &j in &nb[..k] {
            if comp[j] != u32::MAX {
                let r = find(&mut parent, comp[j]);
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
            comp[i] = id;
            origin[i] = id;
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
        comp[i] = elder;
        if v == low[elder as usize] {
            origin[i] = elder;
        }
    }
    let mut index = vec![u32::MAX; parent.len()];
    let mut out: Vec<Vec<usize>> = Vec::new();
    for (i, &k) in origin.iter().enumerate() {
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
fn flood(p: &Prep, grad: &[f64], markers: &[Vec<usize>], eight: bool, compact: f64) -> Vec<u32> {
    let (w, h) = (p.w, p.h);
    let mut lab = vec![0u32; w * h];
    let centre: Vec<(f64, f64)> = markers
        .iter()
        .map(|m| {
            let n = m.len() as f64;
            (m.iter().map(|&i| (i / w) as f64).sum::<f64>() / n, m.iter().map(|&i| (i % w) as f64).sum::<f64>() / n)
        })
        .collect();
    for (k, m) in markers.iter().enumerate() {
        for &i in m {
            lab[i] = k as u32 + 1;
        }
    }
    let mut heap: BinaryHeap<Reverse<(u64, u64, u32, u32)>> = BinaryHeap::new();
    let mut seq = 0u64;
    let mut nb = [0usize; 8];
    let mut push = |i: usize, k: u32, lab: &[u32], heap: &mut BinaryHeap<Reverse<(u64, u64, u32, u32)>>| {
        let k8 = around(i, w, h, eight, &mut nb);
        for &j in &nb[..k8] {
            if p.valid[j] && lab[j] == 0 {
                let mut pri = grad[j];
                if compact > 0.0 {
                    let (cr, cc) = centre[k as usize - 1];
                    pri += compact * (((j / w) as f64 - cr).powi(2) + ((j % w) as f64 - cc).powi(2)).sqrt();
                }
                heap.push(Reverse((pri.to_bits(), seq, j as u32, k)));
                seq += 1;
            }
        }
    };
    for (k, m) in markers.iter().enumerate() {
        for &i in m {
            push(i, k as u32 + 1, &lab, &mut heap);
        }
    }
    while let Some(Reverse((_, _, j, k))) = heap.pop() {
        let j = j as usize;
        if lab[j] != 0 {
            continue;
        }
        lab[j] = k;
        push(j, k, &lab, &mut heap);
    }
    lab
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
             every band is no object. One thread: the same result on every run.",
            Access::Global,
        )
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands, the colour"))
        .input(PortSpec::optional(
            "markers",
            Type::Map,
            "a map of the seeds: cells with a value are markers (a group of connected cells of one value is one); \
             missing elsewhere. Without it the markers are found by the gradient's h-minima",
        ))
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
        let standardise: bool = args.param("standardise", true)?;
        super::warnings::image(ctx, args.node, &l.grid, &l.names, &l.data, standardise);
        let mut p = prep(&l, standardise);
        if sigma > 0.0 {
            smooth(&mut p, sigma);
        }
        let grad = gradient(&p);
        let eight = nbs == 8;
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
                minima(&p, &grad, eight, h)
            }
        };
        let lab = flood(&p, &grad, &markers, eight, compact);
        let region: Vec<u64> = (0..p.w * p.h)
            .map(|i| match (p.valid[i], lab[i]) {
                (false, _) => NO_REGION,
                (true, 0) => LOOSE,
                (true, k) => k as u64,
            })
            .collect();
        finish(&l, &region)
    }
}
