//! Statistics of rasters: a summary of a map or of every layer of a stack (`raster.stats`, with a histogram), a
//! statistic of a stack's layers at every cell (`stack.reduce`), the statistics of a map in the zones of another map
//! (`raster.zonal`, with each cell given its zone's) and the area of each class in them (`raster.tabulate`), the
//! correlation and covariance between maps (`raster.correlation`), and in a moving window between two
//! (`raster.local_correlation`), and maps rescaled to a common scale (`raster.standardize`).
//!
//! The statistics are vector.zonal's (rasterstats's): count, nodata, sum, mean, min, max, range, std, var, median,
//! majority, minority, unique, percentile_<p>; std and var of the population (numpy's), the percentiles
//! interpolated linearly (numpy's), majority and minority the smallest value of a tie. Missing cells are left out.

use std::collections::HashMap;
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;

use crate::vector::stat_code;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Stats));
    r.add(Arc::new(Reduce));
    r.add(Arc::new(Zonal));
    r.add(Arc::new(Tabulate));
    r.add(Arc::new(Correlation));
    r.add(Arc::new(LocalCorrelation));
    r.add(Arc::new(Standardize));
}

/// The layers of a map, a stack or a series of maps, on one grid: their names and values.
pub(crate) struct Layers {
    pub grid: Arc<Grid>,
    pub names: Vec<String>,
    pub data: Vec<Arc<Vec<f64>>>,
}

pub(crate) fn layers(args: &Args, port: &str) -> Result<Layers> {
    let node = args.node;
    let one = |m: &Map| Layers { grid: m.grid.clone(), names: vec![port.to_string()], data: vec![m.data.clone()] };
    let of_stack = |s: &Stack| Layers {
        grid: s.grid.clone(),
        names: s.keys.iter().map(|k| k.to_string()).collect(),
        data: s.layers.clone(),
    };
    match args.get(port) {
        None => Err(Error::msg(format!("node {node:?}: no input {port:?}"))),
        Some(Value::Map(m)) => Ok(one(m)),
        Some(Value::Stack(s)) => Ok(of_stack(s)),
        Some(Value::Raster(r)) if r.layers.len() > 1 && !r.picked => Ok(of_stack(&r.stack()?)),
        Some(Value::Raster(r)) => Ok(one(&r.map()?)),
        Some(Value::Series(steps)) => {
            let mut out: Option<Layers> = None;
            for (i, v) in steps.iter().enumerate() {
                let Value::Map(m) = v else {
                    return Err(Error::msg(format!(
                        "node {node:?}: {port} is a series of {}, not of maps",
                        v.ty().name()
                    )));
                };
                let l = out.get_or_insert_with(|| Layers { grid: m.grid.clone(), names: vec![], data: vec![] });
                if !l.grid.same_as(&m.grid) {
                    return Err(Error::msg(format!("node {node:?}: the maps of {port} are not of one grid")));
                }
                l.names.push(format!("{port}_{}", i + 1));
                l.data.push(m.data.clone());
            }
            out.ok_or_else(|| Error::msg(format!("node {node:?}: {port} is an empty series")))
        }
        Some(other) => Err(Error::msg(format!(
            "node {node:?}: {port} is a {}, not a map, a stack or a series of maps",
            other.ty().name()
        ))),
    }
}

/// The statistics a node asks for by a setting (a name or a list), its default when not given.
fn stat_names(args: &Args, name: &str, default: &[&str]) -> Result<Vec<(String, (i32, f64))>> {
    let names: Vec<String> = match args.params.get(name) {
        None | Some(serde_json::Value::Null) => default.iter().map(|s| s.to_string()).collect(),
        Some(serde_json::Value::String(s)) => {
            s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
        }
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| Error::msg(format!("node {:?}: the setting {name:?}: {e}", args.node)))?,
    };
    names
        .into_iter()
        .map(|s| match stat_code(&s) {
            Some(c) => Ok((s, c)),
            None => Err(Error::msg(format!(
                "node {:?}: unknown statistic {s:?} (count, nodata, sum, mean, min, max, range, std, var, median, \
                 majority, minority, unique, percentile_<p>)",
                args.node
            ))),
        })
        .collect()
}

/// A percentile (0-100) of values sorted, interpolated linearly (numpy's).
fn percentile(sorted: &[f64], q: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    let pos = q / 100.0 * (n - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = (lo + 1).min(n - 1);
    let t = pos - lo as f64;
    sorted[lo] + (sorted[hi] - sorted[lo]) * t
}

/// A statistic (by `stat_code`) of the values with one, sorted, and the count of those without.
fn statistic(code: (i32, f64), sorted: &[f64], missing: usize) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return match code.0 {
            0 | 12 => 0.0,
            1 => missing as f64,
            _ => f64::NAN,
        };
    }
    let mean = || sorted.iter().sum::<f64>() / n as f64;
    let var = || {
        let m = mean();
        sorted.iter().map(|x| (x - m).powi(2)).sum::<f64>() / n as f64
    };
    // the values and how many times each comes, increasing
    let runs = || {
        let mut out: Vec<(f64, usize)> = Vec::new();
        for &x in sorted {
            match out.last_mut() {
                Some((v, c)) if *v == x => *c += 1,
                _ => out.push((x, 1)),
            }
        }
        out
    };
    match code.0 {
        0 => n as f64,
        1 => missing as f64,
        2 => sorted.iter().sum(),
        3 => mean(),
        4 => sorted[0],
        5 => sorted[n - 1],
        6 => sorted[n - 1] - sorted[0],
        7 => var().sqrt(),
        8 => var(),
        9 => percentile(sorted, 50.0),
        10 => {
            let r = runs();
            let most = r.iter().map(|x| x.1).max().unwrap_or(0);
            r.iter().find(|x| x.1 == most).map_or(f64::NAN, |x| x.0)
        }
        11 => {
            let r = runs();
            let least = r.iter().map(|x| x.1).min().unwrap_or(0);
            r.iter().find(|x| x.1 == least).map_or(f64::NAN, |x| x.0)
        }
        12 => runs().len() as f64,
        _ => percentile(sorted, code.1),
    }
}

/// The values of a layer with one, sorted, and the count of those without.
fn sorted_values(data: &[f64]) -> (Vec<f64>, usize) {
    let mut v: Vec<f64> = data.iter().copied().filter(|x| !x.is_nan()).collect();
    let missing = data.len() - v.len();
    v.par_sort_unstable_by(f64::total_cmp);
    (v, missing)
}

/// `raster.stats`.
struct Stats;

const SUMMARY: [&str; 12] = [
    "count",
    "nodata",
    "sum",
    "mean",
    "std",
    "min",
    "percentile_5",
    "percentile_25",
    "median",
    "percentile_75",
    "percentile_95",
    "max",
];

impl Operator for Stats {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.stats",
            "A summary of a map, or of every layer of a stack or of a series of maps, as tables: the cells with a \
             value (count) and without (nodata), the sum, mean, standard deviation, minimum, percentiles and maximum \
             (the statistics of vector.zonal over the whole map), and a histogram of bins of equal width.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Any, "a map, a stack or a series of maps"))
        .output(PortSpec::new("table", Type::Table, "a row per layer (1.., in the stack's order): its statistics"))
        .output(PortSpec::new(
            "histogram",
            Type::Table,
            "a row per layer and bin: layer, bin (1..), from, to, count and share of the cells with a value",
        ))
        .param(PortSpec::optional(
            "stats",
            Type::Any,
            "the statistics: count, nodata, sum, mean, min, max, range, std, var, median, majority, minority, \
             unique, percentile_<p> (default count, nodata, sum, mean, std, min, percentiles 5, 25, 50, 75, 95, max)",
        ))
        .param(PortSpec::optional("bins", Type::Integer, "the histogram's bins (20)"))
        .param(PortSpec::optional(
            "range",
            Type::Any,
            "the histogram's [from, to] (default each layer's minimum and maximum); values outside left out",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let l = layers(args, "map")?;
        let stats = stat_names(args, "stats", &SUMMARY)?;
        let bins: usize = args.param("bins", 20)?;
        if bins == 0 {
            return Err(Error::msg(format!("node {node:?}: a histogram of no bins")));
        }
        let range: Option<[f64; 2]> = args.opt_param("range")?;
        if let Some([a, b]) = range
            && (a >= b || a.is_nan() || b.is_nan())
        {
            return Err(Error::msg(format!("node {node:?}: the range's from must be below its to")));
        }
        let mut table =
            Table::new(std::iter::once("layer".to_string()).chain(stats.iter().map(|s| s.0.clone())).collect());
        let mut histogram = Table::new(["layer", "bin", "from", "to", "count", "share"].map(String::from).to_vec());
        for (i, data) in l.data.iter().enumerate() {
            let layer = (i + 1) as f64;
            let (sorted, missing) = sorted_values(data);
            let mut row = vec![layer];
            row.extend(stats.iter().map(|s| statistic(s.1, &sorted, missing)));
            table.rows.push(row);
            if sorted.is_empty() {
                continue;
            }
            let [lo, hi] = range.unwrap_or([sorted[0], sorted[sorted.len() - 1]]);
            let width = (hi - lo) / bins as f64;
            let mut counts = vec![0usize; bins];
            for &x in &sorted {
                if x < lo || x > hi {
                    continue;
                }
                // (numpy's: the last bin holds its right edge)
                let b = if width > 0.0 { (((x - lo) / width) as usize).min(bins - 1) } else { 0 };
                counts[b] += 1;
            }
            let n = sorted.len() as f64;
            for (b, &c) in counts.iter().enumerate() {
                let from = lo + width * b as f64;
                let to = if b + 1 == bins { hi } else { lo + width * (b + 1) as f64 };
                histogram.rows.push(vec![layer, (b + 1) as f64, from, to, c as f64, c as f64 / n]);
            }
        }
        Ok(outputs([("table", Value::Table(table)), ("histogram", Value::Table(histogram))]))
    }
}

/// `stack.reduce`.
struct Reduce;

impl Operator for Reduce {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "stack.reduce",
            "A statistic of a stack's layers (or of a series of maps) at every cell, as terra's app or a cell \
             statistics tool: the mean, standard deviation, minimum, maximum, median, sum, count of layers with a \
             value, majority (the mode), a percentile... over the layers with a value at the cell (none: missing), \
             or the layer (1.., in the stack's order) of the largest or smallest value (argmax, argmin; a tie: the \
             first).",
            Access::None,
        )
        .input(PortSpec::new("maps", Type::Any, "a stack or a series of maps"))
        .output(PortSpec::new("map", Type::Map, "the statistic at every cell"))
        .param(
            PortSpec::optional(
                "stat",
                Type::Text,
                "mean (default), std, var, min, max, range, median, sum, count, nodata, majority, minority, \
                 unique, percentile_<p>, argmax, argmin",
            )
            .choices([
                "mean",
                "std",
                "var",
                "min",
                "max",
                "range",
                "median",
                "sum",
                "count",
                "nodata",
                "majority",
                "minority",
                "unique",
                "percentile_90",
                "argmax",
                "argmin",
            ]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let l = layers(args, "maps")?;
        let stat: String = args.param("stat", "mean".to_string())?;
        let arg = match stat.as_str() {
            "argmax" => Some(true),
            "argmin" => Some(false),
            _ => None,
        };
        let code = match arg {
            Some(_) => (0, 0.0),
            None => stat_code(&stat).ok_or_else(|| Error::msg(format!("node {node:?}: unknown statistic {stat:?}")))?,
        };
        let k = l.data.len();
        let mut out = vec![f64::NAN; l.grid.cells()];
        out.par_chunks_mut(l.grid.width.max(1)).enumerate().for_each(|(row, line)| {
            let mut values = Vec::with_capacity(k);
            for (c, o) in line.iter_mut().enumerate() {
                let cell = row * l.grid.width + c;
                if let Some(largest) = arg {
                    let mut best: Option<(usize, f64)> = None;
                    for (i, d) in l.data.iter().enumerate() {
                        let x = d[cell];
                        if x.is_nan() {
                            continue;
                        }
                        if best.is_none_or(|(_, b)| if largest { x > b } else { x < b }) {
                            best = Some((i, x));
                        }
                    }
                    *o = best.map_or(f64::NAN, |(i, _)| (i + 1) as f64);
                    continue;
                }
                values.clear();
                values.extend(l.data.iter().map(|d| d[cell]).filter(|x| !x.is_nan()));
                values.sort_unstable_by(f64::total_cmp);
                *o = statistic(code, &values, k - values.len());
            }
        });
        Ok(outputs([("map", Value::Map(Map::new(l.grid.clone(), out)))]))
    }
}

/// The zones of a map of zone ids (missing: none), increasing, and the zone (its index) of every cell.
fn zones_of(zones: &Map) -> (Vec<f64>, Vec<Option<usize>>) {
    let ids = zones.classes();
    let at: HashMap<u64, usize> = ids.iter().enumerate().map(|(i, z)| (z.to_bits(), i)).collect();
    let of = zones.data.iter().map(|z| if z.is_nan() { None } else { at.get(&z.to_bits()).copied() }).collect();
    (ids, of)
}

/// The grid of a map against another's, or why they differ.
fn same_grid(node: &str, a: &Grid, b: &Grid, what: &str) -> Result<()> {
    if a.width != b.width || a.height != b.height {
        return Err(Error::msg(format!(
            "node {node:?}: {what} are of different sizes ({}x{} and {}x{} cells): raster.match makes them one grid",
            a.width, a.height, b.width, b.height
        )));
    }
    Ok(())
}

/// `raster.zonal`.
struct Zonal;

impl Operator for Zonal {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.zonal",
            "Statistics of a map (or of every layer of a stack) in the zones of a map of zone ids (municipalities, \
             watersheds, classes rasterized), as GRASS's r.univar by zones or terra's zonal: a row per zone, and a \
             map of every cell given its zone's statistic (zonal statistics as a raster). The statistics of \
             vector.zonal; the cells without a zone or without a value left out.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Any, "a map, a stack or a series of maps"))
        .input(PortSpec::new("zones", Type::Map, "the zone of every cell (its id)"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per zone (increasing): zone, then every statistic (of every layer: <layer>_<statistic>)",
        ))
        .output(PortSpec::new("map", Type::Map, "every cell given its zone's statistic `to_map` (of the first layer)"))
        .param(PortSpec::optional(
            "stats",
            Type::Any,
            "count, nodata, sum, mean, min, max, range, std, var, median, majority, minority, unique, \
             percentile_<p> (default count, mean, min, max, std)",
        ))
        .param(PortSpec::optional("to_map", Type::Text, "the statistic of the map (mean)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let l = layers(args, "map")?;
        let zones = args.map("zones")?;
        same_grid(node, &l.grid, &zones.grid, "the map and the zones")?;
        let stats = stat_names(args, "stats", &["count", "mean", "min", "max", "std"])?;
        let to_map: String = args.param("to_map", "mean".to_string())?;
        let to_map =
            stat_code(&to_map).ok_or_else(|| Error::msg(format!("node {node:?}: unknown statistic {to_map:?}")))?;
        let (ids, of) = zones_of(zones);
        let multi = l.data.len() > 1;
        let mut columns = vec!["zone".to_string()];
        for name in &l.names {
            for s in &stats {
                columns.push(if multi { format!("{name}_{}", s.0) } else { s.0.clone() });
            }
        }
        let mut table = Table::new(columns);
        table.rows = ids.iter().map(|&z| vec![z]).collect();
        let mut first = Vec::new();
        for (i, data) in l.data.iter().enumerate() {
            let mut values: Vec<Vec<f64>> = vec![Vec::new(); ids.len()];
            let mut missing = vec![0usize; ids.len()];
            for (x, z) in data.iter().zip(&of) {
                if let Some(z) = *z {
                    if x.is_nan() { missing[z] += 1 } else { values[z].push(*x) }
                }
            }
            values.par_iter_mut().for_each(|v| v.sort_unstable_by(f64::total_cmp));
            for (z, v) in values.iter().enumerate() {
                table.rows[z].extend(stats.iter().map(|s| statistic(s.1, v, missing[z])));
            }
            if i == 0 {
                first = values.iter().zip(&missing).map(|(v, &m)| statistic(to_map, v, m)).collect();
            }
        }
        let map: Vec<f64> = of.iter().map(|z| z.map_or(f64::NAN, |z| first[z])).collect();
        Ok(outputs([("table", Value::Table(table)), ("map", Value::Map(Map::new(l.grid.clone(), map)))]))
    }
}

/// `raster.tabulate`.
struct Tabulate;

impl Operator for Tabulate {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.tabulate",
            "The area of every class of a map in every zone of another, as ArcGIS's Tabulate Area or a cross \
             tabulation: a row per zone, a column per class (class_<k>) and the zone's total; in cells, in area \
             (the map's units squared), in hectares (of a map in metres) or as each class's share of the zone. \
             Cells without a zone or a class left out.",
            Access::None,
        )
        .input(PortSpec::new("zones", Type::Map, "the zone of every cell (its id)"))
        .input(PortSpec::new("classes", Type::Map, "the class of every cell"))
        .output(PortSpec::new("table", Type::Table, "zone, class_<k> for every class, total"))
        .param(
            PortSpec::optional("units", Type::Text, "cells (default), area, hectares or share")
                .choices(["cells", "area", "hectares", "share"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let zones = args.map("zones")?;
        let classes = args.map("classes")?;
        same_grid(node, &zones.grid, &classes.grid, "the zones and the classes")?;
        let units: String = args.param("units", "cells".to_string())?;
        let (w, h) = zones.grid.resolution();
        let unit = match units.as_str() {
            "cells" | "share" => 1.0,
            "area" => w * h,
            "hectares" => w * h / 10_000.0,
            u => return Err(Error::msg(format!("node {node:?}: units {u:?}: cells, area, hectares or share"))),
        };
        let (ids, of) = zones_of(zones);
        let (kinds, class_of) = zones_of(classes);
        let mut counts = vec![vec![0usize; kinds.len()]; ids.len()];
        for (z, k) in of.iter().zip(&class_of) {
            if let (Some(z), Some(k)) = (z, k) {
                counts[*z][*k] += 1;
            }
        }
        let mut columns = vec!["zone".to_string()];
        columns.extend(kinds.iter().map(|k| format!("class_{}", crate::value::class_label(*k))));
        columns.push("total".into());
        let mut table = Table::new(columns);
        for (z, c) in ids.iter().zip(&counts) {
            let total: usize = c.iter().sum();
            let scale = if units == "share" { if total > 0 { 1.0 / total as f64 } else { f64::NAN } } else { unit };
            let mut row = vec![*z];
            row.extend(c.iter().map(|&n| n as f64 * scale));
            row.push(total as f64 * scale);
            table.rows.push(row);
        }
        Ok(outputs([("table", Value::Table(table))]))
    }
}

/// The ranks of values (1.., ties given their mean rank).
fn ranks(x: &[f64]) -> Vec<f64> {
    let mut order: Vec<usize> = (0..x.len()).collect();
    order.sort_by(|&a, &b| x[a].total_cmp(&x[b]));
    let mut r = vec![0.0; x.len()];
    let mut i = 0;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && x[order[j + 1]] == x[order[i]] {
            j += 1;
        }
        let rank = (i + j) as f64 / 2.0 + 1.0;
        for &o in &order[i..=j] {
            r[o] = rank;
        }
        i = j + 1;
    }
    r
}

/// The least squares fit of y on x: n, correlation, covariance (n - 1), slope, intercept.
fn fit(x: &[f64], y: &[f64]) -> [f64; 5] {
    let n = x.len() as f64;
    if x.len() < 2 {
        return [n, f64::NAN, f64::NAN, f64::NAN, f64::NAN];
    }
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxx += (a - mx) * (a - mx);
        syy += (b - my) * (b - my);
        sxy += (a - mx) * (b - my);
    }
    let r = if sxx > 0.0 && syy > 0.0 { sxy / (sxx * syy).sqrt() } else { f64::NAN };
    let slope = if sxx > 0.0 { sxy / sxx } else { f64::NAN };
    [n, r, sxy / (n - 1.0), slope, my - slope * mx]
}

/// `raster.correlation`.
struct Correlation;

impl Operator for Correlation {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.correlation",
            "The correlation and covariance between maps over their cells, as terra's layerCor or R's cor: between \
             every two layers of a stack (or of a series of maps), and those of `y` when given; Pearson's or \
             Spearman's (of the ranks, ties given their mean rank). Every pair over the cells where both have a \
             value; with the least squares line of the second on the first.",
            Access::None,
        )
        .input(PortSpec::new("x", Type::Any, "a map, a stack or a series of maps"))
        .input(PortSpec::optional("y", Type::Any, "another map or stack (of the same grid)"))
        .output(PortSpec::new(
            "matrix",
            Type::Table,
            "the correlations: a row per layer (1..), a column per layer by its name",
        ))
        .output(PortSpec::new("covariance", Type::Table, "the covariances (n - 1), as the matrix"))
        .output(PortSpec::new(
            "pairs",
            Type::Table,
            "a row per pair of layers: x, y (the layers), n (cells), correlation, covariance, slope, intercept, r2",
        ))
        .param(
            PortSpec::optional("method", Type::Text, "pearson (default) or spearman").choices(["pearson", "spearman"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let mut l = layers(args, "x")?;
        if args.get("y").is_some() {
            let y = layers(args, "y")?;
            same_grid(node, &l.grid, &y.grid, "x and y")?;
            l.names.extend(y.names);
            l.data.extend(y.data);
        }
        let k = l.data.len();
        if k < 2 {
            return Err(Error::msg(format!(
                "node {node:?}: a correlation is between two maps: x a stack, or a map and y another"
            )));
        }
        let spearman = match args.param("method", "pearson".to_string())?.as_str() {
            "pearson" => false,
            "spearman" => true,
            m => return Err(Error::msg(format!("node {node:?}: method {m:?}: pearson or spearman"))),
        };
        let pairs: Vec<(usize, usize)> = (0..k).flat_map(|a| (a + 1..k).map(move |b| (a, b))).collect();
        let fits: Vec<([f64; 5], f64)> = pairs
            .par_iter()
            .map(|&(a, b)| {
                let (x, y): (Vec<f64>, Vec<f64>) = l.data[a]
                    .iter()
                    .zip(l.data[b].iter())
                    .filter(|(x, y)| !x.is_nan() && !y.is_nan())
                    .map(|(x, y)| (*x, *y))
                    .unzip();
                let f = fit(&x, &y);
                let (r, cov) = if spearman {
                    let g = fit(&ranks(&x), &ranks(&y));
                    (g[1], g[2])
                } else {
                    (f[1], f[2])
                };
                ([f[0], r, cov, f[3], f[4]], f[1])
            })
            .collect();
        let variance = |d: &[f64]| {
            let v: Vec<f64> = d.iter().copied().filter(|x| !x.is_nan()).collect();
            let v = if spearman { ranks(&v) } else { v };
            let n = v.len() as f64;
            let m = v.iter().sum::<f64>() / n;
            v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)
        };
        let columns: Vec<String> = std::iter::once("layer".to_string()).chain(l.names.iter().cloned()).collect();
        let mut matrix = Table::new(columns.clone());
        let mut covariance = Table::new(columns);
        for a in 0..k {
            let mut r = vec![(a + 1) as f64];
            let mut c = vec![(a + 1) as f64];
            for b in 0..k {
                if a == b {
                    let v = variance(&l.data[a]);
                    r.push(if v > 0.0 { 1.0 } else { f64::NAN });
                    c.push(v);
                    continue;
                }
                let i = pairs.iter().position(|&p| p == (a.min(b), a.max(b))).unwrap_or(0);
                r.push(fits[i].0[1]);
                c.push(fits[i].0[2]);
            }
            matrix.rows.push(r);
            covariance.rows.push(c);
        }
        let mut table = Table::new(
            ["x", "y", "n", "correlation", "covariance", "slope", "intercept", "r2"].map(String::from).to_vec(),
        );
        for (&(a, b), (f, pearson)) in pairs.iter().zip(&fits) {
            table.rows.push(vec![(a + 1) as f64, (b + 1) as f64, f[0], f[1], f[2], f[3], f[4], pearson * pearson]);
        }
        Ok(outputs([
            ("matrix", Value::Table(matrix)),
            ("covariance", Value::Table(covariance)),
            ("pairs", Value::Table(table)),
        ]))
    }
}

/// The cells of a window around a cell (row and column offsets): a square, or a circle of the radius.
fn window(radius: usize, circle: bool) -> Vec<(isize, isize)> {
    let r = radius as isize;
    (-r..=r)
        .flat_map(|dr| (-r..=r).map(move |dc| (dr, dc)))
        .filter(|(dr, dc)| !circle || dr * dr + dc * dc <= r * r)
        .collect()
}

/// `raster.local_correlation`.
struct LocalCorrelation;

impl Operator for LocalCorrelation {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.local_correlation",
            "The correlation and the least squares line between two maps in a moving window around every cell \
             (geographically local regression, as terra's focalPairs or focalReg): Pearson's correlation, the \
             slope and intercept of y on x and r², over the cells of the window where both have a value. No value \
             where a map has none at the cell, or the window fewer than `min_cells` pairs.",
            Access::None,
        )
        .input(PortSpec::new("x", Type::Map, "the first map"))
        .input(PortSpec::new("y", Type::Map, "the second map (of the same grid)"))
        .output(PortSpec::new("correlation", Type::Map, "Pearson's correlation in the window"))
        .output(PortSpec::new("slope", Type::Map, "the slope of y on x"))
        .output(PortSpec::new("intercept", Type::Map, "the intercept of y on x"))
        .output(PortSpec::new("r2", Type::Map, "the share of y's variance the line explains"))
        .param(PortSpec::optional("radius", Type::Integer, "the window's radius in cells (3: 7 x 7)"))
        .param(PortSpec::optional("shape", Type::Text, "square (default) or circle").choices(["square", "circle"]))
        .param(PortSpec::optional("min_cells", Type::Integer, "the fewest pairs of a window (3)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let x = args.map("x")?;
        let y = args.map("y")?;
        same_grid(node, &x.grid, &y.grid, "x and y")?;
        let radius: usize = args.param("radius", 3)?;
        let circle = match args.param("shape", "square".to_string())?.as_str() {
            "square" => false,
            "circle" => true,
            s => return Err(Error::msg(format!("node {node:?}: shape {s:?}: square or circle"))),
        };
        let least: usize = args.param("min_cells", 3)?;
        let least = least.max(2);
        let offsets = window(radius, circle);
        let (w, h) = (x.grid.width, x.grid.height);
        let rows: Vec<Vec<[f64; 4]>> = (0..h)
            .into_par_iter()
            .map(|r| {
                (0..w)
                    .map(|c| {
                        let (x0, y0) = (x.data[r * w + c], y.data[r * w + c]);
                        if x0.is_nan() || y0.is_nan() {
                            return [f64::NAN; 4];
                        }
                        // the sums about the centre's values (fewer digits lost)
                        let (mut n, mut sx, mut sy, mut sxx, mut syy, mut sxy) = (0usize, 0.0, 0.0, 0.0, 0.0, 0.0);
                        for &(dr, dc) in &offsets {
                            let (rr, cc) = (r as isize + dr, c as isize + dc);
                            if rr < 0 || cc < 0 || rr >= h as isize || cc >= w as isize {
                                continue;
                            }
                            let i = rr as usize * w + cc as usize;
                            let (a, b) = (x.data[i], y.data[i]);
                            if a.is_nan() || b.is_nan() {
                                continue;
                            }
                            let (a, b) = (a - x0, b - y0);
                            n += 1;
                            sx += a;
                            sy += b;
                            sxx += a * a;
                            syy += b * b;
                            sxy += a * b;
                        }
                        if n < least {
                            return [f64::NAN; 4];
                        }
                        let k = n as f64;
                        let vxx = sxx - sx * sx / k;
                        let vyy = syy - sy * sy / k;
                        let vxy = sxy - sx * sy / k;
                        let r = if vxx > 0.0 && vyy > 0.0 { vxy / (vxx * vyy).sqrt() } else { f64::NAN };
                        let slope = if vxx > 0.0 { vxy / vxx } else { f64::NAN };
                        let intercept = (y0 + sy / k) - slope * (x0 + sx / k);
                        [r, slope, intercept, r * r]
                    })
                    .collect()
            })
            .collect();
        let part = |i: usize| -> Value {
            Value::Map(Map::new(x.grid.clone(), rows.iter().flat_map(|row| row.iter().map(|v| v[i])).collect()))
        };
        Ok(outputs([("correlation", part(0)), ("slope", part(1)), ("intercept", part(2)), ("r2", part(3))]))
    }
}

/// `raster.standardize`.
struct Standardize;

impl Operator for Standardize {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.standardize",
            "A map (or every layer of a stack, or every map of a series) put on a common scale, for drivers and \
             predictors compared or combined: the z-score ((x - mean) / std, the std of the population), \
             min-max to 0-1, or the percentile stretch to 0-1 between two percentiles (the values beyond them 0 \
             or 1). A layer of one value is all 0. The centre and scale of each layer in a table, to standardize \
             other maps the same way (map.calc).",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Any, "a map, a stack or a series of maps"))
        .output(PortSpec::new("map", Type::Any, "the same, standardized: a map, a stack or a series"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per layer (1..): center and scale, the value standardized (x - center) / scale",
        ))
        .param(PortSpec::optional("method", Type::Text, "zscore (default), minmax or percentile").choices([
            "zscore",
            "minmax",
            "percentile",
        ]))
        .param(PortSpec::optional("low", Type::Number, "percentile: the lower percentile (2)"))
        .param(PortSpec::optional("high", Type::Number, "percentile: the upper percentile (98)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let l = layers(args, "map")?;
        let method: String = args.param("method", "zscore".to_string())?;
        let (low, high): (f64, f64) = (args.param("low", 2.0)?, args.param("high", 98.0)?);
        if method == "percentile" && !(0.0 <= low && low < high && high <= 100.0) {
            return Err(Error::msg(format!("node {node:?}: the percentiles: 0 <= low < high <= 100")));
        }
        let mut table = Table::new(["layer", "center", "scale"].map(String::from).to_vec());
        let mut out = Vec::new();
        for (i, data) in l.data.iter().enumerate() {
            let (sorted, missing) = sorted_values(data);
            let (center, scale) = match method.as_str() {
                "zscore" => (statistic((3, 0.0), &sorted, missing), statistic((7, 0.0), &sorted, missing)),
                "minmax" => {
                    let lo = statistic((4, 0.0), &sorted, missing);
                    (lo, statistic((5, 0.0), &sorted, missing) - lo)
                }
                "percentile" => {
                    let lo = percentile(&sorted, low);
                    (lo, percentile(&sorted, high) - lo)
                }
                m => return Err(Error::msg(format!("node {node:?}: method {m:?}: zscore, minmax or percentile"))),
            };
            table.rows.push(vec![(i + 1) as f64, center, scale]);
            let clamp = method == "percentile";
            let values: Vec<f64> = data
                .iter()
                .map(|&x| {
                    if x.is_nan() {
                        return f64::NAN;
                    }
                    let v = if scale > 0.0 { (x - center) / scale } else { 0.0 };
                    if clamp { v.clamp(0.0, 1.0) } else { v }
                })
                .collect();
            out.push(Arc::new(values));
        }
        let map = match args.get("map") {
            Some(Value::Stack(s)) => Value::Stack(Stack { grid: s.grid.clone(), keys: s.keys.clone(), layers: out }),
            Some(Value::Raster(r)) if r.layers.len() > 1 && !r.picked => {
                let s = r.stack()?;
                Value::Stack(Stack { grid: s.grid, keys: s.keys, layers: out })
            }
            Some(Value::Series(_)) => {
                Value::Series(out.into_iter().map(|d| Value::Map(Map { grid: l.grid.clone(), data: d })).collect())
            }
            _ => Value::Map(Map { grid: l.grid.clone(), data: out.remove(0) }),
        };
        Ok(outputs([("map", map), ("table", Value::Table(table))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Key;

    fn grid(w: usize, h: usize) -> Arc<Grid> {
        Arc::new(Grid::new(w, h))
    }

    fn map(w: usize, h: usize, v: &[f64]) -> Value {
        Value::Map(Map::new(grid(w, h), v.to_vec()))
    }

    fn stack(w: usize, h: usize, layers: &[(&str, &[f64])]) -> Value {
        Value::Stack(Stack {
            grid: grid(w, h),
            keys: layers.iter().map(|(k, _)| Key::Name(k.to_string())).collect(),
            layers: layers.iter().map(|(_, v)| Arc::new(v.to_vec())).collect(),
        })
    }

    fn run(op: &dyn Operator, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
        let params = params.as_object().cloned().unwrap_or_default();
        let args =
            Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
        op.run(&mut Ctx::default(), &args).unwrap()
    }

    fn table(o: &Outputs, port: &str) -> Table {
        match &o[port] {
            Value::Table(t) => t.clone(),
            _ => panic!("{port} is no table"),
        }
    }

    fn data(o: &Outputs, port: &str) -> Vec<f64> {
        match &o[port] {
            Value::Map(m) => m.data.to_vec(),
            _ => panic!("{port} is no map"),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a.is_nan() && b.is_nan()) || (a - b).abs() < 1e-9
    }

    const N: f64 = f64::NAN;

    #[test]
    fn a_map_is_summarised() {
        let o = run(
            &Stats,
            vec![("map", map(3, 2, &[1.0, 2.0, 3.0, 4.0, N, 10.0]))],
            serde_json::json!({ "stats": ["count", "nodata", "sum", "mean", "std", "median", "percentile_25",
                                          "majority", "unique"], "bins": 3 }),
        );
        let t = table(&o, "table");
        assert_eq!(t.columns[0], "layer");
        // 1 2 3 4 10: mean 4, population variance (9 + 4 + 1 + 0 + 36) / 5 = 10
        let want = [1.0, 5.0, 1.0, 20.0, 4.0, 10f64.sqrt(), 3.0, 2.0, 1.0, 5.0];
        assert!(t.rows[0].iter().zip(want).all(|(a, b)| close(*a, b)), "{:?}", t.rows[0]);
        let h = table(&o, "histogram");
        // bins of 3 from 1 to 10: [1, 4) [4, 7) [7, 10]
        let counts: Vec<f64> = h.rows.iter().map(|r| r[4]).collect();
        assert_eq!(counts, [3.0, 1.0, 1.0]);
        assert!(close(h.rows[1][2], 4.0) && close(h.rows[2][3], 10.0));
        assert!(close(h.rows[0][5], 0.6));
    }

    #[test]
    fn a_stack_is_reduced_at_every_cell() {
        let s = stack(2, 1, &[("a", &[1.0, N]), ("b", &[5.0, 2.0]), ("c", &[3.0, 2.0])]);
        let at =
            |stat: &str| data(&run(&Reduce, vec![("maps", s.clone())], serde_json::json!({ "stat": stat })), "map");
        assert_eq!(at("mean"), [3.0, 2.0]);
        assert_eq!(at("max"), [5.0, 2.0]);
        assert_eq!(at("count"), [3.0, 2.0]);
        assert_eq!(at("median"), [3.0, 2.0]);
        assert_eq!(at("argmax"), [2.0, 2.0]);
        assert_eq!(at("argmin"), [1.0, 2.0]);
        assert_eq!(at("majority"), [1.0, 2.0]);
        assert_eq!(at("range"), [4.0, 0.0]);
        let series = Value::Series(vec![map(1, 1, &[2.0]), map(1, 1, &[4.0])]);
        let o = run(&Reduce, vec![("maps", series)], serde_json::json!({ "stat": "sum" }));
        assert_eq!(data(&o, "map"), [6.0]);
    }

    #[test]
    fn zones_of_a_map_are_summarised() {
        let values = map(3, 2, &[1.0, 2.0, 3.0, 4.0, 5.0, N]);
        let zones = map(3, 2, &[1.0, 1.0, 2.0, 2.0, N, 2.0]);
        let o = run(
            &Zonal,
            vec![("map", values), ("zones", zones)],
            serde_json::json!({ "stats": ["count", "nodata", "mean", "max"], "to_map": "sum" }),
        );
        let t = table(&o, "table");
        assert_eq!(t.columns, ["zone", "count", "nodata", "mean", "max"]);
        assert_eq!(t.rows, [vec![1.0, 2.0, 0.0, 1.5, 2.0], vec![2.0, 2.0, 1.0, 3.5, 4.0]]);
        let m = data(&o, "map");
        assert_eq!(&m[..4], [3.0, 3.0, 7.0, 7.0]);
        assert!(m[4].is_nan());
        assert_eq!(m[5], 7.0);
        let two = stack(2, 1, &[("a", &[1.0, 3.0]), ("b", &[2.0, 4.0])]);
        let o =
            run(&Zonal, vec![("map", two), ("zones", map(2, 1, &[7.0, 7.0]))], serde_json::json!({ "stats": "mean" }));
        let t = table(&o, "table");
        assert_eq!(t.columns, ["zone", "a_mean", "b_mean"]);
        assert_eq!(t.rows, [vec![7.0, 2.0, 3.0]]);
    }

    #[test]
    fn classes_are_tabulated_by_zone() {
        let zones = map(4, 1, &[1.0, 1.0, 1.0, 2.0]);
        let classes = map(4, 1, &[10.0, 20.0, 10.0, N]);
        let o = run(&Tabulate, vec![("zones", zones.clone()), ("classes", classes.clone())], serde_json::json!({}));
        let t = table(&o, "table");
        assert_eq!(t.columns, ["zone", "class_10", "class_20", "total"]);
        assert_eq!(t.rows, [vec![1.0, 2.0, 1.0, 3.0], vec![2.0, 0.0, 0.0, 0.0]]);
        let o = run(&Tabulate, vec![("zones", zones), ("classes", classes)], serde_json::json!({ "units": "share" }));
        let t = table(&o, "table");
        assert!(close(t.rows[0][1], 2.0 / 3.0) && close(t.rows[0][3], 1.0) && t.rows[1][1].is_nan());
    }

    #[test]
    fn maps_are_correlated() {
        let s = stack(
            5,
            1,
            &[("a", &[1.0, 2.0, 3.0, 4.0, 5.0]), ("b", &[2.0, 4.0, 6.0, 8.0, N]), ("c", &[5.0, 4.0, 3.0, 2.0, 1.0])],
        );
        let o = run(&Correlation, vec![("x", s)], serde_json::json!({}));
        let m = table(&o, "matrix");
        assert_eq!(m.columns, ["layer", "a", "b", "c"]);
        assert!(close(m.rows[0][2], 1.0) && close(m.rows[0][3], -1.0) && close(m.rows[1][1], 1.0));
        assert!(close(m.rows[0][1], 1.0));
        let c = table(&o, "covariance");
        // var(a) = 2.5 (n - 1); cov(a, b) over 4 cells: 2 var(1..4) = 2 * 5/3
        assert!(close(c.rows[0][1], 2.5) && close(c.rows[0][2], 10.0 / 3.0));
        let p = table(&o, "pairs");
        assert_eq!(p.rows[0][..3], [1.0, 2.0, 4.0]);
        assert!(close(p.rows[0][5], 2.0) && close(p.rows[0][6], 0.0) && close(p.rows[0][7], 1.0));
        // Spearman's of a monotone curve: 1
        let o = run(
            &Correlation,
            vec![("x", map(4, 1, &[1.0, 2.0, 3.0, 4.0])), ("y", map(4, 1, &[1.0, 8.0, 27.0, 64.0]))],
            serde_json::json!({ "method": "spearman" }),
        );
        let m = table(&o, "matrix");
        assert_eq!(m.columns, ["layer", "x", "y"]);
        assert!(close(m.rows[0][2], 1.0));
        assert!(close(table(&o, "pairs").rows[0][3], 1.0) && table(&o, "pairs").rows[0][7] < 1.0);
    }

    #[test]
    fn maps_are_correlated_in_a_window() {
        // y = 2x + 1 everywhere: the line found in every window
        let x: Vec<f64> = (0..25).map(|i| ((i * 7) % 11) as f64).collect();
        let y: Vec<f64> = x.iter().map(|v| 2.0 * v + 1.0).collect();
        let o = run(
            &LocalCorrelation,
            vec![("x", map(5, 5, &x)), ("y", map(5, 5, &y))],
            serde_json::json!({ "radius": 1 }),
        );
        assert!(data(&o, "correlation").iter().all(|r| close(*r, 1.0)));
        assert!(data(&o, "slope").iter().all(|s| close(*s, 2.0)));
        assert!(data(&o, "intercept").iter().all(|s| close(*s, 1.0)));
        // a corner of a circle of radius 1: 3 cells
        let o = run(
            &LocalCorrelation,
            vec![("x", map(5, 5, &x)), ("y", map(5, 5, &y))],
            serde_json::json!({ "radius": 1, "shape": "circle", "min_cells": 4 }),
        );
        let r = data(&o, "r2");
        assert!(r[0].is_nan() && close(r[6], 1.0));
    }

    #[test]
    fn maps_are_standardized() {
        let o = run(&Standardize, vec![("map", map(4, 1, &[1.0, 2.0, 3.0, N]))], serde_json::json!({}));
        let z = data(&o, "map");
        let sd = (2.0f64 / 3.0).sqrt();
        assert!(close(z[0], -1.0 / sd) && close(z[1], 0.0) && z[3].is_nan());
        let s = stack(3, 1, &[("a", &[2.0, 4.0, 6.0]), ("b", &[5.0, 5.0, 5.0])]);
        let o = run(&Standardize, vec![("map", s)], serde_json::json!({ "method": "minmax" }));
        let Value::Stack(st) = &o["map"] else { panic!("no stack") };
        assert_eq!(*st.layers[0], [0.0, 0.5, 1.0]);
        assert_eq!(*st.layers[1], [0.0, 0.0, 0.0]);
        assert_eq!(table(&o, "table").rows[0], [1.0, 2.0, 4.0]);
        let v: Vec<f64> = (0..=100).map(f64::from).collect();
        let o = run(
            &Standardize,
            vec![("map", map(101, 1, &v))],
            serde_json::json!({ "method": "percentile", "low": 10, "high": 90 }),
        );
        let p = data(&o, "map");
        assert!(close(p[0], 0.0) && close(p[50], 0.5) && close(p[100], 1.0));
    }
}
