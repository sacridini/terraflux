//! The comparisons of simulated and observed maps against landschaft's (tests/data/reference/map_validation.npz
//! and .json, tools/make_map_validation_reference.py): lulcc's three-map comparison (tables, budget, figures of
//! merit) at several resolutions, MOLUSCE's kappas and error budget, FUTURES' validation whole, by tiles and by
//! regions; to the bit (MOLUSCE's correlation and joint information uncertainty, which numpy computes with BLAS
//! and its own logarithm, to 1e-12).

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
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

fn table(v: &Value, skip: usize) -> Vec<f64> {
    let Value::Table(t) = v else { panic!("{v:?}") };
    t.rows.iter().flat_map(|r| r[skip..].iter().copied()).collect()
}

#[test]
fn validations_are_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("map_validation.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("map_validation.json")).unwrap()).unwrap();
    let mut g = Grid::new(52, 40);
    g.transform = [1000.0, 30.0, 0.0, 5000.0, 0.0, -30.0];
    let g = Arc::new(g);
    let map = |k: &str| Value::Map(Map::new(g.clone(), z[k].data.clone()));

    let out = run(
        "validate.three_map",
        vec![("t0", map("in/t0")), ("t1", map("in/t1")), ("simulated", map("in/sim"))],
        serde_json::json!({"factors": [1, 2, 4, 7]}),
    );
    let Value::Table(t) = &out["table"] else { panic!() };
    for f in [1.0, 2.0, 4.0, 7.0] {
        let got: Vec<f64> = t.rows.iter().filter(|r| r[0] == f).flat_map(|r| r[3..].iter().copied()).collect();
        check(&format!("table {f}"), &got, &z[format!("tmc/{f}").as_str()].data);
    }
    check("budget", &table(&out["budget"], 3), &z["tmc/budget"].data);
    check("figure of merit", &table(&out["figure_of_merit"], 1), &z["tmc/fom"].data);
    check("transitions", &table(&out["transitions"], 2), &z["tmc/transition"].data);

    let out =
        run("validate.kappa", vec![("reference", map("in/t1")), ("simulated", map("in/sim"))], serde_json::json!({}));
    let got = table(&out["table"], 0);
    let want = &z["kappa"].data;
    check("kappa", &got[..5], &want[..5]);
    for k in 5..7 {
        assert!((got[k] - want[k]).abs() <= 1e-12 * want[k].abs().max(1.0), "kappa {k}: {} {}", got[k], want[k]);
    }
    for scale in [2, 3] {
        let out = run(
            "validate.error_budget",
            vec![("reference", map("in/t1")), ("simulated", map("in/sim"))],
            serde_json::json!({"iterations": 4, "scale": scale}),
        );
        check(&format!("error budget {scale}"), &table(&out["table"], 0), &z[format!("budget/{scale}").as_str()].data);
    }

    for (name, names) in meta["futures"].as_object().unwrap() {
        let (r, s, o) = match name.as_str() {
            "binary" => ("in/ref", "in/fsim", Some("in/orig")),
            "no_original" => ("in/ref", "in/fsim", None),
            _ => ("in/three", "in/sim3", Some("in/t03")),
        };
        let mut inputs = vec![("reference", map(r)), ("simulated", map(s))];
        if let Some(o) = o {
            inputs.push(("original", map(o)));
        }
        let out = run("validate.futures", inputs, serde_json::json!({}));
        let Value::Table(m) = &out["metrics"] else { panic!() };
        let names: Vec<String> = names.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(m.columns, names, "{name}");
        check(&format!("futures {name}"), &m.rows[0], &z[format!("futures/{name}").as_str()].data);
        check(
            &format!("futures {name} classes"),
            &table(&out["classes"], 0),
            &z[format!("futures/{name}/classes").as_str()].data,
        );
    }
    for (name, columns) in meta["grid"].as_object().unwrap() {
        let mut inputs = vec![("reference", map("in/ref")), ("simulated", map("in/fsim"))];
        if name != "tiles_no_original" {
            inputs.push(("original", map("in/orig")));
        }
        let params = match name.as_str() {
            "tiles" => serde_json::json!({"size": 15}),
            "tiles_no_original" => serde_json::json!({"size": 20}),
            _ => {
                inputs.push(("regions", map("in/regions")));
                serde_json::json!({})
            }
        };
        let out = run("validate.futures_grid", inputs, params);
        let Value::Table(t) = &out["table"] else { panic!() };
        let columns: Vec<String> =
            columns.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(t.columns, columns, "{name}");
        check(&format!("grid {name}"), &table(&out["table"], 0), &z[format!("grid/{name}").as_str()].data);
    }
}
