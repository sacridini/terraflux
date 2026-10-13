//! Landscape metrics of sample plots (landscapemetrics' sample_lsm and scale_sample), square zones tiling a map
//! (landschaft's grid_zones), series of maps (pylandstats' SpatioTemporalAnalysis) and the correlation of
//! metrics (landscapemetrics' calculate_correlation, R's cor), by landschaft's C++.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::Type;
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

use crate::change::numpy_sum;
use crate::metrics::{Row, chosen, find, measure, metric_options, options, settings};
use crate::value::{Key, Stack};
use crate::vector::{on_grid, vector_of};
use crate::{
    Access, Args, Ctx, Error, Map, OpSignature, Operator, Outputs, PortSpec, Registry, Result, Table, Value, outputs,
};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Sample));
    r.add(Arc::new(GridZones));
    r.add(Arc::new(Series));
    r.add(Arc::new(Signatures));
    r.add(Arc::new(Correlation));
}

/// R's seq(start, stop, length.out = n).
fn r_seq(start: f64, stop: f64, n: usize) -> Vec<f64> {
    let by = (stop - start) / (n - 1) as f64;
    let mut v = vec![start];
    v.extend((1..n - 1).map(|k| start + k as f64 * by));
    v.push(stop);
    v
}

/// A sample plot: its polygons (rings closed) and whether it is a square buffer.
#[derive(Clone)]
struct Plot {
    polygons: Vec<Vec<Vec<Xy>>>,
    rectangle: bool,
}

impl Plot {
    fn rings(&self) -> impl Iterator<Item = (bool, &Vec<Xy>)> {
        self.polygons.iter().flat_map(|p| p.iter().enumerate().map(|(k, r)| (k == 0, r)))
    }

    fn geometry(&self) -> Geometry {
        if self.polygons.len() == 1 {
            Geometry::Polygon(self.polygons[0].clone())
        } else {
            Geometry::MultiPolygon(self.polygons.clone())
        }
    }

    /// An axis-aligned rectangle (one ring of five points, two xs and two ys, a coordinate changed per side).
    fn is_rectangle(&self) -> bool {
        if self.polygons.len() != 1 || self.polygons[0].len() != 1 {
            return false;
        }
        let r = &self.polygons[0][0];
        if r.len() != 5 || r[0] != r[4] {
            return false;
        }
        let distinct = |f: &dyn Fn(&Xy) -> f64| {
            let mut v: Vec<f64> = r.iter().map(f).collect();
            v.sort_by(f64::total_cmp);
            v.dedup();
            v.len()
        };
        distinct(&|p| p[0]) == 2
            && distinct(&|p| p[1]) == 2
            && r.windows(2).all(|w| ((w[0][0] != w[1][0]) as u8 + (w[0][1] != w[1][1]) as u8) == 1)
    }
}

/// landscapemetrics' construct_buffer: a square (half side `size`) or a 100-vertex circle (radius `size`) around
/// a point, its sines and cosines R's (x87) bits.
fn buffer(p: Xy, size: f64, circle: Option<&(Vec<f64>, Vec<f64>)>) -> Plot {
    let (x, y) = (p[0], p[1]);
    match circle {
        None => Plot {
            polygons: vec![vec![vec![
                [x - size, y - size],
                [x - size, y + size],
                [x + size, y + size],
                [x + size, y - size],
                [x - size, y - size],
            ]]],
            rectangle: true,
        },
        Some((sin, cos)) => {
            let mut ring: Vec<Xy> = sin.iter().zip(cos).map(|(s, c)| [s * size + x, c * size + y]).collect();
            ring.push(ring[0]);
            Plot { polygons: vec![vec![ring]], rectangle: false }
        }
    }
}

/// The plots of features: buffers of `size` around points, polygons as they are.
fn plots_of(v: &Vector, size: Option<f64>, circle: Option<&(Vec<f64>, Vec<f64>)>) -> Result<Vec<Plot>> {
    let mut out = Vec::new();
    for g in &v.geometries {
        let points: Vec<Xy> = match g {
            Geometry::Point(p) => vec![*p],
            Geometry::MultiPoint(ps) => ps.clone(),
            Geometry::Polygon(p) => {
                out.push(Plot { polygons: vec![p.clone()], rectangle: false });
                continue;
            }
            Geometry::MultiPolygon(p) => {
                out.push(Plot { polygons: p.clone(), rectangle: false });
                continue;
            }
            Geometry::Empty => continue,
            _ => return Err(Error::msg("sample plots are points or polygons")),
        };
        let size = size.filter(|s| *s > 0.0).ok_or_else(|| Error::msg("points need a buffer size > 0 (size)"))?;
        out.extend(points.into_iter().map(|p| buffer(p, size, circle)));
    }
    Ok(out)
}

/// A plot's rows and columns of a grid, and the cells of that window it takes.
type Window = (std::ops::Range<usize>, std::ops::Range<usize>, Vec<u8>);

/// A plot's window of the map and the cells in it, as terra::crop(snap = "near", mask = TRUE, touches = TRUE):
/// the rows, the columns and the cells the plot touches (GDAL's all-touched rasterization); None outside.
fn crop(plot: &Plot, grid: &Grid) -> Result<Option<Window>> {
    let t = grid.transform;
    let (rx, ry) = (t[1], -t[5]);
    let (nrow, ncol) = (grid.height as i64, grid.width as i64);
    let (x0, ytop) = (t[0], t[3]);
    let y0 = ytop - nrow as f64 * ry;
    let (mut xmin, mut ymin, mut xmax, mut ymax) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (_, r) in plot.rings() {
        for p in r {
            xmin = xmin.min(p[0]);
            xmax = xmax.max(p[0]);
            ymin = ymin.min(p[1]);
            ymax = ymax.max(p[1]);
        }
    }
    let near = |v: f64| (v + 0.5).floor() as i64;
    let (mut c0, mut c1) = (near((xmin - x0) / rx), near((xmax - x0) / rx));
    let (mut b0, mut b1) = (near((ymin - y0) / ry), near((ymax - y0) / ry));
    let collapsed = c0 == c1 || b0 == b1;
    if c0 == c1 {
        c0 -= 1;
    }
    if b0 == b1 {
        b0 -= 1;
    }
    (c0, c1, b0, b1) = (c0.max(0), c1.min(ncol), b0.max(0), b1.min(nrow));
    if c1 <= c0 || b1 <= b0 {
        return Ok(None);
    }
    let (wy, wx) = ((b1 - b0) as usize, (c1 - c0) as usize);
    let inside = if !collapsed && (plot.rectangle || plot.is_rectangle()) {
        vec![1u8; wy * wx]
    } else {
        let rings: Vec<(Vec<f64>, Vec<f64>)> =
            plot.rings().map(|(_, r)| (r.iter().map(|p| p[0]).collect(), r.iter().map(|p| p[1]).collect())).collect();
        let (xs, ys): (Vec<_>, Vec<_>) = rings.into_iter().unzip();
        let sizes: Vec<i64> = xs.iter().map(|v: &Vec<f64>| v.len() as i64).collect();
        let gt = [x0 + c0 as f64 * rx, rx, 0.0, y0 + b1 as f64 * ry, 0.0, -ry];
        ffi::lsm_rasterize(&sizes, &xs.concat(), &ys.concat(), &gt, wx as i32, wy as i32)?
    };
    Ok(Some(((nrow - b1) as usize..(nrow - b0) as usize, c0 as usize..c1 as usize, inside)))
}

/// A plot's planar area: numpy's shoelace over its rings, the holes taken off.
fn planar_area(plot: &Plot) -> f64 {
    let mut total = 0.0;
    for (exterior, r) in plot.rings() {
        let n = r.len();
        let terms: Vec<f64> = (0..n).map(|i| r[i][0] * r[(i + 1) % n][1] - r[(i + 1) % n][0] * r[i][1]).collect();
        let a = numpy_sum(&terms).abs() / 2.0;
        total += if exterior { a } else { -a };
    }
    total
}

/// The metrics measured on every plot: its rows, and the % of its area that has cells of the map.
struct Sampled {
    rows: Vec<Row>,
    percentage_inside: f64,
}

fn sample_map(
    map: &Map,
    plots: &[Plot],
    areas: &[f64],
    opt: &ffi::LsmOptions,
    specs: &[(&'static str, &str)],
) -> Result<Vec<Sampled>> {
    let grid = &map.grid;
    let (nx, t) = (grid.width, grid.transform);
    let (rx, ry) = (t[1].abs(), t[5].abs());
    let mut out = Vec::with_capacity(plots.len());
    for (plot, &area) in plots.iter().zip(areas) {
        let Some((rows, cols, inside)) = crop(plot, grid)? else {
            let mut g = (**grid).clone();
            (g.width, g.height) = (1, 1);
            let empty = Map::new(Arc::new(g), vec![f64::NAN]);
            out.push(Sampled { rows: measure(&empty, opt, specs, false)?.rows, percentage_inside: 0.0 });
            continue;
        };
        let wx = cols.len();
        let mut data = Vec::with_capacity(rows.len() * wx);
        for (i, r) in rows.clone().enumerate() {
            for (j, c) in cols.clone().enumerate() {
                data.push(if inside[i * wx + j] != 0 { map.data[r * nx + c] } else { f64::NAN });
            }
        }
        let cells = data.iter().filter(|v| !v.is_nan()).count();
        let mut g = (**grid).clone();
        g.transform[0] = t[0] + cols.start as f64 * t[1];
        g.transform[3] = t[3] + rows.start as f64 * t[5];
        (g.width, g.height) = (wx, rows.len());
        let sub = Map::new(Arc::new(g), data);
        let inside_area = cells as f64 * rx * ry / 10000.0;
        let pct = if inside_area > 0.0 { inside_area / (area / 10000.0) * 100.0 } else { 0.0 };
        out.push(Sampled { rows: measure(&sub, opt, specs, false)?.rows, percentage_inside: pct });
    }
    Ok(out)
}

/// The level of a node's metrics (landscape by default).
fn level_of(args: &Args, levels: &[&'static str]) -> Result<&'static str> {
    let level: String = args.param("level", "landscape".to_string())?;
    levels
        .iter()
        .copied()
        .find(|l| *l == level)
        .ok_or_else(|| Error::msg(format!("node {:?}: level is {}, not {level:?}", args.node, levels.join(", "))))
}

/// The sizes of the buffers: one (`size` a number) or several (a list); none without `size`.
fn sizes_of(args: &Args) -> Result<(Vec<Option<f64>>, bool)> {
    match args.params.get("size") {
        None | Some(serde_json::Value::Null) => Ok((vec![None], false)),
        Some(serde_json::Value::Number(n)) => Ok((vec![n.as_f64()], false)),
        Some(v) => {
            let s: Vec<f64> = serde_json::from_value(v.clone())
                .map_err(|e| Error::msg(format!("node {:?}: size: {e} (a number or a list)", args.node)))?;
            if s.is_empty() || s.iter().any(|x| x.is_nan() || *x <= 0.0) {
                return Err(Error::msg(format!("node {:?}: size: buffer sizes > 0", args.node)));
            }
            Ok((s.into_iter().map(Some).collect(), true))
        }
    }
}

/// The circle's sines and cosines (R's bits) for circular buffers, none for squares.
fn circle_of(args: &Args) -> Result<Option<(Vec<f64>, Vec<f64>)>> {
    let shape: String = args.param("shape", "square".to_string())?;
    match shape.as_str() {
        "square" => Ok(None),
        "circle" => {
            let sc = ffi::lsm_r_sincos(&r_seq(0.0, 2.0 * std::f64::consts::PI, 100));
            let (s, c) = sc.split_at(100);
            Ok(Some((s.to_vec(), c.to_vec())))
        }
        s => Err(Error::msg(format!("node {:?}: shape is square or circle, not {s:?}", args.node))),
    }
}

/// The plots' areas: geodesic (WGS84) when their coordinate system is known, as landscapemetrics', else planar.
fn areas_of(plots: &[Plot], crs: &Option<String>) -> Result<Vec<f64>> {
    match crs {
        Some(_) => {
            let v = Vector {
                geometries: plots.iter().map(Plot::geometry).collect(),
                attributes: Vec::new(),
                crs: crs.clone(),
            };
            terraflux_gdal::geodesic_areas(&v).map_err(Error::msg)
        }
        None => Ok(plots.iter().map(planar_area).collect()),
    }
}

/// A class (NA last) and a patch, to order rows.
type RowKey = (bool, i32, i32);

/// A row of a level: its class, its patch and the metrics.
type LevelRow = (Option<i32>, Option<i32>, Vec<f64>);

/// A plot's rows of a level: per class or patch (or the one of the landscape) its class, patch and metrics (NaN
/// where none); `classes` rows besides those measured.
fn level_rows(level: &str, rows: &[Row], names: &[&str], classes: &[Option<i32>]) -> Vec<LevelRow> {
    let column = |m: &str| names.iter().position(|n| *n == m);
    let key = |c: Option<i32>, id: Option<i32>| -> RowKey { (c.is_none(), c.unwrap_or(0), id.unwrap_or(0)) };
    let mut out: BTreeMap<RowKey, LevelRow> = BTreeMap::new();
    if level == "landscape" {
        out.insert(key(None, None), (None, None, vec![f64::NAN; names.len()]));
    }
    for &c in classes {
        out.insert(key(c, None), (c, None, vec![f64::NAN; names.len()]));
    }
    for r in rows.iter().filter(|r| r.level == level) {
        let Some(j) = column(&r.metric) else { continue };
        let (c, id) = match level {
            "landscape" => (None, None),
            "class" => (r.class, None),
            _ => (r.class, r.id),
        };
        out.entry(key(c, id)).or_insert_with(|| (c, id, vec![f64::NAN; names.len()])).2[j] = r.value;
    }
    out.into_values().collect()
}

struct Sample;

impl Operator for Sample {
    fn signature(&self) -> OpSignature {
        settings(
            OpSignature::new(
                "metrics.sample",
                "Landscape metrics of sample plots, as landscapemetrics' sample_lsm and scale_sample: squares or \
                 circles (100 vertices) of a size around points, or polygons as they are. Each plot is cut from \
                 the map as terra's crop(mask = TRUE): the window snapped to the nearest cells' edges, the cells \
                 the plot touches kept, the others NA; its metrics those of a landscape of its own. Several sizes \
                 give a row per size. The % of each plot's area that has cells of the map (its area geodesic \
                 when the plots' coordinate system is known, as landscapemetrics).",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map of classes"))
            .input(PortSpec::new("plots", Type::Vector, "points (buffers around them) or polygons"))
            .output(PortSpec::new(
                "table",
                Type::Table,
                "per plot (and size, class or patch): plot_id, size, class, patch, percentage_inside, the metrics",
            ))
            .output(PortSpec::new("plots", Type::Vector, "the plots measured, with plot_id (and size)")),
        )
        .param(PortSpec::optional("level", Type::Text, "landscape (default), class or patch").choices([
            "landscape",
            "class",
            "patch",
        ]))
        .param(PortSpec::optional("size", Type::Any, "points: the buffer's half side or radius, or a list of them"))
        .param(
            PortSpec::optional("shape", Type::Text, "points: square (default) or circle buffers")
                .choices(["square", "circle"]),
        )
        .param(PortSpec::optional("id_column", Type::Text, "the plots' identifiers (default 1, 2...)"))
        .param(PortSpec::optional(
            "all_classes",
            Type::Bool,
            "class level: a row (NA) for every class of the map that a plot lacks, and for NA when the map has it",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let v = on_grid(vector_of(args, "plots")?, &map.grid)?;
        let level = level_of(args, &["landscape", "class", "patch"])?;
        let metrics = chosen(args, level)?;
        let names: Vec<&str> = metrics.iter().map(|m| m.metric).collect();
        let specs: Vec<(&'static str, &str)> = metrics.iter().map(|m| (level, m.metric)).collect();
        let opt = options(args)?;
        let (sizes, several) = sizes_of(args)?;
        let circle = circle_of(args)?;
        let all_classes: bool = args.param("all_classes", false)?;
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        // the map's classes, for all_classes
        let mut classes: Vec<Option<i32>> = Vec::new();
        if all_classes && level == "class" {
            let mut c: Vec<i32> = map.data.iter().filter(|v| !v.is_nan()).map(|v| v.trunc() as i32).collect();
            c.sort_unstable();
            c.dedup();
            classes = c.into_iter().map(Some).collect();
            if map.data.iter().any(|v| v.is_nan()) {
                classes.push(None);
            }
        }
        let mut columns = vec!["plot_id".to_string()];
        if several {
            columns.push("size".into());
        }
        if level != "landscape" {
            columns.push("class".into());
        }
        if level == "patch" {
            columns.push("patch".into());
        }
        columns.push("percentage_inside".into());
        columns.extend(names.iter().map(|n| n.to_string()));
        // sample_lsm's order: plot, class (NA last), patch, size
        type Entry = ((usize, RowKey, usize), Vec<f64>);
        let mut entries: Vec<Entry> = Vec::new();
        let mut geometries = Vec::new();
        let (mut plot_ids, mut plot_sizes) = (Vec::new(), Vec::new());
        let id_column: Option<String> = args.opt_param("id_column")?;
        for (s, &size) in sizes.iter().enumerate() {
            let plots = plots_of(&v, size, circle.as_ref()).map_err(fail)?;
            let ids: Vec<f64> = match &id_column {
                Some(c) => {
                    let ids = match v.column(c) {
                        Some(Column::Numbers(x)) => x.clone(),
                        _ => {
                            return Err(Error::msg(format!(
                                "node {:?}: id_column: the plots have no numbers {c:?}",
                                args.node
                            )));
                        }
                    };
                    // points of multipoints numbered 1, 2... as landscapemetrics
                    if ids.len() == plots.len() { ids } else { (1..=plots.len()).map(|i| i as f64).collect() }
                }
                None => (1..=plots.len()).map(|i| i as f64).collect(),
            };
            let mut order: Vec<usize> = (0..plots.len()).collect();
            order.sort_by(|&a, &b| ids[a].total_cmp(&ids[b]));
            let mut rank = vec![0; plots.len()];
            for (r, &k) in order.iter().enumerate() {
                rank[k] = r;
            }
            let areas = areas_of(&plots, &v.crs).map_err(fail)?;
            let sampled = sample_map(map, &plots, &areas, &opt, &specs).map_err(fail)?;
            for (k, sm) in sampled.iter().enumerate() {
                for (c, id, values) in level_rows(level, &sm.rows, &names, &classes) {
                    let mut row = vec![ids[k]];
                    if several {
                        row.push(size.unwrap_or(f64::NAN));
                    }
                    if level != "landscape" {
                        row.push(c.map_or(f64::NAN, f64::from));
                    }
                    if level == "patch" {
                        row.push(id.map_or(f64::NAN, f64::from));
                    }
                    row.push(sm.percentage_inside);
                    row.extend(values);
                    entries.push(((rank[k], (c.is_none(), c.unwrap_or(0), id.unwrap_or(0)), s), row));
                }
            }
            for (k, p) in plots.iter().enumerate() {
                geometries.push(p.geometry());
                plot_ids.push(ids[k]);
                plot_sizes.push(size.unwrap_or(f64::NAN));
            }
        }
        entries.sort_by_key(|e| e.0);
        let table = Table { columns, rows: entries.into_iter().map(|(_, r)| r).collect() };
        let mut attributes = vec![("plot_id".to_string(), Column::Numbers(plot_ids))];
        if several {
            attributes.push(("size".into(), Column::Numbers(plot_sizes)));
        }
        let plots = Vector { geometries, attributes, crs: v.crs.clone() };
        Ok(outputs([("table", Value::Table(table)), ("plots", Value::Vector(Arc::new(plots)))]))
    }
}

/// Squares of a side tiling a grid from its top-left corner (partial squares at the right and bottom), row by
/// row, as landschaft's grid_zones.
fn grid_zones(grid: &Grid, size: f64) -> Vec<Plot> {
    let t = grid.transform;
    let (xmin, ymax) = (t[0], t[3]);
    let (xmax, ymin) = (xmin + grid.width as f64 * t[1], ymax + grid.height as f64 * t[5]);
    let mut zones = Vec::new();
    let mut y1 = ymax;
    while y1 > ymin + 1e-9 * size {
        let y0 = ymin.max(y1 - size);
        let mut x0 = xmin;
        while x0 < xmax - 1e-9 * size {
            let x1 = xmax.min(x0 + size);
            let ring = vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]];
            zones.push(Plot { polygons: vec![vec![ring]], rectangle: false });
            x0 = x1;
        }
        y1 = y0;
    }
    zones
}

/// A positive side, if given.
fn side(args: &Args, name: &str) -> Result<Option<f64>> {
    match args.opt_param::<f64>(name)? {
        Some(s) if s.is_nan() || s <= 0.0 => Err(Error::msg(format!("node {:?}: {name}: a side > 0", args.node))),
        s => Ok(s),
    }
}

struct GridZones;

impl Operator for GridZones {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.grid_zones",
            "Square zones tiling a map, as landschaft's grid_zones: squares of a side (map units) from the map's \
             top-left corner, partial squares at its right and bottom, row by row; for metrics.sample and \
             metrics.series by zones.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the map tiled"))
        .output(PortSpec::new("zones", Type::Vector, "the squares, numbered (zone)"))
        .param(PortSpec::new("size", Type::Number, "the squares' side, in map units"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let size =
            side(args, "size")?.ok_or_else(|| Error::msg(format!("node {:?}: size: the squares' side", args.node)))?;
        let zones = grid_zones(&map.grid, size);
        let v = Vector {
            geometries: zones.iter().map(Plot::geometry).collect(),
            attributes: vec![("zone".into(), Column::Numbers((1..=zones.len()).map(|i| i as f64).collect()))],
            crs: crate::prepare::crs_of(&map.grid),
        };
        Ok(outputs([("zones", Value::Vector(Arc::new(v)))]))
    }
}

/// The zones of a node over a grid and their areas: polygons or buffers around points (`plots`), or squares
/// tiling it (`grid`); none for whole maps.
fn zones_of(args: &Args, grid: &Arc<Grid>) -> Result<Option<(Vec<Plot>, Vec<f64>)>> {
    let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
    match (args.get("plots"), side(args, "grid")?) {
        (Some(_), Some(_)) => Err(Error::msg(format!("node {:?}: zones (plots) or a grid, not both", args.node))),
        (Some(_), None) => {
            let v = on_grid(vector_of(args, "plots")?, grid)?;
            let p = plots_of(&v, args.opt_param("size")?, circle_of(args)?.as_ref()).map_err(fail)?;
            let areas = areas_of(&p, &v.crs).map_err(fail)?;
            Ok(Some((p, areas)))
        }
        (None, Some(s)) => {
            let p = grid_zones(grid, s);
            let areas = p.iter().map(planar_area).collect();
            Ok(Some((p, areas)))
        }
        (None, None) => Ok(None),
    }
}

/// The rows measured of every zone (0 for whole maps) of every layer: (zone, layer, rows).
fn measure_layers(
    stack: &Stack,
    plots: Option<&(Vec<Plot>, Vec<f64>)>,
    opt: &ffi::LsmOptions,
    specs: &[(&'static str, &str)],
) -> Result<Vec<(usize, usize, Vec<Row>)>> {
    let mut measured = Vec::new();
    for (d, layer) in stack.layers.iter().enumerate() {
        let map = Map::new(stack.grid.clone(), layer.as_ref().clone());
        match plots {
            Some((p, areas)) => {
                for (z, s) in sample_map(&map, p, areas, opt, specs)?.into_iter().enumerate() {
                    measured.push((z, d, s.rows));
                }
            }
            None => measured.push((0, d, measure(&map, opt, specs, false)?.rows)),
        }
    }
    Ok(measured)
}

/// Class metrics that are 0 (not NA) for a class absent from a landscape (pylandstats).
const ZERO_WHEN_ABSENT: [&str; 11] = ["ca", "pland", "np", "pd", "lpi", "te", "ed", "tca", "cpland", "ndca", "dcad"];

struct Series;

impl Operator for Series {
    fn signature(&self) -> OpSignature {
        settings(
            OpSignature::new(
                "metrics.series",
                "Landscape metrics along a series of maps (a layer per date), as pylandstats' \
                 SpatioTemporalAnalysis in landschaft: of the whole maps, of zones (polygons), of buffers around \
                 points or of square zones tiling the maps (grid). Landscape level: a row per (zone and) date; \
                 class level: a row per class, (zone and) date, every class at every date, 0 instead of NA for \
                 the area, edge and count metrics of a class absent (fillna).",
                Access::Global,
            )
            .input(PortSpec::new("maps", Type::Stack, "the maps of classes, a layer per date"))
            .input(PortSpec::optional("plots", Type::Vector, "zones (polygons) or points (buffers around them)"))
            .output(PortSpec::new("table", Type::Table, "per (class,) (zone and) date: the metrics")),
        )
        .param(PortSpec::optional("level", Type::Text, "landscape (default) or class").choices(["landscape", "class"]))
        .param(PortSpec::optional("dates", Type::Any, "the layers' dates (default: their keys, or 1, 2...)"))
        .param(PortSpec::optional("grid", Type::Number, "zones: squares of this side tiling the maps"))
        .param(PortSpec::optional("size", Type::Number, "points: the buffer's half side or radius"))
        .param(
            PortSpec::optional("shape", Type::Text, "points: square (default) or circle buffers")
                .choices(["square", "circle"]),
        )
        .param(PortSpec::optional(
            "fillna",
            Type::Bool,
            "class level: 0 for absent classes' counts (default true)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let stack = args.stack("maps")?;
        let level = level_of(args, &["landscape", "class"])?;
        let metrics = chosen(args, level)?;
        let names: Vec<&str> = metrics.iter().map(|m| m.metric).collect();
        let specs: Vec<(&'static str, &str)> = metrics.iter().map(|m| (level, m.metric)).collect();
        let opt = options(args)?;
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let n = stack.layers.len();
        let dates: Vec<f64> = match args.opt_param::<Vec<f64>>("dates")? {
            Some(d) if d.len() == n => d,
            Some(d) => return Err(Error::msg(format!("node {:?}: dates: {} for {n} layers", args.node, d.len()))),
            None => stack
                .keys
                .iter()
                .enumerate()
                .map(|(i, k)| match k {
                    Key::Class(c) => *c,
                    Key::Name(s) => s.parse().unwrap_or((i + 1) as f64),
                    _ => (i + 1) as f64,
                })
                .collect(),
        };
        let plots = zones_of(args, &stack.grid)?;
        let zonal = plots.is_some();
        let mut measured = measure_layers(stack, plots.as_ref(), &opt, &specs).map_err(fail)?;
        let mut columns = Vec::new();
        if level == "class" {
            columns.push("class".to_string());
        }
        if zonal {
            columns.push("zone".into());
        }
        columns.push("date".into());
        columns.extend(names.iter().map(|n| n.to_string()));
        let mut rows = Vec::new();
        let row_of = |c: Option<i32>, z: usize, d: usize, values: Vec<f64>| {
            let mut row: Vec<f64> = c.map(f64::from).into_iter().collect();
            if zonal {
                row.push((z + 1) as f64);
            }
            row.push(dates[d]);
            row.extend(values);
            row
        };
        if level == "landscape" {
            measured.sort_by_key(|(z, d, _)| (*z, *d));
            for (z, d, r) in &measured {
                let (_, _, values) = level_rows(level, r, &names, &[]).remove(0);
                rows.push(row_of(None, *z, *d, values));
            }
        } else {
            // every class at every zone and date
            let mut cells: BTreeMap<(i32, usize, usize), Vec<f64>> = BTreeMap::new();
            for (z, d, r) in &measured {
                for (c, _, values) in level_rows(level, r, &names, &[]) {
                    if let Some(c) = c {
                        cells.insert((c, *z, *d), values);
                    }
                }
            }
            let mut classes: Vec<i32> = cells.keys().map(|k| k.0).collect();
            classes.dedup();
            let mut zones: Vec<usize> = cells.keys().map(|k| k.1).collect();
            zones.sort_unstable();
            zones.dedup();
            let fillna: bool = args.param("fillna", true)?;
            for &c in &classes {
                for &z in &zones {
                    for d in 0..n {
                        let mut values = cells.get(&(c, z, d)).cloned().unwrap_or_else(|| vec![f64::NAN; names.len()]);
                        if fillna {
                            for (v, m) in values.iter_mut().zip(&names) {
                                if v.is_nan() && ZERO_WHEN_ABSENT.contains(m) {
                                    *v = 0.0;
                                }
                            }
                        }
                        rows.push(row_of(Some(c), z, d, values));
                    }
                }
            }
        }
        Ok(outputs([("table", Value::Table(Table { columns, rows }))]))
    }
}

/// The metrics of a level named in a setting (a name or a list; none when not given).
fn metric_list(args: &Args, name: &str, level: &str) -> Result<Vec<&'static str>> {
    let names: Vec<String> = match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::String(s)) if s.trim().is_empty() => Vec::new(),
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| Error::msg(format!("node {:?}: {name}: {e} (a name or a list)", args.node)))?,
    };
    names
        .iter()
        .map(|n| {
            find(level, n)
                .map(|m| m.metric)
                .ok_or_else(|| Error::msg(format!("node {:?}: {name}: {n:?} is no {level} metric", args.node)))
        })
        .collect()
}

/// A zone's (or layer's) landscape metrics and its classes' metrics.
type ZoneRow = (Vec<f64>, BTreeMap<i32, Vec<f64>>);

struct Signatures;

impl Operator for Signatures {
    fn signature(&self) -> OpSignature {
        metric_options(
            OpSignature::new(
                "metrics.signatures",
                "Spatial signatures, as pylandstats' SpatialSignatureAnalysis in landschaft: landscapes described                  by their metrics, a row per zone of a map (polygons, buffers around points or a grid of squares)                  or per layer of a stack, a column per landscape metric and per class metric and class                  (pland_11...), in alphabetical order. For metrics.pca, metrics.nmf, metrics.cluster and                  metrics.clustergram.",
                Access::Global,
            )
            .input(PortSpec::new("maps", Type::Any, "a map of classes (its zones) or a stack (a row per layer)"))
            .input(PortSpec::optional("plots", Type::Vector, "zones (polygons) or points (buffers around them)"))
            .output(PortSpec::new("table", Type::Table, "per zone (or landscape): the metrics")),
        )
        .param(PortSpec::optional("landscape_metrics", Type::Any, "landscape metrics (a name or a list: shdi, ed...)"))
        .param(PortSpec::optional("class_metrics", Type::Any, "class metrics, a column per class (pland...)"))
        .param(PortSpec::optional("classes", Type::Any, "the classes of the class metrics (default all)"))
        .param(PortSpec::optional("grid", Type::Number, "zones: squares of this side tiling the map"))
        .param(PortSpec::optional("size", Type::Number, "points: the buffer's half side or radius"))
        .param(
            PortSpec::optional("shape", Type::Text, "points: square (default) or circle buffers")
                .choices(["square", "circle"]),
        )
        .param(PortSpec::optional("fillna", Type::Bool, "0 for absent classes' counts and areas (default true)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let stack = match args.get("maps") {
            Some(Value::Stack(s)) => s.clone(),
            _ => {
                let m = args.map("maps")?;
                Stack { grid: m.grid.clone(), keys: vec![Key::Name("1".into())], layers: vec![m.data.clone()] }
            }
        };
        let mut land = metric_list(args, "landscape_metrics", "landscape")?;
        let mut class = metric_list(args, "class_metrics", "class")?;
        if land.is_empty() && class.is_empty() {
            return Err(Error::msg(format!("node {:?}: choose landscape_metrics and/or class_metrics", args.node)));
        }
        // the columns in alphabetical order, as pandas' pivot leaves them
        land.sort_unstable();
        land.dedup();
        class.sort_unstable();
        class.dedup();
        let wanted: Option<Vec<i32>> =
            args.opt_param::<Vec<f64>>("classes")?.map(|c| c.into_iter().map(|v| v as i32).collect::<Vec<_>>());
        let fillna: bool = args.param("fillna", true)?;
        let plots = zones_of(args, &stack.grid)?;
        let zonal = plots.is_some();
        if zonal && stack.layers.len() > 1 {
            return Err(Error::msg(format!("node {:?}: zones of one map, not of a stack", args.node)));
        }
        let specs: Vec<(&'static str, &str)> =
            land.iter().map(|m| ("landscape", *m)).chain(class.iter().map(|m| ("class", *m))).collect();
        let measured = measure_layers(&stack, plots.as_ref(), &options(args)?, &specs).map_err(fail)?;
        // a row per zone or layer: its landscape metrics and those of its classes
        let mut rows: BTreeMap<usize, ZoneRow> = BTreeMap::new();
        for (z, d, r) in &measured {
            let values = if land.is_empty() { Vec::new() } else { level_rows("landscape", r, &land, &[]).remove(0).2 };
            let mut by_class = BTreeMap::new();
            if !class.is_empty() {
                for (c, _, v) in level_rows("class", r, &class, &[]) {
                    if let Some(c) = c {
                        by_class.insert(c, v);
                    }
                }
            }
            rows.insert(if zonal { *z } else { *d }, (values, by_class));
        }
        let classes: Vec<i32> = match &wanted {
            Some(w) => {
                let mut w = w.clone();
                w.sort_unstable();
                w.dedup();
                w
            }
            None => {
                let mut c: Vec<i32> = rows.values().flat_map(|(_, b)| b.keys().copied()).collect();
                c.sort_unstable();
                c.dedup();
                c
            }
        };
        let mut columns = vec![if zonal { "zone" } else { "landscape" }.to_string()];
        columns.extend(land.iter().map(|m| m.to_string()));
        for m in &class {
            columns.extend(classes.iter().map(|c| format!("{m}_{c}")));
        }
        let mut table = Table::new(columns);
        for (i, (values, by_class)) in rows {
            // a zone without classes has no class metrics in landschaft's table
            if land.is_empty() && by_class.is_empty() {
                continue;
            }
            let mut row = vec![(i + 1) as f64];
            row.extend(values);
            for (j, m) in class.iter().enumerate() {
                for c in &classes {
                    let v = by_class.get(c).map_or(f64::NAN, |v| v[j]);
                    row.push(if v.is_nan() && fillna && ZERO_WHEN_ABSENT.contains(m) { 0.0 } else { v });
                }
            }
            table.rows.push(row);
        }
        Ok(outputs([("table", Value::Table(table))]))
    }
}

/// The columns of a table that are keys, not metrics.
pub const KEYS: [&str; 11] =
    ["class", "patch", "id", "plot_id", "size", "zone", "date", "layer", "percentage_inside", "landscape", "cluster"];

struct Correlation;

impl Operator for Correlation {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "metrics.correlation",
            "The correlation between the metrics of a table (its columns, over its rows), as landscapemetrics' \
             calculate_correlation: R's cor, Pearson's, Spearman's or Kendall's, the same values. A metric with \
             NA values has NA correlations (but its own, 1). Rows without a class are left out.",
            Access::Global,
        )
        .input(PortSpec::new("table", Type::Table, "the metrics, e.g. of metrics.patch or metrics.class"))
        .output(PortSpec::new("table", Type::Table, "the correlation matrix: a row and a column per metric"))
        .param(
            PortSpec::optional("method", Type::Text, "pearson (default), spearman or kendall")
                .choices(["pearson", "spearman", "kendall"]),
        )
        .param(PortSpec::optional(
            "columns",
            Type::Any,
            "the metrics (default every column but the keys: class, patch, plot_id, size, zone, date...)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let Some(Value::Table(t)) = args.get("table") else {
            return Err(Error::msg(format!("node {:?}: connect a table", args.node)));
        };
        let method: String = args.param("method", "pearson".to_string())?;
        if !["pearson", "spearman", "kendall"].contains(&method.as_str()) {
            return Err(Error::msg(format!("node {:?}: method is pearson, spearman or kendall", args.node)));
        }
        let columns: Vec<usize> = match args.opt_param::<Vec<String>>("columns")? {
            Some(names) => names
                .iter()
                .map(|n| {
                    t.columns
                        .iter()
                        .position(|c| c == n)
                        .ok_or_else(|| Error::msg(format!("node {:?}: columns: the table has no {n:?}", args.node)))
                })
                .collect::<Result<_>>()?,
            None => (0..t.columns.len()).filter(|&j| !KEYS.contains(&t.columns[j].as_str())).collect(),
        };
        if columns.len() < 2 {
            return Err(Error::msg(format!("node {:?}: more than one metric to correlate", args.node)));
        }
        let class = t.columns.iter().position(|c| c == "class");
        let rows: Vec<&Vec<f64>> = t.rows.iter().filter(|r| class.is_none_or(|j| !r[j].is_nan())).collect();
        let (n, m) = (rows.len(), columns.len());
        let x: Vec<f64> = columns.iter().flat_map(|&j| rows.iter().map(move |r| r[j])).collect();
        let r = ffi::lsm_r_cor(&x, n as i64, m as i64, &method)
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let out = Table {
            columns: columns.iter().map(|&j| t.columns[j].clone()).collect(),
            rows: (0..m).map(|i| r[i * m..(i + 1) * m].to_vec()).collect(),
        };
        Ok(outputs([("table", Value::Table(out))]))
    }
}
