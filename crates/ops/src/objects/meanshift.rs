//! `segment.meanshift`: mean-shift segmentation (Comaniciu & Meer 2002) in the spirit of OTB's large-scale mean shift.
//!
//! Three steps. The filter moves each cell's point in the joint domain (position and bands) to the mean of the cells
//! within `spatial_radius` cells and `range_radius` in the bands (a flat kernel), again and again until it hardly
//! moves, and keeps the bands of where it stopped. Then cells next to each other (4 neighbours) whose filtered bands
//! are within `range_radius` of the bands' mean of the region they join make one region. Then each region under
//! `min_size` cells joins the neighbour it is most alike. The filter of a cell does not depend on the others', so it
//! runs by rows on the shared pool (the central cap of threads) with the result of one thread, whatever their number.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::superpixels::{Prep, finish, prep};
use super::{NO_REGION, Objects, find};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(MeanShift));
}

/// The bands of each cell after the filter (`nb` values a cell; 0 for a cell without a value).
fn filter(p: &Prep, hs: f64, hr: f64, iterations: usize, threshold: f64) -> Vec<f64> {
    let (w, h, nb) = (p.w, p.h, p.nb);
    let (hs2, hr2) = (hs * hs, hr * hr);
    let mut out = vec![0.0; w * h * nb];
    out.par_chunks_mut(w * nb).enumerate().for_each_init(
        || vec![0.0f64; nb * 2],
        |buf, (r, row)| {
            let (cur, acc) = buf.split_at_mut(nb);
            for c in 0..w {
                let i = r * w + c;
                if !p.valid[i] {
                    continue;
                }
                cur.copy_from_slice(p.feat(i));
                let (mut yr, mut yc) = (r as f64, c as f64);
                for _ in 0..iterations {
                    acc.iter_mut().for_each(|a| *a = 0.0);
                    let (mut sr, mut sc, mut n) = (0.0f64, 0.0f64, 0u32);
                    let r0 = (yr - hs).ceil().max(0.0) as usize;
                    let r1 = (yr + hs).floor().min((h - 1) as f64) as usize;
                    for rr in r0..=r1 {
                        let dy = rr as f64 - yr;
                        let dx = (hs2 - dy * dy).max(0.0).sqrt();
                        let c0 = (yc - dx).ceil().max(0.0) as usize;
                        let c1 = (yc + dx).floor().min((w - 1) as f64) as usize;
                        for cc in c0..=c1 {
                            let j = rr * w + cc;
                            if !p.valid[j] {
                                continue;
                            }
                            let f = p.feat(j);
                            let mut d2 = 0.0;
                            for b in 0..nb {
                                let d = f[b] - cur[b];
                                d2 += d * d;
                            }
                            if d2 <= hr2 {
                                for b in 0..nb {
                                    acc[b] += f[b];
                                }
                                sr += rr as f64;
                                sc += cc as f64;
                                n += 1;
                            }
                        }
                    }
                    if n == 0 {
                        break;
                    }
                    let n = n as f64;
                    let (nr, nc) = (sr / n, sc / n);
                    let mut shift = (nr - yr).powi(2) + (nc - yc).powi(2);
                    for b in 0..nb {
                        let v = acc[b] / n;
                        shift += (v - cur[b]).powi(2);
                        cur[b] = v;
                    }
                    (yr, yc) = (nr, nc);
                    if shift.sqrt() < threshold {
                        break;
                    }
                }
                row[c * nb..(c + 1) * nb].copy_from_slice(cur);
            }
        },
    );
    out
}

/// The regions of the filtered cells: grown from each cell not yet in one, scan order, over 4 neighbours, taking a cell
/// whose bands are within `hr` of the mean of the region so far.
fn group(p: &Prep, f: &[f64], hr: f64) -> Vec<u64> {
    let (w, h, nb) = (p.w, p.h, p.nb);
    let hr2 = hr * hr;
    let mut region = vec![NO_REGION; w * h];
    let mut next = 0u64;
    let mut sum = vec![0.0f64; nb];
    let mut stack: Vec<usize> = Vec::new();
    for s in 0..w * h {
        if !p.valid[s] || region[s] != NO_REGION {
            continue;
        }
        region[s] = next;
        sum.copy_from_slice(&f[s * nb..(s + 1) * nb]);
        let mut n = 1.0f64;
        stack.push(s);
        while let Some(i) = stack.pop() {
            let (r, c) = (i / w, i % w);
            let cand = [
                (r > 0).then(|| i - w),
                (c > 0).then(|| i - 1),
                (c + 1 < w).then(|| i + 1),
                (r + 1 < h).then(|| i + w),
            ];
            for j in cand.into_iter().flatten() {
                if !p.valid[j] || region[j] != NO_REGION {
                    continue;
                }
                let d2: f64 = (0..nb).map(|b| (f[j * nb + b] - sum[b] / n).powi(2)).sum();
                if d2 <= hr2 {
                    region[j] = next;
                    for b in 0..nb {
                        sum[b] += f[j * nb + b];
                    }
                    n += 1.0;
                    stack.push(j);
                }
            }
        }
        next += 1;
    }
    region
}

/// Each object under `min` cells joined to its most alike neighbour (the nearest bands' mean; ties to the lowest id),
/// the smallest first; a region that grows by it and is still small waits its turn again. An object with no neighbour
/// stays. The regions to give to `Objects::from_regions`.
fn merge(o: &Objects, f: &[f64], nb: usize, min: u32) -> Vec<u64> {
    let n = o.count;
    let mut parent: Vec<u32> = (0..=n as u32).collect();
    let mut size: Vec<u32> = std::iter::once(0).chain(o.cells.iter().copied()).collect();
    let mut sum = vec![0.0f64; (n + 1) * nb];
    for (i, &l) in o.labels.iter().enumerate() {
        if l != 0 {
            for b in 0..nb {
                sum[l as usize * nb + b] += f[i * nb + b];
            }
        }
    }
    let mut adj: Vec<Vec<u32>> = std::iter::once(Vec::new())
        .chain((1..=n as u32).map(|id| o.neighbours_of(id).iter().map(|&(a, _)| a).collect()))
        .collect();
    let mut heap: BinaryHeap<Reverse<(u32, u32)>> =
        (1..=n as u32).filter(|&id| size[id as usize] < min).map(|id| Reverse((size[id as usize], id))).collect();
    while let Some(Reverse((s, id))) = heap.pop() {
        if parent[id as usize] != id || size[id as usize] != s {
            continue;
        }
        let me = id as usize;
        let mut list = std::mem::take(&mut adj[me]);
        for a in list.iter_mut() {
            *a = find(&mut parent, *a);
        }
        list.sort_unstable();
        list.dedup();
        list.retain(|&a| a != id);
        let mut best: Option<(f64, u32)> = None;
        for &a in &list {
            let d: f64 = (0..nb)
                .map(|b| (sum[a as usize * nb + b] / size[a as usize] as f64 - sum[me * nb + b] / s as f64).powi(2))
                .sum();
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, a));
            }
        }
        let Some((_, to)) = best else {
            adj[me] = list;
            continue;
        };
        parent[me] = to;
        size[to as usize] += s;
        for b in 0..nb {
            sum[to as usize * nb + b] += sum[me * nb + b];
        }
        adj[to as usize].extend(list);
        if size[to as usize] < min {
            heap.push(Reverse((size[to as usize], to)));
        }
    }
    o.labels.iter().map(|&l| if l == 0 { NO_REGION } else { find(&mut parent, l) as u64 }).collect()
}

/// `segment.meanshift`.
struct MeanShift;

impl Operator for MeanShift {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.meanshift",
            "Objects by mean shift (Comaniciu & Meer 2002), as OTB's large-scale mean-shift segmentation: the filter \
             moves each cell's point in the joint domain (position, bands) to the mean of the cells within \
             spatial_radius cells and range_radius in the bands (a flat kernel), until the move is under threshold or \
             max_iterations; cells next to each other (4 neighbours) whose filtered bands are within range_radius of \
             the mean of the region they join make a region; regions under min_size cells join the neighbour they \
             are most alike. The bands are standardised (z-score over the cells with a value) by default, so that \
             range_radius is in standard deviations and weighs the bands alike; standardise: false uses the raw \
             values (and a range_radius in them). The cost is about cells x spatial_radius² x bands x iterations \
             (1000 x 1000 cells, 4 bands and the defaults: a few seconds on the cap of threads); the filter of a \
             cell is its own, so the result is the same for any number of threads. A cell without a value in every \
             band is no object.",
            Access::Global,
        )
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands, the colour"))
        .input(super::zones::port(false))
        .output(PortSpec::new("objects", Type::Objects, "the regions, each one connected"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per object: id, cells, area (map units squared), and mean_<band> and sd_<band> for each band",
        ))
        .param(PortSpec::optional("standardise", Type::Bool, "standardise each band (z-score) first (default true)"))
        .param(PortSpec::optional(
            "spatial_radius",
            Type::Number,
            "hs, the radius in cells of the window of the filter (default 5): larger smooths more and costs its square",
        ))
        .param(PortSpec::optional(
            "range_radius",
            Type::Number,
            "hr, the distance in the bands (Euclidean) within which cells count as alike (default 0.5: half a \
             standard deviation of the bands on the standardised ones)",
        ))
        .param(PortSpec::optional("max_iterations", Type::Integer, "the most moves of the filter (default 10)"))
        .param(PortSpec::optional(
            "threshold",
            Type::Number,
            "the filter stops when a move (cells and band units together) is under this (default 0.01)",
        ))
        .param(PortSpec::optional(
            "min_size",
            Type::Integer,
            "the least cells of a region; smaller ones join the most alike neighbour (default 20; 1 none)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let hs: f64 = args.param("spatial_radius", 5.0)?;
        if !hs.is_finite() || hs < 1.0 {
            return Err(bad(format!("spatial_radius {hs}: 1 cell or more")));
        }
        let hr: f64 = args.param("range_radius", 0.5)?;
        if !hr.is_finite() || hr <= 0.0 {
            return Err(bad(format!("range_radius {hr}: above 0")));
        }
        let iters: i64 = args.param("max_iterations", 10)?;
        if iters < 1 {
            return Err(bad(format!("max_iterations {iters}: 1 or more")));
        }
        let thr: f64 = args.param("threshold", 0.01)?;
        if !thr.is_finite() || thr < 0.0 {
            return Err(bad(format!("threshold {thr}: 0 or more")));
        }
        let min: i64 = args.param("min_size", 20)?;
        if min < 1 {
            return Err(bad(format!("min_size {min}: 1 or more")));
        }
        let l = crate::raster_stats::layers(args, "image")?;
        let standardise: bool = args.param("standardise", true)?;
        super::warnings::image(ctx, args.node, &l.grid, &l.names, &l.data, standardise);
        let p = prep(&l, standardise);
        let f = filter(&p, hs, hr, iters as usize, thr);
        let first = Objects::from_regions(l.grid.clone(), &group(&p, &f, hr), 4);
        finish(args, &l, &merge(&first, &f, p.nb, min.min(u32::MAX as i64) as u32))
    }
}
