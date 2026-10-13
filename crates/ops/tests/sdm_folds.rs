//! Partitions for cross-validation against landschaft's (tests/data/reference/sdm_folds.npz and .json,
//! tools/make_sdm_folds_reference.py): random, jackknife, quadrant, checkerboard, spatial blocks (hexagons and
//! squares, random, systematic and checkerboard selection), clusters of the coordinates and of the predictors,
//! and buffered folds, to the bit.

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

fn check(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if let Some(k) = got.iter().zip(want).position(|(a, b)| a.to_bits() != b.to_bits()) {
        panic!("{what}: at {k}: {} {}", got[k], want[k]);
    }
}

#[test]
fn folds_are_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_folds.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_folds.json")).unwrap()).unwrap();
    let (lon, lat, y) = (&z["in/lon"].data, &z["in/lat"].data, &z["in/y"].data);
    let records = |projected: bool| {
        let names = if projected { ["x", "y", "presence"] } else { ["lon", "lat", "presence"] };
        let mut t = Table::new(names.iter().map(|s| s.to_string()).collect());
        let f = if projected { 100000.0 } else { 1.0 };
        t.rows = (0..lon.len()).map(|k| vec![lon[k] * f, lat[k] * f, y[k]]).collect();
        Value::Table(t)
    };
    let tr: Vec<f64> = meta["transform"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let mut g = Grid::new(48, 40);
    g.transform = [tr[0], tr[1], tr[2], tr[3], tr[4], tr[5]];
    let n = 48 * 40;
    let e = &z["in/env"].data;
    let env = Value::Stack(Stack {
        grid: Arc::new(g),
        keys: vec![Key::Name("a".into()), Key::Name("b".into())],
        layers: vec![Arc::new(e[..n].to_vec()), Arc::new(e[n..].to_vec())],
    });
    for c in meta["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let mut inputs = vec![("records", records(c["projected"].as_bool().unwrap()))];
        if c["env"].as_bool().unwrap() {
            inputs.push(("env", env.clone()));
        }
        let out = run(c["op"].as_str().unwrap(), inputs, c["params"].clone());
        if c["op"] == "sdm.buffer_folds" {
            let Value::Json(serde_json::Value::Array(folds)) = &out["folds"] else { panic!() };
            let idx = |f: &serde_json::Value, k: &str| -> Vec<f64> {
                f[k].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect()
            };
            let mut got: Vec<f64> = folds.iter().map(|f| idx(f, "train").len() as f64).collect();
            got.extend(folds.iter().map(|f| idx(f, "test").len() as f64));
            got.extend(folds.iter().map(|f| idx(f, "train").iter().sum::<f64>()));
            got.extend(folds.iter().map(|f| idx(f, "test").iter().sum::<f64>()));
            check(name, &got, &z[name].data);
            continue;
        }
        let Value::Table(t) = &out["records"] else { panic!() };
        let got: Vec<f64> = t.rows.iter().map(|r| r[3]).collect();
        check(name, &got, &z[name].data);
    }
}
