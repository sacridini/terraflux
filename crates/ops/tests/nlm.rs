//! The neutral landscape models against the library's (tests/data/reference/nlm.npz and .json,
//! tools/make_nlm_reference.py): every model (R's generator and NLMR's C++ ones, with and without NLMR's quirks;
//! the Voronoi mosaics with each germ's own value) and the utilities (classify, binarize, merge, rescale), to the
//! bit.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Stack, Value};
use terraflux_raster::Grid;

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn check(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if let Some(k) = got.iter().zip(want).position(|(a, b)| !same(*a, *b)) {
        panic!("{what}: at {k}: {} {}", got[k], want[k]);
    }
}

fn run(op: &str, inputs: Vec<(String, Value)>, params: &serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: inputs.into_iter().collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn values(v: &Value) -> Vec<f64> {
    match v {
        Value::Map(m) => m.data.to_vec(),
        Value::Stack(s) => s.layers.iter().flat_map(|l| l.iter().copied()).collect(),
        v => panic!("{v:?}"),
    }
}

#[test]
fn models_and_utilities_match_the_library() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let z = terraflux_raster::npy::read_npz(root.join("tests/data/reference/nlm.npz")).unwrap();
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("tests/data/reference/nlm.json")).unwrap()).unwrap();
    let grid = {
        let x = &z["in/x"];
        Arc::new(Grid::new(x.shape[1], x.shape[0]))
    };
    let map = |name: &str| Value::Map(Map::new(grid.clone(), z[&format!("in/{name}")].data.clone()));
    for c in cases.as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let mut inputs = Vec::new();
        for (port, src) in c["inputs"].as_object().unwrap() {
            let v = match src {
                serde_json::Value::Array(names) => Value::Stack(Stack {
                    grid: grid.clone(),
                    keys: names.iter().map(|n| terraflux_ops::Key::Name(n.as_str().unwrap().into())).collect(),
                    layers: names
                        .iter()
                        .map(|n| Arc::new(z[&format!("in/{}", n.as_str().unwrap())].data.clone()))
                        .collect(),
                }),
                s => map(s.as_str().unwrap()),
            };
            inputs.push((port.clone(), v));
        }
        let out = run(c["op"].as_str().unwrap(), inputs, &c["params"]);
        // the infinite field alone has no sum
        let port = match c["op"].as_str().unwrap() {
            "nlm.classify" => "classes",
            _ if !out.contains_key("map") => "infinit",
            _ => "map",
        };
        let got = values(&out[port]);
        let mut want = z[name].data.clone();
        if c["op"] == "nlm.classify" || c["op"] == "nlm.binarize" {
            // The library's 0 (no class) is a missing cell here
            want.iter_mut().filter(|v| **v == 0.0).for_each(|v| *v = f64::NAN);
        }
        check(name, &got, &want);
        if let Value::Map(m) = &out[port] {
            let shape: Vec<usize> =
                c["shape"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect();
            assert_eq!(vec![m.grid.height, m.grid.width], shape, "{name}: shape");
        }
        for extra in ["steps", "infinit"] {
            if let Some(w) = z.get(&format!("{name}/{extra}")) {
                let got = out.get(extra).unwrap_or_else(|| panic!("{name}: no {extra}"));
                check(&format!("{name} {extra}"), &values(got), &w.data);
            }
        }
        if let Some(w) = z.get(&format!("{name}_direction")) {
            let Value::Number(d) = out["direction"] else { panic!("{name}: no direction") };
            check(&format!("{name} direction"), &[d], &w.data);
        }
    }
}

/// A drawn map keeps the coordinate system asked for (the models that need places, like swat.watershed, ask for
/// one) and the same values: only the grid's georeference changes.
#[test]
fn a_drawn_map_takes_a_crs() {
    let plain = run("nlm.fbm", vec![], &serde_json::json!({ "rows": 12, "columns": 10, "seed": 5, "resolution": 30 }));
    let placed = run(
        "nlm.fbm",
        vec![],
        &serde_json::json!({ "rows": 12, "columns": 10, "seed": 5, "resolution": 30, "crs": "EPSG:32722" }),
    );
    let (Value::Map(a), Value::Map(b)) = (&plain["map"], &placed["map"]) else { panic!("no maps") };
    assert!(terraflux_ops::prepare::crs_of(&a.grid).is_none());
    assert!(terraflux_ops::prepare::crs_of(&b.grid).is_some_and(|c| c.contains("32722") || c.contains("UTM")));
    check("the same values", &b.data, &a.data);
}

/// 100 x 100 cells of five blocky classes (columns in fifths), the first cell of the last row NaN if asked.
fn blocks(nan: bool) -> Value {
    let mut v: Vec<f64> = (0..100 * 100).map(|i| ((i % 100) / 20 + 1) as f64).collect();
    if nan {
        v[0] = f64::NAN;
    }
    Value::Map(Map::new(Arc::new(Grid::new(100, 100)), v))
}

fn image(classes: Value, params: serde_json::Value) -> Outputs {
    run("nlm.image", vec![("classes".into(), classes)], &params)
}

fn layers(o: &Outputs) -> Vec<Vec<f64>> {
    let Value::Stack(s) = &o["image"] else { panic!("no image") };
    s.layers.iter().map(|l| l.to_vec()).collect()
}

const SIG: [[f64; 6]; 5] = [
    [0.06, 0.05, 0.03, 0.02, 0.01, 0.005],
    [0.03, 0.05, 0.03, 0.30, 0.15, 0.07],
    [0.05, 0.08, 0.07, 0.30, 0.25, 0.15],
    [0.07, 0.10, 0.12, 0.25, 0.30, 0.22],
    [0.12, 0.13, 0.14, 0.18, 0.22, 0.20],
];

#[test]
fn image_is_deterministic_for_a_seed() {
    let a = layers(&image(blocks(false), serde_json::json!({ "seed": 3 })));
    let b = layers(&image(blocks(false), serde_json::json!({ "seed": 3 })));
    let c = layers(&image(blocks(false), serde_json::json!({ "seed": 4 })));
    assert_eq!(a.len(), 6);
    for k in 0..6 {
        check("same seed", &a[k], &b[k]);
    }
    assert!(a[3] != c[3]);
}

#[test]
fn image_without_noise_texture_blur_is_the_signature() {
    let o = image(blocks(true), serde_json::json!({ "noise": 0, "texture": 0, "blur": 0 }));
    let l = layers(&o);
    for (b, band) in l.iter().enumerate() {
        assert!(band[0].is_nan());
        for i in 1..band.len() {
            assert_eq!(band[i], SIG[(i % 100) / 20][b], "band {b} cell {i}");
        }
    }
    let Value::Table(t) = &o["signatures"] else { panic!("no table") };
    assert_eq!(t.columns[..3], ["class", "name", "blue"]);
    assert_eq!(t.label("name", t.rows[1][1]), Some("forest"));
    assert_eq!(values(&o["truth"]).len(), 10_000);
}

#[test]
fn image_with_defaults_keeps_class_means_and_ndvi() {
    let l = layers(&image(blocks(false), serde_json::json!({ "seed": 2 })));
    for (c, sig) in SIG.iter().enumerate() {
        // the middle of each block, away from the blurred edges
        let cells: Vec<usize> = (0..10_000).filter(|i| (i % 100) / 20 == c && (i % 20) >= 2 && (i % 20) < 18).collect();
        for (b, band) in l.iter().enumerate() {
            let mean = cells.iter().map(|&i| band[i]).sum::<f64>() / cells.len() as f64;
            assert!((mean - sig[b]).abs() < 0.01, "class {c} band {b}: {mean}");
        }
    }
    let ndvi = |c: usize| {
        let cells: Vec<usize> = (0..10_000).filter(|i| (i % 100) / 20 == c).collect();
        let m = |b: usize| cells.iter().map(|&i| l[b][i]).sum::<f64>() / cells.len() as f64;
        (m(3) - m(2)) / (m(3) + m(2))
    };
    assert!(ndvi(1) > 0.7, "forest {}", ndvi(1));
    assert!(ndvi(0) < 0.0, "water {}", ndvi(0));
}

#[test]
fn image_checks_classes_and_picks_bands() {
    let r = Registry::standard();
    let params = serde_json::json!({}).as_object().cloned().unwrap();
    let mut classes = blocks(false);
    if let Value::Map(m) = &mut classes {
        let mut v = m.data.to_vec();
        v[5] = 9.0;
        *m = Map::new(m.grid.clone(), v);
    }
    let args = Args { node: "n", inputs: [("classes".to_string(), classes)].into_iter().collect(), params: &params };
    let e = r.get("nlm.image").unwrap().run(&mut Ctx::default(), &args).unwrap_err().to_string();
    assert!(e.contains("signatures") && e.contains('9'), "{e}");
    let o = image(blocks(false), serde_json::json!({ "bands": ["red", "nir"], "noise": 0, "texture": 0, "blur": 0 }));
    let Value::Stack(s) = &o["image"] else { panic!("no image") };
    assert_eq!(s.names(), ["red", "nir"]);
    assert_eq!(s.layers[1][0], 0.02);
}
