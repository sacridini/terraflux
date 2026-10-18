//! Classic spatial analysis: the kernel density of points and their count per cell (`vector.density`), contour
//! lines (`raster.contour`, as gdal_contour), what can be seen from observers (`raster.viewshed`), the true area
//! of every cell (`raster.cell_area`, on the ellipsoid for longitudes and latitudes), and a map classified by breaks
//! (`raster.classify`: quantiles, equal intervals, Jenks's natural breaks, standard deviations, or breaks given).

use std::collections::HashMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

use crate::raster_stats::{percentile, sorted_values};
use crate::vector::{attribute_table, on_grid, vector_of};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Density));
    r.add(Arc::new(Contour));
    r.add(Arc::new(Viewshed));
    r.add(Arc::new(CellArea));
    r.add(Arc::new(Classify));
}

/// The centre of a cell (row, column) in the grid's coordinates.
fn centre(g: &Grid, r: f64, c: f64) -> Xy {
    let t = g.transform;
    [t[0] + (c + 0.5) * t[1] + (r + 0.5) * t[2], t[3] + (c + 0.5) * t[4] + (r + 0.5) * t[5]]
}

/// The cell (row, column, as fractions) holding a point of a north-up grid.
fn cell_of(g: &Grid, p: Xy) -> (f64, f64) {
    let t = g.transform;
    ((p[1] - t[3]) / t[5], (p[0] - t[0]) / t[1])
}

/// The points of features (every point of multipoints) and their index.
fn points_of(v: &Vector) -> Vec<(usize, Xy)> {
    let mut out = Vec::new();
    for (i, g) in v.geometries.iter().enumerate() {
        if g.is_point() {
            g.points(&mut |p| out.push((i, p)));
        }
    }
    out
}

/// ArcGIS's default search radius of a kernel density (Silverman's rule, spatially): 0.9 min(SD, √(1/ln 2) Dm)
/// n^-0.2, SD the standard distance of the points and Dm their median distance to their mean centre.
fn default_radius(points: &[(usize, Xy)]) -> f64 {
    let n = points.len() as f64;
    let (mx, my) = (points.iter().map(|p| p.1[0]).sum::<f64>() / n, points.iter().map(|p| p.1[1]).sum::<f64>() / n);
    let mut d: Vec<f64> = points.iter().map(|p| ((p.1[0] - mx).powi(2) + (p.1[1] - my).powi(2)).sqrt()).collect();
    let sd = (d.iter().map(|x| x * x).sum::<f64>() / n).sqrt();
    d.sort_by(f64::total_cmp);
    let dm = percentile(&d, 50.0);
    0.9 * sd.min((1.0 / std::f64::consts::LN_2).sqrt() * dm) * n.powf(-0.2)
}

/// `vector.density`.
struct Density;

impl Operator for Density {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.density",
            "The density of points on a map's grid, as ArcGIS's Kernel Density or QGIS's Heatmap: each point (its \
             weight an attribute, or 1) spread by a kernel over a radius, the kernel's integral the weight, so the \
             map is points per square map unit (or per hectare, per km²); quartic (biweight, the default, \
             ArcGIS's), triweight, Epanechnikov, triangular or uniform; or the points in each cell counted (count; \
             with a field, their sum). Without a radius ArcGIS's default (Silverman's rule). The points are \
             reprojected to the map; distances in its units (a map in metres for densities per area).",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the points"))
        .input(PortSpec::new("map", Type::Map, "a map on whose grid the density is made"))
        .output(PortSpec::new("map", Type::Map, "the density (or count) of every cell"))
        .param(
            PortSpec::optional(
                "kernel",
                Type::Text,
                "quartic (default), triweight, epanechnikov, triangular, uniform or count",
            )
            .choices(["quartic", "triweight", "epanechnikov", "triangular", "uniform", "count"]),
        )
        .param(PortSpec::optional("radius", Type::Number, "the kernel's radius in map units (Silverman's rule)"))
        .param(PortSpec::optional("field", Type::Text, "the attribute weighting each point (none: 1)"))
        .param(
            PortSpec::optional("units", Type::Text, "per square map unit (default), per hectare or per km²")
                .choices(["map", "hectares", "km2"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let m = args.map("map")?;
        let g = &m.grid;
        let v = on_grid(vector_of(args, "vector")?, g)?;
        let points = points_of(&v);
        let weights: Vec<f64> = match args.opt_param::<String>("field")?.filter(|f| !f.is_empty()) {
            Some(f) => v.column(&f).ok_or_else(|| err(format!("the points have no attribute {f:?}")))?.numbers(),
            None => vec![1.0; v.len()],
        };
        let kernel: String = args.param("kernel", "quartic".to_string())?;
        let (w, h) = (g.width, g.height);
        let mut out = vec![0.0; w * h];
        if kernel == "count" {
            for &(i, p) in &points {
                let (r, c) = cell_of(g, p);
                if r >= 0.0 && c >= 0.0 && (r as usize) < h && (c as usize) < w && !weights[i].is_nan() {
                    out[r as usize * w + c as usize] += weights[i];
                }
            }
            return Ok(outputs([("map", Value::Map(Map::new(g.clone(), out)))]));
        }
        let shape: fn(f64) -> f64 = match kernel.as_str() {
            "quartic" => |u| 3.0 * (1.0 - u * u).powi(2),
            "triweight" => |u| 4.0 * (1.0 - u * u).powi(3),
            "epanechnikov" => |u| 2.0 * (1.0 - u * u),
            "triangular" => |u| 3.0 * (1.0 - u),
            "uniform" => |_| 1.0,
            k => {
                return Err(err(format!(
                    "kernel {k:?}: quartic, triweight, epanechnikov, triangular, uniform or count"
                )));
            }
        };
        let radius: f64 = match args.opt_param("radius")? {
            Some(r) => r,
            None if points.len() > 1 => default_radius(&points),
            None => return Err(err("one point: give the radius".into())),
        };
        if radius.is_nan() || radius <= 0.0 {
            return Err(err("the radius must be above 0".into()));
        }
        let per = match args.param("units", "map".to_string())?.as_str() {
            "map" => 1.0,
            "hectares" => 10_000.0,
            "km2" => 1_000_000.0,
            u => return Err(err(format!("units {u:?}: map, hectares or km2"))),
        };
        let (rw, rh) = g.resolution();
        let scale = per / (std::f64::consts::PI * radius * radius);
        let (dc, dr) = ((radius / rw).ceil() as isize, (radius / rh).ceil() as isize);
        for &(i, p) in &points {
            let wt = weights[i];
            if wt.is_nan() {
                continue;
            }
            let (r, c) = cell_of(g, p);
            let (r0, c0) = (r.floor() as isize, c.floor() as isize);
            for rr in (r0 - dr).max(0)..=(r0 + dr).min(h as isize - 1) {
                for cc in (c0 - dc).max(0)..=(c0 + dc).min(w as isize - 1) {
                    let q = centre(g, rr as f64, cc as f64);
                    let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
                    if d < radius {
                        out[rr as usize * w + cc as usize] += wt * scale * shape(d / radius);
                    }
                }
            }
        }
        Ok(outputs([("map", Value::Map(Map::new(g.clone(), out)))]))
    }
}

/// The contour lines of a grid's values at a level, by marching squares over the cells' centres: polylines of
/// (row, column) positions, those that close rings closed.
fn contours(data: &[f64], w: usize, h: usize, level: f64) -> Vec<Vec<(f64, f64)>> {
    // an edge between two centres: horizontal (r, c)-(r, c + 1) is 2 (r w + c), vertical (r, c)-(r + 1, c) + 1
    let hor = |r: usize, c: usize| 2 * (r * w + c);
    let ver = |r: usize, c: usize| 2 * (r * w + c) + 1;
    let point = |e: usize| -> (f64, f64) {
        let (k, vertical) = (e / 2, e % 2 == 1);
        let (r, c) = (k / w, k % w);
        let (a, b) = (data[k], if vertical { data[k + w] } else { data[k + 1] });
        let t = (level - a) / (b - a);
        if vertical { (r as f64 + t, c as f64) } else { (r as f64, c as f64 + t) }
    };
    let mut segments: Vec<[usize; 2]> = Vec::new();
    for r in 0..h.saturating_sub(1) {
        for c in 0..w.saturating_sub(1) {
            let (tl, tr, br, bl) =
                (data[r * w + c], data[r * w + c + 1], data[(r + 1) * w + c + 1], data[(r + 1) * w + c]);
            if tl.is_nan() || tr.is_nan() || br.is_nan() || bl.is_nan() {
                continue;
            }
            let above = |v: f64| usize::from(v >= level);
            let case = above(tl) << 3 | above(tr) << 2 | above(br) << 1 | above(bl);
            let (top, right, bottom, left) = (hor(r, c), ver(r, c + 1), hor(r + 1, c), ver(r, c));
            let centre_above = (tl + tr + br + bl) / 4.0 >= level;
            let pairs: &[[usize; 2]] = match case {
                0 | 15 => &[],
                1 | 14 => &[[left, bottom]],
                2 | 13 => &[[bottom, right]],
                3 | 12 => &[[left, right]],
                4 | 11 => &[[top, right]],
                6 | 9 => &[[top, bottom]],
                7 | 8 => &[[left, top]],
                // saddles: the centre's side decides which corners join
                5 if centre_above => &[[left, top], [bottom, right]],
                5 => &[[left, bottom], [top, right]],
                10 if centre_above => &[[left, bottom], [top, right]],
                _ => &[[left, top], [bottom, right]],
            };
            segments.extend_from_slice(pairs);
        }
    }
    let mut at: HashMap<usize, Vec<usize>> = HashMap::new();
    for (s, pair) in segments.iter().enumerate() {
        for &e in pair {
            at.entry(e).or_default().push(s);
        }
    }
    let mut used = vec![false; segments.len()];
    let mut lines = Vec::new();
    // open lines from their ends first, then the rings
    let mut starts: Vec<(usize, usize)> = Vec::new();
    for (s, pair) in segments.iter().enumerate() {
        for &e in pair {
            if at[&e].len() == 1 {
                starts.push((s, e));
            }
        }
    }
    starts.extend((0..segments.len()).map(|s| (s, segments[s][0])));
    for (s0, e0) in starts {
        if used[s0] {
            continue;
        }
        let mut line = vec![point(e0)];
        let (mut s, mut e) = (s0, e0);
        loop {
            used[s] = true;
            let next_e = if segments[s][0] == e { segments[s][1] } else { segments[s][0] };
            line.push(point(next_e));
            match at[&next_e].iter().find(|&&x| !used[x]) {
                Some(&x) => {
                    s = x;
                    e = next_e;
                }
                None => break,
            }
        }
        lines.push(line);
    }
    lines
}

/// `raster.contour`.
struct Contour;

impl Operator for Contour {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.contour",
            "The contour lines of a map, as gdal_contour: lines through the places of equal value, at every \
             multiple of an interval from a base (or at the levels given), traced by marching squares between the \
             cells' centres (linear between two), each line's level as an attribute; the squares of a cell \
             without a value left out.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Map, "the map (an elevation, say)"))
        .output(PortSpec::new("vector", Type::Vector, "the lines, each with its level"))
        .output(PortSpec::new("table", Type::Table, "the feature (1..) and its level"))
        .param(PortSpec::optional("interval", Type::Number, "the distance between levels (e.g. 10 m)"))
        .param(PortSpec::optional("base", Type::Number, "a level the others are multiples of the interval from (0)"))
        .param(PortSpec::optional("levels", Type::Any, "the levels, in place of an interval"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: &str| Error::msg(format!("node {node:?}: {m}"));
        let m = args.map("map")?;
        let (sorted, _) = sorted_values(&m.data);
        let (Some(&lo), Some(&hi)) = (sorted.first(), sorted.last()) else { return Err(err("the map has no value")) };
        let levels: Vec<f64> = match args.opt_param::<Vec<f64>>("levels")? {
            Some(l) => l,
            None => {
                let interval: f64 =
                    args.opt_param("interval")?.ok_or_else(|| err("give the interval or the levels"))?;
                if interval.is_nan() || interval <= 0.0 {
                    return Err(err("the interval must be above 0"));
                }
                let base: f64 = args.param("base", 0.0)?;
                let (k0, k1) = (((lo - base) / interval).ceil() as i64, ((hi - base) / interval).floor() as i64);
                if k1 - k0 > 10_000 {
                    return Err(err("more than 10000 levels: a larger interval"));
                }
                (k0..=k1).map(|k| base + k as f64 * interval).collect()
            }
        };
        let g = &m.grid;
        let mut v = Vector { crs: crate::prepare::crs_of(g), ..Default::default() };
        let mut level = Vec::new();
        for l in levels.into_iter().filter(|l| *l > lo && *l < hi) {
            for line in contours(&m.data, g.width, g.height, l) {
                v.geometries.push(Geometry::LineString(line.into_iter().map(|(r, c)| centre(g, r, c)).collect()));
                level.push(l);
            }
        }
        v.attributes = vec![("level".into(), Column::Numbers(level))];
        let t = attribute_table(&v);
        Ok(outputs([("vector", Value::Vector(Arc::new(v))), ("table", Value::Table(t))]))
    }
}

/// `raster.viewshed`.
struct Viewshed;

impl Operator for Viewshed {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.viewshed",
            "What can be seen from observers over an elevation map, as gdal_viewshed or GRASS's r.viewshed: a cell \
             is seen when the line of sight from an observer's eye (the ground plus its height) to the cell (plus \
             the target's height) passes above every cell between, traced along rays to the edge of each \
             observer's reach (Franklin and Ray's R2); with `curvature`, the earth's curvature and refraction \
             lowering the far cells (GDAL's coefficient, 0.85714). The map counts the observers seeing each cell \
             (one observer: 1 seen, 0 not).",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Map, "the elevation (in the units of the map's coordinates)"))
        .input(PortSpec::optional("points", Type::Vector, "the observers (or x and y)"))
        .output(PortSpec::new("map", Type::Map, "how many observers see every cell"))
        .param(PortSpec::optional("x", Type::Number, "the observer's x, without points"))
        .param(PortSpec::optional("y", Type::Number, "the observer's y, without points"))
        .param(PortSpec::optional("observer_height", Type::Number, "the eye above the ground (2, as GDAL's)"))
        .param(PortSpec::optional("target_height", Type::Number, "what is seen above the ground (0)"))
        .param(PortSpec::optional("radius", Type::Number, "how far an observer sees, in map units (the whole map)"))
        .param(PortSpec::optional("curvature", Type::Bool, "the earth's curvature and refraction (false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: &str| Error::msg(format!("node {node:?}: {m}"));
        let m = args.map("map")?;
        let g = &m.grid;
        let observers: Vec<Xy> = match args.get("points") {
            Some(_) => points_of(&*on_grid(vector_of(args, "points")?, g)?).into_iter().map(|p| p.1).collect(),
            None => match (args.opt_param::<f64>("x")?, args.opt_param::<f64>("y")?) {
                (Some(x), Some(y)) => vec![[x, y]],
                _ => return Err(err("give the observers: points, or x and y")),
            },
        };
        let eye: f64 = args.param("observer_height", 2.0)?;
        let target: f64 = args.param("target_height", 0.0)?;
        let (rw, rh) = g.resolution();
        let reach: f64 = args.param("radius", f64::INFINITY)?;
        let curvature: bool = args.param("curvature", false)?;
        let drop = |d: f64| if curvature { 0.85714 * d * d / (2.0 * 6_371_008.8) } else { 0.0 };
        let (w, h) = (g.width as isize, g.height as isize);
        let mut count = vec![0.0; m.data.len()];
        for o in observers {
            let (r, c) = cell_of(g, o);
            let (or, oc) = (r.floor() as isize, c.floor() as isize);
            if or < 0 || oc < 0 || or >= h || oc >= w || m.data[(or * w + oc) as usize].is_nan() {
                continue;
            }
            let z0 = m.data[(or * w + oc) as usize] + eye;
            let (kr, kc) = ((reach / rh).ceil().min(h as f64) as isize, (reach / rw).ceil().min(w as f64) as isize);
            let (r0, r1, c0, c1) = ((or - kr).max(0), (or + kr).min(h - 1), (oc - kc).max(0), (oc + kc).min(w - 1));
            let mut seen = vec![false; m.data.len()];
            let mut visited = vec![false; m.data.len()];
            seen[(or * w + oc) as usize] = true;
            visited[(or * w + oc) as usize] = true;
            // the rays to every cell of the edge of the observer's square
            let mut ends: Vec<(isize, isize)> = Vec::new();
            for cc in c0..=c1 {
                ends.push((r0, cc));
                ends.push((r1, cc));
            }
            for rr in r0..=r1 {
                ends.push((rr, c0));
                ends.push((rr, c1));
            }
            for (er, ec) in ends {
                let (dr, dc) = (er - or, ec - oc);
                let steps = dr.abs().max(dc.abs());
                let mut steepest = f64::NEG_INFINITY;
                for k in 1..=steps {
                    let rr = or + (dr as f64 * k as f64 / steps as f64).round() as isize;
                    let cc = oc + (dc as f64 * k as f64 / steps as f64).round() as isize;
                    let i = (rr * w + cc) as usize;
                    let z = m.data[i];
                    if z.is_nan() {
                        continue;
                    }
                    let d = (((rr - or) as f64 * rh).powi(2) + ((cc - oc) as f64 * rw).powi(2)).sqrt();
                    if d > reach {
                        break;
                    }
                    visited[i] = true;
                    let z = z - drop(d);
                    if (z + target - z0) / d >= steepest {
                        seen[i] = true;
                    }
                    steepest = steepest.max((z - z0) / d);
                }
            }
            // the cells no ray passed through: their own line of sight
            for rr in r0..=r1 {
                for cc in c0..=c1 {
                    let i = (rr * w + cc) as usize;
                    let d = (((rr - or) as f64 * rh).powi(2) + ((cc - oc) as f64 * rw).powi(2)).sqrt();
                    if visited[i] || m.data[i].is_nan() || d > reach {
                        continue;
                    }
                    let (dr, dc) = (rr - or, cc - oc);
                    let steps = dr.abs().max(dc.abs());
                    let mut steepest = f64::NEG_INFINITY;
                    for k in 1..steps {
                        let (r, c) = (
                            or + (dr as f64 * k as f64 / steps as f64).round() as isize,
                            oc + (dc as f64 * k as f64 / steps as f64).round() as isize,
                        );
                        let z = m.data[(r * w + c) as usize];
                        if !z.is_nan() {
                            let e = (((r - or) as f64 * rh).powi(2) + ((c - oc) as f64 * rw).powi(2)).sqrt();
                            steepest = steepest.max((z - drop(e) - z0) / e);
                        }
                    }
                    seen[i] = (m.data[i] - drop(d) + target - z0) / d >= steepest;
                }
            }
            for (n, s) in count.iter_mut().zip(&seen) {
                if *s {
                    *n += 1.0;
                }
            }
        }
        let out = count.iter().zip(m.data.iter()).map(|(n, z)| if z.is_nan() { f64::NAN } else { *n }).collect();
        Ok(outputs([("map", Value::Map(Map::new(g.clone(), out)))]))
    }
}

/// Whether a grid's coordinates are longitudes and latitudes (by its coordinate system, when known).
fn geographic(g: &Grid) -> bool {
    g.geo.is_geographic().unwrap_or_else(|| {
        crate::prepare::crs_of(g).is_some_and(|c| terraflux_gdal::is_geographic(&c).unwrap_or(false))
    })
}

/// The area (m²) on the WGS84 ellipsoid between two latitudes (degrees) over a span of longitude (degrees).
fn band_area(lat1: f64, lat2: f64, lon_span: f64) -> f64 {
    let a: f64 = 6_378_137.0;
    let f = 1.0 / 298.257_223_563;
    let b = a * (1.0 - f);
    let e = (1.0 - (b / a).powi(2)).sqrt();
    let q = |lat: f64| {
        let s = lat.to_radians().sin();
        s / (1.0 - e * e * s * s) + (1.0 / (2.0 * e)) * ((1.0 + e * s) / (1.0 - e * s)).ln()
    };
    (lon_span.to_radians() * b * b / 2.0 * (q(lat2) - q(lat1))).abs()
}

/// `raster.cell_area`.
struct CellArea;

impl Operator for CellArea {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.cell_area",
            "The true area of every cell of a map's grid, as terra's cellSize: on a grid of longitudes and \
             latitudes the area of the cell on the WGS84 ellipsoid (exact, by its latitudes: smaller towards the \
             poles), on a projected grid its width times its height (in metres by the grid's unit). In square \
             metres, hectares or km²: a map of classes times it is each cell's area of the class.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Map, "a map on whose grid the areas are made"))
        .output(PortSpec::new("map", Type::Map, "the area of every cell"))
        .param(
            PortSpec::optional("units", Type::Text, "m2 (default), hectares or km2").choices(["m2", "hectares", "km2"]),
        )
        .param(
            PortSpec::optional("geographic", Type::Text, "auto (by the coordinate system, default), yes or no")
                .choices(["auto", "yes", "no"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let m = args.map("map")?;
        let g = &m.grid;
        let per = match args.param("units", "m2".to_string())?.as_str() {
            "m2" => 1.0,
            "hectares" => 10_000.0,
            "km2" => 1_000_000.0,
            u => return Err(Error::msg(format!("node {node:?}: units {u:?}: m2, hectares or km2"))),
        };
        let lonlat = match args.param("geographic", "auto".to_string())?.as_str() {
            "auto" => geographic(g),
            "yes" | "true" => true,
            "no" | "false" => false,
            x => return Err(Error::msg(format!("node {node:?}: geographic {x:?}: auto, yes or no"))),
        };
        let (rw, rh) = g.resolution();
        let rows: Vec<f64> = if lonlat {
            let t = g.transform;
            (0..g.height).map(|r| band_area(t[3] + r as f64 * t[5], t[3] + (r + 1) as f64 * t[5], rw) / per).collect()
        } else {
            let unit = g.geo.linear_unit();
            vec![rw * rh * unit * unit / per; g.height]
        };
        let out = (0..g.cells()).map(|i| rows[i / g.width.max(1)]).collect();
        Ok(outputs([("map", Value::Map(Map::new(g.clone(), out)))]))
    }
}

/// `raster.classify`.
struct Classify;

impl Operator for Classify {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.classify",
            "A map's values in classes 1.. by breaks, as a GIS's graduated symbology or classInt: quantiles (each \
             class a share of the cells), equal intervals, Jenks's natural breaks (of at most `sample` values \
             spread evenly over the sorted cells), standard deviations (breaks at the mean and every `sd_width` \
             standard deviations from it, as QGIS's), or the breaks given. A class holds the values above its \
             lower break up to its upper one (the first its lower too), as R's cut.",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Map, "the values"))
        .output(PortSpec::new("map", Type::Map, "the class of every cell"))
        .output(PortSpec::new("table", Type::Table, "a row per class: class, from, to, cells, share"))
        .param(
            PortSpec::optional("method", Type::Text, "quantile (default), equal, jenks, sd or breaks")
                .choices(["quantile", "equal", "jenks", "sd", "breaks"]),
        )
        .param(PortSpec::optional("n", Type::Integer, "the classes (5)"))
        .param(PortSpec::optional("breaks", Type::Any, "breaks: the values between classes"))
        .param(PortSpec::optional("sd_width", Type::Number, "sd: the classes' width in standard deviations (1)"))
        .param(PortSpec::optional("sample", Type::Integer, "jenks: at most so many values (10000)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let m = args.map("map")?;
        let (sorted, _) = sorted_values(&m.data);
        let (Some(&lo), Some(&hi)) = (sorted.first(), sorted.last()) else {
            return Err(err("the map has no value".into()));
        };
        let n: usize = args.param("n", 5)?;
        let method: String = args.param("method", "quantile".to_string())?;
        if n == 0 && method != "breaks" && method != "sd" {
            return Err(err("n: one class at least".into()));
        }
        // the breaks between classes
        let mut inner: Vec<f64> = match method.as_str() {
            "quantile" => (1..n).map(|k| percentile(&sorted, 100.0 * k as f64 / n as f64)).collect(),
            "equal" => (1..n).map(|k| lo + (hi - lo) * k as f64 / n as f64).collect(),
            "jenks" => {
                let most: usize = args.param("sample", 10_000)?;
                let v: Vec<f64> = if sorted.len() > most.max(2) {
                    (0..most).map(|i| sorted[i * (sorted.len() - 1) / (most - 1)]).collect()
                } else {
                    sorted.clone()
                };
                let distinct = 1 + v.windows(2).filter(|x| x[0] != x[1]).count();
                let b = ffi::nlm_jenks_breaks(&v, n.min(distinct) as i32).map_err(|e| err(e.to_string()))?;
                b[1..b.len().saturating_sub(1)].to_vec()
            }
            "sd" => {
                let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
                let sd = (sorted.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / sorted.len() as f64).sqrt();
                let width = sd * args.param("sd_width", 1.0)?;
                if width.is_nan() || width <= 0.0 {
                    vec![]
                } else {
                    let (k0, k1) = (((lo - mean) / width).floor() as i64, ((hi - mean) / width).ceil() as i64);
                    (k0..=k1).map(|k| mean + k as f64 * width).collect()
                }
            }
            "breaks" => {
                let b: Vec<f64> = args.opt_param("breaks")?.ok_or_else(|| err("give the breaks".into()))?;
                b
            }
            x => return Err(err(format!("method {x:?}: quantile, equal, jenks, sd or breaks"))),
        };
        inner.sort_by(f64::total_cmp);
        inner.dedup();
        if method != "breaks" {
            inner.retain(|b| *b > lo && *b < hi);
        }
        let classes = inner.len() + 1;
        let class_of =
            |v: f64| -> f64 { if v.is_nan() { f64::NAN } else { (1 + inner.partition_point(|b| *b < v)) as f64 } };
        let out: Vec<f64> = m.data.iter().map(|&v| class_of(v)).collect();
        let mut cells = vec![0usize; classes];
        for v in out.iter().filter(|v| !v.is_nan()) {
            cells[*v as usize - 1] += 1;
        }
        let total = sorted.len() as f64;
        let mut t = Table::new(["class", "from", "to", "cells", "share"].map(String::from).to_vec());
        for k in 0..classes {
            let from = if k == 0 { lo.min(inner.first().copied().unwrap_or(lo)) } else { inner[k - 1] };
            let to = if k + 1 == classes { hi.max(inner.last().copied().unwrap_or(hi)) } else { inner[k] };
            t.rows.push(vec![(k + 1) as f64, from, to, cells[k] as f64, cells[k] as f64 / total]);
        }
        Ok(outputs([("map", Value::Map(Map::new(m.grid.clone(), out))), ("table", Value::Table(t))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(w: usize, h: usize, v: &[f64]) -> Value {
        Value::Map(Map::new(Arc::new(Grid::new(w, h)), v.to_vec()))
    }

    fn run(op: &dyn Operator, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
        let params = params.as_object().cloned().unwrap_or_default();
        let args =
            Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
        op.run(&mut Ctx::default(), &args).unwrap()
    }

    fn data(o: &Outputs) -> Vec<f64> {
        match &o["map"] {
            Value::Map(m) => m.data.to_vec(),
            _ => panic!("no map"),
        }
    }

    fn points(p: &[Xy]) -> Value {
        Value::Vector(Arc::new(Vector {
            geometries: p.iter().map(|x| Geometry::Point(*x)).collect(),
            ..Default::default()
        }))
    }

    #[test]
    fn points_are_spread_and_counted() {
        // a point at the centre of cell (20, 20) of a 41 x 41 grid (rows down: y = -row)
        let p = points(&[[20.5, -20.5]]);
        let o = run(
            &Density,
            vec![("vector", p.clone()), ("map", map(41, 41, &[0.0; 41 * 41]))],
            serde_json::json!({ "radius": 10 }),
        );
        let d = data(&o);
        let peak = 3.0 / (std::f64::consts::PI * 100.0);
        assert!((d[20 * 41 + 20] - peak).abs() < 1e-12);
        let total: f64 = d.iter().sum();
        assert!((total - 1.0).abs() < 0.01, "{total}");
        assert_eq!(d[0], 0.0);
        let two = points(&[[1.2, -0.3], [1.7, -0.9], [0.5, -2.5]]);
        let o = run(
            &Density,
            vec![("vector", two), ("map", map(3, 3, &[0.0; 9]))],
            serde_json::json!({ "kernel": "count" }),
        );
        assert_eq!(data(&o), [0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn contours_are_traced() {
        // values growing to the east: a line per level, north to south through the cells' centres
        let v: Vec<f64> = (0..15).map(|i| (i % 5) as f64).collect();
        let o = run(&Contour, vec![("map", map(5, 3, &v))], serde_json::json!({ "interval": 1 }));
        let Value::Vector(lines) = &o["vector"] else { panic!() };
        assert_eq!(lines.column("level").unwrap().numbers(), [1.0, 2.0, 3.0]);
        let Geometry::LineString(l) = &lines.geometries[0] else { panic!() };
        assert_eq!(l.len(), 3);
        assert!(l.iter().all(|p| (p[0] - 1.5).abs() < 1e-12));
        // a hill: closed rings
        #[rustfmt::skip]
        let hill = [
            0.0, 0.0, 0.0, 0.0,
            0.0, 5.0, 5.0, 0.0,
            0.0, 5.0, 5.0, 0.0,
            0.0, 0.0, 0.0, 0.0,
        ];
        let o = run(&Contour, vec![("map", map(4, 4, &hill))], serde_json::json!({ "levels": [2.5] }));
        let Value::Vector(lines) = &o["vector"] else { panic!() };
        assert_eq!(lines.len(), 1);
        let Geometry::LineString(l) = &lines.geometries[0] else { panic!() };
        assert_eq!(l.first(), l.last());
        assert_eq!(l.len(), 9);
    }

    #[test]
    fn a_wall_hides_what_is_behind_it() {
        let dem = map(7, 1, &[0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0]);
        let o = run(&Viewshed, vec![("map", dem)], serde_json::json!({ "x": 0.5, "y": -0.5, "observer_height": 1 }));
        assert_eq!(data(&o), [1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
        let flat = map(9, 9, &[0.0; 81]);
        let o = run(
            &Viewshed,
            vec![("map", flat.clone()), ("points", points(&[[4.5, -4.5], [0.5, -0.5]]))],
            serde_json::json!({}),
        );
        assert!(data(&o).iter().all(|v| *v == 2.0));
        let o = run(&Viewshed, vec![("map", flat)], serde_json::json!({ "x": 4.5, "y": -4.5, "radius": 2 }));
        let d = data(&o);
        assert_eq!(d[4 * 9 + 6], 1.0);
        assert_eq!(d[4 * 9 + 8], 0.0);
    }

    #[test]
    fn cells_have_their_true_area() {
        let mut g = Grid::new(2, 2);
        g.transform = [0.0, 1.0, 0.0, 1.0, 0.0, -1.0];
        g.geo.wkt = Some("GEOGCS[\"WGS 84\"]".into());
        let m = Value::Map(Map::new(Arc::new(g), vec![0.0; 4]));
        let a = data(&run(&CellArea, vec![("map", m)], serde_json::json!({ "units": "km2" })));
        // a degree by a degree at the equator, the next row south of it the same
        assert!((a[0] - 12308.4639).abs() < 1e-3, "{a:?}");
        assert!((a[0] - a[2]).abs() < 1e-6);
        let a = data(&run(&CellArea, vec![("map", map(2, 1, &[0.0; 2]))], serde_json::json!({})));
        assert_eq!(a, [1.0, 1.0]);
    }

    #[test]
    fn values_are_classified() {
        let v: Vec<f64> = (1..=10).map(f64::from).collect();
        let at = |p: serde_json::Value| {
            let o = run(&Classify, vec![("map", map(10, 1, &v))], p);
            let Value::Table(t) = &o["table"] else { panic!() };
            (data(&o), t.clone())
        };
        let (c, t) = at(serde_json::json!({ "method": "equal", "n": 2 }));
        assert_eq!(c, [1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0]);
        assert_eq!(t.rows[0], [1.0, 1.0, 5.5, 5.0, 0.5]);
        let (c, _) = at(serde_json::json!({ "method": "quantile", "n": 2 }));
        assert_eq!(c[4..6], [1.0, 2.0]);
        let (c, t) = at(serde_json::json!({ "method": "sd" }));
        // breaks at 5.5 - 2.87, 5.5 and 5.5 + 2.87
        assert_eq!(c, [1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0]);
        assert_eq!(t.rows.len(), 4);
        let (c, _) = at(serde_json::json!({ "method": "breaks", "breaks": [3, 8] }));
        assert_eq!(c, [1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0]);
        let clusters = map(6, 1, &[1.0, 2.0, 3.0, 10.0, 11.0, 12.0]);
        let o = run(&Classify, vec![("map", clusters)], serde_json::json!({ "method": "jenks", "n": 2 }));
        assert_eq!(data(&o), [1.0, 1.0, 1.0, 2.0, 2.0, 2.0]);
    }
}
