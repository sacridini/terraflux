//! One step of a CLUE-S or ordered allocation with lulcc's decision rules (landschaft's `Model.run`):
//! `lulcc.history`, `lulcc.step`.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::{NbRule, ffi};

use crate::value::{Regional, class_label};
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

/// The allocation of the step.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum Allocation {
    /// CLUE-S (Verburg et al. 2002) as lulcc's CluesModel.
    Clue {
        #[serde(default = "jitter")]
        jitter: f64,
        #[serde(default = "scale")]
        scale: f64,
        #[serde(default = "max_iter")]
        max_iter: f64,
        #[serde(default = "five")]
        max_diff: f64,
        #[serde(default = "five")]
        mean_diff: f64,
    },
    /// The ordered allocation of Fuchs et al. (2013), lulcc's OrderedModel.
    Ordered {
        order: Vec<f64>,
        #[serde(default = "yes")]
        stochastic: bool,
    },
}

fn jitter() -> f64 {
    0.0001
}
fn scale() -> f64 {
    0.0005
}
fn max_iter() -> f64 {
    1000.0
}
fn five() -> f64 {
    5.0
}
fn yes() -> bool {
    true
}

/// Neighbourhood rules: a class may go only where the mean of its presence over the window reaches the threshold.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Neighbourhood {
    /// { class = threshold }
    pub rules: BTreeMap<String, f64>,
    /// The window (odd sides), one for all classes (default 3 x 3 ones), or { class = window }.
    #[serde(default)]
    pub weights: Option<serde_json::Value>,
}

impl Neighbourhood {
    fn rules(&self) -> Result<Vec<NbRule>> {
        let mut out = Vec::new();
        for (k, &threshold) in &self.rules {
            let cls: f64 = k.trim().parse().map_err(|_| Error::msg(format!("{k:?} is not a class")))?;
            let w = match &self.weights {
                None => None,
                Some(serde_json::Value::Object(o)) => {
                    o.iter().find(|(c, _)| c.trim().parse::<f64>().ok() == Some(cls)).map(|(_, v)| v.clone())
                }
                Some(v) => Some(v.clone()),
            };
            let w: Vec<Vec<f64>> = match w {
                None => vec![vec![1.0; 3]; 3],
                Some(v) => serde_json::from_value(v).map_err(|e| Error::msg(format!("a neighbourhood window: {e}")))?,
            };
            let (wy, wx) = (w.len(), w.first().map_or(0, |r| r.len()));
            if wy % 2 == 0 || wx % 2 == 0 || w.iter().any(|r| r.len() != wx) {
                return Err(Error::msg("a neighbourhood window needs two odd sides"));
            }
            out.push(NbRule { cls, threshold, wy: wy as i32, wx: wx as i32, weights: w.concat() });
        }
        Ok(out)
    }
}

/// The settings of a step.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepSettings {
    pub allocation: Allocation,
    /// Added to the suitability of each cell's own class (CLUE only): one value, a list in the demand's class
    /// order, or { class = value }.
    #[serde(default)]
    pub elasticity: Option<serde_json::Value>,
    /// The conversion matrix in the demand's class order (rows the current class): 1, 0, -1, -2, 100 + t,
    /// 1000 + t.
    #[serde(default)]
    pub allowed: Option<Vec<Vec<i64>>>,
    #[serde(default)]
    pub neighbourhood: Option<Neighbourhood>,
}

fn elasticities(e: &Option<serde_json::Value>, classes: &[f64]) -> Result<Vec<f64>> {
    let n = classes.len();
    match e {
        None | Some(serde_json::Value::Null) => Ok(vec![0.0; n]),
        Some(serde_json::Value::Number(x)) => Ok(vec![x.as_f64().unwrap(); n]),
        Some(serde_json::Value::Array(a)) => {
            let v: Vec<f64> = a
                .iter()
                .map(|x| x.as_f64().ok_or_else(|| Error::msg("elasticities are numbers")))
                .collect::<Result<_>>()?;
            if v.len() != n {
                return Err(Error::msg("one elasticity per class"));
            }
            Ok(v)
        }
        Some(serde_json::Value::Object(o)) => {
            let mut v = vec![0.0; n];
            for (k, x) in o {
                let c: f64 = k.trim().parse().map_err(|_| Error::msg(format!("{k:?} is not a class")))?;
                if let Some(j) = classes.iter().position(|&y| y == c) {
                    v[j] = x.as_f64().ok_or_else(|| Error::msg("elasticities are numbers"))?;
                }
            }
            Ok(v)
        }
        Some(other) => Err(Error::msg(format!("cannot read the elasticities {other}"))),
    }
}

/// The history at the start: the given map (missing cells counted 1), else 1 at every cell with a class.
pub fn initial_history(landscape: &Map, history: Option<&Map>) -> Map {
    let data = landscape
        .data
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            if v.is_nan() {
                f64::NAN
            } else {
                history.map_or(1.0, |h| if h.data[i].is_nan() { 1.0 } else { h.data[i].trunc() })
            }
        })
        .collect();
    Map::new(landscape.grid.clone(), data)
}

/// The output of one step.
pub struct Step {
    pub landscape: Map,
    pub history: Map,
    pub info: Table,
}

/// The demand of a step: one for the whole map, or one per region.
#[derive(Clone, Copy)]
pub enum Demands<'a> {
    One(&'a crate::Demand),
    Regional(&'a Regional),
}

impl Demands<'_> {
    fn classes(&self) -> Vec<f64> {
        match self {
            Demands::One(d) => d.classes.clone(),
            Demands::Regional(r) => r.classes(),
        }
    }
}

/// The allocation's report over the regions: iterations and differences at their largest (the mean
/// difference averaged), converged if all are.
fn merge_info(infos: &[Vec<(String, f64)>]) -> Vec<(String, f64)> {
    if infos.len() == 1 {
        return infos[0].clone();
    }
    infos[0]
        .iter()
        .enumerate()
        .map(|(j, (k, _))| {
            let v: Vec<f64> = infos.iter().map(|i| i[j].1).collect();
            let x = match k.as_str() {
                "converged" => f64::from(u8::from(v.iter().all(|&x| x != 0.0))),
                "mean_diff" => v.iter().sum::<f64>() / v.len() as f64,
                _ => v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            };
            (k.clone(), x)
        })
        .collect()
}

/// One step of landschaft's `Model.run` (lulcc's allocate); with regions, region by region in the order of
/// their ids with the one generator (cells of no region keep their class).
#[allow(clippy::too_many_arguments)]
pub fn step(
    ctx: &mut Ctx,
    landscape: &Map,
    history: &Map,
    suitability: &crate::Stack,
    demand: Demands<'_>,
    step: usize,
    rng: &mut terraflux_ls_sys::RRng,
    restrictions: Option<&Map>,
    s: &StepSettings,
) -> Result<Step> {
    let land = &landscape.data;
    let (ny, nx) = landscape.shape();
    let classes = demand.classes();
    let nc = classes.len();
    let found = landscape.classes();
    let unknown: Vec<String> = found.iter().filter(|c| !classes.contains(c)).map(|&c| class_label(c)).collect();
    if !unknown.is_empty() {
        return Err(Error::msg(format!("the landscape has classes without demand: {unknown:?}")));
    }
    let cells: Vec<usize> = (0..land.len()).filter(|&i| !land[i].is_nan()).collect();
    let mut x0: Vec<f64> = cells.iter().map(|&i| land[i]).collect();
    let mut layers = Vec::with_capacity(nc);
    let mut missing = Vec::new();
    for &c in &classes {
        match suitability.get(&Key::Class(c)) {
            Some(l) => layers.push(l.clone()),
            None => missing.push(class_label(c)),
        }
    }
    if !missing.is_empty() {
        return Err(Error::msg(format!("the potential gives no suitability for classes {missing:?}")));
    }
    let n = cells.len();
    let mut tprob = vec![0.0; n * nc];
    for (i, &cell) in cells.iter().enumerate() {
        for j in 0..nc {
            tprob[i * nc + j] = layers[j][cell];
        }
    }
    let uses_elasticity = matches!(s.allocation, Allocation::Clue { .. });
    if uses_elasticity {
        let elas = elasticities(&s.elasticity, &classes)?;
        for (i, &x) in x0.iter().enumerate() {
            for j in 0..nc {
                if x == classes[j] {
                    tprob[i * nc + j] += elas[j];
                }
            }
        }
    }
    if let Some(nb) = &s.neighbourhood {
        let allow = ffi::neighbourhood_allow(land, ny as i64, nx as i64, &classes, &nb.rules()?)?;
        for (i, &cell) in cells.iter().enumerate() {
            for j in 0..nc {
                tprob[i * nc + j] *= allow[cell * nc + j];
            }
        }
    }
    let hist: Vec<i64> = cells.iter().map(|&i| history.data[i] as i64).collect();
    let mask: Vec<i64> = match restrictions {
        None => vec![1; n],
        Some(r) => cells.iter().map(|&i| if !r.data[i].is_nan() && r.data[i] != 0.0 { 0 } else { 1 }).collect(),
    };
    let rules: Option<Vec<i64>> = match &s.allowed {
        Some(allowed) => {
            if allowed.len() != nc || allowed.iter().any(|r| r.len() != nc) {
                return Err(Error::msg("the conversion matrix must be square, one row and column per class"));
            }
            Some(allowed.concat())
        }
        None => None,
    };
    // the groups of cells allocated together, each with its demand
    let groups: Vec<(Vec<usize>, &crate::Demand, Option<f64>)> = match demand {
        Demands::One(d) => vec![((0..n).collect(), d, None)],
        Demands::Regional(r) => {
            let mut members = vec![Vec::new(); r.ids.len()];
            for (i, &cell) in cells.iter().enumerate() {
                let k = r.index[cell];
                if k >= 0 {
                    members[k as usize].push(i);
                }
            }
            let mut out = Vec::with_capacity(r.ids.len());
            for (k, g) in members.into_iter().enumerate() {
                out.push((g, r.demand(k)?.as_ref(), Some(r.ids[k])));
            }
            out
        }
    };
    let mut x1 = x0.clone();
    let mut infos = Vec::with_capacity(groups.len());
    for (g, d_all, region) in &groups {
        let m = g.len();
        let d = d_all.at(step, &classes)?;
        let mut tp: Vec<f64> = Vec::with_capacity(m * nc);
        for &i in g {
            tp.extend_from_slice(&tprob[i * nc..(i + 1) * nc]);
        }
        let mut xg: Vec<f64> = g.iter().map(|&i| x0[i]).collect();
        if let Some(rules) = &rules {
            let prev = d_all.at(step - 1, &classes)?;
            let cd: Vec<f64> = d.iter().zip(&prev).map(|(a, b)| a - b).collect();
            let hg: Vec<i64> = g.iter().map(|&i| hist[i]).collect();
            let am = ffi::allow_matrix(&xg, &hg, &classes, &cd, rules)?;
            for (t, a) in tp.iter_mut().zip(&am) {
                *t *= a;
            }
        }
        let mg: Vec<i64> = g.iter().map(|&i| mask[i]).collect();
        let auto = ffi::auto_convert(&xg, &tp, &classes, &mg)?;
        for (i, &a) in auto.iter().enumerate() {
            if !a.is_nan() {
                xg[i] = a;
                tp[i * nc..(i + 1) * nc].fill(f64::NAN);
            }
        }
        for (&i, &x) in g.iter().zip(&xg) {
            x0[i] = x;
        }
        let place = region.map_or(String::new(), |id| format!(" in region {}", class_label(id)));
        let mut info = Vec::new();
        let xr = {
            let mut rng = rng.pin_mut();
            match &s.allocation {
                Allocation::Clue { jitter, scale, max_iter, max_diff, mean_diff } => {
                    let mut t = vec![0.0; m * nc];
                    for i in 0..m {
                        for j in 0..nc {
                            t[j * m + i] = tp[i * nc + j];
                        }
                    }
                    let r = ffi::clues(
                        &t,
                        &xg,
                        &d,
                        &classes,
                        *jitter,
                        *scale,
                        *max_iter,
                        *max_diff,
                        *mean_diff,
                        rng.as_mut(),
                    )?;
                    if !r.converged {
                        ctx.warn(format!(
                            "step {step}{place}: the allocation did not meet the demand in {max_iter} iterations \
                             (largest difference {} cells): the last one is kept",
                            r.max_diff
                        ));
                    }
                    info.push(("iterations".to_string(), r.iterations as f64));
                    info.push(("converged".into(), if r.converged { 1.0 } else { 0.0 }));
                    info.push(("max_diff".into(), r.max_diff));
                    info.push(("mean_diff".into(), r.mean_diff));
                    r.allocation
                }
                Allocation::Ordered { order, stochastic } => {
                    let r = ffi::ordered(&tp, &xg, &d, &classes, order, *stochastic, rng.as_mut())?;
                    if r.short_cells > 0 {
                        ctx.warn(format!(
                            "step {step}{place}: the demand was not met by {} cells: too few cells could take a \
                             class",
                            r.short_cells
                        ));
                    }
                    info.push(("iterations".to_string(), r.iterations as f64));
                    info.push(("converged".into(), if r.converged { 1.0 } else { 0.0 }));
                    r.allocation
                }
            }
        };
        for (&i, &x) in g.iter().zip(&xr) {
            x1[i] = x;
        }
        infos.push(info);
    }
    let mut info = vec![("step".to_string(), step as f64)];
    info.extend(merge_info(&infos));
    let mut out = vec![f64::NAN; land.len()];
    let mut hist_out = vec![f64::NAN; land.len()];
    for (i, &cell) in cells.iter().enumerate() {
        out[cell] = x1[i];
        hist_out[cell] = if x0[i] == x1[i] { (hist[i] + 1) as f64 } else { 0.0 };
    }
    for &c in &classes {
        info.push((format!("cells_{}", class_label(c)), x1.iter().filter(|&&v| v == c).count() as f64));
    }
    let table = Table {
        columns: info.iter().map(|(k, _)| k.clone()).collect(),
        rows: vec![info.iter().map(|(_, v)| *v).collect()],
    };
    Ok(Step {
        landscape: Map::new(landscape.grid.clone(), out),
        history: Map::new(landscape.grid.clone(), hist_out),
        info: table,
    })
}

struct History;

impl Operator for History {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "lulcc.history",
            "The steps each cell has had its class at the start (lulcc's hist): the given map, missing cells \
             counted 1, else 1 everywhere.",
            Access::Local,
        )
        .input(PortSpec::new("landscape", Type::Map, "the map at the start"))
        .input(PortSpec::optional("history", Type::Map, "the steps each cell has had its class"))
        .output(PortSpec::new("history", Type::Map, "the history to carry along the steps"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        Ok(outputs([("history", Value::Map(initial_history(args.map("landscape")?, args.opt_map("history")?)))]))
    }
}

/// The demand input of a step: `demand` or `regional`, one of them.
fn demands<'a>(args: &'a Args) -> Result<Demands<'a>> {
    match (args.get("demand"), args.get("regional")) {
        (Some(_), None) => Ok(Demands::One(args.demand("demand")?.as_ref())),
        (None, Some(_)) => Ok(Demands::Regional(args.regional("regional")?.as_ref())),
        _ => Err(Error::msg(format!("node {:?}: give a demand or a regional demand, one of them", args.node))),
    }
}

struct StepOp;

impl Operator for StepOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "lulcc.step",
            "One step of a CLUE-S or ordered allocation (lulcc's allocate): the elasticity of each cell's class \
             (CLUE), the neighbourhood rules, the conversion matrix with its time and demand codes, the automatic \
             conversions of cells whose class is no longer allowed, then the allocation of the step's demand.",
            Access::Global,
        )
        .input(PortSpec::new("landscape", Type::Map, "the map at the start of the step"))
        .input(PortSpec::new("history", Type::Map, "the steps each cell has had its class"))
        .input(PortSpec::new("suitability", Type::Stack, "a layer per class (e.g. glm.predict)"))
        .input(PortSpec::optional("demand", Type::Demand, "the cells of each class at each step"))
        .input(PortSpec::optional("regional", Type::Regional, "or a demand per region (demand.regional)"))
        .input(PortSpec::new("step", Type::Integer, "the step (1, 2...)"))
        .input(PortSpec::new("rng", Type::Rng, "R's generator, carried from step to step"))
        .input(PortSpec::optional("restrictions", Type::Map, "cells that keep their class (non-zero)"))
        .output(PortSpec::new("landscape", Type::Map, "the map at the end of the step"))
        .output(PortSpec::new("history", Type::Map, "the history updated"))
        .output(PortSpec::new("info", Type::Table, "iterations, convergence and cells of each class"))
        .param(PortSpec::new(
            "allocation",
            Type::Any,
            "{ method = \"clue\", jitter, scale, max_iter, max_diff, mean_diff } or { method = \"ordered\", \
             order = [...], stochastic }",
        ))
        .param(PortSpec::optional("elasticity", Type::Any, "per class (CLUE): one value, a list or { class = value }"))
        .param(PortSpec::optional("allowed", Type::Any, "the conversion matrix, in the demand's class order"))
        .param(PortSpec::optional(
            "neighbourhood",
            Type::Any,
            "{ rules = { class = threshold }, weights = window }",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s: StepSettings = serde_json::from_value(serde_json::Value::Object(args.params.clone()))
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let k = args.integer("step")?;
        if k < 1 {
            return Err(Error::msg("the step must be >= 1"));
        }
        let rng = args.rng("rng")?.clone();
        let mut guard = rng.0.lock().unwrap();
        let r = step(
            ctx,
            args.map("landscape")?,
            args.map("history")?,
            args.stack("suitability")?,
            demands(args)?,
            k as usize,
            &mut guard,
            args.opt_map("restrictions")?,
            &s,
        )?;
        Ok(outputs([
            ("landscape", Value::Map(r.landscape)),
            ("history", Value::Map(r.history)),
            ("info", Value::Table(r.info)),
        ]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(History));
    r.add(Arc::new(StepOp));
}
