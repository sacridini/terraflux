//! Species distribution modelling from landschaft's sdm module: how far new conditions lie from a model's
//! (MESS, MOP, ExDet) and how well a model separates presences from the background (AUC, thresholds, Boyce,
//! omission rates).

pub mod evaluate;
pub mod extrapolation;
pub mod maxnet;

use std::sync::Arc;

use extrapolation::Frame;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;

use crate::{Args, Ctx, Error, Key, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Mess));
    r.add(Arc::new(Mop));
    r.add(Arc::new(Exdet));
    r.add(Arc::new(Evaluate));
    r.add(Arc::new(BoyceIndex));
    r.add(Arc::new(Threshold));
    r.add(Arc::new(OmissionRate));
    r.add(Arc::new(FitMaxNet));
    r.add(Arc::new(PredictMaxNet));
}

fn bad(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

fn key_name(k: &Key) -> String {
    match k {
        Key::Name(n) => n.clone(),
        k => format!("{k:?}"),
    }
}

/// A table, a stack of maps (a column per layer) or a map; the grid of the maps.
fn frame_of(args: &Args, port: &str) -> Result<(Frame, Option<Arc<Grid>>)> {
    match args.get(port) {
        Some(Value::Table(t)) => Ok((
            Frame {
                names: t.columns.clone(),
                columns: (0..t.columns.len()).map(|c| t.rows.iter().map(|r| r[c]).collect()).collect(),
            },
            None,
        )),
        Some(Value::Stack(s)) => Ok((
            Frame {
                names: s.keys.iter().map(key_name).collect(),
                columns: s.layers.iter().map(|l| l.to_vec()).collect(),
            },
            Some(s.grid.clone()),
        )),
        Some(Value::Map(m)) => {
            Ok((Frame { names: vec!["layer".into()], columns: vec![m.data.to_vec()] }, Some(m.grid.clone())))
        }
        _ => Err(bad(args, format!("connect {port}: a table, a stack of maps or a map"))),
    }
}

/// The result as a stack on the grid of the target, or a table.
fn frame_out(f: Frame, grid: Option<Arc<Grid>>) -> Outputs {
    let v = match grid {
        Some(grid) => Value::Stack(Stack {
            grid,
            keys: f.names.iter().map(|n| Key::Name(n.clone())).collect(),
            layers: f.columns.into_iter().map(Arc::new).collect(),
        }),
        None => {
            let n = f.columns.first().map_or(0, |c| c.len());
            let mut t = Table::new(f.names);
            t.rows = (0..n).map(|r| f.columns.iter().map(|c| c[r]).collect()).collect();
            Value::Table(t)
        }
    };
    outputs([("result", v)])
}

fn sides(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::new(
        "reference",
        Type::Any,
        "the conditions the model knows: a table of the predictors (e.g. its presences and background) or a stack",
    ))
    .input(PortSpec::new("env", Type::Any, "the new conditions: a stack of the predictors (by name), or a table"))
    .output(PortSpec::new("result", Type::Any, "a stack on the new conditions' grid, or a table"))
}

struct Mess;

impl Operator for Mess {
    fn signature(&self) -> OpSignature {
        sides(OpSignature::new(
            "sdm.mess",
            "Elith's multivariate environmental similarity surface (landschaft's mess, dismo's): per predictor \
             the similarity of each value to the reference's (negative outside their range), and the least of \
             them; negative where the model extrapolates.",
            Access::Global,
        ))
        .param(PortSpec::optional("full", Type::Bool, "every predictor's MESS too (default false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (reference, _) = frame_of(args, "reference")?;
        let (target, grid) = frame_of(args, "env")?;
        let f = extrapolation::mess(&reference, &target, args.param("full", false)?).map_err(|e| bad(args, e))?;
        Ok(frame_out(f, grid))
    }
}

struct Mop;

impl Operator for Mop {
    fn signature(&self) -> OpSignature {
        sides(OpSignature::new(
            "sdm.mop",
            "The mobility-oriented parity of new conditions (landschaft's mop, the mop package): the mean \
             distance of each to its nearest reference conditions (Euclidean or Mahalanobis), and where the \
             predictors fall outside the reference's ranges (basic; simple: how many; detailed: which, below \
             and above, as sums of 2^k).",
            Access::Global,
        ))
        .param(
            PortSpec::optional("type", Type::Text, "basic (default), simple or detailed")
                .choices(["basic", "simple", "detailed"]),
        )
        .param(PortSpec::optional("distance", Type::Text, "euclidean (default), mahalanobis or none").choices([
            "euclidean",
            "mahalanobis",
            "none",
        ]))
        .param(
            PortSpec::optional(
                "where",
                Type::Text,
                "the conditions whose distance to take: in_range (default), out_range or all",
            )
            .choices(["in_range", "out_range", "all"]),
        )
        .param(PortSpec::optional(
            "percentage",
            Type::Number,
            "the share of the reference nearest, (0, 100] (default 1)",
        ))
        .param(PortSpec::optional("scale", Type::Bool, "scale the predictors (Euclidean; default false)"))
        .param(PortSpec::optional("center", Type::Bool, "center them (Euclidean; default false)"))
        .param(PortSpec::optional("rescale", Type::Bool, "the distances rescaled to 0-1 (default false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (reference, _) = frame_of(args, "reference")?;
        let (target, grid) = frame_of(args, "env")?;
        let kind: String = args.param("type", "basic".into())?;
        let distance: String = args.param("distance", "euclidean".into())?;
        let r#where: String = args.param("where", "in_range".into())?;
        let o = extrapolation::Mop {
            kind: &kind,
            distance: if distance == "none" { None } else { Some(&distance) },
            r#where: &r#where,
            percentage: args.param("percentage", 1.0)?,
            scale: args.param("scale", false)?,
            center: args.param("center", false)?,
            rescale: args.param("rescale", false)?,
        };
        let f = extrapolation::mop(&reference, &target, &o).map_err(|e| bad(args, e))?;
        Ok(frame_out(f, grid))
    }
}

struct Exdet;

impl Operator for Exdet {
    fn signature(&self) -> OpSignature {
        sides(OpSignature::new(
            "sdm.exdet",
            "Mesgaran's extrapolation detection (landschaft's exdet): NT1, below 0 where a predictor falls outside \
             the reference's range, and inside the ranges NT2, the Mahalanobis distance against the reference's \
             largest (above 1 a novel combination); mic, the most influential covariate.",
            Access::Global,
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (reference, _) = frame_of(args, "reference")?;
        let (target, grid) = frame_of(args, "env")?;
        let f = extrapolation::exdet(&reference, &target).map_err(|e| bad(args, e))?;
        Ok(frame_out(f, grid))
    }
}

/// Predictions: a column of a table (`column`, else the first) or a map's cells.
fn predictions(args: &Args, port: &str) -> Result<Vec<f64>> {
    match args.get(port) {
        Some(Value::Table(t)) => {
            let column: Option<String> = args.opt_param("column")?;
            let c = match column {
                Some(c) => {
                    t.columns.iter().position(|x| *x == c).ok_or_else(|| bad(args, format!("no column {c:?}")))?
                }
                None => 0,
            };
            Ok(t.rows.iter().map(|r| r[c]).collect())
        }
        Some(Value::Map(m)) => Ok(m.data.to_vec()),
        _ => Err(bad(args, format!("connect {port}: a table of predictions or a map"))),
    }
}

fn predicted(sig: OpSignature, a: (&str, &str), b: (&str, &str)) -> OpSignature {
    sig.input(PortSpec::new(a.0, Type::Any, a.1)).input(PortSpec::new(b.0, Type::Any, b.1)).param(PortSpec::optional(
        "column",
        Type::Text,
        "the tables' column of the predictions (default the first)",
    ))
}

fn row_table(names: &[&str], values: &[f64]) -> Value {
    let mut t = Table::new(names.iter().map(|s| s.to_string()).collect());
    t.rows = vec![values.to_vec()];
    Value::Table(t)
}

fn columns_table(names: &[&str], cols: &[Vec<f64>]) -> Value {
    let n = cols.first().map_or(0, |c| c.len());
    let mut t = Table::new(names.iter().map(|s| s.to_string()).collect());
    t.rows = (0..n).map(|r| cols.iter().map(|c| c[r]).collect()).collect();
    Value::Table(t)
}

fn boyce_table(b: &evaluate::Boyce) -> Value {
    let used: Vec<f64> = (0..b.f_ratio.len()).map(|k| f64::from(u8::from(b.used.contains(&k)))).collect();
    columns_table(&["f_ratio", "hs", "used"], &[b.f_ratio.clone(), b.hs.clone(), used])
}

struct Evaluate;

impl Operator for Evaluate {
    fn signature(&self) -> OpSignature {
        predicted(
            OpSignature::new(
                "sdm.evaluate",
                "How well predictions separate presences from the background (landschaft's evaluate, dismo's): \
                 AUC, the point-biserial correlation and its p-value, the prevalence, the largest TSS and the \
                 Boyce index; the confusion table at every threshold (kappa, sensitivity, specificity, TSS...) and \
                 the thresholds that optimize them (max_kappa, max_spec_sens, no_omission, equal_prevalence, \
                 equal_sens_spec, mtp, p10).",
                Access::Global,
            )
            .output(PortSpec::new("stats", Type::Table, "auc, cor, pcor, np, na, prevalence, max_tss, boyce"))
            .output(PortSpec::new("table", Type::Table, "the confusion table per threshold"))
            .output(PortSpec::new("thresholds", Type::Table, "the optimal thresholds"))
            .output(PortSpec::new("boyce", Type::Table, "Boyce's classes: f_ratio, hs, used")),
            ("presences", "the predictions at the presences"),
            ("background", "at the background points (or absences)"),
        )
        .param(PortSpec::optional("thresholds", Type::Any, "the thresholds of the table (default dismo's)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let p = predictions(args, "presences")?;
        let a = predictions(args, "background")?;
        let tr: Option<Vec<f64>> = args.opt_param("thresholds")?;
        let e = evaluate::evaluate(&p, &a, tr.as_deref()).map_err(|e| bad(args, e))?;
        Ok(outputs([
            ("stats", row_table(&evaluate::STATS, &e.stats)),
            ("table", columns_table(&evaluate::TABLE, &e.table)),
            ("thresholds", row_table(&evaluate::THRESHOLDS, &e.thresholds)),
            ("boyce", boyce_table(&e.boyce)),
        ]))
    }
}

struct BoyceIndex;

impl Operator for BoyceIndex {
    fn signature(&self) -> OpSignature {
        predicted(
            OpSignature::new(
                "sdm.boyce",
                "The continuous Boyce index (landschaft's boyce, ecospat's ecospat.boyce): the correlation \
                 between the classes of suitability and their ratio of predicted to expected presences, in a \
                 moving window (default) or in classes.",
                Access::Global,
            )
            .output(PortSpec::new("cor", Type::Number, "the index (rounded to 3 digits)"))
            .output(PortSpec::new("table", Type::Table, "the classes: f_ratio, hs, used")),
            ("presences", "the predictions at the presences"),
            ("available", "the predictions everywhere (or at the background and the presences)"),
        )
        .param(PortSpec::optional(
            "nclass",
            Type::Any,
            "0 (default, a moving window), a number of classes or their breaks",
        ))
        .param(PortSpec::optional("window", Type::Number, "the window's width (default a tenth of the range)"))
        .param(PortSpec::optional("res", Type::Integer, "the window's steps (default 100)"))
        .param(
            PortSpec::optional("method", Type::Text, "spearman (default), pearson or kendall")
                .choices(["spearman", "pearson", "kendall"]),
        )
        .param(PortSpec::optional("rm_duplicate", Type::Bool, "drop repeated ratios (default true)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let p = predictions(args, "presences")?;
        let a = predictions(args, "available")?;
        let breaks: Vec<f64>;
        let classes = match args.params.get("nclass") {
            Some(serde_json::Value::Array(_)) => {
                breaks = args.param("nclass", vec![])?;
                evaluate::Classes::Breaks(&breaks)
            }
            _ => match args.param::<usize>("nclass", 0)? {
                0 => evaluate::Classes::Window(args.opt_param("window")?, args.param("res", 100)?),
                n => evaluate::Classes::Count(n),
            },
        };
        let method: String = args.param("method", "spearman".into())?;
        let b =
            evaluate::boyce(&p, &a, &classes, &method, args.param("rm_duplicate", true)?).map_err(|e| bad(args, e))?;
        Ok(outputs([("cor", Value::Number(b.cor)), ("table", boyce_table(&b))]))
    }
}

struct Threshold;

impl Operator for Threshold {
    fn signature(&self) -> OpSignature {
        predicted(
            OpSignature::new(
                "sdm.threshold",
                "A threshold of suitability from presences and background (landschaft's threshold, dismo's): \
                 max_spec_sens (default), max_kappa, no_omission, equal_prevalence, equal_sens_spec, mtp, p10, or \
                 the highest that keeps a sensitivity.",
                Access::Global,
            )
            .output(PortSpec::new("threshold", Type::Number, "the threshold")),
            ("presences", "the predictions at the presences"),
            ("background", "at the background points"),
        )
        .param(PortSpec::optional("method", Type::Text, "the criterion (default max_spec_sens)"))
        .param(PortSpec::optional("sensitivity", Type::Number, "of the method sensitivity (default 0.9)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let p = predictions(args, "presences")?;
        let a = predictions(args, "background")?;
        let method: String = args.param("method", "max_spec_sens".into())?;
        let t = evaluate::threshold(&p, &a, &method, args.param("sensitivity", 0.9)?).map_err(|e| bad(args, e))?;
        Ok(outputs([("threshold", Value::Number(t))]))
    }
}

struct OmissionRate;

impl Operator for OmissionRate {
    fn signature(&self) -> OpSignature {
        predicted(
            OpSignature::new(
                "sdm.omission_rate",
                "The share of test presences predicted below the training presences' minimum (mtp) or 10th \
                 percentile (p10, default) (landschaft's omission_rate, ENMeval's).",
                Access::Global,
            )
            .output(PortSpec::new("rate", Type::Number, "the omission rate")),
            ("train", "the predictions at the training presences"),
            ("test", "at the test presences"),
        )
        .param(PortSpec::optional("threshold", Type::Text, "p10 (default) or mtp").choices(["p10", "mtp"]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let train = predictions(args, "train")?;
        let test = predictions(args, "test")?;
        let t: String = args.param("threshold", "p10".into())?;
        let r = evaluate::omission_rate(&train, &test, &t).map_err(|e| bad(args, e))?;
        Ok(outputs([("rate", Value::Number(r))]))
    }
}

/// The rows of a table as a frame.
fn table_frame(t: &Table) -> Frame {
    Frame {
        names: t.columns.clone(),
        columns: (0..t.columns.len()).map(|c| t.rows.iter().map(|r| r[c]).collect()).collect(),
    }
}

/// The predictors and the presences (1) and background (0): a table with a presence column, or a table of
/// presences and one of background points.
fn training(args: &Args) -> Result<(Frame, Vec<f64>)> {
    if let Some(Value::Table(t)) = args.get("data") {
        let presence: String = args.param("presence", "presence".into())?;
        let mut f = table_frame(t);
        let k =
            f.names.iter().position(|n| *n == presence).ok_or_else(|| bad(args, format!("no column {presence:?}")))?;
        f.names.remove(k);
        let y = f.columns.remove(k);
        return Ok((f, y));
    }
    let (Some(Value::Table(p)), Some(Value::Table(b))) = (args.get("presences"), args.get("background")) else {
        return Err(bad(args, "connect a table with a presence column, or tables of presences and background"));
    };
    if p.columns != b.columns {
        return Err(bad(args, "the presences and the background must have the same columns"));
    }
    let (fp, fb) = (table_frame(p), table_frame(b));
    let f = Frame {
        names: fp.names.clone(),
        columns: fp.columns.iter().zip(&fb.columns).map(|(a, b)| a.iter().chain(b).copied().collect()).collect(),
    };
    let y = std::iter::repeat_n(1.0, p.rows.len()).chain(std::iter::repeat_n(0.0, b.rows.len())).collect();
    Ok((f, y))
}

struct FitMaxNet;

impl Operator for FitMaxNet {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "sdm.maxnet",
            "A MaxNet species distribution model (landschaft's MaxNet, the maxnet package): Maxent as a lasso \
             logistic regression of presences against background points over linear, quadratic, product, hinge, \
             threshold and categorical features of the predictors (glmnet's path, maxnet's regularisation).",
            Access::Global,
        )
        .input(PortSpec::optional(
            "data",
            Type::Table,
            "the predictors and a presence column (1 presence, 0 background)",
        ))
        .input(PortSpec::optional("presences", Type::Table, "or the predictors at the presences"))
        .input(PortSpec::optional("background", Type::Table, "and at the background points"))
        .output(PortSpec::new("model", Type::SdmModel, "the fitted model"))
        .output(PortSpec::new(
            "coefficients",
            Type::Any,
            "every feature with a nonzero coefficient and its coefficient",
        ))
        .param(PortSpec::optional("presence", Type::Text, "the presence column of data (default presence)"))
        .param(PortSpec::optional("categorical", Type::Any, "the predictors that are categories (classes)"))
        .param(PortSpec::optional("classes", Type::Text, "the feature classes: default, or letters of l, q, p, h, t"))
        .param(PortSpec::optional("regmult", Type::Number, "the regularisation multiplier (default 1)"))
        .param(PortSpec::optional(
            "add_samples_to_background",
            Type::Bool,
            "add the presences to the background (default true)",
        ))
        .param(
            PortSpec::optional(
                "type",
                Type::Text,
                "the predictions' output: cloglog (default), logistic, exponential or link",
            )
            .choices(maxnet::OUTPUTS),
        )
        .param(PortSpec::optional(
            "clamp",
            Type::Bool,
            "clamp the predictors to the training ranges (default true)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (frame, y) = training(args)?;
        let categorical: Vec<String> = match args.params.get("categorical") {
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(serde_json::Value::String(c)) => vec![c.clone()],
            Some(_) => args.param("categorical", vec![])?,
        };
        let classes: String = args.param("classes", "default".into())?;
        let output: String = args.param("type", "cloglog".into())?;
        let s = maxnet::Settings {
            categorical: &categorical,
            classes: &classes,
            regmult: args.param("regmult", 1.0)?,
            add_samples_to_background: args.param("add_samples_to_background", true)?,
            output: &output,
            clamp: args.param("clamp", true)?,
        };
        let m = maxnet::fit(&frame, &y, &s).map_err(|e| bad(args, e))?;
        let coefficients: serde_json::Map<String, serde_json::Value> =
            m.names.iter().zip(&m.betas).map(|(n, b)| (n.clone(), serde_json::json!(b))).collect();
        Ok(outputs([
            ("model", Value::SdmModel(Arc::new(m))),
            ("coefficients", Value::Json(serde_json::Value::Object(coefficients))),
        ]))
    }
}

struct PredictMaxNet;

impl Operator for PredictMaxNet {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "sdm.maxnet_predict",
            "A MaxNet's predictions (maxnet's predict): the suitability of every cell of a stack of the \
             predictors (by name), or of every row of a table; cloglog, logistic, exponential or the link.",
            Access::Global,
        )
        .input(PortSpec::new("model", Type::SdmModel, "a fitted model (sdm.maxnet)"))
        .input(PortSpec::new("env", Type::Any, "a stack of the predictors (by name), or a table"))
        .output(PortSpec::new("prediction", Type::Any, "a map on the stack's grid, or a table"))
        .param(PortSpec::optional("type", Type::Text, "the output (default the model's)").choices(maxnet::OUTPUTS))
        .param(PortSpec::optional("clamp", Type::Bool, "clamp to the training ranges (default the model's)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let Some(Value::SdmModel(m)) = args.get("model") else { return Err(bad(args, "connect a fitted model")) };
        let (frame, grid) = frame_of(args, "env")?;
        let output: Option<String> = args.opt_param("type")?;
        let p = m.predict(&frame, output.as_deref(), args.opt_param("clamp")?).map_err(|e| bad(args, e))?;
        let v = match grid {
            Some(grid) => Value::Map(crate::Map::new(grid, p)),
            None => {
                let mut t = Table::new(vec!["prediction".into()]);
                t.rows = p.into_iter().map(|x| vec![x]).collect();
                Value::Table(t)
            }
        };
        Ok(outputs([("prediction", v)]))
    }
}
