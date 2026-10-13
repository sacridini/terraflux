//! The evaluation of species distribution models against landschaft's (tests/data/reference/sdm_evaluate.npz
//! and .json, tools/make_sdm_evaluate_reference.py): the statistics, confusion tables and thresholds of
//! evaluate, Boyce's index, every threshold method and the omission rates, to the bit (the p-value of the
//! correlation, Student's t, to rounding).

use std::path::Path;

use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn column(v: &[f64]) -> Value {
    let mut t = Table::new(vec!["prediction".into()]);
    t.rows = v.iter().map(|x| vec![*x]).collect();
    Value::Table(t)
}

fn flat(v: &Value) -> Vec<f64> {
    match v {
        Value::Table(t) => t.rows.iter().flatten().copied().collect(),
        Value::Number(x) => vec![*x],
        v => panic!("{v:?}"),
    }
}

fn check(what: &str, got: &[f64], want: &[f64], loose: &[usize]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        let same = a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan());
        assert!(
            same || (loose.contains(&k) && (a - b).abs() <= 1e-10 * a.abs().max(b.abs())),
            "{what} at {k}: {a} {b}"
        );
    }
}

#[test]
fn evaluation_is_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_evaluate.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_evaluate.json")).unwrap()).unwrap();
    let pres = column(&z["in/pres"].data);
    let small = column(&z["in/bg_small"].data);
    for c in meta["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        if c["op"] == "sdm.evaluate" {
            let bg = column(&z[c["background"].as_str().unwrap()].data);
            let out = run("sdm.evaluate", vec![("presences", pres.clone()), ("background", bg)], c["params"].clone());
            // the p-value (third) to rounding
            check(&format!("{name} stats"), &flat(&out["stats"]), &z[&format!("{name}/stats")].data, &[2]);
            check(&format!("{name} table"), &flat(&out["table"]), &z[&format!("{name}/table")].data, &[]);
            check(
                &format!("{name} thresholds"),
                &flat(&out["thresholds"]),
                &z[&format!("{name}/thresholds")].data,
                &[],
            );
            let b: Vec<f64> = match &out["boyce"] {
                Value::Table(t) => t.rows.iter().flat_map(|r| [r[0], r[1]]).collect(),
                _ => panic!(),
            };
            check(&format!("{name} boyce"), &b, &z[&format!("{name}/boyce")].data, &[]);
        } else {
            let avail: Vec<f64> = z["in/bg_small"].data.iter().chain(&z["in/pres"].data).copied().collect();
            let out =
                run("sdm.boyce", vec![("presences", pres.clone()), ("available", column(&avail))], c["params"].clone());
            check(&format!("{name} cor"), &flat(&out["cor"]), &z[&format!("{name}/cor")].data, &[]);
            let Value::Table(t) = &out["table"] else { panic!() };
            let b: Vec<f64> = t.rows.iter().flat_map(|r| [r[0], r[1]]).collect();
            check(name, &b, &z[name].data, &[]);
            let used: Vec<f64> =
                t.rows.iter().enumerate().filter(|(_, r)| r[2] == 1.0).map(|(k, _)| k as f64).collect();
            check(&format!("{name} used"), &used, &z[&format!("{name}/used")].data, &[]);
        }
    }
    let mut got = Vec::new();
    for m in meta["methods"].as_array().unwrap() {
        let out = run(
            "sdm.threshold",
            vec![("presences", pres.clone()), ("background", small.clone())],
            serde_json::json!({"method": m}),
        );
        got.extend(flat(&out["threshold"]));
    }
    let out = run(
        "sdm.threshold",
        vec![("presences", pres.clone()), ("background", small.clone())],
        serde_json::json!({"method": "sensitivity", "sensitivity": 0.85}),
    );
    got.extend(flat(&out["threshold"]));
    check("thresholds", &got, &z["thresholds"].data, &[]);
    let test = column(&z["in/test"].data);
    let few = column(&z["in/pres"].data[..7]);
    let mut got = Vec::new();
    for (train, t) in [(&pres, "mtp"), (&pres, "p10"), (&few, "p10")] {
        let out = run(
            "sdm.omission_rate",
            vec![("train", train.clone()), ("test", test.clone())],
            serde_json::json!({"threshold": t}),
        );
        got.extend(flat(&out["rate"]));
    }
    check("omission", &got, &z["omission"].data, &[]);
}
