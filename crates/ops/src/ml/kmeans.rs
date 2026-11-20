//! `ml.kmeans`: the clusters of the records of a table or the cells of a stack, by k-means from scikit-learn's greedy
//! k-means++ start (the Janus k-means of `multivar::kmeans`), k given or chosen by the best silhouette; fitted to all
//! the complete records up to a million (a sample above), every record then given its cluster, the clusters numbered
//! by the first principal component of their centres, and the cluster of any other records from the fit's means,
//! scales and centres (`ml.predict`).

use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::pca::{Records, records, warn_constant};
use super::{Model, fail};
use crate::multivar::clusters::{self, Clusters};
use crate::multivar::kmeans::{Init, Settings, kmeans_with};
use crate::multivar::linalg::moments;
use crate::signatures::NumpyRng;
use crate::{Args, Ctx, Map, Operator, Outputs, Result, Stack, Table, Value, outputs};

/// The records each k is fitted on at most when several are tried (the one chosen is then fitted on all).
const CHOICE_RECORDS: usize = 100_000;

/// The records the silhouette is computed on at most (it is quadratic in them).
const SILHOUETTE_RECORDS: usize = 10_000;

/// The k tried when it is not given: 2 to this.
const AUTO_MAX: usize = 10;

pub struct Groups;

/// The cluster of the cells of a stack (the layers the model reads, by name), NaN where a variable has none or `cells`
/// has none.
fn stack_clusters(args: &Args, c: &Clusters, stack: &Stack, cells: Option<&Map>) -> Result<clusters::Assigned> {
    let layers: Vec<&Arc<Vec<f64>>> = c
        .predictors
        .iter()
        .map(|n| stack.by_name(n).ok_or_else(|| fail(args, format!("the stack has no layer {n:?}"))))
        .collect::<Result<_>>()?;
    let slices: Vec<&[f64]> = layers.iter().map(|l| l.as_slice()).collect();
    if slices.iter().any(|l| l.len() != stack.grid.cells()) {
        return Err(fail(args, "the layers differ in size from the grid"));
    }
    c.assign(&slices, cells.map(|m| m.data.as_slice())).map_err(|e| fail(args, e))
}

/// A table with the cluster of each row as a column `cluster` after its own columns (NaN for a row with a variable
/// none), and the sizes and inertia of the clusters.
fn table_clusters(args: &Args, c: &Clusters, t: &Table) -> Result<(Table, clusters::Assigned)> {
    let columns: Vec<Vec<f64>> = c
        .predictors
        .iter()
        .map(|n| t.column(n).ok_or_else(|| fail(args, format!("the table has no column {n:?}"))))
        .collect::<Result<_>>()?;
    let slices: Vec<&[f64]> = columns.iter().map(Vec::as_slice).collect();
    let a = c.assign(&slices, None).map_err(|e| fail(args, e))?;
    if t.columns.iter().any(|n| n == "cluster") {
        return Err(fail(args, "the table already has a column \"cluster\""));
    }
    let mut out = t.clone();
    out.columns.push("cluster".into());
    for (row, l) in out.rows.iter_mut().zip(&a.labels) {
        row.push(*l);
    }
    Ok((out, a))
}

/// What `ml.predict` gives a k-means model: the cluster of the cells of a stack (`map`) or the rows of a table (`table`
/// with a column `cluster`).
pub(super) fn apply(args: &Args, c: &Clusters) -> Result<Outputs> {
    match (args.get("predictors"), args.get("table")) {
        (Some(_), Some(_)) => Err(fail(args, "maps (predictors) or a table, not both")),
        (Some(_), None) => {
            let stack = args.stack("predictors")?;
            let a = stack_clusters(args, c, stack, None)?;
            Ok(outputs([("map", Value::Map(Map::new(stack.grid.clone(), a.labels)))]))
        }
        (None, Some(Value::Table(t))) => Ok(outputs([("table", Value::Table(table_clusters(args, c, t)?.0))])),
        (None, Some(v)) => Err(fail(args, format!("table: a table, not a {:?}", v.ty()))),
        (None, None) => Err(fail(args, "connect the maps (predictors) or a table")),
    }
}

/// The k asked for: one, a list to choose among, or none (chosen from 2 to `AUTO_MAX`).
enum Ks {
    Auto,
    List(Vec<usize>),
}

/// The setting `k`: a whole number (1 or more), a list of whole numbers (2 or more) or "auto" (the default).
fn ks_of(args: &Args) -> Result<Ks> {
    let bad = |what: String| {
        fail(args, format!("k {what}: a whole number (1 or more), a list of them (2 or more) or \"auto\""))
    };
    let whole =
        |v: &serde_json::Value| v.as_f64().filter(|x| x.fract() == 0.0 && *x >= 1.0 && *x < 1e9).map(|x| x as usize);
    match args.params.get("k") {
        None => Ok(Ks::Auto),
        Some(serde_json::Value::String(s)) if s == "auto" => Ok(Ks::Auto),
        Some(v @ serde_json::Value::Number(_)) => Ok(Ks::List(vec![whole(v).ok_or_else(|| bad(v.to_string()))?])),
        Some(serde_json::Value::Array(a)) if !a.is_empty() => {
            let mut ks = Vec::new();
            for v in a {
                match whole(v) {
                    Some(k) if k >= 2 => ks.push(k),
                    _ => return Err(bad(v.to_string())),
                }
            }
            ks.sort_unstable();
            ks.dedup();
            Ok(Ks::List(ks))
        }
        Some(v) => Err(bad(v.to_string())),
    }
}

impl Operator for Groups {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "ml.kmeans",
            "The clusters of some variables by k-means (Lloyd 1982) from the greedy k-means++ start of scikit-learn \
             (Arthur & Vassilvitskii 2007), the best of `n_init` runs: from a table's records, or the cells of a \
             stack (a layer per variable; `cells` the cells to use) that have every variable; above a million \
             records the fit uses a sample (said in a note). Each variable is divided by its standard deviation \
             first (`standardise`, the default, right for variables in different units). `k` is a number, a list \
             (each tried: inertia, silhouette on a sample of at most 10 000 records and Calinski-Harabasz index, \
             the best silhouette kept) or \"auto\" (2 to 10, the default). The clusters are numbered from 1 by \
             the first principal component of their centres, the same numbers from one run to the next. The model \
             gives other data the nearest centre with ml.predict, from the means and scales of the fit. A record \
             with a variable none has no cluster.",
            Access::Global,
        )
        .input(PortSpec::optional("table", Type::Table, "the records: a column per variable"))
        .input(PortSpec::optional("predictors", Type::Stack, "or maps: a layer per variable (stack.of)"))
        .input(PortSpec::optional("cells", Type::Map, "with maps: the cells to fit on (non-zero; default all)"))
        .output(PortSpec::optional("clusters", Type::Map, "with maps: the cluster of each cell (1 to k)"))
        .output(PortSpec::optional("table", Type::Table, "with a table: its columns, then the cluster of each row"))
        .output(PortSpec::new(
            "centres",
            Type::Table,
            "a row per cluster: cluster, size (its records), inertia (their squared distance to the centre, in the \
             fit's units: standardised unless standardise is false) and the centre's value of each variable, in \
             the variable's own units",
        ))
        .output(PortSpec::new(
            "choice",
            Type::Table,
            "a row per k tried: k, records (those it was fitted on), inertia, silhouette and calinski_harabasz",
        ))
        .output(PortSpec::new("model", Type::MlModel, "the centres, means and scales, for ml.predict on other data"))
        .param(PortSpec::optional(
            "k",
            Type::Any,
            "the clusters: a number, a list of numbers to choose among by the best silhouette, or \"auto\" (2 to 10; \
             default)",
        ))
        .param(PortSpec::optional(
            "n_init",
            Type::Integer,
            "runs from different starts, the least inertia kept (default 10)",
        ))
        .param(PortSpec::optional("max_iter", Type::Integer, "iterations of a run at most (default 300)"))
        .param(PortSpec::optional(
            "tol",
            Type::Number,
            "a run stops when its centres move, in squared distance, less than this times the variables' mean \
             variance (default 1e-4)",
        ))
        .param(PortSpec::optional(
            "standardise",
            Type::Bool,
            "true (default): each variable divided by its standard deviation; false: in its own units",
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
        .param(PortSpec::optional(
            "seed",
            Type::Integer,
            "the seed of the sample's draw and of the starts (default 1)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let standardise: bool = args.param("standardise", true)?;
        let n_init: usize = args.param("n_init", 10)?;
        let max_iter: usize = args.param("max_iter", 300)?;
        let tol: f64 = args.param("tol", 1e-4)?;
        if n_init == 0 {
            return Err(fail(args, "n_init 0: 1 or more"));
        }
        if max_iter == 0 {
            return Err(fail(args, "max_iter 0: 1 or more"));
        }
        if !tol.is_finite() || tol < 0.0 {
            return Err(fail(args, format!("tol {tol}: 0 or more")));
        }
        let ks_asked = ks_of(args)?;
        let seed: u64 = args.param("seed", 1)?;
        let Records { names, columns, idx } = records(ctx, args)?;
        let (n, d) = (idx.len(), names.len());
        let ks: Vec<usize> = match ks_asked {
            Ks::Auto if n < 3 => {
                return Err(fail(args, format!("k auto: tries 2 or more, and {n} record(s) are too few")));
            }
            Ks::Auto => (2..=AUTO_MAX.min(n - 1)).collect(),
            Ks::List(ks) => ks,
        };
        let chosen_by_silhouette = ks.len() > 1;
        // the records of the fit, in the fit's units: the mean taken and, standardised, divided by the deviation
        let used: Vec<Vec<f64>> = columns.iter().map(|c| idx.iter().map(|&i| c[i]).collect()).collect();
        let slices: Vec<&[f64]> = used.iter().map(Vec::as_slice).collect();
        let m = moments(&slices, 0).ok_or_else(|| fail(args, "no record has every variable"))?;
        let scale = if standardise {
            let (scale, _, constant) = m.standardising();
            warn_constant(ctx, args, &names, &constant);
            scale
        } else {
            vec![1.0; d]
        };
        let mean = m.mean;
        let mut x = vec![0.0; n * d];
        x.par_chunks_mut(d).enumerate().for_each(|(i, row)| {
            for j in 0..d {
                row[j] = (used[j][i] - mean[j]) / scale[j];
            }
        });
        // with several k, each is fitted on at most `CHOICE_RECORDS` records (a draw, sorted), the one chosen on all
        let sub: Option<Vec<usize>> = (chosen_by_silhouette && n > CHOICE_RECORDS).then(|| {
            let mut pick = NumpyRng::new(seed).choice_distinct(n, CHOICE_RECORDS);
            pick.sort_unstable();
            pick
        });
        let xc_own: Vec<f64>;
        let xc: &[f64] = match &sub {
            Some(pick) => {
                xc_own = pick.iter().flat_map(|&i| x[i * d..(i + 1) * d].iter().copied()).collect();
                &xc_own
            }
            None => &x,
        };
        let nc = xc.len() / d;
        if let Some(&big) = ks.last().filter(|&&k| k > nc) {
            return Err(fail(args, format!("k {big}: more than the {nc} records the fit has")));
        }
        // the silhouette's records, the same for every k
        let sample: Vec<usize> = if nc > SILHOUETTE_RECORDS {
            let mut pick = NumpyRng::new(seed.wrapping_add(1)).choice_distinct(nc, SILHOUETTE_RECORDS);
            pick.sort_unstable();
            pick
        } else {
            (0..nc).collect()
        };
        let set = Settings { starts: n_init, max_iter, tol };
        let mut fits = Vec::new();
        let mut choice =
            Table::new(["k", "records", "inertia", "silhouette", "calinski_harabasz"].map(String::from).to_vec());
        for &k in &ks {
            let km = kmeans_with(xc, nc, d, k, seed, Init::Greedy, set);
            let sil = clusters::silhouette(xc, d, &km.labels, k, &sample);
            let ch = clusters::calinski_harabasz(xc, d, &km.labels, k);
            choice.rows.push(vec![k as f64, nc as f64, km.inertia, sil, ch]);
            fits.push((km, sil));
        }
        // the best silhouette (the fewest clusters on a tie; the first k when none is a number)
        let mut best = 0;
        for (i, (_, s)) in fits.iter().enumerate() {
            if s.is_finite() && (!fits[best].1.is_finite() || *s > fits[best].1) {
                best = i;
            }
        }
        let k = ks[best];
        if chosen_by_silhouette {
            ctx.warn(format!(
                "node {:?}: k = {k} has the best silhouette of {} values of k, {} to {} (the table choice){}",
                args.node,
                ks.len(),
                ks[0],
                ks[ks.len() - 1],
                if sub.is_some() { "; each tried on a sample of 100000 records, the one chosen on all" } else { "" }
            ));
        }
        let km =
            if sub.is_some() { kmeans_with(&x, n, d, k, seed, Init::Greedy, set) } else { fits.swap_remove(best).0 };
        // the centres numbered by their first principal component, and every record's cluster
        let rows: Vec<Vec<f64>> = km.centres.chunks_exact(d).map(<[f64]>::to_vec).collect();
        let model = Clusters {
            predictors: names.clone(),
            mean,
            scale,
            centres: clusters::order(&rows).into_iter().map(|i| rows[i].clone()).collect(),
        };
        let mut out = match args.get("table") {
            Some(Value::Table(t)) => {
                let (table, a) = table_clusters(args, &model, t)?;
                let mut out = outputs([("table", Value::Table(table))]);
                out.insert("centres".into(), Value::Table(centres_table(&model, &a)));
                out
            }
            _ => {
                let stack = args.stack("predictors")?;
                let a = stack_clusters(args, &model, stack, args.opt_map("cells")?)?;
                let table = centres_table(&model, &a);
                let mut out = outputs([("clusters", Value::Map(Map::new(stack.grid.clone(), a.labels)))]);
                out.insert("centres".into(), Value::Table(table));
                out
            }
        };
        out.insert("choice".into(), Value::Table(choice));
        out.insert("model".into(), Value::MlModel(Arc::new(Model::KMeans(model))));
        Ok(out)
    }
}

/// A row per cluster: its number, its records, its inertia, its centre in the variables' own units.
fn centres_table(model: &Clusters, a: &clusters::Assigned) -> Table {
    let mut cols = vec!["cluster".to_string(), "size".to_string(), "inertia".to_string()];
    cols.extend(model.predictors.iter().cloned());
    let mut t = Table::new(cols);
    for c in 0..model.k() {
        t.rows.push([(c + 1) as f64, a.sizes[c] as f64, a.inertia[c]].into_iter().chain(model.original(c)).collect());
    }
    t
}
