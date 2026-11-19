//! `ml.pca`: the principal components of the variables of a table or of the layers of a stack, fitted to all the
//! complete records up to a million (a sample above), and the scores of any other records from the fit's means and
//! scales (`ml.predict`).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::{Model, fail, sample_size};
use crate::multivar::pca::{self, Keep, Pca};
use crate::signatures::NumpyRng;
use crate::value::Key;
use crate::{Args, Ctx, Operator, Outputs, Result, Stack, Table, Value, outputs};

/// The records the fit uses at most when no `sample` is given.
const FIT_RECORDS: usize = 1_000_000;

pub struct Components;

/// The names of the scores: pc1, pc2...
fn pc_names(k: usize) -> Vec<String> {
    (1..=k).map(|i| format!("pc{i}")).collect()
}

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
        keys: pc_names(p.k()).into_iter().map(Key::Name).collect(),
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
    let names = pc_names(p.k());
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
            let stack = args.stack("predictors")?;
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
             positive, as scikit-learn's and metrics.pca's. A record with a variable none has no scores.",
            Access::Global,
        )
        .input(PortSpec::optional("table", Type::Table, "the records: a column per variable"))
        .input(PortSpec::optional("predictors", Type::Stack, "or maps: a layer per variable (stack.of)"))
        .input(PortSpec::optional("cells", Type::Map, "with maps: the cells to fit on (non-zero; default all)"))
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
        .output(PortSpec::new("model", Type::MlModel, "the PCA, for ml.predict on other data"))
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
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let standardise: bool = args.param("standardise", true)?;
        let keep = keep_of(args)?;
        let Records { names, columns, idx } = records(ctx, args)?;
        let used: Vec<Vec<f64>> = columns.iter().map(|c| idx.iter().map(|&i| c[i]).collect()).collect();
        let p = pca::fit(names.clone(), &used, standardise, keep).map_err(|e| fail(args, e))?;
        // the tables
        let mut cols = vec!["component".to_string(), "explained".to_string()];
        cols.extend(names);
        let mut loadings = Table::new(cols);
        for (c, v) in p.components.iter().enumerate() {
            loadings.rows.push([(c + 1) as f64, p.explained[c]].into_iter().chain(v.iter().copied()).collect());
        }
        let mut explained = Table::new(["component", "variance", "ratio", "cumulative"].map(String::from).to_vec());
        let mut cum = 0.0;
        for c in 0..p.variance.len() {
            cum += p.explained[c];
            explained.rows.push(vec![(c + 1) as f64, p.variance[c], p.explained[c], cum]);
        }
        // the scores of every record
        let mut out = match (args.get("table"), args.get("predictors")) {
            (Some(Value::Table(t)), _) => outputs([("table", Value::Table(table_scores(args, &p, t)?))]),
            _ => outputs([("scores", Value::Stack(stack_scores(args, &p, args.stack("predictors")?)?))]),
        };
        out.insert("loadings".into(), Value::Table(loadings));
        out.insert("explained".into(), Value::Table(explained));
        out.insert("model".into(), Value::MlModel(Arc::new(Model::Pca(p))));
        Ok(out)
    }
}
