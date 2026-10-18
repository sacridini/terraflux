//! Neighbourhoods of rasters and the cleaning of maps of classes: a statistic in a moving window of any shape
//! (`raster.focal`), the majority filter (`raster.majority`), small patches merged into their neighbours
//! (`raster.sieve`, as gdal_sieve), the patches numbered (`raster.label`, as r.clump), and classes grown or shrunk
//! cell by cell (`raster.expand`, `raster.shrink`, as ArcGIS's Expand and Shrink).
//!
//! Cells beyond the map's edges have no value; cells without a value are left out of every window.

use std::collections::HashMap;
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::raster_stats::statistic;
use crate::vector::stat_code;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Focal));
    r.add(Arc::new(Majority));
    r.add(Arc::new(Sieve));
    r.add(Arc::new(Label));
    r.add(Arc::new(Expand));
    r.add(Arc::new(Shrink));
}

/// The neighbours of a cell (row and column offsets) by connectivity, 4 (rook) or 8 (queen).
fn neighbours(args: &Args, default: usize) -> Result<&'static [(isize, isize)]> {
    const FOUR: [(isize, isize); 4] = [(-1, 0), (0, -1), (0, 1), (1, 0)];
    const EIGHT: [(isize, isize); 8] = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)];
    match args.param("neighbours", default)? {
        4 => Ok(&FOUR),
        8 => Ok(&EIGHT),
        n => Err(Error::msg(format!("node {:?}: neighbours {n}: 4 or 8", args.node))),
    }
}

/// The cell at an offset from another, when it is on the map.
fn at(w: usize, h: usize, cell: usize, (dr, dc): (isize, isize)) -> Option<usize> {
    let (r, c) = ((cell / w) as isize + dr, (cell % w) as isize + dc);
    (r >= 0 && c >= 0 && (r as usize) < h && (c as usize) < w).then(|| r as usize * w + c as usize)
}

/// The most common value of values with one (a tie: none, or the smallest when `smallest`), and its count.
fn most_common(values: &[f64], smallest: bool) -> Option<(f64, usize)> {
    let mut counts: Vec<(f64, usize)> = Vec::new();
    for &v in values {
        match counts.iter_mut().find(|(x, _)| *x == v) {
            Some((_, c)) => *c += 1,
            None => counts.push((v, 1)),
        }
    }
    let top = counts.iter().map(|x| x.1).max()?;
    let mut tied: Vec<f64> = counts.iter().filter(|x| x.1 == top).map(|x| x.0).collect();
    if tied.len() > 1 && !smallest {
        return None;
    }
    tied.sort_by(f64::total_cmp);
    Some((tied[0], top))
}

/// `raster.focal`.
struct Focal;

impl Operator for Focal {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.focal",
            "A statistic of the cells in a moving window around every cell, as terra's focal or GRASS's \
             r.neighbors: the mean, median, majority (the mode), standard deviation, variety (unique), range, \
             minimum, maximum, sum, count, a percentile... (the statistics of vector.zonal) over the window's cells \
             with a value; the window a square, a circle, an annulus (a ring between two radii) or a Gaussian \
             kernel (the mean or sum weighted by exp(-d² / 2σ²)). No value where the cell has none, unless \
             `fill`.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Map, "the map"))
        .output(PortSpec::new("map", Type::Map, "the statistic of every cell's window"))
        .param(PortSpec::optional(
            "stat",
            Type::Text,
            "mean (default), median, majority, minority, std, var, unique (variety), range, min, max, sum, \
             count, nodata, percentile_<p>",
        ))
        .param(
            PortSpec::optional("shape", Type::Text, "square (default), circle, annulus or gaussian")
                .choices(["square", "circle", "annulus", "gaussian"]),
        )
        .param(PortSpec::optional("radius", Type::Integer, "the window's radius in cells (1: 3 x 3)"))
        .param(PortSpec::optional("inner", Type::Integer, "annulus: the inner radius, its cells left out (radius / 2)"))
        .param(PortSpec::optional("sigma", Type::Number, "gaussian: the standard deviation in cells (radius / 2)"))
        .param(PortSpec::optional("fill", Type::Bool, "a value also where the cell has none (false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let m = args.map("map")?;
        let stat: String = args.param("stat", "mean".to_string())?;
        let stat = match stat.as_str() {
            "mode" => "majority".to_string(),
            "variety" => "unique".to_string(),
            "sd" => "std".to_string(),
            _ => stat,
        };
        let code = stat_code(&stat).ok_or_else(|| Error::msg(format!("node {node:?}: unknown statistic {stat:?}")))?;
        let radius: usize = args.param("radius", 1)?;
        let r = radius as isize;
        let shape: String = args.param("shape", "square".to_string())?;
        let inner: f64 = args.param("inner", (radius / 2) as f64)?;
        let sigma: f64 = args.param("sigma", (radius as f64 / 2.0).max(0.5))?;
        let d2 = |dr: isize, dc: isize| (dr * dr + dc * dc) as f64;
        let all = (-r..=r).flat_map(|dr| (-r..=r).map(move |dc| (dr, dc)));
        let window: Vec<((isize, isize), f64)> = match shape.as_str() {
            "square" => all.map(|o| (o, 1.0)).collect(),
            "circle" => all.filter(|&(a, b)| d2(a, b) <= (r * r) as f64).map(|o| (o, 1.0)).collect(),
            "annulus" => {
                all.filter(|&(a, b)| d2(a, b) <= (r * r) as f64 && d2(a, b) > inner * inner).map(|o| (o, 1.0)).collect()
            }
            "gaussian" => {
                if !matches!(stat.as_str(), "mean" | "sum") {
                    return Err(Error::msg(format!(
                        "node {node:?}: a Gaussian window weighs the cells: its statistic is mean or sum"
                    )));
                }
                if sigma <= 0.0 {
                    return Err(Error::msg(format!("node {node:?}: sigma must be above 0")));
                }
                all.filter(|&(a, b)| d2(a, b) <= (r * r) as f64)
                    .map(|(a, b)| ((a, b), (-d2(a, b) / (2.0 * sigma * sigma)).exp()))
                    .collect()
            }
            s => return Err(Error::msg(format!("node {node:?}: shape {s:?}: square, circle, annulus or gaussian"))),
        };
        if window.is_empty() {
            return Err(Error::msg(format!("node {node:?}: the window holds no cell")));
        }
        let fill: bool = args.param("fill", false)?;
        let weighted = shape == "gaussian";
        let (w, h) = (m.grid.width, m.grid.height);
        let mut out = vec![f64::NAN; m.data.len()];
        out.par_chunks_mut(w.max(1)).enumerate().for_each(|(row, line)| {
            let mut values = Vec::with_capacity(window.len());
            for (c, o) in line.iter_mut().enumerate() {
                let cell = row * w + c;
                if !fill && m.data[cell].is_nan() {
                    continue;
                }
                values.clear();
                let (mut missing, mut sum, mut weights) = (0usize, 0.0, 0.0);
                for &(off, wt) in &window {
                    let Some(i) = at(w, h, cell, off) else { continue };
                    let v = m.data[i];
                    if v.is_nan() {
                        missing += 1;
                    } else if weighted {
                        sum += wt * v;
                        weights += wt;
                    } else {
                        values.push(v);
                    }
                }
                *o = if weighted {
                    match stat.as_str() {
                        "mean" if weights > 0.0 => sum / weights,
                        "mean" => f64::NAN,
                        _ if weights > 0.0 => sum,
                        _ => f64::NAN,
                    }
                } else {
                    values.sort_unstable_by(f64::total_cmp);
                    statistic(code, &values, missing)
                };
            }
        });
        Ok(outputs([("map", Value::Map(Map::new(m.grid.clone(), out)))]))
    }
}

/// `raster.majority`.
struct Majority;

impl Operator for Majority {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.majority",
            "A map of classes cleaned by the majority filter, as ArcGIS's Majority Filter: a cell takes the most \
             common class of its neighbours (with a value) when more than half of them hold it (`replace` \
             majority) or at least half (half); a tie between classes keeps the cell. Repeated `iterations` times, \
             each on the map the one before left.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Map, "the classes"))
        .output(PortSpec::new("map", Type::Map, "the classes filtered"))
        .param(PortSpec::optional(
            "neighbours",
            Type::Integer,
            "4 or 8 (default): the cells around, or a diamond and a \
                               square of the radius",
        ))
        .param(PortSpec::optional("radius", Type::Integer, "the neighbourhood's radius in cells (1)"))
        .param(
            PortSpec::optional("replace", Type::Text, "majority (more than half, default) or half (at least half)")
                .choices(["majority", "half"]),
        )
        .param(PortSpec::optional("iterations", Type::Integer, "the times the filter is applied (1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let m = args.map("map")?;
        let diamond = neighbours(args, 8)?.len() == 4;
        let r: isize = args.param("radius", 1)?;
        let half = match args.param("replace", "majority".to_string())?.as_str() {
            "majority" => false,
            "half" => true,
            x => return Err(Error::msg(format!("node {node:?}: replace {x:?}: majority or half"))),
        };
        let iterations: usize = args.param("iterations", 1)?;
        let window: Vec<(isize, isize)> = (-r..=r)
            .flat_map(|dr| (-r..=r).map(move |dc| (dr, dc)))
            .filter(|&(a, b)| (a, b) != (0, 0) && (!diamond || a.abs() + b.abs() <= r))
            .collect();
        let (w, h) = (m.grid.width, m.grid.height);
        let mut data = m.data.to_vec();
        for _ in 0..iterations {
            let before = data.clone();
            data.par_chunks_mut(w.max(1)).enumerate().for_each(|(row, line)| {
                let mut values = Vec::with_capacity(window.len());
                for (c, o) in line.iter_mut().enumerate() {
                    if o.is_nan() {
                        continue;
                    }
                    let cell = row * w + c;
                    values.clear();
                    values.extend(
                        window.iter().filter_map(|&off| at(w, h, cell, off)).map(|i| before[i]).filter(|v| !v.is_nan()),
                    );
                    if let Some((v, n)) = most_common(&values, false) {
                        let k = values.len();
                        if (half && 2 * n >= k) || 2 * n > k {
                            *o = v;
                        }
                    }
                }
            });
        }
        Ok(outputs([("map", Value::Map(Map::new(m.grid.clone(), data)))]))
    }
}

/// The patches of a map: the groups of connected cells of one value (cells without one in none), numbered 0.. in
/// the order of their first cell: the patch of every cell, and each patch's value and cells.
fn patches(data: &[f64], w: usize, h: usize, around: &[(isize, isize)]) -> (Vec<Option<usize>>, Vec<f64>, Vec<usize>) {
    let mut id: Vec<Option<usize>> = vec![None; data.len()];
    let (mut values, mut sizes) = (Vec::new(), Vec::new());
    let mut stack = Vec::new();
    for start in 0..data.len() {
        if id[start].is_some() || data[start].is_nan() {
            continue;
        }
        let p = values.len();
        let v = data[start];
        id[start] = Some(p);
        stack.push(start);
        let mut n = 0;
        while let Some(cell) = stack.pop() {
            n += 1;
            for &off in around {
                if let Some(i) = at(w, h, cell, off)
                    && id[i].is_none()
                    && data[i] == v
                {
                    id[i] = Some(p);
                    stack.push(i);
                }
            }
        }
        values.push(v);
        sizes.push(n);
    }
    (id, values, sizes)
}

/// `raster.sieve`.
struct Sieve;

impl Operator for Sieve {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.sieve",
            "The patches smaller than a size merged into their largest neighbour, as gdal_sieve (GDALSieveFilter): \
             a map of classes rid of its specks, the smallest patches first, each taking the value of the largest \
             patch it touches (a patch grown by the merge counts as its new size). A patch touching none (all \
             around without a value) stays. The cells without a value stay so.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Map, "the classes"))
        .output(PortSpec::new("map", Type::Map, "the classes sieved"))
        .param(PortSpec::new("threshold", Type::Integer, "the fewest cells of a patch kept"))
        .param(PortSpec::optional(
            "neighbours",
            Type::Integer,
            "4 (default) or 8: the cells of a patch connected",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        let threshold: usize = args.param("threshold", 0)?;
        let around = neighbours(args, 4)?;
        let (w, h) = (m.grid.width, m.grid.height);
        let (id, values, mut sizes) = patches(&m.data, w, h, around);
        let k = values.len();
        // the patches each touches
        let mut touching: Vec<Vec<usize>> = vec![Vec::new(); k];
        for cell in 0..m.data.len() {
            let Some(a) = id[cell] else { continue };
            for &off in around {
                if let Some(b) = at(w, h, cell, off).and_then(|i| id[i])
                    && a != b
                    && !touching[a].contains(&b)
                {
                    touching[a].push(b);
                }
            }
        }
        // merged patches, by union-find
        let mut parent: Vec<usize> = (0..k).collect();
        fn root(parent: &mut [usize], mut p: usize) -> usize {
            while parent[p] != p {
                parent[p] = parent[parent[p]];
                p = parent[p];
            }
            p
        }
        let mut order: Vec<usize> = (0..k).filter(|&p| sizes[p] < threshold).collect();
        order.sort_by_key(|&p| (sizes[p], p));
        for p in order {
            let a = root(&mut parent, p);
            if sizes[a] >= threshold {
                continue;
            }
            // the largest patch it touches (a tie: the first found), the patches merged into it its own
            let list = std::mem::take(&mut touching[a]);
            let mut best: Option<usize> = None;
            for &n in &list {
                let b = root(&mut parent, n);
                if b != a && best.is_none_or(|x| sizes[b] > sizes[x]) {
                    best = Some(b);
                }
            }
            match best {
                Some(b) => {
                    parent[a] = b;
                    sizes[b] += sizes[a];
                    touching[b].extend(list);
                }
                None => touching[a] = list,
            }
        }
        let out: Vec<f64> = id
            .iter()
            .map(|p| match p {
                Some(p) => values[root(&mut parent, *p)],
                None => f64::NAN,
            })
            .collect();
        Ok(outputs([("map", Value::Map(Map::new(m.grid.clone(), out)))]))
    }
}

/// `raster.label`.
struct Label;

impl Operator for Label {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.label",
            "The patches of a map numbered, as GRASS's r.clump or scipy's label: every group of connected cells of \
             one value given an id (1.., in the order of their first cell, row by row), with a table of each \
             patch's value, cells and area. Only the classes given, when given; the cells without a value in none.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Map, "the classes"))
        .output(PortSpec::new("map", Type::Map, "the id of every cell's patch"))
        .output(PortSpec::new("table", Type::Table, "a row per patch: id, value, cells, area (map units squared)"))
        .param(PortSpec::optional("neighbours", Type::Integer, "4 (default) or 8"))
        .param(PortSpec::optional("classes", Type::Any, "the classes labelled (default every one)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        let around = neighbours(args, 4)?;
        let classes = match args.params.get("classes") {
            None | Some(serde_json::Value::Null) => None,
            Some(_) => Some(classes_of(args)?),
        };
        let data: Vec<f64> = match &classes {
            Some(c) => m.data.iter().map(|v| if c.contains(v) { *v } else { f64::NAN }).collect(),
            None => m.data.to_vec(),
        };
        let (id, values, sizes) = patches(&data, m.grid.width, m.grid.height, around);
        let out: Vec<f64> = id.iter().map(|p| p.map_or(f64::NAN, |p| (p + 1) as f64)).collect();
        let (rw, rh) = m.grid.resolution();
        let mut t = Table::new(["id", "value", "cells", "area"].map(String::from).to_vec());
        t.rows = values
            .iter()
            .zip(&sizes)
            .enumerate()
            .map(|(p, (v, n))| vec![(p + 1) as f64, *v, *n as f64, *n as f64 * rw * rh])
            .collect();
        Ok(outputs([("map", Value::Map(Map::new(m.grid.clone(), out))), ("table", Value::Table(t))]))
    }
}

/// The classes of a node (`classes`: one or a list), or why there are none.
fn classes_of(args: &Args) -> Result<Vec<f64>> {
    let c: Vec<f64> = match args.params.get("classes") {
        Some(serde_json::Value::Number(n)) => n.as_f64().into_iter().collect(),
        _ => args.opt_param("classes")?.unwrap_or_default(),
    };
    if c.is_empty() {
        return Err(Error::msg(format!("node {:?}: choose the classes", args.node)));
    }
    Ok(c)
}

/// The classes of a map grown into the cells around them, or shrunk away from them, a cell a step.
fn grow(args: &Args, expand: bool) -> Result<Outputs> {
    let m = args.map("map")?;
    let classes = classes_of(args)?;
    let around = neighbours(args, 8)?;
    let steps: usize = args.param("cells", 1)?;
    let into_missing: bool = args.param("into_missing", false)?;
    let (w, h) = (m.grid.width, m.grid.height);
    let mut data = m.data.to_vec();
    let mut changes: HashMap<usize, f64> = HashMap::new();
    for _ in 0..steps {
        changes.clear();
        let mut values = Vec::with_capacity(around.len());
        for (cell, &v) in data.iter().enumerate() {
            let inside = classes.contains(&v);
            // growing: a cell outside the classes beside one; shrinking: a cell of them beside another value
            let candidate = if expand { !inside && (into_missing || !v.is_nan()) } else { inside };
            if !candidate {
                continue;
            }
            values.clear();
            values.extend(
                around
                    .iter()
                    .filter_map(|&off| at(w, h, cell, off))
                    .map(|i| data[i])
                    .filter(|x| !x.is_nan() && classes.contains(x) == expand),
            );
            if let Some((x, _)) = most_common(&values, true) {
                changes.insert(cell, x);
            }
        }
        if changes.is_empty() {
            break;
        }
        for (&cell, &x) in &changes {
            data[cell] = x;
        }
    }
    Ok(outputs([("map", Value::Map(Map::new(m.grid.clone(), data)))]))
}

/// `raster.expand`.
struct Expand;

impl Operator for Expand {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.expand",
            "Classes of a map grown a number of cells into the others, as ArcGIS's Expand: a cell a step, every \
             cell beside one of the classes taking the most common of them around it (a tie: the smallest). The \
             cells without a value stay so, unless `into_missing`.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Map, "the classes"))
        .output(PortSpec::new("map", Type::Map, "the classes grown"))
        .param(PortSpec::new("classes", Type::Any, "the classes grown (one or a list)"))
        .param(PortSpec::optional("cells", Type::Integer, "how many cells (1)"))
        .param(PortSpec::optional("neighbours", Type::Integer, "4 or 8 (default)"))
        .param(PortSpec::optional("into_missing", Type::Bool, "into the cells without a value too (false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        grow(args, true)
    }
}

/// `raster.shrink`.
struct Shrink;

impl Operator for Shrink {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.shrink",
            "Classes of a map shrunk a number of cells, as ArcGIS's Shrink: a cell a step, every cell of the classes \
             beside another value taking the most common of those around it (a tie: the smallest). A cell beside \
             none but cells without a value stays.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Map, "the classes"))
        .output(PortSpec::new("map", Type::Map, "the classes shrunk"))
        .param(PortSpec::new("classes", Type::Any, "the classes shrunk (one or a list)"))
        .param(PortSpec::optional("cells", Type::Integer, "how many cells (1)"))
        .param(PortSpec::optional("neighbours", Type::Integer, "4 or 8 (default)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        grow(args, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_raster::Grid;

    fn map(w: usize, h: usize, v: &[f64]) -> Value {
        Value::Map(Map::new(Arc::new(Grid::new(w, h)), v.to_vec()))
    }

    fn run(op: &dyn Operator, m: Value, params: serde_json::Value) -> Outputs {
        let params = params.as_object().cloned().unwrap_or_default();
        let args = Args { node: "n", inputs: [("map".to_string(), m)].into(), params: &params };
        op.run(&mut Ctx::default(), &args).unwrap()
    }

    fn data(o: &Outputs) -> Vec<f64> {
        match &o["map"] {
            Value::Map(m) => m.data.to_vec(),
            _ => panic!("no map"),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a.is_nan() && b.is_nan()) || (a - b).abs() < 1e-9
    }

    const N: f64 = f64::NAN;

    #[test]
    fn a_window_is_summarised() {
        #[rustfmt::skip]
        let v = [
            1.0, 2.0, 3.0,
            4.0, 5.0, 6.0,
            7.0, 8.0, N,
        ];
        let m = map(3, 3, &v);
        let at = |p: serde_json::Value| data(&run(&Focal, m.clone(), p));
        let mean = at(serde_json::json!({}));
        assert!(close(mean[0], 3.0) && close(mean[4], 36.0 / 8.0) && mean[8].is_nan());
        let filled = at(serde_json::json!({ "fill": true }));
        assert!(close(filled[8], 19.0 / 3.0));
        assert_eq!(at(serde_json::json!({ "stat": "max" }))[4], 8.0);
        assert_eq!(at(serde_json::json!({ "stat": "variety" }))[0], 4.0);
        assert_eq!(at(serde_json::json!({ "stat": "nodata" }))[4], 1.0);
        // a circle of radius 1: the cross
        assert_eq!(at(serde_json::json!({ "stat": "sum", "shape": "circle" }))[4], 25.0);
        // an annulus of radii 1 and 0: the cross without the centre
        assert_eq!(at(serde_json::json!({ "stat": "sum", "shape": "annulus", "inner": 0 }))[4], 20.0);
        // a Gaussian mean of a constant map is the constant
        let flat = data(&run(&Focal, map(5, 5, &[2.0; 25]), serde_json::json!({ "shape": "gaussian", "radius": 2 })));
        assert!(flat.iter().all(|x| close(*x, 2.0)));
    }

    #[test]
    fn specks_are_filtered_and_sieved() {
        #[rustfmt::skip]
        let v = [
            1.0, 1.0, 1.0, 1.0,
            1.0, 2.0, 1.0, 1.0,
            1.0, 1.0, 3.0, 3.0,
            1.0, 1.0, 3.0, 3.0,
        ];
        let m = map(4, 4, &v);
        let f = data(&run(&Majority, m.clone(), serde_json::json!({})));
        assert_eq!(f[5], 1.0);
        assert_eq!(f[10], 3.0, "3 of 3 beside a corner cell's own class... {f:?}");
        let s = data(&run(&Sieve, m.clone(), serde_json::json!({ "threshold": 2 })));
        assert_eq!(s[5], 1.0);
        assert_eq!(s[15], 3.0);
        let s = data(&run(&Sieve, m, serde_json::json!({ "threshold": 5 })));
        assert!(s.iter().all(|x| *x == 1.0), "{s:?}");
    }

    #[test]
    fn patches_are_numbered() {
        #[rustfmt::skip]
        let v = [
            1.0, 1.0, 2.0,
            2.0, N, 2.0,
            2.0, 1.0, 1.0,
        ];
        let o = run(&Label, map(3, 3, &v), serde_json::json!({}));
        let ids = data(&o);
        assert_eq!(ids[0], 1.0);
        assert_eq!(ids[1], 1.0);
        assert_eq!(ids[2], 2.0);
        assert_eq!(ids[5], 2.0);
        assert_eq!(ids[3], 3.0);
        assert_eq!(ids[7], 4.0);
        let Value::Table(t) = &o["table"] else { panic!("no table") };
        assert_eq!(t.rows[1], [2.0, 2.0, 2.0, 2.0]);
        let o = run(&Label, map(3, 3, &v), serde_json::json!({ "neighbours": 8, "classes": [2] }));
        let ids = data(&o);
        assert!(ids[0].is_nan());
        assert_eq!(ids[2], 1.0, "{ids:?}");
        assert_eq!(ids[3], 2.0);
    }

    #[test]
    fn classes_grow_and_shrink() {
        let v = [1.0, 2.0, 2.0, 2.0, N];
        let e = data(&run(&Expand, map(5, 1, &v), serde_json::json!({ "classes": 1, "cells": 2 })));
        assert!(e[..3].iter().all(|x| *x == 1.0) && e[3] == 2.0 && e[4].is_nan());
        let s = data(&run(&Shrink, map(5, 1, &v), serde_json::json!({ "classes": [2] })));
        assert_eq!(s[..4], [1.0, 1.0, 2.0, 2.0]);
        let e = data(&run(&Expand, map(5, 1, &v), serde_json::json!({ "classes": 2, "into_missing": true })));
        assert_eq!(e[4], 2.0);
    }
}
