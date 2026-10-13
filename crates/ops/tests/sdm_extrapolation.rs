//! MESS, MOP and ExDet against landschaft's (tests/data/reference/sdm_extrapolation.npz and .json,
//! tools/make_sdm_extrapolation_reference.py): on a stack of three predictors and on a table. MESS and the
//! Euclidean MOP to the bit; the Mahalanobis MOP and ExDet (numpy's covariance, LAPACK's inverse) to rounding.

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

const NAMES: [&str; 3] = ["temp", "prec", "soil"];

fn table(rows: &[f64]) -> Value {
    let mut t = Table::new(NAMES.iter().map(|s| s.to_string()).collect());
    t.rows = rows.chunks(3).map(|r| r.to_vec()).collect();
    Value::Table(t)
}

#[test]
fn extrapolation_is_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_extrapolation.npz")).unwrap();
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_extrapolation.json")).unwrap()).unwrap();
    let e = &z["in/env"];
    let (ny, nx) = (e.shape[1], e.shape[2]);
    let mut g = Grid::new(nx, ny);
    g.transform = [0.0, 1000.0, 0.0, ny as f64 * 1000.0, 0.0, -1000.0];
    let env = Value::Stack(Stack {
        grid: Arc::new(g),
        keys: NAMES.iter().map(|n| Key::Name(n.to_string())).collect(),
        layers: (0..3).map(|k| Arc::new(e.data[k * ny * nx..(k + 1) * ny * nx].to_vec())).collect(),
    });
    let reference = table(&z["in/reference"].data);
    let target = table(&z["in/table"].data);
    for c in cases.as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let on_table = c["target"] == "table";
        let inputs =
            vec![("reference", reference.clone()), ("env", if on_table { target.clone() } else { env.clone() })];
        let out = run(c["op"].as_str().unwrap(), inputs, c["params"].clone());
        let names: Vec<String> = c["names"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().into()).collect();
        let got: Vec<f64> = match &out["result"] {
            Value::Stack(s) => {
                let keys: Vec<String> =
                    s.keys.iter().map(|k| if let Key::Name(n) = k { n.clone() } else { panic!() }).collect();
                assert_eq!(keys, names, "{name}");
                s.layers.iter().flat_map(|l| l.iter().copied()).collect()
            }
            Value::Table(t) => {
                assert_eq!(t.columns, names, "{name}");
                (0..t.columns.len()).flat_map(|c| t.rows.iter().map(move |r| r[c])).collect()
            }
            v => panic!("{v:?}"),
        };
        let want = &z[name].data;
        assert_eq!(got.len(), want.len(), "{name}");
        let exact = !(name.starts_with("exdet") || name == "mop_mahalanobis");
        for (k, (a, b)) in got.iter().zip(want).enumerate() {
            let same = a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan());
            assert!(same || (!exact && (a - b).abs() <= 1e-9 * a.abs().max(b.abs())), "{name} at {k}: {a} {b}");
        }
    }
}
