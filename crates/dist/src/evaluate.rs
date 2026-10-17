//! What a calibration measures on the worker: the last simulated map against an observed one, so that only
//! numbers travel back; or (a sensitivity analysis) a number of an output.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use terraflux_exec::Results;
use terraflux_ops::validate::{agreement, similarity_of_differences};
use terraflux_ops::{Map, Value};
use terraflux_raster::Raster;

use crate::{Error, Result};

/// The last map of an output compared with an observed map, from the same start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    /// The output whose last map is compared (`simulation.maps`).
    #[serde(default = "maps")]
    pub output: String,
    /// The observed map: a path (`blob:<hash>` when sent to workers).
    pub observed: String,
    /// Its band, by name or 0-based index (default: the last).
    #[serde(default)]
    pub band: Option<String>,
    /// What the simulation starts from: an input (`t0`) or a node's output (`landuse.raster`).
    #[serde(default = "start")]
    pub start: String,
    /// The windows of the fuzzy similarity.
    #[serde(default = "windows")]
    pub windows: Vec<i32>,
    /// Instead of the comparison, the number of an output (`node.port`): a number, a table's column (`statistic`,
    /// its first row) or a map's statistic (`statistic`: mean (default), sum, min, max).
    #[serde(default)]
    pub measure: Option<String>,
    #[serde(default)]
    pub statistic: Option<String>,
}

fn maps() -> String {
    "simulation.maps".into()
}
fn start() -> String {
    "t0".into()
}
fn windows() -> Vec<i32> {
    vec![1, 3, 5, 7, 9, 11]
}

/// The fuzzy similarity of the changes in one window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Similarity {
    pub window: i32,
    #[serde(deserialize_with = "nullable")]
    pub simulated: f64,
    #[serde(deserialize_with = "nullable")]
    pub observed: f64,
    #[serde(deserialize_with = "nullable")]
    pub minimum: f64,
}

/// A number, null for NaN (JSON has none).
fn nullable<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::NAN))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    #[serde(deserialize_with = "nullable")]
    pub figure_of_merit: f64,
    #[serde(deserialize_with = "nullable")]
    pub overall: f64,
    pub hits: i64,
    pub misses: i64,
    pub false_alarms: i64,
    pub wrong_hits: i64,
    pub similarity: Vec<Similarity>,
    /// The number measured (a sensitivity analysis's), NaN for a comparison.
    #[serde(default = "nan", deserialize_with = "nullable")]
    pub value: f64,
}

fn nan() -> f64 {
    f64::NAN
}

impl Metrics {
    /// A metric by name: `fom` (figure_of_merit), `overall`, or `similarity:<window>` (the minimum).
    pub fn get(&self, name: &str) -> Option<f64> {
        match name {
            "fom" | "figure_of_merit" => Some(self.figure_of_merit),
            "value" => Some(self.value),
            "overall" => Some(self.overall),
            _ => {
                let w: i32 = name.strip_prefix("similarity:")?.parse().ok()?;
                self.similarity.iter().find(|s| s.window == w).map(|s| s.minimum)
            }
        }
    }
}

/// The last map of a value: a map, the last of a series or a stack, the last layer of rasters on disk.
pub fn last_map(v: &Value) -> Option<Map> {
    match v {
        Value::Map(m) => Some(m.clone()),
        Value::Series(s) => last_map(s.last()?),
        Value::Stack(s) => Some(Map { grid: s.grid.clone(), data: s.layers.last()?.clone() }),
        Value::Raster(r) => r.choose(r.layers.len().checked_sub(1)?).map().ok(),
        _ => None,
    }
}

/// The first map of a value (the start of a simulation): a map, the first of a series or a stack, the layer
/// chosen of rasters on disk.
pub fn first_map(v: &Value) -> Option<Map> {
    match v {
        Value::Map(m) => Some(m.clone()),
        Value::Series(s) => first_map(s.first()?),
        Value::Stack(s) => Some(Map { grid: s.grid.clone(), data: s.layers.first()?.clone() }),
        Value::Raster(r) => r.map().ok(),
        _ => None,
    }
}

/// The number of a value: itself, a table's column (its first row) or a map's statistic.
pub fn measure(v: &Value, statistic: Option<&str>) -> Result<f64> {
    let stat = statistic.unwrap_or("mean");
    match v {
        Value::Number(_) | Value::Integer(_) => Ok(v.as_number().unwrap()),
        Value::Table(t) => {
            let c = match statistic {
                Some(c) => t.column(c).ok_or_else(|| Error::Other(format!("the table has no column {c:?}")))?,
                None => {
                    t.columns.last().and_then(|c| t.column(c)).ok_or_else(|| Error::Other("an empty table".into()))?
                }
            };
            c.first().copied().ok_or_else(|| Error::Other("the table has no row".into()))
        }
        _ => {
            let m = last_map(v).ok_or_else(|| Error::Other(format!("a {:?} is not measured", v.ty())))?;
            let cells: Vec<f64> = m.data.iter().copied().filter(|x| !x.is_nan()).collect();
            let sum: f64 = cells.iter().sum();
            Ok(match stat {
                "mean" => sum / cells.len() as f64,
                "sum" => sum,
                "min" => cells.iter().copied().fold(f64::INFINITY, f64::min),
                "max" => cells.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                s => return Err(Error::Other(format!("statistic {s:?}: mean, sum, min or max of a map"))),
            })
        }
    }
}

pub fn evaluate(e: &Evaluation, results: &Results, inputs: &BTreeMap<String, Value>) -> Result<Metrics> {
    if let Some(output) = &e.measure {
        let v = results.get(output).ok_or_else(|| Error::Other(format!("the model gives no {output}")))?;
        return Ok(Metrics {
            figure_of_merit: f64::NAN,
            overall: f64::NAN,
            hits: 0,
            misses: 0,
            false_alarms: 0,
            wrong_hits: 0,
            similarity: Vec::new(),
            value: measure(v, e.statistic.as_deref())?,
        });
    }
    let sim =
        results.get(&e.output).and_then(last_map).ok_or_else(|| Error::Other(format!("{} gives no map", e.output)))?;
    let start = match inputs.get(e.start.trim_start_matches('$')) {
        Some(v) => first_map(v),
        None => results.get(&e.start).and_then(first_map),
    };
    let Some(start) = start else {
        return Err(Error::Other(format!("{} gives no map to start from", e.start)));
    };
    let (start, sim) = (&start, &sim);
    let r = Raster::read(&e.observed).map_err(|x| Error::Other(format!("{}: {x}", e.observed)))?;
    let band = match &e.band {
        None => r.bands.len() - 1,
        Some(b) => r
            .bands
            .iter()
            .position(|x| x.name.as_deref() == Some(b.as_str()))
            .or_else(|| b.parse::<usize>().ok().filter(|i| *i < r.bands.len()))
            .ok_or_else(|| Error::Other(format!("{} has no band {b:?}", e.observed)))?,
    };
    if r.grid.width != start.grid.width || r.grid.height != start.grid.height {
        return Err(Error::Other("the observed map is on another grid".into()));
    }
    let observed = Map::new(start.grid.clone(), r.bands[band].masked(r.nodata));
    let a = agreement(&observed, sim, start).map_err(|x| Error::Other(x.to_string()))?;
    let s =
        similarity_of_differences(start, &observed, sim, &e.windows, true).map_err(|x| Error::Other(x.to_string()))?;
    Ok(Metrics {
        figure_of_merit: a.figure_of_merit,
        overall: a.overall,
        hits: a.hits,
        misses: a.misses,
        false_alarms: a.false_alarms,
        wrong_hits: a.wrong_hits,
        similarity: s
            .rows
            .iter()
            .map(|r| Similarity { window: r[0] as i32, simulated: r[1], observed: r[2], minimum: r[3] })
            .collect(),
        value: f64::NAN,
    })
}
