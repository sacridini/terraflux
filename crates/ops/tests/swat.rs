//! SWAT+ against landschaft's (tests/data/reference/swat.npz, tools/make_swat_reference.py): the Hubbard Brook W2
//! example run at every step, the tables SWAT+ prints and the daily weather, to the bit.

use std::path::Path;

use terraflux_ops::{Args, Ctx, Outputs, Registry, Value};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: Default::default(), params: &params };
    r.get("swat.run").unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("swat.run: {e}"))
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

#[test]
fn swat_is_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/swat.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/swat.json")).unwrap()).unwrap();
    let example = root().join("extern/landschaft/src/landschaft/hydro/swat/data/hubbard_brook.zip");
    for case in meta["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut params = case.as_object().unwrap().clone();
        params.remove("name");
        params.insert("path".into(), serde_json::json!(example.to_str().unwrap()));
        let out = run(serde_json::Value::Object(params));
        let mut checked = 0;
        for (key, want) in z.iter().filter(|(k, _)| k.starts_with(&format!("{name}/"))) {
            let port = &key[name.len() + 1..];
            let Value::Table(t) = &out[port] else { panic!("{key}") };
            assert_eq!(t.rows.len(), want.shape[0], "{key}: rows");
            assert_eq!(t.columns.len(), want.shape[1], "{key}: columns");
            let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
            let bad = got.iter().zip(&want.data).filter(|(a, b)| !same(**a, **b)).count();
            assert_eq!(bad, 0, "{key}: {bad} values differ");
            checked += 1;
        }
        assert!(checked > 1, "{name}");
    }
}
