//! Dinamica EGO's functors of landschaft 1.50 (its `ls.simulate`): the Patcher of a continuous state
//! (PatcherContinuousState: new patches that take a stock up to a quota) and the endogenous road constructor of
//! the Amazon models (Soares-Filho et al. 2004): the cost map (CalcCostMap), destinations drawn in quadrants
//! (CalcDestinationMap), pathways down the costs (CalcPathwayMap) and a step of road building, through
//! landschaft's C++, the same bits.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(ContinuousPatcher));
    r.add(Arc::new(CostMap));
    r.add(Arc::new(Destinations));
    r.add(Arc::new(Pathways));
    r.add(Arc::new(Roads));
}

fn fail<'a>(args: &'a Args<'_>) -> impl Fn(Error) -> Error + 'a {
    move |e| Error::msg(format!("node {:?}: {e}", args.node))
}

/// The seed of a step of a simulation (landschaft's: seed + step x 0x9E3779B9), the seed itself without a step.
fn step_seed(args: &Args, mask: u64) -> Result<u64> {
    let seed = if args.get("seed").is_some() { args.integer("seed")? } else { 1 };
    let step = if args.get("step").is_some() { args.integer("step")? } else { 0 };
    Ok((seed as u64).wrapping_add((step as u64).wrapping_mul(0x9E37_79B9)) & mask)
}

/// Cells with a value other than 0 (no value: absent).
fn present(v: &[f64]) -> Vec<u8> {
    v.iter().map(|x| u8::from(!x.is_nan() && *x != 0.0)).collect()
}

fn same_shape(a: &Map, b: &Map, what: &str) -> Result<()> {
    if a.shape() != b.shape() {
        return Err(Error::msg(format!("{what} has shape {:?}, the first map {:?}", b.shape(), a.shape())));
    }
    Ok(())
}

/// Cell width and height (map units).
fn res(g: &Grid) -> (f64, f64) {
    (g.transform[1].abs(), g.transform[5].abs())
}

fn map(grid: &Arc<Grid>, v: Vec<f64>) -> Value {
    Value::Map(Map::new(grid.clone(), v))
}

// -- the Patcher of a continuous state --------------------------------------------------------------------------

struct ContinuousPatcher;

impl Operator for ContinuousPatcher {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "dinamica.continuous_patcher",
            "Dinamica EGO's Patcher of a continuous state (PatcherContinuousState), landschaft's ContinuousPatcher: \
             new patches (logging, clearing, harvest) that take the stock of their cells (biomass, carbon, timber) \
             until a quota is met. Seeds drawn among the cells with stock and probability ranked from the highest \
             (prune_factor x quota of stock), in proportion to the probability; lognormal patch sizes grown as the \
             Patcher's.",
            Access::Global,
        )
        .input(PortSpec::new("stock", Type::Map, "the stock of each cell (per hectare, cell or area unit: unit)"))
        .input(PortSpec::new("probability", Type::Map, "the probability of each cell (no value or <= 0: never)"))
        .input(PortSpec::new("quota", Type::Number, "the stock to take, in the stock's units times the area unit"))
        .input(PortSpec::optional("step", Type::Integer, "the step of a simulation (1, 2...; none: the seed as is)"))
        .input(PortSpec::optional("seed", Type::Integer, "the seed (1)"))
        .output(PortSpec::new("stock", Type::Map, "the stock left (0 where taken)"))
        .output(PortSpec::new("probability", Type::Map, "the probability, 0 where taken"))
        .output(PortSpec::new("taken", Type::Map, "the stock taken from each cell"))
        .output(PortSpec::new("patch", Type::Map, "the patch of each cell taken (1, 2...), 0 elsewhere"))
        .output(PortSpec::new("info", Type::Table, "quota, taken, remaining (negative: exceeded), patches, cells"))
        .param(PortSpec::optional("mean", Type::Number, "the mean patch size, in unit (1)"))
        .param(PortSpec::optional("variance", Type::Number, "the variance of the patch sizes (0)"))
        .param(PortSpec::optional("isometry", Type::Number, "0 single cells, 1 the probabilities, 2 compact (1)"))
        .param(
            PortSpec::optional(
                "unit",
                Type::Text,
                "of the patch sizes and the stock's area: hectares (default, Dinamica's), cells or area",
            )
            .choices(["hectares", "cells", "area"]),
        )
        .param(PortSpec::optional("prune_factor", Type::Number, "the stock ranked, times the quota (10)"))
        .param(PortSpec::optional(
            "window",
            Type::Any,
            "the neighbour window of growth [rows, columns] ([3, 3])",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        continuous(args).map_err(fail(args))
    }
}

fn continuous(args: &Args) -> Result<Outputs> {
    let (mean, variance, isometry, prune): (f64, f64, f64, f64) = (
        args.param("mean", 1.0)?,
        args.param("variance", 0.0)?,
        args.param("isometry", 1.0)?,
        args.param("prune_factor", 10.0)?,
    );
    let unit: String = args.param("unit", "hectares".to_string())?;
    if !["hectares", "cells", "area"].contains(&unit.as_str()) {
        return Err(Error::msg("unit must be one of hectares, cells, area"));
    }
    if mean.is_nan() || mean <= 0.0 {
        return Err(Error::msg("the mean patch size must be positive"));
    }
    if variance.is_nan() || variance < 0.0 {
        return Err(Error::msg("the variance must be >= 0"));
    }
    if !(0.0..=2.0).contains(&isometry) {
        return Err(Error::msg("the isometry must be between 0 and 2"));
    }
    if prune.is_nan() || prune <= 0.0 {
        return Err(Error::msg("prune_factor must be positive"));
    }
    let w: (i32, i32) = args.param("window", (3, 3))?;
    if w.0 < 1 || w.1 < 1 || w.0 % 2 == 0 || w.1 % 2 == 0 {
        return Err(Error::msg("the window needs two odd sides"));
    }
    let stock = args.map("stock")?;
    let prob = args.map("probability")?;
    same_shape(stock, prob, "the probability map")?;
    let quota = args.number("quota")?;
    let area = match unit.as_str() {
        "cells" => 1.0,
        u => {
            let (rx, ry) = res(&stock.grid);
            if u == "hectares" { rx * ry / 1e4 } else { rx * ry }
        }
    };
    let seed = step_seed(args, u64::MAX)?;
    let (ny, nx) = stock.shape();
    let scaled: Vec<f64> = stock.data.iter().map(|v| v * area).collect();
    let r = ffi::sim_continuous_patches(
        &scaled,
        &prob.data,
        ny as i64,
        nx as i64,
        quota,
        mean / area,
        variance / (area * area),
        isometry,
        prune,
        w.0,
        w.1,
        seed,
    )?;
    let hit: Vec<bool> = r.taken.iter().map(|t| *t > 0.0).collect();
    let left = stock.data.iter().zip(&hit).map(|(v, h)| if *h { 0.0 } else { *v }).collect();
    let p_out = prob.data.iter().zip(&hit).map(|(v, h)| if *h { 0.0 } else { *v }).collect();
    let mut info = Table::new(["quota", "taken", "remaining", "patches", "cells"].map(String::from).to_vec());
    info.rows.push(vec![quota, crate::change::numpy_sum(&r.taken), r.remaining, r.patches as f64, r.cells as f64]);
    let grid = &stock.grid;
    Ok(outputs([
        ("stock", map(grid, left)),
        ("probability", map(grid, p_out)),
        ("taken", map(grid, r.taken.clone())),
        ("patch", map(grid, r.patch.iter().map(|&p| f64::from(p)).collect())),
        ("info", Value::Table(info)),
    ]))
}

// -- roads --------------------------------------------------------------------------------------------------

/// The friction of a cost map: positive (no value: impassable), times the cell size when relative.
fn friction(f: &Map, relative: bool) -> Result<Vec<f64>> {
    if f.data.iter().any(|v| *v <= 0.0) {
        return Err(Error::msg("frictions must be positive (no value: impassable)"));
    }
    if !relative {
        return Ok(f.data.to_vec());
    }
    let (rx, ry) = res(&f.grid);
    if (rx - ry).abs() > 1e-9 * rx.abs().max(ry.abs()) {
        return Err(Error::msg("relative frictions need square cells"));
    }
    Ok(f.data.iter().map(|v| v * rx).collect())
}

/// landschaft's friction_cost: the cost from the nearest source and the predecessors' directions (255 where no
/// source is reached).
fn cost_map(
    sources: &[u8],
    friction: &[f64],
    (ny, nx): (usize, usize),
    diagonal: bool,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let d = if diagonal { 2f64.sqrt() } else { 1.0 };
    let r = ffi::sim_friction_cost(sources, friction, ny as i64, nx as i64, d, true)?;
    let dirs = r.dir.iter().zip(&r.cost).map(|(&k, c)| if c.is_nan() { 255.0 } else { f64::from(k) }).collect();
    Ok((r.cost, dirs))
}

struct CostMap;

impl Operator for CostMap {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "dinamica.cost_map",
            "The cost of reaching every cell from the nearest source over a friction surface, Dinamica EGO's \
             CalcCostMap (landschaft's friction_cost): a step into a cell costs its friction (x sqrt 2 on a \
             diagonal), 8 neighbours, Dijkstra's sums; and the direction of each cell's predecessor (1 NW, 2 N, 3 \
             NE, 4 E, 5 SE, 6 S, 7 SW, 8 W; 0 at the sources, 255 unreached).",
            Access::Global,
        )
        .input(PortSpec::new("sources", Type::Map, "the sources (a value other than 0)"))
        .input(PortSpec::new("friction", Type::Map, "the effort to cross each cell (> 0; no value: impassable)"))
        .output(PortSpec::new("cost", Type::Map, "the cost (no value where unreached)"))
        .output(PortSpec::new("direction", Type::Map, "the predecessors' directions"))
        .param(PortSpec::optional("diagonal", Type::Bool, "diagonal steps cost sqrt(2) times more (default true)"))
        .param(PortSpec::optional("relative", Type::Bool, "frictions per map unit, times the cell size"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let r = || -> Result<Outputs> {
            let s = args.map("sources")?;
            let f = args.map("friction")?;
            same_shape(s, f, "the friction")?;
            let fr = friction(f, args.param("relative", false)?)?;
            let (cost, dirs) = cost_map(&present(&s.data), &fr, s.shape(), args.param("diagonal", true)?)?;
            Ok(outputs([("cost", map(&s.grid, cost)), ("direction", map(&s.grid, dirs))]))
        };
        r().map_err(fail(args))
    }
}

/// landschaft's destinations: the attraction of the cells drawn, no value elsewhere.
fn destinations(
    att: &Map,
    values: &[f64],
    q: (i32, i32),
    per: i32,
    min_distance: f64,
    attempts: i32,
    seed: u64,
) -> Result<Vec<f64>> {
    if q.0 < 1 || q.1 < 1 {
        return Err(Error::msg("quadrants must be at least [1, 1]"));
    }
    if per < 0 || attempts < 1 {
        return Err(Error::msg("per_quadrant must be >= 0 and max_attempts >= 1"));
    }
    let (ny, nx) = att.shape();
    let (dx, dy) = res(&att.grid);
    let idx = ffi::sim_destinations(values, ny as i64, nx as i64, q.0, q.1, per, min_distance, attempts, dx, dy, seed)?;
    let mut out = vec![f64::NAN; values.len()];
    for i in idx {
        out[i as usize] = values[i as usize];
    }
    Ok(out)
}

struct Destinations;

impl Operator for Destinations {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "dinamica.destinations",
            "Cells drawn by attraction in quadrants of a map, Dinamica EGO's CalcDestinationMap (landschaft's \
             destinations): in each of the horizontal x vertical parts, up to per_quadrant cells in proportion to \
             their attraction, a draw closer than min_distance to an earlier one drawn again.",
            Access::Global,
        )
        .input(PortSpec::new("attraction", Type::Map, "how attractive each cell is (no value or <= 0: never)"))
        .input(PortSpec::optional("seed", Type::Integer, "the seed (1)"))
        .output(PortSpec::new("map", Type::Map, "the attraction of the destinations, no value elsewhere"))
        .param(PortSpec::optional("quadrants", Type::Any, "[horizontal, vertical] divisions ([1, 1])"))
        .param(PortSpec::optional("per_quadrant", Type::Integer, "destinations per quadrant (1)"))
        .param(PortSpec::optional("min_distance", Type::Number, "between destinations, map units (0)"))
        .param(PortSpec::optional("max_attempts", Type::Integer, "draws of a destination (15)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let r = || -> Result<Outputs> {
            let a = args.map("attraction")?;
            let out = destinations(
                a,
                &a.data,
                args.param("quadrants", (1, 1))?,
                args.param("per_quadrant", 1)?,
                args.param("min_distance", 0.0)?,
                args.param("max_attempts", 15)?,
                step_seed(args, u64::MAX)?,
            )?;
            Ok(outputs([("map", map(&a.grid, out))]))
        };
        r().map_err(fail(args))
    }
}

/// landschaft's pathways: the network with the new paths (or the paths alone), no value elsewhere.
#[allow(clippy::too_many_arguments)]
fn pathways(
    network: &Map,
    sources: &[f64],
    cost: Option<&[f64]>,
    dirs: Option<&[f64]>,
    lottery: bool,
    merge: bool,
    value: f64,
    seed: u64,
) -> Result<Vec<f64>> {
    let (ny, nx) = network.shape();
    let d: Vec<u8> =
        dirs.map(|d| d.iter().map(|v| if v.is_nan() { 255 } else { *v as u8 }).collect()).unwrap_or_default();
    let net = present(&network.data);
    let new = ffi::sim_pathways(&net, &present(sources), cost.unwrap_or(&[]), &d, ny as i64, nx as i64, lottery, seed)?;
    Ok((0..new.len())
        .map(|i| {
            if new[i] > 0 {
                value
            } else if merge && net[i] > 0 {
                network.data[i]
            } else {
                f64::NAN
            }
        })
        .collect())
}

struct Pathways;

impl Operator for Pathways {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "dinamica.pathways",
            "Paths from source cells to a network, Dinamica EGO's CalcPathwayMap (landschaft's pathways): from each \
             source (row order) to the cheapest of its 8 neighbours until a cell of cost <= 0 (the network), or \
             along the directions of a cost map until the network.",
            Access::Global,
        )
        .input(PortSpec::new("network", Type::Map, "the network (a value other than 0)"))
        .input(PortSpec::new("sources", Type::Map, "where paths start (a value other than 0), e.g. destinations"))
        .input(PortSpec::optional("cost", Type::Map, "the cost of reaching the network (<= 0 on it)"))
        .input(PortSpec::optional("directions", Type::Map, "or the directions of a cost map"))
        .input(PortSpec::optional("seed", Type::Integer, "for the lottery (1)"))
        .output(PortSpec::new("map", Type::Map, "the network with the new paths (no value elsewhere)"))
        .param(PortSpec::optional("lottery", Type::Bool, "ties between cheapest neighbours drawn at random"))
        .param(PortSpec::optional("merge", Type::Bool, "the network with the new paths (default true), or only them"))
        .param(PortSpec::optional("value", Type::Number, "of the new path cells (1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let r = || -> Result<Outputs> {
            let n = args.map("network")?;
            let s = args.map("sources")?;
            same_shape(n, s, "the sources")?;
            let (c, d) = (args.opt_map("cost")?, args.opt_map("directions")?);
            if c.is_some() == d.is_some() {
                return Err(Error::msg("give the cost or the directions"));
            }
            for m in [c, d].into_iter().flatten() {
                same_shape(n, m, "the cost or direction map")?;
            }
            let out = pathways(
                n,
                &s.data,
                c.map(|m| m.data.as_slice()),
                d.map(|m| m.data.as_slice()),
                args.param("lottery", false)?,
                args.param("merge", true)?,
                args.param("value", 1.0)?,
                step_seed(args, u64::MAX)?,
            )?;
            Ok(outputs([("map", map(&n.grid, out))]))
        };
        r().map_err(fail(args))
    }
}

/// The exact Euclidean distance (map units) to the present cells; infinite without any.
fn distance(present: &[u8], (ny, nx): (usize, usize), cell: f64) -> Result<Vec<f64>> {
    if !present.iter().any(|&p| p > 0) {
        return Ok(vec![f64::INFINITY; present.len()]);
    }
    Ok(ffi::nlm_distance(present, ny as i64, nx as i64, 2, cell, 0.0, 0.0, 1.0, 1.0)?)
}

struct Roads;

impl Operator for Roads {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "dinamica.roads",
            "A step of endogenous road building, Dinamica EGO's road constructor (landschaft's RoadBuilder.step): \
             the attraction kept where the distance to the roads is within distance (and the mask allows), \
             destinations drawn in quadrants, each joined to the network down the cost of reaching it over the \
             friction; the network with the new roads, and the distance to them (a dynamic variable).",
            Access::Global,
        )
        .input(PortSpec::new("network", Type::Map, "the roads (a value other than 0)"))
        .input(PortSpec::new("friction", Type::Map, "the effort to build across each cell (> 0; no value: never)"))
        .input(PortSpec::new("attraction", Type::Map, "how attractive each cell is as a road's end"))
        .input(PortSpec::optional("mask", Type::Map, "where roads may end (non-zero)"))
        .input(PortSpec::optional("step", Type::Integer, "the step of a simulation (1, 2...; none: the seed as is)"))
        .input(PortSpec::optional("seed", Type::Integer, "the seed (1)"))
        .output(PortSpec::new("network", Type::Map, "the network with the new roads"))
        .output(PortSpec::new("distance", Type::Map, "the distance to the roads, map units"))
        .param(PortSpec::optional("quadrants", Type::Any, "[horizontal, vertical] divisions ([2, 10])"))
        .param(PortSpec::optional(
            "distance",
            Type::Any,
            "[least, most] distance from the roads of a new end ([1500, 3000])",
        ))
        .param(PortSpec::optional("spacing", Type::Number, "the least distance between destinations of a step (1000)"))
        .param(PortSpec::optional("per_quadrant", Type::Integer, "destinations per quadrant (1)"))
        .param(PortSpec::optional("max_attempts", Type::Integer, "draws of a destination (15)"))
        .param(PortSpec::optional("lottery", Type::Bool, "ties drawn at random (default true)"))
        .param(PortSpec::optional("diagonal", Type::Bool, "diagonal steps cost sqrt(2) times more (default true)"))
        .param(PortSpec::optional("relative", Type::Bool, "frictions per map unit"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        roads(args).map_err(fail(args))
    }
}

fn roads(args: &Args) -> Result<Outputs> {
    let n = args.map("network")?;
    let f = args.map("friction")?;
    let a = args.map("attraction")?;
    same_shape(n, f, "the friction")?;
    same_shape(n, a, "the attraction")?;
    let (rx, ry) = res(&n.grid);
    if (rx - ry).abs() > 1e-9 * rx.max(ry) {
        return Err(Error::msg("road building needs square cells"));
    }
    let seed = step_seed(args, 0x7FFF_FFFF_FFFF_FFFF)?;
    let shape = n.shape();
    let net = present(&n.data);
    let dist = distance(&net, shape, rx)?;
    let (lo, hi): (f64, f64) = args.param("distance", (1500.0, 3000.0))?;
    let mask = args.opt_map("mask")?;
    if let Some(m) = mask {
        same_shape(n, m, "the mask")?;
    }
    let att: Vec<f64> = (0..net.len())
        .map(|i| {
            let ok = dist[i] >= lo
                && dist[i] <= hi
                && net[i] == 0
                && mask.is_none_or(|m| !m.data[i].is_nan() && m.data[i] != 0.0);
            if ok { a.data[i] } else { f64::NAN }
        })
        .collect();
    let dest = destinations(
        n,
        &att,
        args.param("quadrants", (2, 10))?,
        args.param("per_quadrant", 1)?,
        args.param("spacing", 1000.0)?,
        args.param("max_attempts", 15)?,
        seed,
    )?;
    let sources: Vec<u8> = net.clone();
    let fr = friction(f, args.param("relative", false)?)?;
    let (cost, _) = cost_map(&sources, &fr, shape, args.param("diagonal", true)?)?;
    let out = pathways(n, &dest, Some(&cost), None, args.param("lottery", true)?, true, 1.0, seed.wrapping_add(1))?;
    let d = distance(&present(&out), shape, rx)?;
    Ok(outputs([("network", map(&n.grid, out)), ("distance", map(&n.grid, d))]))
}
