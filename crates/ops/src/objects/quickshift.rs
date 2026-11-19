//! `segment.quickshift`: quick shift, Vedaldi & Soatto's (2008) mode seeking, as scikit-image's `quickshift`.
//!
//! Each cell is a point of the joint space (column, row, ratio times the bands). Its density is a Parzen estimate with
//! a Gaussian of `kernel_size` over the cells within 3 sigma (a disc), and it links to the nearest cell of the joint
//! space that has a higher density, if that is within `max_dist`; the cells without such a link are the roots, and
//! each tree is a segment.
//!
//! Both steps are per cell and independent, so they run by rows in parallel with the same numbers on any number of
//! threads. The density is a sum over the window, row by row: the spatial part of the Gaussian is separable (a table
//! of one dimension), the colour part is one exponential per pair of cells, a polynomial of the kind the processor
//! runs on several cells at once, in single precision, with the bands in planes so a row of the window is contiguous.
//! The link looks at the cells in the order of their distance in space (a distance in the joint space is never
//! smaller) and stops at the first that is too far to beat the best found, so a cell that is not near a peak looks at a
//! handful. The trees become labels by following the links with their path halved.

use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::superpixels::{Prep, finish, prep, smooth};
use super::{NO_REGION, find};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Quickshift));
}

/// No link: the cell is a root (or has no value).
const NO: u32 = u32::MAX;

/// e^-t for t of 0 and above (a polynomial on 2^-t's fraction, a relative error under 2e-6, 0 for t over 80).
#[inline(always)]
fn exp_neg(t: f32) -> f32 {
    const LOG2E: f32 = std::f32::consts::LOG2_E;
    const LN2: f32 = std::f32::consts::LN_2;
    let y = t.min(80.0) * LOG2E;
    let i = y as i32;
    let z = (y - i as f32) * LN2;
    // e^-z, z in [0, 0.7)
    let p = 1.0
        + z * (-1.0
            + z * (0.5 + z * (-1.0 / 6.0 + z * (1.0 / 24.0 + z * (-1.0 / 120.0 + z * (1.0 / 720.0 - z / 5040.0))))));
    f32::from_bits(((127 - i) as u32) << 23) * p
}

/// The image for the two steps: the bands in planes (scaled so that the squared gap of two cells is the colour part of
/// the Gaussian's exponent), and the cells with a value as 1 or 0.
struct Joint {
    w: usize,
    h: usize,
    planes: Vec<Vec<f32>>,
    valid: Vec<bool>,
    weight: Vec<f32>,
}

fn joint(p: &Prep, ratio: f64, kernel: f64) -> Joint {
    let n = p.w * p.h;
    // the squared gap times ratio² / (2 sigma²) is the exponent: scale the bands by ratio / (sigma sqrt 2)
    let s = (ratio / (kernel * std::f64::consts::SQRT_2)) as f32;
    let planes: Vec<Vec<f32>> =
        (0..p.nb).into_par_iter().map(|b| (0..n).map(|i| p.x[i * p.nb + b] as f32 * s).collect()).collect();
    Joint {
        w: p.w,
        h: p.h,
        planes,
        valid: p.valid.clone(),
        weight: p.valid.iter().map(|&v| if v { 1.0 } else { 0.0 }).collect(),
    }
}

/// The density of every cell, rows in parallel. The window is the disc of radius `rad` (cells) around the cell.
fn densities(j: &Joint, kernel: f64, rad: usize) -> Vec<f32> {
    let (w, h) = (j.w, j.h);
    let side = 2 * rad + 1;
    // the Gaussian in space along one axis, from -rad to rad, and the half width of the disc at each row offset
    let g: Vec<f32> = (0..side)
        .map(|k| {
            let d = k as f64 - rad as f64;
            (-(d * d) / (2.0 * kernel * kernel)).exp() as f32
        })
        .collect();
    let half: Vec<usize> =
        (0..side).map(|k| ((rad * rad - (k as i64 - rad as i64).pow(2) as usize) as f64).sqrt() as usize).collect();
    let mut dens = vec![0.0f32; w * h];
    dens.par_chunks_mut(w.max(1)).enumerate().for_each_init(
        || (vec![0.0f32; side], vec![0.0f32; side]),
        |(d2, tmp), (r, row)| {
            for (c, out) in row.iter_mut().enumerate() {
                if !j.valid[r * w + c] {
                    continue;
                }
                let mut total = 0.0f32;
                let (r0, r1) = (r.saturating_sub(rad), (r + rad).min(h - 1));
                for rr in r0..=r1 {
                    let k = rr + rad - r;
                    let hw = half[k];
                    let (c0, c1) = (c.saturating_sub(hw), (c + hw).min(w - 1));
                    let len = c1 - c0 + 1;
                    let base = rr * w + c0;
                    let d2 = &mut d2[..len];
                    d2.fill(0.0);
                    for plane in &j.planes {
                        let xc = plane[r * w + c];
                        for (d, &x) in d2.iter_mut().zip(&plane[base..base + len]) {
                            let e = x - xc;
                            *d += e * e;
                        }
                    }
                    let gs = &g[c0 + rad - c..c0 + rad - c + len];
                    let vw = &j.weight[base..base + len];
                    let tmp = &mut tmp[..len];
                    for (((t, &d), &gk), &v) in tmp.iter_mut().zip(d2.iter()).zip(gs).zip(vw) {
                        *t = exp_neg(d) * gk * v;
                    }
                    // eight lanes, summed in a fixed order: the same number on any machine's vector width
                    let mut lanes = [0.0f32; 8];
                    // (chunks_exact: as_chunks needs a newer Rust than the crate's)
                    #[allow(clippy::chunks_exact_to_as_chunks)]
                    let mut ch = tmp.chunks_exact(8);
                    for c8 in &mut ch {
                        for l in 0..8 {
                            lanes[l] += c8[l];
                        }
                    }
                    let mut s = ((lanes[0] + lanes[4]) + (lanes[1] + lanes[5]))
                        + ((lanes[2] + lanes[6]) + (lanes[3] + lanes[7]));
                    for &x in ch.remainder() {
                        s += x;
                    }
                    total += s * g[k];
                }
                *out = total;
            }
        },
    );
    dens
}

/// Each cell's link: the nearest cell of the joint space of a higher density (a tie in density: the one of the lower
/// number is the higher) within `max_dist`, a tie in distance going to the lower number; `NO` for none.
fn links(j: &Joint, dens: &[f32], kernel: f64, max_dist: f64) -> Vec<u32> {
    let (w, h) = (j.w, j.h);
    let rad = max_dist.floor() as i64;
    // the offsets within the radius, by their squared distance, then by the cell's number
    let mut offs: Vec<(f32, i32, i32)> = Vec::new();
    for dr in -rad..=rad {
        for dc in -rad..=rad {
            let d2 = dr * dr + dc * dc;
            if (d2 as f64) <= max_dist * max_dist && (dr, dc) != (0, 0) {
                offs.push((d2 as f32, dr as i32, dc as i32));
            }
        }
    }
    offs.sort_by(|a, b| a.0.total_cmp(&b.0).then((a.1, a.2).cmp(&(b.1, b.2))));
    let two_s2 = (2.0 * kernel * kernel) as f32;
    let max2 = (max_dist * max_dist) as f32;
    let mut link = vec![NO; w * h];
    link.par_chunks_mut(w.max(1)).enumerate().for_each(|(r, row)| {
        for (c, out) in row.iter_mut().enumerate() {
            let i = r * w + c;
            if !j.valid[i] {
                continue;
            }
            let di = dens[i];
            let (mut best, mut at) = (max2, NO);
            for &(ds, dr, dc) in &offs {
                if ds > best {
                    break;
                }
                let (rr, cc) = (r as i64 + dr as i64, c as i64 + dc as i64);
                if rr < 0 || cc < 0 || rr >= h as i64 || cc >= w as i64 {
                    continue;
                }
                let q = rr as usize * w + cc as usize;
                if !j.valid[q] {
                    continue;
                }
                let dq = dens[q];
                if !(dq > di || (dq == di && q < i)) {
                    continue;
                }
                let mut d2 = 0.0f32;
                for plane in &j.planes {
                    let e = plane[q] - plane[i];
                    d2 += e * e;
                }
                let d = ds + two_s2 * d2;
                if d < best || (d == best && (q as u32) < at) {
                    (best, at) = (d, q as u32);
                }
            }
            *out = at;
        }
    });
    link
}

/// Quick shift: the segment (its root cell) of each cell, `NO_REGION` for a cell without a value.
fn quickshift(p: &Prep, ratio: f64, kernel: f64, max_dist: f64) -> Vec<u64> {
    let j = joint(p, ratio, kernel);
    let rad = (3.0 * kernel).ceil() as usize;
    let dens = densities(&j, kernel, rad);
    let link = links(&j, &dens, kernel, max_dist);
    let mut parent: Vec<u32> = link.iter().enumerate().map(|(i, &l)| if l == NO { i as u32 } else { l }).collect();
    (0..p.w * p.h).map(|i| if p.valid[i] { find(&mut parent, i as u32) as u64 } else { NO_REGION }).collect()
}

/// `segment.quickshift`.
struct Quickshift;

impl Operator for Quickshift {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.quickshift",
            "Objects by quick shift (Vedaldi & Soatto 2008, as scikit-image's quickshift): every cell is a point of \
             (column, row, ratio times the bands), its density a Parzen estimate with a Gaussian of `kernel_size` \
             (sigma) over the cells within 3 sigma, and it links to the nearest point (in that joint space) of a \
             higher density, if within `max_dist`; the trees of the links are the segments, so the number of objects \
             comes from the two distances and not from a count. The bands are standardised (z-score over the cells \
             with a value) by default, so a band of range 0-10000 weighs as one of 0-1 and one standard deviation \
             counts as `ratio` cells: scikit-image works on Lab colours of 0-100, so a `ratio` of 5 to 30 is the \
             like of its default 1 on a photograph. Differences from scikit-image: the window is a disc, not a \
             square; the link looks as far as `max_dist` (scikit-image's window of 3 sigma also limits it); cells of \
             an equal density rank by their number (the lower one is higher), so a flat area is one segment where \
             scikit-image leaves each cell its own; a segment whose cells the links leave apart (a link jumps up \
             to `max_dist` cells) is split into its connected pieces, as the objects are always connected \
             (4 neighbours). A cell without a value in every band is no object. The same result on every run, on any \
             number of threads. The cost is about (6 sigma)² exponentials per cell: a large `kernel_size` is slow.",
            Access::Global,
        )
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands, the colour"))
        .output(PortSpec::new("objects", Type::Objects, "the segments, each one connected"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per object: id, cells, area (map units squared), and mean_<band> and sd_<band> for each band",
        ))
        .param(PortSpec::optional(
            "ratio",
            Type::Number,
            "the weight of colour against position (default 1), 0 or more: higher gives more weight to the bands",
        ))
        .param(PortSpec::optional(
            "kernel_size",
            Type::Number,
            "sigma of the Gaussian that estimates the density, in cells (default 5), above 0: higher, fewer modes",
        ))
        .param(PortSpec::optional(
            "max_dist",
            Type::Number,
            "the longest link, in the joint space (default 10), above 0: higher, larger segments",
        ))
        .param(PortSpec::optional(
            "sigma",
            Type::Number,
            "a smoothing of the image first, a Gaussian in cells (default 0)",
        ))
        .param(PortSpec::optional(
            "standardise",
            Type::Bool,
            "standardise each band (z-score) first (default true)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let ratio: f64 = args.param("ratio", 1.0)?;
        if !ratio.is_finite() || ratio < 0.0 {
            return Err(bad(format!("ratio {ratio}: 0 or more")));
        }
        let kernel: f64 = args.param("kernel_size", 5.0)?;
        if !kernel.is_finite() || kernel <= 0.0 || kernel > 100.0 {
            return Err(bad(format!("kernel_size {kernel}: above 0, at most 100")));
        }
        let max_dist: f64 = args.param("max_dist", 10.0)?;
        if !max_dist.is_finite() || max_dist <= 0.0 || max_dist > 1000.0 {
            return Err(bad(format!("max_dist {max_dist}: above 0, at most 1000")));
        }
        let sigma: f64 = args.param("sigma", 0.0)?;
        if !sigma.is_finite() || sigma < 0.0 {
            return Err(bad(format!("sigma {sigma}: 0 or more")));
        }
        let l = crate::raster_stats::layers(args, "image")?;
        if l.grid.width * l.grid.height >= 1 << 31 {
            return Err(bad("an image of 2^31 cells or more is more than segment.quickshift works in memory".into()));
        }
        let standardise: bool = args.param("standardise", true)?;
        super::warnings::image(ctx, node, &l.grid, &l.names, &l.data, standardise);
        let mut p = prep(&l, standardise);
        if sigma > 0.0 {
            smooth(&mut p, sigma);
        }
        finish(&l, &quickshift(&p, ratio, kernel, max_dist))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_neg_matches_exp() {
        let mut worst = 0.0f64;
        for k in 0..20000 {
            let t = k as f32 * 0.0035;
            let (got, want) = (exp_neg(t) as f64, (-(t as f64)).exp());
            worst = worst.max(((got - want) / want).abs());
        }
        assert!(worst < 5e-6, "relative error {worst}");
        assert_eq!(exp_neg(0.0), 1.0);
        assert!(exp_neg(1e9) < 1e-30);
    }
}
