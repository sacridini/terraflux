//! `ml.som`: a self-organizing map (Kohonen 1982) of the records of a table or the cells of a stack, trained on all the
//! complete records up to a million (a sample above) by the batch algorithm or online (the MiniSom engine of
//! `multivar::som`, to the bit), its nodes optionally grouped in k groups (the two levels of Vesanto & Alhoniemi
//! 2000), every record then given its node and group, and the node of any other records from the fit's means, scales
//! and weights (`ml.predict`).

use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::pca::{Records, records, warn_constant};
use super::{Model, fail};
use crate::multivar::codebook::{
    self, Decay, Grouping, MAX_AUTO_NODES, MAX_NODES, Neighbourhood, Params, Som, Start, Training, group_nodes,
    vesanto, wanted_nodes,
};
use crate::multivar::linalg::moments;
use crate::multivar::som::Topology;
use crate::{Args, Ctx, Map, Operator, Outputs, Result, Stack, Table, Value, outputs};

/// The most training steps of an online training (a step per record and pass).
const MAX_STEPS: usize = 50_000_000;

pub struct Organise;

/// The node (and group) of the cells of a stack (the layers the model reads, by name), NaN where a variable has none
/// or `cells` has none.
fn stack_nodes(args: &Args, s: &Som, stack: &Stack, cells: Option<&Map>) -> Result<codebook::Assigned> {
    let layers: Vec<&Arc<Vec<f64>>> = s
        .predictors
        .iter()
        .map(|n| stack.by_name(n).ok_or_else(|| fail(args, format!("the stack has no layer {n:?}"))))
        .collect::<Result<_>>()?;
    let slices: Vec<&[f64]> = layers.iter().map(|l| l.as_slice()).collect();
    if slices.iter().any(|l| l.len() != stack.grid.cells()) {
        return Err(fail(args, "the layers differ in size from the grid"));
    }
    s.assign(&slices, cells.map(|m| m.data.as_slice())).map_err(|e| fail(args, e))
}

/// A table with the node of each row as a column `node` (and `group`, when the nodes are grouped) after its own
/// columns (NaN for a row with a variable none).
fn table_nodes(args: &Args, s: &Som, t: &Table) -> Result<(Table, codebook::Assigned)> {
    let columns: Vec<Vec<f64>> = s
        .predictors
        .iter()
        .map(|n| t.column(n).ok_or_else(|| fail(args, format!("the table has no column {n:?}"))))
        .collect::<Result<_>>()?;
    let slices: Vec<&[f64]> = columns.iter().map(Vec::as_slice).collect();
    let a = s.assign(&slices, None).map_err(|e| fail(args, e))?;
    let grouped = !a.groups.is_empty();
    for name in ["node", "group"].iter().take(if grouped { 2 } else { 1 }) {
        if t.columns.iter().any(|n| n == name) {
            return Err(fail(args, format!("the table already has a column {name:?}")));
        }
    }
    let mut out = t.clone();
    out.columns.push("node".into());
    if grouped {
        out.columns.push("group".into());
    }
    for (i, row) in out.rows.iter_mut().enumerate() {
        row.push(a.nodes[i]);
        if grouped {
            row.push(a.groups[i]);
        }
    }
    Ok((out, a))
}

/// What `ml.predict` gives a map model: the node (`map`) and group (`groups`, when the nodes are grouped) of the cells
/// of a stack, or the rows of a table (`table` with the columns `node` and `group`).
pub(super) fn apply(args: &Args, s: &Som) -> Result<Outputs> {
    match (args.get("predictors"), args.get("table")) {
        (Some(_), Some(_)) => Err(fail(args, "maps (predictors) or a table, not both")),
        (Some(_), None) => {
            let stack = args.stack("predictors")?;
            let a = stack_nodes(args, s, stack, None)?;
            let mut out = outputs([("map", Value::Map(Map::new(stack.grid.clone(), a.nodes)))]);
            if !a.groups.is_empty() {
                out.insert("groups".into(), Value::Map(Map::new(stack.grid.clone(), a.groups)));
            }
            Ok(out)
        }
        (None, Some(Value::Table(t))) => Ok(outputs([("table", Value::Table(table_nodes(args, s, t)?.0))])),
        (None, Some(v)) => Err(fail(args, format!("table: a table, not a {:?}", v.ty()))),
        (None, None) => Err(fail(args, "connect the maps (predictors) or a table")),
    }
}

/// A setting that is one of some words.
fn choice<T: Copy>(args: &Args, name: &str, default: &str, options: &[(&str, T)]) -> Result<T> {
    let word: String = args.param(name, default.to_string())?;
    options.iter().find(|(w, _)| *w == word).map(|(_, v)| *v).ok_or_else(|| {
        let all: Vec<&str> = options.iter().map(|(w, _)| *w).collect();
        fail(args, format!("{name} {word:?}: one of {}", all.join(", ")))
    })
}

/// A row per node: its number, row, column, records, group (when grouped), U-matrix distance, topology (1 rectangular,
/// 2 hexagonal, named by the table's labels: the viewer draws the grid by it), and its weights in the variables' own
/// units.
fn codebook_table(s: &Som, counts: &[usize]) -> Table {
    let mut cols: Vec<String> = ["node", "row", "column", "count"].map(String::from).to_vec();
    if !s.groups.is_empty() {
        cols.push("group".into());
    }
    cols.push("umatrix".into());
    cols.push("topology".into());
    cols.extend(s.predictors.iter().cloned());
    let mut t = Table::new(cols);
    t.labels.insert("topology".into(), vec!["rectangular".into(), "hexagonal".into()]);
    let topology = if s.topology == Topology::Hexagonal { 2.0 } else { 1.0 };
    let u = s.umatrix();
    for k in 0..s.len() {
        let mut row = vec![(k + 1) as f64, (k / s.columns + 1) as f64, (k % s.columns + 1) as f64, counts[k] as f64];
        if !s.groups.is_empty() {
            row.push(s.groups[k] as f64);
        }
        row.push(u[k]);
        row.push(topology);
        row.extend(s.original(k));
        t.rows.push(row);
    }
    t
}

impl Operator for Organise {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "ml.som",
            "A self-organizing map (Kohonen 1982) of some variables: a grid of nodes, each with a weight per \
             variable, trained so that records near each other fall on nodes near each other; from a table's \
             records, or the cells of a stack (a layer per variable; `cells` the cells to use) that have every \
             variable; above a million records the fit uses a sample (said in a note). The variables are divided by \
             their standard deviation first (`standardise`, the default, right for variables in different units). \
             Without `rows` and `cols` the grid has about 5 sqrt(n) nodes (Vesanto & Alhoniemi 2000), its sides in \
             the ratio of the square roots of the first two eigenvalues of the variables' covariance (said in a \
             note). `training` is batch (the default: all the records, then the weights) or online (a record at a \
             time, in random order); the MiniSom algorithm, with a rectangular or hexagonal `topology`. \
             `clusters` k above 0 groups the nodes in k groups by Ward's method (or k-means), numbered by the first \
             principal component of their centres. Each record gets its node (the row-major number, from 1) and its \
             group; the model gives other data the same with ml.predict, from the means and scales of the fit. A \
             record with a variable none has no node.",
            Access::Global,
        )
        .input(PortSpec::optional("table", Type::Table, "the records: a column per variable"))
        .input(PortSpec::optional("predictors", Type::Stack, "or maps: a layer per variable (stack.of)"))
        .input(PortSpec::optional("cells", Type::Map, "with maps: the cells to fit on (non-zero; default all)"))
        .output(PortSpec::optional("nodes", Type::Map, "with maps: the node of each cell, from 1 (row by row)"))
        .output(PortSpec::optional(
            "groups",
            Type::Map,
            "with maps and clusters: the group of each cell's node (1 to k)",
        ))
        .output(PortSpec::optional(
            "table",
            Type::Table,
            "with a table: its columns, then the node of each row (and its group, with clusters)",
        ))
        .output(PortSpec::new(
            "codebook",
            Type::Table,
            "a row per node: node, row, column, count (its records), group (with clusters), umatrix (its mean \
             distance, in the fit's units, to the nodes next to it), topology (rectangular or hexagonal) and its \
             weight for each variable, in the variable's own units",
        ))
        .output(PortSpec::new(
            "quality",
            Type::Table,
            "one row: records (those fitted), rows, columns, quantisation_error (their mean distance to their \
             node, in the fit's units: standardised unless standardise is false), topographic_error (the share \
             whose two nearest nodes are not next to each other) and empty_nodes",
        ))
        .output(PortSpec::new("model", Type::MlModel, "the weights, means and scales, for ml.predict on other data"))
        .param(PortSpec::optional("rows", Type::Integer, "the rows of nodes (with cols; default by Vesanto's rule)"))
        .param(PortSpec::optional("cols", Type::Integer, "the nodes of a row (with rows)"))
        .param(
            PortSpec::optional(
                "topology",
                Type::Text,
                "rectangular (default) or hexagonal (MiniSom's: every other row shifted half a node)",
            )
            .choices(["rectangular", "hexagonal"]),
        )
        .param(
            PortSpec::optional(
                "neighbourhood",
                Type::Text,
                "how the winner draws its neighbours in: gaussian (default), bubble or triangle",
            )
            .choices(["gaussian", "bubble", "triangle"]),
        )
        .param(PortSpec::optional(
            "learning_rate",
            Type::Number,
            "the weights' step towards the records, shrinking by `decay` (default 1 for batch, 0.5 for online)",
        ))
        .param(PortSpec::optional(
            "sigma",
            Type::Number,
            "the neighbourhood's spread, in nodes, shrinking towards 1 (default half the longer side of the map)",
        ))
        .param(
            PortSpec::optional(
                "decay",
                Type::Text,
                "how the learning rate and sigma shrink: asymptotic (default; MiniSom's), inverse or linear",
            )
            .choices(["asymptotic", "inverse", "linear"]),
        )
        .param(
            PortSpec::optional(
                "training",
                Type::Text,
                "batch (default: Kohonen's batch algorithm, on all the threads) or online (a record at a time)",
            )
            .choices(["batch", "online"]),
        )
        .param(PortSpec::optional(
            "iterations",
            Type::Integer,
            "batch: the iterations over all the records (default 50); online: the passes over them (default 5)",
        ))
        .param(
            PortSpec::optional(
                "init",
                Type::Text,
                "the weights before training: pca (default; spread on the first two principal components) or random \
                 (records drawn with the seed)",
            )
            .choices(["pca", "random"]),
        )
        .param(PortSpec::optional(
            "clusters",
            Type::Integer,
            "0 (default): no groups; k above 0: the nodes grouped in k groups",
        ))
        .param(
            PortSpec::optional("grouping", Type::Text, "with clusters: ward (default) or kmeans")
                .choices(["ward", "kmeans"]),
        )
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
            "the seed of the sample's draw, of the random start and of the online order (default 1)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let standardise: bool = args.param("standardise", true)?;
        let topology = choice(
            args,
            "topology",
            "rectangular",
            &[("rectangular", Topology::Rectangular), ("hexagonal", Topology::Hexagonal)],
        )?;
        let neighbourhood = choice(
            args,
            "neighbourhood",
            "gaussian",
            &[
                ("gaussian", Neighbourhood::Gaussian),
                ("bubble", Neighbourhood::Bubble),
                ("triangle", Neighbourhood::Triangle),
            ],
        )?;
        let decay = choice(
            args,
            "decay",
            "asymptotic",
            &[("asymptotic", Decay::Asymptotic), ("inverse", Decay::Inverse), ("linear", Decay::Linear)],
        )?;
        let training = choice(args, "training", "batch", &[("batch", Training::Batch), ("online", Training::Online)])?;
        let start = choice(args, "init", "pca", &[("pca", Start::Pca), ("random", Start::Random)])?;
        let grouping = choice(args, "grouping", "ward", &[("ward", Grouping::Ward), ("kmeans", Grouping::KMeans)])?;
        let default_iterations = if training == Training::Batch { 50 } else { 5 };
        let iterations: usize = args.param("iterations", default_iterations)?;
        if iterations == 0 {
            return Err(fail(args, "iterations 0: 1 or more"));
        }
        let learning_rate: f64 = args.param("learning_rate", if training == Training::Batch { 1.0 } else { 0.5 })?;
        if !learning_rate.is_finite() || learning_rate <= 0.0 || learning_rate > 1.0 {
            return Err(fail(args, format!("learning_rate {learning_rate}: above 0, 1 at most")));
        }
        let sigma_asked: Option<f64> = args.opt_param("sigma")?;
        if let Some(s) = sigma_asked.filter(|s| !s.is_finite() || *s <= 0.0) {
            return Err(fail(args, format!("sigma {s}: above 0")));
        }
        let clusters: usize = args.param("clusters", 0)?;
        let (rows_asked, cols_asked): (Option<usize>, Option<usize>) =
            (args.opt_param("rows")?, args.opt_param("cols")?);
        if rows_asked.is_some() != cols_asked.is_some() {
            return Err(fail(args, "rows and cols go together: give both, or neither for the size by Vesanto's rule"));
        }
        if rows_asked == Some(0) || cols_asked == Some(0) {
            return Err(fail(args, "rows and cols: 1 or more"));
        }
        if let Some((r, c)) = rows_asked.zip(cols_asked).filter(|(r, c)| r.saturating_mul(*c) > MAX_NODES) {
            return Err(fail(args, format!("{r} x {c} nodes: at most {MAX_NODES}")));
        }
        let seed: u64 = args.param("seed", 1)?;
        let Records { names, columns, idx } = records(ctx, args)?;
        let (n, d) = (idx.len(), names.len());
        // the records of the fit, in the fit's units: the mean taken and, standardised, divided by the deviation
        let used: Vec<Vec<f64>> = columns.iter().map(|c| idx.iter().map(|&i| c[i]).collect()).collect();
        let slices: Vec<&[f64]> = used.iter().map(Vec::as_slice).collect();
        let m = moments(&slices, 0).ok_or_else(|| fail(args, "no record has every variable"))?;
        // the covariance of the fit's units: the correlation when standardised
        let (scale, cov) = if standardise {
            let (scale, corr, constant) = m.standardising();
            warn_constant(ctx, args, &names, &constant);
            (scale, corr)
        } else {
            (vec![1.0; d], m.cov.clone())
        };
        let mean = m.mean;
        let mut x = vec![0.0; n * d];
        x.par_chunks_mut(d).enumerate().for_each(|(i, row)| {
            for j in 0..d {
                row[j] = (used[j][i] - mean[j]) / scale[j];
            }
        });
        // the grid
        let (rows, cols) = match (rows_asked, cols_asked) {
            (Some(r), Some(c)) => (r, c),
            _ => {
                let (r, c) = vesanto(n, &cov, d);
                let wanted = ((5.0 * (n as f64).sqrt()).ceil() as usize).max(4);
                ctx.warn(format!(
                    "node {:?}: a map of {r} x {c} = {} nodes by Vesanto's rule (5 sqrt(n) = {wanted} nodes for the {n} \
                     records{}; the sides in the ratio of the first two eigenvalues of the {}); the settings rows and \
                     cols give another size",
                    args.node,
                    r * c,
                    if wanted > wanted_nodes(n) { format!(", at most {MAX_AUTO_NODES}") } else { String::new() },
                    if standardise { "correlation" } else { "covariance" },
                ));
                (r, c)
            }
        };
        if training == Training::Online && n.saturating_mul(iterations) > MAX_STEPS {
            return Err(fail(
                args,
                format!(
                    "online training of {n} records for {iterations} passes: more than {MAX_STEPS} steps; use \
                     batch, or a sample"
                ),
            ));
        }
        if clusters > rows * cols {
            return Err(fail(args, format!("clusters {clusters}: more than the {} nodes", rows * cols)));
        }
        // sigma: half the longer side, whole for the bubble and triangle (which count nodes)
        let sigma = sigma_asked.unwrap_or_else(|| {
            let half = (rows.max(cols) as f64 / 2.0).max(1.0);
            if neighbourhood == Neighbourhood::Gaussian { half } else { half.ceil() }
        });
        let p = Params {
            rows,
            columns: cols,
            topology,
            neighbourhood,
            learning_rate,
            sigma,
            decay,
            training,
            iterations,
            start,
            seed,
        };
        if start == Start::Pca && d < 2 {
            ctx.warn(format!(
                "node {:?}: init pca needs two variables or more; the weights start from random records",
                args.node
            ));
        }
        let weights = codebook::fit(&x, n, d, &cov, &p);
        let mut model = Som { predictors: names, rows, columns: cols, topology, mean, scale, weights, groups: vec![] };
        if clusters > 0 {
            model.groups = group_nodes(&model.weights, clusters, grouping, seed);
        }
        let (qe, te) = model.quality(&x);
        // every record's node
        let (mut out, counts) = match args.get("table") {
            Some(Value::Table(t)) => {
                let (table, a) = table_nodes(args, &model, t)?;
                (outputs([("table", Value::Table(table))]), a.counts)
            }
            _ => {
                let stack = args.stack("predictors")?;
                let a = stack_nodes(args, &model, stack, args.opt_map("cells")?)?;
                let mut out = outputs([("nodes", Value::Map(Map::new(stack.grid.clone(), a.nodes)))]);
                if !a.groups.is_empty() {
                    out.insert("groups".into(), Value::Map(Map::new(stack.grid.clone(), a.groups)));
                }
                (out, a.counts)
            }
        };
        out.insert("codebook".into(), Value::Table(codebook_table(&model, &counts)));
        let mut quality = Table::new(
            ["records", "rows", "columns", "quantisation_error", "topographic_error", "empty_nodes"]
                .map(String::from)
                .to_vec(),
        );
        let empty = counts.iter().filter(|&&c| c == 0).count();
        quality.rows.push(vec![n as f64, rows as f64, cols as f64, qe, te, empty as f64]);
        out.insert("quality".into(), Value::Table(quality));
        out.insert("model".into(), Value::MlModel(Arc::new(Model::Som(model))));
        Ok(out)
    }
}
