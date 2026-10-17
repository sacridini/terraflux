//! The random forests and the potential of each class against scikit-learn's and landschaft's
//! (tests/data/reference/ml.npz and .json, tools/make_ml_reference.py): a classifier's probabilities of three
//! classes and a regressor's values with their importances, and landschaft's Estimator's maps of PLUS's land
//! expansion analysis and of the presence of each class with the drivers' importances; to the bit.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Map, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn reference() -> (BTreeMap<String, Array>, serde_json::Value) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("ml.npz")).unwrap();
    let meta = serde_json::from_str(&std::fs::read_to_string(dir.join("ml.json")).unwrap()).unwrap();
    (z, meta)
}

fn check(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if let Some(k) = got.iter().zip(want).position(|(a, b)| a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan())) {
        panic!("{what}: at {k}: {} {}", got[k], want[k]);
    }
}

/// The records of the table: five predictors x0..x4 and the target, or the test's predictors.
fn records(x: &Array, y: Option<&[f64]>) -> Table {
    let (n, k) = (x.shape[0], x.shape[1]);
    let mut t = Table::new((0..k).map(|j| format!("x{j}")).chain(y.map(|_| "target".to_string())).collect());
    for i in 0..n {
        t.rows.push(x.data[i * k..(i + 1) * k].iter().copied().chain(y.map(|y| y[i])).collect());
    }
    t
}

fn table(v: &Value) -> &Table {
    let Value::Table(t) = v else { panic!("{v:?}") };
    t
}

#[test]
fn forests_are_scikit_learns() {
    let (z, meta) = reference();
    let x = &z["table/x"];
    let test = records(&z["table/test"], None);
    for (task, target) in [("classifier", "table/class"), ("regressor", "table/value")] {
        for name in ["default", "shaped"] {
            let mut params = meta[format!("{task}/{name}")].clone();
            params["task"] = serde_json::json!(if task == "classifier" { "classes" } else { "values" });
            let fit = run("ml.fit", vec![("table", Value::Table(records(x, Some(&z[target].data))))], params);
            check(
                &format!("{task} {name} importance"),
                &table(&fit["importance"]).rows[0],
                &z[format!("{task}/{name}/importance").as_str()].data,
            );
            let out = run(
                "ml.predict",
                vec![("model", fit["model"].clone()), ("table", Value::Table(test.clone()))],
                serde_json::json!({}),
            );
            let t = table(&out["table"]);
            if task == "classifier" {
                // the probabilities of the classes 1, 2 and 5, row by row
                let got: Vec<f64> = t.rows.iter().flat_map(|r| r[6..9].to_vec()).collect();
                check(&format!("{task} {name}"), &got, &z[format!("{task}/{name}/proba").as_str()].data);
                assert_eq!(t.columns[6..9], ["p_1", "p_2", "p_5"]);
            } else {
                let got: Vec<f64> = t.rows.iter().map(|r| r[5]).collect();
                check(&format!("{task} {name}"), &got, &z[format!("{task}/{name}/predict").as_str()].data);
            }
        }
    }
}

#[test]
fn the_potential_is_landschafts() {
    let (z, meta) = reference();
    let t0 = &z["pie/t0"];
    let (ny, nx) = (t0.shape[0], t0.shape[1]);
    let g = Arc::new(Grid::new(nx, ny));
    let map = |k: &str| Value::Map(Map::new(g.clone(), z[k].data.clone()));
    let names: Vec<String> =
        meta["pie/drivers"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let drivers = Stack {
        grid: g.clone(),
        keys: names.iter().map(|n| Key::Name(n.clone())).collect(),
        layers: names.iter().map(|n| Arc::new(z[format!("pie/driver/{n}").as_str()].data.clone())).collect(),
    };
    for name in ["expansion", "stratified", "presence"] {
        let m = &meta[format!("pie/{name}")];
        let mut params =
            serde_json::json!({"n_estimators": 10, "random_state": 0, "target": m["target"], "seed": m["seed"]});
        for k in ["sample", "stratify", "balance"] {
            if !m[k].is_null() {
                params[k] = m[k].clone();
            }
        }
        let mut inputs = vec![("landscape", map("pie/t0")), ("drivers", Value::Stack(drivers.clone()))];
        if m["target"] == "expansion" {
            inputs.push(("later", map("pie/t1")));
        }
        let out = run("ml.potential", inputs, params);
        let Value::Stack(p) = &out["potential"] else { panic!() };
        let classes: Vec<f64> = m["classes"].as_array().unwrap().iter().map(|c| c.as_f64().unwrap()).collect();
        assert_eq!(p.keys, classes.iter().map(|&c| Key::Class(c)).collect::<Vec<_>>());
        for (k, c) in classes.iter().enumerate() {
            check(&format!("{name} {c}"), &p.layers[k], &z[format!("pie/{name}/{c}").as_str()].data);
        }
        // landschaft's importance: a row per driver (sorted), a column per class
        let imp = table(&out["importance"]);
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(imp.columns[1..], sorted[..]);
        let want = &z[format!("pie/{name}/importance").as_str()];
        let got: Vec<f64> = (0..sorted.len()).flat_map(|d| imp.rows.iter().map(move |r| r[1 + d])).collect();
        check(&format!("{name} importance"), &got, &want.data);

        // the model again on the drivers: the same maps
        let again = run(
            "ml.predict",
            vec![("model", out["model"].clone()), ("predictors", Value::Stack(drivers.clone()))],
            serde_json::json!({}),
        );
        let Value::Stack(q) = &again["probability"] else { panic!() };
        for (a, b) in q.layers.iter().zip(&p.layers) {
            check(&format!("{name} again"), a, b);
        }
    }
}

/// A forest on maps: the cells of a target learnt from a stack, and predicted back.
#[test]
fn a_forest_of_maps() {
    let g = Arc::new(Grid::new(40, 30));
    let n = g.cells();
    let a: Vec<f64> = (0..n).map(|i| (i % 40) as f64).collect();
    let b: Vec<f64> = (0..n).map(|i| (i / 40) as f64).collect();
    let target: Vec<f64> = (0..n).map(|i| if a[i] + b[i] > 35.0 { 2.0 } else { 1.0 }).collect();
    let stack = Stack {
        grid: g.clone(),
        keys: vec![Key::Name("a".into()), Key::Name("b".into())],
        layers: vec![Arc::new(a), Arc::new(b)],
    };
    let fit = run(
        "ml.fit",
        vec![("predictors", Value::Stack(stack.clone())), ("target", Value::Map(Map::new(g.clone(), target.clone())))],
        serde_json::json!({"n_estimators": 20, "sample": 600, "stratify": true}),
    );
    let out = run(
        "ml.predict",
        vec![("model", fit["model"].clone()), ("predictors", Value::Stack(stack))],
        serde_json::json!({}),
    );
    let Value::Map(m) = &out["map"] else { panic!() };
    let right = m.data.iter().zip(&target).filter(|(a, b)| a == b).count();
    assert!(right as f64 > 0.97 * n as f64, "{right} of {n}");
    let Value::Stack(p) = &out["probability"] else { panic!() };
    assert_eq!(p.keys, vec![Key::Class(1.0), Key::Class(2.0)]);
}
