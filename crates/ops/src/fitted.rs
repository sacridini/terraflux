//! Fitted pieces given in the model file: models fitted elsewhere (landschaft's Python API writes them so)
//! run as they are, without fitting again. `woe.weights`, `glm.coefficients`, `markov.matrix`,
//! `patches.settings`.

use std::sync::Arc;

use serde::de::DeserializeOwned;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::{Args, Ctx, Error, GlmModel, Markov, Operator, Outputs, PatchParams, Registry, Result, Value, outputs};

fn read<T: DeserializeOwned>(args: &Args, name: &str) -> Result<T> {
    let v = args.params.get(name).ok_or_else(|| Error::msg(format!("node {:?}: give {name:?}", args.node)))?;
    serde_json::from_value(v.clone()).map_err(|e| Error::msg(format!("node {:?}: {name:?}: {e}", args.node)))
}

struct Weights;

impl Operator for Weights {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "woe.weights",
            "Weights of evidence given in the model (as woe.fit gives them): transitions, ranges and weights.",
            Access::None,
        )
        .output(PortSpec::new("model", Type::Woe, "the weights"))
        .param(PortSpec::new("weights", Type::Any, "the fitted weights (woe.fit's model)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m: crate::woe::WoeModel = read(args, "weights")?;
        Ok(outputs([("model", Value::Woe(Arc::new(m)))]))
    }
}

struct Coefficients;

impl Operator for Coefficients {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "glm.coefficients",
            "Logistic regressions given in the model: per class, its drivers and coefficients (the intercept \
             first), as glm.fit gives them.",
            Access::None,
        )
        .output(PortSpec::new("model", Type::Glm, "the regressions"))
        .param(PortSpec::new("fits", Type::Any, "[{ class, drivers, coefficients, pivot, rank }, ...]"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m: GlmModel = GlmModel { fits: read(args, "fits")? };
        for f in &m.fits {
            if f.coefficients.len() != f.drivers.len() + 1 || f.pivot.len() != f.coefficients.len() {
                return Err(Error::msg(format!("class {}: one coefficient per driver and the intercept", f.class)));
            }
        }
        Ok(outputs([("model", Value::Glm(Arc::new(m)))]))
    }
}

struct Matrix;

impl Operator for Matrix {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "markov.matrix",
            "A Markov chain given in the model: the transition probabilities of a period over `steps` steps (rows \
             the class at the start); those per step are its principal root, unless given.",
            Access::None,
        )
        .output(PortSpec::new("markov", Type::Markov, "the chain"))
        .param(PortSpec::new("classes", Type::Any, "the class of each row and column"))
        .param(PortSpec::new("single", Type::Any, "the probabilities of the period, a row per class"))
        .param(PortSpec::optional("multi", Type::Any, "the probabilities per step (default: the root)"))
        .param(PortSpec::optional("steps", Type::Integer, "steps in the period (1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let classes: Vec<f64> = read(args, "classes")?;
        let k = classes.len();
        let rows: Vec<Vec<f64>> = read(args, "single")?;
        if rows.len() != k || rows.iter().any(|r| r.len() != k) {
            return Err(Error::msg("the transition matrix must be square, a row and column per class"));
        }
        let steps: usize = args.param("steps", 1)?;
        if steps < 1 {
            return Err(Error::msg("steps must be >= 1"));
        }
        let single = rows.concat();
        let multi = match args.opt_param::<Vec<Vec<f64>>>("multi")? {
            Some(m) if m.len() == k && m.iter().all(|r| r.len() == k) => m.concat(),
            Some(_) => return Err(Error::msg("multi must be square, a row and column per class")),
            None => ffi::markov_root(&single, k as i32, steps as i32)?,
        };
        Ok(outputs([("markov", Value::Markov(Arc::new(Markov { classes, single, multi, steps })))]))
    }
}

struct Settings;

impl Operator for Settings {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "patches.settings",
            "The Expander and Patcher settings given in the model: per transition the Expander's share and the \
             mean, variance and isometry of the patches of each function.",
            Access::None,
        )
        .output(PortSpec::new("patches", Type::Patches, "the settings"))
        .param(PortSpec::new(
            "transitions",
            Type::Any,
            "[{ from, to, expansion, expander = [mean, variance, isometry], patcher = [...] }, ...]",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let p = PatchParams { transitions: read(args, "transitions")? };
        Ok(outputs([("patches", Value::Patches(Arc::new(p)))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Weights));
    r.add(Arc::new(Coefficients));
    r.add(Arc::new(Matrix));
    r.add(Arc::new(Settings));
}
