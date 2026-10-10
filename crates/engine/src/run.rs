//! Dinamica EGO's classic model on tiles, from the fit to the last step.

use std::path::PathBuf;

use terraflux_ops::woe::{Settings as WoeSettings, WoeModel};
use terraflux_ops::{Markov, PatchParams};

use crate::dinamica::{Done, Settings, probabilities, step, transitions};
use crate::tiles::{Layer, Source, Tiling};
use crate::{Result, fit};

/// The pieces of the model.
#[derive(Debug, Clone, PartialEq)]
pub struct Fitted {
    pub markov: Markov,
    pub woe: WoeModel,
    pub patches: PatchParams,
}

/// Fit the model on two maps (by reductions over the tiles).
#[allow(clippy::too_many_arguments)]
pub fn fit_dinamica(
    t0: &dyn Source,
    t1: &dyn Source,
    drivers: &[(String, &dyn Source)],
    tiling: &Tiling,
    steps: usize,
    woe: &WoeSettings,
    expansion: Option<f64>,
    isometry: f64,
) -> Result<Fitted> {
    terraflux_ops::threads::ensure();
    let markov = fit::markov(t0, t1, tiling, steps)?;
    let tr = markov.transitions();
    let w = fit::woe(t0, t1, drivers, tiling, woe, Some(tr.clone()))?;
    let patches = fit::patches(t0, t1, tiling, &tr, expansion, isometry)?;
    Ok(Fitted { markov, woe: w, patches })
}

/// Simulate `steps` steps from `start`; `on_step(step, land, done)` gets each step's land (kept in `dir`, a
/// folder per step, when given). Returns the last land.
#[allow(clippy::too_many_arguments)]
pub fn simulate(
    tiling: &Tiling,
    start: &dyn Source,
    drivers: &[(String, &dyn Source)],
    fitted: &Fitted,
    steps: usize,
    seed: u64,
    settings: &Settings,
    dir: Option<PathBuf>,
    mut on_step: impl FnMut(usize, &Layer, &[Done]) -> Result<()>,
) -> Result<Option<Layer>> {
    terraflux_ops::threads::ensure();
    let trs = transitions(&fitted.markov, &fitted.patches)?;
    // the probability layers follow the order of the weights' transitions: one per transition of the step
    let order: Vec<usize> = trs
        .iter()
        .map(|t| {
            fitted
                .woe
                .transitions
                .iter()
                .position(|&(f, to)| f == t.from && to == t.to)
                .ok_or_else(|| crate::Error::Msg(format!("no weights for {} -> {}", t.from, t.to)))
        })
        .collect::<Result<_>>()?;
    let mut land: Option<Layer> = None;
    for k in 1..=steps {
        let sub = dir.as_ref().map(|d| d.join(format!("step_{k:03}")));
        let current: &dyn Source = match &land {
            Some(l) => l,
            None => start,
        };
        crate::trace(|| format!("step {k}: start"));
        let probs = probabilities(tiling, &fitted.woe, current, drivers, sub.as_ref().map(|d| d.join("probability")))?;
        crate::trace(|| format!("step {k}: probabilities"));
        let ordered: Vec<&Layer> = order.iter().map(|&i| &probs[i]).collect();
        let (next, done) = step(tiling, current, &ordered, &trs, seed, k as u64, settings, sub)?;
        on_step(k, &next, &done)?;
        land = Some(next);
    }
    Ok(land)
}
