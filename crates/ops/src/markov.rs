//! Markov chains of land change (`markov.fit`).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::{Args, Ctx, Error, Map, Markov, Operator, Outputs, Registry, Result, Value, outputs};

/// The sorted distinct classes of some maps.
pub fn classes_of(maps: &[&Map]) -> Vec<f64> {
    let mut all: Vec<f64> = maps.iter().flat_map(|m| m.classes()).collect();
    all.sort_by(|a, b| a.partial_cmp(b).unwrap());
    all.dedup();
    all
}

/// The cells of each class of `a` holding each class of `b` (classes x classes, row-major).
pub fn crosstab(a: &Map, b: &Map, classes: &[f64]) -> Result<Vec<f64>> {
    Ok(ffi::crosstab(&a.data, &b.data, classes, classes)?.into_iter().map(|v| v as f64).collect())
}

/// The chain of the change from `t0` to `t1` over `steps` time steps (landschaft's `Markov.from_maps`).
pub fn fit(t0: &Map, t1: &Map, steps: usize) -> Result<Markov> {
    if t0.data.len() != t1.data.len() {
        return Err(Error::msg("the two maps differ in size"));
    }
    if steps < 1 {
        return Err(Error::msg("steps must be >= 1"));
    }
    let classes = classes_of(&[t0, t1]);
    let counts = crosstab(t0, t1, &classes)?;
    let mut kept = Vec::new();
    let single = ffi::markov_from_counts(&counts, classes.len() as i32, &mut kept)?;
    let kept: Vec<f64> = kept.iter().map(|&i| classes[i as usize]).collect();
    let k = kept.len() as i32;
    let multi = ffi::markov_root(&single, k, steps as i32)?;
    Ok(Markov { classes: kept, single, multi, steps })
}

struct Fit;

impl Operator for Fit {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "markov.fit",
            "A Markov chain of land change from two maps: the transition probabilities of the period (cells of each \
             class at the start that hold each class at the end) and, over `steps` time steps, those of one step \
             (the principal root of the period's matrix, as Dinamica EGO's DetermineTransitionMatrix).",
            Access::Global,
        )
        .input(PortSpec::new("t0", Type::Map, "the map at the start of the period"))
        .input(PortSpec::new("t1", Type::Map, "the map at its end"))
        .input(PortSpec::new("steps", Type::Integer, "time steps in the period"))
        .output(PortSpec::new("markov", Type::Markov, "the chain"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let steps = args.integer("steps")?;
        if steps < 1 {
            return Err(Error::msg("steps must be >= 1"));
        }
        let m = fit(args.map("t0")?, args.map("t1")?, steps as usize)?;
        Ok(outputs([("markov", Value::Markov(Arc::new(m)))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Fit));
}
