//! MaxNet against landschaft's (tests/data/reference/sdm_maxnet.npz and .json,
//! tools/make_sdm_maxnet_reference.py): its coefficients (names and values), alpha and entropy with the default
//! classes, linear and quadratic, products, hinges and thresholds, a categorical predictor and the presences
//! kept out of the background; its predictions on a stack and a table (every output, clamped and not), to the
//! bit.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::Grid;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

const NAMES: [&str; 4] = ["temp", "prec", "soil", "cover"];

fn table(rows: &[f64], k: usize) -> Value {
    let mut t = Table::new(NAMES[..k].iter().map(|s| s.to_string()).collect());
    t.rows = rows.chunks(4).map(|r| r[..k].to_vec()).collect();
    Value::Table(t)
}

fn check(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if let Some(k) = got.iter().zip(want).position(|(a, b)| a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan())) {
        panic!("{what}: at {k}: {} {}", got[k], want[k]);
    }
}

#[test]
fn maxnet_is_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_maxnet.npz")).unwrap();
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_maxnet.json")).unwrap()).unwrap();
    let e = &z["in/env"];
    let (ny, nx) = (e.shape[1], e.shape[2]);
    let mut g = Grid::new(nx, ny);
    g.transform = [0.0, 1000.0, 0.0, ny as f64 * 1000.0, 0.0, -1000.0];
    let grid = Arc::new(g);
    for c in cases.as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let k = if c["with_cover"].as_bool().unwrap() { 4 } else { 3 };
        let out = run(
            "sdm.maxnet",
            vec![("presences", table(&z["in/pres"].data, k)), ("background", table(&z["in/bg"].data, k))],
            c["params"].clone(),
        );
        let Value::SdmModel(m) = &out["model"] else { panic!() };
        assert_eq!(m.classes, c["classes"].as_str().unwrap(), "{name}: classes");
        let names: Vec<String> = c["names"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().into()).collect();
        assert_eq!(m.names, names, "{name}: features");
        check(&format!("{name} betas"), &m.betas, &z[&format!("{name}/betas")].data);
        check(&format!("{name} alpha, entropy"), &[m.alpha, m.entropy], &z[&format!("{name}/stats")].data);
        let env = Value::Stack(Stack {
            grid: grid.clone(),
            keys: NAMES[..k].iter().map(|n| Key::Name(n.to_string())).collect(),
            layers: (0..k).map(|b| Arc::new(e.data[b * ny * nx..(b + 1) * ny * nx].to_vec())).collect(),
        });
        for t in ["cloglog", "logistic", "exponential", "link"] {
            let p = run(
                "sdm.maxnet_predict",
                vec![("model", out["model"].clone()), ("env", env.clone())],
                serde_json::json!({"type": t}),
            );
            let Value::Map(map) = &p["prediction"] else { panic!() };
            check(&format!("{name} {t}"), &map.data, &z[&format!("{name}/stack_{t}")].data);
        }
        for (clamp, key) in [(true, "table"), (false, "table_free")] {
            let p = run(
                "sdm.maxnet_predict",
                vec![("model", out["model"].clone()), ("env", table(&z["in/table"].data, k))],
                serde_json::json!({"clamp": clamp}),
            );
            let Value::Table(t) = &p["prediction"] else { panic!() };
            let got: Vec<f64> = t.rows.iter().map(|r| r[0]).collect();
            check(&format!("{name} {key}"), &got, &z[&format!("{name}/{key}")].data);
        }
    }
}
