//! `vector.split`: polygons and lines cut by lines, through GEOS (terraflux_gdal::geos::split), bit for bit as the
//! library's `tf.vector.split` (`_vector_shapes.py`), with the attributes kept.

use std::sync::Arc;

use terraflux_gdal::geos::split;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::vector::{attribute_table, vector_of};
use crate::vector_tools::like;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Split));
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

fn check(v: &Vector, what: &str) -> std::result::Result<(), String> {
    let cuttable = |g: &Geometry| {
        matches!(
            g,
            Geometry::LineString(_) | Geometry::MultiLineString(_) | Geometry::Polygon(_) | Geometry::MultiPolygon(_)
        )
    };
    if v.geometries.iter().any(|g| !nothing(g) && !cuttable(g)) {
        return Err(format!("{what} must hold lines or polygons (no points or collections)"));
    }
    Ok(())
}

/// `name` with `_2` appended while it is taken.
fn free(name: &str, taken: &[String]) -> String {
    let mut new = name.to_string();
    while taken.contains(&new) {
        new.push_str("_2");
    }
    new
}

/// `vector.split`.
struct Split;

impl Operator for Split {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "vector.split",
            "Each polygon or line cut by the lines of another layer, one feature per piece (PostGIS's ST_Split, \
             QGIS's Split with lines; the library's tf.vector.split). The blades are lines, or polygons (a polygon \
             gives its boundary); points raise an error. For each feature the blade is the union of the blades \
             that intersect it. A polygon is cut as shapely's split does: the faces polygonized from the union of \
             its boundary and the blade, those within the polygon (a multipolygon part by part; holes are kept). A \
             line is cut at its crossings with the blade (a stretch along the blade is removed). A feature that is \
             not cut stays as it is, one feature (a blade that only dangles inside, or touches it from outside; a \
             multi-part feature none of whose parts is cut). Each piece keeps the attributes and takes `part` (1 \
             the largest by area, or by length for lines; on a tie the higher, then the more western point on \
             surface) and `parts` (how many pieces the feature gave); a name the layer already has gets `_2`. \
             Empty features stay as one piece. Rows follow the layer, then `part`. The blades are reprojected to \
             the first layer's coordinate system.",
            Access::None,
        )
        .input(PortSpec::new("vector", Type::Vector, "the polygons or lines to cut"))
        .input(PortSpec::new("lines", Type::Vector, "the lines (or polygons, their boundaries) that cut"))
        .output(PortSpec::new("vector", Type::Vector, "the pieces, with part and parts"))
        .output(PortSpec::new("table", Type::Table, "their attributes"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let e = err(args.node);
        let a = vector_of(args, "vector")?;
        let b = like(vector_of(args, "lines")?, &a.crs)?;
        check(a, "vector").map_err(&e)?;
        check(&b, "lines").map_err(&e)?;
        let cut = split(&a.geometries, &b.geometries).map_err(&e)?;
        let (mut rows, mut part, mut parts, mut geoms) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (i, c) in cut.into_iter().enumerate() {
            let pieces = c.unwrap_or_else(|| vec![a.geometries[i].clone()]);
            let n = pieces.len() as f64;
            for (k, p) in pieces.into_iter().enumerate() {
                rows.push(i);
                part.push((k + 1) as f64);
                parts.push(n);
                geoms.push(p);
            }
        }
        let mut out = a.select(&rows);
        out.geometries = geoms;
        let mut taken: Vec<String> = a.attributes.iter().map(|(n, _)| n.clone()).collect();
        for (name, x) in [("part", part), ("parts", parts)] {
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

    fn run(ins: Vec<(&str, Vector)>) -> Result<Outputs> {
        let none = serde_json::json!({});
        let args = Args {
            node: "n",
            inputs: ins.into_iter().map(|(k, v)| (k.to_string(), Value::Vector(Arc::new(v)))).collect(),
            params: none.as_object().unwrap(),
        };
        Split.run(&mut Ctx::default(), &args)
    }

    #[test]
    fn points_are_refused() {
        let pts = Vector { geometries: vec![Geometry::Point([0.0, 0.0])], ..Vector::default() };
        let line = Vector { geometries: vec![Geometry::LineString(vec![[0.0, 0.0], [1.0, 1.0]])], ..Vector::default() };
        assert!(run(vec![("vector", pts.clone()), ("lines", line.clone())]).is_err());
        assert!(run(vec![("vector", line), ("lines", pts)]).is_err());
    }
}
