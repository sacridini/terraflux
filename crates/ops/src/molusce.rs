//! MOLUSCE's simulator (the QGIS plugin; landschaft's `MolusceCA`): `rng.numpy`, `molusce.total`, `molusce.step`.
//!
//! At each iteration every cell is predicted the transition of largest weight (MOLUSCE's weights of evidence) or
//! the class of largest potential, with a confidence; the cells of each observed transition with the highest
//! confidence (plus a draw of numpy's generator / 1000) change.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::{MolusceChange, MolusceFactor, ffi};

use crate::value::{NumpyRng, class_label};
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

/// The weights of one category set: a transition, a driver, its categories and their weights.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightEntry {
    pub from: f64,
    pub to: f64,
    pub driver: String,
    pub categories: Vec<f64>,
    pub weights: Vec<f64>,
}

/// MOLUSCE's weights of evidence as fitted by landschaft's `MolusceWoE`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weights {
    /// The classes of the model (codes from * m + to).
    pub classes: Vec<f64>,
    /// The drivers, in the order the weights add up.
    pub drivers: Vec<String>,
    /// { driver = [bounds] } for the drivers cut into categories (the others are integer categories).
    #[serde(default)]
    pub bins: BTreeMap<String, Vec<f64>>,
    /// Per transition and driver, in the order of the transitions' codes.
    pub weights: Vec<WeightEntry>,
}

/// The total weight of each transition (landschaft's `MolusceWoE.total`): per transition, the sum over the
/// drivers of the weight of each cell's category; NaN where a driver has no value.
pub fn total(w: &Weights, drivers: &Stack) -> Result<Stack> {
    let n = drivers.layers.first().map_or(0, |l| l.len());
    let mut factors = Vec::with_capacity(w.drivers.len() * n);
    for name in &w.drivers {
        let layer = drivers.by_name(name).ok_or_else(|| Error::msg(format!("drivers lack {name:?}")))?;
        match w.bins.get(name) {
            Some(bounds) => factors.extend(ffi::molusce_categories(layer, bounds)?),
            None => {
                if layer.iter().any(|v| !v.is_nan() && v.fract() != 0.0) {
                    return Err(Error::msg(format!("the driver {name:?} has fractional values: give its bins")));
                }
                factors.extend(layer.iter().copied());
            }
        }
    }
    let mut transitions: Vec<(f64, f64)> = Vec::new();
    for e in &w.weights {
        if !transitions.contains(&(e.from, e.to)) {
            transitions.push((e.from, e.to));
        }
    }
    let mut flat = Vec::with_capacity(transitions.len() * w.drivers.len());
    for &(f, t) in &transitions {
        for name in &w.drivers {
            let e = w.weights.iter().find(|e| (e.from, e.to) == (f, t) && &e.driver == name).ok_or_else(|| {
                Error::msg(format!("no weights of {name:?} for {} -> {}", class_label(f), class_label(t)))
            })?;
            if e.categories.len() != e.weights.len() {
                return Err(Error::msg("one weight per category"));
            }
            flat.push(MolusceFactor { categories: e.categories.clone(), weights: e.weights.clone() });
        }
    }
    let totals = ffi::molusce_total(&factors, w.drivers.len() as i32, n as i64, &flat)?;
    Ok(Stack {
        grid: drivers.grid.clone(),
        keys: transitions.iter().map(|&(f, t)| Key::Transition(f, t)).collect(),
        layers: totals.chunks(n.max(1)).take(transitions.len()).map(|c| Arc::new(c.to_vec())).collect(),
    })
}

/// The cells of each transition at every iteration (the observed cross table).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Count {
    pub from: f64,
    pub to: f64,
    pub cells: i64,
}

/// What a step predicts from: the total weight of each transition, or a potential per class.
pub enum Forecast<'a> {
    Total(&'a Stack),
    Potential(&'a Stack),
}

/// One iteration of MOLUSCE's Simulator (landschaft's `MolusceCA`).
pub fn step(
    land: &Map,
    forecast: Forecast<'_>,
    classes: &[f64],
    counts: &[Count],
    iteration: usize,
    rng: &mut terraflux_ls_sys::NumpyRng,
) -> Result<(Map, Table)> {
    let n = land.data.len();
    let m = classes.len();
    let f = match forecast {
        Forecast::Total(s) => {
            let mut codes = Vec::with_capacity(s.keys.len());
            let mut totals = Vec::with_capacity(s.keys.len() * n);
            for (k, l) in s.keys.iter().zip(&s.layers) {
                let Key::Transition(a, b) = k else {
                    return Err(Error::msg(format!("the total weights are named by transition, not {k}")));
                };
                let pos = |c: f64| {
                    classes.iter().position(|&x| x == c).ok_or_else(|| {
                        Error::msg(format!("the transition {k} has a class the model does not: {}", class_label(c)))
                    })
                };
                codes.push((pos(*a)? * m + pos(*b)?) as i64);
                totals.extend(l.iter().copied());
            }
            ffi::molusce_prediction_woe(&land.data, classes, &codes, &totals)?
        }
        Forecast::Potential(s) => {
            let mut layers = Vec::with_capacity(m * n);
            for &c in classes {
                match s.get(&Key::Class(c)) {
                    Some(l) => layers.extend(l.iter().copied()),
                    None => layers.extend(std::iter::repeat_n(f64::NAN, n)),
                }
            }
            ffi::molusce_prediction_classes(&land.data, &layers, classes)?
        }
    };
    let pairs: Vec<MolusceChange> =
        counts.iter().map(|c| MolusceChange { from: c.from, to: c.to, cells: c.cells }).collect();
    let r = ffi::molusce_ca_step(&land.data, &f.prediction, &f.confidence, &pairs, rng.pin_mut())?;
    let mut columns = vec!["step".to_string()];
    let mut row = vec![iteration as f64];
    for c in &r.changed {
        columns.push(format!("changed_{}_{}", class_label(c.from), class_label(c.to)));
        row.push(c.cells as f64);
    }
    Ok((Map::new(land.grid.clone(), r.state), Table { columns, rows: vec![row], ..Default::default() }))
}

struct RngNumpy;

impl Operator for RngNumpy {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "rng.numpy",
            "numpy's legacy random number generator after numpy.random.seed(seed) (Mersenne-Twister), as MOLUSCE \
             draws: carry it along a loop so that every iteration draws after the previous one.",
            Access::None,
        )
        .input(PortSpec::new("seed", Type::Integer, "the seed (taken modulo 2^32)"))
        .output(PortSpec::new("rng", Type::Rng, "the generator"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let seed = args.integer("seed")?.rem_euclid(1 << 32) as u32;
        let rng = NumpyRng(Arc::new(Mutex::new(terraflux_ls_sys::NumpyRng::new(seed))));
        Ok(outputs([("rng", Value::NumpyRng(rng))]))
    }
}

struct TotalOp;

impl Operator for TotalOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "molusce.total",
            "MOLUSCE's total weight of each transition: per transition, the sum over the drivers of the weight of each \
             cell's category (drivers cut by their bins, or integer categories); NaN where a driver has no value.",
            Access::Local,
        )
        .input(PortSpec::new("drivers", Type::Stack, "the drivers, by name"))
        .output(PortSpec::new("total", Type::Stack, "a layer per transition (\"from_to_to\", persistence included)"))
        .param(PortSpec::new("classes", Type::Any, "the classes of the model"))
        .param(PortSpec::new("drivers", Type::Any, "the drivers, in order"))
        .param(PortSpec::optional("bins", Type::Any, "{ driver = [bounds] }"))
        .param(PortSpec::new("weights", Type::Any, "[{ from, to, driver, categories, weights }]"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let w: Weights = serde_json::from_value(serde_json::Value::Object(args.params.clone()))
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        Ok(outputs([("total", Value::Stack(total(&w, args.stack("drivers")?)?))]))
    }
}

struct StepOp;

impl Operator for StepOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "molusce.step",
            "One iteration of MOLUSCE's Simulator: every cell is predicted the transition of largest total weight (or \
             the class of largest potential) with a confidence (100 x the gap to the second, truncated); then for each \
             pair of classes of the map, the cells predicted to take it with the highest confidence + U(0, 1) / 1000 \
             change, as many as the observed cross table says.",
            Access::Global,
        )
        .input(PortSpec::new("landscape", Type::Map, "the map at the start of the iteration"))
        .input(PortSpec::optional("total", Type::Stack, "the total weight of each transition (molusce.total)"))
        .input(PortSpec::optional("potential", Type::Stack, "or a potential per class"))
        .input(PortSpec::new("rng", Type::Rng, "numpy's generator (rng.numpy), carried from step to step"))
        .input(PortSpec::new("step", Type::Integer, "the iteration (1, 2...)"))
        .output(PortSpec::new("landscape", Type::Map, "the map at the end of the iteration"))
        .output(PortSpec::new("info", Type::Table, "the cells each transition changed"))
        .param(PortSpec::new("classes", Type::Any, "the classes of the model"))
        .param(PortSpec::new("counts", Type::Any, "the cells of each transition: [{ from, to, cells }]"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let classes: Vec<f64> =
            args.opt_param("classes")?.ok_or_else(|| Error::msg(format!("node {:?}: give the classes", args.node)))?;
        let counts: Vec<Count> =
            args.opt_param("counts")?.ok_or_else(|| Error::msg(format!("node {:?}: give the counts", args.node)))?;
        let forecast = match (args.opt_stack("total")?, args.opt_stack("potential")?) {
            (Some(t), None) => Forecast::Total(t),
            (None, Some(p)) => Forecast::Potential(p),
            _ => return Err(Error::msg(format!("node {:?}: give the total weights or a potential", args.node))),
        };
        let k = args.integer("step")?;
        let rng = args.numpy_rng("rng")?.clone();
        let mut guard = rng.0.lock().unwrap();
        let (m, info) = step(args.map("landscape")?, forecast, &classes, &counts, k.max(0) as usize, &mut guard)?;
        Ok(outputs([("landscape", Value::Map(m)), ("info", Value::Table(info))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(RngNumpy));
    r.add(Arc::new(TotalOp));
    r.add(Arc::new(StepOp));
}
