//! sdm.fit's GLM and random forests against landschaft's fit with scikit-learn's (tests/data/reference/
//! sdm_models.npz and .json, tools/make_sdm_models_reference.py): the forests' suitability at every record, their
//! folds' metrics, means and standard deviations and threshold to the bit (the same trees); the GLMs' to the
//! optimum's precision.

use std::path::Path;

use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn flat(v: &Value) -> Vec<f64> {
    match v {
        Value::Table(t) => t.rows.iter().flatten().copied().collect(),
        Value::Number(x) => vec![*x],
        _ => panic!("{v:?}"),
    }
}

fn compare(what: &str, got: &[f64], want: &[f64], tol: f64) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        let ok = if tol == 0.0 {
            a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
        } else {
            (a - b).abs() <= tol || (a.is_nan() && b.is_nan())
        };
        assert!(ok, "{what}: at {k}: {a} {b}");
    }
}

#[test]
fn models_are_scikit_learns() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_models.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_models.json")).unwrap()).unwrap();
    let columns: Vec<String> = serde_json::from_value(meta["columns"].clone()).unwrap();
    let mut records = Table::new(columns.clone());
    records.rows = z["records"].data.chunks(columns.len()).map(|r| r.to_vec()).collect();
    let mut env = records.clone();
    env.columns.truncate(columns.len() - 2);
    env.rows.iter_mut().for_each(|r| r.truncate(columns.len() - 2));
    for (name, settings) in meta["cases"].as_object().unwrap() {
        let fit = run("sdm.fit", vec![("records", Value::Table(records.clone()))], settings.clone());
        let tol = if name.starts_with("glm") { 1e-7 } else { 0.0 };
        let model = fit["model"].clone();
        let p = run(
            "sdm.maxnet_predict",
            vec![("model", model), ("env", Value::Table(env.clone()))],
            serde_json::json!({}),
        );
        compare(&format!("{name} suitability"), &flat(&p["prediction"]), &z[&format!("{name}/suitability")].data, tol);
        compare(&format!("{name} threshold"), &flat(&fit["threshold"]), &z[&format!("{name}/threshold")].data, tol);
        // the folds' metrics without the fold number
        let Value::Table(m) = &fit["metrics"] else { panic!() };
        let metrics: Vec<f64> = m.rows.iter().flat_map(|r| r[1..].to_vec()).collect();
        let want = &z[&format!("{name}/metrics")].data;
        let want: Vec<f64> = want.chunks(m.columns.len()).flat_map(|r| r[1..].to_vec()).collect();
        compare(&format!("{name} metrics"), &metrics, &want, tol.max(if tol > 0.0 { 1e-6 } else { 0.0 }));
        compare(
            &format!("{name} summary"),
            &flat(&fit["summary"]),
            &z[&format!("{name}/summary")].data,
            if tol > 0.0 { 1e-6 } else { 0.0 },
        );
    }
}
