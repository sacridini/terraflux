//! Demands: the cells each class must cover at each step (`demand.from_maps`, `demand.table`).

use std::sync::Arc;

use serde::Deserialize;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::markov::{classes_of, crosstab};
use crate::value::{RegionDemand, Regional};
use crate::{Args, Ctx, Demand, Error, Key, Map, Markov, Operator, Outputs, Registry, Result, Stack, Value, outputs};

/// The times of the maps of a series: given, else the band names when they are all numbers, else 0, 1, 2...
pub fn series_times(stack: &Stack, times: Option<Vec<f64>>) -> Result<Vec<f64>> {
    let n = stack.layers.len();
    let t = match times {
        Some(t) => t,
        None => {
            let parsed: Option<Vec<f64>> = stack
                .keys
                .iter()
                .map(|k| match k {
                    Key::Name(s) => s.trim().parse::<f64>().ok(),
                    _ => None,
                })
                .collect();
            parsed.unwrap_or_else(|| (0..n).map(|i| i as f64).collect())
        }
    };
    if t.len() != n {
        return Err(Error::msg(format!("{} times for {n} maps", t.len())));
    }
    if t.windows(2).any(|w| w[1] <= w[0]) {
        return Err(Error::msg("times must increase"));
    }
    Ok(t)
}

/// The areas of observed maps interpolated and extrapolated linearly (lulcc's `approxExtrapDemand`,
/// landschaft's `AreaDemand.from_maps`): each class's cells at `out_times` (default every whole time from
/// the first map to the last), negative values set to 0, each time rounded to whole cells adding up to the
/// cells of the first map.
pub fn from_maps(maps: &[Map], times: &[f64], out_times: Option<Vec<f64>>) -> Result<Demand> {
    if maps.len() < 2 {
        return Err(Error::msg("at least two maps are needed"));
    }
    let refs: Vec<&Map> = maps.iter().collect();
    let classes = classes_of(&refs);
    let k = classes.len();
    let mut areas = Vec::with_capacity(maps.len());
    for m in maps {
        let c = crosstab(m, m, &classes)?;
        areas.push((0..k).map(|i| c[i * k + i]).collect::<Vec<f64>>());
    }
    let tout = out_times.unwrap_or_else(|| {
        let (first, last) = (times[0], times[times.len() - 1]);
        let n = (last + 1.0 - first).ceil().max(0.0) as usize; // np.arange(first, last + 1)
        (0..n).map(|i| first + i as f64).collect()
    });
    let mut demand = vec![0.0; tout.len() * k];
    for (j, _) in classes.iter().enumerate() {
        let y: Vec<f64> = areas.iter().map(|a| a[j]).collect();
        let v = ffi::approx_extrap(times, &y, &tout)?;
        for (i, x) in v.into_iter().enumerate() {
            demand[i * k + j] = if x < 0.0 { 0.0 } else { x };
        }
    }
    let ncell: f64 = areas[0].iter().sum();
    let rounded = ffi::round_sum(&demand, tout.len() as i64, k as i64, ncell as i64)?;
    let table = rounded.chunks(k).map(|r| r.to_vec()).collect();
    Ok(Demand { classes, times: tout, table })
}

struct FromMaps;

impl Operator for FromMaps {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "demand.from_maps",
            "The cells of each class at each step, interpolated and extrapolated linearly from observed maps \
             (lulcc's approxExtrapDemand): negative areas set to 0, every step rounded to whole cells that add up \
             to the cells of the first map.",
            Access::Global,
        )
        .input(PortSpec::new("maps", Type::Stack, "the observed maps, one band per date"))
        .output(PortSpec::new("demand", Type::Demand, "the demand"))
        .output(PortSpec::new("steps", Type::Integer, "its steps (the dates after the first)"))
        .param(PortSpec::optional("times", Type::Any, "the date of each map (default: the band names)"))
        .param(PortSpec::optional(
            "out_times",
            Type::Any,
            "the dates of the demand (default: every whole date)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = args.stack("maps")?;
        let times = series_times(s, args.opt_param("times")?)?;
        let maps: Vec<Map> = s.layers.iter().map(|l| Map { grid: s.grid.clone(), data: l.clone() }).collect();
        let d = from_maps(&maps, &times, args.opt_param("out_times")?)?;
        let steps = d.steps() as i64;
        Ok(outputs([("demand", Value::Demand(Arc::new(d))), ("steps", Value::Integer(steps))]))
    }
}

#[derive(Deserialize)]
struct TableSpec {
    classes: Vec<f64>,
    rows: Vec<Vec<f64>>,
    #[serde(default)]
    times: Option<Vec<f64>>,
}

struct FromTable;

impl Operator for FromTable {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "demand.table",
            "A demand given as a table: `classes` and one row of cells per step (the first the start).",
            Access::None,
        )
        .output(PortSpec::new("demand", Type::Demand, "the demand"))
        .output(PortSpec::new("steps", Type::Integer, "its steps"))
        .param(PortSpec::new("table", Type::Any, "{ classes = [...], rows = [[...], ...], times = [...] }"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let t: TableSpec =
            args.opt_param("table")?.ok_or_else(|| Error::msg(format!("node {:?}: give the table", args.node)))?;
        if t.rows.len() < 2 || t.rows.iter().any(|r| r.len() != t.classes.len()) {
            return Err(Error::msg("a demand needs the start and one step, each a value per class"));
        }
        let times = t.times.unwrap_or_else(|| (0..t.rows.len()).map(|i| i as f64).collect());
        let steps = t.rows.len() as i64 - 1;
        Ok(outputs([
            ("demand", Value::Demand(Arc::new(Demand { classes: t.classes, times, table: t.rows }))),
            ("steps", Value::Integer(steps)),
        ]))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkovSpec {
    classes: Vec<f64>,
    single: Vec<Vec<f64>>,
    #[serde(default)]
    multi: Option<Vec<Vec<f64>>>,
    #[serde(default = "one")]
    steps: usize,
}

fn one() -> usize {
    1
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum PartSpec {
    Table(TableSpec),
    Markov(MarkovSpec),
}

fn part(spec: PartSpec) -> Result<RegionDemand> {
    Ok(match spec {
        PartSpec::Table(t) => {
            if t.rows.len() < 2 || t.rows.iter().any(|r| r.len() != t.classes.len()) {
                return Err(Error::msg("a demand needs the start and one step, each a value per class"));
            }
            let times = t.times.unwrap_or_else(|| (0..t.rows.len()).map(|i| i as f64).collect());
            RegionDemand::Demand(Arc::new(Demand { classes: t.classes, times, table: t.rows }))
        }
        PartSpec::Markov(m) => {
            let k = m.classes.len();
            if m.single.len() != k || m.single.iter().any(|r| r.len() != k) {
                return Err(Error::msg("the transition matrix must be square, a row and column per class"));
            }
            if m.steps < 1 {
                return Err(Error::msg("steps must be >= 1"));
            }
            let single = m.single.concat();
            let multi = match m.multi {
                Some(x) if x.len() == k && x.iter().all(|r| r.len() == k) => x.concat(),
                Some(_) => return Err(Error::msg("multi must be square, a row and column per class")),
                None => ffi::markov_root(&single, k as i32, m.steps as i32)?,
            };
            RegionDemand::Markov(Arc::new(Markov { classes: m.classes, single, multi, steps: m.steps }))
        }
    })
}

/// A demand per region from the map of region ids and each region's demand (in the order of `ids`).
pub fn regional(regions: &Map, ids: Vec<f64>, parts: Vec<RegionDemand>) -> Result<Regional> {
    if ids.is_empty() || ids.len() != parts.len() {
        return Err(Error::msg("give a demand per region, one per id"));
    }
    if ids.iter().any(|v| v.fract() != 0.0) {
        return Err(Error::msg("the region ids must be integers"));
    }
    let mut order: Vec<usize> = (0..ids.len()).collect();
    order.sort_by(|&a, &b| ids[a].partial_cmp(&ids[b]).unwrap());
    if order.windows(2).any(|w| ids[w[0]] == ids[w[1]]) {
        return Err(Error::msg("the region ids must be distinct"));
    }
    let ids: Vec<f64> = order.iter().map(|&k| ids[k]).collect();
    let parts: Vec<RegionDemand> = order.iter().map(|&k| parts[k].clone()).collect();
    if parts.windows(2).any(|w| w[0].steps() != w[1].steps()) {
        let mut steps: Vec<usize> = parts.iter().map(RegionDemand::steps).collect();
        steps.sort_unstable();
        steps.dedup();
        return Err(Error::msg(format!("the regions' demands have different numbers of steps: {steps:?}")));
    }
    if regions.data.iter().any(|v| !v.is_nan() && v.fract() != 0.0) {
        return Err(Error::msg("the region ids must be integers"));
    }
    let index = regions.data.iter().map(|v| ids.iter().position(|id| id == v).map_or(-1, |k| k as i32)).collect();
    Ok(Regional { ids, index, parts })
}

struct RegionalOp;

impl Operator for RegionalOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "demand.regional",
            "A demand per region (municipalities, biomes, basins...): each region's cells meet its own demand, cells              of no region keep their class. The demand of a region is a table of cells per class and step (CLUE-S,              ordered allocation, CARS) or a Markov chain (Expander and Patcher, CARS).",
            Access::Global,
        )
        .input(PortSpec::new("regions", Type::Map, "the region id of each cell"))
        .output(PortSpec::new("regional", Type::Regional, "the demand of each region"))
        .output(PortSpec::new("steps", Type::Integer, "their steps"))
        .param(PortSpec::new("ids", Type::Any, "the region ids"))
        .param(PortSpec::new(
            "demands",
            Type::Any,
            "per id: { table = { classes, rows, times } } or { markov = { classes, single, multi, steps } }",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let ids: Vec<f64> =
            args.opt_param("ids")?.ok_or_else(|| Error::msg(format!("node {:?}: give the ids", args.node)))?;
        let specs: Vec<PartSpec> =
            args.opt_param("demands")?.ok_or_else(|| Error::msg(format!("node {:?}: give the demands", args.node)))?;
        let parts = specs.into_iter().map(part).collect::<Result<Vec<_>>>()?;
        let r = regional(args.map("regions")?, ids, parts)?;
        let steps = r.steps() as i64;
        Ok(outputs([("regional", Value::Regional(Arc::new(r))), ("steps", Value::Integer(steps))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(FromMaps));
    r.add(Arc::new(FromTable));
    r.add(Arc::new(RegionalOp));
}
