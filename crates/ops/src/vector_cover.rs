//! Smoothing and polygon coverages (`vector.smooth`, `vector.coverage_simplify`, `vector.coverage_validate`): Chaikin's
//! corner cutting in plain arithmetic (bit for bit as the library's tf.vector.smooth) and GEOS's coverage functions
//! (3.12 or later) through terraflux_gdal::geos, with the attributes kept.

use std::sync::Arc;

use terraflux_gdal::geos::{coverage_invalid_edges, coverage_simplify};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::vector::{attribute_table, vector_of};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Smooth));
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

/// `vector.coverage_simplify`.
struct CoverageSimplify;

impl Operator for CoverageSimplify {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.coverage_simplify",
            "The polygons of a coverage simplified together with Visvalingam-Whyatt (GEOS 3.12's, as PostGIS's \
             ST_CoverageSimplify and QGIS's Simplify coverage): neighbours keep sharing their edges, so no gap or \
             overlap opens, which simplifying each polygon alone (vector.simplify) cannot promise. The layer must be \
             polygons that form a valid coverage (see vector.coverage_validate). `tolerance` is in the layer's \
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
}
