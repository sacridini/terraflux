//! `ml.pca`: the principal components of the variables of a table or of the layers of a stack, fitted to all the
//! complete records up to a million (a sample above), and the scores of any other records from the fit's means and
//! scales (`ml.predict`).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::{Model, fail, sample_size};
use crate::multivar::linalg;
use crate::multivar::pca::{self, Keep, Pca};
use crate::signatures::NumpyRng;
use crate::value::Key;
use crate::{Args, Ctx, Operator, Outputs, Result, Stack, Table, Value, outputs};

/// The records the fit uses at most when no `sample` is given.
const FIT_RECORDS: usize = 1_000_000;

pub struct Components;

/// The scores of the cells of a stack: a layer per component, NaN where a variable has none.
fn stack_scores(args: &Args, p: &Pca, stack: &Stack) -> Result<Stack> {
    let layers: Vec<&Arc<Vec<f64>>> = p
        .predictors
        .iter()
        .map(|n| stack.by_name(n).ok_or_else(|| fail(args, format!("the stack has no layer {n:?}"))))
        .collect::<Result<_>>()?;
    let slices: Vec<&[f64]> = layers.iter().map(|l| l.as_slice()).collect();
    if slices.iter().any(|l| l.len() != stack.grid.cells()) {
        return Err(fail(args, "the layers differ in size from the grid"));
    }
    let scores = p.scores(&slices).map_err(|e| fail(args, e))?;
    Ok(Stack {
        grid: stack.grid.clone(),
        keys: p.names().into_iter().map(Key::Name).collect(),
        layers: scores.into_iter().map(Arc::new).collect(),
    })
}

/// A table with the scores of its rows as columns pc1, pc2... after its own, NaN for a row with a variable none.
fn table_scores(args: &Args, p: &Pca, t: &Table) -> Result<Table> {
    let columns: Vec<Vec<f64>> = p
        .predictors
        .iter()
        .map(|n| t.column(n).ok_or_else(|| fail(args, format!("the table has no column {n:?}"))))
        .collect::<Result<_>>()?;
    let slices: Vec<&[f64]> = columns.iter().map(Vec::as_slice).collect();
    let scores = p.scores(&slices).map_err(|e| fail(args, e))?;
    let names = p.names();
    if let Some(c) = names.iter().find(|n| t.columns.contains(n)) {
        return Err(fail(args, format!("the table already has a column {c:?}")));
    }
    let mut out = t.clone();
    out.columns.extend(names);
    for (i, row) in out.rows.iter_mut().enumerate() {
        row.extend(scores.iter().map(|s| s[i]));
    }
    Ok(out)
}

/// What `ml.predict` gives a PCA model: the scores of the layers of a stack (`scores`) or the rows of a table (`table`).
pub(super) fn apply(args: &Args, p: &Pca) -> Result<Outputs> {
    match (args.get("predictors"), args.get("table")) {
        (Some(_), Some(_)) => Err(fail(args, "maps (predictors) or a table, not both")),
        (Some(_), None) => {
            let s = stack_scores(args, p, args.stack("predictors")?)?;
            Ok(outputs([("scores", Value::Stack(s))]))
        }
        (None, Some(Value::Table(t))) => Ok(outputs([("table", Value::Table(table_scores(args, p, t)?))])),
        (None, Some(v)) => Err(fail(args, format!("table: a table, not a {:?}", v.ty()))),
        (None, None) => Err(fail(args, "connect the maps (predictors) or a table")),
    }
}

/// How many components the setting asks for: a number of them (1 or more, a whole number) or a share of the variance
/// (above 0, below 1).
fn keep_of(args: &Args) -> Result<Keep> {
    let c: f64 = args.param("components", 0.95)?;
    if !c.is_finite() || c <= 0.0 {
        return Err(fail(
            args,
            format!("components {c}: a number of components (1 or more) or a share of the variance (0 to 1)"),
        ));
    }
    if c < 1.0 {
        Ok(Keep::Share(c))
    } else if c.fract() == 0.0 {
        Ok(Keep::Count(c as usize))
    } else {
        Err(fail(args, format!("components {c}: a whole number of components, or a share of the variance below 1")))
    }
}

/// Tell the constant columns of a standardised fit: they are centred and not divided, and weigh nothing.
pub(super) fn warn_constant(ctx: &mut Ctx, args: &Args, names: &[String], constant: &[usize]) {
    if constant.is_empty() {
        return;
    }
    let list: Vec<String> = constant.iter().map(|&j| format!("{:?}", names[j])).collect();
    ctx.warn(format!(
        "node {:?}: {} constant: they weigh nothing when standardising (scale 1, as scikit-learn's StandardScaler)",
        args.node,
        list.join(", ")
    ));
}

/// The records a fit reads: the variables' names and values (a table's columns, or a stack's named layers), and the
/// indices of the records the fit uses, those with every variable (and, with maps, in `cells`), all of them up to a
/// million or a sample (`sample`, `seed`; said in a note).
pub(super) struct Records {
    pub names: Vec<String>,
    pub columns: Vec<Arc<Vec<f64>>>,
    pub idx: Vec<usize>,
}

/// The records of the node's `table` or `predictors` (and `cells`), and the sample of them to fit on.
pub(super) fn records(ctx: &mut Ctx, args: &Args) -> Result<Records> {
    records_of(ctx, args, None)
}

/// [`records`], the maps being `stack` when it is given (the layers of two dates side by side) instead of the node's
/// `predictors`.
fn records_of(ctx: &mut Ctx, args: &Args, stack: Option<&Stack>) -> Result<Records> {
    // the variables: their names and values
    let (names, columns): (Vec<String>, Vec<Arc<Vec<f64>>>) = match (args.get("table"), args.get("predictors")) {
        (Some(_), Some(_)) => return Err(fail(args, "a table or maps (predictors), not both")),
        (Some(Value::Table(t)), None) => {
            if args.get("cells").is_some() {
                return Err(fail(args, "cells goes with maps, not with a table"));
            }
            let names: Vec<String> = match args.opt_param::<Vec<String>>("columns")? {
                Some(c) => c,
                None => t.columns.iter().filter(|c| !t.is_text(c)).cloned().collect(),
            };
            if names.is_empty() {
                return Err(fail(args, "the table has no numeric column"));
            }
            let mut columns = Vec::new();
            for n in &names {
                columns
                    .push(Arc::new(t.column(n).ok_or_else(|| fail(args, format!("the table has no column {n:?}")))?));
            }
            (names, columns)
        }
        (Some(v), None) => return Err(fail(args, format!("table: a table, not a {:?}", v.ty()))),
        (None, Some(_)) => {
            let stack = match stack {
                Some(s) => s,
                None => args.stack("predictors")?,
            };
            let (names, layers): (Vec<String>, Vec<Arc<Vec<f64>>>) = stack
                .keys
                .iter()
                .zip(&stack.layers)
                .filter_map(|(k, l)| match k {
                    Key::Name(n) => Some((n.clone(), l.clone())),
                    _ => None,
                })
                .unzip();
            if names.is_empty() {
                return Err(fail(args, "the predictors' stack has no layer named by a variable"));
            }
            if layers.iter().any(|l| l.len() != stack.grid.cells()) {
                return Err(fail(args, "the layers differ in size from the grid"));
            }
            (names, layers)
        }
        (None, None) => return Err(fail(args, "connect the records: a table, or maps (predictors)")),
    };
    if let Some(i) = (1..names.len()).find(|&i| names[..i].contains(&names[i])) {
        return Err(fail(args, format!("the variable {:?} is twice", names[i])));
    }
    let n = columns[0].len();
    // the records with every variable (and, with maps, in the cells)
    let cells = args.opt_map("cells")?;
    if cells.is_some_and(|c| c.data.len() != n) {
        return Err(fail(args, "cells differs in size from the predictors"));
    }
    let complete: Vec<usize> = (0..n)
        .filter(|&i| {
            columns.iter().all(|c| !c[i].is_nan()) && cells.is_none_or(|c| !c.data[i].is_nan() && c.data[i] != 0.0)
        })
        .collect();
    let total = complete.len();
    if total == 0 {
        return Err(fail(args, "no record has every variable"));
    }
    // the sample: numpy's draw, sorted (all of them up to a million by default)
    let want = match args.opt_param::<f64>("sample")? {
        Some(s) if !s.is_finite() || s <= 0.0 => {
            return Err(fail(args, format!("sample {s}: a number of records (1 or more) or a share (below 1)")));
        }
        Some(s) => sample_size(s, total),
        None => total.min(FIT_RECORDS),
    };
    if want == 0 {
        return Err(fail(args, "the sample holds no record"));
    }
    let idx: Vec<usize> = if want < total {
        let seed: u64 = args.param("seed", 1)?;
        let mut pick = NumpyRng::new(seed).choice_distinct(total, want);
        pick.sort_unstable();
        ctx.warn(format!(
            "node {:?}: fitted on a sample of {want} of the {total} records with every variable (seed {seed}; \
             the setting sample)",
            args.node
        ));
        pick.into_iter().map(|k| complete[k]).collect()
    } else {
        complete
    };
    Ok(Records { names, columns, idx })
}

/// The maps of two dates of the same layers: `predictors` and `later`, and how they are analysed.
struct Pair<'a> {
    /// The layers' names (the same in both).
    names: Vec<String>,
    first: &'a Stack,
    later: &'a Stack,
    selective: bool,
}

impl Pair<'_> {
    /// The layers of both dates side by side as a stack: `<name>_1` for each of the first, then `<name>_2` for each of
    /// the later.
    fn stacked(&self) -> Stack {
        let mut keys = Vec::new();
        let mut layers = Vec::new();
        for (date, s) in [(1, self.first), (2, self.later)] {
            for n in &self.names {
                keys.push(Key::Name(format!("{n}_{date}")));
                layers.push(s.by_name(n).expect("checked").clone());
            }
        }
        Stack { grid: self.first.grid.clone(), keys, layers }
    }
}

/// The two dates of the node (`later`, `change`), or none when it has only one: refused when `later` does not fit
/// `predictors`.
fn dates_of<'a>(args: &'a Args) -> Result<Option<Pair<'a>>> {
    let change: Option<String> = args.opt_param("change")?;
    let Some(later) = args.get("later") else {
        return match change {
            Some(_) => Err(fail(args, "change goes with two dates: connect the later maps (later)")),
            None => Ok(None),
        };
    };
    if args.get("table").is_some() {
        return Err(fail(args, "later goes with maps (predictors), not with a table"));
    }
    if args.get("predictors").is_none() {
        return Err(fail(args, "connect the maps of the first date (predictors) with the later ones"));
    }
    let selective = match change.as_deref() {
        None | Some("stacked") => false,
        Some("selective") => true,
        Some(c) => return Err(fail(args, format!("change {c:?}: stacked or selective"))),
    };
    if args.get("cells").is_some() && selective {
        return Err(fail(args, "cells goes with change stacked, not selective"));
    }
    let Value::Stack(later) = later else { return Err(fail(args, format!("later: a stack, not a {:?}", later.ty()))) };
    let first = args.stack("predictors")?;
    let names = first.names();
    if names.is_empty() {
        return Err(fail(args, "the predictors' stack has no layer named by a variable"));
    }
    let (had, has) = (names.clone(), later.names());
    let lacking: Vec<&String> = had.iter().filter(|n| !has.contains(n)).collect();
    let extra: Vec<&String> = has.iter().filter(|n| !had.contains(n)).collect();
    if !lacking.is_empty() || !extra.is_empty() || first.layers.len() != later.layers.len() {
        let list = |v: &[&String]| v.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>().join(", ");
        return Err(fail(
            args,
            format!(
                "the later maps must have the layers of the first date, by name (the first: {}; the later: {}){}{}",
                names.join(", "),
                has.join(", "),
                if lacking.is_empty() { String::new() } else { format!("; missing in later: {}", list(&lacking)) },
                if extra.is_empty() { String::new() } else { format!("; not in the first: {}", list(&extra)) },
            ),
        ));
    }
    if later.grid != first.grid {
        return Err(fail(args, "the later maps are on another grid than the first date's (raster.match them first)"));
    }
    Ok(Some(Pair { names, first, later, selective }))
}

/// How much a component of the two dates stacked (the first date's loadings, then the later's) is a change:
/// |a - b|^2 / (|a - b|^2 + |a + b|^2), 1 for a pure difference of the dates and 0 for a pure sum.
fn change_of(v: &[f64]) -> f64 {
    let n = v.len() / 2;
    let (mut diff, mut sum) = (0.0, 0.0);
    for j in 0..n {
        diff += (v[j] - v[n + j]).powi(2);
        sum += (v[j] + v[n + j]).powi(2);
    }
    diff / (diff + sum)
}

/// The eigenvalues (from the largest) and eigenvectors (rows) of a symmetric matrix, the matrix scaled to a mean
/// diagonal of 1 first so that the rotations' stopping rule means the same for bands of any size of numbers.
fn sym_eigen(a: &[f64], d: usize) -> (Vec<f64>, Vec<Vec<f64>>) {
    let scale = (0..d).map(|i| a[i * d + i]).sum::<f64>() / d as f64;
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let scaled: Vec<f64> = a.iter().map(|v| v / scale).collect();
    let (values, vectors) = linalg::eigen(&scaled, d);
    (values.into_iter().map(|v| v * scale).collect(), vectors)
}

/// `sum_k f(lambda_k) q_k q_k^T` of a symmetric matrix's eigenvalues and eigenvectors (rows), as a d x d matrix.
fn spectral_function(values: &[f64], vectors: &[Vec<f64>], f: impl Fn(f64) -> f64) -> Vec<Vec<f64>> {
    let d = values.len();
    let mut m = vec![vec![0.0; d]; d];
    for (k, q) in vectors.iter().enumerate() {
        let fk = f(values[k]);
        for i in 0..d {
            for j in 0..d {
                m[i][j] += fk * q[i] * q[j];
            }
        }
    }
    m
}

/// The covariance of the differences between neighbouring cells, over 2: the pairs (a cell and the one `step` cells
/// on, to its right when `step` is 1) whose first cell is `anchor` and whose second has every band. `None` with fewer
/// than two pairs.
fn noise_cov(
    columns: &[std::sync::Arc<Vec<f64>>],
    anchor: &[bool],
    ok: &[bool],
    width: usize,
    right: bool,
) -> Option<Vec<f64>> {
    let n = anchor.len();
    let mut diffs: Vec<Vec<f64>> = vec![Vec::new(); columns.len()];
    for i in (0..n).filter(|&i| anchor[i]) {
        let j = if right {
            if i % width == width - 1 {
                continue;
            }
            i + 1
        } else {
            i + width
        };
        if j >= n || !ok[j] {
            continue;
        }
        for (d, c) in diffs.iter_mut().zip(columns) {
            d.push(c[i] - c[j]);
        }
    }
    let slices: Vec<&[f64]> = diffs.iter().map(Vec::as_slice).collect();
    linalg::moments(&slices, 1).map(|m| m.cov.iter().map(|v| v / 2.0).collect())
}

/// The minimum noise fraction (Green, Berman, Switzer and Craig 1988; Lee, Woodyatt and Berman 1990's noise-adjusted
/// principal components): the noise's covariance N from the differences between neighbouring cells, the data whitened
/// by it (N^-1/2), and the principal components of the whitened data, from the largest eigenvalue (the signal-to-noise
/// ratio plus 1). The score of a component is its eigenvector's product with N^-1/2 and the centred cell, so its noise
/// has a variance of 1 (as Spectral Python's `MNFResult.reduce`); `denoised` is the cell rebuilt from the components
/// kept (`denoise`).
fn mnf(ctx: &mut Ctx, args: &Args, keep: Keep) -> Result<Outputs> {
    if args.get("table").is_some() {
        return Err(fail(
            args,
            "method mnf reads maps (predictors): the noise is the difference between neighbouring cells",
        ));
    }
    if args.get("later").is_some() || args.opt_param::<String>("change")?.is_some() {
        return Err(fail(args, "method mnf is for one date: not with later (and change)"));
    }
    let noise: String = args.param("noise", "shift".to_string())?;
    if !["shift", "shift_both"].contains(&noise.as_str()) {
        return Err(fail(args, format!("noise {noise:?}: shift or shift_both")));
    }
    let stack = args.stack("predictors")?;
    let Records { names, columns, idx } = records_of(ctx, args, None)?;
    let (d, n, width) = (names.len(), columns[0].len(), stack.grid.width);
    // the cells with every band (and in `cells`), and those the fit and the noise pairs start from
    let cells = args.opt_map("cells")?;
    let ok: Vec<bool> = (0..n)
        .map(|i| {
            columns.iter().all(|c| !c[i].is_nan()) && cells.is_none_or(|c| !c.data[i].is_nan() && c.data[i] != 0.0)
        })
        .collect();
    let mut anchor = vec![false; n];
    idx.iter().for_each(|&i| anchor[i] = true);
    let whole = ok.iter().filter(|&&b| b).count();
    if idx.len() < whole {
        ctx.warn(format!(
            "node {:?}: the signal and the noise are from a sample: the {} cells drawn and their right-hand and lower \
             neighbours with every band, of the {whole} cells with every band",
            args.node,
            idx.len()
        ));
    }
    // the signal: the covariance of the cells
    let used: Vec<Vec<f64>> = columns.iter().map(|c| idx.iter().map(|&i| c[i]).collect()).collect();
    let slices: Vec<&[f64]> = used.iter().map(Vec::as_slice).collect();
    let signal = linalg::moments(&slices, 1).ok_or_else(|| fail(args, "an MNF needs two cells with every band"))?;
    // the noise: the covariance of the differences
    let few = || fail(args, "the noise needs two pairs of neighbouring cells with every band at least");
    let right = noise_cov(&columns, &anchor, &ok, width, true);
    let noise_matrix = if noise == "shift" {
        right.ok_or_else(few)?
    } else {
        match (right, noise_cov(&columns, &anchor, &ok, width, false)) {
            (Some(h), Some(v)) => h.iter().zip(&v).map(|(a, b)| (a + b) / 2.0).collect(),
            (Some(c), None) | (None, Some(c)) => c,
            (None, None) => return Err(few()),
        }
    };
    let (nvals, nvecs) = sym_eigen(&noise_matrix, d);
    let top = nvals.first().copied().unwrap_or(0.0);
    if !top.is_finite() || top <= 0.0 || nvals.iter().any(|&v| v <= top * 1e-12) {
        return Err(fail(
            args,
            "the noise has no variance in some direction (a band that does not change between neighbours, or bands \
             that repeat each other): drop those bands",
        ));
    }
    let inv_root = spectral_function(&nvals, &nvecs, |v| 1.0 / v.sqrt());
    let root = spectral_function(&nvals, &nvecs, f64::sqrt);
    // the whitened signal's covariance N^-1/2 S N^-1/2, and its eigenvectors
    let times = |a: &[Vec<f64>], b: &[Vec<f64>]| -> Vec<Vec<f64>> {
        (0..d).map(|i| (0..d).map(|j| (0..d).map(|k| a[i][k] * b[k][j]).sum()).collect()).collect()
    };
    let s: Vec<Vec<f64>> = (0..d).map(|i| signal.cov[i * d..(i + 1) * d].to_vec()).collect();
    let c = times(&times(&inv_root, &s), &inv_root);
    let sym: Vec<f64> = (0..d * d).map(|k| (c[k / d][k % d] + c[k % d][k / d]) / 2.0).collect();
    let (values, vectors) = sym_eigen(&sym, d);
    let variance: Vec<f64> = values.iter().map(|v| v.max(0.0)).collect();
    let total: f64 = variance.iter().sum();
    if total.is_nan() || total <= 0.0 {
        return Err(fail(args, "the bands have no variance"));
    }
    let explained: Vec<f64> = variance.iter().map(|v| v / total).collect();
    let k = pca::kept(&explained, keep).map_err(|e| fail(args, e))?;
    // the loading of a band in component c is (N^-1/2 v_c) and its way back (N^1/2 v_c), signed so that the
    // largest loading is positive
    let (mut loadings, mut back) = (Vec::new(), Vec::new());
    for v in vectors.iter().take(k) {
        let mut w: Vec<f64> = (0..d).map(|j| (0..d).map(|i| inv_root[j][i] * v[i]).sum()).collect();
        let mut b: Vec<f64> = (0..d).map(|j| (0..d).map(|i| root[j][i] * v[i]).sum()).collect();
        let sign = linalg::sign_of(&w);
        w.iter_mut().chain(b.iter_mut()).for_each(|x| *x *= sign);
        loadings.push(w);
        back.push(b);
    }
    let p = Pca {
        predictors: names.clone(),
        mean: signal.mean,
        scale: vec![1.0; d],
        components: loadings,
        variance,
        explained,
        prefix: "mnf".into(),
    };
    let scores = stack_scores(args, &p, stack)?;
    // the cells rebuilt from the components kept
    let denoised = Stack {
        grid: stack.grid.clone(),
        keys: names.iter().cloned().map(Key::Name).collect(),
        layers: (0..d)
            .map(|j| {
                Arc::new(
                    (0..n)
                        .map(|i| {
                            let mut v = 0.0;
                            for (c, layer) in scores.layers.iter().enumerate() {
                                v += back[c][j] * layer[i];
                            }
                            p.mean[j] + v
                        })
                        .collect(),
                )
            })
            .collect(),
    };
    let mut head = vec!["component".to_string(), "explained".to_string()];
    head.extend(names.iter().cloned());
    let mut table = Table::new(head);
    for (c, w) in p.components.iter().enumerate() {
        table.rows.push([(c + 1) as f64, p.explained[c]].into_iter().chain(w.iter().copied()).collect());
    }
    let head = ["component", "variance", "ratio", "cumulative", "snr"].map(String::from).to_vec();
    let mut eigenvalues = Table::new(head);
    let mut cum = 0.0;
    for c in 0..d {
        cum += p.explained[c];
        eigenvalues.rows.push(vec![(c + 1) as f64, p.variance[c], p.explained[c], cum, p.variance[c] - 1.0]);
    }
    let mut out = outputs([("scores", Value::Stack(scores))]);
    out.insert("denoised".into(), Value::Stack(denoised));
    out.insert("loadings".into(), Value::Table(table));
    out.insert("explained".into(), Value::Table(eigenvalues));
    out.insert("model".into(), Value::MlModel(Arc::new(Model::Pca(p))));
    Ok(out)
}

/// Chavez and Kwarteng's selective PCA: for each layer, the PCA of the pair (the layer at the first date, at the later)
/// and its two components, `<layer>_pc1` (what the dates share) and `<layer>_pc2` (what changed).
fn selective(ctx: &mut Ctx, args: &Args, pr: &Pair, standardise: bool) -> Result<Outputs> {
    let mut keys = Vec::new();
    let mut layers = Vec::new();
    let mut loadings = Table::new(["band", "component", "explained", "first", "later"].map(String::from).to_vec());
    let mut explained = Table::new(["band", "component", "variance", "ratio", "cumulative"].map(String::from).to_vec());
    for (b, name) in pr.names.iter().enumerate() {
        let pair = [format!("{name}_1"), format!("{name}_2")];
        let sub = Stack {
            grid: pr.first.grid.clone(),
            keys: pair.iter().cloned().map(Key::Name).collect(),
            layers: vec![
                pr.first.by_name(name).expect("checked").clone(),
                pr.later.by_name(name).expect("checked").clone(),
            ],
        };
        let Records { names, columns, idx } = records_of(ctx, args, Some(&sub))?;
        let used: Vec<Vec<f64>> = columns.iter().map(|c| idx.iter().map(|&i| c[i]).collect()).collect();
        let (p, constant) = pca::fit_noting(names.clone(), &used, standardise, Keep::Count(2))
            .map_err(|e| fail(args, format!("{name}: {e}")))?;
        warn_constant(ctx, args, &names, &constant);
        let scores = stack_scores(args, &p, &sub)?;
        for (c, layer) in scores.layers.into_iter().enumerate() {
            keys.push(Key::Name(format!("{name}_pc{}", c + 1)));
            layers.push(layer);
        }
        let mut cum = 0.0;
        for c in 0..2 {
            loadings.rows.push(vec![
                (b + 1) as f64,
                (c + 1) as f64,
                p.explained[c],
                p.components[c][0],
                p.components[c][1],
            ]);
            cum += p.explained[c];
            explained.rows.push(vec![(b + 1) as f64, (c + 1) as f64, p.variance[c], p.explained[c], cum]);
        }
    }
    loadings.labels.insert("band".into(), pr.names.clone());
    explained.labels.insert("band".into(), pr.names.clone());
    let mut out = outputs([("scores", Value::Stack(Stack { grid: pr.first.grid.clone(), keys, layers }))]);
    out.insert("loadings".into(), Value::Table(loadings));
    out.insert("explained".into(), Value::Table(explained));
    // (no model: a selective PCA is one fit per layer, which ml.predict has no model to hold)
    Ok(out)
}

impl Operator for Components {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "ml.pca",
            "The principal components of some variables (Pearson 1901, Hotelling 1933): the eigenvectors of their \
             correlation (the default: each variable by its standard deviation, for variables in different units) \
             or covariance, from one pass over the records of a table, or over the cells of a stack (a layer per \
             variable; `cells` the cells to use) that have every variable; above a million records the fit uses a \
             sample (said in a note). The scores are each record's coordinates on the components, from the mean \
             and scale of the fit: the model scores other data with ml.predict. Each component's largest loading is \
             positive, as scikit-learn's and metrics.pca's. A record with a variable none has no scores. With maps \
             of a later date (`later`, the same layers by name) it finds the change between two dates: stacked \
             (Byrne, Crapper and Mayo 1980; Fung and LeDrew 1987) is the PCA of the layers of both dates \
             (<layer>_1, <layer>_2), and the change column of the explained table tells how much each component is \
             a difference of the dates; selective (Chavez and Kwarteng 1989) is a PCA of each layer's two dates, \
             <layer>_pc1 the part that stayed and <layer>_pc2 the part that changed. A cell with a layer none at \
             either date stays out. With `method` mnf (maps, one date; Green, Berman, Switzer and Craig 1988; Lee, \
             Woodyatt and Berman 1990) the noise's covariance is taken from the differences between neighbouring \
             cells, the bands are whitened by it and the components of the whitened bands come ordered by \
             signal-to-noise ratio: `denoised` is the image rebuilt from the components kept.",
            Access::Global,
        )
        .input(PortSpec::optional("table", Type::Table, "the records: a column per variable"))
        .input(PortSpec::optional("predictors", Type::Stack, "or maps: a layer per variable (stack.of)"))
        .input(PortSpec::optional("cells", Type::Map, "with maps: the cells to fit on (non-zero; default all)"))
        .input(PortSpec::optional(
            "later",
            Type::Stack,
            "with maps: the same layers (by name) on the same grid at a later date: the two dates are analysed \
             together, see `change`",
        ))
        .output(PortSpec::optional("scores", Type::Stack, "with maps: a layer per component (pc1, pc2...)"))
        .output(PortSpec::optional("table", Type::Table, "with a table: its columns, then the scores (pc1, pc2...)"))
        .output(PortSpec::new(
            "loadings",
            Type::Table,
            "per component kept: its share of the variance (explained) and the loadings of the variables (as \
             metrics.pca's, for the circle of correlations)",
        ))
        .output(PortSpec::new(
            "explained",
            Type::Table,
            "per component, all of them: its variance, its share of the total and the cumulative share",
        ))
        .output(PortSpec::optional(
            "model",
            Type::MlModel,
            "the PCA, for ml.predict on other data (not with change selective: a fit per layer)",
        ))
        .param(PortSpec::optional(
            "components",
            Type::Number,
            "the components kept: a number (1 or more) or the share of the variance they explain at least (below 1; \
             default 0.95)",
        ))
        .param(PortSpec::optional(
            "standardise",
            Type::Bool,
            "true (default): the correlation, each variable divided by its standard deviation; false: the covariance",
        ))
        .param(PortSpec::optional(
            "columns",
            Type::Any,
            "with a table: the variables' columns (default: all the numeric ones)",
        ))
        .param(PortSpec::optional(
            "sample",
            Type::Number,
            "the records to fit on: a number (1 or more) or a share (below 1) drawn at random (default all up to a \
             million)",
        ))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of the sample's draw (numpy's; default 1)"))
        .param(
            PortSpec::optional(
                "change",
                Type::Text,
                "with later: stacked (default), one PCA of the layers of both dates (<layer>_1, <layer>_2), whose \
                 explained table has a column change (1 a pure difference of the dates, 0 a pure sum); or \
                 selective, a PCA of each layer's pair of dates, scores <layer>_pc1 (what stayed) and <layer>_pc2 \
                 (what changed), and no model",
            )
            .choices(["stacked", "selective"]),
        )
        .param(
            PortSpec::optional(
                "method",
                Type::Text,
                "pca (default), or mnf, the minimum noise fraction (Green et al. 1988): the components ordered by \
                 signal-to-noise ratio, the noise being the difference between neighbouring cells; with maps \
                 (predictors) only. Its scores are mnf1, mnf2..., the variance of the explained table is the \
                 eigenvalue (the SNR plus 1), `components` a number or a share of the eigenvalues' sum, and \
                 `standardise` does not matter (the transform does not depend on the bands' units)",
            )
            .choices(["pca", "mnf"]),
        )
        .param(
            PortSpec::optional(
                "noise",
                Type::Text,
                "with mnf: shift (default), the covariance of the difference between each cell and its right-hand \
                 neighbour, over 2 (as ENVI's and Spectral Python's); or shift_both, the mean of that and of the \
                 difference with the neighbour below",
            )
            .choices(["shift", "shift_both"]),
        )
        .output(PortSpec::optional(
            "denoised",
            Type::Stack,
            "with mnf: the layers (named as the predictors) rebuilt from the components kept, the noise left out",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let standardise: bool = args.param("standardise", true)?;
        let keep = keep_of(args)?;
        let method: String = args.param("method", "pca".to_string())?;
        match method.as_str() {
            "pca" => {
                if args.opt_param::<String>("noise")?.is_some() {
                    return Err(fail(args, "noise goes with method mnf"));
                }
            }
            "mnf" => return mnf(ctx, args, keep),
            m => return Err(fail(args, format!("method {m:?}: pca or mnf"))),
        }
        let pair = dates_of(args)?;
        if let Some(pr) = pair.as_ref().filter(|pr| pr.selective) {
            return selective(ctx, args, pr, standardise);
        }
        // two dates stacked: the layers of the first, then those of the later, a variable each
        let both = pair.as_ref().map(|pr| pr.stacked());
        let Records { names, columns, idx } = records_of(ctx, args, both.as_ref())?;
        let used: Vec<Vec<f64>> = columns.iter().map(|c| idx.iter().map(|&i| c[i]).collect()).collect();
        let (p, constant, change) = if both.is_some() {
            // (every component is fitted to tell which one is the change, then those asked for are kept)
            let (mut p, constant) = pca::fit_noting(names.clone(), &used, standardise, Keep::Count(names.len()))
                .map_err(|e| fail(args, e))?;
            let k = pca::kept(&p.explained, keep).map_err(|e| fail(args, e))?;
            let change: Vec<f64> = p.components.iter().map(|v| change_of(v)).collect();
            p.components.truncate(k);
            (p, constant, Some(change))
        } else {
            let (p, constant) = pca::fit_noting(names.clone(), &used, standardise, keep).map_err(|e| fail(args, e))?;
            (p, constant, None)
        };
        warn_constant(ctx, args, &names, &constant);
        if let Some(change) = &change {
            // the component most like a change: the largest among those with a share of the variance worth the name
            let best = (0..change.len())
                .filter(|&c| p.explained[c] >= 0.01)
                .max_by(|&a, &b| change[a].total_cmp(&change[b]).then(b.cmp(&a)));
            if let Some(c) = best {
                ctx.warn(format!(
                    "node {:?}: pc{} is the component most like a change between the dates (change {:.3}, {:.1}% of \
                     the variance; 1 is a pure difference of the dates, 0 a pure sum)",
                    args.node,
                    c + 1,
                    change[c],
                    100.0 * p.explained[c]
                ));
            }
        }
        // the tables
        let mut cols = vec!["component".to_string(), "explained".to_string()];
        cols.extend(names);
        let mut loadings = Table::new(cols);
        for (c, v) in p.components.iter().enumerate() {
            loadings.rows.push([(c + 1) as f64, p.explained[c]].into_iter().chain(v.iter().copied()).collect());
        }
        let mut head = ["component", "variance", "ratio", "cumulative"].map(String::from).to_vec();
        if change.is_some() {
            head.push("change".into());
        }
        let mut explained = Table::new(head);
        let mut cum = 0.0;
        for c in 0..p.variance.len() {
            cum += p.explained[c];
            let mut row = vec![(c + 1) as f64, p.variance[c], p.explained[c], cum];
            row.extend(change.as_ref().map(|v| v[c]));
            explained.rows.push(row);
        }
        // the scores of every record
        let mut out = match (args.get("table"), args.get("predictors")) {
            (Some(Value::Table(t)), _) => outputs([("table", Value::Table(table_scores(args, &p, t)?))]),
            _ => {
                let maps = match &both {
                    Some(b) => b,
                    None => args.stack("predictors")?,
                };
                outputs([("scores", Value::Stack(stack_scores(args, &p, maps)?))])
            }
        };
        out.insert("loadings".into(), Value::Table(loadings));
        out.insert("explained".into(), Value::Table(explained));
        out.insert("model".into(), Value::MlModel(Arc::new(Model::Pca(p))));
        Ok(out)
    }
}
