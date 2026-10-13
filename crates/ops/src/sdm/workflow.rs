//! The modelling workflow of landschaft's sdm with MaxNet: the records' predictor values (dataset), the model
//! fitted with cross-validation over folds (fit: AUC, TSS, Boyce and omission per fold, their means and
//! standard deviations, the threshold of maximum sensitivity plus specificity), response curves and
//! permutation importance (numpy's generator).

use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;

use super::evaluate::{Classes, boyce, evaluate, omission_rate};
use super::extrapolation::Frame;
use super::maxnet::{self, MaxNet};
use crate::change::numpy_sum;
use crate::signatures::NumpyRng;

/// The predictors' values at points (the cell each falls in): a column per layer, NaN outside.
pub fn values_at(grid: &Grid, layers: &[&[f64]], x: &[f64], y: &[f64]) -> Result<Vec<Vec<f64>>, String> {
    let (ny, nx) = (grid.height, grid.width);
    let values: Vec<f64> = layers.iter().flat_map(|l| l.iter().copied()).collect();
    let out =
        ffi::io_extract(&values, layers.len() as i32, ny as i32, nx as i32, &grid.transform, x, y, false, 0, ny as i32)
            .map_err(|e| e.to_string())?;
    let nb = layers.len();
    Ok((0..nb).map(|b| (0..x.len()).map(|i| out[i * nb + b]).collect()).collect())
}

/// One fold's metrics (landschaft's _fold_metrics): auc_train, auc_test, auc_diff, tss, boyce, or_mtp, or_10p,
/// n_test.
pub const METRICS: [&str; 8] = ["auc_train", "auc_test", "auc_diff", "tss", "boyce", "or_mtp", "or_10p", "n_test"];

fn fold_metrics(pred: &[f64], y: &[f64], train: &[usize], test: &[usize]) -> Result<Vec<f64>, String> {
    let pick =
        |idx: &[usize], class: f64| -> Vec<f64> { idx.iter().filter(|&&i| y[i] == class).map(|&i| pred[i]).collect() };
    let (tr_p, te_p, tr_b, te_b) = (pick(train, 1.0), pick(test, 1.0), pick(train, 0.0), pick(test, 0.0));
    let bg_all: Vec<f64> = (0..y.len()).filter(|&i| y[i] == 0.0).map(|i| pred[i]).collect();
    let ref_bg = if te_b.is_empty() { bg_all } else { te_b };
    let mut out = vec![f64::NAN; 8];
    out[7] = te_p.len() as f64;
    if !tr_p.is_empty() && !tr_b.is_empty() {
        out[0] = evaluate(&tr_p, &tr_b, None)?.stats[0];
    }
    if !te_p.is_empty() && !ref_bg.is_empty() {
        let e = evaluate(&te_p, &ref_bg, None)?;
        out[1] = e.stats[0];
        out[3] = e.stats[6];
        out[4] = if te_p.len() > 1 {
            let avail: Vec<f64> = ref_bg.iter().chain(&te_p).copied().collect();
            boyce(&te_p, &avail, &Classes::Window(None, 100), "spearman", true)?.cor
        } else {
            f64::NAN
        };
        out[2] = (out[0] - out[1]).abs();
        out[5] = omission_rate(&tr_p, &te_p, "mtp")?;
        out[6] = omission_rate(&tr_p, &te_p, "p10")?;
    }
    Ok(out)
}

/// The rows of a frame.
fn rows(f: &Frame, idx: &[usize]) -> Frame {
    Frame { names: f.names.clone(), columns: f.columns.iter().map(|c| idx.iter().map(|&i| c[i]).collect()).collect() }
}

/// A MaxNet's suitability (its predict_proba's presence column: its output, cloglog by default).
fn suitability(m: &MaxNet, x: &Frame) -> Result<Vec<f64>, String> {
    m.predict(x, None, None)
}

/// pandas' groupby mean (Kahan's summation) and std (Welford's, ddof 1) of a column, NaN left out.
fn mean_sd(v: &[f64]) -> (f64, f64) {
    let (mut sum, mut comp, mut count) = (0.0, 0.0, 0.0);
    let (mut mean, mut m2, mut nobs) = (0.0, 0.0, 0.0);
    for &x in v.iter().filter(|x| !x.is_nan()) {
        count += 1.0;
        let y = x - comp;
        let t = sum + y;
        comp = t - sum - y;
        sum = t;
        nobs += 1.0;
        let old = mean;
        mean += (x - old) / nobs;
        m2 += (x - mean) * (x - old);
    }
    let sd = if nobs - 1.0 <= 0.0 { f64::NAN } else { (m2 / (nobs - 1.0)).sqrt() };
    (sum / count, sd)
}

/// A fitted model with its cross-validation: the model, a row of METRICS per fold (with the fold first), their
/// means and standard deviations, and the threshold of maximum sensitivity plus specificity.
pub struct Fitted {
    pub model: MaxNet,
    pub metrics: Vec<Vec<f64>>,
    pub summary: Vec<f64>,
    pub threshold: f64,
}

/// landschaft's fit with MaxNet: the model on every record, and with folds (0: training only) one per fold.
pub fn fit(x: &Frame, y: &[f64], folds: Option<&[f64]>, s: &maxnet::Settings, cv: bool) -> Result<Fitted, String> {
    let mut metrics = Vec::new();
    if let (true, Some(f)) = (cv, folds) {
        let mut ks: Vec<f64> = f.iter().copied().filter(|k| *k > 0.0).collect();
        ks.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ks.dedup();
        for (n, k) in ks.iter().enumerate() {
            let train: Vec<usize> = (0..y.len()).filter(|&i| f[i] != *k).collect();
            let test: Vec<usize> = (0..y.len()).filter(|&i| f[i] == *k).collect();
            let ytr: Vec<f64> = train.iter().map(|&i| y[i]).collect();
            let m = maxnet::fit(&rows(x, &train), &ytr, s)?;
            let mut pred = vec![f64::NAN; y.len()];
            for (i, v) in train.iter().zip(suitability(&m, &rows(x, &train))?) {
                pred[*i] = v;
            }
            for (i, v) in test.iter().zip(suitability(&m, &rows(x, &test))?) {
                pred[*i] = v;
            }
            let mut row = vec![(n + 1) as f64];
            row.extend(fold_metrics(&pred, y, &train, &test)?);
            metrics.push(row);
        }
    }
    let model = maxnet::fit(x, y, s)?;
    let pred = suitability(&model, x)?;
    let (p, a): (Vec<f64>, Vec<f64>) = (
        (0..y.len()).filter(|&i| y[i] == 1.0).map(|i| pred[i]).collect(),
        (0..y.len()).filter(|&i| y[i] == 0.0).map(|i| pred[i]).collect(),
    );
    let threshold = evaluate(&p, &a, None)?.thresholds[1];
    let mut summary = Vec::new();
    if !metrics.is_empty() {
        for c in 1..8 {
            let col: Vec<f64> = metrics.iter().map(|r| r[c]).collect();
            let (m, sd) = mean_sd(&col);
            summary.push(m);
            summary.push(sd);
        }
    }
    Ok(Fitted { model, metrics, summary, threshold })
}

/// numpy's linspace(start, stop, n).
fn linspace(start: f64, stop: f64, n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![start];
    }
    let div = (n - 1) as f64;
    let delta = stop - start;
    let step = delta / div;
    let mut v: Vec<f64> =
        (0..n).map(|k| if step == 0.0 { (k as f64 / div) * delta + start } else { k as f64 * step + start }).collect();
    v[n - 1] = stop;
    v
}

/// Response curves: each predictor over its range in n steps, the others at their mean (or median); a value
/// and a suitability column per predictor. `x`: the records' predictors, in their order.
pub fn response_curves(m: &MaxNet, x: &Frame, predictors: &[String], n: usize, fixed: &str) -> Result<Frame, String> {
    let xs = x;
    let base: Vec<f64> = xs
        .columns
        .iter()
        .map(|c| {
            if fixed == "mean" {
                numpy_sum(c) / c.len() as f64
            } else {
                let mut s = c.clone();
                s.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let k = s.len();
                if k % 2 == 1 { s[k / 2] } else { (s[k / 2 - 1] + s[k / 2]) / 2.0 }
            }
        })
        .collect();
    let mut out = Frame { names: Vec::new(), columns: Vec::new() };
    for p in predictors {
        let j = xs.names.iter().position(|q| q == p).ok_or_else(|| format!("the model has no predictor {p:?}"))?;
        let col = &xs.columns[j];
        let lo = col.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = col.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let grid = linspace(lo, hi, n);
        let g = Frame {
            names: xs.names.clone(),
            columns: (0..xs.names.len()).map(|c| if c == j { grid.clone() } else { vec![base[c]; n] }).collect(),
        };
        let s = suitability(m, &g)?;
        out.names.push(p.clone());
        out.columns.push(grid);
        out.names.push(format!("{p}_suitability"));
        out.columns.push(s);
    }
    Ok(out)
}

/// Permutation importance: the mean drop of AUC (and its standard deviation) when each predictor is shuffled
/// (numpy's generator), the most important first.
pub fn importance(
    m: &MaxNet,
    x: &Frame,
    y: &[f64],
    n_repeats: usize,
    seed: u64,
) -> Result<Vec<(String, f64, f64)>, String> {
    let xs = x;
    let mut rng = NumpyRng::new(seed);
    let auc = |p: &[f64]| -> Result<f64, String> {
        let (a, b): (Vec<f64>, Vec<f64>) = (
            (0..y.len()).filter(|&i| y[i] == 1.0).map(|i| p[i]).collect(),
            (0..y.len()).filter(|&i| y[i] == 0.0).map(|i| p[i]).collect(),
        );
        Ok(evaluate(&a, &b, None)?.stats[0])
    };
    let auc0 = auc(&suitability(m, xs)?)?;
    let mut rows = Vec::new();
    for (j, name) in xs.names.iter().enumerate() {
        let mut drops = Vec::with_capacity(n_repeats);
        for _ in 0..n_repeats {
            let mut xp = xs.clone();
            rng.shuffle(&mut xp.columns[j]);
            drops.push(auc0 - auc(&suitability(m, &xp)?)?);
        }
        let mean = numpy_sum(&drops) / drops.len() as f64;
        let sd = if n_repeats > 1 {
            let dev: Vec<f64> = drops.iter().map(|d| (d - mean) * (d - mean)).collect();
            (numpy_sum(&dev) / (drops.len() - 1) as f64).sqrt()
        } else {
            0.0
        };
        rows.push((name.clone(), mean, sd));
    }
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| rows[b].1.partial_cmp(&rows[a].1).unwrap());
    Ok(order.into_iter().map(|k| rows[k].clone()).collect())
}
