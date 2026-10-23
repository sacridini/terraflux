//! Multi-action problems from rasters (landschaft's actions_raster_data, after prioripy's raster input): the
//! planning units are the cells with a monitoring cost, a layer per feature (its amount in every cell) and per
//! threat (its intensity, > 0 where it occurs), the actions' costs numbers or maps; prioriactions's tables built
//! from them as landschaft builds them, solved as prioritize.actions solves them, the solution as maps.

use std::sync::Arc;

use serde_json::Value as Json;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::actions::{self, Actions, ActionsData, actions_data};
use crate::milp::{Units, r_round};
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(ActionsRaster));
}

fn bad(m: impl std::fmt::Display) -> Error {
    Error::msg(m.to_string())
}

/// A value per layer: one for all, a list in the layers' order or {name: value} (every layer named).
fn per_layer(v: Option<&Json>, names: &[String], what: &str, default: Json) -> Result<Vec<Json>> {
    match v {
        None | Some(Json::Null) => Ok(vec![default; names.len()]),
        Some(Json::Object(m)) => {
            let missing: Vec<&String> = names.iter().filter(|n| !m.contains_key(*n)).collect();
            if !missing.is_empty() {
                return Err(bad(format!("{what}: no value for {missing:?}")));
            }
            Ok(names.iter().map(|n| m[n].clone()).collect())
        }
        Some(Json::Array(a)) => {
            if a.len() != names.len() {
                return Err(bad(format!("{what}: {} values for {} layers", a.len(), names.len())));
            }
            Ok(a.clone())
        }
        Some(v) => Ok(vec![v.clone(); names.len()]),
    }
}

fn numbers(v: &[Json], what: &str) -> Result<Vec<f64>> {
    v.iter().map(|x| x.as_f64().ok_or_else(|| bad(format!("{what}: {x} is not a number")))).collect()
}

/// The tables of a problem from rasters, before the targets: what [`raster_data`] builds.
pub struct RasterTables {
    pub pu: Table,
    pub features: Table,
    pub dist_features: Table,
    pub threats: Table,
    pub dist_threats: Table,
    pub boundary: Option<Table>,
    /// the cell of every planning unit
    pub cells: Vec<usize>,
}

/// The settings of a problem from rasters: the actions' costs (a number, a map's name, or one per threat), the
/// targets (shares with `relative`), the weight of every threat's actions' boundary, the boundary data.
pub struct RasterSettings<'a> {
    pub action_cost: Option<&'a Json>,
    pub target_recovery: Option<&'a Json>,
    pub target_conservation: Option<&'a Json>,
    pub relative: bool,
    pub blm_actions: Option<&'a Json>,
    pub boundary: bool,
}

/// A multi-action problem from rasters, as landschaft's actions_raster_data: the units are the cells with a
/// monitoring cost (ids 1, 2... in the cells' order); the features' and threats' layers (no value 0) their
/// amounts; the action against a threat costs its number or its map's value in the cell; the units locked in or
/// out where those maps are not 0; relative targets shares of what the feature has where a threat harms it
/// (recovery) or where none does (conservation), rounded to 3 decimals.
pub fn raster_data(
    cost: &Map,
    features: &Stack,
    threats: &Stack,
    maps: &dyn Fn(&str) -> std::result::Result<Vec<f64>, String>,
    locked: [Option<&Map>; 2],
    sensitivity: Option<&Table>,
    s: &RasterSettings,
) -> Result<(ActionsData, RasterTables)> {
    let (ny, nx) = cost.shape();
    let units = |stack: &Stack, what: &str| -> Result<Units> {
        if (stack.grid.height, stack.grid.width) != (ny, nx) {
            return Err(bad(format!("the {what} are not on the monitoring cost's grid")));
        }
        let layers: Vec<&[f64]> = stack.layers.iter().map(|l| l.as_slice()).collect();
        let names = stack.keys.iter().map(|k| k.to_string()).collect();
        Units::raster(&cost.data, &layers, names, (ny, nx), cost.grid.transform).map_err(bad)
    };
    let fu = units(features, "features")?;
    let tu = units(threats, "threats")?;
    let cells = fu.cells.clone();
    let n = cells.len();
    if n == 0 {
        return Err(bad("no planning units: the monitoring cost has no cell with a value"));
    }
    let distinct = |v: &[String]| v.iter().collect::<std::collections::BTreeSet<_>>().len() == v.len();
    let (fnames, tnames) = (fu.names.clone(), tu.names.clone());
    if !distinct(&fnames) || !distinct(&tnames) {
        return Err(bad("the features and the threats need distinct layer names"));
    }
    let at_cells = |v: &[f64]| -> Vec<f64> { cells.iter().map(|&c| v[c]).collect() };
    // the units locked in (2), then out (3)
    let mut status = vec![0.0; n];
    for (mask, code) in locked.iter().zip([2.0, 3.0]) {
        if let Some(m) = mask {
            if m.shape() != (ny, nx) {
                return Err(bad("the locked units are not on the monitoring cost's grid"));
            }
            for (st, v) in status.iter_mut().zip(at_cells(&m.data)) {
                if v.is_finite() && v != 0.0 {
                    *st = code;
                }
            }
        }
    }
    let mut pu = Table::new(["id", "monitoring_cost", "status"].map(String::from).to_vec());
    pu.rows = (0..n).map(|i| vec![(i + 1) as f64, fu.cost[i], status[i]]).collect();
    let mut dist_features = Table::new(["pu", "feature", "amount"].map(String::from).to_vec());
    for k in 0..fu.rij.nrow {
        let (idx, val) = fu.rij.row(k);
        dist_features.rows.extend(idx.iter().zip(val).map(|(&u, &a)| vec![(u + 1) as f64, (k + 1) as f64, a]));
    }
    // the actions: a threat in a unit, its cost there
    let costs = per_layer(s.action_cost, &tnames, "action_cost", Json::from(1.0))?;
    let mut dt: Vec<[f64; 5]> = Vec::new();
    for (k, c) in costs.iter().enumerate() {
        let (idx, val) = tu.rij.row(k);
        if val.iter().any(|v| *v < 0.0) {
            return Err(bad("threat intensities must be >= 0"));
        }
        let values: Vec<f64> = match c {
            Json::Number(x) => vec![x.as_f64().unwrap(); n],
            Json::String(name) => at_cells(&maps(name).map_err(bad)?),
            other => return Err(bad(format!("action_cost of {}: a number or a map's name, not {other}", tnames[k]))),
        };
        for (&u, &a) in idx.iter().zip(val) {
            if !values[u].is_finite() {
                return Err(bad(format!("action_cost of {} has no value in cells where the threat occurs", tnames[k])));
            }
            dt.push([(u + 1) as f64, (k + 1) as f64, a, values[u], 0.0]);
        }
    }
    dt.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let mut dist_threats = Table::new(["pu", "threat", "amount", "action_cost", "status"].map(String::from).to_vec());
    dist_threats.rows = dt.iter().map(|r| r.to_vec()).collect();
    let blm = numbers(&per_layer(s.blm_actions, &tnames, "blm_actions", Json::from(0.0))?, "blm_actions")?;
    let mut threats_t = Table::new(["id", "blm_actions"].map(String::from).to_vec());
    threats_t.rows = blm.iter().enumerate().map(|(k, b)| vec![(k + 1) as f64, *b]).collect();
    // the boundary: every pair of units sharing an edge once (its cell's upper, then left neighbour), then the
    // unit's perimeter, the units in order (prioritizr's boundary_matrix, its upper triangle by columns)
    let boundary = s.boundary.then(|| {
        let (xres, yres) = crate::prioritize_tools::res(&cost.grid);
        let mut unit = vec![usize::MAX; ny * nx];
        for (i, &c) in cells.iter().enumerate() {
            unit[c] = i;
        }
        let mut t = Table::new(["id1", "id2", "boundary"].map(String::from).to_vec());
        for (i, &c) in cells.iter().enumerate() {
            let id = (i + 1) as f64;
            if c >= nx && unit[c - nx] != usize::MAX {
                t.rows.push(vec![(unit[c - nx] + 1) as f64, id, xres]);
            }
            if c % nx > 0 && unit[c - 1] != usize::MAX {
                t.rows.push(vec![(unit[c - 1] + 1) as f64, id, yres]);
            }
            t.rows.push(vec![id, id, (xres + yres) * 2.0]);
        }
        t
    });
    let mut features_t = Table::new(["id", "target_recovery", "target_conservation"].map(String::from).to_vec());
    features_t.rows = (0..fnames.len()).map(|k| vec![(k + 1) as f64, 0.0, 0.0]).collect();
    let d = actions_data(&pu, &features_t, &dist_features, &threats_t, &dist_threats, sensitivity, boundary.as_ref())
        .map_err(bad)?;
    let tr = numbers(&per_layer(s.target_recovery, &fnames, "target_recovery", Json::from(0.0))?, "target_recovery")?;
    let tc = numbers(
        &per_layer(s.target_conservation, &fnames, "target_conservation", Json::from(0.0))?,
        "target_conservation",
    )?;
    // the features actions_data keeps (those with an amount)
    let kept: Vec<usize> = d.feature_ids.iter().map(|id| *id as usize - 1).collect();
    let (rec, cons): (Vec<f64>, Vec<f64>) = if s.relative {
        if tr.iter().chain(&tc).any(|t| !(0.0..=1.0).contains(t)) {
            return Err(bad("relative targets must be between 0 and 1"));
        }
        let p = ffi::actions_pairs(&d.input).map_err(|e| bad(e.what()))?;
        let mut held = [vec![0.0; kept.len()], vec![0.0; kept.len()]];
        for ((&f, &a), &t) in p.feature.iter().zip(&p.amount).zip(&p.threatened) {
            held[usize::from(t != 0)][f as usize] += a;
        }
        kept.iter().enumerate().map(|(j, &k)| (tr[k] * held[1][j], tc[k] * held[0][j])).unzip()
    } else {
        kept.iter().map(|&k| (tr[k], tc[k])).unzip()
    };
    features_t.rows =
        kept.iter().enumerate().map(|(j, &k)| vec![(k + 1) as f64, r_round(rec[j], 3), r_round(cons[j], 3)]).collect();
    let d = actions_data(&pu, &features_t, &dist_features, &threats_t, &dist_threats, sensitivity, boundary.as_ref())
        .map_err(bad)?;
    let tables =
        RasterTables { pu, features: features_t, dist_features, threats: threats_t, dist_threats, boundary, cells };
    Ok((d, tables))
}

/// The monitored cells and every threat's actions as maps (monitoring, then the threats' names), no value outside
/// the planning units; x the units then the actions, in the problem's order.
pub fn solution_maps(d: &ActionsData, t: &RasterTables, x: &[f64], cost: &Map, names: &[String]) -> Stack {
    let size = cost.data.len();
    let n = d.unit_ids.len();
    let mut layers = vec![vec![f64::NAN; size]; names.len() + 1];
    for l in &mut layers {
        for &c in &t.cells {
            l[c] = 0.0;
        }
    }
    for (i, id) in d.unit_ids.iter().enumerate() {
        layers[0][t.cells[*id as usize - 1]] = x[i].round();
    }
    for k in 0..d.action_unit.len() {
        let cell = t.cells[d.action_unit[k] as usize - 1];
        layers[d.action_threat[k] as usize][cell] = x[n + k].round();
    }
    Stack {
        grid: cost.grid.clone(),
        keys: std::iter::once("monitoring".to_string()).chain(names.iter().cloned()).map(Key::Name).collect(),
        layers: layers.into_iter().map(Arc::new).collect(),
    }
}

struct ActionsRaster;

impl Operator for ActionsRaster {
    fn signature(&self) -> OpSignature {
        let base = Actions.signature();
        let mut sig = OpSignature::new(
            "prioritize.actions_raster",
            "Multi-action planning on rasters (landschaft's actions_raster_data): the planning units are the \
             cells with a monitoring cost, a layer per feature its amounts and per threat its intensity (> 0 \
             where it occurs; the action there removes it), the actions' costs numbers or maps; the tables of \
             prioritize.actions built as landschaft builds them (relative targets shares of what each feature has \
             where a threat harms it and where none does), solved as prioritize.actions solves them, the \
             monitored cells and every threat's actions as maps.",
            Access::Global,
        )
        .input(PortSpec::new("monitoring_cost", Type::Map, "the cost of monitoring every cell (no value: not a unit)"))
        .input(PortSpec::new("features", Type::Stack, "the features' amounts, a layer each (no value 0)"))
        .input(PortSpec::new("threats", Type::Stack, "the threats' intensities, a layer each (> 0 where it occurs)"))
        .input(PortSpec::optional("maps", Type::Stack, "the maps of the actions' costs, named in action_cost"))
        .input(PortSpec::optional("locked_in", Type::Map, "cells that must be monitored (not 0)"))
        .input(PortSpec::optional("locked_out", Type::Map, "cells that must not be monitored (not 0)"))
        .input(PortSpec::optional(
            "sensitivity",
            Type::Table,
            "which features each threat harms: feature, threat (1-based, in the layers' order), delta1-delta4 \
             (default every feature to every threat)",
        ))
        .output(PortSpec::new(
            "maps",
            Type::Stack,
            "the monitored cells (monitoring) and every threat's actions (by its name), 0/1",
        ))
        .output(PortSpec::new("pu", Type::Table, "the planning units built: id, monitoring_cost, status"))
        .output(PortSpec::new("dist_threats", Type::Table, "the actions built: pu, threat, amount, action_cost"))
        .param(PortSpec::optional(
            "action_cost",
            Type::Any,
            "the cost of acting against every threat: a number or a map's name (one of maps) for all, a list in \
             the threats' order or {threat: ...} (default 1)",
        ))
        .param(PortSpec::optional(
            "target_recovery",
            Type::Any,
            "the features' recovery targets: a number, a list in the features' order or {feature: ...}",
        ))
        .param(PortSpec::optional("target_conservation", Type::Any, "the features' conservation targets, the same"))
        .param(PortSpec::optional(
            "relative",
            Type::Bool,
            "the targets are shares (0-1) of the feature where threats harm it (recovery) or none does \
             (conservation); default true",
        ))
        .param(PortSpec::optional("blm_actions", Type::Any, "the weight of every threat's actions' boundary (0)"))
        .param(PortSpec::optional(
            "boundary",
            Type::Bool,
            "the cells' shared edges as boundary data (for the blms)",
        ));
        sig.outputs.extend(base.outputs);
        sig.params.extend(base.params);
        sig
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let fail = |e: Error| Error::msg(format!("node {node:?}: {e}"));
        let cost = args.map("monitoring_cost")?;
        let (features, threats) = (args.stack("features")?, args.stack("threats")?);
        let stack = args.opt_stack("maps")?;
        let maps = |name: &str| -> std::result::Result<Vec<f64>, String> {
            let s = stack.ok_or_else(|| format!("action_cost: no maps (for {name:?})"))?;
            if (s.grid.height, s.grid.width) != cost.shape() {
                return Err("the maps are not on the monitoring cost's grid".into());
            }
            let k = s.keys.iter().position(|k| k.to_string() == name).ok_or_else(|| format!("maps: no {name:?}"))?;
            Ok(s.layers[k].to_vec())
        };
        let table = |port: &str| match args.get(port) {
            Some(Value::Table(t)) => Some(t),
            _ => None,
        };
        let settings = RasterSettings {
            action_cost: args.params.get("action_cost"),
            target_recovery: args.params.get("target_recovery"),
            target_conservation: args.params.get("target_conservation"),
            relative: args.param("relative", true)?,
            blm_actions: args.params.get("blm_actions"),
            boundary: args.param("boundary", false)?,
        };
        let locked = [args.opt_map("locked_in")?, args.opt_map("locked_out")?];
        let (d, tables) =
            raster_data(cost, features, threats, &maps, locked, table("sensitivity"), &settings).map_err(fail)?;
        let (mut out, d, x) = actions::solve(ctx, args, d)?;
        let names: Vec<String> = threats.keys.iter().map(|k| k.to_string()).collect();
        out.insert("maps".into(), Value::Stack(solution_maps(&d, &tables, &x, cost, &names)));
        let mut dt = tables.dist_threats.clone();
        dt.columns.truncate(4);
        dt.rows.iter_mut().for_each(|r| r.truncate(4));
        out.insert("pu".into(), Value::Table(tables.pu));
        out.insert("dist_threats".into(), Value::Table(dt));
        Ok(out)
    }
}
