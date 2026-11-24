//! vector.split against the library's tf.vector.split (tests/data/reference/vector_shape.json,
//! tools/make_vector_shape_reference.py): the same rows in the same order, the same `part` and `parts`, the same
//! attributes, and the same geometries, compared by their WKB bit for bit (GEOS's union, polygonize, difference and
//! point on surface give the same pieces from the same starting vertex in the reference's GEOS and the app's). Skipped
//! without GDAL/GEOS. And the operator refuses points.

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

fn read(file: &str) -> Value {
    let path = root().join(format!("tests/data/reference/vectors/{file}.gpkg"));
    Value::Vector(vector(&run("vector.read", vec![], json!({"path": path.to_string_lossy()})).unwrap()))
}

#[test]
fn split_matches_the_library() {
    if !has_geos() {
        return;
    }
    let r: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/vector_shape.json")).unwrap())
            .unwrap();
    eprintln!("GEOS {}, reference {}", terraflux_gdal::geos::geos().unwrap().version, r["geos"]);
    let mut failures = Vec::new();
    let mut checked = 0;
    for c in r["split"].as_array().unwrap() {
        let (file, lines) = (c["file"].as_str().unwrap(), c["lines"].as_str().unwrap());
        let what = format!("split {file} by {lines}");
        let out = match run("vector.split", vec![("vector", read(file)), ("lines", read(lines))], json!({})) {
            Ok(o) => o,
            Err(e) => {
                failures.push(format!("{what}: {e}"));
                continue;
            }
        };
        let v = vector(&out);
        let want = c["geometries"].as_array().unwrap();
        if v.len() != want.len() {
            failures.push(format!("{what}: {} rows, the library {}", v.len(), want.len()));
            continue;
        }
        for (i, (g, w)) in v.geometries.iter().zip(want).enumerate() {
            let w = expected(w);
            if g.to_wkb() != w.to_wkb() {
                failures.push(format!("{what}: row {i}: {g:?} against {w:?}"));
            }
        }
        // the attributes: the same columns, in order, with the same values (part and parts included)
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
    eprintln!("{checked} cases bit for bit the library's");
    assert!(checked >= 12, "only {checked} cases");
}

#[test]
fn split_refuses_points() {
    if !has_geos() {
        return;
    }
    let pts = Value::Vector(Arc::new(Vector {
        geometries: vec![Geometry::Point([1.0, 1.0])],
        attributes: vec![],
        crs: None,
    }));
    let e = run("vector.split", vec![("vector", pts.clone()), ("lines", read("shape_roads"))], json!({}));
    assert!(e.unwrap_err().contains("lines or polygons"));
    let e = run("vector.split", vec![("vector", read("shape_forest")), ("lines", pts)], json!({}));
    assert!(e.unwrap_err().contains("lines or polygons"));
}
