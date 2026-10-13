//! How far new conditions lie from those a model was fitted on (landschaft's mess, mop and exdet): Elith's
//! MESS, the mop package's MOP (the range of each variable and the mean distance to the nearest reference
//! points) and Mesgaran's ExDet (univariate extrapolation NT1 and multivariate novelty NT2, with the most
//! influential covariates). MESS and the Euclidean MOP are landschaft's to the bit; what goes through numpy's
//! covariance and LAPACK's inverse (the Mahalanobis distances) to rounding.

use terraflux_ls_sys::ffi;

/// Columns of a table by name, a value per row.
#[derive(Clone, Debug)]
pub struct Frame {
    pub names: Vec<String>,
    pub columns: Vec<Vec<f64>>,
}

impl Frame {
    pub fn rows(&self) -> usize {
        self.columns.first().map_or(0, |c| c.len())
    }

    /// The columns named, in that order.
    pub fn select(&self, names: &[String]) -> Result<Frame, String> {
        let missing: Vec<&String> = names.iter().filter(|n| !self.names.contains(n)).collect();
        if !missing.is_empty() {
            return Err(format!("the reference has no predictor {missing:?}"));
        }
        Ok(Frame {
            names: names.to_vec(),
            columns: names
                .iter()
                .map(|n| self.columns[self.names.iter().position(|x| x == n).unwrap()].clone())
                .collect(),
        })
    }

    /// The rows without a missing value.
    pub fn dropna(&self) -> Frame {
        let keep: Vec<usize> = (0..self.rows()).filter(|&r| self.columns.iter().all(|c| !c[r].is_nan())).collect();
        Frame {
            names: self.names.clone(),
            columns: self.columns.iter().map(|c| keep.iter().map(|&r| c[r]).collect()).collect(),
        }
    }

    /// Row-major values.
    fn matrix(&self) -> Vec<f64> {
        (0..self.rows()).flat_map(|r| self.columns.iter().map(move |c| c[r])).collect()
    }
}

/// landschaft's _messi: the MESS of a variable at p against the reference values v.
fn messi(p: &[f64], v: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = v.iter().copied().filter(|x| !x.is_nan()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len() as f64;
    let (minv, maxv) = (v[0], v[v.len() - 1]);
    p.iter()
        .map(|&x| {
            if x.is_nan() {
                return f64::NAN;
            }
            let k = v.partition_point(|&y| y <= x) as f64;
            let f = 100.0 * k / n;
            if f == 0.0 {
                100.0 * (x - minv) / (maxv - minv)
            } else if f == 100.0 {
                100.0 * (maxv - x) / (maxv - minv)
            } else if f > 50.0 {
                200.0 - 2.0 * f
            } else {
                2.0 * f
            }
        })
        .collect()
}

/// Elith's MESS of every row of the target against the reference: per variable (`full`) and their minimum.
pub fn mess(reference: &Frame, target: &Frame, full: bool) -> Result<Frame, String> {
    let r = reference.select(&target.names)?.dropna();
    if r.rows() < 2 {
        return Err("the reference needs at least two complete rows".into());
    }
    let parts: Vec<Vec<f64>> = target.columns.iter().zip(&r.columns).map(|(p, v)| messi(p, v)).collect();
    let n = target.rows();
    let m: Vec<f64> = (0..n)
        .map(|k| {
            let mut best = f64::NAN;
            for p in &parts {
                let x = p[k];
                if !x.is_nan() && (best.is_nan() || x < best) {
                    best = x;
                }
            }
            if best.is_finite() { best } else { f64::NAN }
        })
        .collect();
    let mut out = Frame { names: Vec::new(), columns: Vec::new() };
    if full {
        for (name, p) in target.names.iter().zip(parts) {
            out.names.push(format!("{name}_mess"));
            out.columns.push(p);
        }
    }
    out.names.push("mess".into());
    out.columns.push(m);
    Ok(out)
}

/// numpy's cov of the rows of an n x p matrix (row-major): p x p.
fn cov(x: &[f64], n: usize, p: usize) -> Vec<f64> {
    let mean: Vec<f64> = (0..p).map(|j| (0..n).map(|i| x[i * p + j]).sum::<f64>() / n as f64).collect();
    let mut c = vec![0.0; p * p];
    for a in 0..p {
        for b in 0..p {
            let s: f64 = (0..n).map(|i| (x[i * p + a] - mean[a]) * (x[i * p + b] - mean[b])).sum();
            c[a * p + b] = s / (n as f64 - 1.0);
        }
    }
    c
}

/// The inverse of a p x p matrix (Gauss-Jordan with partial pivoting); an error when it is singular to working
/// precision (numpy's 1 / cond_1 below the machine epsilon).
fn inverse(a: &[f64], p: usize) -> Result<Vec<f64>, String> {
    let singular = || "the covariance of the predictors is singular: some are collinear (drop them first)".to_string();
    let mut m = a.to_vec();
    let mut inv = vec![0.0; p * p];
    (0..p).for_each(|k| inv[k * p + k] = 1.0);
    for col in 0..p {
        let piv = (col..p).max_by(|&x, &y| m[x * p + col].abs().partial_cmp(&m[y * p + col].abs()).unwrap()).unwrap();
        if m[piv * p + col] == 0.0 || !m[piv * p + col].is_finite() {
            return Err(singular());
        }
        for k in 0..p {
            m.swap(col * p + k, piv * p + k);
            inv.swap(col * p + k, piv * p + k);
        }
        let d = m[col * p + col];
        for k in 0..p {
            m[col * p + k] /= d;
            inv[col * p + k] /= d;
        }
        for r in 0..p {
            if r != col {
                let f = m[r * p + col];
                for k in 0..p {
                    m[r * p + k] -= f * m[col * p + k];
                    inv[r * p + k] -= f * inv[col * p + k];
                }
            }
        }
    }
    let norm1 = |x: &[f64]| (0..p).map(|j| (0..p).map(|i| x[i * p + j].abs()).sum::<f64>()).fold(0.0, f64::max);
    if 1.0 / (norm1(a) * norm1(&inv)) < f64::EPSILON {
        return Err(singular());
    }
    Ok(inv)
}

/// The lower Cholesky factor of a symmetric positive-definite p x p matrix.
fn cholesky(a: &[f64], p: usize) -> Result<Vec<f64>, String> {
    let mut l = vec![0.0; p * p];
    for i in 0..p {
        for j in 0..=i {
            let s: f64 = (0..j).map(|k| l[i * p + k] * l[j * p + k]).sum();
            if i == j {
                let d = a[i * p + i] - s;
                if d <= 0.0 {
                    return Err("the covariance of the predictors is not positive definite".into());
                }
                l[i * p + j] = d.sqrt();
            } else {
                l[i * p + j] = (a[i * p + j] - s) / l[j * p + j];
            }
        }
    }
    Ok(l)
}

/// The squared Mahalanobis distances of the rows of x (n x p) from a centre.
fn mahalanobis2(x: &[f64], n: usize, p: usize, centre: &[f64], cov: &[f64]) -> Result<Vec<f64>, String> {
    let inv = inverse(cov, p)?;
    Ok((0..n)
        .map(|i| {
            let d: Vec<f64> = (0..p).map(|j| x[i * p + j] - centre[j]).collect();
            (0..p).map(|a| (0..p).map(|b| d[a] * inv[a * p + b] * d[b]).sum::<f64>()).sum()
        })
        .collect())
}

/// The settings of mop.
pub struct Mop<'a> {
    /// basic, simple or detailed.
    pub kind: &'a str,
    /// euclidean, mahalanobis or none.
    pub distance: Option<&'a str>,
    /// in_range, out_range or all: the rows whose distance to take.
    pub r#where: &'a str,
    pub percentage: f64,
    pub scale: bool,
    pub center: bool,
    pub rescale: bool,
}

/// The mop package's mop: the mean distance of every row to its nearest reference rows (`percentage` of them),
/// and where the variables fall outside the reference's ranges (basic 1, simple their count, detailed which:
/// sums of 2^k below and above).
pub fn mop(reference: &Frame, target: &Frame, o: &Mop) -> Result<Frame, String> {
    if !["basic", "simple", "detailed"].contains(&o.kind) {
        return Err("type must be basic, simple or detailed".into());
    }
    if !matches!(o.distance, None | Some("euclidean") | Some("mahalanobis")) {
        return Err("distance must be euclidean, mahalanobis or none".into());
    }
    if !["in_range", "out_range", "all"].contains(&o.r#where) {
        return Err("where must be in_range, out_range or all".into());
    }
    if !(o.percentage > 0.0 && o.percentage <= 100.0) {
        return Err("percentage must be in (0, 100]".into());
    }
    let p = target.names.len();
    let r = reference.select(&target.names)?.dropna();
    let mut m = r.matrix();
    let nm = r.rows();
    let n = target.rows();
    let gall = target.matrix();
    let ok: Vec<bool> = (0..n).map(|i| (0..p).all(|j| !gall[i * p + j].is_nan())).collect();
    let mut g: Vec<f64> = (0..n).filter(|&i| ok[i]).flat_map(|i| gall[i * p..(i + 1) * p].to_vec()).collect();
    let ng = g.len() / p.max(1);
    if o.distance == Some("euclidean") && (o.scale || o.center) {
        let mut both: Vec<f64> = m.iter().chain(&g).copied().collect();
        let nb = nm + ng;
        if o.center {
            let means = ffi::sdm_r_colmeans(&both, nb as i64, p as i64).map_err(|e| e.to_string())?;
            for i in 0..nb {
                for j in 0..p {
                    both[i * p + j] -= means[j];
                }
            }
        }
        if o.scale {
            let sc: Vec<f64> = (0..p)
                .map(|j| {
                    let sq: Vec<f64> = (0..nb).map(|i| both[i * p + j] * both[i * p + j]).collect();
                    (ffi::r_sum(&sq) / (nb.saturating_sub(1).max(1)) as f64).sqrt()
                })
                .collect();
            for i in 0..nb {
                for j in 0..p {
                    both[i * p + j] /= sc[j];
                }
            }
        }
        m = both[..nm * p].to_vec();
        g = both[nm * p..].to_vec();
    }
    let lo: Vec<f64> = (0..p).map(|j| (0..nm).map(|i| m[i * p + j]).fold(f64::INFINITY, f64::min)).collect();
    let hi: Vec<f64> = (0..p).map(|j| (0..nm).map(|i| m[i * p + j]).fold(f64::NEG_INFINITY, f64::max)).collect();
    let below = |i: usize, j: usize| g[i * p + j] < lo[j];
    let above = |i: usize, j: usize| g[i * p + j] > hi[j];
    let simple: Vec<f64> = (0..ng).map(|i| (0..p).filter(|&j| below(i, j) || above(i, j)).count() as f64).collect();
    let basic: Vec<f64> = simple.iter().map(|&s| if s > 0.0 { 1.0 } else { f64::NAN }).collect();
    let full = |v: Vec<f64>| -> Vec<f64> {
        let mut out = vec![f64::NAN; n];
        let mut k = 0;
        for (i, o) in ok.iter().enumerate() {
            if *o {
                out[i] = v[k];
                k += 1;
            }
        }
        out
    };
    let mut out = Frame { names: Vec::new(), columns: Vec::new() };
    if let Some(distance) = o.distance {
        let rows: Vec<usize> = (0..ng)
            .filter(|&i| match o.r#where {
                "in_range" => basic[i].is_nan(),
                "out_range" => !basic[i].is_nan(),
                _ => true,
            })
            .collect();
        let per_out = ((nm as f64) * (o.percentage / 100.0)).round_ties_even() as i64;
        let mut d = vec![f64::NAN; ng];
        if !rows.is_empty() {
            let sub: Vec<f64> = rows.iter().flat_map(|&i| g[i * p..(i + 1) * p].to_vec()).collect();
            let v = if distance == "euclidean" {
                ffi::sdm_knn_mean_distance(&m, nm as i64, &sub, rows.len() as i64, p as i64, per_out.max(1), false)
            } else {
                let c = cov(&m, nm, p);
                let l = cholesky(&inverse(&c, p)?, p)?;
                let l = &l;
                // x @ L (the whitened rows: w = L^T, x @ w^T = x @ L)
                let white = |x: &[f64], rows: usize| -> Vec<f64> {
                    (0..rows)
                        .flat_map(|i| (0..p).map(move |j| (0..p).map(|k| x[i * p + k] * l[k * p + j]).sum::<f64>()))
                        .collect()
                };
                let (wm, wg) = (white(&m, nm), white(&sub, rows.len()));
                ffi::sdm_knn_mean_distance(&wm, nm as i64, &wg, rows.len() as i64, p as i64, per_out.max(1), true)
            }
            .map_err(|e| e.to_string())?;
            for (k, &i) in rows.iter().enumerate() {
                d[i] = v[k];
            }
            if o.rescale {
                let (lo, hi) =
                    rows.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &i| (a.min(d[i]), b.max(d[i])));
                for &i in &rows {
                    d[i] = (d[i] - lo) / (hi - lo);
                }
            }
        }
        out.names.push("mop".into());
        out.columns.push(full(d));
    }
    out.names.push("basic".into());
    out.columns.push(full(basic));
    if o.kind == "simple" || o.kind == "detailed" {
        out.names.push("simple".into());
        out.columns.push(full(simple.iter().map(|&s| if s > 0.0 { s } else { f64::NAN }).collect()));
    }
    if o.kind == "detailed" {
        let codes: Vec<f64> = (1..=p).map(|k| 2f64.powi(k as i32)).collect();
        let sum = |test: &dyn Fn(usize, usize) -> bool| -> Vec<f64> {
            (0..ng)
                .map(|i| {
                    let v: Vec<f64> = (0..p).map(|j| if test(i, j) { codes[j] } else { 0.0 }).collect();
                    let s = crate::change::numpy_sum(&v);
                    if s > 0.0 { s } else { f64::NAN }
                })
                .collect()
        };
        out.names.push("low_combined".into());
        out.columns.push(full(sum(&below)));
        out.names.push("high_combined".into());
        out.columns.push(full(sum(&above)));
    }
    Ok(out)
}

/// Mesgaran's ExDet: NT1 (univariate extrapolation, negative outside the reference's ranges) or, inside them,
/// NT2 (the Mahalanobis distance against the reference's largest; above 1 novel combinations), and the most
/// influential covariate (mic).
pub fn exdet(reference: &Frame, target: &Frame) -> Result<Frame, String> {
    let p = target.names.len();
    let r = reference.select(&target.names)?.dropna();
    let nr = r.rows();
    let refm = r.matrix();
    let n = target.rows();
    let all = target.matrix();
    let ok: Vec<bool> = (0..n).map(|i| (0..p).all(|j| !all[i * p + j].is_nan())).collect();
    let tg: Vec<f64> = (0..n).filter(|&i| ok[i]).flat_map(|i| all[i * p..(i + 1) * p].to_vec()).collect();
    let nt = tg.len() / p.max(1);
    let a: Vec<f64> = (0..p).map(|j| (0..nr).map(|i| refm[i * p + j]).fold(f64::INFINITY, f64::min)).collect();
    let b: Vec<f64> = (0..p).map(|j| (0..nr).map(|i| refm[i * p + j]).fold(f64::NEG_INFINITY, f64::max)).collect();
    let np_min = |x: f64, y: f64| if x.is_nan() || y.is_nan() { f64::NAN } else { x.min(y) };
    let parts: Vec<Vec<f64>> = (0..nt)
        .map(|i| {
            (0..p).map(|j| np_min(np_min(tg[i * p + j] - a[j], b[j] - tg[i * p + j]), 0.0) / (b[j] - a[j])).collect()
        })
        .collect();
    let nt1: Vec<f64> = parts.iter().map(|r| ffi::r_sum(r)).collect();
    let argmin = |r: &[f64]| -> usize {
        if let Some(k) = r.iter().position(|x| x.is_nan()) {
            return k;
        }
        let mut best = 0;
        for k in 1..r.len() {
            if r[k] < r[best] {
                best = k;
            }
        }
        best
    };
    let mic_uni: Vec<f64> =
        (0..nt).map(|i| if nt1[i] != 0.0 { argmin(&parts[i]) as f64 + 1.0 } else { f64::NAN }).collect();
    let u: Vec<usize> = (0..nt).filter(|&i| nt1[i] == 0.0).collect();
    let centre: Vec<f64> =
        (0..p).map(|j| ffi::cost_r_mean(&(0..nr).map(|i| refm[i * p + j]).collect::<Vec<_>>())).collect();
    let c = cov(&refm, nr, p);
    let d2_ref = mahalanobis2(&refm, nr, p, &centre, &c)?;
    let d2max = d2_ref.iter().copied().filter(|x| x.is_finite()).fold(f64::NEG_INFINITY, f64::max);
    let mut ex = nt1.clone();
    let mut mic_comb = vec![f64::NAN; nt];
    if !u.is_empty() {
        let tu: Vec<f64> = u.iter().flat_map(|&i| tg[i * p..(i + 1) * p].to_vec()).collect();
        let d2 = mahalanobis2(&tu, u.len(), p, &centre, &c)?;
        for (k, &i) in u.iter().enumerate() {
            ex[i] = d2[k] / d2max;
        }
        if p > 1 && u.len() >= 2 {
            let mut pct = vec![0.0; u.len() * p];
            for k in 0..p {
                let keep: Vec<usize> = (0..p).filter(|&j| j != k).collect();
                let q = keep.len();
                let sub = |x: &[f64], rows: usize| -> Vec<f64> {
                    (0..rows).flat_map(|i| keep.iter().map(move |&j| x[i * p + j])).collect()
                };
                let refk = sub(&refm, nr);
                let ck = cov(&refk, nr, q);
                let centrek: Vec<f64> = keep.iter().map(|&j| centre[j]).collect();
                let d2k = mahalanobis2(&sub(&tu, u.len()), u.len(), q, &centrek, &ck)?;
                for i in 0..u.len() {
                    pct[i * p + k] = 100.0 * (d2[i] - d2k[i]) / d2k[i];
                }
            }
            for (k, &i) in u.iter().enumerate() {
                if ex[i] > 1.0 {
                    let row = &pct[k * p..(k + 1) * p];
                    let mut best = 0;
                    for j in 1..p {
                        if row[j] > row[best] || (row[j].is_nan() && !row[best].is_nan()) {
                            best = j;
                        }
                    }
                    mic_comb[i] = best as f64 + 1.0;
                }
            }
        }
    }
    let z = |x: f64| if x.is_nan() { 0.0 } else { x };
    let mic: Vec<f64> = (0..nt).map(|i| z(mic_uni[i]) + z(mic_comb[i])).collect();
    let full = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![f64::NAN; n];
        let mut k = 0;
        for (i, o) in ok.iter().enumerate() {
            if *o {
                out[i] = v[k];
                k += 1;
            }
        }
        out
    };
    Ok(Frame { names: vec!["exdet".into(), "mic".into()], columns: vec![full(&ex), full(&mic)] })
}
