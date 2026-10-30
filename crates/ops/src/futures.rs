//! FUTURES' patch growing (r.futures.simulation; landschaft's `PatchGrowing`), with its flood submodel
//! (landschaft's `Flooding`): `futures.simulation`, every step in one run of landschaft's C++.
//!
//! The pieces are the model's (the potential's coefficients per subregion, the demand per subregion, the patch
//! libraries, the starting development pressure, zoning, floods); this module prepares them as
//! `PatchGrowing.simulate` does (subregions numbered in order of first appearance, the cells left out, the sum of
//! the predictor terms in floats) and runs the simulation.

use std::sync::Arc;

use serde::Deserialize;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::{FuturesClimateIn, FuturesIn, ffi};

use crate::value::class_label;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

const NULL: i32 = i32::MIN;

/// r.futures.simulation's predefined zoning effects (zone id, effect), in the module's order.
const ZONING_EFFECTS: [(i32, f32); 14] = [
    (100, 0.0),
    (101, -0.124),
    (110, -0.440),
    (120, -0.656),
    (130, -0.78),
    (131, -0.79),
    (200, -0.157),
    (201, -0.026),
    (202, -0.127),
    (203, -0.817),
    (300, -0.105),
    (301, 0.115),
    (302, -1.0),
    (0, 0.0),
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PotentialSpec {
    /// The subregion of each row (null: one row for every subregion).
    pub ids: Option<Vec<f64>>,
    pub intercept: Vec<f64>,
    pub devpressure: Vec<f64>,
    pub predictors: Vec<String>,
    /// A row per subregion, a coefficient per predictor.
    pub coefficients: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemandSpec {
    pub subregions: Vec<f64>,
    /// A row per step, a value per subregion.
    pub rows: Vec<Vec<f64>>,
    #[serde(default)]
    pub years: Option<Vec<f64>>,
    #[serde(default)]
    pub population: Option<Vec<Vec<f64>>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchLibrary {
    /// The subregion of each list (null: one list for all).
    pub ids: Option<Vec<f64>>,
    pub lists: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoningSpec {
    pub ids: Vec<f64>,
    pub stringency: Vec<f64>,
    pub zones: Vec<i64>,
    /// A row per subregion of `ids`, an effect per zone.
    pub effects: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageSpec {
    pub levels: Vec<f64>,
    pub ids: Vec<f64>,
    pub rows: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepthStep {
    pub step: i64,
    pub probabilities: Vec<f64>,
    pub layers: Vec<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbabilityStep {
    pub step: i64,
    pub layer: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Redistribution {
    pub from: Vec<f64>,
    pub to: Vec<f64>,
    /// A row per `from`, the probabilities (%) of each `to`.
    pub rows: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClimateSpec {
    pub damage: DamageSpec,
    /// null (the only curve), "subregions", "potential" or "map" (the `damage_regions` input).
    #[serde(default)]
    pub damage_regions: Option<String>,
    pub response: Vec<f64>,
    pub stddev: f64,
    #[serde(default)]
    pub depths: Option<Vec<DepthStep>>,
    #[serde(default)]
    pub probabilities: Option<Vec<ProbabilityStep>>,
    #[serde(default)]
    pub hand_percentile: Option<f64>,
    #[serde(default)]
    pub redistribution: Option<Redistribution>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub potential: PotentialSpec,
    pub demand: DemandSpec,
    pub patch_sizes: PatchLibrary,
    pub neighborhood: i32,
    pub approach: String,
    pub gamma: f64,
    pub scaling_factor: f64,
    pub discount_factor: f64,
    pub compactness_mean: f64,
    pub compactness_range: f64,
    pub num_neighbors: i32,
    pub seed_search: String,
    pub incentive_power: f64,
    #[serde(default)]
    pub zoning_effects: Option<ZoningSpec>,
    #[serde(default)]
    pub climate: Option<ClimateSpec>,
}

/// The distinct values in the order they first appear row by row (NaN left out), as whole numbers.
fn first_appearance(v: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for &x in v {
        if !x.is_nan() && !out.contains(&x) {
            out.push(x);
        }
    }
    out.into_iter().map(f64::trunc).collect()
}

/// The index of each cell's value among `ids` (-1 for none) where `keep`.
fn index_of(v: &[f64], ids: &[f64], keep: impl Fn(usize) -> bool) -> Vec<i32> {
    v.iter()
        .enumerate()
        .map(|(i, x)| if keep(i) { ids.iter().position(|y| y == x).map_or(-1, |k| k as i32) } else { -1 })
        .collect()
}

fn clip_index(k: i32) -> usize {
    k.max(0) as usize
}

/// The flood submodel's inputs (landschaft's `Flooding._inputs`); the cells it leaves out added to `null`.
#[allow(clippy::too_many_arguments)]
fn climate_inputs(
    c: &ClimateSpec,
    args: &Args,
    n: usize,
    null: &mut [bool],
    regions: &[f64],
    reg_index: &[i32],
    pregions: &[f64],
    pot_index: Option<&[i32]>,
    demand: &DemandSpec,
) -> Result<FuturesClimateIn> {
    let size = |m: &Map, what: &str| {
        if m.data.len() == n { Ok(()) } else { Err(Error::msg(format!("the {what} map differs in size"))) }
    };
    let huc = args.map("catchments")?;
    let ac = args.map("adaptive_capacity")?;
    size(huc, "catchments")?;
    size(ac, "adaptive capacity")?;
    let mut extra: Vec<bool> = ac.data.iter().map(|v| v.is_nan()).collect();
    let probability_mode = c.depths.is_none();
    if probability_mode {
        let hand = args.map("hand")?;
        size(hand, "HAND")?;
        for (e, h) in extra.iter_mut().zip(hand.data.iter()) {
            *e |= h.is_nan();
        }
    }
    let cand: Vec<bool> = (0..n).map(|i| !null[i] && !huc.data[i].is_nan()).collect();
    let masked: Vec<f64> = (0..n).map(|i| if cand[i] { huc.data[i] } else { f64::NAN }).collect();
    let ids = first_appearance(&masked);
    let huc_index = index_of(&huc.data, &ids, |i| cand[i]);
    // depth-damage regions
    let dmg = &c.damage;
    if dmg.rows.iter().any(|r| r.len() != dmg.levels.len()) || dmg.rows.len() != dmg.ids.len() || dmg.rows.is_empty() {
        return Err(Error::msg("the damage curves: a row per region, a value per level"));
    }
    let (ddf, curves): (Vec<i32>, Vec<&Vec<f64>>) = match c.damage_regions.as_deref() {
        None => (vec![0; n], vec![&dmg.rows[0]]),
        Some(src) => {
            let (order, idx): (Vec<f64>, Vec<i32>) = match src {
                "subregions" => (regions.to_vec(), reg_index.to_vec()),
                "potential" => (pregions.to_vec(), pot_index.unwrap_or(reg_index).to_vec()),
                "map" => {
                    let m = args.map("damage_regions")?;
                    size(m, "damage regions")?;
                    for i in 0..n {
                        extra[i] |= cand[i] && m.data[i].is_nan();
                    }
                    let masked: Vec<f64> = (0..n).map(|i| if cand[i] { m.data[i] } else { f64::NAN }).collect();
                    let order = first_appearance(&masked);
                    let idx = index_of(&m.data, &order, |i| cand[i]);
                    (order, idx)
                }
                s => return Err(Error::msg(format!("damage_regions: {s:?} is not subregions, potential or map"))),
            };
            let mut curves = Vec::with_capacity(order.len());
            for r in &order {
                let k = dmg
                    .ids
                    .iter()
                    .position(|x| x == r)
                    .ok_or_else(|| Error::msg(format!("no depth-damage curve for region {}", class_label(*r))))?;
                curves.push(&dmg.rows[k]);
            }
            (idx, curves)
        }
    };
    for i in 0..n {
        null[i] |= cand[i] && extra[i];
    }
    if c.response.len() != 4 {
        return Err(Error::msg("the response is (vulnerability_a, vulnerability_b, resilience_a, resilience_b)"));
    }
    let mut cl = FuturesClimateIn {
        present: true,
        depth_mode: !probability_mode,
        huc: huc_index,
        nhuc: ids.len() as i32,
        huc_ids: ids.iter().map(|&x| x as i32).collect(),
        adaptive_capacity: ac.data.iter().map(|&v| v as f32).collect(),
        levels: dmg.levels.clone(),
        damage: curves.iter().flat_map(|r| r.iter().copied()).collect(),
        ddf_region: ddf,
        response: c.response.iter().map(|&v| v as f32).collect(),
        stddev: c.stddev as f32,
        region_ids: regions.iter().map(|&r| r as i32).collect(),
        ..Default::default()
    };
    let floods = args.opt_stack("floods")?;
    let layer = |k: usize| -> Result<Vec<f32>> {
        let s = floods.ok_or_else(|| Error::msg("the floods need their maps"))?;
        let l = s.layers.get(k).ok_or_else(|| Error::msg("a flood names a layer the stack does not have"))?;
        if l.len() != n {
            return Err(Error::msg("a flood map differs in size"));
        }
        Ok(l.iter().map(|&v| v as f32).collect())
    };
    let steps: Vec<i64>;
    if let Some(depths) = &c.depths {
        let mut ds: Vec<&DepthStep> = depths.iter().collect();
        ds.sort_by_key(|d| d.step);
        let mut probs: Vec<f64> = ds.iter().flat_map(|d| d.probabilities.iter().copied()).collect();
        probs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        probs.dedup();
        cl.probabilities = probs.iter().map(|&p| p as f32).collect();
        for d in &ds {
            for p in &probs {
                let j = d
                    .probabilities
                    .iter()
                    .position(|x| x == p)
                    .ok_or_else(|| Error::msg(format!("step {} lacks depth maps for probability {p}", d.step)))?;
                cl.depths.extend(layer(d.layers[j])?);
            }
        }
        steps = ds.iter().map(|d| d.step).collect();
    } else {
        let mut ps: Vec<&ProbabilityStep> =
            c.probabilities.as_ref().ok_or_else(|| Error::msg("give depths or probabilities"))?.iter().collect();
        ps.sort_by_key(|p| p.step);
        for p in &ps {
            cl.flood_probability.extend(layer(p.layer)?);
        }
        cl.hand = args.map("hand")?.data.iter().map(|&v| v as f32).collect();
        cl.percentile =
            c.hand_percentile.ok_or_else(|| Error::msg("flood probabilities need the HAND percentile"))? as f32;
        steps = ps.iter().map(|p| p.step).collect();
    }
    cl.flood_steps = steps.iter().map(|&s| (s - 1) as i32).collect();
    if let Some(a) = args.opt_map("adaptation")? {
        size(a, "adaptation")?;
        cl.adaptation = a.data.iter().map(|&v| if v.is_nan() { NULL } else { v as i32 }).collect();
    }
    if let Some(r) = &c.redistribution {
        let pop = demand.population.as_ref().ok_or_else(|| {
            Error::msg("redistribution needs the population demand (DevelopmentDemand(population=...))")
        })?;
        if r.rows.len() != r.from.len() || r.rows.iter().any(|x| x.len() != r.to.len()) {
            return Err(Error::msg("the redistribution matrix: a row per from, a value per to"));
        }
        cl.has_matrix = true;
        cl.from_ids = r.from.iter().map(|&x| x as i32).collect();
        cl.to_ids = r.to.iter().map(|&x| x as i32).collect();
        cl.matrix = r.rows.iter().flatten().map(|&v| (v / 100.0) as f32).collect();
        for reg in regions {
            let j = demand
                .subregions
                .iter()
                .position(|x| x == reg)
                .ok_or_else(|| Error::msg(format!("no population for subregion {}", class_label(*reg))))?;
            cl.population.extend(pop.iter().map(|row| row[j] as f32));
        }
    }
    Ok(cl)
}

/// The run: the binary maps (1 developed) after each step, the development codes (the step each cell was
/// developed in, -1 undeveloped, negative abandoned), and the report per step and subregion.
pub struct Run {
    pub maps: Vec<Map>,
    pub development: Vec<Map>,
    pub info: Vec<Table>,
}

/// FUTURES as landschaft's `PatchGrowing.simulate`.
pub fn simulate(ctx: &mut Ctx, s: &Settings, args: &Args) -> Result<Run> {
    let land = args.map("landscape")?;
    let n = land.data.len();
    let dev = &land.data;
    let check = |m: &Map, what: &str| {
        if m.data.len() == n {
            Ok(())
        } else {
            Err(Error::msg(format!("the {what} map differs in size from the landscape")))
        }
    };
    let mut null: Vec<bool> = dev.iter().map(|v| v.is_nan()).collect();
    if let Some(r) = args.opt_map("restrictions")? {
        check(r, "restrictions")?;
        for (z, &v) in null.iter_mut().zip(r.data.iter()) {
            *z |= !v.is_nan() && v != 0.0;
        }
    }
    let sub: Vec<f64> = match args.opt_map("subregions")? {
        Some(m) => {
            check(m, "subregions")?;
            m.data.to_vec()
        }
        None => vec![1.0; n],
    };
    let regions = first_appearance(&sub);
    let reg_index = index_of(&sub, &regions, |_| true);
    for (z, v) in null.iter_mut().zip(&sub) {
        *z |= v.is_nan();
    }
    let (pregions, pot_index) = match args.opt_map("potential_subregions")? {
        Some(m) => {
            check(m, "potential subregions")?;
            let p = first_appearance(&m.data);
            let idx = index_of(&m.data, &p, |_| true);
            for (z, v) in null.iter_mut().zip(m.data.iter()) {
                *z |= v.is_nan();
            }
            (p, Some(idx))
        }
        None => (regions.clone(), None),
    };
    let pm = args.map("pressure")?;
    check(pm, "pressure")?;
    let pressure: Vec<f32> = pm.data.iter().map(|&v| v as f32).collect();
    for (z, v) in null.iter_mut().zip(&pressure) {
        *z |= v.is_nan();
    }
    let weight: Option<Vec<f32>> = match args.opt_map("weight")? {
        Some(m) => {
            check(m, "weight")?;
            for (z, v) in null.iter_mut().zip(m.data.iter()) {
                *z |= v.is_nan();
            }
            Some(
                m.data
                    .iter()
                    .map(|&v| {
                        let v = if v.is_nan() { 0.0 } else { v.clamp(f64::MIN, f64::MAX) };
                        v.clamp(-1.0, 1.0) as f32
                    })
                    .collect(),
            )
        }
        None => None,
    };
    let zone: Option<Vec<i32>> = match args.opt_map("zoning")? {
        Some(m) => {
            check(m, "zoning")?;
            Some(m.data.iter().map(|&v| if v.is_nan() { 0 } else { v as i32 }).collect())
        }
        None => None,
    };
    // the coefficients of each potential subregion
    let pt = &s.potential;
    let nrow = pt.intercept.len();
    if pt.devpressure.len() != nrow
        || pt.coefficients.len() != nrow
        || pt.coefficients.iter().any(|r| r.len() != pt.predictors.len())
    {
        return Err(Error::msg("the potential: a row per subregion, a coefficient per predictor"));
    }
    let row_of = |r: f64| -> Result<usize> {
        match &pt.ids {
            None => Ok(0),
            Some(ids) => ids.iter().position(|&x| x == r).ok_or_else(|| {
                Error::msg(format!("the potential has no coefficients for subregion {}", class_label(r)))
            }),
        }
    };
    let rows: Vec<usize> = pregions.iter().map(|&r| row_of(r)).collect::<Result<_>>()?;
    let intercept: Vec<f64> = rows.iter().map(|&k| pt.intercept[k]).collect();
    let dpcoef: Vec<f64> = rows.iter().map(|&k| pt.devpressure[k]).collect();
    let mut xs: Vec<Vec<f32>> = Vec::with_capacity(pt.predictors.len());
    if !pt.predictors.is_empty() {
        let drivers: &Stack = args.stack("drivers")?;
        for name in &pt.predictors {
            let l = drivers.by_name(name).ok_or_else(|| Error::msg(format!("drivers lack {name:?}")))?;
            if l.len() != n {
                return Err(Error::msg(format!("driver {name:?} differs in size from the landscape")));
            }
            xs.push(l.iter().map(|&v| v as f32).collect());
        }
    }
    let d = &s.demand;
    if d.rows.iter().any(|r| r.len() != d.subregions.len()) {
        return Err(Error::msg("the demand: a row per step, a value per subregion"));
    }
    let climate = match &s.climate {
        Some(c) => climate_inputs(c, args, n, &mut null, &regions, &reg_index, &pregions, pot_index.as_deref(), d)?,
        None => FuturesClimateIn::default(),
    };
    for x in &xs {
        for (z, v) in null.iter_mut().zip(x) {
            *z |= v.is_nan();
        }
    }
    let pidx = pot_index.as_deref().unwrap_or(&reg_index);
    let mut agg = vec![0.0f32; n];
    for (j, x) in xs.iter().enumerate() {
        let coef: Vec<f64> = rows.iter().map(|&k| pt.coefficients[k][j]).collect();
        for i in 0..n {
            if !null[i] {
                let term = (coef[clip_index(pidx[i])] * f64::from(x[i])) as f32;
                agg[i] += term;
            }
        }
    }
    let developed: Vec<i32> = (0..n)
        .map(|i| if null[i] { NULL } else { ((if dev[i].is_nan() { 0.0 } else { dev[i] }) - 1.0) as i32 })
        .collect();
    let absent: Vec<String> = regions.iter().filter(|r| !d.subregions.contains(r)).map(|&r| class_label(r)).collect();
    if !absent.is_empty() {
        return Err(Error::msg(format!("no demand for subregions {absent:?}")));
    }
    let nsteps = d.rows.len();
    let mut table = Vec::with_capacity(regions.len() * nsteps);
    for r in &regions {
        let j = d.subregions.iter().position(|x| x == r).unwrap();
        table.extend(d.rows.iter().map(|row| row[j] as f32));
    }
    // the patch libraries
    let ps = &s.patch_sizes;
    let lists: Vec<&Vec<f64>> = match &ps.ids {
        Some(ids) => regions
            .iter()
            .map(|r| {
                ids.iter()
                    .position(|x| x == r)
                    .map(|k| &ps.lists[k])
                    .ok_or_else(|| Error::msg(format!("no patch sizes for subregion {}", class_label(*r))))
            })
            .collect::<Result<_>>()?,
        None => vec![ps.lists.first().ok_or_else(|| Error::msg("give the patch sizes"))?],
    };
    let mut patch_sizes = Vec::new();
    let mut patch_offsets = vec![0];
    for l in &lists {
        let mut sizes: Vec<i32> =
            l.iter().map(|&x| ((x.trunc()) * s.discount_factor).trunc() as i32).filter(|&v| v > 0).collect();
        if sizes.is_empty() {
            sizes.push(1);
        }
        patch_sizes.extend(sizes);
        patch_offsets.push(patch_sizes.len() as i32);
    }
    let incentive: Vec<f32> = if s.incentive_power != 1.0 {
        let step = 1.0 / 1000.0;
        (0..1001).map(|i| (f64::from(i) * step).powf(s.incentive_power) as f32).collect()
    } else {
        Vec::new()
    };
    // zoning
    let (mut zone_ids, mut zone_regions, mut zone_effects, mut stringency, mut user) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), false);
    if zone.is_some() {
        let predefined = |ids: &mut Vec<i32>, regs: &mut Vec<i32>, effs: &mut Vec<f32>| {
            for (z, e) in ZONING_EFFECTS {
                ids.push(z);
                regs.push(-1);
                effs.push(e);
            }
        };
        match &s.zoning_effects {
            None => predefined(&mut zone_ids, &mut zone_regions, &mut zone_effects),
            Some(z) => {
                if z.stringency.len() != z.ids.len() || z.effects.len() != z.ids.len() {
                    return Err(Error::msg("the zoning effects: a row per subregion"));
                }
                let pos = |r: &f64| {
                    z.ids
                        .iter()
                        .position(|x| x == r)
                        .ok_or_else(|| Error::msg(format!("no zoning effects for subregion {}", class_label(*r))))
                };
                stringency = regions.iter().map(|r| pos(r).map(|k| z.stringency[k] as f32)).collect::<Result<_>>()?;
                user = !stringency.iter().all(|&v| v == 1.0);
                if z.zones.is_empty() {
                    predefined(&mut zone_ids, &mut zone_regions, &mut zone_effects);
                } else {
                    for (k, r) in regions.iter().enumerate() {
                        let row = &z.effects[pos(r)?];
                        for (c, &zid) in z.zones.iter().enumerate() {
                            zone_ids.push(zid as i32);
                            zone_regions.push(k as i32);
                            zone_effects.push(row[c] as f32);
                        }
                    }
                }
            }
        }
    }
    let approach = match s.approach.as_str() {
        "occurrence" => 0,
        "gravity" => 1,
        "kernel" => 2,
        a => return Err(Error::msg(format!("approach must be occurrence, gravity or kernel, not {a:?}"))),
    };
    let seed = args.integer("seed")?;
    let settings = FuturesIn {
        rows: land.shape().0 as i64,
        cols: land.shape().1 as i64,
        nregions: regions.len() as i32,
        npot: pregions.len() as i32,
        intercept,
        devpressure_coef: dpcoef,
        neighborhood: s.neighborhood,
        approach,
        gamma: s.gamma as f32,
        scaling: s.scaling_factor as f32,
        patch_sizes,
        patch_offsets,
        single_column: ps.ids.is_none(),
        compactness_mean: s.compactness_mean as f32,
        compactness_range: s.compactness_range as f32,
        num_neighbors: s.num_neighbors,
        seed_probability: s.seed_search == "probability",
        incentive,
        user_effects: user,
        zone_ids,
        zone_regions,
        zone_effects,
        stringency,
        demand: table,
        nsteps: nsteps as i32,
        seed,
    };
    let r = ffi::futures_run(
        &settings,
        &climate,
        &developed,
        &reg_index,
        pot_index.as_deref().unwrap_or(&[]),
        &pressure,
        &agg,
        weight.as_deref().unwrap_or(&[]),
        zone.as_deref().unwrap_or(&[]),
    )?;
    let nreg = regions.len();
    let mut maps = Vec::with_capacity(nsteps);
    let mut development = Vec::with_capacity(nsteps);
    let mut info = Vec::with_capacity(nsteps);
    for k in 0..nsteps {
        let codes = &r.series[k * n..(k + 1) * n];
        let coded: Vec<f64> = codes.iter().map(|&c| if c == NULL { f64::NAN } else { f64::from(c) }).collect();
        maps.push(Map::new(
            land.grid.clone(),
            coded.iter().map(|&c| if c.is_nan() { f64::NAN } else { f64::from(u8::from(c >= 0.0)) }).collect(),
        ));
        development.push(Map::new(land.grid.clone(), coded));
        let mut columns = vec!["step".to_string(), "year".to_string()];
        let year = d.years.as_ref().and_then(|y| y.get(k).copied()).unwrap_or((k + 1) as f64);
        let mut row = vec![(k + 1) as f64, year];
        for (j, &rid) in regions.iter().enumerate() {
            columns.push(format!("developed_{}", class_label(rid)));
            row.push(r.converted[k * nreg + j] as f64);
            columns.push(format!("overflow_{}", class_label(rid)));
            row.push(r.overflow[k * nreg + j] as f64);
            let w = r.warnings[k * nreg + j];
            let mut msgs = Vec::new();
            if w & 1 != 0 {
                msgs.push("not enough undeveloped cells, all developed");
            }
            if w & 2 != 0 {
                msgs.push("too many failed attempts to find a seed");
            }
            if !msgs.is_empty() {
                ctx.warn(format!("step {}, subregion {}: {}", k + 1, class_label(rid), msgs.join("; ")));
            }
        }
        info.push(Table { columns, rows: vec![row], ..Default::default() });
    }
    Ok(Run { maps, development, info })
}

pub(crate) struct Simulation;

impl Operator for Simulation {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "futures.simulation",
            "FUTURES' patch growing (r.futures.simulation): at every step each subregion develops its demand in \
             patches grown from seeds drawn on the development probability (the potential's coefficients, the \
             development pressure updated around every new cell, zoning, incentives), with the flood submodel \
             (damage, retreat, adaptation, migration) when given.",
            Access::Global,
        )
        .input(PortSpec::new("landscape", Type::Map, "developed cells at the start (1 developed, 0 not)"))
        .input(PortSpec::new("seed", Type::Integer, "the seed of the simulation"))
        .input(PortSpec::new("pressure", Type::Map, "the development pressure at the start"))
        .input(PortSpec::optional("drivers", Type::Stack, "the potential's predictors, by name"))
        .input(PortSpec::optional("subregions", Type::Map, "the subregions (default: one)"))
        .input(PortSpec::optional("potential_subregions", Type::Map, "the subregions of the coefficients"))
        .input(PortSpec::optional("weight", Type::Map, "-1 to 1, changing the probability"))
        .input(PortSpec::optional("zoning", Type::Map, "zone ids"))
        .input(PortSpec::optional("restrictions", Type::Map, "cells never developed (non-zero)"))
        .input(PortSpec::optional("catchments", Type::Map, "floods: the catchments (HUCs)"))
        .input(PortSpec::optional("adaptive_capacity", Type::Map, "floods: adaptive capacity, -1 to 1"))
        .input(PortSpec::optional("hand", Type::Map, "floods: height above the nearest drainage"))
        .input(PortSpec::optional("floods", Type::Stack, "floods: the depth or probability maps"))
        .input(PortSpec::optional("damage_regions", Type::Map, "floods: the depth-damage region of each cell"))
        .input(PortSpec::optional("adaptation", Type::Map, "floods: the return period cells are protected against"))
        .output(PortSpec::new("maps", Type::Series, "developed (1) or not (0) after each step"))
        .output(PortSpec::new("development", Type::Series, "the step each cell was developed in (-1 never)"))
        .output(PortSpec::new("info", Type::Series, "cells developed and carried over, per step and subregion"))
        .param(PortSpec::new("potential", Type::Any, "{ ids, intercept, devpressure, predictors, coefficients }"))
        .param(PortSpec::new("demand", Type::Any, "{ subregions, rows, years, population }"))
        .param(PortSpec::new("patch_sizes", Type::Any, "{ ids, lists }"))
        .param(PortSpec::new("neighborhood", Type::Integer, "the radius of the pressure's window (cells)"))
        .param(PortSpec::new("approach", Type::Text, "the pressure's decay").choices([
            "gravity",
            "occurrence",
            "kernel",
        ]))
        .param(PortSpec::new("gamma", Type::Number, "the pressure's decay exponent"))
        .param(PortSpec::new("scaling_factor", Type::Number, "the pressure's scaling factor"))
        .param(PortSpec::new("discount_factor", Type::Number, "multiplies the patch sizes"))
        .param(PortSpec::new("compactness_mean", Type::Number, "the patches' distance exponent"))
        .param(PortSpec::new("compactness_range", Type::Number, "its range"))
        .param(PortSpec::new("num_neighbors", Type::Integer, "4 or 8").choices(["4", "8"]))
        .param(PortSpec::new("seed_search", Type::Text, "how seeds are drawn").choices(["probability", "random"]))
        .param(PortSpec::new("incentive_power", Type::Number, "infill (> 1) or sprawl (< 1)"))
        .param(PortSpec::optional("zoning_effects", Type::Any, "{ ids, stringency, zones, effects }"))
        .param(PortSpec::optional("climate", Type::Any, "the flood submodel"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s: Settings = serde_json::from_value(serde_json::Value::Object(args.params.clone()))
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let r = simulate(ctx, &s, args)?;
        Ok(outputs([
            ("maps", Value::Series(r.maps.into_iter().map(Value::Map).collect())),
            ("development", Value::Series(r.development.into_iter().map(Value::Map).collect())),
            ("info", Value::Series(r.info.into_iter().map(Value::Table).collect())),
        ]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Simulation));
}
