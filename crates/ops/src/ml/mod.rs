//! Machine learning on tables and maps: scikit-learn's random forests and histogram gradient boosting ported to the
//! bit (`forest`, `boosting`), fitted to the records of a table or the cells of maps (`ml.fit`), predicted on others
//! (`ml.predict`), and the potential of each class from a model per class as landschaft's Estimator gives it
//! (`ml.potential`): where each class is, or where it appeared between two maps (PLUS's land expansion analysis,
//! LEAS), for the allocations.

pub mod boosting;
pub mod forest;
pub mod inspect;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::sdm::extrapolation::Frame;
use crate::signatures::NumpyRng;
use crate::value::{Key, class_label};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};
use boosting::Boosting;
use forest::Forest;

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Fit));
    r.add(Arc::new(Predict));
    r.add(Arc::new(Potential));
    inspect::register(r);
}

/// A fitted model: one forest or boosting of classes or values (`ml.fit`), or one per class, each of the class
/// against the other cells (`ml.potential`: `target` presence or expansion; its forests or its boostings).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Model {
    Forest(Forest),
    Boosting(Boosting),
    PerClass {
        target: String,
        classes: Vec<f64>,
        #[serde(default)]
        forests: Vec<Forest>,
        #[serde(default)]
        boostings: Vec<Boosting>,
    },
}

/// One fitted model of classes or values.
enum Fitted {
    Forest(Forest),
    Boosting(Boosting),
}

impl Fitted {
    fn importances(&self) -> Vec<f64> {
        match self {
            Fitted::Forest(f) => f.importances(),
            Fitted::Boosting(b) => b.importances(),
        }
    }

    fn predictors(&self) -> &[String] {
        match self {
            Fitted::Forest(f) => &f.predictors,
            Fitted::Boosting(b) => &b.predictors,
        }
    }
}

impl Model {
    /// The predictors it takes, in its order.
    pub fn predictors(&self) -> &[String] {
        match self {
            Model::Forest(f) => &f.predictors,
            Model::Boosting(b) => &b.predictors,
            Model::PerClass { forests, boostings, .. } => {
                forests.first().map_or_else(|| &boostings[0].predictors, |f| &f.predictors)
            }
        }
    }

    /// The classes it gives a probability of (none for a regression).
    pub fn classes(&self) -> &[f64] {
        match self {
            Model::Forest(f) => &f.classes,
            Model::Boosting(b) => &b.classes,
            Model::PerClass { classes, .. } => classes,
        }
    }

    /// Its outputs for the rows: a column per class (its probability, or its potential), or one of values.
    pub fn outputs(&self, x: &Frame) -> std::result::Result<Vec<Vec<f64>>, String> {
        match self {
            Model::Forest(f) => f.outputs(x),
            Model::Boosting(b) => b.outputs(x),
            Model::PerClass { forests, boostings, .. } => {
                if forests.is_empty() {
                    boostings.iter().map(|b| b.outputs(x).map(|mut p| p.swap_remove(1))).collect()
                } else {
                    forests.iter().map(|f| f.predict(x)).collect()
                }
            }
        }
    }
}

fn fail(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

// -- the settings of a model ---------------------------------------------------------------------------------------

fn forest_settings(sig: OpSignature) -> OpSignature {
    sig.param(
        PortSpec::optional(
            "method",
            Type::Text,
            "random_forest (default; scikit-learn's RandomForest) or gradient_boosting (its HistGradientBoosting, \
             without early stopping)",
        )
        .choices(["random_forest", "gradient_boosting"]),
    )
    .param(PortSpec::optional("n_estimators", Type::Integer, "random_forest: the trees (default 100)"))
    .param(PortSpec::optional("max_depth", Type::Integer, "the trees' depth at most (default none)"))
    .param(PortSpec::optional(
        "min_samples_split",
        Type::Integer,
        "random_forest: the records a node needs to split (default 2)",
    ))
    .param(PortSpec::optional(
        "min_samples_leaf",
        Type::Integer,
        "the records of a leaf at least (default: 1 for a forest, 20 for boosting)",
    ))
    .param(PortSpec::optional(
        "max_features",
        Type::Any,
        "the predictors tried at each split: sqrt, log2, all, a number or a share (default: sqrt for classes, \
         all for values, as scikit-learn); gradient_boosting: a share (default 1)",
    ))
    .param(PortSpec::optional("bootstrap", Type::Bool, "random_forest: each tree on a bootstrap (default true)"))
    .param(PortSpec::optional("learning_rate", Type::Number, "gradient_boosting: the shrinkage of the leaves (0.1)"))
    .param(PortSpec::optional("max_iter", Type::Integer, "gradient_boosting: the iterations (100)"))
    .param(PortSpec::optional(
        "max_leaf_nodes",
        Type::Integer,
        "gradient_boosting: the leaves of a tree at most (31; null for no limit)",
    ))
    .param(PortSpec::optional("l2_regularization", Type::Number, "gradient_boosting: the leaves' L2 penalty (0)"))
    .param(PortSpec::optional("max_bins", Type::Integer, "gradient_boosting: the bins of a predictor, 2-255 (255)"))
    .param(PortSpec::optional("random_state", Type::Integer, "scikit-learn's seed of the model (default 0)"))
}

fn boosting_settings_of(args: &Args) -> Result<boosting::Settings> {
    let d = boosting::Settings::default();
    Ok(boosting::Settings {
        learning_rate: args.param("learning_rate", d.learning_rate)?,
        max_iter: args.param("max_iter", d.max_iter)?,
        max_leaf_nodes: args.param("max_leaf_nodes", d.max_leaf_nodes)?,
        max_depth: args.opt_param("max_depth")?,
        min_samples_leaf: args.param("min_samples_leaf", d.min_samples_leaf)?,
        l2_regularization: args.param("l2_regularization", d.l2_regularization)?,
        max_features: args.param("max_features", d.max_features)?,
        max_bins: args.param("max_bins", d.max_bins)?,
        random_state: args.param("random_state", d.random_state)?,
    })
}

/// A model of the node's method fitted to the records: of classes, or of values (`regression`).
fn fit_model(args: &Args, x: &Frame, y: &[f64], regression: bool) -> Result<Fitted> {
    let method: String = args.param("method", "random_forest".to_string())?;
    let fitted = match method.as_str() {
        "random_forest" => {
            let s = settings_of(args, regression)?;
            if regression { forest::regressor(x, y, None, &s) } else { forest::classifier(x, y, None, &s) }
                .map(Fitted::Forest)
        }
        "gradient_boosting" => {
            let s = boosting_settings_of(args)?;
            if regression { boosting::regressor(x, y, &s) } else { boosting::classifier(x, y, &s) }
                .map(Fitted::Boosting)
        }
        m => return Err(fail(args, format!("method: random_forest or gradient_boosting, not {m:?}"))),
    };
    fitted.map_err(|e| fail(args, e))
}

fn settings_of(args: &Args, regression: bool) -> Result<forest::Settings> {
    let d = forest::Settings::default();
    let all = serde_json::json!(1.0);
    Ok(forest::Settings {
        n_estimators: args.param("n_estimators", d.n_estimators)?,
        max_depth: args.opt_param("max_depth")?,
        min_samples_split: args.param("min_samples_split", d.min_samples_split)?,
        min_samples_leaf: args.param("min_samples_leaf", d.min_samples_leaf)?,
        max_features: args.params.get("max_features").cloned().unwrap_or(if regression { all } else { d.max_features }),
        bootstrap: args.param("bootstrap", d.bootstrap)?,
        random_state: args.param("random_state", 0)?,
    })
}

// -- the records ----------------------------------------------------------------------------------------------------

/// How many of `n` records a sample keeps: a number (1 or more) or a share (below 1), as landschaft's Estimator.
fn sample_size(sample: f64, n: usize) -> usize {
    if sample >= 1.0 { sample as usize } else { (sample * n as f64).round_ties_even() as usize }
}

/// landschaft's Estimator._draw: the records `idx` (of labels `y`, 0/1) kept by a sample (stratified: from each label
/// apart, in their proportions, one at least), then as many absences as presences (`balance`); numpy's
/// `default_rng.choice` without replacement, sorted.
fn draw(
    idx: &[usize],
    y: &[bool],
    sample: Option<f64>,
    stratify: bool,
    balance: bool,
    rng: &mut NumpyRng,
) -> Vec<usize> {
    let mut idx = idx.to_vec();
    let mut y = y.to_vec();
    match sample {
        Some(s) if stratify => {
            let frac = if s >= 1.0 { (s as usize) as f64 / idx.len() as f64 } else { s }.min(1.0);
            let mut keep = Vec::new();
            for v in [false, true] {
                let group: Vec<usize> = (0..y.len()).filter(|&k| y[k] == v).collect();
                let m = group.len().min(((frac * group.len() as f64).round_ties_even() as usize).max(1));
                keep.extend(rng.choice_distinct(group.len(), m).into_iter().map(|j| group[j]));
            }
            keep.sort_unstable();
            y = keep.iter().map(|&k| y[k]).collect();
            idx = keep.iter().map(|&k| idx[k]).collect();
        }
        Some(s) => {
            let n = sample_size(s, idx.len());
            if n < idx.len() {
                let mut pick = rng.choice_distinct(idx.len(), n);
                pick.sort_unstable();
                y = pick.iter().map(|&k| y[k]).collect();
                idx = pick.iter().map(|&k| idx[k]).collect();
            }
        }
        None => {}
    }
    if balance {
        let pos: Vec<usize> = (0..y.len()).filter(|&k| y[k]).collect();
        let neg: Vec<usize> = (0..y.len()).filter(|&k| !y[k]).collect();
        let m = pos.len().min(neg.len());
        let mut keep: Vec<usize> = rng.choice_distinct(pos.len(), m).into_iter().map(|j| pos[j]).collect();
        keep.extend(rng.choice_distinct(neg.len(), m).into_iter().map(|j| neg[j]));
        keep.sort_unstable();
        idx = keep.iter().map(|&k| idx[k]).collect();
    }
    idx
}

/// Maps by name: the names and the layers.
type Named = (Vec<String>, Vec<Arc<Vec<f64>>>);

/// The layers of a stack keyed by name, by name (increasing, as landschaft sorts the drivers).
fn named_layers(stack: &Stack) -> Result<Named> {
    let mut named: Vec<(String, Arc<Vec<f64>>)> = stack
        .keys
        .iter()
        .zip(&stack.layers)
        .filter_map(|(k, l)| match k {
            Key::Name(n) => Some((n.clone(), l.clone())),
            _ => None,
        })
        .collect();
    if named.is_empty() {
        return Err(Error::msg("the predictors' stack has no layer named by a variable"));
    }
    named.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(named.into_iter().unzip())
}

/// The rows `idx` of maps as a frame.
fn frame_of(names: &[String], layers: &[Arc<Vec<f64>>], idx: &[usize]) -> Frame {
    Frame { names: names.to_vec(), columns: layers.iter().map(|l| idx.iter().map(|&i| l[i]).collect()).collect() }
}

/// The distinct values of the cells with one, increasing.
fn distinct(values: impl Iterator<Item = f64>) -> Vec<f64> {
    let mut v: Vec<f64> = values.filter(|v| !v.is_nan()).collect();
    v.sort_by(f64::total_cmp);
    v.dedup();
    v
}

/// Whether a target of these values is a set of classes: whole numbers, at most 50 of them.
fn looks_like_classes(values: &[f64]) -> bool {
    let d = distinct(values.iter().copied());
    d.len() <= 50 && d.iter().all(|v| v.fract() == 0.0)
}

/// A table of the importance of each predictor: a row, a column per predictor (or a row per class after the
/// column class).
fn importance_table(predictors: &[String], rows: &[(Option<f64>, Vec<f64>)]) -> Table {
    let by_class = rows.iter().any(|(c, _)| c.is_some());
    let mut t =
        Table::new(by_class.then(|| "class".to_string()).into_iter().chain(predictors.iter().cloned()).collect());
    for (c, v) in rows {
        t.rows.push(c.iter().copied().chain(v.iter().copied()).collect());
    }
    t
}

// -- ml.fit -------------------------------------------------------------------------------------------------------

struct Fit;

impl Operator for Fit {
    fn signature(&self) -> OpSignature {
        forest_settings(
            OpSignature::new(
                "ml.fit",
                "A random forest or a histogram gradient boosting fitted to records (scikit-learn's \
                 RandomForestClassifier and HistGradientBoostingClassifier for classes, their regressors for values, \
                 the same trees): the rows of a table, or the cells of maps (the predictors a stack, the target a \
                 map; a sample of the cells, drawn as numpy's default_rng(seed) draws it). Its predictors' \
                 importances: a forest's mean decrease of impurity, a boosting's share of the gain of its splits.",
                Access::Global,
            )
            .input(PortSpec::optional(
                "table",
                Type::Table,
                "the records: the predictors and the target, a column each",
            ))
            .input(PortSpec::optional("predictors", Type::Stack, "or maps: a layer per predictor (stack.of)"))
            .input(PortSpec::optional("target", Type::Map, "with maps: the classes or values to learn"))
            .input(PortSpec::optional("cells", Type::Map, "with maps: the cells to learn from (non-zero; default all)"))
            .output(PortSpec::new("model", Type::MlModel, "the fitted forest"))
            .output(PortSpec::new("importance", Type::Table, "the importance of each predictor (they add up to 1)"))
            .param(PortSpec::optional("column", Type::Text, "with a table: the target's column (default target)"))
            .param(PortSpec::optional(
                "columns",
                Type::Any,
                "with a table: the predictors' columns (default: all but the target's)",
            ))
            .param(
                PortSpec::optional(
                    "task",
                    Type::Text,
                    "classes or values; auto (default): classes when the target holds 50 whole numbers or fewer",
                )
                .choices(["auto", "classes", "values"]),
            )
            .param(PortSpec::optional(
                "sample",
                Type::Number,
                "the records to learn from: a number (1 or more) or a share (below 1) drawn at random (default all)",
            ))
            .param(PortSpec::optional("stratify", Type::Bool, "draw the sample from each class apart (default false)"))
            .param(PortSpec::optional(
                "seed",
                Type::Integer,
                "the seed of the sample's draw (numpy's; default 1)",
            )),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (x, y) = match (args.get("table"), args.get("predictors")) {
            (Some(_), Some(_)) => return Err(fail(args, "a table or maps (predictors), not both")),
            (Some(Value::Table(t)), None) => {
                let column: String = args.param("column", "target".to_string())?;
                let y = t
                    .column(&column)
                    .ok_or_else(|| fail(args, format!("the table has no column {column:?} (setting column)")))?;
                let names: Vec<String> = match args.opt_param::<Vec<String>>("columns")? {
                    Some(c) => c,
                    None => t.columns.iter().filter(|c| **c != column).cloned().collect(),
                };
                let mut columns = Vec::new();
                for n in &names {
                    columns.push(t.column(n).ok_or_else(|| fail(args, format!("the table has no column {n:?}")))?);
                }
                let keep: Vec<usize> =
                    (0..y.len()).filter(|&r| !y[r].is_nan() && columns.iter().all(|c| !c[r].is_nan())).collect();
                let frame =
                    Frame { names, columns: columns.iter().map(|c| keep.iter().map(|&r| c[r]).collect()).collect() };
                (frame, keep.iter().map(|&r| y[r]).collect::<Vec<f64>>())
            }
            (Some(v), None) => return Err(fail(args, format!("table: a table, not a {:?}", v.ty()))),
            (None, Some(_)) => {
                let stack = args.stack("predictors")?;
                let target = args.map("target")?;
                let (names, layers) = named_layers(stack).map_err(|e| fail(args, e))?;
                if target.data.len() != stack.grid.cells() {
                    return Err(fail(args, "the target and the predictors differ in size"));
                }
                let cells = args.opt_map("cells")?;
                let idx: Vec<usize> = (0..target.data.len())
                    .filter(|&i| {
                        !target.data[i].is_nan()
                            && layers.iter().all(|l| !l[i].is_nan())
                            && cells.is_none_or(|c| !c.data[i].is_nan() && c.data[i] != 0.0)
                    })
                    .collect();
                (frame_of(&names, &layers, &idx), idx.iter().map(|&i| target.data[i]).collect())
            }
            (None, None) => return Err(fail(args, "connect the records: a table, or maps (predictors and target)")),
        };
        if y.is_empty() {
            return Err(fail(args, "no record has a target and every predictor"));
        }
        let task: String = args.param("task", "auto".to_string())?;
        let regression = match task.as_str() {
            "auto" => !looks_like_classes(&y),
            "classes" => false,
            "values" => true,
            t => return Err(fail(args, format!("task: auto, classes or values, not {t:?}"))),
        };
        // the sample: numpy's draw of the records (from each class apart when stratified)
        let (x, y) = match args.opt_param::<f64>("sample")? {
            None => (x, y),
            Some(s) => {
                let stratify: bool = args.param("stratify", false)?;
                if stratify && regression {
                    return Err(fail(args, "stratify draws from each class: the target holds values"));
                }
                let mut rng = NumpyRng::new(args.param("seed", 1)?);
                let all: Vec<usize> = (0..y.len()).collect();
                let pick = if stratify {
                    let frac = if s >= 1.0 { (s as usize) as f64 / y.len() as f64 } else { s }.min(1.0);
                    let mut keep = Vec::new();
                    for c in distinct(y.iter().copied()) {
                        let group: Vec<usize> = all.iter().copied().filter(|&k| y[k] == c).collect();
                        let m = group.len().min(((frac * group.len() as f64).round_ties_even() as usize).max(1));
                        keep.extend(rng.choice_distinct(group.len(), m).into_iter().map(|j| group[j]));
                    }
                    keep.sort_unstable();
                    keep
                } else {
                    draw(&all, &vec![false; y.len()], Some(s), false, false, &mut rng)
                };
                let frame = Frame {
                    names: x.names.clone(),
                    columns: x.columns.iter().map(|c| pick.iter().map(|&k| c[k]).collect()).collect(),
                };
                (frame, pick.iter().map(|&k| y[k]).collect())
            }
        };
        let f = fit_model(args, &x, &y, regression)?;
        let importance = importance_table(f.predictors(), &[(None, f.importances())]);
        let model = match f {
            Fitted::Forest(f) => Model::Forest(f),
            Fitted::Boosting(b) => Model::Boosting(b),
        };
        Ok(outputs([("model", Value::MlModel(Arc::new(model))), ("importance", Value::Table(importance))]))
    }
}

// -- ml.predict ---------------------------------------------------------------------------------------------------

struct Predict;

impl Operator for Predict {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "ml.predict",
            "A fitted model (ml.fit, ml.potential) on maps or on the rows of a table: the value, or the class of \
             the largest probability (the first on ties, as scikit-learn's predict) and the probability of each \
             class (a forest's predict_proba, or each class's potential). No value where a predictor has none.",
            Access::Global,
        )
        .input(PortSpec::new("model", Type::MlModel, "the fitted model"))
        .input(PortSpec::optional("predictors", Type::Stack, "maps: a layer per predictor, named as in the fit"))
        .input(PortSpec::optional("table", Type::Table, "or the rows of a table, a column per predictor"))
        .output(PortSpec::optional("map", Type::Map, "with maps: the value, or the class of the largest probability"))
        .output(PortSpec::optional("probability", Type::Stack, "with maps: the probability of each class"))
        .output(PortSpec::optional(
            "table",
            Type::Table,
            "with a table: its columns, then prediction and the probability of each class (p_<class>)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let Some(Value::MlModel(model)) = args.get("model") else { return Err(fail(args, "connect a fitted model")) };
        let names = model.predictors().to_vec();
        let classes = model.classes().to_vec();
        // the prediction of a row: the value, or the first class of the largest probability
        let best = |cols: &[Vec<f64>], i: usize| -> f64 {
            if classes.is_empty() {
                return cols[0][i];
            }
            if cols[0][i].is_nan() {
                return f64::NAN;
            }
            let mut k = 0;
            for c in 1..cols.len() {
                if cols[c][i] > cols[k][i] {
                    k = c;
                }
            }
            classes[k]
        };
        match (args.get("predictors"), args.get("table")) {
            (Some(_), Some(_)) => Err(fail(args, "maps (predictors) or a table, not both")),
            (Some(_), None) => {
                let stack = args.stack("predictors")?;
                let layers: Vec<Arc<Vec<f64>>> = names
                    .iter()
                    .map(|n| {
                        stack.by_name(n).cloned().ok_or_else(|| fail(args, format!("the stack has no layer {n:?}")))
                    })
                    .collect::<Result<_>>()?;
                let all: Vec<usize> = (0..stack.grid.cells()).collect();
                let cols = model.outputs(&frame_of(&names, &layers, &all)).map_err(|e| fail(args, e))?;
                let map = Map::new(stack.grid.clone(), all.iter().map(|&i| best(&cols, i)).collect());
                let mut out = outputs([("map", Value::Map(map))]);
                if !classes.is_empty() {
                    let probability = Stack {
                        grid: stack.grid.clone(),
                        keys: classes.iter().map(|&c| Key::Class(c)).collect(),
                        layers: cols.into_iter().map(Arc::new).collect(),
                    };
                    out.insert("probability".into(), Value::Stack(probability));
                }
                Ok(out)
            }
            (None, Some(Value::Table(t))) => {
                let columns: Vec<Vec<f64>> = names
                    .iter()
                    .map(|n| t.column(n).ok_or_else(|| fail(args, format!("the table has no column {n:?}"))))
                    .collect::<Result<_>>()?;
                let cols = model.outputs(&Frame { names: names.clone(), columns }).map_err(|e| fail(args, e))?;
                let mut out = t.clone();
                out.columns.push("prediction".into());
                if !classes.is_empty() {
                    out.columns.extend(classes.iter().map(|c| format!("p_{}", class_label(*c))));
                }
                for (i, row) in out.rows.iter_mut().enumerate() {
                    row.push(best(&cols, i));
                    if !classes.is_empty() {
                        row.extend(cols.iter().map(|c| c[i]));
                    }
                }
                Ok(outputs([("table", Value::Table(out))]))
            }
            (None, Some(v)) => Err(fail(args, format!("table: a table, not a {:?}", v.ty()))),
            (None, None) => Err(fail(args, "connect the maps (predictors) or a table")),
        }
    }
}

// -- ml.potential -------------------------------------------------------------------------------------------------

struct Potential;

impl Operator for Potential {
    fn signature(&self) -> OpSignature {
        forest_settings(
            OpSignature::new(
                "ml.potential",
                "The potential of each class from a model per class (landschaft's Estimator with scikit-learn's \
                 RandomForestClassifier, or HistGradientBoostingClassifier, the same maps): where each class is \
                 (presence, as lulcc's models), or where it appeared between two maps among the cells without it \
                 (expansion: PLUS's land expansion analysis, LEAS, the growth probability CARS takes); with each \
                 driver's contribution (the models' importances).",
                Access::Global,
            )
            .input(PortSpec::new("landscape", Type::Map, "the map at the start"))
            .input(PortSpec::optional("later", Type::Map, "the later map (expansion)"))
            .input(PortSpec::new("drivers", Type::Stack, "the explanatory variables, a layer each"))
            .input(PortSpec::optional("cells", Type::Map, "the cells to learn from (non-zero; default all)"))
            .output(PortSpec::new(
                "potential",
                Type::Stack,
                "the probability of each class (of its presence, or of its expansion), where the drivers have values",
            ))
            .output(PortSpec::new(
                "importance",
                Type::Table,
                "a row per class: class, then each driver's contribution (they add up to 1)",
            ))
            .output(PortSpec::new("model", Type::MlModel, "the forests, for ml.predict on other drivers"))
            .param(
                PortSpec::optional(
                    "target",
                    Type::Text,
                    "expansion (where the class appeared; needs the later map) or presence (where it is); default: \
                     expansion with a later map, else presence",
                )
                .choices(["expansion", "presence"]),
            )
            .param(PortSpec::optional("classes", Type::Any, "the classes to model (default: those of the maps)"))
            .param(PortSpec::optional(
                "sample",
                Type::Number,
                "the cells each class's forest learns from: a number (1 or more) or a share (below 1) drawn at \
                 random (default all)",
            ))
            .param(PortSpec::optional(
                "stratify",
                Type::Bool,
                "draw the sample from the presences and the absences apart, in their proportions (default false)",
            ))
            .param(PortSpec::optional("balance", Type::Bool, "as many absences as presences (default false)"))
            .param(PortSpec::optional(
                "seed",
                Type::Integer,
                "the seed of the draws (numpy's default_rng; default 1)",
            )),
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let a = args.map("landscape")?;
        let later = args.opt_map("later")?;
        let stack = args.stack("drivers")?;
        let n = a.data.len();
        if stack.grid.cells() != n || later.is_some_and(|b| b.data.len() != n) {
            return Err(fail(args, "the maps and the drivers differ in size"));
        }
        let target: String =
            args.param("target", if later.is_some() { "expansion" } else { "presence" }.to_string())?;
        let expansion = match target.as_str() {
            "expansion" => true,
            "presence" => false,
            t => return Err(fail(args, format!("target: expansion or presence, not {t:?}"))),
        };
        let b = match (expansion, later) {
            (true, Some(b)) => Some(b),
            (true, None) => return Err(fail(args, "expansion needs the later map")),
            (false, _) => None,
        };
        let (names, layers) = named_layers(stack).map_err(|e| fail(args, e))?;
        let cells = args.opt_map("cells")?;
        let valid: Vec<bool> = (0..n)
            .map(|i| {
                !a.data[i].is_nan()
                    && layers.iter().all(|l| !l[i].is_nan())
                    && cells.is_none_or(|c| !c.data[i].is_nan() && c.data[i] != 0.0)
                    && b.is_none_or(|b| !b.data[i].is_nan())
            })
            .collect();
        let classes: Vec<f64> = match args.opt_param::<Vec<f64>>("classes")? {
            Some(c) => c,
            None => distinct(a.data.iter().chain(b.map_or(&[][..], |b| &b.data[..])).copied()),
        };
        let sample: Option<f64> = args.opt_param("sample")?;
        let (stratify, balance): (bool, bool) = (args.param("stratify", false)?, args.param("balance", false)?);
        let mut rng = NumpyRng::new(args.param("seed", 1)?);
        let (mut forests, mut boostings) = (Vec::new(), Vec::new());
        let mut importance = Vec::with_capacity(classes.len());
        for (k, &c) in classes.iter().enumerate() {
            if !ctx.progress(Some(k as f64 / classes.len() as f64), format!("class {}", class_label(c))) {
                return Err(fail(args, "stopped"));
            }
            let (idx, y): (Vec<usize>, Vec<bool>) = match b {
                None => (0..n).filter(|&i| valid[i]).map(|i| (i, a.data[i] == c)).unzip(),
                Some(b) => (0..n).filter(|&i| valid[i] && a.data[i] != c).map(|i| (i, b.data[i] == c)).unzip(),
            };
            let idx = draw(&idx, &y, sample, stratify, balance, &mut rng);
            let labels: Vec<f64> =
                idx.iter().map(|&i| f64::from(u8::from(b.map_or(a.data[i], |b| b.data[i]) == c))).collect();
            if labels.iter().all(|&v| v == labels[0]) {
                return Err(fail(
                    args,
                    format!(
                        "class {}: the cells to learn from are all {} (leave it out with the setting classes)",
                        class_label(c),
                        if labels.first() == Some(&1.0) { "presences" } else { "absences" }
                    ),
                ));
            }
            let f = fit_model(args, &frame_of(&names, &layers, &idx), &labels, false)?;
            importance.push((Some(c), f.importances()));
            match f {
                Fitted::Forest(f) => forests.push(f),
                Fitted::Boosting(b) => boostings.push(b),
            }
        }
        let model = Model::PerClass { target, classes: classes.clone(), forests, boostings };
        let all: Vec<usize> = (0..n).collect();
        let cols = model.outputs(&frame_of(&names, &layers, &all)).map_err(|e| fail(args, e))?;
        let potential = Stack {
            grid: stack.grid.clone(),
            keys: classes.iter().map(|&c| Key::Class(c)).collect(),
            layers: cols.into_iter().map(Arc::new).collect(),
        };
        Ok(outputs([
            ("potential", Value::Stack(potential)),
            ("importance", Value::Table(importance_table(&names, &importance))),
            ("model", Value::MlModel(Arc::new(model))),
        ]))
    }
}
