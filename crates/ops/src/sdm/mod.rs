//! Species distribution modelling from landschaft's sdm module: how far new conditions lie from a model's
//! (MESS, MOP, ExDet).

pub mod extrapolation;

use std::sync::Arc;

use extrapolation::Frame;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;

use crate::{Args, Ctx, Error, Key, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Mess));
    r.add(Arc::new(Mop));
    r.add(Arc::new(Exdet));
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
