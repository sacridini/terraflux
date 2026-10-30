//! CLUMondo's demand-driven allocation of land systems (Van Asselen & Verburg 2013; landschaft's `CLUMondo`):
//! `clumondo.ages`, `clumondo.step`.
//!
//! Each year the suitability of every land system (CLUMondo's logistic regressions, or given maps), its
//! neighbourhood functions and location preferences, then landschaft's C++ port of CLUMondo's year (the
//! iteration on the demand for goods and services, conversion resistances, the conversion matrix and its codes,
//! the 2017 executable's extensions, the land system roles' caps and pair resistances).

use std::sync::Arc;

use serde::Deserialize;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::{ClumondoNbIn, ClumondoSettingsIn, ffi};

use crate::value::{GlmModel, class_label};
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

const NO_DATA: i32 = -9999;
const RESTRICTED: i32 = -9998;

/// The supply of one cell of each system from a step on (CLUMondo's dynamic `lusmatrix`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupplyAt {
    pub step: usize,
    /// A row per system, a value per service.
    pub rows: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patches {
    pub radius: Vec<i32>,
    pub threshold: Vec<f32>,
    pub min: Vec<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cap {
    /// systems x systems: 1 for the transitions it limits.
    pub pairs: Vec<Vec<u8>>,
    pub cells: i64,
}

/// A neighbourhood function (CLUMondo's neighmat.txt and alloc2.reg), systems by their position.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Neighbourhood {
    pub system: i32,
    pub weight: f32,
    /// A square of odd side.
    pub kernel: Vec<Vec<f32>>,
    pub from: Vec<bool>,
    pub constant: f32,
    /// [system, b] pairs.
    pub coefficients: Vec<(i32, f32)>,
}

/// A location preference (CLUMondo's locspec): the layers of the `preferences` stack by the step they hold from.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preference {
    pub system: i32,
    pub weight: f32,
    /// [step, layer] pairs (the step null: every step).
    pub layers: Vec<(Option<usize>, usize)>,
}

/// The settings of a CLUMondo model, as landschaft's `CLUMondo` compiles them.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub systems: Vec<f64>,
    pub services: Vec<String>,
    pub supply: Vec<SupplyAt>,
    /// The demand of each service, a row per step (the first the start).
    pub demand: Vec<Vec<f64>>,
    /// The conversion order (lusconv): a row per system, a rank per service.
    pub order: Vec<Vec<i32>>,
    pub weights: Vec<f32>,
    /// The conversion resistance of each system.
    pub resistance: Vec<f32>,
    /// The conversion matrix (rows the current system) with CLUMondo's codes, 2-99 the `allow` stack's layers.
    pub allowed: Vec<Vec<i32>>,
    /// "percent" or "absolute".
    pub deviation: String,
    pub mean_deviation: f32,
    pub max_deviation: f32,
    pub speed: f32,
    pub max_loops: i32,
    #[serde(default)]
    pub kind: Option<Vec<i32>>,
    #[serde(default)]
    pub dev_limit: Option<Vec<f32>>,
    #[serde(default)]
    pub patches: Option<Patches>,
    #[serde(default)]
    pub pair_resistance: Option<Vec<Vec<f32>>>,
    #[serde(default)]
    pub caps: Vec<Cap>,
    #[serde(default)]
    pub neighbourhood: Vec<Neighbourhood>,
    #[serde(default)]
    pub preferences: Vec<Preference>,
}

impl Settings {
    fn ffi(&self, ncols: usize) -> Result<ClumondoSettingsIn> {
        let ns = self.systems.len();
        let nd = self.services.len();
        let square = |m: &Vec<Vec<i32>>, cols: usize| m.len() == ns && m.iter().all(|r| r.len() == cols);
        if !square(&self.allowed, ns) {
            return Err(Error::msg("the conversion matrix must be systems x systems"));
        }
        if !square(&self.order, nd) {
            return Err(Error::msg("the conversion order must be systems x services"));
        }
        let itermode = match self.deviation.as_str() {
            "percent" => 0,
            "absolute" => 1,
            d => return Err(Error::msg(format!("deviation must be \"percent\" or \"absolute\", not {d:?}"))),
        };
        let mut s = ClumondoSettingsIn {
            nsys: ns as i32,
            ndem: nd as i32,
            allowed: self.allowed.concat(),
            lusconv: self.order.concat(),
            weight: self.weights.clone(),
            resistance: self.resistance.clone(),
            itermode,
            mean_dev: self.mean_deviation,
            max_dev: self.max_deviation,
            speed: self.speed,
            max_loops: self.max_loops,
            kind: self.kind.clone().unwrap_or_default(),
            dev_limit: self.dev_limit.clone().unwrap_or_default(),
            ..Default::default()
        };
        if let Some(p) = &self.patches {
            s.nx = ncols as i64;
            s.patch_radius = p.radius.clone();
            s.patch_threshold = p.threshold.clone();
            s.patch_min = p.min.clone();
        }
        if let Some(pr) = &self.pair_resistance {
            if pr.len() != ns || pr.iter().any(|r| r.len() != ns) {
                return Err(Error::msg("the pair resistances must be systems x systems"));
            }
            s.pair_resistance = pr.concat();
        }
        for c in &self.caps {
            if c.pairs.len() != ns || c.pairs.iter().any(|r| r.len() != ns) {
                return Err(Error::msg("a cap's transitions must be systems x systems"));
            }
            s.cap_pairs.extend(c.pairs.concat());
            s.cap_cells.push(c.cells);
        }
        Ok(s)
    }

    /// The supply table in force at a step (the last given at or before it, from step 1).
    fn supply_at(&self, step: usize) -> Result<Vec<f32>> {
        let t = self
            .supply
            .iter()
            .filter(|t| t.step <= step.max(1))
            .max_by_key(|t| t.step)
            .ok_or_else(|| Error::msg("the supply needs step 1"))?;
        if t.rows.len() != self.systems.len() || t.rows.iter().any(|r| r.len() != self.services.len()) {
            return Err(Error::msg("the supply must be systems x services"));
        }
        Ok(t.rows.iter().flatten().map(|&v| v as f32).collect())
    }
}

/// The logistic suitability of each system (CLUMondo's calc_reg, landschaft's `_base_suitability` with a
/// `LogisticRegression`): aliased drivers left out, in floats.
fn logit(model: &GlmModel, drivers: &Stack, systems: &[f64], region: &[i32]) -> Result<Vec<f32>> {
    let n = region.len();
    let mut out = Vec::with_capacity(systems.len() * n);
    for &s in systems {
        let fit = model.fits.iter().find(|f| f.class == s).ok_or_else(|| {
            Error::msg(format!("the potential gives no suitability for land system {}", class_label(s)))
        })?;
        let b = &fit.coefficients;
        let mut x = Vec::new();
        let mut coef = Vec::new();
        for (name, &v) in fit.drivers.iter().zip(b.iter().skip(1)) {
            if v.is_nan() {
                continue;
            }
            let layer = drivers.by_name(name).ok_or_else(|| Error::msg(format!("drivers lack {name:?}")))?;
            x.extend(layer.iter().map(|&v| v as f32));
            coef.push(v as f32);
        }
        out.extend(ffi::clumondo_logit(region, &x, b[0], &coef)?);
    }
    Ok(out)
}

/// The suitability of each system from given maps (floats; -9999 outside the study area).
fn given(stack: &Stack, systems: &[f64], region: &[i32]) -> Result<Vec<f32>> {
    let mut out = Vec::with_capacity(systems.len() * region.len());
    for &s in systems {
        let l = stack.get(&Key::Class(s)).ok_or_else(|| {
            Error::msg(format!("the potential gives no suitability for land system {}", class_label(s)))
        })?;
        out.extend(l.iter().zip(region).map(|(&v, &r)| if r < 0 { -9999.0 } else { v as f32 }));
    }
    Ok(out)
}

/// The demand's iteration variables carried from year to year (a list of numbers).
fn elasticities(v: &Value, nd: usize) -> Result<Vec<f32>> {
    let x: Vec<f32> = match v {
        Value::Json(serde_json::Value::Array(a)) => a
            .iter()
            .map(|x| x.as_f64().map(|v| v as f32).ok_or_else(|| Error::msg("the elasticities are numbers")))
            .collect::<Result<_>>()?,
        Value::Series(s) => s
            .iter()
            .map(|x| x.as_number().map(|v| v as f32).ok_or_else(|| Error::msg("the elasticities are numbers")))
            .collect::<Result<_>>()?,
        Value::Number(_) | Value::Integer(_) => vec![v.as_number().unwrap() as f32; nd],
        v => return Err(Error::msg(format!("the elasticities are a list of numbers, not a {:?}", v.ty()))),
    };
    if x.len() != nd {
        return Err(Error::msg("one elasticity per service"));
    }
    Ok(x)
}

/// What a year's suitability comes from.
pub enum Potential<'a> {
    Logit(&'a GlmModel, &'a Stack),
    Given(&'a Stack),
}

/// The inputs of a year besides the settings.
pub struct Year<'a> {
    pub landscape: &'a Map,
    pub age: &'a Map,
    pub elasticity: Vec<f32>,
    pub step: usize,
    pub potential: Potential<'a>,
    pub restrictions: Option<&'a Map>,
    pub allow: Option<&'a Stack>,
    pub preferences: Option<&'a Stack>,
}

/// The outputs of a year: the map, the ages, the iteration variables, whether it stopped, its report.
pub struct YearOut {
    pub landscape: Map,
    pub age: Map,
    pub elasticity: Vec<f32>,
    pub stop: bool,
    pub info: Table,
}

/// One year of CLUMondo (landschaft's `CLUMondo.simulate`).
pub fn year(ctx: &mut Ctx, s: &Settings, y: Year<'_>) -> Result<YearOut> {
    let land = y.landscape;
    let (ny, nx) = land.shape();
    let n = ny * nx;
    let ns = s.systems.len();
    let mut region = vec![0i32; n];
    if let Some(r) = y.restrictions {
        for (g, &v) in region.iter_mut().zip(r.data.iter()) {
            if !v.is_nan() && v != 0.0 {
                *g = RESTRICTED;
            }
        }
    }
    let mut oldco = vec![NO_DATA; n];
    for (i, &v) in land.data.iter().enumerate() {
        if v.is_nan() {
            region[i] = NO_DATA;
            continue;
        }
        oldco[i] =
            s.systems.iter().position(|&x| x == v).ok_or_else(|| {
                Error::msg(format!("the landscape has a land system without supply: {}", class_label(v)))
            })? as i32;
    }
    let mut suit = match y.potential {
        Potential::Logit(m, d) => logit(m, d, &s.systems, &region)?,
        Potential::Given(st) => given(st, &s.systems, &region)?,
    };
    if !s.neighbourhood.is_empty() {
        let parts: Vec<ClumondoNbIn> = s
            .neighbourhood
            .iter()
            .map(|p| {
                let side = p.kernel.len();
                if side % 2 == 0 || p.kernel.iter().any(|r| r.len() != side) {
                    return Err(Error::msg("a neighbourhood kernel must be square with an odd side"));
                }
                if p.from.len() != ns {
                    return Err(Error::msg("a neighbourhood's from: a flag per system"));
                }
                Ok(ClumondoNbIn {
                    system: p.system,
                    weight: p.weight,
                    kernel: p.kernel.concat(),
                    radius: (side / 2) as i32,
                    from: p.from.iter().map(|&b| u8::from(b)).collect(),
                    constant: p.constant,
                    coef_systems: p.coefficients.iter().map(|c| c.0).collect(),
                    coef_values: p.coefficients.iter().map(|c| c.1).collect(),
                })
            })
            .collect::<Result<_>>()?;
        suit = ffi::clumondo_add_neighbourhood(&suit, ny as i64, nx as i64, &oldco, &region, &parts)?;
    }
    for p in &s.preferences {
        let stack = y.preferences.ok_or_else(|| Error::msg("location preferences need their maps"))?;
        // the layer given for the last step at or before this one, else the first (landschaft's _at_step)
        let fixed = p.layers.iter().find(|(st, _)| st.is_none());
        let layer = match fixed {
            Some(&(_, k)) => k,
            None => {
                let before = p.layers.iter().filter(|(st, _)| st.unwrap() <= y.step).max_by_key(|(st, _)| st.unwrap());
                let first = p.layers.iter().min_by_key(|(st, _)| st.unwrap());
                before.or(first).ok_or_else(|| Error::msg("a location preference without maps"))?.1
            }
        };
        let values: Vec<f32> = stack
            .layers
            .get(layer)
            .ok_or_else(|| Error::msg("a location preference names a layer the stack does not have"))?
            .iter()
            .map(|&v| v as f32)
            .collect();
        let k = p.system as usize;
        if k >= ns {
            return Err(Error::msg("a location preference of an unknown system"));
        }
        let row = ffi::clumondo_add_preference(&suit[k * n..(k + 1) * n], &region, &values, p.weight)?;
        suit[k * n..(k + 1) * n].copy_from_slice(&row);
    }
    let allow_maps: Vec<f32> = match y.allow {
        None => Vec::new(),
        Some(st) => st.layers.iter().flat_map(|l| l.iter().map(|&v| if v.is_nan() { 0.0 } else { v as f32 })).collect(),
    };
    let demand = s.demand.get(y.step).ok_or_else(|| Error::msg(format!("the demand has no step {}", y.step)))?;
    if demand.len() != s.services.len() {
        return Err(Error::msg("the demand must have a value per service"));
    }
    let mut settings = s.ffi(nx)?;
    settings.lusmatrix = s.supply_at(y.step)?;
    let age: Vec<i32> = y.age.data.iter().map(|&v| v as i32).collect();
    let r = ffi::clumondo_year(&settings, &oldco, &region, &age, &suit, &allow_maps, demand, &y.elasticity)?;
    let mut columns: Vec<String> =
        ["step", "loops", "converged", "max_deviation", "mean_deviation"].iter().map(|s| s.to_string()).collect();
    let mut row = vec![
        y.step as f64,
        f64::from(r.loops),
        f64::from(u8::from(r.converged)),
        f64::from(r.max_diff),
        f64::from(r.mean_diff),
    ];
    for (sv, v) in s.services.iter().zip(&r.supply) {
        columns.push(format!("supply_{sv}"));
        row.push(*v);
    }
    for (&sy, &c) in s.systems.iter().zip(&r.cells) {
        columns.push(format!("cells_{}", class_label(sy)));
        row.push(c as f64);
    }
    let info = Table { columns, rows: vec![row], ..Default::default() };
    if !r.converged {
        ctx.warn(format!(
            "no solution in step {}: the demand was not met within the deviations in {} iterations; the simulation \
             stops there (as CLUMondo)",
            y.step, s.max_loops
        ));
        return Ok(YearOut { landscape: land.clone(), age: y.age.clone(), elasticity: y.elasticity, stop: true, info });
    }
    let out: Vec<f64> = r
        .newco
        .iter()
        .zip(land.data.iter())
        .map(|(&c, &v)| if v.is_nan() || c < 0 { f64::NAN } else { s.systems[c as usize] })
        .collect();
    Ok(YearOut {
        landscape: Map::new(land.grid.clone(), out),
        age: Map::new(land.grid.clone(), r.age.iter().map(|&a| f64::from(a)).collect()),
        elasticity: r.dem_elas,
        stop: false,
        info,
    })
}

struct Ages;

impl Operator for Ages {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "clumondo.ages",
            "The years each cell has had its land system at the start: the given map (missing cells counted 1), \
             or rand() % max_age + 1 per cell from the runtime generator seeded with the seed (CLUMondo's random \
             ages), else 1.",
            Access::Global,
        )
        .input(PortSpec::new("landscape", Type::Map, "the map at the start"))
        .input(PortSpec::optional("history", Type::Map, "the years each cell has had its system"))
        .input(PortSpec::optional("seed", Type::Integer, "the seed of the random ages"))
        .output(PortSpec::new("age", Type::Map, "the ages to carry along the years"))
        .param(PortSpec::optional("max_age", Type::Integer, "random ages from 1 to max_age"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let land = args.map("landscape")?;
        let n = land.data.len();
        let age: Vec<f64> = if let Some(max_age) = args.opt_param::<i32>("max_age")? {
            let seed = match args.get("seed") {
                Some(_) => args.integer("seed")?,
                None => 1,
            };
            ffi::clumondo_random_age(n as i64, max_age, (seed & 0xFFFF_FFFF) as u32)?
                .into_iter()
                .map(f64::from)
                .collect()
        } else if let Some(h) = args.opt_map("history")? {
            if h.data.len() != n {
                return Err(Error::msg("the history map differs in size from the landscape"));
            }
            h.data.iter().map(|&v| if v.is_nan() { 1.0 } else { f64::from(v as i32) }).collect()
        } else {
            vec![1.0; n]
        };
        Ok(outputs([("age", Value::Map(Map::new(land.grid.clone(), age)))]))
    }
}

struct StepOp;

impl Operator for StepOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "clumondo.step",
            "One year of CLUMondo: the suitability of each land system (logistic regressions in floats, or given \
             maps) with its neighbourhood functions and location preferences, then the allocation that meets the \
             demand for goods and services (conversion resistances, the conversion matrix with its codes, ages). \
             `stop` is true when the year has no solution (CLUMondo stops there: use it as the loop's until).",
            Access::Global,
        )
        .input(PortSpec::new("landscape", Type::Map, "the map of land systems at the start of the year"))
        .input(PortSpec::new("age", Type::Map, "the years each cell has had its system (clumondo.ages)"))
        .input(PortSpec::new("elasticity", Type::Any, "the demand's iteration variables (zeros at the start)"))
        .input(PortSpec::new("step", Type::Integer, "the year (1, 2...)"))
        .input(PortSpec::optional("model", Type::Glm, "logistic regressions of the systems"))
        .input(PortSpec::optional("drivers", Type::Stack, "their drivers"))
        .input(PortSpec::optional("suitability", Type::Stack, "or the suitability of each system"))
        .input(PortSpec::optional("restrictions", Type::Map, "cells that keep their system (non-zero)"))
        .input(PortSpec::optional("allow", Type::Stack, "the conversion maps (codes 2, 3... of the matrix)"))
        .input(PortSpec::optional("preferences", Type::Stack, "the location preferences' maps"))
        .output(PortSpec::new("landscape", Type::Map, "the map at the end of the year"))
        .output(PortSpec::new("age", Type::Map, "the ages"))
        .output(PortSpec::new("elasticity", Type::Any, "the iteration variables for the next year"))
        .output(PortSpec::new("stop", Type::Bool, "whether the year had no solution"))
        .output(PortSpec::new("info", Type::Table, "iterations, deviations, supply and cells of each system"))
        .param(PortSpec::new(
            "settings",
            Type::Any,
            "{ systems, services, supply, demand, order, weights, resistance, allowed, deviation, mean_deviation, \
             max_deviation, speed, max_loops, ... } as landschaft's Model.save writes them",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s: Settings = args
            .opt_param("settings")?
            .ok_or_else(|| Error::msg(format!("node {:?}: give the settings", args.node)))?;
        let potential = match (args.get("model"), args.opt_stack("suitability")?) {
            (Some(_), None) => match args.get("model") {
                Some(Value::Glm(m)) => Potential::Logit(m, args.stack("drivers")?),
                _ => return Err(Error::msg("the model must be logistic regressions")),
            },
            (None, Some(st)) => Potential::Given(st),
            _ => return Err(Error::msg(format!("node {:?}: give the regressions or the suitability", args.node))),
        };
        let k = args.integer("step")?;
        if k < 1 {
            return Err(Error::msg("the step must be >= 1"));
        }
        let elasticity =
            elasticities(args.get("elasticity").ok_or_else(|| Error::msg("no elasticity"))?, s.services.len())?;
        let r = year(
            ctx,
            &s,
            Year {
                landscape: args.map("landscape")?,
                age: args.map("age")?,
                elasticity,
                step: k as usize,
                potential,
                restrictions: args.opt_map("restrictions")?,
                allow: args.opt_stack("allow")?,
                preferences: args.opt_stack("preferences")?,
            },
        )?;
        let elas = serde_json::Value::Array(r.elasticity.iter().map(|&v| serde_json::json!(f64::from(v))).collect());
        Ok(outputs([
            ("landscape", Value::Map(r.landscape)),
            ("age", Value::Map(r.age)),
            ("elasticity", Value::Json(elas)),
            ("stop", Value::Bool(r.stop)),
            ("info", Value::Table(r.info)),
        ]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Ages));
    r.add(Arc::new(StepOp));
}
