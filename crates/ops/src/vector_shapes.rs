//! Operators on the shape of each feature (`vector.centroid`, `vector.hull`, `vector.simplify`, `vector.envelope`,
//! `vector.make_valid`, `vector.explode`, `vector.boundary`): GEOS's own functions through terraflux_gdal::geos,
//! as shapely and the library's `tf.vector` call them, with the attributes kept.

use std::sync::Arc;

use terraflux_gdal::geos::{Unary, unary, union};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::vector::{attribute_table, vector_of};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Centroid));
    r.add(Arc::new(Hull));
    r.add(Arc::new(Simplify));
    r.add(Arc::new(Envelope));
    r.add(Arc::new(MakeValid));
    r.add(Arc::new(Explode));
    r.add(Arc::new(Boundary));
}

fn give(v: Vector) -> Outputs {
    let t = attribute_table(&v);
    outputs([("vector", Value::Vector(Arc::new(v))), ("table", Value::Table(t))])
}

fn err(node: &str) -> impl Fn(String) -> Error + '_ {
    move |m| Error::msg(format!("node {node:?}: {m}"))
}

/// The signature of an operator of one layer to one layer (and its table).
fn shape_signature(name: &str, doc: &str, out: &str) -> OpSignature {
    OpSignature::new(name, doc, Access::None)
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .output(PortSpec::new("vector", Type::Vector, out))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
}

/// The features with their geometries replaced by `op` of them (the attributes kept).
fn mapped(args: &Args, op: Unary) -> Result<Outputs> {
    let v = vector_of(args, "vector")?;
    let geometries = unary(&v.geometries, op).map_err(err(args.node))?;
    Ok(give(Vector { geometries, attributes: v.attributes.clone(), crs: v.crs.clone() }))
}

/// `vector.centroid`.
struct Centroid;

impl Operator for Centroid {
    fn signature(&self) -> OpSignature {
        shape_signature(
            "vector.centroid",
            "The centroid of every feature as a point (GEOS's, as PostGIS's ST_Centroid and QGIS's Centroids), \
             with their attributes; with `inside`, a point guaranteed to lie on the feature (PostGIS's \
             ST_PointOnSurface, QGIS's Point on surface): the centroid of a concave polygon (a C, an L) can fall \
             outside it. Empty features stay empty.",
            "the points",
        )
        .param(PortSpec::optional("inside", Type::Bool, "a point on the feature, not its centroid (false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let op = if args.param("inside", false)? { Unary::PointOnSurface } else { Unary::Centroid };
        mapped(args, op)
    }
}

/// `vector.hull`.
struct Hull;

impl Operator for Hull {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.hull",
            "The convex or concave hull of every feature (PostGIS's ST_ConvexHull and ST_ConcaveHull, QGIS's \
             Convex hull and Concave hull), with their attributes. With `by`, one hull per value of an attribute \
             over the union of its features, in the order of their first appearance, with that value and count (the \
             features of the group, as vector.dissolve); with `dissolve`, one hull of every feature (only count). \
             The concave hull needs GEOS 3.11: its `ratio` is 1 for the convex hull and 0 for the tightest, \
             `allow_holes` lets it have holes.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the features"))
        .output(PortSpec::new("vector", Type::Vector, "the hulls"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional("kind", Type::Text, "convex or concave (convex)").choices(["convex", "concave"]))
        .param(PortSpec::optional("ratio", Type::Number, "the concave hull's ratio, 0 to 1 (0.3)"))
        .param(PortSpec::optional("allow_holes", Type::Bool, "the concave hull may have holes (false)"))
        .param(PortSpec::optional("by", Type::Text, "the attribute: one hull per value of it"))
        .param(PortSpec::optional("dissolve", Type::Bool, "one hull of every feature (false); not with `by`"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let v = vector_of(args, "vector")?;
        let kind: String = args.param("kind", "convex".to_string())?;
        let op = match kind.trim().to_lowercase().as_str() {
            "convex" => Unary::ConvexHull,
            "concave" => {
                Unary::ConcaveHull { ratio: args.param("ratio", 0.3)?, allow_holes: args.param("allow_holes", false)? }
            }
            _ => return Err(e(format!("unknown hull kind {kind:?} (convex or concave)"))),
        };
        let by: Option<String> = args.opt_param::<String>("by")?.filter(|b| !b.is_empty());
        let dissolve: bool = args.param("dissolve", false)?;
        if by.is_some() && dissolve {
            return Err(e("by and dissolve cannot be used together".into()));
        }
        if by.is_none() && !dissolve {
            return mapped(args, op);
        }
        // the groups in the order of their first feature
        let keys: Vec<String> = match &by {
            None => vec![String::new(); v.len()],
            Some(b) => match v.column(b) {
                Some(Column::Numbers(x)) => x.iter().map(|x| x.to_bits().to_string()).collect(),
                Some(Column::Texts(t)) => t.clone(),
                None => return Err(e(format!("the features have no attribute {b:?}"))),
            },
        };
        let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
        for (i, k) in keys.iter().enumerate() {
            match groups.iter_mut().find(|(g, _)| g == k) {
                Some((_, m)) => m.push(i),
                None => groups.push((k, vec![i])),
            }
        }
        let merged = groups
            .iter()
            .map(|(_, m)| {
                let geoms: Vec<Geometry> = m.iter().map(|&i| v.geometries[i].clone()).collect();
                union(&geoms)
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(&e)?;
        let geometries = unary(&merged, op).map_err(&e)?;
        let mut attributes = Vec::new();
        if let Some(b) = &by {
            let first: Vec<usize> = groups.iter().map(|(_, m)| m[0]).collect();
            let col = match v.column(b).expect("checked above") {
                Column::Numbers(x) => Column::Numbers(first.iter().map(|&i| x[i]).collect()),
                Column::Texts(t) => Column::Texts(first.iter().map(|&i| t[i].clone()).collect()),
            };
            attributes.push((b.clone(), col));
        }
        attributes.push(("count".into(), Column::Numbers(groups.iter().map(|(_, m)| m.len() as f64).collect())));
        Ok(give(Vector { geometries, attributes, crs: v.crs.clone() }))
    }
}

/// `vector.simplify`.
struct Simplify;

impl Operator for Simplify {
    fn signature(&self) -> OpSignature {
        shape_signature(
            "vector.simplify",
            "Every geometry simplified with Douglas-Peucker (GEOS's, as PostGIS's ST_SimplifyPreserveTopology and \
             ST_Simplify, QGIS's Simplify): the vertices that deviate less than `tolerance` (in the units of the \
             layer) from the line between their neighbours are dropped. With `preserve_topology` (the default) the \
             result stays valid: no ring collapses or crosses another.",
            "the simplified features",
        )
        .param(PortSpec::new("tolerance", Type::Number, "the largest deviation dropped, in the layer's units"))
        .param(PortSpec::optional("preserve_topology", Type::Bool, "keep the geometries valid (true)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let tolerance: f64 = args
            .opt_param("tolerance")?
            .ok_or_else(|| Error::msg(format!("node {:?}: give the tolerance", args.node)))?;
        let op = if args.param("preserve_topology", true)? {
            Unary::TopologyPreserveSimplify(tolerance)
        } else {
            Unary::Simplify(tolerance)
        };
        mapped(args, op)
    }
}

/// `vector.envelope`.
struct Envelope;

impl Operator for Envelope {
    fn signature(&self) -> OpSignature {
        shape_signature(
            "vector.envelope",
            "The bounding box of every feature as a polygon (PostGIS's ST_Envelope, QGIS's Bounding boxes), with \
             their attributes. As GEOS does, the envelope of a point is the point and that of a horizontal or \
             vertical line a line.",
            "the bounding boxes",
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        mapped(args, Unary::Envelope)
    }
}

/// `vector.make_valid`.
struct MakeValid;

impl Operator for MakeValid {
    fn signature(&self) -> OpSignature {
        shape_signature(
            "vector.make_valid",
            "Every invalid geometry repaired (GEOS's linework method, as PostGIS's ST_MakeValid and QGIS's Fix \
             geometries): a bow-tie polygon becomes a multipolygon of its two parts. Valid geometries are kept as \
             they are. Where the repair leaves a mix (a polygon and a line), the parts of the highest dimension \
             are kept.",
            "the repaired features",
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        mapped(args, Unary::MakeValid)
    }
}

/// `vector.boundary`.
struct Boundary;

impl Operator for Boundary {
    fn signature(&self) -> OpSignature {
        shape_signature(
            "vector.boundary",
            "The boundary of every feature (PostGIS's ST_Boundary, QGIS's Boundary): the rings of a polygon as \
             lines, the end points of a line as points (a closed line has none: it is empty); the boundary of a \
             point is empty. The attributes are kept.",
            "the boundaries",
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        mapped(args, Unary::Boundary)
    }
}

/// The single parts of a geometry (itself when it has one part; an empty one gives one empty part).
fn parts(g: &Geometry) -> Vec<Geometry> {
    // a geometry without a vertex (an empty polygon) is the empty geometry
    if g.bounds().is_none() {
        return vec![Geometry::Empty];
    }
    let many = match g {
        Geometry::MultiPoint(p) => p.iter().map(|&p| Geometry::Point(p)).collect(),
        Geometry::MultiLineString(l) => l.iter().map(|l| Geometry::LineString(l.clone())).collect(),
        Geometry::MultiPolygon(p) => p.iter().map(|p| Geometry::Polygon(p.clone())).collect(),
        _ => vec![g.clone()],
    };
    if many.is_empty() { vec![Geometry::Empty] } else { many }
}

/// `vector.explode`.
struct Explode;

impl Operator for Explode {
    fn signature(&self) -> OpSignature {
        shape_signature(
            "vector.explode",
            "Multi-part features split into single parts (PostGIS's ST_Dump, QGIS's Multipart to singleparts): \
             one feature per part with the attributes repeated, in the order of the features and, in each, of the \
             parts. Single-part features are kept as they are, and an empty feature stays one empty feature.",
            "the parts",
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = vector_of(args, "vector")?;
        let mut from = Vec::with_capacity(v.len());
        let mut geometries = Vec::with_capacity(v.len());
        for (i, g) in v.geometries.iter().enumerate() {
            for p in parts(g) {
                from.push(i);
                geometries.push(p);
            }
        }
        let mut out = v.select(&from);
        out.geometries = geometries;
        Ok(give(out))
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

    fn layer() -> Vector {
        Vector {
            geometries: vec![
                rect(0.0, 0.0, 2.0, 2.0),
                Geometry::MultiPoint(vec![[1.0, 1.0], [5.0, 5.0], [9.0, 1.0]]),
                Geometry::Empty,
                rect(1.0, 0.0, 4.0, 2.0),
            ],
            attributes: vec![
                ("id".into(), Column::Numbers(vec![1.0, 2.0, 3.0, 4.0])),
                ("g".into(), Column::Texts(vec!["a".into(), "b".into(), "a".into(), "a".into()])),
            ],
            crs: Some("EPSG:32633".into()),
        }
    }

    fn ready() -> bool {
        terraflux_gdal::geos::geos().is_ok()
    }

    #[test]
    fn explode_splits_the_parts_and_keeps_empties() {
        let o = run(&Explode, layer(), serde_json::json!({})).unwrap();
        let v = vector(&o);
        assert_eq!(v.len(), 6);
        assert_eq!(v.column("id").unwrap().numbers(), vec![1.0, 2.0, 2.0, 2.0, 3.0, 4.0]);
        assert_eq!(v.geometries[3], Geometry::Point([9.0, 1.0]));
        assert_eq!(v.geometries[4], Geometry::Empty);
        assert_eq!(v.crs.as_deref(), Some("EPSG:32633"));
    }

    #[test]
    fn centroid_and_envelope_keep_the_attributes() {
        if !ready() {
            return;
        }
        let o = run(&Centroid, layer(), serde_json::json!({})).unwrap();
        let v = vector(&o);
        assert_eq!(v.geometries[0], Geometry::Point([1.0, 1.0]));
        assert_eq!(v.geometries[2], Geometry::Empty);
        assert_eq!(v.column("id").unwrap().numbers(), vec![1.0, 2.0, 3.0, 4.0]);
        assert!(o.contains_key("table"));
        let e = vector(&run(&Envelope, layer(), serde_json::json!({})).unwrap());
        assert_eq!(e.geometries[1], rect(1.0, 1.0, 9.0, 5.0));
    }

    #[test]
    fn hulls_by_group_and_dissolved() {
        if !ready() {
            return;
        }
        let by = vector(&run(&Hull, layer(), serde_json::json!({"by": "g"})).unwrap());
        assert_eq!(by.len(), 2);
        assert!(matches!(by.column("g"), Some(Column::Texts(t)) if t == &["a", "b"]));
        assert_eq!(by.column("count").unwrap().numbers(), vec![3.0, 1.0]);
        // the union of the two overlapping squares is one 4 x 2 rectangle
        assert_eq!(by.geometries[0].bounds(), Some([0.0, 0.0, 4.0, 2.0]));
        let all = vector(&run(&Hull, layer(), serde_json::json!({"dissolve": true})).unwrap());
        assert_eq!(all.len(), 1);
        assert!(all.column("g").is_none());
        assert_eq!(all.column("count").unwrap().numbers(), vec![4.0]);
        assert!(run(&Hull, layer(), serde_json::json!({"dissolve": true, "by": "g"})).is_err());
        assert!(run(&Hull, layer(), serde_json::json!({"kind": "round"})).is_err());
        assert!(run(&Hull, layer(), serde_json::json!({"by": "nope"})).is_err());
    }

    #[test]
    fn simplify_needs_a_tolerance() {
        if !ready() {
            return;
        }
        let line = Vector {
            geometries: vec![Geometry::LineString(vec![[0.0, 0.0], [1.0, 0.1], [2.0, 0.0]])],
            ..Vector::default()
        };
        assert!(run(&Simplify, line.clone(), serde_json::json!({})).is_err());
        let v = vector(&run(&Simplify, line, serde_json::json!({"tolerance": 0.5})).unwrap());
        assert_eq!(v.geometries[0], Geometry::LineString(vec![[0.0, 0.0], [2.0, 0.0]]));
    }
}
