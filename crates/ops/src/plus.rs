//! PLUS's CARS, the CA of multi-type random patch seeds (Liang et al. 2021), as landschaft's `CARS`:
//! `plus.demand`, `cars.step`.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::value::{RegionDemand, Regional, class_label};
use crate::{
    Args, Ctx, Demand, Error, Key, Map, Markov, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs,
};

/// The index of each cell's class among `classes` (-1 without a class); an error for a class without demand.
fn class_index(land: &Map, classes: &[f64]) -> Result<Vec<i32>> {
    let mut unknown = Vec::new();
    let idx = land
        .data
        .iter()
        .map(|&v| {
            if v.is_nan() {
                return -1;
            }
            match classes.iter().position(|&c| c == v) {
                Some(j) => j as i32,
                None => {
                    if !unknown.contains(&v) {
                        unknown.push(v);
                    }
                    -1
                }
            }
        })
        .collect();
    if !unknown.is_empty() {
        let names: Vec<String> = unknown.iter().map(|&c| class_label(c)).collect();
        return Err(Error::msg(format!("the landscape has classes without demand: {names:?}")));
    }
    Ok(idx)
}

/// The cells of each class at each step for CARS (landschaft's `CARS._demand`): a Markov chain projected
/// from the cells of the starting map (rounded to whole cells adding up to the map's), or an area demand
/// (whole cells that add up to the map's at every step).
pub fn demand(start: &Map, markov: Option<&Markov>, area: Option<&Demand>, steps: Option<usize>) -> Result<Demand> {
    if let (Some(m), None) = (markov, area) {
        return demand_in(start, None, &m.classes, markov, area, steps);
    }
    match (markov, area) {
        (Some(m), None) => {
            let classes = m.classes.clone();
            let idx = class_index(start, &classes)?;
            let k = classes.len();
            let mut counts = vec![0.0; k];
            for &i in &idx {
                if i >= 0 {
                    counts[i as usize] += 1.0;
                }
            }
            let ncell = idx.iter().filter(|&&i| i >= 0).count() as i64;
            let n = steps.unwrap_or(m.steps);
            let proj = ffi::markov_project(&counts, &m.multi, k as i32, n as i32)?;
            let rounded = ffi::round_sum(&proj[k..], n as i64, k as i64, ncell)?;
            let mut table = vec![counts];
            table.extend(rounded.chunks(k).map(|r| r.to_vec()));
            Ok(Demand { classes, times: (0..=n).map(|t| t as f64).collect(), table })
        }
        (None, Some(d)) => {
            let n = steps.unwrap_or(d.steps());
            if n > d.steps() {
                return Err(Error::msg(format!("the demand has {} steps, not {n}", d.steps())));
            }
            let ncell = start.data.iter().filter(|v| !v.is_nan()).count() as f64;
            let table: Vec<Vec<f64>> = d.table[..=n].to_vec();
            for (s, row) in table.iter().enumerate() {
                if row.iter().any(|v| v.fract() != 0.0 || *v < 0.0) {
                    return Err(Error::msg("the demand must be whole, non-negative numbers of cells"));
                }
                let sum: f64 = row.iter().sum();
                if s > 0 && sum != ncell {
                    return Err(Error::msg(format!(
                        "the demand of step {s} adds up to {sum} cells, the map has {ncell}"
                    )));
                }
            }
            class_index(start, &d.classes)?;
            Ok(Demand { classes: d.classes.clone(), times: d.times[..=n].to_vec(), table })
        }
        _ => Err(Error::msg("give a Markov chain or an area demand, one of them")),
    }
}

/// The demand of the cells of one region (`inside`; all with a class when `None`) over `classes` (the classes
/// of every region): landschaft's `CARS._demand` with the region's counts. A Markov chain is projected over
/// its own classes and rounded over `classes`.
pub fn demand_in(
    start: &Map,
    inside: Option<&[bool]>,
    classes: &[f64],
    markov: Option<&Markov>,
    area: Option<&Demand>,
    steps: Option<usize>,
) -> Result<Demand> {
    let idx = class_index(start, classes)?;
    let k = classes.len();
    let mut counts = vec![0.0; k];
    let mut ncell = 0i64;
    for (i, &c) in idx.iter().enumerate() {
        if c >= 0 && inside.is_none_or(|m| m[i]) {
            counts[c as usize] += 1.0;
            ncell += 1;
        }
    }
    match (markov, area) {
        (Some(m), None) => {
            let extra: Vec<String> = classes
                .iter()
                .enumerate()
                .filter(|&(j, c)| !m.classes.contains(c) && counts[j] > 0.0)
                .map(|(_, &c)| class_label(c))
                .collect();
            if !extra.is_empty() {
                return Err(Error::msg(format!("the Markov chain has no rows for classes {extra:?}")));
            }
            let km = m.k();
            let start_m: Vec<f64> =
                m.classes.iter().map(|c| classes.iter().position(|x| x == c).map_or(0.0, |j| counts[j])).collect();
            let n = steps.unwrap_or(m.steps);
            let proj = ffi::markov_project(&start_m, &m.multi, km as i32, n as i32)?;
            let mut table = vec![0.0; n * k];
            for s in 0..n {
                for (jm, c) in m.classes.iter().enumerate() {
                    if let Some(j) = classes.iter().position(|x| x == c) {
                        table[s * k + j] = proj[(s + 1) * km + jm];
                    }
                }
            }
            let rounded = ffi::round_sum(&table, n as i64, k as i64, ncell)?;
            let mut rows = vec![counts];
            rows.extend(rounded.chunks(k).map(|r| r.to_vec()));
            Ok(Demand { classes: classes.to_vec(), times: (0..=n).map(|t| t as f64).collect(), table: rows })
        }
        (None, Some(d)) => {
            let n = steps.unwrap_or(d.steps());
            if n > d.steps() {
                return Err(Error::msg(format!("the demand has {} steps, not {n}", d.steps())));
            }
            let table: Vec<Vec<f64>> = (0..=n).map(|s| d.at(s, classes)).collect::<Result<_>>()?;
            for (s, row) in table.iter().enumerate() {
                if row.iter().any(|v| v.fract() != 0.0 || *v < 0.0) {
                    return Err(Error::msg("the demand must be whole, non-negative numbers of cells"));
                }
                let sum: f64 = row.iter().sum();
                if s > 0 && sum != ncell as f64 {
                    return Err(Error::msg(format!(
                        "the demand of step {s} adds up to {sum} cells, the map has {ncell}"
                    )));
                }
            }
            Ok(Demand { classes: classes.to_vec(), times: d.times[..=n].to_vec(), table })
        }
        _ => Err(Error::msg("give a Markov chain or an area demand, one of them")),
    }
}

/// CARS's demand of each region (landschaft's `CARS.simulate` with a `Regional`): the classes of all the
/// regions, each region's table from its own cells.
pub fn regional_demand(start: &Map, regional: &Regional, steps: Option<usize>) -> Result<Regional> {
    let classes = regional.classes();
    let mut parts = Vec::with_capacity(regional.parts.len());
    for (k, part) in regional.parts.iter().enumerate() {
        let inside: Vec<bool> = regional.index.iter().map(|&r| r == k as i32).collect();
        let (m, a) = match part {
            RegionDemand::Markov(m) => (Some(m.as_ref()), None),
            RegionDemand::Demand(d) => (None, Some(d.as_ref())),
        };
        let d = demand_in(start, Some(&inside), &classes, m, a, steps)
            .map_err(|e| Error::msg(format!("region {}: {e}", class_label(regional.ids[k]))))?;
        parts.push(RegionDemand::Demand(Arc::new(d)));
    }
    if parts.windows(2).any(|w| w[0].steps() != w[1].steps()) {
        return Err(Error::msg("the regions' demands have different numbers of steps"));
    }
    Ok(Regional { ids: regional.ids.clone(), index: regional.index.clone(), parts })
}

fn per_class(v: Option<&serde_json::Value>, classes: &[f64], what: &str, default: f64) -> Result<Vec<f64>> {
    match v {
        None | Some(serde_json::Value::Null) => Ok(vec![default; classes.len()]),
        Some(serde_json::Value::Number(x)) => Ok(vec![x.as_f64().unwrap(); classes.len()]),
        Some(serde_json::Value::Object(o)) => {
            let mut map = BTreeMap::new();
            for (k, x) in o {
                let c: f64 = k.trim().parse().map_err(|_| Error::msg(format!("{k:?} is not a class")))?;
                if !classes.contains(&c) {
                    return Err(Error::msg(format!("{what} for a class the model does not have: {k}")));
                }
                map.insert(k.clone(), (c, x.as_f64().ok_or_else(|| Error::msg(format!("{what} are numbers")))?));
            }
            Ok(classes.iter().map(|c| map.values().find(|(k, _)| k == c).map_or(default, |&(_, x)| x)).collect())
        }
        Some(other) => Err(Error::msg(format!("cannot read the {what} {other}"))),
    }
}

/// The demand of a CARS step: one table, or one per region (`plus.demand` of a regional demand).
#[derive(Clone, Copy)]
pub enum CarsDemand<'a> {
    One(&'a Demand),
    Regional(&'a Regional),
}

/// One step of CARS from the map at the start of the step.
#[allow(clippy::too_many_arguments)]
pub fn step(
    ctx: &mut Ctx,
    land: &Map,
    probability: &Stack,
    demand: CarsDemand<'_>,
    step: usize,
    seed: i64,
    restrictions: Option<&Map>,
    params: &serde_json::Map<String, serde_json::Value>,
) -> Result<(Map, Table)> {
    let tables: Vec<&Demand> = match demand {
        CarsDemand::One(d) => vec![d],
        CarsDemand::Regional(r) => {
            (0..r.parts.len()).map(|k| r.demand(k).map(|d| d.as_ref())).collect::<Result<_>>()?
        }
    };
    let classes = &tables[0].classes;
    let k = classes.len();
    let (ny, nx) = land.shape();
    let n = ny * nx;
    let idx = class_index(land, classes)?;
    let mut row: Vec<f64> = Vec::with_capacity(tables.len() * k);
    for d in &tables {
        if d.classes != *classes {
            return Err(Error::msg("the regions' demands must have the same classes (plus.demand)"));
        }
        row.extend(d.table.get(step).ok_or_else(|| Error::msg(format!("the demand has no step {step}")))?);
    }
    let mut prob = vec![0.0; k * n];
    let mut missing = Vec::new();
    for (j, &c) in classes.iter().enumerate() {
        match probability.get(&Key::Class(c)) {
            Some(l) => {
                for (i, &v) in l.iter().enumerate() {
                    let p = if v.is_nan() { 0.0 } else { v.clamp(f64::MIN, f64::MAX) };
                    if !(0.0..=1.0).contains(&p) {
                        return Err(Error::msg("the growth probabilities must be in [0, 1]"));
                    }
                    prob[j * n + i] = p;
                }
            }
            None => {
                if tables.iter().any(|d| d.table.windows(2).any(|w| w[1][j] > w[0][j])) {
                    missing.push(class_label(c));
                }
            }
        }
    }
    if !missing.is_empty() {
        return Err(Error::msg(format!(
            "the potential gives no growth probability for classes {missing:?}, whose demand grows"
        )));
    }
    let weights = per_class(params.get("weights"), classes, "weights", 1.0)?;
    let seeds = per_class(params.get("seeds"), classes, "seeds", 0.1)?;
    if weights.iter().chain(&seeds).any(|v| !(0.0..=1.0).contains(v)) {
        return Err(Error::msg("weights and seeds must be in [0, 1]"));
    }
    let allowed: Vec<u8> = match params.get("allowed") {
        None | Some(serde_json::Value::Null) => vec![1; k * k],
        Some(v) => {
            let m: Vec<Vec<i64>> =
                serde_json::from_value(v.clone()).map_err(|e| Error::msg(format!("allowed: {e}")))?;
            if m.len() != k || m.iter().any(|r| r.len() != k) {
                return Err(Error::msg("the conversion matrix must be square, one row and column per class"));
            }
            if m.iter().flatten().any(|&x| x != 0 && x != 1) {
                return Err(Error::msg("CARS takes a 0/1 conversion matrix"));
            }
            m.concat().into_iter().map(|x| x as u8).collect()
        }
    };
    let fixed: Vec<u8> = match restrictions {
        None => Vec::new(),
        Some(r) => r.data.iter().map(|&v| u8::from(!v.is_nan() && v != 0.0)).collect(),
    };
    let get = |name: &str, default: f64| -> Result<f64> {
        match params.get(name) {
            None => Ok(default),
            Some(v) => v.as_f64().ok_or_else(|| Error::msg(format!("{name} must be a number"))),
        }
    };
    let window = get("window", 3.0)? as i32;
    let decay = get("decay", 0.9)?;
    let step_size = get("step_size", 500.0)?;
    let max_iter = get("max_iter", 5000.0)? as i64;
    if !(decay > 0.0 && decay <= 1.0) {
        return Err(Error::msg("decay must be in (0, 1]"));
    }
    if !(3..=255).contains(&window) || window % 2 == 0 {
        return Err(Error::msg("window must be odd, from 3 to 255"));
    }
    let target: Vec<i64> = row.iter().map(|&v| v as i64).collect();
    let sub = (seed as u64).wrapping_add((step as u64).wrapping_mul(0x9E3779B97F4A7C15));
    let region: &[i32] = match demand {
        CarsDemand::One(_) => &[],
        CarsDemand::Regional(r) => &r.index,
    };
    let r = ffi::cars(
        ny as i64,
        nx as i64,
        k as i32,
        &idx,
        &fixed,
        region,
        tables.len() as i32,
        &prob,
        &weights,
        &seeds,
        &allowed,
        &target,
        window,
        decay,
        step_size,
        max_iter,
        sub,
    )?;
    let gap: i64 = target.iter().zip(&r.last).map(|(a, b)| (a - b).abs()).sum();
    let out: Vec<f64> = r.landscape.iter().map(|&c| if c >= 0 { classes[c as usize] } else { f64::NAN }).collect();
    let mut columns =
        vec!["step", "iterations", "converged", "decays", "gap"].into_iter().map(String::from).collect::<Vec<_>>();
    let mut values =
        vec![step as f64, r.iterations as f64, f64::from(u8::from(r.converged)), r.decays as f64, gap as f64];
    for (j, &c) in classes.iter().enumerate() {
        columns.push(format!("cells_{}", class_label(c)));
        values.push(r.landscape.iter().filter(|&&x| x == j as i32).count() as f64);
    }
    if !r.converged {
        ctx.warn(format!(
            "step {step}: CARS stopped after {max_iter} iterations {gap} cells away from the demand (too few cells can \
             take the classes short of it)"
        ));
    }
    Ok((Map::new(land.grid.clone(), out), Table { columns, rows: vec![values] }))
}

struct DemandOp;

impl Operator for DemandOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "plus.demand",
            "The cells of each class at each step for CARS: a Markov chain projected from the starting map (PLUS's \
             default, rounded to whole cells), or an area demand.",
            Access::Global,
        )
        .input(PortSpec::new("start", Type::Map, "the map the simulation starts from"))
        .input(PortSpec::optional("markov", Type::Markov, "a Markov chain"))
        .input(PortSpec::optional("demand", Type::Demand, "or an area demand"))
        .input(PortSpec::optional("regional", Type::Regional, "or a demand per region (Markov chains or areas)"))
        .output(PortSpec::optional("demand", Type::Demand, "cells per class and step"))
        .output(PortSpec::optional("regional", Type::Regional, "or per region and class, for a regional demand"))
        .output(PortSpec::new("steps", Type::Integer, "its steps"))
        .param(PortSpec::optional("steps", Type::Integer, "steps (default: the chain's or the demand's)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        if args.get("regional").is_some() {
            if args.get("markov").is_some() || args.get("demand").is_some() {
                return Err(Error::msg("give a Markov chain, an area demand or a regional demand, one of them"));
            }
            let r = regional_demand(args.map("start")?, args.regional("regional")?, args.opt_param("steps")?)?;
            let steps = r.steps() as i64;
            return Ok(outputs([("regional", Value::Regional(Arc::new(r))), ("steps", Value::Integer(steps))]));
        }
        let markov = match args.get("markov") {
            Some(_) => Some(args.markov("markov")?.as_ref()),
            None => None,
        };
        let area = match args.get("demand") {
            Some(_) => Some(args.demand("demand")?.as_ref()),
            None => None,
        };
        let d = demand(args.map("start")?, markov, area, args.opt_param("steps")?)?;
        let steps = d.steps() as i64;
        Ok(outputs([("demand", Value::Demand(Arc::new(d))), ("steps", Value::Integer(steps))]))
    }
}

struct StepOp;

impl Operator for StepOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "cars.step",
            "One step of PLUS's CARS: iterations from the current map until every class has the cells of the \
             step's demand, each free cell giving each class the probability of growth x neighbourhood x an \
             adaptive coefficient (random seeds where no neighbour has the class), a roulette wheel picking the \
             class and a decaying threshold deciding the change.",
            Access::Global,
        )
        .input(PortSpec::new("landscape", Type::Map, "the map at the start of the step"))
        .input(PortSpec::new("probability", Type::Stack, "the growth probability of each class (0-1)"))
        .input(PortSpec::optional("demand", Type::Demand, "the cells of each class at each step (plus.demand)"))
        .input(PortSpec::optional("regional", Type::Regional, "or per region (plus.demand of a regional demand)"))
        .input(PortSpec::new("step", Type::Integer, "the step (1, 2...)"))
        .input(PortSpec::new("seed", Type::Integer, "the seed of the simulation"))
        .input(PortSpec::optional("restrictions", Type::Map, "cells that keep their class (non-zero)"))
        .output(PortSpec::new("landscape", Type::Map, "the map at the end of the step"))
        .output(PortSpec::new("info", Type::Table, "iterations, convergence, gap and cells of each class"))
        .param(PortSpec::optional("weights", Type::Any, "neighbourhood weight of each class, 0-1 (1)"))
        .param(PortSpec::optional("seeds", Type::Any, "patch generation threshold of each class, 0-1 (0.1)"))
        .param(PortSpec::optional("decay", Type::Number, "decay factor of the threshold, (0, 1] (0.9)"))
        .param(PortSpec::optional("step_size", Type::Number, "gap reduction below which the threshold decays (500)"))
        .param(PortSpec::optional("window", Type::Integer, "side of the neighbourhood, odd (3)"))
        .param(PortSpec::optional("max_iter", Type::Integer, "iterations per step (5000)"))
        .param(PortSpec::optional("allowed", Type::Any, "0/1 conversion matrix in the demand's class order"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let k = args.integer("step")?;
        if k < 1 {
            return Err(Error::msg("the step must be >= 1"));
        }
        let (m, info) = step(
            ctx,
            args.map("landscape")?,
            args.stack("probability")?,
            match (args.get("demand"), args.get("regional")) {
                (Some(_), None) => CarsDemand::One(args.demand("demand")?.as_ref()),
                (None, Some(_)) => CarsDemand::Regional(args.regional("regional")?.as_ref()),
                _ => return Err(Error::msg(format!("node {:?}: give a demand or a regional one", args.node))),
            },
            k as usize,
            args.integer("seed")?,
            args.opt_map("restrictions")?,
            args.params,
        )?;
        Ok(outputs([("landscape", Value::Map(m)), ("info", Value::Table(info))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(DemandOp));
    r.add(Arc::new(StepOp));
}
