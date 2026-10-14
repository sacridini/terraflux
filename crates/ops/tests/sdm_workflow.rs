//! The modelling workflow against landschaft's (tests/data/reference/sdm_workflow.npz and .json,
//! tools/make_sdm_workflow_reference.py): the dataset, random folds and spatial blocks on it, MaxNet fitted with
//! cross-validation over them (every fold's metrics, their means and standard deviations, the threshold and the
//! coefficients), the response curves and the permutation importance, to the bit.

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

fn table(cols: &[&str], rows: &[f64]) -> Value {
    let mut t = Table::new(cols.iter().map(|s| s.to_string()).collect());
    t.rows = rows.chunks(cols.len()).map(|r| r.to_vec()).collect();
    Value::Table(t)
}

fn flat(v: &Value) -> Vec<f64> {
    let Value::Table(t) = v else { panic!("{v:?}") };
    t.rows.iter().flatten().copied().collect()
}

#[test]
fn workflow_is_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_workflow.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_workflow.json")).unwrap()).unwrap();
    let e = &z["in/env"];
    let (ny, nx) = (e.shape[1], e.shape[2]);
    let mut g = Grid::new(nx, ny);
    g.transform = [0.0, 1000.0, 0.0, ny as f64 * 1000.0, 0.0, -1000.0];
    let names = ["temp", "prec", "soil", "cover"];
    let env = Value::Stack(Stack {
        grid: Arc::new(g),
        keys: names.iter().map(|n| Key::Name(n.to_string())).collect(),
        layers: (0..4).map(|b| Arc::new(e.data[b * ny * nx..(b + 1) * ny * nx].to_vec())).collect(),
    });
    let xy = serde_json::json!({"x": "x", "y": "y_coord"});
    let out = run(
        "sdm.dataset",
        vec![
            ("presences", table(&["x", "y_coord"], &z["in/pres"].data)),
            ("background", table(&["x", "y_coord"], &z["in/bg"].data)),
            ("env", env),
        ],
        xy,
    );
    let records = out["records"].clone();
    check("dataset", &flat(&records), &z["dataset"].data);
    for (case, op, params) in [
        ("random", "sdm.random_folds", serde_json::json!({"k": 4, "seed": 2})),
        ("blocks", "sdm.spatial_blocks", serde_json::json!({"size": 6000.0, "k": 3, "hexagon": false, "seed": 4})),
    ] {
        assert!(meta["cases"].as_array().unwrap().iter().any(|c| c == case));
        let with = run(op, vec![("records", records.clone())], params).remove("records").unwrap();
        let Value::Table(t) = &with else { panic!() };
        let folds: Vec<f64> = t.rows.iter().map(|r| r[r.len() - 1]).collect();
        check(&format!("{case} folds"), &folds, &z[&format!("{case}/folds")].data);
        let fit = run("sdm.fit", vec![("records", with)], serde_json::json!({"categorical": ["cover"]}));
        check(&format!("{case} metrics"), &flat(&fit["metrics"]), &z[&format!("{case}/metrics")].data);
        check(&format!("{case} summary"), &flat(&fit["summary"]), &z[&format!("{case}/summary")].data);
        let Value::Number(th) = fit["threshold"] else { panic!() };
        check(&format!("{case} threshold"), &[th], &z[&format!("{case}/threshold")].data);
        let Value::SdmModel(m) = &fit["model"] else { panic!() };
        let terraflux_ops::sdm::model::Model::Maxnet(m) = &**m else { panic!("not a MaxNet") };
        check(&format!("{case} betas"), &m.betas, &z[&format!("{case}/betas")].data);
    }
    let fit =
        run("sdm.fit", vec![("records", records.clone())], serde_json::json!({"categorical": ["cover"], "cv": false}));
    for fixed in ["mean", "median"] {
        let c = run(
            "sdm.response_curves",
            vec![("model", fit["model"].clone()), ("records", records.clone())],
            serde_json::json!({"n": 25, "fixed": fixed}),
        );
        let Value::Table(t) = &c["curves"] else { panic!() };
        // landschaft's rows per predictor: value, suitability
        let got: Vec<f64> =
            (0..t.columns.len() / 2).flat_map(|p| t.rows.iter().flat_map(move |r| [r[2 * p], r[2 * p + 1]])).collect();
        check(&format!("curves {fixed}"), &got, &z[&format!("curves_{fixed}")].data);
    }
    let imp = run(
        "sdm.importance",
        vec![("model", fit["model"].clone()), ("records", records)],
        serde_json::json!({"n_repeats": 3, "seed": 7}),
    );
    check("importance", &flat(&imp["importance"]), &z["importance"].data);
    let Value::Json(order) = &imp["predictors"] else { panic!() };
    assert_eq!(order, &meta["importance"]);
}
