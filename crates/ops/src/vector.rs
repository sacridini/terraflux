//! Vector features: read and written by OGR (GDAL), burnt into maps, their zones' statistics and the exact sums of
//! maps over polygons, and distances to them. Polygons are burnt as GDAL's rasterizer does through landschaft's
//! C++ port of it (`lsio::polygon_runs`, the same cells), lines by GDAL, points by their cell, as landschaft's
//! rasterize; the exact sums are exactextract's (landschaft's port of it, exactextractr's bits).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Read));
    r.add(Arc::new(Write));
    r.add(Arc::new(Rasterize));
    r.add(Arc::new(Zonal));
    r.add(Arc::new(ExactSum));
    r.add(Arc::new(Distance));
}

/// A vector input of a node.
pub fn vector_of<'a>(args: &'a Args, port: &str) -> Result<&'a Arc<Vector>> {
    match args.get(port) {
        Some(Value::Vector(v)) => Ok(v),
        Some(v) => Err(Error::msg(format!("node {:?}: {port} is a {:?}, not vectors", args.node, v.ty()))),
        None => Err(Error::msg(format!("node {:?}: connect the features ({port})", args.node))),
    }
}

/// The features' attributes as a table: the feature (1..) and the attributes that are numbers.
pub fn attribute_table(v: &Vector) -> Table {
    let numeric: Vec<(&String, &Vec<f64>)> =
        v.attributes.iter().filter_map(|(n, c)| if let Column::Numbers(x) = c { Some((n, x)) } else { None }).collect();
    let mut t =
        Table::new(std::iter::once("feature".to_string()).chain(numeric.iter().map(|(n, _)| (*n).clone())).collect());
    t.rows = (0..v.len())
        .map(|i| std::iter::once((i + 1) as f64).chain(numeric.iter().map(|(_, x)| x[i])).collect())
        .collect();
    t
}

/// The features in the map's coordinate system (reprojected by GDAL when both are known and differ).
pub fn on_grid(v: &Arc<Vector>, grid: &Grid) -> Result<Arc<Vector>> {
    let (Some(from), Some(to)) = (&v.crs, crate::prepare::crs_of(grid)) else { return Ok(v.clone()) };
    if terraflux_gdal::same_crs(from, &to).unwrap_or(true) {
        return Ok(v.clone());
    }
    Ok(Arc::new(terraflux_gdal::reproject_vector(v, &to).map_err(Error::msg)?))
}

/// The polygons of the geometries as GDAL's rasterizer and landschaft's kernels take them: the points (x, y
/// pairs), the rings' offsets, and per geometry its rings (every ring of every polygon), each ring clockwise.
pub fn polygon_arrays(geoms: &[&Geometry]) -> (Vec<f64>, Vec<i64>, Vec<i64>) {
    let (mut xy, mut ring_off, mut geom_rings) = (Vec::new(), vec![0i64], vec![0i64]);
    for g in geoms {
        for poly in g.polygons() {
            for ring in poly {
                // clockwise: the orientation decides the cells whose centres lie on a horizontal edge
                let n = ring.len();
                let area2: f64 =
                    (0..n).map(|k| ring[k][0] * ring[(k + 1) % n][1] - ring[(k + 1) % n][0] * ring[k][1]).sum();
                if area2 > 0.0 {
                    ring.iter().rev().for_each(|p| xy.extend_from_slice(p));
                } else {
                    ring.iter().for_each(|p| xy.extend_from_slice(p));
                }
                ring_off.push((xy.len() / 2) as i64);
            }
        }
        geom_rings.push((ring_off.len() - 1) as i64);
    }
    (xy, ring_off, geom_rings)
}

/// Every geometry (polygons) as its cells on the grid, as GDAL burns them.
pub fn runs(geoms: &[&Geometry], grid: &Grid, all_touched: bool) -> Result<ffi::RunsOut> {
    let (xy, ring_off, geom_rings) = polygon_arrays(geoms);
    if grid.transform[2] != 0.0 || grid.transform[4] != 0.0 {
        return Err(Error::msg("the map's grid is rotated: polygons are burnt on north-up grids"));
    }
    Ok(ffi::io_polygon_runs(
        &xy,
        &ring_off,
        &geom_rings,
        &grid.transform,
        grid.width as i32,
        grid.height as i32,
        all_touched,
    )?)
}

/// Burn the features into `out` (the grid's cells), value k for feature k: polygons first, then lines, then
/// points; within each the later ones win, or the values add up.
pub fn burn(out: &mut [f64], grid: &Grid, v: &Vector, values: &[f64], all_touched: bool, add: bool) -> Result<()> {
    let idx = |f: &dyn Fn(&Geometry) -> bool| -> Vec<usize> { (0..v.len()).filter(|&i| f(&v.geometries[i])).collect() };
    let polys = idx(&|g| g.is_polygonal());
    if !polys.is_empty() {
        let geoms: Vec<&Geometry> = polys.iter().map(|&i| &v.geometries[i]).collect();
        let r = runs(&geoms, grid, all_touched)?;
        let vals: Vec<f64> = polys.iter().map(|&i| values[i]).collect();
        ffi::io_burn_runs(out, grid.height as i32, grid.width as i32, &r, &vals, add)?;
    }
    let lines = idx(&|g| matches!(g, Geometry::LineString(_) | Geometry::MultiLineString(_)));
    if !lines.is_empty() {
        let geoms: Vec<Geometry> = lines.iter().map(|&i| v.geometries[i].clone()).collect();
        let vals: Vec<f64> = lines.iter().map(|&i| values[i]).collect();
        terraflux_gdal::rasterize(out, grid.width, grid.height, grid.transform, &geoms, &vals, all_touched, add)
            .map_err(|e| Error::msg(format!("lines are burnt by GDAL: {e}")))?;
    }
    let t = &grid.transform;
    for i in idx(&|g| g.is_point()) {
        let mut cells = Vec::new();
        v.geometries[i].points(&mut |p| {
            let col = ((p[0] - t[0]) / t[1]).floor();
            let row = ((p[1] - t[3]) / t[5]).floor();
            if col >= 0.0 && row >= 0.0 && (col as usize) < grid.width && (row as usize) < grid.height {
                cells.push(row as usize * grid.width + col as usize);
            }
        });
        for c in cells {
            if add { out[c] += values[i] } else { out[c] = values[i] }
        }
    }
    Ok(())
}

/// The value burnt for every feature: an attribute (missing: not burnt) or one value.
fn burn_values(args: &Args, v: &Vector) -> Result<(Vec<f64>, Vec<usize>)> {
    match args.opt_param::<String>("field")? {
        Some(f) if !f.is_empty() => {
            let c = v.column(&f).ok_or_else(|| {
                let names: Vec<&str> = v.attributes.iter().map(|(n, _)| n.as_str()).collect();
                Error::msg(format!("node {:?}: no attribute {f:?} ({})", args.node, names.join(", ")))
            })?;
            let x = c.numbers();
            let keep = (0..v.len()).filter(|&i| x[i].is_finite()).collect();
            Ok((x, keep))
        }
        _ => {
            let value: f64 = args.param("value", 1.0)?;
            Ok((vec![value; v.len()], (0..v.len()).collect()))
        }
    }
}

/// Layers on one grid: the grid, their names, their values.
type Layers = (Arc<Grid>, Vec<String>, Vec<Arc<Vec<f64>>>);

/// A map (or a stack) input as its layers on one grid.
fn layers_of(args: &Args, port: &str) -> Result<Layers> {
    match args.get(port) {
        Some(Value::Stack(s)) => Ok((s.grid.clone(), s.keys.iter().map(|k| k.to_string()).collect(), s.layers.clone())),
        _ => {
            let m = args.map(port)?;
            Ok((m.grid.clone(), vec![port.to_string()], vec![m.data.clone()]))
        }
    }
}

struct Read;

impl Operator for Read {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.read",
            "Vector features from a file GDAL reads (GeoPackage, Shapefile, GeoJSON, FlatGeobuf, KML...): their \
             geometries, attributes and coordinate system.",
            Access::Global,
        )
        .output(PortSpec::new("vector", Type::Vector, "the features"))
        .output(PortSpec::new("table", Type::Table, "the feature (1..) and its attributes that are numbers"))
        .param(PortSpec::new("path", Type::Text, "the file, relative to the model's folder"))
        .param(PortSpec::optional("layer", Type::Text, "the layer (default the first)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let path: String = args.param("path", String::new())?;
        if path.is_empty() {
            return Err(Error::msg(format!("node {:?}: choose the file", args.node)));
        }
        let layer: Option<String> = args.opt_param("layer")?;
        let v = terraflux_gdal::read_vector(&path, layer.as_deref().filter(|l| !l.is_empty())).map_err(Error::msg)?;
        let t = attribute_table(&v);
        Ok(outputs([("vector", Value::Vector(Arc::new(v))), ("table", Value::Table(t))]))
    }
}

struct Write;

impl Operator for Write {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.write",
            "Write vector features to a file by GDAL, its format by the extension (gpkg by default; shp, geojson, \
             fgb, kml, gml).",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .output(PortSpec::new("path", Type::Text, "the file written"))
        .param(PortSpec::new("path", Type::Text, "the file, relative to the model's folder"))
        .param(PortSpec::optional("layer", Type::Text, "the layer's name (default the file's)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = vector_of(args, "vector")?;
        let path: String = args.param("path", String::new())?;
        if path.is_empty() {
            return Err(Error::msg(format!("node {:?}: choose the file", args.node)));
        }
        let mut path = std::path::PathBuf::from(path);
        if path.extension().is_none() {
            path.set_extension("gpkg");
        }
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| Error::msg(format!("{}: {e}", dir.display())))?;
        }
        let layer: String = args.param(
            "layer",
            path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "features".into()),
        )?;
        terraflux_gdal::write_vector(&path, &layer, v).map_err(Error::msg)?;
        Ok(outputs([("path", Value::Text(path.to_string_lossy().into_owned()))]))
    }
}

struct Rasterize;

impl Operator for Rasterize {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.rasterize",
            "Burn vector features into the grid of a map, as GDAL's rasterizer (landschaft's rasterize): the \
             cells whose centre a polygon takes (or every cell it touches), the cells of lines and of points; an \
             attribute's value or one value; the later features over the earlier, or added up (counting \
             overlaps). The features reprojected to the map's coordinate system first.",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .input(PortSpec::new("like", Type::Map, "a map on the grid to burn into"))
        .output(PortSpec::new("map", Type::Map, "the features burnt"))
        .param(PortSpec::optional("field", Type::Text, "the attribute burnt (features without it are left out)"))
        .param(PortSpec::optional("value", Type::Number, "the value burnt without a field (1)"))
        .param(PortSpec::optional("fill", Type::Number, "the cells no feature takes (0; null: no value)"))
        .param(PortSpec::optional("all_touched", Type::Bool, "every cell a feature touches (false)"))
        .param(PortSpec::optional("merge", Type::Text, "replace (default: the last feature wins) or add"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let like = args.map("like")?;
        let v = on_grid(vector_of(args, "vector")?, &like.grid)?;
        let (values, keep) = burn_values(args, &v)?;
        let fill = match args.params.get("fill") {
            Some(serde_json::Value::Null) => f64::NAN,
            _ => args.param("fill", 0.0)?,
        };
        let merge: String = args.param("merge", "replace".to_string())?;
        if merge != "replace" && merge != "add" {
            return Err(Error::msg(format!("node {:?}: merge is replace or add", args.node)));
        }
        let kept = v.select(&keep);
        let vals: Vec<f64> = keep.iter().map(|&i| values[i]).collect();
        let mut out = vec![fill; like.grid.width * like.grid.height];
        burn(&mut out, &like.grid, &kept, &vals, args.param("all_touched", false)?, merge == "add")?;
        Ok(outputs([("map", Value::Map(Map::new(like.grid.clone(), out)))]))
    }
}

/// landschaft's statistics of zones by name, and their codes in lsio::zonal_stats.
fn stat_code(s: &str) -> Option<(i32, f64)> {
    Some(match s {
        "count" => (0, 0.0),
        "nodata" => (1, 0.0),
        "sum" => (2, 0.0),
        "mean" => (3, 0.0),
        "min" => (4, 0.0),
        "max" => (5, 0.0),
        "range" => (6, 0.0),
        "std" => (7, 0.0),
        "var" => (8, 0.0),
        "median" => (9, 0.0),
        "majority" => (10, 0.0),
        "minority" => (11, 0.0),
        "unique" => (12, 0.0),
        p => (13, p.strip_prefix("percentile_")?.parse::<f64>().ok().filter(|v| (0.0..=100.0).contains(v))?),
    })
}

struct Zonal;

impl Operator for Zonal {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.zonal",
            "Statistics of a map (or of every layer of a stack) in every polygon, landschaft's zonal_stats: each \
             polygon's cells as GDAL burns them (polygons may overlap), the cells with a value.",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the polygons"))
        .input(PortSpec::new("map", Type::Any, "a map or a stack"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "per feature (1..): every statistic (of every layer: <layer>_<statistic>)",
        ))
        .param(PortSpec::optional(
            "stats",
            Type::Any,
            "count, nodata, sum, mean, min, max, range, std, var, median, majority, minority, unique, \
             percentile_<p> (default count, mean, min, max, std)",
        ))
        .param(PortSpec::optional("all_touched", Type::Bool, "every cell a polygon touches (false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let (grid, names, layers) = layers_of(args, "map")?;
        let v = on_grid(vector_of(args, "vector")?, &grid)?;
        if let Some(i) = v.geometries.iter().position(|g| !g.is_polygonal() && !matches!(g, Geometry::Empty)) {
            return Err(err(format!("feature {}: zones are polygons (buffer points or lines first)", i + 1)));
        }
        let stats: Vec<String> = match args.params.get("stats") {
            None | Some(serde_json::Value::Null) => ["count", "mean", "min", "max", "std"].map(String::from).to_vec(),
            Some(serde_json::Value::String(s)) => vec![s.clone()],
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| err(format!("stats: {e}")))?,
        };
        let codes: Vec<(i32, f64)> = stats
            .iter()
            .map(|s| stat_code(s).ok_or_else(|| err(format!("unknown statistic {s:?}"))))
            .collect::<Result<_>>()?;
        let geoms: Vec<&Geometry> = v.geometries.iter().collect();
        let r = runs(&geoms, &grid, args.param("all_touched", false)?)?;
        let values: Vec<f64> = layers.iter().flat_map(|l| l.iter().copied()).collect();
        let (c, p): (Vec<i32>, Vec<f64>) = codes.iter().copied().unzip();
        let out = ffi::io_zonal_stats(&values, layers.len() as i32, grid.height as i32, grid.width as i32, &r, &c, &p)?;
        let multi = layers.len() > 1;
        let mut cols = vec!["feature".to_string()];
        for b in &names {
            for s in &stats {
                cols.push(if multi { format!("{b}_{s}") } else { s.clone() });
            }
        }
        let mut t = Table::new(cols);
        let per = layers.len() * stats.len();
        t.rows = (0..v.len())
            .map(|z| std::iter::once((z + 1) as f64).chain(out[z * per..(z + 1) * per].iter().copied()).collect())
            .collect();
        Ok(outputs([("table", Value::Table(t))]))
    }
}

/// exactextractr's exact_extract(fun = "sum") of every layer over every polygonal feature: features x layers.
pub fn exact_sums(grid: &Grid, layers: &[Arc<Vec<f64>>], geoms: &[&Geometry]) -> Result<Vec<f64>> {
    let t = &grid.transform;
    if t[2] != 0.0 || t[4] != 0.0 || t[5] >= 0.0 {
        return Err(Error::msg("the map must be north-up"));
    }
    let (dx, dy) = (t[1], -t[5]);
    let (ny, nx) = (grid.height, grid.width);
    let bounds = [t[0], t[3] - ny as f64 * dy, t[0] + nx as f64 * dx, t[3], dx, dy];
    // every polygon of every feature, its rings as they are
    let (mut xy, mut ring_off, mut poly_off, mut feat_off) = (Vec::new(), vec![0i64], vec![0i64], vec![0i64]);
    for g in geoms {
        for poly in g.polygons() {
            for ring in poly {
                ring.iter().for_each(|p| xy.extend_from_slice(p));
                ring_off.push((xy.len() / 2) as i64);
            }
            poly_off.push((ring_off.len() - 1) as i64);
        }
        feat_off.push((poly_off.len() - 1) as i64);
    }
    let values: Vec<f64> = layers.iter().flat_map(|l| l.iter().copied()).collect();
    Ok(ffi::io_exact_sums(
        &values,
        layers.len() as i64,
        ny as i64,
        nx as i64,
        &bounds,
        &xy,
        &ring_off,
        &poly_off,
        &feat_off,
    )?)
}

struct ExactSum;

impl Operator for ExactSum {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.exact_sum",
            "The coverage-weighted sum of a map (or of every layer of a stack) over every polygon, \
             exactextractr's exact_extract(fun = \"sum\"): every cell weighted by the share of it the polygon \
             covers, exactly (the amounts of features in planning units of polygons).",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the polygons"))
        .input(PortSpec::new("map", Type::Any, "a map or a stack"))
        .output(PortSpec::new("table", Type::Table, "per feature (1..): the sum of every layer"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, names, layers) = layers_of(args, "map")?;
        let v = on_grid(vector_of(args, "vector")?, &grid)?;
        if let Some(i) = v.geometries.iter().position(|g| !g.is_polygonal() && !matches!(g, Geometry::Empty)) {
            return Err(Error::msg(format!("node {:?}: feature {} is no polygon", args.node, i + 1)));
        }
        let geoms: Vec<&Geometry> = v.geometries.iter().collect();
        let s = exact_sums(&grid, &layers, &geoms)?;
        let mut t = Table::new(std::iter::once("feature".to_string()).chain(names.iter().cloned()).collect());
        let l = layers.len();
        t.rows = (0..v.len())
            .map(|f| std::iter::once((f + 1) as f64).chain(s[f * l..(f + 1) * l].iter().copied()).collect())
            .collect();
        Ok(outputs([("table", Value::Table(t))]))
    }
}

/// The exact Euclidean distance (in map units, cells dx x dy) from every cell's centre to the nearest cell of
/// `target` (infinity without any): Felzenszwalb and Huttenlocher's lower envelopes along the columns, then the rows.
pub fn distance_to(target: &[bool], (ny, nx): (usize, usize), (dx, dy): (f64, f64)) -> Vec<f64> {
    let inf = f64::INFINITY;
    // along the columns: the squared distance to the nearest target in the column
    let mut g = vec![inf; ny * nx];
    for x in 0..nx {
        let mut last: Option<usize> = None;
        for y in 0..ny {
            if target[y * nx + x] {
                last = Some(y);
            }
            if let Some(l) = last {
                g[y * nx + x] = ((y - l) as f64 * dy).powi(2);
            }
        }
        last = None;
        for y in (0..ny).rev() {
            if target[y * nx + x] {
                last = Some(y);
            }
            if let Some(l) = last {
                g[y * nx + x] = g[y * nx + x].min(((l - y) as f64 * dy).powi(2));
            }
        }
    }
    // along the rows: the lower envelope of the parabolas (dx (x - q))^2 + g(q)
    let mut out = vec![inf; ny * nx];
    let (mut v, mut z) = (vec![0usize; nx], vec![0f64; nx + 1]);
    for y in 0..ny {
        let f = &g[y * nx..(y + 1) * nx];
        let finite: Vec<usize> = (0..nx).filter(|&q| f[q].is_finite()).collect();
        if finite.is_empty() {
            continue;
        }
        let px = |q: usize| q as f64 * dx;
        let mut k = 0;
        v[0] = finite[0];
        z[0] = f64::NEG_INFINITY;
        z[1] = f64::INFINITY;
        for &q in &finite[1..] {
            let mut s;
            loop {
                let p = v[k];
                s = ((f[q] + px(q) * px(q)) - (f[p] + px(p) * px(p))) / (2.0 * (px(q) - px(p)));
                if s > z[k] {
                    break;
                }
                k -= 1;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
        }
        let mut j = 0;
        for x in 0..nx {
            while z[j + 1] < px(x) {
                j += 1;
            }
            let p = v[j];
            out[y * nx + x] = ((px(x) - px(p)).powi(2) + f[p]).sqrt();
        }
    }
    out
}

struct Distance;

impl Operator for Distance {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.distance",
            "The distance (in the map's units) from every cell's centre to the nearest cell of the features (as \
             vector.rasterize burns them), exactly (the Euclidean distance transform).",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .input(PortSpec::new("like", Type::Map, "a map on the grid of the distances"))
        .output(PortSpec::new("map", Type::Map, "the distance of every cell (0 in the features)"))
        .param(PortSpec::optional("all_touched", Type::Bool, "every cell a feature touches is one (false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let like = args.map("like")?;
        let v = on_grid(vector_of(args, "vector")?, &like.grid)?;
        let grid = &like.grid;
        let mut mask = vec![0.0; grid.width * grid.height];
        burn(&mut mask, grid, &v, &vec![1.0; v.len()], args.param("all_touched", false)?, false)?;
        let target: Vec<bool> = mask.iter().map(|m| *m != 0.0).collect();
        let (dx, dy) = (grid.transform[1].abs(), grid.transform[5].abs());
        let d = distance_to(&target, (grid.height, grid.width), (dx, dy));
        Ok(outputs([("map", Value::Map(Map::new(grid.clone(), d)))]))
    }
}

// ---------------------------------------------------------------------------------------------------- polygons

/// The segments of the boundaries of a polygonal geometry: every ring's.
fn segments(g: &Geometry) -> Vec<([f64; 2], [f64; 2])> {
    let mut out = Vec::new();
    for poly in g.polygons() {
        for ring in poly {
            for w in ring.windows(2) {
                if w[0] != w[1] {
                    out.push((w[0], w[1]));
                }
            }
        }
    }
    out
}

fn overlaps(a: &Option<[f64; 4]>, b: &Option<[f64; 4]>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3],
        _ => false,
    }
}

/// The length two segments share (collinear and overlapping), 0 else.
fn shared(a: ([f64; 2], [f64; 2]), b: ([f64; 2], [f64; 2])) -> f64 {
    let d = [a.1[0] - a.0[0], a.1[1] - a.0[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let cross = |p: [f64; 2]| d[0] * (p[1] - a.0[1]) - d[1] * (p[0] - a.0[0]);
    let tol = 1e-12 * len2;
    if cross(b.0).abs() > tol || cross(b.1).abs() > tol {
        return 0.0;
    }
    // the positions along a of b's ends
    let t = |p: [f64; 2]| (d[0] * (p[0] - a.0[0]) + d[1] * (p[1] - a.0[1])) / len2;
    let (t0, t1) = (t(b.0), t(b.1));
    let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(1.0));
    if hi > lo { (hi - lo) * len2.sqrt() } else { 0.0 }
}

/// R's round(x, 8) as numpy's: x * 1e8 rounded half to even, / 1e8.
fn round8(x: f64) -> f64 {
    (x * 1e8).round_ties_even() / 1e8
}

/// prioritizr's boundary_matrix of polygons (landschaft's): the length of the boundary every two units share
/// (their boundaries' common paths) and every unit's perimeter on the diagonal, rounded to 8 decimals; both
/// triangles, the zeros left out.
pub fn boundary_matrix(geoms: &[Geometry]) -> crate::milp::Matrix {
    let n = geoms.len();
    let segs: Vec<Vec<([f64; 2], [f64; 2])>> = geoms.iter().map(segments).collect();
    let bounds: Vec<Option<[f64; 4]>> = geoms.iter().map(Geometry::bounds).collect();
    let (mut i, mut j, mut x) = (Vec::new(), Vec::new(), Vec::new());
    for a in 0..n {
        let perimeter: f64 = segs[a].iter().map(|s| (s.1[0] - s.0[0]).hypot(s.1[1] - s.0[1])).sum();
        let p = round8(perimeter);
        if p != 0.0 {
            i.push(a);
            j.push(a);
            x.push(p);
        }
        for b in a + 1..n {
            if !overlaps(&bounds[a], &bounds[b]) {
                continue;
            }
            let len: f64 = segs[a].iter().map(|&sa| segs[b].iter().map(|&sb| shared(sa, sb)).sum::<f64>()).sum();
            let len = round8(len);
            if len != 0.0 {
                i.extend([a, b]);
                j.extend([b, a]);
                x.extend([len, len]);
            }
        }
    }
    crate::milp::Matrix { n, i, j, x }
}

/// Whether two segments meet (touching included).
fn meet(a: ([f64; 2], [f64; 2]), b: ([f64; 2], [f64; 2])) -> bool {
    let orient = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        let v = (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
        if v > 0.0 {
            1
        } else if v < 0.0 {
            -1
        } else {
            0
        }
    };
    let on = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        r[0] >= p[0].min(q[0]) && r[0] <= p[0].max(q[0]) && r[1] >= p[1].min(q[1]) && r[1] <= p[1].max(q[1])
    };
    let (o1, o2, o3, o4) = (orient(a.0, a.1, b.0), orient(a.0, a.1, b.1), orient(b.0, b.1, a.0), orient(b.0, b.1, a.1));
    (o1 != o2 && o3 != o4)
        || (o1 == 0 && on(a.0, a.1, b.0))
        || (o2 == 0 && on(a.0, a.1, b.1))
        || (o3 == 0 && on(b.0, b.1, a.0))
        || (o4 == 0 && on(b.0, b.1, a.1))
}

/// Whether a point is inside a polygonal geometry (even-odd over its rings).
fn inside(g: &Geometry, p: [f64; 2]) -> bool {
    g.polygons().iter().any(|poly| {
        let mut c = false;
        for ring in poly.iter() {
            for w in ring.windows(2) {
                let (a, b) = (w[0], w[1]);
                if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
                    c = !c;
                }
            }
        }
        c
    })
}

/// prioritizr's adjacency_matrix of polygons (landschaft's): 1 for every two units that intersect (touching at a
/// corner included); both triangles.
pub fn adjacency_matrix(geoms: &[Geometry]) -> crate::milp::Matrix {
    let n = geoms.len();
    let segs: Vec<Vec<([f64; 2], [f64; 2])>> = geoms.iter().map(segments).collect();
    let bounds: Vec<Option<[f64; 4]>> = geoms.iter().map(Geometry::bounds).collect();
    let first = |g: &Geometry| -> Option<[f64; 2]> {
        let mut p = None;
        g.points(&mut |q| {
            if p.is_none() {
                p = Some(q)
            }
        });
        p
    };
    let (mut i, mut j, mut x) = (Vec::new(), Vec::new(), Vec::new());
    for a in 0..n {
        for b in a + 1..n {
            if !overlaps(&bounds[a], &bounds[b]) {
                continue;
            }
            let touch = segs[a].iter().any(|&sa| segs[b].iter().any(|&sb| meet(sa, sb)))
                || first(&geoms[b]).is_some_and(|p| inside(&geoms[a], p))
                || first(&geoms[a]).is_some_and(|p| inside(&geoms[b], p));
            if touch {
                i.extend([a, b]);
                j.extend([b, a]);
                x.extend([1.0, 1.0]);
            }
        }
    }
    crate::milp::Matrix { n, i, j, x }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_is_exact() {
        let (ny, nx) = (9, 13);
        let target: Vec<bool> = (0..ny * nx).map(|c| c == 20 || c == 77 || c == 110).collect();
        let d = distance_to(&target, (ny, nx), (2.0, 3.0));
        for (c, &dc) in d.iter().enumerate() {
            let (y, x) = ((c / nx) as f64, (c % nx) as f64);
            let want = (0..ny * nx)
                .filter(|&t| target[t])
                .map(|t| (((t % nx) as f64 - x) * 2.0).hypot(((t / nx) as f64 - y) * 3.0))
                .fold(f64::INFINITY, f64::min);
            assert!((dc - want).abs() < 1e-12, "{c}: {dc} != {want}");
        }
    }
}
