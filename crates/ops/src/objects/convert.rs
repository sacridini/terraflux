//! `objects.to_*`: objects read as other kinds of value.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

use super::objects_in;
use crate::vector_tools::polygons_of;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(ToMap));
    r.add(Arc::new(ToVector));
}

/// `objects.to_vector`.
struct ToVector;

/// A closed ring (first point repeated at the end) cut at its corners `times` times (Chaikin's): each side gives
/// the points a quarter and three quarters along it. The ring stays closed.
fn chaikin(ring: Vec<Xy>, times: usize) -> Vec<Xy> {
    let mut pts = ring;
    pts.pop();
    for _ in 0..times {
        let n = pts.len();
        if n < 3 {
            break;
        }
        let mut next = Vec::with_capacity(n * 2);
        for i in 0..n {
            let (p, q) = (pts[i], pts[(i + 1) % n]);
            next.push([0.75 * p[0] + 0.25 * q[0], 0.75 * p[1] + 0.25 * q[1]]);
            next.push([0.25 * p[0] + 0.75 * q[0], 0.25 * p[1] + 0.75 * q[1]]);
        }
        pts = next;
    }
    pts.push(pts[0]);
    pts
}

impl Operator for ToVector {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.to_vector",
            "Objects as polygons: a polygon per object (with its holes; a multipolygon when its cells are in parts), \
             with the object's attributes: id, class (its name, when the objects have classes), parent (when they \
             have a level above) and every column of the table. The boundaries follow the cell edges exactly, or \
             are smoothed by cutting their corners.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects"))
        .output(PortSpec::new("vector", Type::Vector, "a polygon per object, in the objects' coordinate system"))
        .param(PortSpec::optional(
            "smooth",
            Type::Integer,
            "how many times the corners are cut (Chaikin's): 0 (default) keeps the cell edges; each time rounds the \
             boundary more and makes it a little smaller",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let o = objects_in(args, "objects")?;
        let smooth: i64 = args.param("smooth", 0)?;
        if !(0..=10).contains(&smooth) {
            return Err(Error::msg(format!("node {:?}: smooth {smooth}: 0 to 10", args.node)));
        }
        let g = &o.grid;
        let t = g.transform;
        let at = |p: Xy| [t[0] + p[0] * t[1] + p[1] * t[2], t[3] + p[0] * t[4] + p[1] * t[5]];
        // the ids as values, so the patches are the objects (8 neighbours: an object joined at a corner is one)
        let data: Vec<f64> = o.labels.iter().map(|&l| if l == 0 { f64::NAN } else { l as f64 }).collect();
        let mut polys: Vec<Vec<Vec<Vec<Xy>>>> = vec![Vec::new(); o.count];
        for (id, _, ps) in polygons_of(&data, g.width, g.height, true) {
            for p in ps {
                let rings = p
                    .into_iter()
                    .map(|r| {
                        let mut r: Vec<Xy> = r.into_iter().map(at).collect();
                        r.push(r[0]);
                        if smooth > 0 { chaikin(r, smooth as usize) } else { r }
                    })
                    .collect();
                polys[id as usize - 1].push(rings);
            }
        }
        let mut v = Vector { crs: crate::prepare::crs_of(g), ..Default::default() };
        v.geometries = polys
            .into_iter()
            .map(|mut p| if p.len() == 1 { Geometry::Polygon(p.remove(0)) } else { Geometry::MultiPolygon(p) })
            .collect();
        let mut attrs: Vec<(String, Column)> = Vec::new();
        if !o.classes.is_empty() {
            let names = (1..=o.count as u32).map(|id| o.class_name(id).unwrap_or("").to_string()).collect();
            attrs.push(("class".into(), Column::Texts(names)));
        }
        if o.parent.is_some() {
            let parents = (1..=o.count as u32).map(|id| o.parent_id(id).map_or(f64::NAN, f64::from)).collect();
            attrs.push(("parent".into(), Column::Numbers(parents)));
        }
        for (j, name) in o.table.columns.iter().enumerate() {
            let mut name = name.clone();
            while attrs.iter().any(|(n, _)| *n == name) {
                name.push('_');
            }
            let col = if o.table.is_text(&o.table.columns[j]) {
                Column::Texts(
                    o.table
                        .rows
                        .iter()
                        .map(|r| o.table.label(&o.table.columns[j], r[j]).unwrap_or("").to_string())
                        .collect(),
                )
            } else {
                Column::Numbers(o.table.rows.iter().map(|r| r[j]).collect())
            };
            attrs.push((name, col));
        }
        v.attributes = attrs;
        Ok(outputs([("vector", Value::Vector(Arc::new(v)))]))
    }
}

/// `objects.to_map`.
struct ToMap;

impl Operator for ToMap {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.to_map",
            "Objects as a map: each cell takes a value of its object, the object's id, its class number, its parent's \
             id or an attribute of the table. No value where there is no object.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects"))
        .output(PortSpec::new("map", Type::Map, "the value of each cell's object"))
        .param(PortSpec::optional(
            "what",
            Type::Text,
            "id (default); class (the class number, 1 to the number of classes, no value when unclassified); parent \
             (the id of the object above); or the name of a column of the objects' table",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let o = objects_in(args, "objects")?;
        let what: String = args.param("what", "id".to_string())?;
        let some = |x: u32| if x == 0 { f64::NAN } else { x as f64 };
        // a value per object
        let per: Vec<f64> = match what.as_str() {
            "id" => (1..=o.count).map(|k| k as f64).collect(),
            "class" => o.class_of.iter().map(|&k| some(k)).collect(),
            "parent" => {
                if o.parent.is_none() {
                    return Err(Error::msg(format!(
                        "node {node:?}: the objects have no level above (what = \"parent\")"
                    )));
                }
                o.parent_of.iter().map(|&p| some(p)).collect()
            }
            name => o.table.column(name).ok_or_else(|| {
                Error::msg(format!(
                    "node {node:?}: no attribute {name:?} (id, class, parent or a column of the table: {})",
                    o.table.columns.join(", ")
                ))
            })?,
        };
        let data: Vec<f64> = o.labels.iter().map(|&l| if l == 0 { f64::NAN } else { per[l as usize - 1] }).collect();
        Ok(outputs([("map", Value::Map(Map::new(o.grid.clone(), data)))]))
    }
}
