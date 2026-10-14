//! A GLM of presence and background: scikit-learn's StandardScaler and LogisticRegression (an L2 penalty of
//! 1 / (2 C) on the coefficients, not on the intercept), as landschaft's examples fit it. The optimum is found by
//! Newton's method with the exact Hessian and a line search, to a tolerance far below lbfgs's: the coefficients
//! scikit-learn converges to, within its own tolerance.

use serde::{Deserialize, Serialize};

use super::extrapolation::Frame;

/// The settings of a GLM: C (the inverse of the penalty), the standardization of the predictors.
#[derive(Debug, Clone)]
pub struct Settings {
    pub c: f64,
    pub standardize: bool,
    pub max_iter: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { c: 1.0, standardize: true, max_iter: 100 }
    }
}

/// A fitted GLM: the predictors' centres and scales, the coefficients and the intercept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logistic {
    pub predictors: Vec<String>,
    pub mean: Vec<f64>,
    pub scale: Vec<f64>,
    pub coef: Vec<f64>,
    pub intercept: f64,
}

fn expit(z: f64) -> f64 {
    if z >= 0.0 {
        1.0 / (1.0 + (-z).exp())
    } else {
        let e = z.exp();
        e / (1.0 + e)
    }
}

/// log(1 + exp(z)), without overflow.
fn log1pexp(z: f64) -> f64 {
    if z > 0.0 { z + (-z).exp().ln_1p() } else { z.exp().ln_1p() }
}

/// Solves the symmetric positive definite system a x = b (Cholesky).
fn solve(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let s: f64 = (0..j).map(|k| l[i][k] * l[j][k]).sum();
            if i == j {
                let d = a[i][i] - s;
                if d <= 0.0 {
                    return None;
                }
                l[i][j] = d.sqrt();
            } else {
                l[i][j] = (a[i][j] - s) / l[j][j];
            }
        }
    }
    let mut y = vec![0.0; n];
    for i in 0..n {
        y[i] = (b[i] - (0..i).map(|k| l[i][k] * y[k]).sum::<f64>()) / l[i][i];
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        x[i] = (y[i] - (i + 1..n).map(|k| l[k][i] * x[k]).sum::<f64>()) / l[i][i];
    }
    Some(x)
}

/// LogisticRegression.fit (after StandardScaler, which is not weighted) on the records (presence 1, background
/// 0), each record's loss weighted.
pub fn fit(x: &Frame, y: &[f64], weights: Option<&[f64]>, s: &Settings) -> Result<Logistic, String> {
    let n = y.len();
    let ones = vec![1.0; n];
    let sw = weights.unwrap_or(&ones);
    if sw.len() != n || sw.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err("the weights: one per record, 0 or more".into());
    }
    let p = x.columns.len();
    if n == 0 {
        return Err("no records".into());
    }
    if !(s.c > 0.0 && s.c.is_finite()) {
        return Err("C must be positive".into());
    }
    if y.iter().any(|v| *v != 0.0 && *v != 1.0) || !y.contains(&0.0) || !y.contains(&1.0) {
        return Err("a GLM needs presences (1) and background or absences (0)".into());
    }
    if x.columns.iter().flatten().any(|v| !v.is_finite()) {
        return Err("the records' predictors have missing values".into());
    }
    let (mut mean, mut scale) = (vec![0.0; p], vec![1.0; p]);
    if s.standardize {
        for j in 0..p {
            let c = &x.columns[j];
            let m = c.iter().sum::<f64>() / n as f64;
            let var = c.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / n as f64;
            mean[j] = m;
            // StandardScaler: no scaling of a constant predictor
            scale[j] = if var <= 10.0 * f64::EPSILON * m.abs().max(1.0).powi(2) { 1.0 } else { var.sqrt() };
        }
    }
    let z: Vec<Vec<f64>> = (0..p).map(|j| x.columns[j].iter().map(|v| (v - mean[j]) / scale[j]).collect()).collect();
    let lambda = 1.0 / s.c;
    // the parameters: the coefficients, then the intercept
    let mut beta = vec![0.0; p + 1];
    let eta = |b: &[f64]| -> Vec<f64> { (0..n).map(|i| b[p] + (0..p).map(|j| z[j][i] * b[j]).sum::<f64>()).collect() };
    let objective = |b: &[f64]| -> f64 {
        let e = eta(b);
        let loss: f64 = (0..n).map(|i| sw[i] * (log1pexp(e[i]) - y[i] * e[i])).sum();
        loss + 0.5 * lambda * b[..p].iter().map(|v| v * v).sum::<f64>()
    };
    let iterations = s.max_iter.max(50);
    let mut f = objective(&beta);
    for _ in 0..iterations {
        let e = eta(&beta);
        let prob: Vec<f64> = e.iter().map(|v| expit(*v)).collect();
        let mut g = vec![0.0; p + 1];
        let mut h = vec![vec![0.0; p + 1]; p + 1];
        for i in 0..n {
            let r = sw[i] * (prob[i] - y[i]);
            let wgt = sw[i] * prob[i] * (1.0 - prob[i]);
            let row: Vec<f64> = (0..p).map(|j| z[j][i]).chain(std::iter::once(1.0)).collect();
            for a in 0..=p {
                g[a] += r * row[a];
                for b in 0..=a {
                    h[a][b] += wgt * row[a] * row[b];
                }
            }
        }
        for a in 0..p {
            g[a] += lambda * beta[a];
            h[a][a] += lambda;
        }
        // the upper triangle from the lower
        for a in 1..=p {
            let (upper, lower) = h.split_at_mut(a);
            for (b, row) in upper.iter_mut().enumerate() {
                row[a] = lower[0][b];
            }
        }
        if g.iter().map(|v| v.abs()).fold(0.0, f64::max) <= 1e-12 * n as f64 {
            break;
        }
        // a ridge on the intercept's row when the background and presences separate
        let step = solve(&h, &g).or_else(|| {
            let mut h2 = h.clone();
            h2[p][p] += 1e-12;
            solve(&h2, &g)
        });
        let Some(step) = step else { return Err("the GLM's Hessian is singular".into()) };
        let mut t = 1.0;
        let mut moved = false;
        for _ in 0..60 {
            let cand: Vec<f64> = beta.iter().zip(&step).map(|(b, d)| b - t * d).collect();
            let fc = objective(&cand);
            if fc <= f {
                moved = fc < f || cand != beta;
                beta = cand;
                f = fc;
                break;
            }
            t /= 2.0;
        }
        if !moved {
            break;
        }
    }
    Ok(Logistic { predictors: x.names.clone(), mean, scale, coef: beta[..p].to_vec(), intercept: beta[p] })
}

impl Logistic {
    /// predict_proba's presence column: expit of the linear predictor.
    pub fn predict(&self, x: &Frame) -> Result<Vec<f64>, String> {
        let cols: Vec<&Vec<f64>> = self
            .predictors
            .iter()
            .map(|p| {
                x.names.iter().position(|q| q == p).map(|j| &x.columns[j]).ok_or_else(|| format!("no predictor {p:?}"))
            })
            .collect::<Result<_, String>>()?;
        let n = cols.first().map_or(0, |c| c.len());
        Ok((0..n)
            .map(|i| {
                let d = self.intercept
                    + (0..cols.len()).map(|j| (cols[j][i] - self.mean[j]) / self.scale[j] * self.coef[j]).sum::<f64>();
                expit(d)
            })
            .collect())
    }
}
