//! vector.smooth, vector.coverage_simplify and vector.coverage_validate against the library's tf.vector
//! (tests/data/reference/vector_cover.json, tools/make_vector_cover_reference.py): the same features in the same order,
//! the same attributes and the same coordinates. The smoothing is plain arithmetic: bit for bit always. GEOS's coverage
//! functions are bit for bit when the GEOS version is the reference's (3.13); on another version they are compared
//! by kind, vertex count, bounds and area to 1e-9 (the simplifier may legitimately differ in a minor version) and the
//! count is reported. Skipped without GDAL/GEOS.

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

/// The kind of a geometry, its vertices in order and its area.
fn flat(g: &Geometry) -> (&'static str, Vec<[f64; 2]>, f64) {
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
    let ring = |r: &Vec<[f64; 2]>| r.windows(2).map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1]).sum::<f64>().abs() / 2.0;
    let area = g.polygons().iter().map(|p| ring(&p[0]) - p[1..].iter().map(ring).sum::<f64>()).sum();
    (kind, pts, area)
}

/// Whether two geometries are the same: the same WKB, bit for bit (`weak` false), or the same kind and vertex count
/// with the same bounds and area to 1e-9 relative.
fn same(a: &Geometry, b: &Geometry, weak: bool) -> bool {
    if !weak {
        return a.to_wkb() == b.to_wkb();
    }
    let ((ka, pa, aa), (kb, pb, ab)) = (flat(a), flat(b));
    let close = |x: f64, y: f64| (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0);
    ka == kb
        && pa.len() == pb.len()
        && close(aa, ab)
        && match (a.bounds(), b.bounds()) {
            (Some(p), Some(q)) => (0..4).all(|k| close(p[k], q[k])),
            (p, q) => p == q,
        }
}

#[test]
fn smoothing_and_coverages_match_the_library() {
    if !has_geos() {
        return;
    }
    let r: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/vector_cover.json")).unwrap())
            .unwrap();
    let geos = terraflux_gdal::geos::geos().unwrap().version.clone();
    let covers = terraflux_gdal::geos::geos().unwrap().version_number() >= (3, 12);
    // the reference is GEOS 3.13: on another version the coverage functions may differ in a minor detail
    let same_geos = geos.starts_with(r["geos"].as_str().unwrap_or("?"));
    eprintln!(
        "GEOS {geos}, reference {}: coverage functions {}",
        r["geos"],
        if same_geos { "bit for bit" } else { "weak" }
    );
    let mut failures = Vec::new();
    let (mut checked, mut weakly) = (0, 0);
    for c in r["cases"].as_array().unwrap() {
        let (file, op, params) = (c["file"].as_str().unwrap(), c["op"].as_str().unwrap(), c["params"].clone());
        let coverage = op != "vector.smooth";
        if coverage && !covers {
            continue;
        }
        let weak = coverage && !same_geos;
        let path = root().join(format!("tests/data/reference/vectors/{file}.gpkg"));
        let read = run("vector.read", vec![], json!({"path": path.to_string_lossy()})).unwrap();
        let what = format!("{op} {params} on {file}");
        let out = match run(op, vec![("vector", Value::Vector(vector(&read)))], params) {
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
            if !same(g, &w, weak) {
                failures.push(format!("{what}: feature {i}: {g:?} against {w:?}"));
            } else if !same(g, &w, false) {
                weakly += 1;
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
    assert!(
        failures.is_empty(),
        "{} cases checked, {} failures:
{}",
        checked,
        failures.len(),
        failures.join(
            "
"
        )
    );
    eprintln!("{checked} cases, {weakly} geometries not bit for bit the library's (compared weakly)");
    assert!(checked >= 18, "only {checked} cases");
}
