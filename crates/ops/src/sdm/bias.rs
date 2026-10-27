//! Sampling-bias surfaces (landschaft's bias_surface): the records of a whole taxonomic group as a Gaussian
//! kernel density on the grid of the predictors, MASS::kde2d's product of two normal kernels at the cells'
//! centres, with R's bw.nrd in each direction by default. The bias of `sdm.background` (Maxent's bias file).

use rayon::prelude::*;
use terraflux_raster::Grid;

use super::points::grid_of;
use crate::change::numpy_sum;

/// dnorm(u) underflows to 0 beyond |u| ~ 38.6: the cells beyond 40 bandwidths of a point add nothing.
const REACH: f64 = 40.0;
/// The points summed at once, sorted by y (landschaft's blocks).
const CHUNK: usize = 512;

/// How the kernel's standard deviations are chosen.
pub enum Bandwidth {
    /// R's bw.nrd in each direction.
    Nrd,
    /// In x and in y, in the units of the grid's coordinate system.
    Given(f64, f64),
}

pub struct Settings {
    pub bandwidth: Bandwidth,
    pub isotropic: bool,
    pub limits: (Option<f64>, Option<f64>),
    pub rescale: Option<(f64, f64)>,
    pub mask: bool,
}

/// numpy's std with ddof 1 (pairwise sums).
fn std1(v: &[f64]) -> f64 {
    let n = v.len() as f64;
    let m = numpy_sum(v) / n;
    (numpy_sum(&v.iter().map(|x| (x - m) * (x - m)).collect::<Vec<_>>()) / (n - 1.0)).sqrt()
}

/// numpy's quantile (the linear method, R's type 7) of sorted values, its _lerp.
fn quantile(s: &[f64], q: f64) -> f64 {
    let virt = (s.len() - 1) as f64 * q;
    let lo = virt.floor();
    let (a, b) = (s[lo as usize], s[((lo as usize) + 1).min(s.len() - 1)]);
    let t = virt - lo;
    let d = b - a;
    if t >= 0.5 { b - d * (1.0 - t) } else { a + d * t }
}

/// R's stats::bw.nrd: 1.06 min(sd, IQR / 1.34) n^(-1/5).
fn bw_nrd(v: &[f64]) -> f64 {
    let n = v.len();
    if n < 2 {
        return f64::NAN;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let h = (quantile(&s, 0.75) - quantile(&s, 0.25)) / 1.34;
    1.06 * std1(v).min(h) * (n as f64).powf(-0.2)
}

/// The bandwidths in x and y: the rule or the numbers, the larger of the two if isotropic, within the limits.
pub fn bandwidths(s: &Settings, xs: &[f64], ys: &[f64]) -> Result<(f64, f64), String> {
    let (mut bx, mut by) = match s.bandwidth {
        Bandwidth::Nrd => (bw_nrd(xs), bw_nrd(ys)),
        Bandwidth::Given(x, y) => (x, y),
    };
    if s.isotropic {
        let m = bx.max(by);
        (bx, by) = (m, m);
    }
    let (lo, hi) = (s.limits.0.unwrap_or(f64::NEG_INFINITY), s.limits.1.unwrap_or(f64::INFINITY));
    if s.limits != (None, None) {
        (bx, by) = (bx.max(lo).min(hi), by.max(lo).min(hi));
    }
    if !(bx.is_finite() && by.is_finite() && bx > 0.0 && by > 0.0) {
        return Err(format!(
            "bandwidths must be positive (got {bx}, {by}): with fewer than two distinct coordinates, give a \
             number or limits"
        ));
    }
    Ok((bx, by))
}

/// The indices of values in increasing order (numpy's stable argsort).
fn argsort(v: &[f64]) -> Vec<usize> {
    let mut o: Vec<usize> = (0..v.len()).collect();
    o.sort_by(|&a, &b| v[a].total_cmp(&v[b]));
    o
}

/// numpy's searchsorted(side="left") in sorted values.
fn searchsorted(s: &[f64], v: f64) -> usize {
    s.partition_point(|e| *e < v)
}

/// The span of indices (first, past the last) of some positions of a grid's axis: a range, the axis being
/// evenly spaced.
fn span(idx: &[usize]) -> (usize, usize) {
    idx.iter().fold((usize::MAX, 0), |(a, b), &i| (a.min(i), b.max(i + 1)))
}

/// sum_k w_k phi((gx - x_k) / bx) phi((gy - y_k) / by) / (sum w bx by), row-major (len(gy), len(gx)): blocks of
/// points sorted by y, each over the rows and columns its points reach, as a matrix product (the kernels in y
/// times the weights, by the kernels in x), in bands of rows at once.
pub fn density(gx: &[f64], gy: &[f64], xs: &[f64], ys: &[f64], w: &[f64], bx: f64, by: f64) -> Vec<f64> {
    let (nx, ny) = (gx.len(), gy.len());
    let mut z = vec![0.0; ny * nx];
    let order = argsort(ys);
    let xs: Vec<f64> = order.iter().map(|&k| xs[k]).collect();
    let ys: Vec<f64> = order.iter().map(|&k| ys[k]).collect();
    let w: Vec<f64> = order.iter().map(|&k| w[k]).collect();
    let (ix, iy) = (argsort(gx), argsort(gy));
    let sx: Vec<f64> = ix.iter().map(|&k| gx[k]).collect();
    let sy: Vec<f64> = iy.iter().map(|&k| gy[k]).collect();
    let c = 1.0 / (2.0 * std::f64::consts::PI).sqrt();
    let range = |v: &[f64]| v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
    let bands = 4 * rayon::current_num_threads();
    for s in (0..xs.len()).step_by(CHUNK) {
        let e = (s + CHUNK).min(xs.len());
        let (px, py, pw) = (&xs[s..e], &ys[s..e], &w[s..e]);
        let ((xlo, xhi), (ylo, yhi)) = (range(px), range(py));
        let c0 = searchsorted(&sx, xlo - REACH * bx);
        let c1 = (searchsorted(&sx, xhi + REACH * bx) + 1).min(nx);
        let r0 = searchsorted(&sy, ylo - REACH * by);
        let r1 = (searchsorted(&sy, yhi + REACH * by) + 1).min(ny);
        if c0 >= c1 || r0 >= r1 {
            continue;
        }
        let ((j0, j1), (i0, i1)) = (span(&ix[c0..c1]), span(&iy[r0..r1]));
        let (nc, np) = (j1 - j0, px.len());
        // the kernel in x, a row per point over the columns
        let mut ax = vec![0.0; np * nc];
        ax.par_chunks_mut(nc).zip(px.par_iter()).for_each(|(row, &p)| {
            for (o, g) in row.iter_mut().zip(&gx[j0..j1]) {
                let u = (g - p) / bx;
                *o = c * (-0.5 * u * u).exp();
            }
        });
        let rows = i1 - i0;
        let step = rows.div_ceil(bands).max(16);
        z[i0 * nx..i1 * nx].par_chunks_mut(step * nx).enumerate().for_each(|(b, zb)| {
            let n = zb.len() / nx;
            // the kernel in y times the weights, a row per grid row over the points
            let mut ay = vec![0.0; n * np];
            for (r, row) in ay.chunks_mut(np).enumerate() {
                let g = gy[i0 + b * step + r];
                for ((o, &p), &wp) in row.iter_mut().zip(py).zip(pw) {
                    let u = (g - p) / by;
                    *o = c * (-0.5 * u * u).exp() * wp;
                }
            }
            // zb[:, j0..j1] += ay @ ax; the n rows of nx cells hold columns j0..j1 (j1 <= nx)
            unsafe {
                matrixmultiply::dgemm(
                    n,
                    np,
                    nc,
                    1.0,
                    ay.as_ptr(),
                    np as isize,
                    1,
                    ax.as_ptr(),
                    nc as isize,
                    1,
                    1.0,
                    zb.as_mut_ptr().add(j0),
                    nx as isize,
                    1,
                );
            }
        });
    }
    let d = numpy_sum(&w) * bx * by;
    z.iter_mut().for_each(|v| *v /= d);
    z
}

/// The surface on the grid (NaN on the cells without a value in every layer, if masked) and the bandwidths used.
/// `xs`, `ys`: the records in the grid's coordinate system; `w`: their weights.
pub fn bias_surface(
    grid: &Grid,
    layers: &[&[f64]],
    xs: &[f64],
    ys: &[f64],
    w: Option<&[f64]>,
    s: &Settings,
) -> Result<(Vec<f64>, (f64, f64)), String> {
    let ones;
    let w = match w {
        Some(w) => w,
        None => {
            ones = vec![1.0; xs.len()];
            &ones
        }
    };
    if w.len() != xs.len() {
        return Err(format!("weights must have one value per record ({})", xs.len()));
    }
    if !w.iter().all(|v| v.is_finite() && *v >= 0.0) {
        return Err("weights must be finite and non-negative".into());
    }
    let ok: Vec<usize> = (0..xs.len()).filter(|&k| xs[k].is_finite() && ys[k].is_finite() && w[k] > 0.0).collect();
    if ok.is_empty() {
        return Err("no record with coordinates".into());
    }
    let (xs, ys, w): (Vec<f64>, Vec<f64>, Vec<f64>) =
        (ok.iter().map(|&k| xs[k]).collect(), ok.iter().map(|&k| ys[k]).collect(), ok.iter().map(|&k| w[k]).collect());
    let (bx, by) = bandwidths(s, &xs, &ys)?;
    let (xmin, _, _, ymax, xres, yres, ny, nx) = grid_of(grid);
    let gx: Vec<f64> = (0..nx).map(|k| xmin + (k as f64 + 0.5) * xres).collect();
    let gy: Vec<f64> = (0..ny).map(|k| ymax - (k as f64 + 0.5) * yres).collect();
    let mut z = density(&gx, &gy, &xs, &ys, &w, bx, by);
    if s.mask {
        for (k, v) in z.iter_mut().enumerate() {
            if layers.iter().any(|l| l[k].is_nan()) {
                *v = f64::NAN;
            }
        }
    }
    if let Some((lo, hi)) = s.rescale {
        if lo.is_nan() || hi.is_nan() || lo >= hi {
            return Err("rescale must be [lo, hi] with lo < hi".into());
        }
        let (zmin, zmax) = z
            .iter()
            .filter(|v| !v.is_nan())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
        if zmax <= zmin {
            return Err("the density is the same in every cell (records far from the grid, or a bandwidth too \
                        large): nothing to rescale"
                .into());
        }
        // lo and hi exactly at the extremes
        z.iter_mut().for_each(|v| *v = (lo + (*v - zmin) / (zmax - zmin) * (hi - lo)).clamp(lo, hi));
    }
    Ok((z, (bx, by)))
}
