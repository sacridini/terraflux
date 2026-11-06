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

/// The class-level metrics a map's statistic may be (`metric:<name>:<class>`), with the landscape metrics'
/// default options.
pub const METRIC_NAMES: [&str; 5] = ["np", "pland", "ed", "area_mn", "lpi"];

/// Every form of a map's statistic, for messages.
pub const STATISTICS: &str = "mean, sum, min, max, cells:<class>, share:<class>, area:<class>, changed, \
                              metric:<np|pland|ed|area_mn|lpi>:<class>";

/// A map's statistic, parsed.
#[derive(Debug, Clone, PartialEq)]
enum Stat {
    Mean,
    Sum,
    Min,
    Max,
    Cells(f64),
    Share(f64),
    Area(f64),
    Changed,
    Metric(&'static str, f64),
}

fn class_of(text: &str, stat: &str) -> Result<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|c| c.is_finite())
        .ok_or_else(|| Error::Other(format!("statistic {stat:?}: the class {text:?} is not a number")))
}

fn parse_stat(text: &str) -> Result<Stat> {
    let t = text.trim();
    Ok(match t {
        "mean" => Stat::Mean,
        "sum" => Stat::Sum,
        "min" => Stat::Min,
        "max" => Stat::Max,
        "changed" => Stat::Changed,
        _ => {
            let bad = || Error::Other(format!("statistic {t:?}: a map's statistic is {STATISTICS}"));
            let (head, rest) = t.split_once(':').ok_or_else(bad)?;
            match head {
                "cells" => Stat::Cells(class_of(rest, t)?),
                "share" => Stat::Share(class_of(rest, t)?),
                "area" => Stat::Area(class_of(rest, t)?),
                "metric" => {
                    let (name, class) = rest.split_once(':').ok_or_else(bad)?;
                    let name = METRIC_NAMES.iter().find(|m| **m == name.trim()).ok_or_else(bad)?;
                    Stat::Metric(name, class_of(class, t)?)
                }
                _ => return Err(bad()),
            }
        }
    })
}

/// Whether a statistic needs the map the simulation started from (`changed` of a single map).
fn needs_start(statistic: Option<&str>) -> bool {
    statistic.is_some_and(|s| s.trim() == "changed")
}

/// How many hectares a cell of a grid is, when its coordinates are projected in metres (or feet, from the
/// GeoKeys); none when the grid does not say so reliably (then an area is in the map's units squared).
pub fn hectares_per_cell(g: &terraflux_raster::Grid) -> Option<f64> {
    if g.geo.is_geographic() != Some(false) {
        return None;
    }
    let unit = match &g.geo.wkt {
        // a WKT names its unit; only metres are taken
        Some(w) => {
            let w = w.to_ascii_lowercase();
            if w.contains("metre") || w.contains("meter") { 1.0 } else { return None }
        }
        None => g.geo.linear_unit(),
    };
    let (a, b) = g.resolution();
    Some(a * b * unit * unit / 10_000.0)
}

/// The fraction of the cells with a value in both maps whose value differs.
fn changed(first: &Map, last: &Map) -> Result<f64> {
    if first.data.len() != last.data.len() {
        return Err(Error::Other("the start map is on another grid than the output".into()));
    }
    let (mut n, mut d) = (0usize, 0usize);
    for (a, b) in first.data.iter().zip(last.data.iter()) {
        if a.is_nan() || b.is_nan() {
            continue;
        }
        n += 1;
        d += usize::from(a != b);
    }
    Ok(if n == 0 { f64::NAN } else { d as f64 / n as f64 })
}

/// The number of a value: itself, a table's column (its first row) or a map's statistic. Statistics of a map:
/// mean, sum, min, max; `cells:<class>` (the cells of a class), `share:<class>` (their fraction of the cells with
/// a value), `area:<class>` (hectares when the grid is projected in metres, else the map's units squared),
/// `changed` (the fraction of cells whose value differs between the first and last map of a series or stack),
/// `metric:<np|pland|ed|area_mn|lpi>:<class>` (a class metric of the landscape metrics, default options).
pub fn measure(v: &Value, statistic: Option<&str>) -> Result<f64> {
    measure_from(v, statistic, None)
}

/// As [`measure`], with the map the simulation starts from (what `changed` of a single map is against).
pub fn measure_from(v: &Value, statistic: Option<&str>, start: Option<&Value>) -> Result<f64> {
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
            let stat = parse_stat(statistic.unwrap_or("mean"))?;
            let m = last_map(v).ok_or_else(|| Error::Other(format!("a {:?} is not measured", v.ty())))?;
            let cells: Vec<f64> = m.data.iter().copied().filter(|x| !x.is_nan()).collect();
            let sum: f64 = cells.iter().sum();
            let count = |c: f64| cells.iter().filter(|x| **x == c).count() as f64;
            Ok(match stat {
                Stat::Mean => sum / cells.len() as f64,
                Stat::Sum => sum,
                Stat::Min => cells.iter().copied().fold(f64::INFINITY, f64::min),
                Stat::Max => cells.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                Stat::Cells(c) => count(c),
                Stat::Share(c) => count(c) / cells.len() as f64,
                Stat::Area(c) => {
                    let (a, b) = m.grid.resolution();
                    match hectares_per_cell(&m.grid) {
                        Some(ha) => count(c) * ha,
                        None => count(c) * a * b,
                    }
                }
                Stat::Changed => {
                    let first = if matches!(v, Value::Map(_)) {
                        let s = start.ok_or_else(|| {
                            Error::Other("changed of a map is against the start: choose the start map".into())
                        })?;
                        first_map(s)
                    } else {
                        first_map(v)
                    };
                    let first = first.ok_or_else(|| Error::Other("changed: no start map".into()))?;
                    changed(&first, &m)?
                }
                Stat::Metric(name, c) => {
                    let o = terraflux_ops::metrics::default_options();
                    let r = terraflux_ops::metrics::measure(&m, &o, &[("class", name)], false)
                        .map_err(|e| Error::Other(e.to_string()))?;
                    let class = c.trunc() as i32;
                    // (a class absent from the map has no row: zero patches, nothing else)
                    r.rows
                        .iter()
                        .find(|row| row.class == Some(class))
                        .map_or(if name == "np" { 0.0 } else { f64::NAN }, |row| row.value)
                }
            })
        }
    }
}

pub fn evaluate(e: &Evaluation, results: &Results, inputs: &BTreeMap<String, Value>) -> Result<Metrics> {
    if let Some(output) = &e.measure {
        let v = results.get(output).ok_or_else(|| Error::Other(format!("the model gives no {output}")))?;
        // (the start is looked for only when the statistic is against it)
        let from = if needs_start(e.statistic.as_deref()) && matches!(v, Value::Map(_)) {
            let name = e.start.trim().trim_start_matches('$');
            if name.is_empty() {
                return Err(Error::Other("changed of a map is against the start: choose the start map".into()));
            }
            let found = inputs.get(name).or_else(|| results.get(&e.start));
            Some(found.ok_or_else(|| Error::Other(format!("{} gives no map to start from", e.start)))?)
        } else {
            None
        };
        return Ok(Metrics {
            figure_of_merit: f64::NAN,
            overall: f64::NAN,
            hits: 0,
            misses: 0,
            false_alarms: 0,
            wrong_hits: 0,
            similarity: Vec::new(),
            value: measure_from(v, e.statistic.as_deref(), from)?,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use terraflux_raster::Grid;

    use super::*;

    fn map(w: usize, h: usize, data: &[f64]) -> Map {
        Map::new(Arc::new(Grid::new(w, h)), data.to_vec())
    }

    fn stat(m: &Map, s: &str) -> f64 {
        measure(&Value::Map(m.clone()), Some(s)).unwrap()
    }

    #[test]
    fn a_class_is_counted_shared_and_measured_in_area() {
        let nan = f64::NAN;
        let m = map(3, 2, &[1.0, 1.0, 2.0, 2.0, 1.0, nan]);
        assert_eq!(stat(&m, "cells:1"), 3.0);
        assert_eq!(stat(&m, "cells:7"), 0.0);
        assert!((stat(&m, "share:2") - 0.4).abs() < 1e-12);
        // no georeference: the map's units squared
        assert_eq!(stat(&m, "area:1"), 3.0);
        assert_eq!(stat(&m, "mean"), 1.4);
        // 100 m cells in a projected system: a cell is one hectare
        let mut g = Grid::new(3, 2);
        g.transform = [0.0, 100.0, 0.0, 0.0, 0.0, -100.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 1, 1024, 0, 1, 1]);
        assert_eq!(hectares_per_cell(&g), Some(1.0));
        let p = Map::new(Arc::new(g.clone()), m.data.to_vec());
        assert_eq!(stat(&p, "area:1"), 3.0);
        g.geo.geokeys = Some(vec![1, 1, 0, 1, 1024, 0, 1, 2]);
        assert_eq!(hectares_per_cell(&g), None);
        g.geo.geokeys = None;
        assert_eq!(hectares_per_cell(&g), None);
    }

    #[test]
    fn what_changed_is_against_the_start_or_the_first_map() {
        let a = map(2, 2, &[1.0, 1.0, 2.0, f64::NAN]);
        let b = map(2, 2, &[1.0, 2.0, 2.0, 5.0]);
        // a series: first against last, the cells with a value in both
        let series = Value::Series(vec![Value::Map(a.clone()), Value::Map(b.clone())]);
        assert!((measure(&series, Some("changed")).unwrap() - 1.0 / 3.0).abs() < 1e-12);
        // a single map: against the start
        let one = Value::Map(b.clone());
        let r = measure_from(&one, Some("changed"), Some(&Value::Map(a))).unwrap();
        assert!((r - 1.0 / 3.0).abs() < 1e-12);
        let e = measure(&one, Some("changed")).unwrap_err().to_string();
        assert!(e.contains("choose the start map"), "{e}");
    }

    #[test]
    fn a_class_metric_is_read_from_the_landscape_metrics() {
        // two patches of class 1 (4-neighbour) and one of class 2
        let m = map(4, 2, &[1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 2.0, 1.0]);
        assert_eq!(stat(&m, "metric:np:1"), 2.0);
        assert_eq!(stat(&m, "metric:np:2"), 1.0);
        assert_eq!(stat(&m, "metric:np:9"), 0.0);
        assert!((stat(&m, "metric:pland:2") - 25.0).abs() < 1e-9);
        assert!(stat(&m, "metric:lpi:1") > 0.0);
    }

    #[test]
    fn a_wrong_statistic_says_the_valid_ones() {
        let m = Value::Map(map(1, 1, &[1.0]));
        for s in ["median", "share", "metric:np", "metric:zz:1"] {
            let e = measure(&m, Some(s)).unwrap_err().to_string();
            assert!(e.contains("cells:<class>") && e.contains("changed"), "{s}: {e}");
        }
        let e = measure(&m, Some("cells:forest")).unwrap_err().to_string();
        assert!(e.contains("not a number"), "{e}");
    }
}
