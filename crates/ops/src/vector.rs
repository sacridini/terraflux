//! Vector features: read and written by OGR (GDAL), burnt into maps, their zones' statistics and the exact sums of
//! maps over polygons, and distances to them. Polygons are burnt as GDAL's rasterizer does through landschaft's
//! C++ port of it (`lsio::polygon_runs`, the same cells), lines by GDAL, points by their cell, as landschaft's
//! rasterize; the exact sums are exactextract's (landschaft's port of it, exactextractr's bits).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::vector_tiled::Layers;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Read));
    r.add(Arc::new(Write));
    r.add(Arc::new(Rasterize));
    r.add(Arc::new(Zonal));
    r.add(Arc::new(ExactSum));
    r.add(Arc::new(Distance));
    r.add(Arc::new(Join));
    r.add(Arc::new(Extract));
    r.add(Arc::new(Buffer));
    r.add(Arc::new(Clip));
    r.add(Arc::new(Dissolve));
}

/// The rows of a table joined to features: for every feature, the row whose `key` equals its `on` (its number 1..
/// when `on` is none), if any.
pub fn join_rows(
    v: &Vector,
    t: &Table,
    on: Option<&str>,
    key: usize,
) -> std::result::Result<Vec<Option<usize>>, String> {
    let of_feature: Vec<f64> = match on {
        None => (1..=v.len()).map(|i| i as f64).collect(),
        Some(name) => match v.column(name) {
            Some(Column::Numbers(x)) => x.clone(),
            Some(Column::Texts(x)) => x.iter().map(|s| s.trim().parse().unwrap_or(f64::NAN)).collect(),
            None => return Err(format!("the features have no attribute {name:?}")),
        },
    };
    let mut index = std::collections::HashMap::new();
    for (r, row) in t.rows.iter().enumerate() {
        index.entry(row[key].to_bits()).or_insert(r);
    }
    Ok(of_feature.iter().map(|k| if k.is_nan() { None } else { index.get(&k.to_bits()).copied() }).collect())
}

struct Join;

impl Operator for Join {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.join",
            "A table's columns as attributes of features: every feature takes the row whose key column equals its \
             attribute `on` (its number 1.. by default), e.g. the cluster of every zone of metrics.cluster on \
             vector.grid_zones' squares (key zone); no value where no row matches. Attributes of the same name are \
             replaced.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .input(PortSpec::new("table", Type::Table, "the table"))
        .output(PortSpec::new("vector", Type::Vector, "the features with the table's columns"))
        .param(PortSpec::optional("on", Type::Text, "the features' attribute (default their number, 1..)"))
        .param(PortSpec::optional(
            "key",
            Type::Text,
            "the table's column (default zone, feature, plot_id or id if it has one, else its first)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: String| Error::msg(format!("node {:?}: {e}", args.node));
        let v = vector_of(args, "vector")?;
        let Some(Value::Table(t)) = args.get("table") else { return Err(fail("connect a table".into())) };
        let key = match args.opt_param::<String>("key")?.filter(|k| !k.is_empty()) {
            Some(k) => t.columns.iter().position(|c| *c == k).ok_or_else(|| fail(format!("the table has no {k:?}")))?,
            None => ["zone", "feature", "plot_id", "id"]
                .iter()
                .find_map(|k| t.columns.iter().position(|c| c == k))
                .unwrap_or(0),
        };
        let on: Option<String> = args.opt_param::<String>("on")?.filter(|o| !o.is_empty());
        let rows = join_rows(v, t, on.as_deref(), key).map_err(fail)?;
        let mut out = (**v).clone();
        for (j, name) in t.columns.iter().enumerate().filter(|(j, _)| *j != key) {
            let values = rows.iter().map(|r| r.map_or(f64::NAN, |r| t.rows[r][j])).collect();
            match out.attributes.iter_mut().find(|(n, _)| n == name) {
                Some((_, c)) => *c = Column::Numbers(values),
                None => out.attributes.push((name.clone(), Column::Numbers(values))),
            }
        }
        Ok(outputs([("vector", Value::Vector(Arc::new(out)))]))
    }
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

/// A map (or a stack) input as its layers on one grid, and the cells of a layer of the strips it is read by (none:
/// whole): by blocks of `block_rows` rows when asked for, and rasters on disk of more than 250 million cells
/// always (16 million cells a strip by default).
fn layers_of(args: &Args, port: &str) -> Result<(Layers, Option<usize>)> {
    let rows = crate::metrics::rows_param(args)?;
    let budget = |nx: usize| rows.map_or(1 << 24, |r| r * nx);
    let memory = |grid: &Arc<Grid>, names: Vec<String>, layers: Vec<Arc<Vec<f64>>>| {
        let b = rows.map(|r| r * grid.width);
        (Layers::Memory { grid: grid.clone(), names, layers }, b)
    };
    Ok(match args.get(port) {
        Some(Value::Stack(s)) => memory(&s.grid, s.keys.iter().map(|k| k.to_string()).collect(), s.layers.clone()),
        Some(Value::Raster(r)) if rows.is_some() || r.cells() > crate::metrics::TILED_CELLS => {
            let nx = r.grid_of(r.chosen).width;
            let layers = Layers::Disk { names: vec![port.to_string()], rasters: r.clone(), layers: vec![r.chosen] };
            (layers, Some(budget(nx)))
        }
        Some(Value::Raster(r)) => {
            let m = r.map()?;
            memory(&m.grid, vec![port.to_string()], vec![m.data.clone()])
        }
        _ => {
            let m = args.map(port)?;
            memory(&m.grid, vec![port.to_string()], vec![m.data.clone()])
        }
    })
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
             overlaps). The features reprojected to the map's coordinate system first. Out of core (block_rows, a              path, or rasters on disk of more than 250 million cells) burnt strip by strip into a GeoTIFF, the              same cells (but where GDAL draws a line through cells' corners with all_touched).",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .input(PortSpec::new("like", Type::Raster, "a map on the grid to burn into (only its grid is read)"))
        .output(PortSpec::new("map", Type::Raster, "the features burnt (on disk for rasters on disk)"))
        .param(PortSpec::optional("field", Type::Text, "the attribute burnt (features without it are left out)"))
        .param(PortSpec::optional("value", Type::Number, "the value burnt without a field (1)"))
        .param(PortSpec::optional("fill", Type::Number, "the cells no feature takes (0; null: no value)"))
        .param(PortSpec::optional("all_touched", Type::Bool, "every cell a feature touches (false)"))
        .param(
            PortSpec::optional("merge", Type::Text, "replace (default: the last feature wins) or add")
                .choices(["replace", "add"]),
        )
        .param(PortSpec::optional(
            "block_rows",
            Type::Integer,
            "out of core: burnt by strips of this many rows into a GeoTIFF (path); on the grid of rasters on disk of more than 250 million cells always",
        ))
        .param(PortSpec::optional(
            "path",
            Type::Text,
            "out of core: the GeoTIFF written (default next to the rasters: name_burnt.tif)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let rows = crate::metrics::rows_param(args)?;
        let (grid, disk) = match args.get("like") {
            Some(Value::Raster(r)) => (Arc::new(r.grid_of(r.chosen).clone()), Some(r.clone())),
            Some(Value::Map(m)) => (m.grid.clone(), None),
            Some(Value::Stack(s)) => (s.grid.clone(), None),
            Some(v) => return Err(Error::msg(format!("node {:?}: like: a map, not a {:?}", args.node, v.ty()))),
            None => return Err(Error::msg(format!("node {:?}: connect a map (like)", args.node))),
        };
        let v = on_grid(vector_of(args, "vector")?, &grid)?;
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
        let all_touched = args.param("all_touched", false)?;
        let path: Option<String> = args.opt_param::<String>("path")?.filter(|p| !p.is_empty());
        if rows.is_some() || path.is_some() || disk.as_ref().is_some_and(|r| r.cells() > crate::metrics::TILED_CELLS) {
            let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
            let path = match (path, &disk) {
                (Some(p), _) => std::path::PathBuf::from(p).with_extension("tif"),
                (None, Some(r)) => {
                    let src = r.images[r.layers[r.chosen].image].path();
                    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    src.with_file_name(format!("{stem}_burnt.tif"))
                }
                (None, None) => return Err(fail(Error::msg("path: the GeoTIFF to write the features to"))),
            };
            if disk.as_ref().is_some_and(|r| r.images.iter().any(|i| i.path() == path)) {
                return Err(fail(Error::msg("the output must differ from the input")));
            }
            if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir).map_err(|e| fail(Error::msg(format!("{}: {e}", dir.display()))))?;
            }
            let rows = crate::metrics::block_rows(rows, grid.width);
            crate::vector_tiled::burn_to_file(&path, &grid, &kept, &vals, fill, all_touched, merge == "add", rows)
                .map_err(fail)?;
            let written = crate::files::Rasters::open(&path.to_string_lossy(), None, None)?;
            return Ok(outputs([("map", Value::Raster(Arc::new(written)))]));
        }
        let mut out = vec![fill; grid.width * grid.height];
        burn(&mut out, &grid, &kept, &vals, all_touched, merge == "add")?;
        Ok(outputs([("map", Value::Map(Map::new(grid, out)))]))
    }
}

/// landschaft's statistics of zones by name, and their codes in lsio::zonal_stats.
pub(crate) fn stat_code(s: &str) -> Option<(i32, f64)> {
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
             polygon's cells as GDAL burns them (polygons may overlap), the cells with a value. Out of core              (block_rows, or rasters on disk of more than 250 million cells) by strips of rows, the same values.",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the polygons"))
        .input(PortSpec::new("map", Type::Raster, "a map or a stack (or rasters on disk, by strips)"))
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
        .param(PortSpec::optional(
            "block_rows",
            Type::Integer,
            "out of core: the map read by strips of this many rows, the same values; rasters on disk of more than \
             250 million cells always are",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let (layers, budget) = layers_of(args, "map")?;
        let v = on_grid(vector_of(args, "vector")?, layers.grid())?;
        if let Some(i) = v.geometries.iter().position(|g| !g.is_polygonal() && !matches!(g, Geometry::Empty)) {
            return Err(err(format!("feature {}: zones are polygons (buffer points or lines first)", i + 1)));
        }
        let stats = stats_of(args)?;
        let geoms: Vec<&Geometry> = v.geometries.iter().collect();
        let t = zonal_table(&geoms, &layers, budget, &stats, args.param("all_touched", false)?)
            .map_err(|e| err(e.to_string()))?;
        Ok(outputs([("table", Value::Table(t))]))
    }
}

/// The statistics a node asks for (`stats`: a name or a list; count, mean, min, max and std by default).
fn stats_of(args: &Args) -> Result<Vec<String>> {
    let stats: Vec<String> = match args.params.get("stats") {
        None | Some(serde_json::Value::Null) => ["count", "mean", "min", "max", "std"].map(String::from).to_vec(),
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(v) => {
            serde_json::from_value(v.clone()).map_err(|e| Error::msg(format!("node {:?}: stats: {e}", args.node)))?
        }
    };
    if let Some(s) = stats.iter().find(|s| stat_code(s).is_none()) {
        return Err(Error::msg(format!("node {:?}: unknown statistic {s:?}", args.node)));
    }
    Ok(stats)
}

/// landschaft's zonal_stats of layers in polygons: a row per feature (1..), a column per statistic (of every
/// layer: <layer>_<statistic> when there are several). Whole, or by strips of `budget` cells of a layer.
fn zonal_table(
    geoms: &[&Geometry],
    layers: &Layers,
    budget: Option<usize>,
    stats: &[String],
    all_touched: bool,
) -> Result<Table> {
    let codes: Vec<(i32, f64)> = stats.iter().filter_map(|s| stat_code(s)).collect();
    let grid = layers.grid();
    let r = runs(geoms, grid, all_touched)?;
    let (c, p): (Vec<i32>, Vec<f64>) = codes.iter().copied().unzip();
    let out = match budget {
        Some(b) => crate::vector_tiled::zonal_stats(layers, &r, &c, &p, b)?,
        None => {
            let values = layers.window(0, grid.height, 0, grid.width)?;
            ffi::io_zonal_stats(&values, layers.bands() as i32, grid.height as i32, grid.width as i32, &r, &c, &p)?
        }
    };
    let names = layers.names();
    let multi = names.len() > 1;
    let mut cols = vec!["feature".to_string()];
    for b in names {
        for s in stats {
            cols.push(if multi { format!("{b}_{s}") } else { s.clone() });
        }
    }
    let mut t = Table::new(cols);
    let per = names.len() * stats.len();
    t.rows = (0..geoms.len())
        .map(|z| std::iter::once((z + 1) as f64).chain(out[z * per..(z + 1) * per].iter().copied()).collect())
        .collect();
    Ok(t)
}

/// Columns added to features (replacing those of the same name), and their attributes as a table.
fn with_columns(v: &Vector, columns: Vec<(String, Vec<f64>)>) -> (Vector, Table) {
    let mut out = v.clone();
    for (name, values) in columns {
        match out.attributes.iter_mut().find(|(n, _)| *n == name) {
            Some((_, c)) => *c = Column::Numbers(values),
            None => out.attributes.push((name, Column::Numbers(values))),
        }
    }
    let t = attribute_table(&out);
    (out, t)
}

struct Extract;

impl Operator for Extract {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.extract",
            "The values of a map (or of every layer of a stack) at points, landschaft's extract: the cell holding \
             each point (nearest) or interpolated from the four nearest cell centres (bilinear, the weights over \
             those with a value); with a buffer, the statistics of the cells within that radius of each point \
             (zonal_stats of GEOS's circles). The points are reprojected to the map. No value outside it. Out of              core (block_rows, or rasters on disk of more than 250 million cells) only the strips of rows holding              points are read, the same values.",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the points"))
        .input(PortSpec::new("map", Type::Raster, "a map or a stack (or rasters on disk, by strips)"))
        .output(PortSpec::new("vector", Type::Vector, "the points with a column per layer (or statistic)"))
        .output(PortSpec::new("table", Type::Table, "per point (feature 1..): its attributes and the values"))
        .param(
            PortSpec::optional("method", Type::Text, "nearest (default) or bilinear").choices(["nearest", "bilinear"]),
        )
        .param(PortSpec::optional("buffer", Type::Number, "a radius (map units): statistics of the cells around"))
        .param(PortSpec::optional("stats", Type::Any, "with a buffer: the statistics, as vector.zonal's"))
        .param(PortSpec::optional("all_touched", Type::Bool, "with a buffer: every cell the circle touches"))
        .param(PortSpec::optional(
            "block_rows",
            Type::Integer,
            "out of core: the map read by strips of this many rows, the same values; rasters on disk of more than \
             250 million cells always are",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let err = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let (layers, budget) = layers_of(args, "map")?;
        let names = layers.names().to_vec();
        let grid = layers.grid().clone();
        let original = vector_of(args, "vector")?;
        let v = on_grid(original, &grid)?;
        let mut xy = Vec::with_capacity(v.len());
        for (i, g) in v.geometries.iter().enumerate() {
            match g {
                Geometry::Point(p) => xy.push(*p),
                Geometry::Empty => xy.push([f64::NAN, f64::NAN]),
                _ => return Err(err(format!("feature {}: the values are taken at points", i + 1))),
            }
        }
        let label = |b: &String| if names.len() == 1 { "value".to_string() } else { b.clone() };
        let columns: Vec<(String, Vec<f64>)> = match args.opt_param::<f64>("buffer")? {
            Some(radius) => {
                if radius.is_nan() || radius <= 0.0 {
                    return Err(err("buffer: a radius > 0".into()));
                }
                let circles = terraflux_gdal::buffer(&v.geometries, radius, 16).map_err(err)?;
                let geoms: Vec<&Geometry> = circles.iter().collect();
                let stats = stats_of(args)?;
                let t = zonal_table(&geoms, &layers, budget, &stats, args.param("all_touched", false)?)?;
                (1..t.columns.len()).map(|j| (t.columns[j].clone(), t.rows.iter().map(|r| r[j]).collect())).collect()
            }
            None => {
                let method: String = args.param("method", "nearest".to_string())?;
                if method != "nearest" && method != "bilinear" {
                    return Err(err("method: nearest or bilinear".into()));
                }
                let (x, y): (Vec<f64>, Vec<f64>) = xy.iter().map(|p| (p[0], p[1])).unzip();
                let nb = layers.bands();
                let rows = budget.map_or(grid.height, |b| (b / grid.width).max(1));
                let out = crate::vector_tiled::extract(&layers, &x, &y, method == "bilinear", rows)?;
                names
                    .iter()
                    .enumerate()
                    .map(|(b, n)| (label(n), (0..xy.len()).map(|i| out[i * nb + b]).collect()))
                    .collect()
            }
        };
        let (out, t) = with_columns(original, columns);
        Ok(outputs([("vector", Value::Vector(Arc::new(out))), ("table", Value::Table(t))]))
    }
}

struct Buffer;

impl Operator for Buffer {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.buffer",
            "The area within a distance of every feature (GEOS through GDAL, as shapely's buffer): circles around \
             points, corridors along lines, polygons grown (or shrunk, with a negative distance); the features' \
             attributes kept, or every buffer merged into one (dissolve).",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .output(PortSpec::new("vector", Type::Vector, "the buffers"))
        .param(PortSpec::new("distance", Type::Number, "the distance, in the features' units"))
        .param(PortSpec::optional("segments", Type::Integer, "segments of a quarter circle (16)"))
        .param(PortSpec::optional("dissolve", Type::Bool, "merge the buffers into one feature (false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let err = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let v = vector_of(args, "vector")?;
        let distance: f64 = args.opt_param("distance")?.ok_or_else(|| err("distance: how far".into()))?;
        let segments: i32 = args.param("segments", 16)?;
        if segments < 1 || !distance.is_finite() {
            return Err(err("a finite distance and segments >= 1".into()));
        }
        let geometries = terraflux_gdal::buffer(&v.geometries, distance, segments).map_err(err)?;
        let out = if args.param("dissolve", false)? {
            let all: Vec<&Geometry> = geometries.iter().collect();
            let u = terraflux_gdal::union(&all).map_err(err)?;
            Vector { geometries: vec![u], attributes: Vec::new(), crs: v.crs.clone() }
        } else {
            Vector { geometries, attributes: v.attributes.clone(), crs: v.crs.clone() }
        };
        Ok(outputs([("vector", Value::Vector(Arc::new(out)))]))
    }
}

struct Clip;

impl Operator for Clip {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.clip",
            "Features cut to the area of a mask (the union of its polygons; GEOS through GDAL, as geopandas' clip): \
             each feature's part inside it, those outside it left out, their attributes kept.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .input(PortSpec::new("mask", Type::Vector, "the polygons they are cut to (reprojected to them)"))
        .output(PortSpec::new("vector", Type::Vector, "the features' parts inside the mask"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let err = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let v = vector_of(args, "vector")?;
        let mask = vector_of(args, "mask")?;
        let mask = match (&mask.crs, &v.crs) {
            (Some(a), Some(b)) if !terraflux_gdal::same_crs(a, b).unwrap_or(true) => {
                Arc::new(terraflux_gdal::reproject_vector(mask, b).map_err(err)?)
            }
            _ => mask.clone(),
        };
        if mask.geometries.iter().any(|g| !g.is_polygonal() && !matches!(g, Geometry::Empty)) {
            return Err(err("the mask is polygons".into()));
        }
        let all: Vec<&Geometry> = mask.geometries.iter().collect();
        let area = terraflux_gdal::union(&all).map_err(err)?;
        let cut = terraflux_gdal::intersection(&v.geometries, &area).map_err(err)?;
        let keep: Vec<usize> = (0..cut.len()).filter(|&i| !matches!(cut[i], Geometry::Empty)).collect();
        let mut out = v.select(&keep);
        out.geometries = keep.iter().map(|&i| cut[i].clone()).collect();
        Ok(outputs([("vector", Value::Vector(Arc::new(out)))]))
    }
}

struct Dissolve;

impl Operator for Dissolve {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.dissolve",
            "Features merged (GEOS's union through GDAL, as geopandas' dissolve): all into one, or one per value \
             of an attribute (`by`), with that value and the features merged (count).",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features (of one kind: points, lines or polygons)"))
        .output(PortSpec::new("vector", Type::Vector, "the features merged"))
        .param(PortSpec::optional("by", Type::Text, "the attribute whose values are merged apart (none: all)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let err = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let v = vector_of(args, "vector")?;
        let by: Option<String> = args.opt_param::<String>("by")?.filter(|b| !b.is_empty());
        // the groups, in the order of their first feature: their value (as text and number) and features
        let keys: Vec<String> = match &by {
            None => vec![String::new(); v.len()],
            Some(b) => match v.column(b) {
                Some(Column::Numbers(x)) => x.iter().map(|x| format!("{:?}", x.to_bits())).collect(),
                Some(Column::Texts(t)) => t.clone(),
                None => return Err(err(format!("the features have no attribute {b:?}"))),
            },
        };
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, k) in keys.iter().enumerate() {
            match groups.iter_mut().find(|(g, _)| g == k) {
                Some((_, m)) => m.push(i),
                None => groups.push((k.clone(), vec![i])),
            }
        }
        let geometries = groups
            .iter()
            .map(|(_, m)| {
                let geoms: Vec<&Geometry> = m.iter().map(|&i| &v.geometries[i]).collect();
                terraflux_gdal::union(&geoms).map_err(err)
            })
            .collect::<Result<Vec<_>>>()?;
        let mut attributes = Vec::new();
        if let Some(b) = &by {
            let first: Vec<usize> = groups.iter().map(|(_, m)| m[0]).collect();
            let col = match v.column(b).unwrap() {
                Column::Numbers(x) => Column::Numbers(first.iter().map(|&i| x[i]).collect()),
                Column::Texts(t) => Column::Texts(first.iter().map(|&i| t[i].clone()).collect()),
            };
            attributes.push((b.clone(), col));
        }
        attributes.push(("count".into(), Column::Numbers(groups.iter().map(|(_, m)| m.len() as f64).collect())));
        Ok(outputs([("vector", Value::Vector(Arc::new(Vector { geometries, attributes, crs: v.crs.clone() })))]))
    }
}

/// The grid as exactextract's: xmin, ymin, xmax, ymax, dx, dy (north-up maps only).
pub fn exact_bounds(grid: &Grid) -> Result<[f64; 6]> {
    let t = &grid.transform;
    if t[2] != 0.0 || t[4] != 0.0 || t[5] >= 0.0 {
        return Err(Error::msg("the map must be north-up"));
    }
    let (dx, dy) = (t[1], -t[5]);
    let (ny, nx) = (grid.height, grid.width);
    Ok([t[0], t[3] - ny as f64 * dy, t[0] + nx as f64 * dx, t[3], dx, dy])
}

/// exactextractr's exact_extract(fun = "sum") of every layer over every polygonal feature: features x layers.
pub fn exact_sums(grid: &Grid, layers: &[Arc<Vec<f64>>], geoms: &[&Geometry]) -> Result<Vec<f64>> {
    let bounds = exact_bounds(grid)?;
    let (ny, nx) = (grid.height, grid.width);
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
             covers, exactly (the amounts of features in planning units of polygons). Out of core (block_rows, or \
             rasters on disk of more than 250 million cells) the windows under every polygon read from disk, the \
             same sums.",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the polygons"))
        .input(PortSpec::new("map", Type::Any, "a map or a stack"))
        .output(PortSpec::new("table", Type::Table, "per feature (1..): the sum of every layer"))
        .param(PortSpec::optional(
            "block_rows",
            Type::Integer,
            "out of core: the values read under every polygon only, the same sums; rasters on disk of more than \
             250 million cells always are",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (layers, budget) = layers_of(args, "map")?;
        let (grid, names) = (Arc::new(layers.grid().clone()), layers.names().to_vec());
        let v = on_grid(vector_of(args, "vector")?, &grid)?;
        if let Some(i) = v.geometries.iter().position(|g| !g.is_polygonal() && !matches!(g, Geometry::Empty)) {
            return Err(Error::msg(format!("node {:?}: feature {} is no polygon", args.node, i + 1)));
        }
        let geoms: Vec<&Geometry> = v.geometries.iter().collect();
        let s = match budget {
            Some(_) => crate::vector_tiled::exact_sums(&layers, &geoms)?,
            None => {
                let all = layers.window(0, grid.height, 0, grid.width)?;
                let layers: Vec<Arc<Vec<f64>>> =
                    all.chunks(grid.cells().max(1)).map(|c| Arc::new(c.to_vec())).collect();
                exact_sums(&grid, &layers, &geoms)?
            }
        };
        let l = names.len();
        let mut t = Table::new(std::iter::once("feature".to_string()).chain(names.iter().cloned()).collect());
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
    (0..ny).flat_map(|y| envelope_row(&g[y * nx..(y + 1) * nx], dx)).collect()
}

/// The distances along a row from the squared distances along the columns `f`: the lower envelope of the
/// parabolas (dx (x - q))^2 + f(q) (infinity without any).
pub fn envelope_row(f: &[f64], dx: f64) -> Vec<f64> {
    let nx = f.len();
    let mut out = vec![f64::INFINITY; nx];
    let finite: Vec<usize> = (0..nx).filter(|&q| f[q].is_finite()).collect();
    if finite.is_empty() {
        return out;
    }
    let (mut v, mut z) = (vec![0usize; nx], vec![0f64; nx + 1]);
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
    for (x, o) in out.iter_mut().enumerate() {
        while z[j + 1] < px(x) {
            j += 1;
        }
        let p = v[j];
        *o = ((px(x) - px(p)).powi(2) + f[p]).sqrt();
    }
    out
}

struct Distance;

impl Operator for Distance {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.distance",
            "The distance (in the map's units) from every cell's centre to the nearest cell of the features (as \
             vector.rasterize burns them), exactly (the Euclidean distance transform). Out of core (block_rows, \
             a path, or rasters on disk of more than 250 million cells) by strips into a GeoTIFF, the same \
             distances (but where GDAL draws a line through cells' corners with all_touched).",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .input(PortSpec::new("like", Type::Raster, "a map on the grid of the distances (only its grid is read)"))
        .output(PortSpec::new(
            "map",
            Type::Raster,
            "the distance of every cell (0 in the features; on disk for \
             rasters on disk)",
        ))
        .param(PortSpec::optional("all_touched", Type::Bool, "every cell a feature touches is one (false)"))
        .param(PortSpec::optional(
            "block_rows",
            Type::Integer,
            "out of core: by strips of this many rows into a GeoTIFF (path); on the grid of rasters on disk of \
             more than 250 million cells always",
        ))
        .param(PortSpec::optional(
            "path",
            Type::Text,
            "out of core: the GeoTIFF written (default next to the rasters: name_distance.tif)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let rows = crate::metrics::rows_param(args)?;
        let (grid, disk) = match args.get("like") {
            Some(Value::Raster(r)) => (Arc::new(r.grid_of(r.chosen).clone()), Some(r.clone())),
            Some(Value::Map(m)) => (m.grid.clone(), None),
            Some(Value::Stack(s)) => (s.grid.clone(), None),
            Some(v) => return Err(Error::msg(format!("node {:?}: like: a map, not a {:?}", args.node, v.ty()))),
            None => return Err(Error::msg(format!("node {:?}: connect a map (like)", args.node))),
        };
        let v = on_grid(vector_of(args, "vector")?, &grid)?;
        let all_touched = args.param("all_touched", false)?;
        let path: Option<String> = args.opt_param::<String>("path")?.filter(|p| !p.is_empty());
        if rows.is_some() || path.is_some() || disk.as_ref().is_some_and(|r| r.cells() > crate::metrics::TILED_CELLS) {
            let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
            let path = match (path, &disk) {
                (Some(p), _) => std::path::PathBuf::from(p).with_extension("tif"),
                (None, Some(r)) => {
                    let src = r.images[r.layers[r.chosen].image].path();
                    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    src.with_file_name(format!("{stem}_distance.tif"))
                }
                (None, None) => return Err(fail(Error::msg("path: the GeoTIFF to write the distances to"))),
            };
            if disk.as_ref().is_some_and(|r| r.images.iter().any(|i| i.path() == path)) {
                return Err(fail(Error::msg("the output must differ from the input")));
            }
            if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir).map_err(|e| fail(Error::msg(format!("{}: {e}", dir.display()))))?;
            }
            let rows = crate::metrics::block_rows(rows, grid.width);
            crate::vector_tiled::distance_to_file(&path, &grid, &v, all_touched, rows).map_err(fail)?;
            let written = crate::files::Rasters::open(&path.to_string_lossy(), None, None)?;
            return Ok(outputs([("map", Value::Raster(Arc::new(written)))]));
        }
        let grid = &grid;
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
