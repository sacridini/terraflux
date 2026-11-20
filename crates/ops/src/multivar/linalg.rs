//! The linear algebra of the multivariate operators: the singular value decomposition of a table (small), the
//! covariance and correlation of a stack's bands in one pass over its cells (big, summed in fixed blocks, the same
//! bits with any number of threads) and the eigenvalues of a symmetric matrix of a few dozen bands.

use rayon::prelude::*;

/// A matrix of rows.
pub type Matrix = Vec<Vec<f64>>;

/// The thin singular value decomposition of an n × m matrix by one-sided Jacobi rotations: U (n × k), the singular
/// values decreasing (k) and Vᵀ (k × m), k = min(n, m).
pub fn svd(a: &Matrix) -> (Matrix, Vec<f64>, Matrix) {
    let n = a.len();
    let m = a.first().map_or(0, Vec::len);
    if n < m {
        // A = (Aᵀ)ᵀ = (U' S V'ᵀ)ᵀ = V' S U'ᵀ
        let t: Matrix = (0..m).map(|j| (0..n).map(|i| a[i][j]).collect()).collect();
        let (u, s, vt) = svd(&t);
        let u2: Matrix = (0..n).map(|i| (0..s.len()).map(|c| vt[c][i]).collect()).collect();
        let vt2: Matrix = (0..s.len()).map(|c| (0..m).map(|j| u[j][c]).collect()).collect();
        return (u2, s, vt2);
    }
    // columns of A, rotated until orthogonal; V accumulates the rotations
    let mut cols: Matrix = (0..m).map(|j| (0..n).map(|i| a[i][j]).collect()).collect();
    let mut v: Matrix = (0..m).map(|j| (0..m).map(|i| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    let dot = |x: &[f64], y: &[f64]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f64>();
    for _sweep in 0..100 {
        let mut rotated = false;
        for p in 0..m {
            for q in p + 1..m {
                let alpha = dot(&cols[p], &cols[p]);
                let beta = dot(&cols[q], &cols[q]);
                let gamma = dot(&cols[p], &cols[q]);
                if gamma == 0.0 || gamma.abs() <= f64::EPSILON * (alpha * beta).sqrt() {
                    continue;
                }
                rotated = true;
                let zeta = (beta - alpha) / (2.0 * gamma);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for x in [&mut cols, &mut v] {
                    let (lo, hi) = x.split_at_mut(q);
                    for (a, b) in lo[p].iter_mut().zip(hi[0].iter_mut()) {
                        let (ap, bq) = (*a, *b);
                        *a = c * ap - s * bq;
                        *b = s * ap + c * bq;
                    }
                }
            }
        }
        if !rotated {
            break;
        }
    }
    let norms: Vec<f64> = cols.iter().map(|c| dot(c, c).sqrt()).collect();
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&i, &j| norms[j].total_cmp(&norms[i]));
    let s: Vec<f64> = order.iter().map(|&j| norms[j]).collect();
    let u: Matrix = (0..n)
        .map(|i| order.iter().map(|&j| if norms[j] > 0.0 { cols[j][i] / norms[j] } else { 0.0 }).collect())
        .collect();
    let vt: Matrix = order.iter().map(|&j| v[j].clone()).collect();
    (u, s, vt)
}

/// The sign that makes a vector's largest absolute value (the first of them) positive, as terraflux-py (and
/// scikit-learn) give components.
pub fn sign_of(v: &[f64]) -> f64 {
    let mut best = 0;
    for (i, x) in v.iter().enumerate() {
        if x.abs() > v[best].abs() {
            best = i;
        }
    }
    if v.get(best).copied().unwrap_or(0.0) < 0.0 { -1.0 } else { 1.0 }
}

/// Cells summed per block: the blocks on the threads, their moments merged in order (the same bits whatever the
/// threads).
const BLOCK: usize = 4096;

/// The means and the covariance of d variables over the cells with a value in every one.
pub struct Moments {
    /// The cells counted.
    pub n: usize,
    /// The mean of each variable (d).
    pub mean: Vec<f64>,
    /// The covariance (d × d, row-major), the sum of products about the mean over `n - ddof`.
    pub cov: Vec<f64>,
}

impl Moments {
    /// What standardising takes, as scikit-learn's StandardScaler does: each variable's standard deviation (1 for a
    /// constant one: centred, not divided), the correlation (a constant variable a zero row and column, it weighs
    /// nothing) and the constant variables. The same numbers as `sd` and `correlation` without a constant one.
    pub fn standardising(&self) -> (Vec<f64>, Vec<f64>, Vec<usize>) {
        let d = self.mean.len();
        let mut scale = self.sd();
        let constant: Vec<usize> = (0..d).filter(|&j| scale[j].is_nan() || scale[j] <= 0.0).collect();
        let mut r = self.correlation();
        for &j in &constant {
            scale[j] = 1.0;
            for i in 0..d {
                r[i * d + j] = 0.0;
                r[j * d + i] = 0.0;
            }
        }
        (scale, r, constant)
    }

    /// The standard deviation of each variable.
    pub fn sd(&self) -> Vec<f64> {
        let d = self.mean.len();
        (0..d).map(|i| self.cov[i * d + i].max(0.0).sqrt()).collect()
    }

    /// The correlation (d × d, row-major). A variable without variance is NaN against the others (and 1 against
    /// itself).
    pub fn correlation(&self) -> Vec<f64> {
        let d = self.mean.len();
        let sd = self.sd();
        let mut r = vec![0.0; d * d];
        for i in 0..d {
            for j in 0..d {
                r[i * d + j] = if i == j {
                    1.0
                } else if sd[i] > 0.0 && sd[j] > 0.0 {
                    (self.cov[i * d + j] / (sd[i] * sd[j])).clamp(-1.0, 1.0)
                } else {
                    f64::NAN
                };
            }
        }
        r
    }
}

/// One block's cell count, means and sums of products about them (the upper triangle of d × d).
struct Part {
    n: usize,
    mean: Vec<f64>,
    m2: Vec<f64>,
}

/// The moments of the cells `lo..hi` of the bands that have a value in every band.
fn part(bands: &[&[f64]], lo: usize, hi: usize) -> Part {
    let d = bands.len();
    let cells: Vec<usize> = (lo..hi).filter(|&i| bands.iter().all(|b| !b[i].is_nan())).collect();
    let n = cells.len();
    let mut mean = vec![0.0; d];
    let mut m2 = vec![0.0; d * d];
    if n == 0 {
        return Part { n, mean, m2 };
    }
    for (j, b) in bands.iter().enumerate() {
        mean[j] = cells.iter().map(|&i| b[i]).sum::<f64>() / n as f64;
    }
    // the centred values of the block, a band after the other
    let centred: Vec<Vec<f64>> =
        bands.iter().zip(&mean).map(|(b, m)| cells.iter().map(|&i| b[i] - m).collect()).collect();
    for i in 0..d {
        for j in i..d {
            m2[i * d + j] = centred[i].iter().zip(&centred[j]).map(|(a, b)| a * b).sum();
        }
    }
    Part { n, mean, m2 }
}

/// The means and covariance of the bands (one slice of cells each, all the same length) in one pass: blocks of
/// cells, each with its own means and sums of products, merged in order (Chan et al.'s update). A cell with NaN in
/// any band is left out. `ddof`: 0 for the population's, 1 for the sample's covariance. `None` without a cell, or
/// with fewer than `ddof + 1`.
pub fn moments(bands: &[&[f64]], ddof: usize) -> Option<Moments> {
    let d = bands.len();
    let len = bands.first().map_or(0, |b| b.len());
    assert!(bands.iter().all(|b| b.len() == len), "the bands must have the same number of cells");
    let parts: Vec<Part> =
        (0..len.div_ceil(BLOCK)).into_par_iter().map(|b| part(bands, b * BLOCK, ((b + 1) * BLOCK).min(len))).collect();
    let mut n = 0usize;
    let mut mean = vec![0.0; d];
    let mut m2 = vec![0.0; d * d];
    for p in parts.into_iter().filter(|p| p.n > 0) {
        if n == 0 {
            (n, mean, m2) = (p.n, p.mean, p.m2);
            continue;
        }
        let (na, nb) = (n as f64, p.n as f64);
        let t = na + nb;
        let delta: Vec<f64> = p.mean.iter().zip(&mean).map(|(b, a)| b - a).collect();
        for i in 0..d {
            for j in i..d {
                m2[i * d + j] += p.m2[i * d + j] + delta[i] * delta[j] * na * nb / t;
            }
        }
        for (m, dl) in mean.iter_mut().zip(&delta) {
            *m += dl * nb / t;
        }
        n += p.n;
    }
    if n <= ddof {
        return None;
    }
    let div = (n - ddof) as f64;
    let mut cov = vec![0.0; d * d];
    for i in 0..d {
        for j in i..d {
            cov[i * d + j] = m2[i * d + j] / div;
            cov[j * d + i] = cov[i * d + j];
        }
    }
    Some(Moments { n, mean, cov })
}

/// The eigenvalues and eigenvectors (columns, row-major n x n) of a symmetric matrix, by cyclic Jacobi rotations,
/// not sorted. Copied from Janus (crates/janus-app/src/atlas_math.rs, GPL-3.0, the same author).
pub fn jacobi(a: &mut [f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut v = vec![0.0; n * n];
    (0..n).for_each(|i| v[i * n + i] = 1.0);
    for _sweep in 0..100 {
        let off: f64 = (0..n).flat_map(|p| (p + 1..n).map(move |q| (p, q))).map(|(p, q)| a[p * n + q].powi(2)).sum();
        if off < 1e-22 {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p * n + q];
                if apq.abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let (akp, akq) = (a[k * n + p], a[k * n + q]);
                    a[k * n + p] = c * akp - s * akq;
                    a[k * n + q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let (apk, aqk) = (a[p * n + k], a[q * n + k]);
                    a[p * n + k] = c * apk - s * aqk;
                    a[q * n + k] = s * apk + c * aqk;
                }
                for k in 0..n {
                    let (vkp, vkq) = (v[k * n + p], v[k * n + q]);
                    v[k * n + p] = c * vkp - s * vkq;
                    v[k * n + q] = s * vkp + c * vkq;
                }
            }
        }
    }
    ((0..n).map(|i| a[i * n + i]).collect(), v)
}

/// The eigenvalues of a symmetric n × n matrix (row-major) from the largest, and their eigenvectors as rows (the
/// signs as Jacobi leaves them: see `sign_of`).
pub fn eigen(a: &[f64], n: usize) -> (Vec<f64>, Matrix) {
    let mut work = a.to_vec();
    let (vals, vecs) = jacobi(&mut work, n);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| vals[j].total_cmp(&vals[i]));
    let values = order.iter().map(|&k| vals[k]).collect();
    let rows = order.iter().map(|&k| (0..n).map(|i| vecs[i * n + k]).collect()).collect();
    (values, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reproducible stream of numbers in [0, 1) (SplitMix64).
    fn stream(seed: u64) -> impl FnMut() -> f64 {
        let mut s = seed;
        move || {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// Correlated bands of `n` cells with NaN here and there.
    fn bands(n: usize, d: usize) -> Vec<Vec<f64>> {
        let mut u = stream(5);
        let base: Vec<f64> = (0..n).map(|_| u() * 10.0).collect();
        (0..d)
            .map(|j| {
                (0..n)
                    .map(|i| {
                        let v = base[i] * (j as f64 + 1.0) * 0.5 + u() * 3.0 + 100.0 * j as f64;
                        if (i * 7 + j * 3) % 97 == 0 { f64::NAN } else { v }
                    })
                    .collect()
            })
            .collect()
    }

    /// The covariance by two passes over the complete cells.
    fn direct(b: &[Vec<f64>], ddof: usize) -> (usize, Vec<f64>, Vec<f64>) {
        let d = b.len();
        let rows: Vec<usize> = (0..b[0].len()).filter(|&i| b.iter().all(|c| !c[i].is_nan())).collect();
        let n = rows.len() as f64;
        let mean: Vec<f64> = b.iter().map(|c| rows.iter().map(|&i| c[i]).sum::<f64>() / n).collect();
        let mut cov = vec![0.0; d * d];
        for i in 0..d {
            for j in 0..d {
                cov[i * d + j] =
                    rows.iter().map(|&r| (b[i][r] - mean[i]) * (b[j][r] - mean[j])).sum::<f64>() / (n - ddof as f64);
            }
        }
        (rows.len(), mean, cov)
    }

    #[test]
    fn covariance_matches_two_passes() {
        // more cells than a block, and a last block that is not full
        let b = bands(3 * BLOCK + 123, 4);
        let slices: Vec<&[f64]> = b.iter().map(|v| v.as_slice()).collect();
        for ddof in [0, 1] {
            let m = moments(&slices, ddof).unwrap();
            let (n, mean, cov) = direct(&b, ddof);
            assert_eq!(m.n, n);
            for (a, e) in m.mean.iter().zip(&mean) {
                assert!((a - e).abs() <= 1e-12 * e.abs().max(1.0), "{a} {e}");
            }
            for (a, e) in m.cov.iter().zip(&cov) {
                assert!((a - e).abs() <= 1e-10 * e.abs().max(1.0), "{a} {e}");
            }
        }
    }

    #[test]
    fn correlation_matches_two_passes() {
        let b = bands(2 * BLOCK + 5, 3);
        let slices: Vec<&[f64]> = b.iter().map(|v| v.as_slice()).collect();
        let r = moments(&slices, 1).unwrap().correlation();
        let (_, _, cov) = direct(&b, 1);
        for i in 0..3 {
            for j in 0..3 {
                let e = cov[i * 3 + j] / (cov[i * 3 + i] * cov[j * 3 + j]).sqrt();
                assert!((r[i * 3 + j] - e).abs() < 1e-12, "{i} {j}: {} {e}", r[i * 3 + j]);
            }
            assert_eq!(r[i * 3 + i], 1.0);
        }
    }

    #[test]
    fn a_constant_band_has_no_correlation() {
        let b = [vec![1.0, 2.0, 3.0, 4.0], vec![5.0; 4]];
        let slices: Vec<&[f64]> = b.iter().map(|v| v.as_slice()).collect();
        let r = moments(&slices, 1).unwrap().correlation();
        assert!(r[1].is_nan() && r[2].is_nan() && r[0] == 1.0 && r[3] == 1.0);
    }

    #[test]
    fn moments_without_cells() {
        let b = [vec![f64::NAN, 1.0], vec![2.0, f64::NAN]];
        let slices: Vec<&[f64]> = b.iter().map(|v| v.as_slice()).collect();
        assert!(moments(&slices, 0).is_none());
    }

    #[test]
    fn moments_same_on_any_threads() {
        let b = bands(9 * BLOCK + 77, 5);
        let slices: Vec<&[f64]> = b.iter().map(|v| v.as_slice()).collect();
        let on = |threads: usize| {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
            pool.install(|| moments(&slices, 1).unwrap())
        };
        let (a, c) = (on(1), on(8));
        assert_eq!(a.n, c.n);
        assert!(a.mean.iter().zip(&c.mean).all(|(x, y)| x.to_bits() == y.to_bits()));
        assert!(a.cov.iter().zip(&c.cov).all(|(x, y)| x.to_bits() == y.to_bits()));
    }

    #[test]
    fn jacobi_known_eigenpairs() {
        // [[2, 1], [1, 2]]: 3 along (1, 1) and 1 along (1, -1)
        let (vals, vecs) = eigen(&[2.0, 1.0, 1.0, 2.0], 2);
        let h = 0.5f64.sqrt();
        assert!((vals[0] - 3.0).abs() < 1e-12 && (vals[1] - 1.0).abs() < 1e-12, "{vals:?}");
        assert!((vecs[0][0].abs() - h).abs() < 1e-12 && (vecs[0][0] - vecs[0][1]).abs() < 1e-12);
        assert!((vecs[1][0].abs() - h).abs() < 1e-12 && (vecs[1][0] + vecs[1][1]).abs() < 1e-12);
        // a diagonal matrix: its entries from the largest, the axes
        let (vals, vecs) = eigen(&[1.0, 0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 0.0, 3.0], 3);
        assert_eq!(vals, [5.0, 3.0, 1.0]);
        assert_eq!(vecs, [vec![0.0, 1.0, 0.0], vec![0.0, 0.0, 1.0], vec![1.0, 0.0, 0.0]]);
        // a tridiagonal matrix of 2 on the diagonal and -1 beside it: 2 - 2 cos(k pi / 4), k = 1..3
        let (vals, _) = eigen(&[2.0, -1.0, 0.0, -1.0, 2.0, -1.0, 0.0, -1.0, 2.0], 3);
        for (k, v) in [3, 2, 1].iter().zip(&vals) {
            let e = 2.0 - 2.0 * (f64::from(*k) * std::f64::consts::PI / 4.0).cos();
            assert!((v - e).abs() < 1e-12, "{v} {e}");
        }
    }

    #[test]
    fn jacobi_of_a_random_symmetric_matrix() {
        let n = 8;
        let mut u = stream(11);
        let mut a = vec![0.0; n * n];
        for i in 0..n {
            for j in i..n {
                a[i * n + j] = u() * 2.0 - 1.0;
                a[j * n + i] = a[i * n + j];
            }
        }
        let (vals, vecs) = eigen(&a, n);
        assert!(vals.windows(2).all(|w| w[0] >= w[1]));
        for (l, v) in vals.iter().zip(&vecs) {
            // A v = lambda v, and v of unit length
            for i in 0..n {
                let av: f64 = (0..n).map(|j| a[i * n + j] * v[j]).sum();
                assert!((av - l * v[i]).abs() < 1e-10, "{av} {}", l * v[i]);
            }
            assert!((v.iter().map(|x| x * x).sum::<f64>() - 1.0).abs() < 1e-12);
        }
        // the trace is the sum of the eigenvalues
        let trace: f64 = (0..n).map(|i| a[i * n + i]).sum();
        assert!((vals.iter().sum::<f64>() - trace).abs() < 1e-10);
    }

    #[test]
    fn svd_reconstructs_and_orders() {
        let a: Matrix = vec![vec![3.0, 1.0], vec![1.0, 3.0], vec![0.0, 2.0]];
        let (u, s, vt) = svd(&a);
        assert!(s[0] >= s[1]);
        for i in 0..3 {
            for j in 0..2 {
                let r: f64 = (0..2).map(|c| u[i][c] * s[c] * vt[c][j]).sum();
                assert!((r - a[i][j]).abs() < 1e-12);
            }
        }
        assert_eq!(sign_of(&[-3.0, 1.0]), -1.0);
        assert_eq!(sign_of(&[1.0, -1.0]), 1.0);
    }
}
