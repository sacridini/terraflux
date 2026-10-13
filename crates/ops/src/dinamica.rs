//! Dinamica EGO's Expander and Patcher (landschaft's `ExpanderPatcher`): `patches.observe`, `dinamica.step`.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::{PatchSpec, ffi};

use crate::value::{PatchTransition, Regional, class_label};
use crate::{
    Args, Ctx, Error, Key, Map, Markov, Operator, Outputs, PatchParams, Registry, Result, Stack, Table, Value, outputs,
};

/// The patches of each transition observed between two maps (landschaft's `presets.observed_patches`).
pub fn observe(t0: &Map, t1: &Map, transitions: &[(f64, f64)]) -> Result<Vec<terraflux_ls_sys::ObservedPatches>> {
    let (ny, nx) = t0.shape();
    transitions
        .iter()
        .map(|&(f, t)| Ok(ffi::observed_patches(&t0.data, &t1.data, ny as i64, nx as i64, f, t)?))
        .collect()
}

/// The Expander and Patcher settings of Dinamica's classic model (landschaft's `presets.dinamica`): the
/// Expander's share and the patch sizes of each transition measured on the observed change (or the share
/// given), the isometry given.
pub fn patch_params(
    t0: &Map,
    t1: &Map,
    transitions: &[(f64, f64)],
    expansion: Option<f64>,
    isometry: f64,
) -> Result<PatchParams> {
    let obs = observe(t0, t1, transitions)?;
    Ok(PatchParams {
        transitions: transitions
            .iter()
            .zip(obs)
            .map(|(&(from, to), o)| PatchTransition {
                from,
                to,
                expansion: expansion.unwrap_or(o.expansion),
                expander: [o.expansion_mean, o.expansion_variance, isometry],
                patcher: [o.new_mean, o.new_variance, isometry],
            })
            .collect(),
    })
}

/// A region of a step: its cells (all when `None`), its chain and its id.
type RegionPlan<'a> = (Option<Vec<bool>>, &'a Markov, Option<f64>);

/// The rates of a step: one Markov chain for the map, or one per region.
#[derive(Clone, Copy)]
pub enum Rates<'a> {
    One(&'a Markov),
    Regional(&'a Regional),
}

/// One step of the Expander and Patcher (landschaft's `ExpanderPatcher.simulate`): each transition with a rate
/// changes `round(rate x cells of its origin class)` cells, a share by the Expander. With regions, each region
/// in the order of its id with its own rates, its cells only and a seed of its own (Dinamica's
/// regionalization), the map carried from one region to the next.
#[allow(clippy::too_many_arguments)]
pub fn step(
    ctx: &mut Ctx,
    landscape: &Map,
    probability: &Stack,
    rates: Rates<'_>,
    patches: &PatchParams,
    step: usize,
    seed: i64,
    prune_factor: f64,
    window: (i32, i32),
) -> Result<(Map, Table)> {
    let (ny, nx) = landscape.shape();
    let values = &landscape.data;
    let mut current = Vec::with_capacity(values.len());
    for &v in values.iter() {
        if v.is_nan() {
            current.push(i32::MIN);
        } else if v.fract() != 0.0 {
            return Err(Error::msg("the landscape's classes must be integers"));
        } else {
            current.push(v as i32);
        }
    }
    // the plan: per region, its cells, its chain and its transitions
    let plan: Vec<RegionPlan<'_>> = match rates {
        Rates::One(m) => vec![(None, m, None)],
        Rates::Regional(r) => (0..r.parts.len())
            .map(|k| {
                let inside = r.index.iter().map(|&x| x == k as i32).collect();
                Ok((Some(inside), r.markov(k)?.as_ref(), Some(r.ids[k])))
            })
            .collect::<Result<_>>()?,
    };
    let mut transitions: Vec<(f64, f64)> = Vec::new();
    for (_, m, _) in &plan {
        for tr in m.transitions() {
            if !transitions.contains(&tr) {
                transitions.push(tr);
            }
        }
    }
    if transitions.is_empty() {
        return Err(Error::msg("the demand has no transitions"));
    }
    let mut probs = Vec::with_capacity(transitions.len());
    for &(f, t) in &transitions {
        let prob = probability.get(&Key::Transition(f, t)).ok_or_else(|| {
            Error::msg(format!("the potential gives no probability for {} -> {}", class_label(f), class_label(t)))
        })?;
        probs
            .push(prob.iter().map(|&v| if v.is_nan() { 0.0 } else { v.clamp(f64::MIN, f64::MAX) }).collect::<Vec<_>>());
    }
    let mut counts = vec![[0i64; 4]; transitions.len()];
    let regional = matches!(rates, Rates::Regional(_));
    for (k, (inside, m, region)) in plan.iter().enumerate() {
        let trs = m.transitions();
        if trs.is_empty() {
            continue;
        }
        let mut specs = Vec::with_capacity(trs.len());
        for &(f, t) in &trs {
            let rate = m.rate(f, t).unwrap();
            let count = current
                .iter()
                .enumerate()
                .filter(|&(i, &c)| c == f as i32 && inside.as_ref().is_none_or(|m| m[i]))
                .count() as f64;
            let n = (rate * count + 0.5).floor() as i64;
            let p = patches
                .get(f, t)
                .ok_or_else(|| Error::msg(format!("no patch settings for {} -> {}", class_label(f), class_label(t))))?;
            let ne = (n as f64 * p.expansion + 0.5).floor() as i64;
            let j = transitions.iter().position(|&x| x == (f, t)).unwrap();
            let probability = match inside {
                None => probs[j].clone(),
                Some(m) => probs[j].iter().zip(m).map(|(&v, &a)| if a { v } else { 0.0 }).collect(),
            };
            specs.push(PatchSpec {
                from: f as i32,
                to: t as i32,
                probability,
                expand: ne,
                patch: n - ne,
                e_mean: p.expander[0],
                e_var: p.expander[1],
                e_iso: p.expander[2],
                p_mean: p.patcher[0],
                p_var: p.patcher[1],
                p_iso: p.patcher[2],
            });
        }
        let base = (seed as u64).wrapping_add((step as u64).wrapping_mul(0x9E3779B9));
        let sub = if regional { base.wrapping_add((k as u64).wrapping_mul(0x632BE59BD9B4E019)) } else { base };
        let r = ffi::allocate_patches(ny as i64, nx as i64, &current, &specs, prune_factor, window.0, window.1, sub)?;
        current = r.landscape.clone();
        let place = region.map_or(String::new(), |id| format!(" in region {}", class_label(id)));
        for (s, (&e, (&p, &np))) in specs.iter().zip(r.expanded.iter().zip(r.patched.iter().zip(&r.patches))) {
            let j = transitions.iter().position(|&x| x == (s.from as f64, s.to as f64)).unwrap();
            let req = s.expand + s.patch;
            counts[j][0] += req;
            counts[j][1] += e;
            counts[j][2] += p;
            counts[j][3] += np;
            if e + p < req {
                ctx.warn(format!(
                    "step {step}: {} -> {} made {} of {req} changes{place} (too few cells with probability)",
                    class_label(s.from as f64),
                    class_label(s.to as f64),
                    e + p
                ));
            }
        }
    }
    let mut columns = vec!["step".to_string()];
    let mut row = vec![step as f64];
    for (&(f, t), c) in transitions.iter().zip(&counts) {
        let tag = format!("{}_{}", class_label(f), class_label(t));
        for (k, v) in ["requested", "expanded", "patched", "patches"].iter().zip(c) {
            columns.push(format!("{k}_{tag}"));
            row.push(*v as f64);
        }
    }
    let out: Vec<f64> =
        values.iter().zip(&current).map(|(&v, &c)| if v.is_nan() { f64::NAN } else { c as f64 }).collect();
    Ok((Map::new(landscape.grid.clone(), out), Table { columns, rows: vec![row] }))
}

struct Observe;

impl Operator for Observe {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "patches.observe",
            "The Expander and Patcher settings of each transition with a rate, measured on the observed change: \
             the share of the changed cells in expansions of existing patches (8 neighbours), and the mean and \
             variance of the sizes of the expansion and new patches; the isometry is given.",
            Access::Global,
        )
        .input(PortSpec::new("t0", Type::Map, "the map at the start"))
        .input(PortSpec::new("t1", Type::Map, "the map at the end"))
        .input(PortSpec::new("markov", Type::Markov, "the transitions with a rate"))
        .output(PortSpec::new("patches", Type::Patches, "the settings of each transition"))
        .param(PortSpec::optional("expansion", Type::Number, "the Expander's share for all (default: observed)"))
        .param(PortSpec::optional("isometry", Type::Number, "the isometry of the patches (1.5)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let tr = args.markov("markov")?.transitions();
        let p = patch_params(
            args.map("t0")?,
            args.map("t1")?,
            &tr,
            args.opt_param("expansion")?,
            args.param("isometry", 1.5)?,
        )?;
        Ok(outputs([("patches", Value::Patches(Arc::new(p)))]))
    }
}

struct StepOp;

impl Operator for StepOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "dinamica.step",
            "One step of Dinamica's Expander and Patcher: the changes of each transition (its rate per step times \
             the cells of its origin class) split between the Expander, which grows existing patches, and the \
             Patcher, which seeds new ones, with lognormal patch sizes shaped by the isometry.",
            Access::Global,
        )
        .input(PortSpec::new("landscape", Type::Map, "the map at the start of the step"))
        .input(PortSpec::new("probability", Type::Stack, "a layer per transition (e.g. woe.probability)"))
        .input(PortSpec::optional("rates", Type::Markov, "the rates per step"))
        .input(PortSpec::optional("regional", Type::Regional, "or the rates of each region (Markov chains)"))
        .input(PortSpec::new("patches", Type::Patches, "the Expander and Patcher settings"))
        .input(PortSpec::new("step", Type::Integer, "the step (1, 2...)"))
        .input(PortSpec::new("seed", Type::Integer, "the seed of the simulation"))
        .output(PortSpec::new("landscape", Type::Map, "the map at the end of the step"))
        .output(PortSpec::new("info", Type::Table, "cells requested and made by each function, patches"))
        .param(PortSpec::optional("prune_factor", Type::Number, "cells ranked per change for the draws (10)"))
        .param(PortSpec::optional("window", Type::Any, "the neighbour window [rows, columns] ([3, 3])"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let prune: f64 = args.param("prune_factor", 10.0)?;
        if prune.is_nan() || prune < 1.0 {
            return Err(Error::msg("prune_factor must be at least 1"));
        }
        let w: (i32, i32) = args.param("window", (3, 3))?;
        if w.0 < 1 || w.1 < 1 || w.0 % 2 == 0 || w.1 % 2 == 0 {
            return Err(Error::msg("the window needs two odd sides"));
        }
        let (m, info) = step(
            ctx,
            args.map("landscape")?,
            args.stack("probability")?,
            match (args.get("rates"), args.get("regional")) {
                (Some(_), None) => Rates::One(args.markov("rates")?.as_ref()),
                (None, Some(_)) => Rates::Regional(args.regional("regional")?.as_ref()),
                _ => return Err(Error::msg(format!("node {:?}: give the rates or a regional demand", args.node))),
            },
            args.patches("patches")?,
            args.integer("step")? as usize,
            args.integer("seed")?,
            prune,
            w,
        )?;
        Ok(outputs([("landscape", Value::Map(m)), ("info", Value::Table(info))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Observe));
    r.add(Arc::new(StepOp));
}
