//! The sampling-bias surface against landschaft's (tests/data/reference/sdm_bias.npz and .json,
//! tools/make_sdm_bias_reference.py): MASS::kde2d's density with R's bw.nrd, isotropic and limited bandwidths, a
//! pair with weights and a rescaling, every cell. The bandwidths to the bit; the density up to rounding (numpy sums
//! the kernels with BLAS's matrix product).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args)
}

fn inputs() -> (Value, Value, serde_json::Value, BTreeMap<String, Array>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_bias.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_bias.json")).unwrap()).unwrap();
    let e = &z["in/env"];
    let (ny, nx) = (e.shape[1], e.shape[2]);
    let mut g = Grid::new(nx, ny);
    for (k, v) in meta["transform"].as_array().unwrap().iter().enumerate() {
        g.transform[k] = v.as_f64().unwrap();
    }
    let env = Value::Stack(Stack {
        grid: Arc::new(g),
        keys: ["a", "b"].iter().map(|n| Key::Name(n.to_string())).collect(),
        layers: (0..2).map(|b| Arc::new(e.data[b * ny * nx..(b + 1) * ny * nx].to_vec())).collect(),
    });
    let mut t = Table::new(["x", "y", "w"].iter().map(|s| s.to_string()).collect());
    t.rows = z["in/points"].data.chunks(3).map(|r| r.to_vec()).collect();
    (Value::Table(t), env, meta, z)
}

#[test]
fn bias_surface_is_landschafts() {
    let (points, env, meta, z) = inputs();
    for (case, params) in meta["cases"].as_object().unwrap() {
        let out = run("sdm.bias_surface", vec![("occurrences", points.clone()), ("env", env.clone())], params.clone())
            .unwrap_or_else(|e| panic!("{case}: {e}"));
        let Value::Json(bw) = &out["bandwidth"] else { panic!() };
        let bw: Vec<f64> = serde_json::from_value(bw.clone()).unwrap();
        let want = &z[&format!("{case}/bandwidth")].data;
        assert!(bw.iter().zip(want).all(|(a, b)| a.to_bits() == b.to_bits()), "{case}: bandwidth {bw:?} {want:?}");
        let Value::Map(m) = &out["bias"] else { panic!() };
        let want = &z[&format!("{case}/bias")].data;
        assert_eq!(m.data.len(), want.len());
        let top = want.iter().copied().filter(|v| !v.is_nan()).fold(0.0, f64::max);
        for (k, (a, b)) in m.data.iter().zip(want).enumerate() {
            assert_eq!(a.is_nan(), b.is_nan(), "{case}: cell {k}: {a} {b}");
            if !b.is_nan() {
                assert!((a - b).abs() <= 1e-10 * b.abs() + 1e-12 * top, "{case}: cell {k}: {a} {b}");
            }
        }
    }
}

#[test]
fn bias_surface_settings() {
    let (points, env, _, _) = inputs();
    let go =
        |p: serde_json::Value| run("sdm.bias_surface", vec![("occurrences", points.clone()), ("env", env.clone())], p);
    let bw = |o: &Outputs| -> Vec<f64> {
        let Value::Json(b) = &o["bandwidth"] else { panic!() };
        serde_json::from_value(b.clone()).unwrap()
    };
    assert_eq!(
        bw(&go(serde_json::json!({"bandwidth": [500.0, 50000.0], "limits": [2000.0, 10000.0]})).unwrap()),
        [2000.0, 10000.0]
    );
    assert_eq!(bw(&go(serde_json::json!({"bandwidth": 500.0, "limits": [2000.0, null]})).unwrap()), [2000.0, 2000.0]);
    // integer weights are the records repeated
    let Value::Table(t) = &points else { panic!() };
    let mut rep = Table::new(vec!["x".into(), "y".into()]);
    rep.rows = t.rows.iter().flat_map(|r| std::iter::repeat_n(vec![r[0], r[1]], r[2] as usize)).collect();
    let a = go(serde_json::json!({"bandwidth": 3000.0, "weights": "w"})).unwrap();
    let b = run(
        "sdm.bias_surface",
        vec![("occurrences", Value::Table(rep)), ("env", env.clone())],
        serde_json::json!({"bandwidth": 3000.0}),
    )
    .unwrap();
    let (Value::Map(a), Value::Map(b)) = (&a["bias"], &b["bias"]) else { panic!() };
    for (x, y) in a.data.iter().zip(b.data.iter()) {
        assert!(x.is_nan() && y.is_nan() || (x - y).abs() <= 1e-12 * y.abs() + 1e-300, "{x} {y}");
    }
    for bad in [
        serde_json::json!({"bandwidth": "silverman"}),
        serde_json::json!({"bandwidth": -1.0}),
        serde_json::json!({"rescale": [1.0, 0.0]}),
        serde_json::json!({"weights": "none"}),
    ] {
        assert!(go(bad.clone()).is_err(), "{bad}");
    }
}
