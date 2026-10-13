//! The neutral landscape models against landschaft's (tests/data/reference/nlm.npz and .json,
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
fn models_and_utilities_are_landschafts() {
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
            // landschaft's 0 (no class) is a missing cell here
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
