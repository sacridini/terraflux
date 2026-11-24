//! `vector.shape`: shape descriptors of polygons, through GEOS (terraflux_gdal::geos::shape), bit for bit as the
//! library's `tf.vector.shape` (`_vector_shapes.py`), with the attributes kept.

use std::sync::Arc;

use terraflux_gdal::geos::{Draw, shape};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::vector::{attribute_table, vector_of};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Shape));
}

fn give(v: Vector) -> Outputs {
    let t = attribute_table(&v);
    outputs([("vector", Value::Vector(Arc::new(v))), ("table", Value::Table(t))])
}

fn err(node: &str) -> impl Fn(String) -> Error + '_ {
    move |m| Error::msg(format!("node {node:?}: {m}"))
}

/// Whether a geometry is nothing (empty, or without a vertex).
fn nothing(g: &Geometry) -> bool {
    matches!(g, Geometry::Empty) || g.bounds().is_none()
}

/// `name` with `_2` appended while it is taken.
fn free(name: &str, taken: &[String]) -> String {
    let mut new = name.to_string();
    while taken.contains(&new) {
        new.push_str("_2");
    }
    new
}

/// `vector.shape`.
struct Shape;

impl Operator for Shape {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.shape",
            "Shape descriptors of each polygon or multipolygon (the library's tf.vector.shape; PostGIS's \
             ST_MaximumInscribedCircle, ST_MinimumBoundingCircle and ST_OrientedEnvelope; FRAGSTATS's CIRCLE), as \
             new columns in the units of the layer: `inscribed_radius` (the radius of the maximum inscribed circle, \
             the patch's depth: how far its most interior point is from the edge), `bounding_radius` (of the \
             minimum bounding circle), `circle` (1 - area / (pi r^2) of the bounding circle: 0 for a circle, near 1 \
             for a long thin patch), `rect_length` and `rect_width` (the sides of the minimum rotated rectangle), \
             `elongation` (1 - width / length: 0 for a square, near 1 for an elongated patch) and `orientation` \
             (the long side's direction in degrees clockwise from north, 0 up to 180). An empty feature gives NaN \
             everywhere; points and lines are refused. A name the layer already has gets `_2`. `tolerance` stops \
             the search of the inscribed circle (default: the larger side of each feature's envelope over 1000). \
             `geometry` sets what the features become: the feature itself, the `inscribed` circle (a polygon of \
             8 segments a quadrant), the `bounding` circle or the `rectangle`.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the polygons"))
        .output(PortSpec::new("vector", Type::Vector, "the features with their shape columns"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
        .param(PortSpec::optional(
            "tolerance",
            Type::Number,
            "the inscribed circle's search tolerance, in the layer's units (the envelope's larger side / 1000)",
        ))
        .param(PortSpec::optional("geometry", Type::Text, "what the geometry becomes (feature)").choices([
            "feature",
            "inscribed",
            "bounding",
            "rectangle",
        ]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let v = vector_of(args, "vector")?;
        let kind: String = args.param("geometry", "feature".to_string())?;
        let draw = match kind.trim().to_lowercase().as_str() {
            "feature" => Draw::Feature,
            "inscribed" => Draw::Inscribed,
            "bounding" => Draw::Bounding,
            "rectangle" => Draw::Rectangle,
            _ => {
                return Err(e(format!("unknown geometry {kind:?} (one of feature, inscribed, bounding, rectangle)")));
            }
        };
        let tolerance: Option<f64> = args.opt_param("tolerance")?;
        if let Some(t) = tolerance.filter(|t| t.is_nan() || *t < 0.0) {
            return Err(e(format!("tolerance must be 0 or more (0 or none: the envelope's larger side / 1000), not {t}")));
        }
        if v.geometries.iter().any(|g| !nothing(g) && !matches!(g, Geometry::Polygon(_) | Geometry::MultiPolygon(_))) {
            return Err(e("shape needs polygons or multipolygons".into()));
        }
        let shapes = shape(&v.geometries, tolerance, draw).map_err(&e)?;
        let nan = |f: &dyn Fn(&terraflux_gdal::geos::Shape) -> f64| -> Vec<f64> {
            shapes.iter().map(|s| s.as_ref().map_or(f64::NAN, f)).collect()
        };
        let columns = [
            ("inscribed_radius", nan(&|s| s.inscribed_radius)),
            ("bounding_radius", nan(&|s| s.bounding_radius)),
            ("circle", nan(&|s| s.circle)),
            ("rect_length", nan(&|s| s.rect_length)),
            ("rect_width", nan(&|s| s.rect_width)),
            ("elongation", nan(&|s| s.elongation)),
            ("orientation", nan(&|s| s.orientation)),
        ];
        let mut out = Vector::clone(v);
        if draw != Draw::Feature {
            for (g, s) in out.geometries.iter_mut().zip(&shapes) {
                if let Some(d) = s.as_ref().and_then(|s| s.geometry.clone()) {
                    *g = d;
                }
            }
        }
        let mut taken: Vec<String> = v.attributes.iter().map(|(n, _)| n.clone()).collect();
        for (name, x) in columns {
            let new = free(name, &taken);
            taken.push(new.clone());
            out.attributes.push((new, Column::Numbers(x)));
        }
        Ok(give(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(v: Vector) -> Result<Outputs> {
        let none = serde_json::json!({});
        let args = Args {
            node: "n",
            inputs: [("vector".to_string(), Value::Vector(Arc::new(v)))].into_iter().collect(),
            params: none.as_object().unwrap(),
        };
        Shape.run(&mut Ctx::default(), &args)
    }

    #[test]
    fn lines_are_refused() {
        let line = Vector { geometries: vec![Geometry::LineString(vec![[0.0, 0.0], [1.0, 1.0]])], ..Vector::default() };
        assert!(run(line).unwrap_err().to_string().contains("polygons"));
    }
}
