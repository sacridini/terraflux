//! The spatial functions of vector.calc and vector.filter (within(x), count(x, "contains"), distance(x)) through the
//! registry, with a vector on an extra input, against the library's tf.vector.relate, tf.vector.count and
//! tf.vector.distance on a seeded case of polygons, points and lines on an integer grid
//! (tests/data/reference/vector_calc.json and vectors/calc_*.gpkg, tools/make_vector_calc_reference.py).
//! Booleans and counts are exact; distances are GEOS's on both sides and are compared to 1e-12 relative.
//! Skipped without GDAL.

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value as Json, json};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Value};
use terraflux_raster::vector::Vector;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn run(op: &str, ins: Vec<(&str, Value)>, params: Json) -> terraflux_ops::Result<Outputs> {
    let args = Args {
        node: "n",
        inputs: ins.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        params: params.as_object().unwrap(),
    };
    Registry::standard().get(op).unwrap().run(&mut Ctx::default(), &args)
}

fn read(name: &str) -> Arc<Vector> {
    let path = root().join(format!("tests/data/reference/vectors/calc_{name}.gpkg"));
    let out = run("vector.read", vec![], json!({"path": path.to_string_lossy()})).unwrap();
    let Value::Vector(v) = &out["vector"] else { panic!() };
    v.clone()
}

fn vector(out: &Outputs) -> Arc<Vector> {
    let Value::Vector(v) = &out["vector"] else { panic!() };
    v.clone()
}

fn has_gdal() -> bool {
    if let Err(e) = terraflux_gdal::gdal() {
        eprintln!("skipped: {e}");
        return false;
    }
    true
}

fn expected(c: &Json) -> Vec<f64> {
    c["expected"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect()
}

/// The script that does what a case does (`x` is the other layer).
fn script(c: &Json) -> String {
    let p = c["predicate"].as_str();
    let d = c["distance"].as_f64();
    match (c["fn"].as_str().unwrap(), p, d) {
        ("distance", ..) => "distance(x)".into(),
        ("relate", Some("dwithin"), Some(d)) => format!("dwithin(x, {d})"),
        ("relate", Some(p), _) => format!("{p}(x)"),
        ("count", Some("dwithin"), Some(d)) => format!("count(x, \"dwithin\", {d})"),
        ("count", Some(p), _) => format!("count(x, \"{p}\")"),
        _ => unreachable!(),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b || (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
}

#[test]
fn calc_matches_the_library() {
    if !has_gdal() {
        return;
    }
    let r: Json =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/vector_calc.json")).unwrap())
            .unwrap();
    let layers: Vec<(&str, Arc<Vector>)> = ["zones", "sites", "lines"].iter().map(|n| (*n, read(n))).collect();
    let layer = |n: &str| layers.iter().find(|(k, _)| *k == n).unwrap().1.clone();
    let pop = r["pop"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect::<Vec<_>>();
    assert_eq!(layer("zones").column("pop").unwrap().numbers(), pop);
    let (mut booleans, mut counts, mut distances) = (0, 0, 0);
    for c in r["cases"].as_array().unwrap() {
        let (a, b) = (layer(c["a"].as_str().unwrap()), layer(c["b"].as_str().unwrap()));
        let want = expected(c);
        let s = script(c);
        let what = format!("{} of {} over {}: {s}", c["fn"], c["a"], c["b"]);
        let ins = || vec![("vector", Value::Vector(a.clone())), ("x", Value::Vector(b.clone()))];
        let out = run("vector.calc", ins(), json!({"expression": s, "column": "r"}))
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        let got = vector(&out).column("r").unwrap().numbers();
        assert_eq!(got.len(), want.len(), "{what}");
        for (i, (g, w)) in got.iter().zip(&want).enumerate() {
            let ok = if c["fn"] == "distance" { close(*g, *w) } else { g == w };
            assert!(ok, "{what}: feature {i}: {g} {w}");
        }
        match c["fn"].as_str().unwrap() {
            "relate" => booleans += 1,
            "count" => counts += 1,
            _ => distances += 1,
        }
        // the filter keeps the features where it holds (a predicate), or where it is at least one (a count)
        if c["fn"] != "distance" {
            let cond = if c["fn"] == "relate" { s.clone() } else { format!("{s} > 0") };
            let out = run("vector.filter", ins(), json!({"expression": format!("{cond} and feature > 2")}))
                .unwrap_or_else(|e| panic!("{what}: {e}"));
            let kept = vector(&out);
            let keep: Vec<usize> = (2..want.len()).filter(|&i| want[i] > 0.0).collect();
            assert_eq!(kept.len(), keep.len(), "{what}: filter");
            if a.column("pop").is_some() {
                let pops = a.column("pop").unwrap().numbers();
                assert_eq!(
                    kept.column("pop").unwrap().numbers(),
                    keep.iter().map(|&i| pops[i]).collect::<Vec<_>>(),
                    "{what}"
                );
            }
        }
    }
    // (11 predicates and a distance each way for six pairs of layers)
    assert_eq!((booleans, counts, distances), (72, 72, 6), "the cases");
}

#[test]
fn calc_in_one_script() {
    if !has_gdal() {
        return;
    }
    let (zones, sites, lines) = (read("zones"), read("sites"), read("lines"));
    let ins = || {
        vec![
            ("vector", Value::Vector(zones.clone())),
            ("sites", Value::Vector(sites.clone())),
            ("roads", Value::Vector(lines.clone())),
            ("d", Value::Number(3.0)),
        ]
    };
    let one = |text: &str| -> Vec<f64> {
        let out = run("vector.calc", ins(), json!({"expression": text, "column": "r"})).unwrap();
        vector(&out).column("r").unwrap().numbers()
    };
    // out lines of several calls, each result the one of the call alone
    let out = run(
        "vector.calc",
        ins(),
        json!({"expression": "out n = count(sites, \"contains\")\nout near = dwithin(roads, d)\nout dist = distance(roads)\n\
                              out both = n > 0 and near"}),
    )
    .unwrap();
    let v = vector(&out);
    let (n, near, dist, both) = (
        v.column("n").unwrap().numbers(),
        v.column("near").unwrap().numbers(),
        v.column("dist").unwrap().numbers(),
        v.column("both").unwrap().numbers(),
    );
    assert_eq!(n, one("count(sites, \"contains\")"));
    assert_eq!(near, one("dwithin(roads, 3)"));
    assert_eq!(dist, one("distance(roads)"));
    for i in 0..n.len() {
        assert_eq!(both[i], (n[i] > 0.0 && near[i] != 0.0) as u8 as f64);
        assert_eq!(near[i] != 0.0, dist[i] <= 3.0, "feature {i}");
    }
    // errors name the node and what to write
    let e = run("vector.calc", ins(), json!({"expression": "sites + 1", "column": "r"})).unwrap_err().0;
    assert!(e.contains("node \"n\"") && e.contains("sites is a layer"), "{e}");
    let e = run("vector.filter", ins(), json!({"expression": "within(parks)"})).unwrap_err().0;
    assert!(e.contains("not a connected layer (the layers: roads, sites"), "{e}");
}
