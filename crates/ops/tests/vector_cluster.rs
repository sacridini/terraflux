//! vector.voronoi, vector.delaunay and vector.cluster against the library's tf.vector
//! (tests/data/reference/vector_cluster.json, tools/make_vector_cluster_reference.py): the same features in the same
//! order, the same attributes and, both on GEOS 3.13, the same coordinates bit for bit. Skipped without GDAL/GEOS.

use std::sync::Arc;

use serde_json::json;
use terraflux_ops::{Args, Ctx, Outputs, Registry, Value};
use terraflux_raster::vector::{Column, Geometry, Vector};

fn root() -> std::path::PathBuf {
    std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).to_path_buf()
}

fn run(op: &str, ins: Vec<(&str, Value)>, params: serde_json::Value) -> Result<Outputs, String> {
    let args = Args {
        node: "n",
        inputs: ins.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        params: params.as_object().unwrap(),
    };
    Registry::standard().get(op).unwrap().run(&mut Ctx::default(), &args).map_err(|e| e.to_string())
}

fn vector(out: &Outputs) -> Arc<Vector> {
    let Value::Vector(v) = &out["vector"] else { panic!() };
    v.clone()
}

fn read(name: &str) -> Value {
    let path = root().join(format!("tests/data/reference/vectors/{name}.gpkg"));
    let out = run("vector.read", vec![], json!({"path": path.to_string_lossy()})).unwrap();
    Value::Vector(vector(&out))
}

fn has_geos() -> bool {
    if let Err(e) = terraflux_gdal::gdal() {
        eprintln!("skipped: {e}");
        return false;
    }
    if let Err(e) = terraflux_gdal::geos::geos() {
        eprintln!("skipped: {e}");
        return false;
    }
    true
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn expected(v: &serde_json::Value) -> Geometry {
    match v.as_str() {
        None | Some("EMPTY") => Geometry::Empty,
        Some(h) => Geometry::from_wkb(&unhex(h)).unwrap(),
    }
}

/// The kind of a geometry and its vertices, in order.
fn flat(g: &Geometry) -> (&'static str, Vec<[f64; 2]>) {
    let kind = match g {
        Geometry::Empty => "empty",
        Geometry::Point(_) => "point",
        Geometry::LineString(_) => "line",
        Geometry::Polygon(_) => "polygon",
        Geometry::MultiPoint(_) => "multipoint",
        Geometry::MultiLineString(_) => "multiline",
        Geometry::MultiPolygon(_) => "multipolygon",
    };
    let mut pts = Vec::new();
    g.points(&mut |p| pts.push(p));
    (kind, pts)
}

/// Whether two geometries are the same: bit for bit (`tol` 0) or the same kind and vertices within `tol` relative to
/// the larger coordinate.
fn same(a: &Geometry, b: &Geometry, tol: f64) -> bool {
    if tol == 0.0 {
        return a.to_wkb() == b.to_wkb();
    }
    let ((ka, pa), (kb, pb)) = (flat(a), flat(b));
    ka == kb
        && pa.len() == pb.len()
        && pa
            .iter()
            .zip(&pb)
            .all(|(p, q)| (0..2).all(|k| (p[k] - q[k]).abs() <= tol * p[k].abs().max(q[k].abs()).max(1.0)))
        && a.to_wkb().len() == b.to_wkb().len()
}

#[test]
fn cluster_operators_match_the_library() {
    if !has_geos() {
        return;
    }
    let r: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/data/reference/vector_cluster.json")).unwrap(),
    )
    .unwrap();
    let geos = terraflux_gdal::geos::geos().unwrap().version.clone();
    // the reference is GEOS 3.13: on another version the coordinates may differ in the last digits
    let tol = if geos.starts_with(r["geos"].as_str().unwrap_or("?")) { 0.0 } else { 1e-12 };
    eprintln!("GEOS {geos}, reference {}: tolerance {tol}", r["geos"]);
    let mut failures = Vec::new();
    let (mut checked, mut inexact) = (0, 0);
    for c in r["cases"].as_array().unwrap() {
        let (file, op, params) = (c["file"].as_str().unwrap(), c["op"].as_str().unwrap(), c["params"].clone());
        let mut ins = vec![("vector", read(file))];
        if let Some(clip) = c["clip"].as_str() {
            ins.push(("clip", read(clip)));
        }
        let what = format!("{op} {params} on {file} (clip {})", c["clip"]);
        let out = match run(op, ins, params) {
            Ok(o) => o,
            Err(e) => {
                failures.push(format!("{what}: {e}"));
                continue;
            }
        };
        let v = vector(&out);
        let want = c["geometries"].as_array().unwrap();
        if v.len() != want.len() {
            failures.push(format!("{what}: {} features, the library {}", v.len(), want.len()));
            continue;
        }
        for (i, (g, w)) in v.geometries.iter().zip(want).enumerate() {
            let w = expected(w);
            if !same(g, &w, tol) {
                failures.push(format!("{what}: feature {i}: {g:?} against {w:?}"));
            } else if !same(g, &w, 0.0) {
                inexact += 1;
            }
        }
        // the attributes: the same columns, in order, with the same values
        let cols = c["columns"].as_array().unwrap();
        let names: Vec<&str> = v.attributes.iter().map(|(n, _)| n.as_str()).collect();
        let theirs: Vec<&str> = cols.iter().map(|p| p[0].as_str().unwrap()).collect();
        if names != theirs {
            failures.push(format!("{what}: columns {names:?}, the library {theirs:?}"));
            continue;
        }
        for ((name, col), pair) in v.attributes.iter().zip(cols) {
            let w = pair[1].as_array().unwrap();
            let ok = match col {
                Column::Numbers(x) => x.iter().zip(w).all(|(a, b)| match b.as_f64() {
                    Some(b) => *a == b,
                    None => a.is_nan(),
                }),
                Column::Texts(t) => t.iter().zip(w).all(|(a, b)| b.as_str().unwrap_or("") == a),
            };
            if !ok {
                failures.push(format!("{what}: column {name:?} differs: {col:?} against {w:?}"));
            }
        }
        checked += 1;
    }
    assert!(failures.is_empty(), "{} cases checked, {} failures:\n{}", checked, failures.len(), failures.join("\n"));
    eprintln!("{checked} cases, {inexact} geometries differ from the library's in the last digits");
    assert!(checked >= 24, "only {checked} cases");
}
