//! What a fitted model learnt, as scikit-learn's inspection tells it: the drop of its score when a predictor's values
//! are shuffled (`ml.importance`, permutation_importance) and its mean prediction along a grid of a predictor's
//! values (`ml.partial_dependence`, partial_dependence by its brute method).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::Model;
use super::forest::Mt19937;
use crate::change::numpy_sum;
use crate::sdm::extrapolation::Frame;
use crate::value::class_label;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Importance));
    r.add(Arc::new(PartialDependence));
}

fn fail(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

fn model_of<'a>(args: &'a Args) -> Result<&'a Arc<Model>> {
    match args.get("model") {
        Some(Value::MlModel(m)) => Ok(m),
        _ => Err(fail(args, "connect a fitted model (ml.fit)")),
    }
}

/// The predictors of the model from the rows of a table that have them all (and the target, when named).
fn records(args: &Args, model: &Model, target: Option<&str>) -> Result<(Frame, Vec<f64>)> {
    let Some(Value::Table(t)) = args.get("table") else { return Err(fail(args, "connect the records (a table)")) };
    let names = model.predictors().to_vec();
    let mut columns = Vec::with_capacity(names.len());
    for n in &names {
        columns.push(t.column(n).ok_or_else(|| fail(args, format!("the table has no column {n:?}")))?);
    }
    let y = match target {
        Some(c) => t.column(c).ok_or_else(|| fail(args, format!("the table has no column {c:?} (setting column)")))?,
        None => vec![0.0; t.rows.len()],
    };
    let keep: Vec<usize> =
        (0..t.rows.len()).filter(|&r| !y[r].is_nan() && columns.iter().all(|c| !c[r].is_nan())).collect();
    if keep.is_empty() {
        return Err(fail(args, "no record has every predictor (and the target)"));
    }
    let frame = Frame { names, columns: columns.iter().map(|c| keep.iter().map(|&r| c[r]).collect()).collect() };
    Ok((frame, keep.iter().map(|&r| y[r]).collect()))
}

/// The prediction of every row: the value, or the class of the largest probability (the first on ties).
fn predictions(model: &Model, x: &Frame) -> std::result::Result<Vec<f64>, String> {
    let cols = model.outputs(x)?;
    let classes = model.classes();
    let n = cols[0].len();
    Ok((0..n)
        .map(|i| {
            if classes.is_empty() {
                return cols[0][i];
            }
            let mut k = 0;
            for c in 1..cols.len() {
                if cols[c][i] > cols[k][i] {
                    k = c;
                }
            }
            classes[k]
        })
        .collect())
}

/// The estimator's score: the accuracy of a classifier, the coefficient of determination (R²) of a regressor.
fn score(model: &Model, x: &Frame, y: &[f64]) -> std::result::Result<f64, String> {
    let p = predictions(model, x)?;
    let n = y.len() as f64;
    if !model.classes().is_empty() {
        return Ok(y.iter().zip(&p).filter(|(a, b)| a == b).count() as f64 / n);
    }
    if y.len() < 2 {
        return Ok(f64::NAN);
    }
    let num = numpy_sum(&y.iter().zip(&p).map(|(a, b)| (a - b) * (a - b)).collect::<Vec<_>>());
    let mean = numpy_sum(y) / n;
    let den = numpy_sum(&y.iter().map(|a| (a - mean) * (a - mean)).collect::<Vec<_>>());
    Ok(match (num != 0.0, den != 0.0) {
        (true, true) => 1.0 - num / den,
        (true, false) => 0.0,
        _ => 1.0,
    })
}

struct Importance;

impl Operator for Importance {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "ml.importance",
            "The permutation importance of each predictor of a fitted model (scikit-learn's \
             permutation_importance, the same numbers): how much its score (a classifier's accuracy, a \
             regressor's R²) drops when the predictor's values are shuffled among the records, shuffle after \
             shuffle (numpy's RandomState); best on records the model did not learn from.",
            Access::None,
        )
        .input(PortSpec::new("model", Type::MlModel, "the fitted model (ml.fit)"))
        .input(PortSpec::new("table", Type::Table, "the records: the predictors and the target, a column each"))
        .output(PortSpec::new("importance", Type::Table, "the mean drop of the score, a column per predictor"))
        .output(PortSpec::new("sd", Type::Table, "its standard deviation over the shuffles"))
        .output(PortSpec::new("repeats", Type::Table, "the drop at every shuffle: repeat, then a column per predictor"))
        .output(PortSpec::new("score", Type::Number, "the score on the records as they are"))
        .param(PortSpec::optional("column", Type::Text, "the target's column (default target)"))
        .param(PortSpec::optional("n_repeats", Type::Integer, "the shuffles of each predictor (default 5)"))
        .param(PortSpec::optional("random_state", Type::Integer, "the seed of the shuffles (default 0)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let model = model_of(args)?;
        if matches!(**model, Model::PerClass { .. }) {
            return Err(fail(args, "a model of ml.fit (one model, not one per class)"));
        }
        let column: String = args.param("column", "target".to_string())?;
        let (x, y) = records(args, model, Some(&column))?;
        let repeats: usize = args.param("n_repeats", 5)?;
        if repeats == 0 {
            return Err(fail(args, "n_repeats: 1 or more"));
        }
        let fail_s = |e: String| fail(args, e);
        let baseline = score(model, &x, &y).map_err(fail_s)?;
        let seed = Mt19937::new(args.param("random_state", 0)?).randint(0, 1 << 31) as u32;
        let n = y.len();
        let nf = x.names.len();
        // drops[f][r]
        let mut drops = vec![vec![0.0; repeats]; nf];
        for (f, d) in drops.iter_mut().enumerate() {
            if !ctx.progress(Some(f as f64 / nf as f64), x.names[f].clone()) {
                return Err(fail(args, "stopped"));
            }
            let mut rng = Mt19937::new(seed);
            let mut idx: Vec<usize> = (0..n).collect();
            let mut permuted = x.clone();
            for r in d.iter_mut() {
                for i in (1..n).rev() {
                    let j = rng.randint(0, i as i64 + 1) as usize;
                    idx.swap(i, j);
                }
                let col = permuted.columns[f].clone();
                permuted.columns[f] = idx.iter().map(|&i| col[i]).collect();
                *r = baseline - score(model, &permuted, &y).map_err(fail_s)?;
            }
        }
        let mean: Vec<f64> = drops.iter().map(|d| numpy_sum(d) / repeats as f64).collect();
        let sd: Vec<f64> = drops
            .iter()
            .zip(&mean)
            .map(|(d, m)| (numpy_sum(&d.iter().map(|v| (v - m) * (v - m)).collect::<Vec<_>>()) / repeats as f64).sqrt())
            .collect();
        let row = |v: Vec<f64>| Table { columns: x.names.clone(), rows: vec![v] };
        let mut each = Table::new(std::iter::once("repeat".to_string()).chain(x.names.iter().cloned()).collect());
        for r in 0..repeats {
            each.rows.push(std::iter::once((r + 1) as f64).chain(drops.iter().map(|d| d[r])).collect());
        }
        Ok(outputs([
            ("importance", Value::Table(row(mean))),
            ("sd", Value::Table(row(sd))),
            ("repeats", Value::Table(each)),
            ("score", Value::Number(baseline)),
        ]))
    }
}

/// scipy's mquantiles (alphap = betap = 0.4) of sorted values at `p`.
fn mquantile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let m = 0.4 + p * (1.0 - 0.4 - 0.4);
    let aleph = n as f64 * p + m;
    let k = aleph.clamp(1.0, (n - 1) as f64).floor();
    let gamma = (aleph - k).clamp(0.0, 1.0);
    let k = k as usize;
    (1.0 - gamma) * sorted[k - 1] + gamma * sorted[k]
}

/// numpy's linspace(a, b, num) (the last value b).
fn linspace(a: f64, b: f64, num: usize) -> Vec<f64> {
    let step = (b - a) / (num - 1) as f64;
    let mut y: Vec<f64> = (0..num).map(|i| i as f64 * step + a).collect();
    y[num - 1] = b;
    y
}

struct PartialDependence;

impl Operator for PartialDependence {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "ml.partial_dependence",
            "How a fitted model's prediction follows a predictor (scikit-learn's partial_dependence, its brute \
             method): every record with the predictor set to each value of a grid (its distinct values when fewer \
             than the grid's resolution, else from one percentile of it to another, as scipy's mquantiles), the \
             predictions averaged: the value of a regressor, the probability of each class of a classifier (the \
             second of two).",
            Access::None,
        )
        .input(PortSpec::new("model", Type::MlModel, "the fitted model (ml.fit)"))
        .input(PortSpec::new("table", Type::Table, "the records, a column per predictor"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per value of the grid: the predictor's value, then the mean prediction (value, or p_<class>)",
        ))
        .param(PortSpec::new("predictor", Type::Text, "the predictor followed"))
        .param(PortSpec::optional("grid_resolution", Type::Integer, "the values of the grid (default 100)"))
        .param(PortSpec::optional(
            "percentiles",
            Type::Any,
            "the grid's ends, as shares (default [0.05, 0.95])",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let model = model_of(args)?;
        if matches!(**model, Model::PerClass { .. }) {
            return Err(fail(args, "a model of ml.fit (one model, not one per class)"));
        }
        let (x, _) = records(args, model, None)?;
        let name: String = args.param("predictor", String::new())?;
        let f = x
            .names
            .iter()
            .position(|n| *n == name)
            .ok_or_else(|| fail(args, format!("the model has no predictor {name:?} (it has {:?})", x.names)))?;
        let resolution: usize = args.param("grid_resolution", 100)?;
        let [lo, hi]: [f64; 2] = args.param("percentiles", [0.05, 0.95])?;
        if resolution < 2 || !(0.0..=1.0).contains(&lo) || !(0.0..=1.0).contains(&hi) || lo >= hi {
            return Err(fail(args, "grid_resolution: 2 or more; percentiles: two shares, the first below the second"));
        }
        let mut sorted = x.columns[f].clone();
        sorted.sort_by(f64::total_cmp);
        let mut unique = sorted.clone();
        unique.dedup_by(|a, b| a == b);
        // -0 and 0 are one value (numpy keeps either, by its sort)
        unique.iter_mut().filter(|v| **v == 0.0).for_each(|v| *v = 0.0);
        let grid = if unique.len() < resolution {
            unique
        } else {
            let (a, b) = (mquantile(&sorted, lo), mquantile(&sorted, hi));
            if (a - b).abs() <= 1e-8 + 1e-5 * b.abs() {
                return Err(fail(args, "the percentiles are too close to each other to make a grid"));
            }
            linspace(a, b, resolution)
        };
        let classes = model.classes().to_vec();
        let columns: Vec<String> = if classes.is_empty() {
            vec!["value".into()]
        } else if classes.len() == 2 {
            vec![format!("p_{}", class_label(classes[1]))]
        } else {
            classes.iter().map(|c| format!("p_{}", class_label(*c))).collect()
        };
        let mut t = Table::new(std::iter::once(name.clone()).chain(columns).collect());
        let mut eval = x.clone();
        let n = eval.rows() as f64;
        for (g, &v) in grid.iter().enumerate() {
            if !ctx.progress(Some(g as f64 / grid.len() as f64), format!("{name} = {v}")) {
                return Err(fail(args, "stopped"));
            }
            eval.columns[f].iter_mut().for_each(|c| *c = v);
            let out = model.outputs(&eval).map_err(|e| fail(args, e))?;
            let take = if classes.len() == 2 { &out[1..] } else { &out[..] };
            t.rows.push(std::iter::once(v).chain(take.iter().map(|c| numpy_sum(c) / n)).collect());
        }
        Ok(outputs([("table", Value::Table(t))]))
    }
}
