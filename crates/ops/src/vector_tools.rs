//! The everyday tools of vector features: a map of classes into polygons (`vector.polygonize`, as
//! gdal_polygonize), features kept or given a column by an expression over their attributes (`vector.filter`,
//! `vector.calc`), their areas, perimeters, lengths and centroids (`vector.geometry`), two layers of polygons
//! overlaid (`vector.overlay`: intersection, union, difference, as geopandas' overlay), the features of one layer
//! joined to those of another where they meet (`vector.spatial_join`: the attributes of the polygon holding a
//! point, the points in a polygon counted), features in another coordinate system (`vector.reproject`), points from
//! a table's columns (`vector.from_table`), and points drawn inside polygons (`vector.points`).

use std::collections::HashMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

use crate::algebra::{Script, parse_script};
use crate::measures::{MEASURES, measure, polygon_area, ring_area};
use crate::raster_neighbours::patches;
use crate::signatures::NumpyRng;
use crate::vector::{attribute_table, vector_of, with_columns};
use crate::vector_algebra;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Polygonize));
    r.add(Arc::new(Filter));
    r.add(Arc::new(Calc));
    r.add(Arc::new(Measure));
    r.add(Arc::new(Overlay));
    r.add(Arc::new(SpatialJoin));
    r.add(Arc::new(Reproject));
    r.add(Arc::new(FromTable));
    r.add(Arc::new(Points));
}

fn out(v: Vector) -> Outputs {
    let t = attribute_table(&v);
    outputs([("vector", Value::Vector(Arc::new(v))), ("table", Value::Table(t))])
}

/// Whether a point lies inside a ring (crossings of a ray to its right).
fn in_ring(p: Xy, r: &[Xy]) -> bool {
    let n = r.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (r[i], r[(i + n - 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
    }
    inside
}

/// Whether a point lies inside a polygon (in its outer ring, in none of its holes).
fn in_polygon(p: Xy, poly: &[Vec<Xy>]) -> bool {
    !poly.is_empty() && in_ring(p, &poly[0]) && !poly[1..].iter().any(|h| in_ring(p, h))
}

/// Polygons, each its rings (the outer first).
type Polygons = Vec<Vec<Vec<Xy>>>;

/// An edge between cells: its start (a corner of the grid) and direction (rows, columns).
type Edge = (usize, (isize, isize));

/// The polygons of the patches of a map: per patch (in the order of its first cell, row by row), its value, its
/// cells and its rings in columns and rows of the grid's corners, outer rings first of each polygon.
pub(crate) fn polygons_of(data: &[f64], w: usize, h: usize, eight: bool) -> Vec<(f64, usize, Polygons)> {
    const FOUR: [(isize, isize); 4] = [(-1, 0), (0, -1), (0, 1), (1, 0)];
    const EIGHT: [(isize, isize); 8] = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)];
    let (id, values, sizes) = patches(data, w, h, if eight { &EIGHT } else { &FOUR });
    let vw = w + 1;
    // the edges between a patch and what is beside it, the patch on their left (rows growing down): start, direction
    let (mut edges, mut patch_of): (Vec<Edge>, Vec<usize>) = (Vec::new(), Vec::new());
    let other = |cell: usize, dr: isize, dc: isize| -> bool {
        let (r, c) = ((cell / w) as isize + dr, (cell % w) as isize + dc);
        r < 0 || c < 0 || r >= h as isize || c >= w as isize || id[r as usize * w + c as usize] != id[cell]
    };
    for (cell, p) in id.iter().enumerate() {
        let Some(p) = *p else { continue };
        let (r, c) = (cell / w, cell % w);
        if other(cell, -1, 0) {
            edges.push((r * vw + c + 1, (0, -1)));
            patch_of.push(p);
        }
        if other(cell, 1, 0) {
            edges.push(((r + 1) * vw + c, (0, 1)));
            patch_of.push(p);
        }
        if other(cell, 0, -1) {
            edges.push((r * vw + c, (1, 0)));
            patch_of.push(p);
        }
        if other(cell, 0, 1) {
            edges.push(((r + 1) * vw + c + 1, (-1, 0)));
            patch_of.push(p);
        }
    }
    let mut leaving: HashMap<(usize, usize), [usize; 2]> = HashMap::with_capacity(edges.len());
    for (e, &(v, _)) in edges.iter().enumerate() {
        leaving.entry((v, patch_of[e])).and_modify(|x| x[1] = e).or_insert([e, usize::MAX]);
    }
    let end = |e: usize| {
        let (v, (dr, dc)) = edges[e];
        ((v / vw) as isize + dr) as usize * vw + ((v % vw) as isize + dc) as usize
    };
    // a patch's cells touching at a corner: around the cell (four neighbours, a left turn) or across (eight, right)
    let turn = |d: (isize, isize)| if eight { (d.1, -d.0) } else { (-d.1, d.0) };
    let mut rings: Vec<Vec<Vec<Xy>>> = vec![Vec::new(); values.len()];
    let mut used = vec![false; edges.len()];
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        let p = patch_of[start];
        let mut walk: Vec<(usize, (isize, isize))> = Vec::new();
        let mut e = start;
        loop {
            used[e] = true;
            walk.push(edges[e]);
            let [a, b] = leaving[&(end(e), p)];
            let next = if b == usize::MAX || edges[a].1 == turn(edges[e].1) { a } else { b };
            if next == start {
                break;
            }
            e = next;
        }
        // the corners where the direction changes
        let n = walk.len();
        let ring: Vec<Xy> = (0..n)
            .filter(|&i| walk[i].1 != walk[(i + n - 1) % n].1)
            .map(|i| [(walk[i].0 % vw) as f64, (walk[i].0 / vw) as f64])
            .collect();
        rings[p].push(ring);
    }
    rings
        .into_iter()
        .enumerate()
        .map(|(p, rs)| {
            // outer rings turn clockwise in columns and rows (negative), holes the other way
            let (outer, holes): (Vec<Vec<Xy>>, Vec<Vec<Xy>>) = rs.into_iter().partition(|r| ring_area(r) < 0.0);
            let mut polys: Vec<Vec<Vec<Xy>>> = outer.into_iter().map(|r| vec![r]).collect();
            for hole in holes {
                let at = [(hole[0][0] + hole[1][0]) / 2.0, (hole[0][1] + hole[1][1]) / 2.0];
                let k = if polys.len() == 1 { 0 } else { polys.iter().position(|q| in_ring(at, &q[0])).unwrap_or(0) };
                polys[k].push(hole);
            }
            (values[p], sizes[p], polys)
        })
        .collect()
}

/// `vector.polygonize`.
struct Polygonize;

impl Operator for Polygonize {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.polygonize",
            "A map of classes as polygons, as gdal_polygonize (GDALPolygonize): a polygon (with its holes) per \
             patch, the connected cells of one value, its value, cells and area as attributes, in the order of \
             their first cell; the cells without a value in none. The patches' cells joined by their sides (4) or \
             also by their corners (8).",
            Access::None,
        )
        .input(PortSpec::new("map", Type::Map, "the classes (or any values: a polygon per patch of one)"))
        .output(PortSpec::new("vector", Type::Vector, "the polygons: value, cells, area"))
        .output(PortSpec::new("table", Type::Table, "the feature (1..) and its value, cells and area"))
        .param(PortSpec::optional("neighbours", Type::Integer, "4 (default, as GDAL's) or 8"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        let eight = match args.param("neighbours", 4)? {
            4 => false,
            8 => true,
            n => return Err(Error::msg(format!("node {:?}: neighbours {n}: 4 or 8", args.node))),
        };
        let g = &m.grid;
        let t = g.transform;
        let at = |p: Xy| [t[0] + p[0] * t[1] + p[1] * t[2], t[3] + p[0] * t[4] + p[1] * t[5]];
        let (rw, rh) = g.resolution();
        let mut v = Vector { crs: crate::prepare::crs_of(g), ..Default::default() };
        let (mut value, mut cells, mut area) = (Vec::new(), Vec::new(), Vec::new());
        for (x, n, polys) in polygons_of(&m.data, g.width, g.height, eight) {
            let mut polys: Vec<Vec<Vec<Xy>>> = polys
                .into_iter()
                .map(|p| {
                    p.into_iter()
                        .map(|r| {
                            let mut r: Vec<Xy> = r.into_iter().map(at).collect();
                            r.push(r[0]);
                            r
                        })
                        .collect()
                })
                .collect();
            v.geometries.push(if polys.len() == 1 {
                Geometry::Polygon(polys.remove(0))
            } else {
                Geometry::MultiPolygon(polys)
            });
            value.push(x);
            cells.push(n as f64);
            area.push(n as f64 * rw * rh);
        }
        v.attributes = vec![
            ("value".into(), Column::Numbers(value)),
            ("cells".into(), Column::Numbers(cells)),
            ("area".into(), Column::Numbers(area)),
        ];
        Ok(out(v))
    }
}

/// The script of a node over features (its setting `expression`).
fn script_of(args: &Args) -> Result<Script> {
    let node = args.node;
    let text: String =
        args.opt_param("expression")?.ok_or_else(|| Error::msg(format!("node {node:?}: no expression")))?;
    parse_script(&text).map_err(|x| Error::msg(format!("node {node:?}: the expression: {}", x.0)))
}

/// An error of a node's script, told with the node.
fn of_node(node: &str) -> impl Fn(Error) -> Error + '_ {
    move |x| Error::msg(format!("node {node:?}: {}", x.0))
}

/// `vector.filter`.
struct Filter;

impl Operator for Filter {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.filter",
            "The features where an expression over their attributes is true, as QGIS's Select by Expression or \
             geopandas' query, in map algebra's language (Python's syntax): the attributes by their names (the \
             characters other than letters, digits and _ as _: individual-local-identifier as \
             individual_local_identifier), numbers and texts (area > 10000 and uso == \"forest\"; float(code) \
             reads a text as a number), the geometry's measures (geometry.area, geometry.perimeter, geometry.length, geometry.x, geometry.y), feature (the \
             number 1..), the node's other inputs (numbers and texts) by their names. A script of several lines \
             gives names values (density = pop / geometry.area) and ends with the condition. A feature where it \
             is missing is left out.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .extra(Type::Any)
        .output(PortSpec::new("vector", Type::Vector, "the features kept"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::new(
            "expression",
            Type::Text,
            "what a feature kept is, e.g. geometry.area > 10000 and uso == \"forest\" (a script: its last line)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = vector_of(args, "vector")?;
        let keep = vector_algebra::kept(v, &args.inputs, &script_of(args)?).map_err(of_node(args.node))?;
        Ok(out(v.select(&keep)))
    }
}

/// `vector.calc`.
struct Calc;

impl Operator for Calc {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.calc",
            "Attributes computed for every feature from a script over the others, as QGIS's field calculator, in \
             map algebra's language (Python's syntax): the attributes by their names (the characters other than \
             letters, digits and _ as _: individual-local-identifier as individual_local_identifier), numbers and \
             texts (\"high\" if pop > 100 else \"low\"; uso == \"forest\"; float(code) reads a text as a number), the geometry's \
             measures (geometry.area, geometry.perimeter, geometry.length, geometry.x, geometry.y), feature (the \
             number 1..), x.sum() and the other methods over all the features, the node's other inputs (numbers \
             and texts) by their names. One statement a line (# a comment): name = value, name[where] = value, and \
             out name = value (or out name) for each attribute given; without out, the last value is the \
             attribute `column`. An attribute of the same name is replaced, else it is added.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .extra(Type::Any)
        .output(PortSpec::new("vector", Type::Vector, "the features with the attributes"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional("column", Type::Text, "the attribute's name (a script without out)"))
        .param(PortSpec::new(
            "expression",
            Type::Text,
            "its value, e.g. geometry.area / 10000; or a script: density = pop / geometry.area; out dense = density \
             > 100",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = vector_of(args, "vector")?;
        let column: String = args.param("column", String::new())?;
        let script = script_of(args)?;
        Ok(out(vector_algebra::calculated(v, &args.inputs, &script, &column).map_err(of_node(args.node))?))
    }
}

/// `vector.geometry`.
struct Measure;

impl Operator for Measure {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.geometry",
            "The measures of every feature's geometry as attributes, as QGIS's Add Geometry Attributes: the area \
             and perimeter of polygons (their holes left out of the area, their rings in the perimeter), the length \
             of lines, and the centroid (x, y: of the area, the length or the points), in the features' units; \
             with `geodesic`, the areas in square metres on the WGS84 ellipsoid (GDAL 3.9 or later).",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .output(PortSpec::new("vector", Type::Vector, "the features with area, perimeter, length, x and y"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional("geodesic", Type::Bool, "the areas on the ellipsoid, in m² (false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = vector_of(args, "vector")?;
        let mut columns: Vec<(String, Vec<f64>)> =
            MEASURES.iter().map(|m| (m.to_string(), measure(v, m).expect("a measure"))).collect();
        if args.param("geodesic", false)? {
            columns[0].1 =
                terraflux_gdal::geodesic_areas(v).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        }
        let (v, _) = with_columns(v, columns);
        Ok(out(v))
    }
}

/// Features in the coordinate system of others (reprojected by GDAL when both are known and differ).
fn like(v: &Arc<Vector>, crs: &Option<String>) -> Result<Arc<Vector>> {
    match (&v.crs, crs) {
        (Some(a), Some(b)) if !terraflux_gdal::same_crs(a, b).unwrap_or(true) => {
            Ok(Arc::new(terraflux_gdal::reproject_vector(v, b).map_err(Error::msg)?))
        }
        _ => Ok(v.clone()),
    }
}

/// Whether two bounds meet.
fn meet(a: Option<[f64; 4]>, b: Option<[f64; 4]>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3],
        _ => false,
    }
}

/// The attributes of features made from those of two layers: a's, then b's (a name a has already with a suffix),
/// each feature's from the feature of each it came from (none: no value).
fn joined(a: &Vector, b: &Vector, from: &[(Option<usize>, Option<usize>)], suffix: &str) -> Vec<(String, Column)> {
    let pick = |c: &Column, at: &dyn Fn(usize) -> Option<usize>| match c {
        Column::Numbers(x) => Column::Numbers((0..from.len()).map(|k| at(k).map_or(f64::NAN, |i| x[i])).collect()),
        Column::Texts(x) => {
            Column::Texts((0..from.len()).map(|k| at(k).map_or(String::new(), |i| x[i].clone())).collect())
        }
    };
    let mut out: Vec<(String, Column)> =
        a.attributes.iter().map(|(n, c)| (n.clone(), pick(c, &|k| from[k].0))).collect();
    for (n, c) in &b.attributes {
        let name = if a.attributes.iter().any(|(x, _)| x == n) { format!("{n}{suffix}") } else { n.clone() };
        out.push((name, pick(c, &|k| from[k].1)));
    }
    out
}

/// The polygonal part of a geometry (none when it has no area).
fn polygonal(g: Geometry) -> Option<Geometry> {
    match g {
        Geometry::Polygon(_) => Some(g),
        Geometry::MultiPolygon(ref m) if !m.is_empty() => Some(g),
        _ => None,
    }
}

/// `vector.overlay`.
struct Overlay;

impl Operator for Overlay {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.overlay",
            "Two layers of polygons overlaid, as geopandas' overlay or QGIS's Intersection, Union and Difference \
             (GEOS through GDAL): the intersection (a polygon where a feature of each meets, with the attributes of \
             both), the union (those, and the parts of each layer the other does not cover, with only their own \
             attributes) or the difference (each feature of the first less the second's). The second layer is \
             reprojected to the first's.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the first layer of polygons"))
        .input(PortSpec::new("other", Type::Vector, "the second layer of polygons"))
        .output(PortSpec::new("vector", Type::Vector, "the polygons overlaid"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional("how", Type::Text, "intersection (default), union or difference").choices([
            "intersection",
            "union",
            "difference",
        ]))
        .param(PortSpec::optional("suffix", Type::Text, "after a name of the second layer the first has (_2)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let a = vector_of(args, "vector")?;
        let b = like(vector_of(args, "other")?, &a.crs)?;
        let how: String = args.param("how", "intersection".to_string())?;
        if !matches!(how.as_str(), "intersection" | "union" | "difference") {
            return Err(err(format!("how {how:?}: intersection, union or difference")));
        }
        for (v, which) in [(a.as_ref(), "first"), (b.as_ref(), "second")] {
            if v.geometries.iter().any(|g| !g.is_polygonal() && !matches!(g, Geometry::Empty)) {
                return Err(err(format!("the {which} layer is not polygons")));
            }
        }
        let suffix: String = args.param("suffix", "_2".to_string())?;
        let (ga, gb) = (
            terraflux_gdal::Geos::new(&a.geometries).map_err(err)?,
            terraflux_gdal::Geos::new(&b.geometries).map_err(err)?,
        );
        let (ba, bb): (Vec<_>, Vec<_>) =
            (a.geometries.iter().map(|g| g.bounds()).collect(), b.geometries.iter().map(|g| g.bounds()).collect());
        // the features of b meeting each of a
        let meets: Vec<Vec<usize>> = (0..a.len())
            .map(|i| (0..b.len()).filter(|&j| meet(ba[i], bb[j]) && ga.intersects_other(i, &gb, j)).collect())
            .collect();
        let mut geoms = Vec::new();
        let mut from: Vec<(Option<usize>, Option<usize>)> = Vec::new();
        if how != "difference" {
            for (i, js) in meets.iter().enumerate() {
                for &j in js {
                    let cut = terraflux_gdal::intersection(std::slice::from_ref(&a.geometries[i]), &b.geometries[j])
                        .map_err(err)?;
                    if let Some(g) = cut.into_iter().next().and_then(polygonal) {
                        geoms.push(g);
                        from.push((Some(i), Some(j)));
                    }
                }
            }
        }
        let less = |g: &Geometry, others: Vec<&Geometry>| -> Result<Option<Geometry>> {
            if others.is_empty() {
                return Ok(Some(g.clone()));
            }
            let u = terraflux_gdal::union(&others).map_err(err)?;
            Ok(polygonal(terraflux_gdal::difference(g, &u).map_err(err)?))
        };
        if how != "intersection" {
            for (i, js) in meets.iter().enumerate() {
                if let Some(g) = less(&a.geometries[i], js.iter().map(|&j| &b.geometries[j]).collect())? {
                    geoms.push(g);
                    from.push((Some(i), None));
                }
            }
        }
        if how == "union" {
            for j in 0..b.len() {
                let is: Vec<&Geometry> =
                    (0..a.len()).filter(|i| meets[*i].contains(&j)).map(|i| &a.geometries[i]).collect();
                if let Some(g) = less(&b.geometries[j], is)? {
                    geoms.push(g);
                    from.push((None, Some(j)));
                }
            }
        }
        let attributes = if how == "difference" {
            a.select(&from.iter().filter_map(|f| f.0).collect::<Vec<_>>()).attributes
        } else {
            joined(a, &b, &from, &suffix)
        };
        Ok(out(Vector { geometries: geoms, attributes, crs: a.crs.clone() }))
    }
}

/// `vector.spatial_join`.
struct SpatialJoin;

impl Operator for SpatialJoin {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.spatial_join",
            "The features of a layer joined to those of another where they meet, as geopandas' sjoin or QGIS's \
             Join Attributes by Location (GEOS's intersects through GDAL): each feature takes the attributes of \
             the first feature of the other layer it meets (`attributes`: the polygon holding a point), or the \
             count of those it meets and the sum and mean of their attributes `fields` (`summary`: the points in \
             every polygon counted). The other layer is reprojected to the first's.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features joined to (points, or polygons counted)"))
        .input(PortSpec::new("other", Type::Vector, "the features joined (polygons, or points to count)"))
        .output(PortSpec::new(
            "vector",
            Type::Vector,
            "the features with the other's attributes, or count, sums, means",
        ))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(
            PortSpec::optional("mode", Type::Text, "attributes (default) or summary")
                .choices(["attributes", "summary"]),
        )
        .param(PortSpec::optional("fields", Type::Any, "summary: the other's attributes summed and averaged"))
        .param(PortSpec::optional("suffix", Type::Text, "attributes: after a name the first layer has (_2)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let a = vector_of(args, "vector")?;
        let b = like(vector_of(args, "other")?, &a.crs)?;
        let (ga, gb) = (
            terraflux_gdal::Geos::new(&a.geometries).map_err(err)?,
            terraflux_gdal::Geos::new(&b.geometries).map_err(err)?,
        );
        let bb: Vec<_> = b.geometries.iter().map(|g| g.bounds()).collect();
        let meets: Vec<Vec<usize>> = a
            .geometries
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let bi = g.bounds();
                (0..b.len()).filter(|&j| meet(bi, bb[j]) && ga.intersects_other(i, &gb, j)).collect()
            })
            .collect();
        let mode: String = args.param("mode", "attributes".to_string())?;
        let v = match mode.as_str() {
            "attributes" => {
                let from: Vec<(Option<usize>, Option<usize>)> =
                    meets.iter().enumerate().map(|(i, js)| (Some(i), js.first().copied())).collect();
                let suffix: String = args.param("suffix", "_2".to_string())?;
                Vector {
                    geometries: a.geometries.clone(),
                    attributes: joined(a, &b, &from, &suffix),
                    crs: a.crs.clone(),
                }
            }
            "summary" => {
                let fields: Vec<String> = match args.params.get("fields") {
                    None | Some(serde_json::Value::Null) => Vec::new(),
                    Some(serde_json::Value::String(s)) => {
                        s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
                    }
                    Some(x) => serde_json::from_value(x.clone()).map_err(|e| err(format!("fields: {e}")))?,
                };
                let mut columns =
                    vec![("count".to_string(), meets.iter().map(|js| js.len() as f64).collect::<Vec<f64>>())];
                for f in &fields {
                    let x =
                        b.column(f).ok_or_else(|| err(format!("the other layer has no attribute {f:?}")))?.numbers();
                    let sums: Vec<f64> =
                        meets.iter().map(|js| js.iter().map(|&j| x[j]).filter(|v| !v.is_nan()).sum()).collect();
                    let means = meets
                        .iter()
                        .map(|js| {
                            let v: Vec<f64> = js.iter().map(|&j| x[j]).filter(|v| !v.is_nan()).collect();
                            if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 }
                        })
                        .collect();
                    columns.push((format!("{f}_sum"), sums));
                    columns.push((format!("{f}_mean"), means));
                }
                with_columns(a, columns).0
            }
            m => return Err(err(format!("mode {m:?}: attributes or summary"))),
        };
        Ok(out(v))
    }
}

/// `vector.reproject`.
struct Reproject;

impl Operator for Reproject {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.reproject",
            "Features in another coordinate system, their coordinates transformed by PROJ through GDAL (as \
             geopandas' to_crs): to an EPSG code, a WKT or a PROJ string, or to the coordinate system of a map or \
             of other features (`like`).",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .input(PortSpec::optional("like", Type::Any, "a map or features whose coordinate system to take"))
        .output(PortSpec::new("vector", Type::Vector, "the features reprojected"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional("crs", Type::Text, "the coordinate system: EPSG:4326, a WKT..."))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = vector_of(args, "vector")?;
        let to: Option<String> = match args.get("like") {
            Some(Value::Map(m)) => crate::prepare::crs_of(&m.grid),
            Some(Value::Stack(s)) => crate::prepare::crs_of(&s.grid),
            Some(Value::Vector(o)) => o.crs.clone(),
            Some(Value::Raster(r)) => crate::prepare::crs_of(&r.map()?.grid),
            Some(other) => {
                return Err(Error::msg(format!(
                    "node {node:?}: like is a {}, not a map or features",
                    other.ty().name()
                )));
            }
            None => args.opt_param::<String>("crs")?.filter(|c| !c.trim().is_empty()),
        };
        let to = to.ok_or_else(|| Error::msg(format!("node {node:?}: give the coordinate system (crs), or like")))?;
        let r = terraflux_gdal::reproject_vector(v, &to).map_err(|e| Error::msg(format!("node {node:?}: {e}")))?;
        Ok(out(r))
    }
}

/// `vector.from_table`.
struct FromTable;

impl Operator for FromTable {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.from_table",
            "Points from a table's columns of coordinates, as QGIS's Create Points Layer from Table or geopandas' \
             points_from_xy: a point per row with an x and a y, the other columns its attributes, in the \
             coordinate system given.",
            Access::None,
        )
        .input(PortSpec::new("table", Type::Table, "the table"))
        .output(PortSpec::new("vector", Type::Vector, "the points"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional("x", Type::Text, "the column of the x (or longitude) (x)"))
        .param(PortSpec::optional("y", Type::Text, "the column of the y (or latitude) (y)"))
        .param(PortSpec::optional(
            "crs",
            Type::Text,
            "their coordinate system: EPSG:4326, a WKT... (none: unknown)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let t = match args.get("table") {
            Some(Value::Table(t)) => t,
            _ => return Err(Error::msg(format!("node {node:?}: no table"))),
        };
        let col = |p: &str| -> Result<usize> {
            let name: String = args.param(p, p.to_string())?;
            t.columns.iter().position(|c| *c == name).ok_or_else(|| {
                Error::msg(format!(
                    "node {node:?}: the table has no column {name:?} (its columns: {})",
                    t.columns.join(", ")
                ))
            })
        };
        let (x, y) = (col("x")?, col("y")?);
        let keep: Vec<&Vec<f64>> = t.rows.iter().filter(|r| r[x].is_finite() && r[y].is_finite()).collect();
        let others: Vec<usize> = (0..t.columns.len()).filter(|&j| j != x && j != y).collect();
        let v = Vector {
            geometries: keep.iter().map(|r| Geometry::Point([r[x], r[y]])).collect(),
            attributes: others
                .iter()
                .map(|&j| (t.columns[j].clone(), Column::Numbers(keep.iter().map(|r| r[j]).collect())))
                .collect(),
            crs: args.opt_param::<String>("crs")?.filter(|c| !c.trim().is_empty()),
        };
        Ok(out(v))
    }
}

/// `vector.points`.
struct Points;

impl Operator for Points {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.points",
            "Points inside polygons, as QGIS's Random Points Inside Polygons and Regular Points: `random`, n points \
             in every polygon drawn uniformly over its area (numpy's generator of the seed); `regular`, the points \
             of a grid of a spacing (from the polygons' lower left corner, half a spacing in) inside each. Each \
             point with the number of its polygon (polygon) and the polygon's attributes.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the polygons"))
        .output(PortSpec::new("vector", Type::Vector, "the points"))
        .output(PortSpec::new("table", Type::Table, "their attributes, x and y"))
        .param(PortSpec::optional("method", Type::Text, "random (default) or regular").choices(["random", "regular"]))
        .param(PortSpec::optional("n", Type::Integer, "random: the points in every polygon (10)"))
        .param(PortSpec::optional("spacing", Type::Number, "regular: the distance between points (map units)"))
        .param(PortSpec::optional("seed", Type::Integer, "random: the generator's seed (1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: &str| Error::msg(format!("node {node:?}: {m}"));
        let v = vector_of(args, "vector")?;
        if v.geometries.iter().any(|g| !g.is_polygonal() && !matches!(g, Geometry::Empty)) {
            return Err(err("points are drawn inside polygons"));
        }
        let inside = |g: &Geometry, p: Xy| g.polygons().iter().any(|q| in_polygon(p, q));
        let mut points: Vec<(usize, Xy)> = Vec::new();
        match args.param("method", "random".to_string())?.as_str() {
            "random" => {
                let n: usize = args.param("n", 10)?;
                let mut rng = NumpyRng::new(args.param("seed", 1)?);
                for (i, g) in v.geometries.iter().enumerate() {
                    let (Some(b), true) = (g.bounds(), polygon_area_of(g) > 0.0) else { continue };
                    let (mut got, mut tries) = (0, 0usize);
                    while got < n && tries < 1000 * n.max(1) {
                        tries += 1;
                        let p = [b[0] + rng.random() * (b[2] - b[0]), b[1] + rng.random() * (b[3] - b[1])];
                        if inside(g, p) {
                            points.push((i, p));
                            got += 1;
                        }
                    }
                }
            }
            "regular" => {
                let s: f64 = args.opt_param("spacing")?.ok_or_else(|| err("regular points: give the spacing"))?;
                if s <= 0.0 {
                    return Err(err("the spacing must be above 0"));
                }
                let b = v.bounds().ok_or_else(|| err("no polygons"))?;
                let (nx, ny) = (((b[2] - b[0]) / s).ceil() as usize, ((b[3] - b[1]) / s).ceil() as usize);
                if nx.saturating_mul(ny) > 50_000_000 {
                    return Err(err("more than 50 million points: a larger spacing"));
                }
                for (i, g) in v.geometries.iter().enumerate() {
                    let Some(gb) = g.bounds() else { continue };
                    let (c0, c1) = (((gb[0] - b[0]) / s).floor() as usize, ((gb[2] - b[0]) / s).ceil() as usize);
                    let (r0, r1) = (((gb[1] - b[1]) / s).floor() as usize, ((gb[3] - b[1]) / s).ceil() as usize);
                    for r in r0..r1.min(ny) {
                        for c in c0..c1.min(nx) {
                            let p = [b[0] + (c as f64 + 0.5) * s, b[1] + (r as f64 + 0.5) * s];
                            if inside(g, p) {
                                points.push((i, p));
                            }
                        }
                    }
                }
            }
            m => return Err(err(&format!("method {m:?}: random or regular"))),
        }
        let from: Vec<usize> = points.iter().map(|p| p.0).collect();
        let mut attributes =
            vec![("polygon".to_string(), Column::Numbers(from.iter().map(|i| (i + 1) as f64).collect()))];
        attributes.extend(v.select(&from).attributes.into_iter().filter(|(n, _)| n != "polygon"));
        attributes.push(("x".into(), Column::Numbers(points.iter().map(|p| p.1[0]).collect())));
        attributes.push(("y".into(), Column::Numbers(points.iter().map(|p| p.1[1]).collect())));
        let out_v = Vector {
            geometries: points.iter().map(|p| Geometry::Point(p.1)).collect(),
            attributes,
            crs: v.crs.clone(),
        };
        Ok(out(out_v))
    }
}

/// The area of a geometry's polygons.
fn polygon_area_of(g: &Geometry) -> f64 {
    g.polygons().iter().map(|p| polygon_area(p)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Map, Table};
    use terraflux_raster::Grid;

    fn run(op: &dyn Operator, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Result<Outputs> {
        let params = params.as_object().cloned().unwrap_or_default();
        let args =
            Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
        op.run(&mut Ctx::default(), &args)
    }

    fn vector(o: &Outputs) -> Arc<Vector> {
        match &o["vector"] {
            Value::Vector(v) => v.clone(),
            _ => panic!("no features"),
        }
    }

    fn square(x: f64, y: f64, s: f64) -> Geometry {
        Geometry::Polygon(vec![vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s], [x, y]]])
    }

    fn numbers(v: &Vector, name: &str) -> Vec<f64> {
        v.column(name).unwrap().numbers()
    }

    const N: f64 = f64::NAN;

    #[test]
    fn a_map_is_polygonized() {
        // a ring of 1 around a 2, a speck of 1 on its own corner, a missing cell
        #[rustfmt::skip]
        let v = [
            1.0, 1.0, 1.0, 3.0,
            1.0, 2.0, 1.0, 3.0,
            1.0, 1.0, 1.0, 3.0,
            N,   3.0, 3.0, 1.0,
        ];
        let m = Value::Map(Map::new(Arc::new(Grid::new(4, 4)), v.to_vec()));
        let o = run(&Polygonize, vec![("map", m.clone())], serde_json::json!({})).unwrap();
        let p = vector(&o);
        assert_eq!(numbers(&p, "value"), [1.0, 3.0, 2.0, 3.0, 1.0]);
        assert_eq!(numbers(&p, "cells"), [8.0, 3.0, 1.0, 2.0, 1.0]);
        // the ring with its hole: its area is its cells', its rings 4 corners each
        let Geometry::Polygon(rings) = &p.geometries[0] else { panic!("{:?}", p.geometries[0]) };
        assert_eq!(rings.len(), 2);
        assert!((polygon_area(rings) - 8.0).abs() < 1e-9);
        assert_eq!(rings[0].len(), 5);
        // every polygon's area its cells' (rows up: y = -row)
        for (g, n) in p.geometries.iter().zip(numbers(&p, "cells")) {
            assert!((polygon_area_of(g) - n).abs() < 1e-9, "{g:?}");
        }
        // with eight neighbours the corner speck (row 2, column 3) joins the ring
        let o = run(&Polygonize, vec![("map", m)], serde_json::json!({ "neighbours": 8 })).unwrap();
        let p = vector(&o);
        assert_eq!(numbers(&p, "cells")[0], 9.0);
        assert!((polygon_area_of(&p.geometries[0]) - 9.0).abs() < 1e-9);
    }

    #[test]
    fn features_are_filtered_and_measured() {
        let v = Vector {
            geometries: vec![
                square(0.0, 0.0, 2.0),
                square(10.0, 0.0, 1.0),
                Geometry::LineString(vec![[0.0, 0.0], [3.0, 4.0]]),
            ],
            attributes: vec![("class".into(), Column::Numbers(vec![1.0, 2.0, 1.0]))],
            crs: None,
        };
        let v = Value::Vector(Arc::new(v));
        let o = run(&Measure, vec![("vector", v.clone())], serde_json::json!({})).unwrap();
        let m = vector(&o);
        assert_eq!(numbers(&m, "area"), [4.0, 1.0, 0.0]);
        assert_eq!(numbers(&m, "perimeter"), [8.0, 4.0, 0.0]);
        assert_eq!(numbers(&m, "length"), [0.0, 0.0, 5.0]);
        let near = |a: Vec<f64>, b: [f64; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
        assert!(near(numbers(&m, "x"), [1.0, 10.5, 1.5]) && near(numbers(&m, "y"), [1.0, 0.5, 2.0]));
        let o = run(
            &Filter,
            vec![("vector", Value::Vector(m.clone()))],
            serde_json::json!({ "expression": "class == 1 and area > 0" }),
        )
        .unwrap();
        assert_eq!(vector(&o).len(), 1);
        let o = run(
            &Calc,
            vec![("vector", Value::Vector(m))],
            serde_json::json!({ "column": "ha", "expression": "area / 2" }),
        )
        .unwrap();
        assert_eq!(numbers(&vector(&o), "ha"), [2.0, 0.5, 0.0]);
    }

    #[test]
    fn points_come_from_a_table_and_inside_polygons() {
        let mut t = Table::new(["x", "y", "v"].map(String::from).to_vec());
        t.rows = vec![vec![1.0, 2.0, 5.0], vec![N, 1.0, 6.0]];
        let o = run(&FromTable, vec![("table", Value::Table(t))], serde_json::json!({ "crs": "EPSG:4326" })).unwrap();
        let p = vector(&o);
        assert_eq!(p.geometries, [Geometry::Point([1.0, 2.0])]);
        assert_eq!(numbers(&p, "v"), [5.0]);
        assert_eq!(p.crs.as_deref(), Some("EPSG:4326"));
        // a square with a hole
        let holed = Geometry::Polygon(vec![
            vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]],
            vec![[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0], [1.0, 1.0]],
        ]);
        let v =
            Value::Vector(Arc::new(Vector { geometries: vec![holed, square(10.0, 0.0, 1.0)], ..Default::default() }));
        let o = run(&Points, vec![("vector", v.clone())], serde_json::json!({ "n": 50, "seed": 3 })).unwrap();
        let p = vector(&o);
        assert_eq!(p.len(), 100);
        let f = numbers(&p, "polygon");
        for (g, k) in p.geometries.iter().zip(f) {
            let Geometry::Point(q) = g else { panic!() };
            if k == 1.0 {
                assert!(!(q[0] > 1.0 && q[0] < 3.0 && q[1] > 1.0 && q[1] < 3.0), "{q:?} in the hole");
            } else {
                assert!(q[0] >= 10.0 && q[0] <= 11.0);
            }
        }
        let o = run(&Points, vec![("vector", v)], serde_json::json!({ "method": "regular", "spacing": 1.0 })).unwrap();
        // 16 cells of the square less the hole's 4, and one in the small square
        assert_eq!(vector(&o).len(), 13);
    }

    #[test]
    fn layers_are_overlaid_and_joined() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        let a = Vector {
            geometries: vec![square(0.0, 0.0, 2.0)],
            attributes: vec![("id".into(), Column::Numbers(vec![1.0]))],
            crs: None,
        };
        let b = Vector {
            geometries: vec![square(1.0, 0.0, 2.0), square(5.0, 5.0, 1.0)],
            attributes: vec![("id".into(), Column::Numbers(vec![7.0, 8.0]))],
            crs: None,
        };
        let (a, b) = (Value::Vector(Arc::new(a)), Value::Vector(Arc::new(b)));
        let at = |how: &str| {
            vector(
                &run(&Overlay, vec![("vector", a.clone()), ("other", b.clone())], serde_json::json!({ "how": how }))
                    .unwrap(),
            )
        };
        let i = at("intersection");
        assert_eq!(i.len(), 1);
        assert!((polygon_area_of(&i.geometries[0]) - 2.0).abs() < 1e-9);
        assert_eq!(numbers(&i, "id_2"), [7.0]);
        let d = at("difference");
        assert!((polygon_area_of(&d.geometries[0]) - 2.0).abs() < 1e-9);
        let u = at("union");
        assert_eq!(u.len(), 4);
        let total: f64 = u.geometries.iter().map(polygon_area_of).sum();
        assert!((total - 7.0).abs() < 1e-9, "{total}");
        let pts = Value::Vector(Arc::new(Vector {
            geometries: vec![Geometry::Point([0.5, 0.5]), Geometry::Point([1.5, 1.5]), Geometry::Point([9.0, 9.0])],
            attributes: vec![("w".into(), Column::Numbers(vec![1.0, 2.0, 4.0]))],
            crs: None,
        }));
        let o = run(
            &SpatialJoin,
            vec![("vector", a.clone()), ("other", pts.clone())],
            serde_json::json!({ "mode": "summary", "fields": "w" }),
        )
        .unwrap();
        let s = vector(&o);
        assert_eq!(numbers(&s, "count"), [2.0]);
        assert_eq!(numbers(&s, "w_sum"), [3.0]);
        let o = run(&SpatialJoin, vec![("vector", pts), ("other", a)], serde_json::json!({})).unwrap();
        let j = vector(&o);
        let id = numbers(&j, "id");
        assert_eq!(id[..2], [1.0, 1.0]);
        assert!(id[2].is_nan());
    }
}
