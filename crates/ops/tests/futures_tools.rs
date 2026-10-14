//! FUTURES' inputs against landschaft's (tests/data/reference/futures_tools.npz and .json,
//! tools/make_futures_tools_reference.py): the development pressure (gravity, occurrence, kernel; padded or not)
//! and the patch library (whole, with a threshold, in subregions, per subregion), to the bit (the kernel, whose
//! numpy has an exponential of its own, to the last bit of a float).

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::Grid;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

#[test]
fn futures_inputs_are_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("futures_tools.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("futures_tools.json")).unwrap()).unwrap();
    let mut g = Grid::new(60, 45);
    g.transform = [1000.0, 30.0, 0.0, 5000.0, 0.0, -30.0];
    let g = Arc::new(g);
    let map = |k: &str| Value::Map(Map::new(g.clone(), z[k].data.clone()));
    for (name, kw) in meta["pressure"].as_object().unwrap() {
        let out = run("futures.devpressure", vec![("developed", map("in/start"))], kw.clone());
        let Value::Map(m) = &out["map"] else { panic!() };
        let want = &z[format!("pressure/{name}").as_str()].data;
        for (k, (a, b)) in m.data.iter().zip(want).enumerate() {
            let same = a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan());
            let close = (a - b).abs() <= 1e-6 * b.abs().max(1.0);
            assert!(same || (name == "kernel" && close), "{name}: cell {k}: {a} {b}");
        }
    }
    for (name, c) in meta["library"].as_object().unwrap() {
        let mut s = c["settings"].clone();
        let mut inputs = vec![("start", map("in/start")), ("end", map("in/end"))];
        if s.as_object_mut().unwrap().remove("_sub").is_some() {
            inputs.push(("subregions", map("in/sub")));
        }
        let out = run("futures.patch_library", inputs, s);
        let Value::Json(lib) = &out["library"] else { panic!() };
        match &c["library"] {
            serde_json::Value::Array(want) => {
                assert!(lib["ids"].is_null(), "{name}");
                let got: Vec<f64> = lib["lists"][0].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
                let want: Vec<f64> = want.iter().map(|v| v.as_f64().unwrap()).collect();
                assert_eq!(got, want, "{name}");
            }
            serde_json::Value::Object(want) => {
                let ids: Vec<String> =
                    lib["ids"].as_array().unwrap().iter().map(|v| (v.as_f64().unwrap() as i64).to_string()).collect();
                assert_eq!(ids, want.keys().cloned().collect::<Vec<_>>(), "{name}");
                for (k, id) in ids.iter().enumerate() {
                    let got: Vec<f64> =
                        lib["lists"][k].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
                    let w: Vec<f64> = want[id].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
                    assert_eq!(got, w, "{name} {id}");
                }
            }
            _ => panic!(),
        }
    }
}
