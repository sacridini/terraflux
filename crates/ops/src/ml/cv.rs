//! The cross-validation of `ml.fit`: the model refitted with each fold left out and tested on it, the predictions
//! of all the folds scored together and by fold (scikit-learn's cross_val_predict and its metrics).

use super::{Fitted, Frame, Model, fail, fit_model};
use crate::signatures::NumpyRng;
use crate::value::class_label;
use crate::{Args, Ctx, Result, Table};

/// What a cross-validation gives.
pub(super) struct Cv {
    /// A row per fold and a last for all the predictions together.
    pub table: Table,
    /// The confusion matrix of all the predictions (classes only): a row per predicted class, a column per
    /// reference class, as objects.accuracy's.
    pub matrix: Option<Table>,
}

/// Random folds (1 to k, as numbers), stratified by class when the target is of classes: each class apart in a random
/// order, dealt round the folds, the deal going on from one class to the next so that the folds hold the same number
/// of records within one.
pub(super) fn random_folds(y: &[f64], regression: bool, k: usize, seed: u64) -> Vec<f64> {
    let mut rng = NumpyRng::new(seed);
    let mut fold = vec![0.0; y.len()];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    if regression {
        groups.push((0..y.len()).collect());
    } else {
        let mut classes = y.to_vec();
        classes.sort_by(f64::total_cmp);
        classes.dedup();
        for c in classes {
            groups.push((0..y.len()).filter(|&i| y[i] == c).collect());
        }
    }
    let mut counter = 0;
    for mut g in groups {
        rng.shuffle(&mut g);
        for i in g {
            fold[i] = (counter % k + 1) as f64;
            counter += 1;
        }
    }
    fold
}

fn subset(x: &Frame, rows: &[usize]) -> Frame {
    Frame { names: x.names.clone(), columns: x.columns.iter().map(|c| rows.iter().map(|&i| c[i]).collect()).collect() }
}

/// The scores of some predictions of classes: the number of records, the accuracy, Cohen's kappa, the macro F1 and the
/// F1 of each class (none, NaN, for a class that is neither in the reference nor in the predictions), from the
/// confusion matrix `count[predicted][reference]`.
pub(super) fn class_scores(count: &[Vec<f64>]) -> Vec<f64> {
    let q = count.len();
    let n: f64 = count.iter().flatten().sum();
    let row: Vec<f64> = count.iter().map(|r| r.iter().sum()).collect();
    let col: Vec<f64> = (0..q).map(|j| count.iter().map(|r| r[j]).sum()).collect();
    let diag: f64 = (0..q).map(|j| count[j][j]).sum();
    let (po, pe) = (diag / n, (0..q).map(|j| row[j] * col[j]).sum::<f64>() / (n * n));
    let kappa = if pe < 1.0 { (po - pe) / (1.0 - pe) } else { f64::NAN };
    let f1: Vec<f64> =
        (0..q).map(|j| if row[j] + col[j] > 0.0 { 2.0 * count[j][j] / (row[j] + col[j]) } else { f64::NAN }).collect();
    let seen: Vec<f64> = f1.iter().copied().filter(|v| !v.is_nan()).collect();
    let macro_f1 = seen.iter().sum::<f64>() / seen.len() as f64;
    let mut out = vec![n, po, kappa, macro_f1];
    out.extend(f1);
    out
}

/// The scores of some predictions of values: the number of records, RMSE, MAE and R2.
pub(super) fn value_scores(pred: &[f64], real: &[f64]) -> Vec<f64> {
    let n = real.len() as f64;
    let mean = real.iter().sum::<f64>() / n;
    let res: f64 = pred.iter().zip(real).map(|(p, r)| (r - p).powi(2)).sum();
    let tot: f64 = real.iter().map(|r| (r - mean).powi(2)).sum();
    let mae = pred.iter().zip(real).map(|(p, r)| (r - p).abs()).sum::<f64>() / n;
    let r2 = if tot > 0.0 {
        1.0 - res / tot
    } else if res == 0.0 {
        1.0
    } else {
        0.0
    };
    vec![n, (res / n).sqrt(), mae, r2]
}

/// The table of the out-of-bag predictions of a forest (`pred` per record: the averaged outputs, none for a record
/// every tree drew), scored as the cross-validation's last row, without its fold column. `known` are the forest's
/// classes (empty for values); returns the table and how many records had no prediction.
pub(super) fn oob_table(
    pred: &[Option<Vec<f64>>],
    y: &[f64],
    known: &[f64],
    name: &dyn Fn(f64) -> String,
) -> (Table, usize) {
    let regression = known.is_empty();
    let mut head = vec!["n".to_string()];
    if regression {
        head.extend(["rmse", "mae", "r2"].map(String::from));
    } else {
        head.extend(["accuracy", "kappa", "macro_f1"].map(String::from));
        head.extend(known.iter().map(|&c| format!("f1_{}", name(c))));
    }
    let mut table = Table::new(head);
    let q = known.len();
    let mut count = vec![vec![0.0f64; q]; q];
    let (mut p, mut r) = (Vec::new(), Vec::new());
    let mut left = 0;
    for (o, &real) in pred.iter().zip(y) {
        let Some(o) = o else {
            left += 1;
            continue;
        };
        if regression {
            p.push(o[0]);
            r.push(real);
        } else {
            // the class of the largest probability, the first on ties
            let mut k = 0;
            for c in 1..o.len() {
                if o[c] > o[k] {
                    k = c;
                }
            }
            count[k][known.partition_point(|c| *c < real)] += 1.0;
        }
    }
    table.rows.push(if regression { value_scores(&p, &r) } else { class_scores(&count) });
    (table, left)
}

/// Fit the model of the node with each fold out and test it on that fold. `folds` is a fold of each record (NaN: in
/// none, left out of the cross-validation); `name` gives a class its name.
pub(super) fn cross_validate(
    ctx: &mut Ctx,
    args: &Args,
    x: &Frame,
    y: &[f64],
    folds: &[f64],
    regression: bool,
    name: &dyn Fn(f64) -> String,
) -> Result<Cv> {
    let mut values: Vec<f64> = folds.iter().copied().filter(|f| !f.is_nan()).collect();
    values.sort_by(f64::total_cmp);
    values.dedup();
    if values.len() < 2 {
        return Err(fail(args, "the cross-validation needs two folds at least"));
    }
    let mut classes = y.to_vec();
    classes.sort_by(f64::total_cmp);
    classes.dedup();
    let q = classes.len();
    let code = |v: f64| classes.partition_point(|c| *c < v);
    let mut predicted = vec![f64::NAN; y.len()];
    let mut by_fold: Vec<Vec<usize>> = Vec::new();
    for &f in &values {
        let test: Vec<usize> = (0..y.len()).filter(|&i| folds[i] == f).collect();
        let train: Vec<usize> = (0..y.len()).filter(|&i| !folds[i].is_nan() && folds[i] != f).collect();
        let ytrain: Vec<f64> = train.iter().map(|&i| y[i]).collect();
        let fitted = fit_model(args, &subset(x, &train), &ytrain, regression, &mut || !ctx.stopped())
            .map_err(|e| fail(args, format!("fold {}: {e}", class_label(f))))?;
        let model = match fitted {
            Fitted::Forest(m) => Model::Forest(m),
            Fitted::Boosting(m) => Model::Boosting(m),
        };
        let out = model.outputs(&subset(x, &test)).map_err(|e| fail(args, e))?;
        let known = model.classes().to_vec();
        for (r, &i) in test.iter().enumerate() {
            predicted[i] = if regression {
                out[0][r]
            } else {
                // the class of the largest probability, the first on ties (ml.predict's)
                let mut k = 0;
                for c in 1..out.len() {
                    if out[c][r] > out[k][r] {
                        k = c;
                    }
                }
                known[k]
            };
        }
        by_fold.push(test);
    }
    // the table: a row per fold, then all together
    let mut head = vec!["fold".to_string(), "n".into()];
    if regression {
        head.extend(["rmse", "mae", "r2"].map(String::from));
    } else {
        head.extend(["accuracy", "kappa", "macro_f1"].map(String::from));
        head.extend(classes.iter().map(|&c| format!("f1_{}", name(c))));
    }
    let mut table = Table::new(head);
    let all: Vec<usize> = by_fold.iter().flatten().copied().collect();
    let mut groups = by_fold.clone();
    groups.push(all.clone());
    let mut matrix = vec![vec![0.0f64; q]; q];
    for (g, rows) in groups.iter().enumerate() {
        let scores = if regression {
            let (p, r): (Vec<f64>, Vec<f64>) = rows.iter().map(|&i| (predicted[i], y[i])).unzip();
            value_scores(&p, &r)
        } else {
            let mut count = vec![vec![0.0f64; q]; q];
            for &i in rows {
                count[code(predicted[i])][code(y[i])] += 1.0;
            }
            if g == groups.len() - 1 {
                matrix = count.clone();
            }
            class_scores(&count)
        };
        let mut row = vec![(g + 1) as f64];
        row.extend(scores);
        table.rows.push(row);
    }
    let mut labels: Vec<String> = values.iter().map(|&f| class_label(f)).collect();
    labels.push("all".into());
    table.labels.insert("fold".into(), labels);
    let matrix = (!regression).then(|| {
        let names: Vec<String> = classes.iter().map(|&c| name(c)).collect();
        let mut t = Table::new(std::iter::once("map".to_string()).chain(names.iter().cloned()).collect());
        for (k, r) in matrix.iter().enumerate() {
            t.rows.push(std::iter::once((k + 1) as f64).chain(r.iter().copied()).collect());
        }
        t.labels.insert("map".into(), names);
        t
    });
    Ok(Cv { table, matrix })
}
