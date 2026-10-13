//! The choice of predictors and the range of autocorrelation against landschaft's
//! (tests/data/reference/sdm_predictors.npz, .json and .gpkg, tools/make_sdm_predictors_reference.py): VIFs on a
//! sampled projected grid, a lon/lat grid, a small grid and a table; selections by VIF, by correlation (with a
//! predictor kept), Spearman's and within an area; the variograms' ranges, to the bit.

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
    if let Some(k) = got.iter().zip(want).position(|(a, b)| a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan())) {
        panic!("{what}: at {k}: {} {}", got[k], want[k]);
    }
}

const NAMES: [&str; 5] = ["p1", "p2", "p3", "p4", "p5"];

#[test]
fn predictors_are_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_predictors.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_predictors.json")).unwrap()).unwrap();
    let env = |name: &str, bands: &[usize]| -> Value {
        let a = &z[&format!("in/{name}")];
        if name == "table" {
            let mut t = Table::new(NAMES.iter().map(|s| s.to_string()).collect());
            t.rows = a.data.chunks(5).map(|r| r.to_vec()).collect();
            return Value::Table(t);
        }
        let (ny, nx) = (a.shape[1], a.shape[2]);
        let tr: Vec<f64> = meta["transforms"][name].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let mut g = Grid::new(nx, ny);
        g.transform = [tr[0], tr[1], tr[2], tr[3], tr[4], tr[5]];
        Value::Stack(Stack {
            grid: Arc::new(g),
            keys: bands.iter().map(|&b| Key::Name(NAMES[b].to_string())).collect(),
            layers: bands.iter().map(|&b| Arc::new(a.data[b * ny * nx..(b + 1) * ny * nx].to_vec())).collect(),
        })
    };
    let area =
        Value::Vector(Arc::new(terraflux_gdal::read_vector(dir.join("sdm_predictors.gpkg"), Some("area")).unwrap()));
    for c in meta["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let e = c["env"].as_str().unwrap();
        let mut params = c["params"].clone();
        match c["op"].as_str().unwrap() {
            "sdm.vif" => {
                let out = run("sdm.vif", vec![("env", env(e, &[0, 1, 2, 3, 4]))], params);
                let Value::Table(t) = &out["vif"] else { panic!() };
                check(name, &t.rows[0], &z[name].data);
            }
            "sdm.select_predictors" => {
                let mut inputs = vec![("env", env(e, &[0, 1, 2, 3, 4]))];
                if params.as_object_mut().unwrap().remove("area").is_some() {
                    inputs.push(("area", area.clone()));
                }
                let out = run("sdm.select_predictors", inputs, params);
                let Value::Json(sel) = &out["selection"] else { panic!() };
                assert_eq!(sel["kept"], c["kept"], "{name}: kept");
                assert_eq!(sel["excluded"], c["excluded"], "{name}: excluded");
                let vif: Vec<f64> = sel["kept"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|k| sel["vif"][k.as_str().unwrap()].as_f64().unwrap())
                    .collect();
                check(name, &vif, &z[name].data);
            }
            _ => {
                let out = run("sdm.autocorrelation_range", vec![("env", env(e, &[0, 2, 4]))], params);
                let (Value::Number(r), Value::Number(b)) = (&out["range"], &out["block_size"]) else { panic!() };
                check(name, &[*r, *b], &z[name].data);
                let Value::Table(t) = &out["layers"] else { panic!() };
                check(
                    &format!("{name} layers"),
                    &t.rows.iter().flatten().copied().collect::<Vec<_>>(),
                    &z[&format!("{name}/layers")].data,
                );
                let Value::Json(m) = &out["models"] else { panic!() };
                assert_eq!(m, &c["models"], "{name}: models");
            }
        }
    }
}
