//! Smoothing and polygon coverages (`vector.smooth`, `vector.coverage_node`, `vector.coverage_simplify`,
//! `vector.coverage_validate`): Chaikin's corner cutting and the shared vertices of a coverage in plain arithmetic (bit
//! for bit as the library's tf.vector.smooth and tf.vector.coverage_node) and GEOS's coverage functions (3.12 or later)
//! through terraflux_gdal::geos, with the attributes kept.

use std::sync::Arc;

use rstar::primitives::GeomWithData;
use rstar::{AABB, RTree};

use terraflux_gdal::geos::{coverage_invalid_edges, coverage_simplify};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::vector::{attribute_table, vector_of};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Smooth));
    r.add(Arc::new(CoverageNode));
    r.add(Arc::new(CoverageSimplify));
    r.add(Arc::new(CoverageValidate));
}

fn give(v: Vector) -> Outputs {
    let t = attribute_table(&v);
    outputs([("vector", Value::Vector(Arc::new(v))), ("table", Value::Table(t))])
}

fn err(node: &str) -> impl Fn(String) -> Error + '_ {
    move |m| Error::msg(format!("node {node:?}: {m}"))
}

/// The two points of Chaikin's cut of the segment p -> q, each coordinate as `0.75 * a + 0.25 * b` (plain
/// operations, no fused multiply-add: the library's numpy gives the same bits).
fn cut(p: [f64; 2], q: [f64; 2]) -> ([f64; 2], [f64; 2]) {
    ([0.75 * p[0] + 0.25 * q[0], 0.75 * p[1] + 0.25 * q[1]], [0.25 * p[0] + 0.75 * q[0], 0.25 * p[1] + 0.75 * q[1]])
}

/// One iteration on an open line: lines of fewer than 3 vertices stay; the ends are kept with `keep_ends`.
fn chaikin_line(xy: &[[f64; 2]], keep_ends: bool) -> Vec<[f64; 2]> {
    if xy.len() < 3 {
        return xy.to_vec();
    }
    let mut out = Vec::with_capacity(2 * xy.len());
    if keep_ends {
        out.push(xy[0]);
    }
    for w in xy.windows(2) {
        let (q, r) = cut(w[0], w[1]);
        out.push(q);
        out.push(r);
    }
    if keep_ends {
        out.push(xy[xy.len() - 1]);
    }
    out
}

/// One iteration on a ring (the last vertex the first): every segment is cut, the closing one included, and the
/// result closes on its first cut. Rings of fewer than 4 coordinates stay.
fn chaikin_ring(xy: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if xy.len() < 4 {
        return xy.to_vec();
    }
    let mut out = Vec::with_capacity(2 * xy.len());
    for w in xy.windows(2) {
        let (q, r) = cut(w[0], w[1]);
        out.push(q);
        out.push(r);
    }
    out.push(out[0]);
    out
}

fn times(xy: &[[f64; 2]], n: usize, f: impl Fn(&[[f64; 2]]) -> Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    let mut v = xy.to_vec();
    for _ in 0..n {
        v = f(&v);
    }
    v
}

/// A geometry smoothed `n` times (points and empty geometries stay).
pub fn smooth_geometry(g: &Geometry, n: usize, keep_ends: bool) -> Geometry {
    // a geometry without a vertex (an empty polygon) is the empty geometry, left as it is
    if g.bounds().is_none() {
        return g.clone();
    }
    let line = |l: &Vec<[f64; 2]>| times(l, n, |x| chaikin_line(x, keep_ends));
    let poly = |p: &Vec<Vec<[f64; 2]>>| p.iter().map(|r| times(r, n, chaikin_ring)).collect::<Vec<_>>();
    match g {
        Geometry::LineString(l) => Geometry::LineString(line(l)),
        Geometry::MultiLineString(m) => Geometry::MultiLineString(m.iter().map(line).collect()),
        Geometry::Polygon(p) => Geometry::Polygon(poly(p)),
        Geometry::MultiPolygon(m) => Geometry::MultiPolygon(m.iter().map(poly).collect()),
        _ => g.clone(),
    }
}

/// `vector.smooth`.
struct Smooth;

impl Operator for Smooth {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.smooth",
            "Lines and polygons smoothed by Chaikin's corner cutting (PostGIS's ST_ChaikinSmoothing, QGIS's Smooth): \
             each iteration replaces every segment by the two points at a quarter and three quarters of it, so the \
             corners are rounded and the vertex count doubles. An open line keeps its first and last vertex with \
             `preserve_ends` (the default); lines of fewer than 3 vertices stay. Polygon rings (holes too) are \
             smoothed all around, the closing segment included, and start at the first cut of their first \
             segment. Points and empty features are kept, multi-part features are smoothed part by part, and the \
             attributes are kept. The area of a polygon shrinks, so a very narrow shape may stop being valid.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .output(PortSpec::new("vector", Type::Vector, "the smoothed features"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional("iterations", Type::Integer, "the times the corners are cut, 1 to 10 (1)"))
        .param(PortSpec::optional(
            "preserve_ends",
            Type::Bool,
            "keep the first and last vertex of a line (true)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = vector_of(args, "vector")?;
        let n: i64 = args.param("iterations", 1)?;
        if !(1..=10).contains(&n) {
            return Err(Error::msg(format!("node {:?}: iterations is an integer from 1 to 10, got {n}", args.node)));
        }
        let keep: bool = args.param("preserve_ends", true)?;
        let geometries = v.geometries.iter().map(|g| smooth_geometry(g, n as usize, keep)).collect();
        Ok(give(Vector { geometries, attributes: v.attributes.clone(), crs: v.crs.clone() }))
    }
}

/// Whether `v` lies on the segment `p -> q`, strictly between its ends, within `tol`, and then its projection (the dot
/// product, for the order along the segment). The arithmetic of tf.vector.coverage_node, operation for operation (no
/// fused multiply-add), so that both decide alike: the distance to the line at most `tol` (`|cross| <= tol * |q - p|`,
/// exactly 0 by default), the projection strictly inside, and farther than `tol` from both ends.
fn on_segment(p: [f64; 2], q: [f64; 2], v: [f64; 2], tol: f64) -> Option<f64> {
    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
    let (wx, wy) = (v[0] - p[0], v[1] - p[1]);
    let (ux, uy) = (v[0] - q[0], v[1] - q[1]);
    let cross = dx * wy - dy * wx;
    let dot = dx * wx + dy * wy;
    let len2 = dx * dx + dy * dy;
    let on = cross.abs() <= tol * len2.sqrt()
        && dot > 0.0
        && dot < len2
        && wx * wx + wy * wy > tol * tol
        && ux * ux + uy * uy > tol * tol;
    on.then_some(dot)
}

/// The polygons with the vertices they share with their neighbours (tf.vector.coverage_node): every vertex of another
/// feature lying on a segment of a ring (see `on_segment`) is added to it, in the order of the projection, then x,
/// then y, a repeated coordinate once. Every ring keeps its start, its orientation and its vertices; a feature's own
/// vertices are not added to it. Gives the geometries and how many vertices each feature gained.
pub fn coverage_node(geoms: &[Geometry], tol: f64) -> std::result::Result<(Vec<Geometry>, Vec<usize>), String> {
    for (i, g) in geoms.iter().enumerate() {
        if !matches!(g, Geometry::Empty | Geometry::Polygon(_) | Geometry::MultiPolygon(_)) && g.bounds().is_some() {
            return Err(format!("feature {i} is not a polygon (a coverage is polygons)"));
        }
    }
    // every vertex of every ring but its last (the ring's first again), with its feature
    let mut items = Vec::new();
    for (f, g) in geoms.iter().enumerate() {
        for poly in g.polygons() {
            for ring in poly {
                for v in ring.iter().take(ring.len().saturating_sub(1)) {
                    items.push(GeomWithData::new(*v, f));
                }
            }
        }
    }
    let tree = RTree::bulk_load(items);
    let mut added = vec![0usize; geoms.len()];
    let mut on: Vec<(f64, [f64; 2])> = Vec::new();
    let mut node_ring = |f: usize, ring: &Vec<[f64; 2]>| -> Vec<[f64; 2]> {
        let mut out = Vec::with_capacity(ring.len());
        for (k, &p) in ring.iter().enumerate() {
            out.push(p);
            let Some(&q) = ring.get(k + 1) else { break };
            let lo = [p[0].min(q[0]) - tol, p[1].min(q[1]) - tol];
            let hi = [p[0].max(q[0]) + tol, p[1].max(q[1]) + tol];
            on.clear();
            for c in tree.locate_in_envelope_intersecting(&AABB::from_corners(lo, hi)) {
                if c.data != f
                    && let Some(dot) = on_segment(p, q, *c.geom(), tol)
                {
                    on.push((dot, *c.geom()));
                }
            }
            on.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1[0].total_cmp(&b.1[0])).then(a.1[1].total_cmp(&b.1[1])));
            on.dedup_by(|a, b| a.1 == b.1);
            added[f] += on.len();
            out.extend(on.iter().map(|x| x.1));
        }
        out
    };
    let out = geoms
        .iter()
        .enumerate()
        .map(|(f, g)| match g {
            Geometry::Polygon(p) => Geometry::Polygon(p.iter().map(|r| node_ring(f, r)).collect()),
            Geometry::MultiPolygon(m) => {
                Geometry::MultiPolygon(m.iter().map(|p| p.iter().map(|r| node_ring(f, r)).collect()).collect())
            }
            _ => g.clone(),
        })
        .collect();
    Ok((out, added))
}

/// `vector.coverage_node`.
struct CoverageNode;

impl Operator for CoverageNode {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.coverage_node",
            "The polygons with the vertices they share with their neighbours: every vertex of another feature that \
             lies on an edge of a polygon is added to that edge, so that shared edges match vertex for vertex and the \
             layer is a valid coverage for vector.coverage_validate and vector.coverage_simplify. The polygons of \
             vector.polygonize need it: where two patches meet the straight edge of a third, that edge has no vertex. \
             Nothing else changes (the features, their order and attributes, every ring's start and the vertices \
             already there), and a column `added` counts the vertices each feature gained; overlaps and gaps stay. \
             A vertex is added when it lies on the segment strictly between its ends: exactly on it by default \
             (coordinates on a grid), or within `tolerance` (in the layer's units, for coordinates that went \
             through a reprojection). Like PostGIS's ST_Snap that only adds vertices, or QGIS's Snap geometries to \
             layer inserting extra vertices.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the polygons"))
        .output(PortSpec::new("vector", Type::Vector, "the polygons with the shared vertices"))
        .output(PortSpec::new("table", Type::Table, "their attributes and added"))
        .param(PortSpec::optional(
            "tolerance",
            Type::Number,
            "how far a neighbour's vertex may be from the edge, in the layer's units (0: exactly on it)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let v = vector_of(args, "vector")?;
        let tol: f64 = args.param("tolerance", 0.0)?;
        if !tol.is_finite() || tol < 0.0 {
            return Err(e(format!("tolerance is a number of 0 or more, got {tol}")));
        }
        let (geometries, added) = coverage_node(&v.geometries, tol).map_err(&e)?;
        let added = Column::Numbers(added.into_iter().map(|n| n as f64).collect());
        let mut attributes = v.attributes.clone();
        match attributes.iter_mut().find(|(n, _)| n == "added") {
            Some((_, c)) => *c = added,
            None => attributes.push(("added".into(), added)),
        }
        Ok(give(Vector { geometries, attributes, crs: v.crs.clone() }))
    }
}

/// `vector.coverage_simplify`.
struct CoverageSimplify;

impl Operator for CoverageSimplify {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.coverage_simplify",
            "The polygons of a coverage simplified together with Visvalingam-Whyatt (GEOS 3.12's, as PostGIS's \
             ST_CoverageSimplify and QGIS's Simplify coverage): neighbours keep sharing their edges, so no gap or \
             overlap opens, which simplifying each polygon alone (vector.simplify) cannot promise. The layer must be \
             polygons that form a valid coverage (see vector.coverage_validate; vector.coverage_node adds the shared \
             vertices that the polygons of vector.polygonize lack). `tolerance` is in the layer's \
             units; with `preserve_boundary` the outer boundary of the coverage is left as it is and only the \
             edges between polygons are simplified. The attributes are kept.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the polygons of the coverage"))
        .output(PortSpec::new("vector", Type::Vector, "the simplified polygons"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::new("tolerance", Type::Number, "the simplification tolerance, in the layer's units"))
        .param(PortSpec::optional(
            "preserve_boundary",
            Type::Bool,
            "leave the outer boundary as it is (false)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let v = vector_of(args, "vector")?;
        let tolerance: f64 = args.opt_param("tolerance")?.ok_or_else(|| e("give the tolerance".into()))?;
        let preserve: bool = args.param("preserve_boundary", false)?;
        let geometries = coverage_simplify(&v.geometries, tolerance, preserve).map_err(&e)?;
        Ok(give(Vector { geometries, attributes: v.attributes.clone(), crs: v.crs.clone() }))
    }
}

/// `vector.coverage_validate`.
struct CoverageValidate;

impl Operator for CoverageValidate {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.coverage_validate",
            "The edges of a polygon coverage that overlap or leave a gap with their neighbours (GEOS 3.12's, as \
             PostGIS's ST_CoverageInvalidEdges and QGIS's Validate coverage), per feature: the features with their \
             attributes, a column `valid` (1 when the feature has no invalid edge, else 0) and as geometry the \
             invalid edges of the feature as lines (empty when valid). `gap_width` also flags edges closer than \
             that to a neighbour, the narrow gaps (in the layer's units; 0 flags overlaps and unmatched edges only).",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the polygons of the coverage"))
        .output(PortSpec::new("vector", Type::Vector, "the features with their invalid edges"))
        .output(PortSpec::new("table", Type::Table, "their attributes and valid"))
        .param(PortSpec::optional(
            "gap_width",
            Type::Number,
            "also flag edges closer than this to a neighbour (0)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = vector_of(args, "vector")?;
        let gap: f64 = args.param("gap_width", 0.0)?;
        let geometries = coverage_invalid_edges(&v.geometries, gap).map_err(err(args.node))?;
        let valid = Column::Numbers(geometries.iter().map(|g| if g.bounds().is_none() { 1.0 } else { 0.0 }).collect());
        let mut attributes = v.attributes.clone();
        match attributes.iter_mut().find(|(n, _)| n == "valid") {
            Some((_, c)) => *c = valid,
            None => attributes.push(("valid".into(), valid)),
        }
        Ok(give(Vector { geometries, attributes, crs: v.crs.clone() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(op: &dyn Operator, v: Vector, params: serde_json::Value) -> Result<Outputs> {
        let args = Args {
            node: "n",
            inputs: [("vector".to_string(), Value::Vector(Arc::new(v)))].into_iter().collect(),
            params: params.as_object().unwrap(),
        };
        op.run(&mut Ctx::default(), &args)
    }

    fn vector(o: &Outputs) -> Arc<Vector> {
        let Value::Vector(v) = &o["vector"] else { panic!() };
        v.clone()
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Geometry {
        Geometry::Polygon(vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]])
    }

    fn corner() -> Geometry {
        Geometry::LineString(vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0]])
    }

    #[test]
    fn a_line_is_cut_at_a_quarter_and_three_quarters() {
        assert_eq!(
            smooth_geometry(&corner(), 1, true),
            Geometry::LineString(vec![[0.0, 0.0], [1.0, 0.0], [3.0, 0.0], [4.0, 1.0], [4.0, 3.0], [4.0, 4.0]])
        );
        assert_eq!(
            smooth_geometry(&corner(), 1, false),
            Geometry::LineString(vec![[1.0, 0.0], [3.0, 0.0], [4.0, 1.0], [4.0, 3.0]])
        );
        // two iterations: the vertex count doubles again, the ends stay
        let Geometry::LineString(l) = smooth_geometry(&corner(), 2, true) else { panic!() };
        assert_eq!(l.len(), 2 + 2 * (6 - 1));
        assert_eq!((l[0], l[l.len() - 1]), ([0.0, 0.0], [4.0, 4.0]));
        // a line of two vertices stays
        let two = Geometry::LineString(vec![[0.0, 0.0], [1.0, 1.0]]);
        assert_eq!(smooth_geometry(&two, 3, true), two);
    }

    #[test]
    fn a_ring_is_cut_all_around_and_closed() {
        let sq = rect(0.0, 0.0, 4.0, 4.0);
        assert_eq!(
            smooth_geometry(&sq, 1, true),
            Geometry::Polygon(vec![vec![
                [1.0, 0.0],
                [3.0, 0.0],
                [4.0, 1.0],
                [4.0, 3.0],
                [3.0, 4.0],
                [1.0, 4.0],
                [0.0, 3.0],
                [0.0, 1.0],
                [1.0, 0.0],
            ]])
        );
        // a multipolygon part by part, a point and an empty feature untouched
        let m = Geometry::MultiPolygon(vec![vec![vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 0.0]]]]);
        let Geometry::MultiPolygon(s) = smooth_geometry(&m, 1, true) else { panic!() };
        assert_eq!(s[0][0].len(), 7);
        assert_eq!(smooth_geometry(&Geometry::Point([1.0, 2.0]), 2, true), Geometry::Point([1.0, 2.0]));
        assert_eq!(smooth_geometry(&Geometry::Empty, 2, true), Geometry::Empty);
    }

    #[test]
    fn the_operator_checks_iterations_and_keeps_attributes() {
        let layer = Vector {
            geometries: vec![corner(), Geometry::Empty],
            attributes: vec![("id".into(), Column::Numbers(vec![1.0, 2.0]))],
            crs: Some("EPSG:32633".into()),
        };
        let o = run(&Smooth, layer.clone(), serde_json::json!({"iterations": 2, "preserve_ends": false})).unwrap();
        let v = vector(&o);
        assert_eq!(v.column("id").unwrap().numbers(), vec![1.0, 2.0]);
        assert_eq!(v.crs.as_deref(), Some("EPSG:32633"));
        assert_eq!(v.geometries[1], Geometry::Empty);
        assert!(o.contains_key("table"));
        assert!(run(&Smooth, layer.clone(), serde_json::json!({"iterations": 0})).is_err());
        assert!(run(&Smooth, layer, serde_json::json!({"iterations": 11})).is_err());
    }

    #[test]
    fn coverage_operators_need_polygons_and_a_tolerance() {
        let squares =
            Vector { geometries: vec![rect(0.0, 0.0, 1.0, 1.0), rect(1.0, 0.0, 2.0, 1.0)], ..Vector::default() };
        assert!(run(&CoverageSimplify, squares.clone(), serde_json::json!({})).is_err());
        let lines = Vector { geometries: vec![corner()], ..Vector::default() };
        assert!(run(&CoverageSimplify, lines.clone(), serde_json::json!({"tolerance": 1.0})).is_err());
        assert!(run(&CoverageValidate, lines, serde_json::json!({})).is_err());
        if terraflux_gdal::geos::geos().is_err() {
            return;
        }
        // GEOS older than 3.12 reports an error instead
        let Ok(o) = run(&CoverageValidate, squares, serde_json::json!({})) else { return };
        let v = vector(&o);
        assert_eq!(v.column("valid").unwrap().numbers(), vec![1.0, 1.0]);
        assert!(v.geometries.iter().all(|g| *g == Geometry::Empty));
    }
    /// A tall cell on the left, two short ones on the right: the left's straight edge x=1 has no vertex at (1, 1).
    /// The rings as shapely's `box` writes them (counter-clockwise from the lower right corner).
    fn t_junction() -> Vec<Geometry> {
        let b = |x0: f64, y0: f64, x1: f64, y1: f64| {
            Geometry::Polygon(vec![vec![[x1, y0], [x1, y1], [x0, y1], [x0, y0], [x1, y0]]])
        };
        vec![b(0.0, 0.0, 1.0, 2.0), b(1.0, 0.0, 2.0, 1.0), b(1.0, 1.0, 2.0, 2.0)]
    }

    #[test]
    fn the_junction_vertex_is_added_to_the_straight_edge() {
        let g = t_junction();
        let (out, added) = coverage_node(&g, 0.0).unwrap();
        assert_eq!(added, vec![1, 0, 0]);
        assert_eq!(
            out[0],
            Geometry::Polygon(vec![vec![[1.0, 0.0], [1.0, 1.0], [1.0, 2.0], [0.0, 2.0], [0.0, 0.0], [1.0, 0.0]]])
        );
        assert_eq!((&out[1], &out[2]), (&g[1], &g[2]));
        // again: nothing to add
        assert_eq!(coverage_node(&out, 0.0).unwrap().1, vec![0, 0, 0]);
    }

    #[test]
    fn holes_multipolygons_and_the_order_along_a_segment() {
        // a frame whose hole holds four quarters: the hole's edges get the quarters' midpoints
        let ring = |c: &[[f64; 2]]| c.to_vec();
        let frame = Geometry::Polygon(vec![
            ring(&[[6.0, 0.0], [6.0, 6.0], [0.0, 6.0], [0.0, 0.0], [6.0, 0.0]]),
            ring(&[[4.0, 2.0], [4.0, 4.0], [2.0, 4.0], [2.0, 2.0], [4.0, 2.0]]),
        ]);
        let quarter = |x: f64, y: f64| rect(x, y, x + 1.0, y + 1.0);
        let multi = Geometry::MultiPolygon(vec![
            vec![ring(&[[11.0, 0.0], [11.0, 3.0], [10.0, 3.0], [10.0, 0.0], [11.0, 0.0]])],
            vec![ring(&[[21.0, 0.0], [21.0, 1.0], [20.0, 1.0], [20.0, 0.0], [21.0, 0.0]])],
        ]);
        let g = vec![
            frame,
            quarter(2.0, 2.0),
            quarter(3.0, 2.0),
            quarter(2.0, 3.0),
            quarter(3.0, 3.0),
            multi.clone(),
            rect(11.0, 0.0, 12.0, 1.0),
            rect(11.0, 1.0, 12.0, 2.0),
            Geometry::Empty,
        ];
        let (out, added) = coverage_node(&g, 0.0).unwrap();
        assert_eq!(added, vec![4, 0, 0, 0, 0, 2, 0, 0, 0]);
        let Geometry::Polygon(f) = &out[0] else { panic!() };
        assert_eq!(
            f[1],
            vec![
                [4.0, 2.0],
                [4.0, 3.0],
                [4.0, 4.0],
                [3.0, 4.0],
                [2.0, 4.0],
                [2.0, 3.0],
                [2.0, 2.0],
                [3.0, 2.0],
                [4.0, 2.0]
            ]
        );
        let Geometry::MultiPolygon(m) = &out[5] else { panic!() };
        assert_eq!(
            m[0][0],
            vec![[11.0, 0.0], [11.0, 1.0], [11.0, 2.0], [11.0, 3.0], [10.0, 3.0], [10.0, 0.0], [11.0, 0.0]]
        );
        let Geometry::MultiPolygon(before) = &multi else { panic!() };
        assert_eq!(m[1], before[1]);
        assert_eq!(out[8], Geometry::Empty);
        // three neighbours along one edge, two of them equal: added from (0, 0) upwards, (0, 3) once
        let long = Geometry::Polygon(vec![ring(&[[0.0, 0.0], [0.0, 4.0], [1.0, 4.0], [1.0, 0.0], [0.0, 0.0]])]);
        let g = vec![
            long,
            rect(-1.0, 0.0, 0.0, 1.0),
            rect(-1.0, 1.0, 0.0, 3.0),
            rect(-1.0, 3.0, 0.0, 4.0),
            rect(-1.0, 3.0, 0.0, 4.0),
        ];
        let (out, added) = coverage_node(&g, 0.0).unwrap();
        assert_eq!(added[0], 2);
        assert_eq!(
            out[0],
            Geometry::Polygon(vec![ring(&[
                [0.0, 0.0],
                [0.0, 1.0],
                [0.0, 3.0],
                [0.0, 4.0],
                [1.0, 4.0],
                [1.0, 0.0],
                [0.0, 0.0]
            ])])
        );
    }

    #[test]
    fn a_tolerance_takes_a_vertex_just_off_the_edge() {
        let g = vec![rect(0.0, 0.0, 1.0, 2.0), rect(1.0 + 1e-9, 0.0, 2.0, 1.0), rect(1.0 + 1e-9, 1.0, 2.0, 2.0)];
        assert_eq!(coverage_node(&g, 0.0).unwrap().1[0], 0);
        let (out, added) = coverage_node(&g, 1e-6).unwrap();
        assert_eq!(added[0], 1);
        let mut pts = Vec::new();
        out[0].points(&mut |p| pts.push(p));
        assert!(pts.contains(&[1.0 + 1e-9, 1.0]));
        // a vertex within the tolerance of an end is that end
        let g = vec![rect(0.0, 0.0, 1.0, 2.0), rect(1.0, 1e-7, 2.0, 1.0)];
        assert_eq!(coverage_node(&g, 1e-6).unwrap().1[0], 1);
    }

    #[test]
    fn the_node_operator_counts_and_checks() {
        let layer = Vector {
            geometries: t_junction(),
            attributes: vec![("n".into(), Column::Numbers(vec![10.0, 20.0, 30.0]))],
            crs: Some("EPSG:32633".into()),
        };
        let o = run(&CoverageNode, layer.clone(), serde_json::json!({})).unwrap();
        let v = vector(&o);
        assert_eq!(v.column("n").unwrap().numbers(), vec![10.0, 20.0, 30.0]);
        assert_eq!(v.column("added").unwrap().numbers(), vec![1.0, 0.0, 0.0]);
        assert_eq!(v.crs.as_deref(), Some("EPSG:32633"));
        assert!(o.contains_key("table"));
        for bad in [-1.0, f64::INFINITY] {
            assert!(run(&CoverageNode, layer.clone(), serde_json::json!({ "tolerance": bad })).is_err());
        }
        let lines = Vector { geometries: vec![corner()], ..Vector::default() };
        assert!(run(&CoverageNode, lines, serde_json::json!({})).unwrap_err().to_string().contains("not a polygon"));
        if terraflux_gdal::geos::geos().is_err() {
            return;
        }
        // the junction makes the raw layer fail GEOS's coverage check, the noded one pass (GEOS 3.12 or later)
        let Ok(raw) = run(&CoverageValidate, layer, serde_json::json!({})) else { return };
        assert_eq!(vector(&raw).column("valid").unwrap().numbers()[0], 0.0);
        let noded = run(&CoverageValidate, (*v).clone(), serde_json::json!({})).unwrap();
        assert_eq!(vector(&noded).column("valid").unwrap().numbers(), vec![1.0, 1.0, 1.0]);
    }
}
