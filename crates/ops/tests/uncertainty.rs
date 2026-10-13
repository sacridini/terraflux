//! The propagation of uncertainty against landschaft's (tests/data/reference/uncertainty.npz,
//! tools/make_uncertainty_reference.py): totals and products of uncertain maps, confidence intervals, the delta
//! method and Monte Carlo (numpy's normal draws, sums and quantiles), to the bit.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::Grid;

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn check(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if let Some(k) = got.iter().zip(want).position(|(a, b)| !same(*a, *b)) {
        panic!("{what}: at {k}: {} {}", got[k], want[k]);
    }
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn values(v: &Value) -> Vec<f64> {
    match v {
        Value::Map(m) => m.data.to_vec(),
        Value::Number(x) => vec![*x],
        Value::Stack(s) => s.layers.iter().flat_map(|l| l.iter().copied()).collect(),
        Value::Table(t) => t.rows.iter().map(|r| r[1]).collect(),
        v => panic!("{v:?}"),
    }
}

#[test]
fn uncertainty_is_landschafts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let z = terraflux_raster::npy::read_npz(root.join("tests/data/reference/uncertainty.npz")).unwrap();
    let grid = Arc::new(Grid::new(31, 23));
    let map = |k: &str| Value::Map(Map::new(grid.clone(), z[k].data.clone()));
    let num = |x: f64| Value::Number(x);

    let s = |inputs, params| values(&run("uncertainty.sum_sd", inputs, params)["sd"]);
    check("sum quadratic", &s(vec![("sd", map("sd"))], serde_json::json!({})), &z["sum_quadratic"].data);
    check(
        "sum linear",
        &s(vec![("sd", map("sd")), ("weights", map("w"))], serde_json::json!({"method": "linear"})),
        &z["sum_linear_w"].data,
    );
    check("sum w 2.5", &s(vec![("sd", map("sd")), ("weights", num(2.5))], serde_json::json!({})), &z["sum_w2"].data);
    for rho in ["-0.3", "0.0", "0.6", "1.0"] {
        let got = s(
            vec![("sd", map("sd")), ("weights", map("w"))],
            serde_json::json!({"correlation": rho.parse::<f64>().unwrap()}),
        );
        check(&format!("sum rho {rho}"), &got, &z[&format!("sum_rho_{rho}")].data);
    }

    let p = |inputs, params| values(&run("uncertainty.product_sd", inputs, params)["sd"]);
    let xs = || vec![("x", map("x")), ("sd_x", map("sx")), ("v", map("v")), ("sd_v", map("sv"))];
    check("product", &p(xs(), serde_json::json!({})), &z["product"].data);
    check(
        "product numbers",
        &p(vec![("x", map("x")), ("sd_x", num(0.2)), ("v", num(3.0)), ("sd_v", map("sv"))], serde_json::json!({})),
        &z["product_numbers"].data,
    );
    check("product total q", &p(xs(), serde_json::json!({"total": true})), &z["product_total_q"].data);
    check(
        "product total l",
        &p(xs(), serde_json::json!({"total": true, "method": "linear"})),
        &z["product_total_l"].data,
    );

    for level in ["0.5", "0.9", "0.95", "0.99", "0.999"] {
        let out = run(
            "uncertainty.interval",
            vec![("value", map("x")), ("sd", map("sx"))],
            serde_json::json!({"level": level.parse::<f64>().unwrap()}),
        );
        let got: Vec<f64> = values(&out["low"]).into_iter().chain(values(&out["high"])).collect();
        check(&format!("interval {level}"), &got, &z[&format!("interval_{level}")].data);
    }

    let pars = serde_json::json!([["factor", 0.47, 0.02], ["base", 1.5, 0.3]]);
    for scheme in ["central", "forward"] {
        for method in ["quadratic", "linear"] {
            let out = run(
                "uncertainty.delta",
                vec![("carbon", map("carbon"))],
                serde_json::json!({"expression": "carbon * factor + base", "parameters": pars, "scheme": scheme,
                    "method": method}),
            );
            let got: Vec<f64> = values(&out["value"]).into_iter().chain(values(&out["sd"])).collect();
            check(&format!("delta {scheme} {method}"), &got, &z[&format!("delta_{scheme}_{method}")].data);
        }
    }
    let out = run(
        "uncertainty.delta",
        vec![("carbon", map("carbon"))],
        serde_json::json!({"expression": "carbon / (a + b * carbon)", "parameters": [["a", 2.0, 0.1], ["b", 0.01, 0.002]]}),
    );
    let got: Vec<f64> = values(&out["value"]).into_iter().chain(values(&out["sd"])).collect();
    check("delta ratio", &got, &z["delta_ratio"].data);
    let out = run(
        "uncertainty.delta",
        vec![],
        serde_json::json!({"expression": "a * b", "parameters": [["a", 2.0, 0.1], ["b", 3.0, 0.2]], "step": 1e-4}),
    );
    let got: Vec<f64> = values(&out["value"]).into_iter().chain(values(&out["sd"])).collect();
    check("delta scalar", &got, &z["delta_scalar"].data);

    let out = run(
        "uncertainty.monte_carlo",
        vec![("carbon", map("carbon"))],
        serde_json::json!({"expression": "carbon * factor + base", "parameters": pars, "n": 300, "seed": 3,
            "quantiles": [0.025, 0.5, 0.975]}),
    );
    check("mc mean", &values(&out["mean"]), &z["mc_mean"].data);
    check("mc sd", &values(&out["sd"]), &z["mc_sd"].data);
    check("mc quantiles", &values(&out["quantiles"]), &z["mc_q"].data);
    let out = run(
        "uncertainty.monte_carlo",
        vec![],
        serde_json::json!({"expression": "a * b", "parameters": [["a", 2.0, 0.1], ["b", 3.0, 0.2]], "n": 5000, "seed": 1}),
    );
    let got: Vec<f64> =
        values(&out["mean"]).into_iter().chain(values(&out["sd"])).chain(values(&out["quantiles"])).collect();
    check("mc scalar", &got, &z["mc_scalar"].data);
}
