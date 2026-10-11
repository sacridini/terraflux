//! prioritize.zonation against Zonation 5 v2.4: the rank maps of the z5 binary that landschaft keeps
//! (extern/landschaft/tests/data/reference/prioritize/core), bit for bit, on the rasters of its cases
//! (tests/data/reference/zonation_inputs.npz, tools/make_zonation_inputs.py).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Map, Registry, Stack, Value};
use terraflux_raster::Grid;

fn run(
    inputs: Vec<(&str, Value)>,
    params: serde_json::Map<String, serde_json::Value>,
) -> Result<terraflux_ops::Outputs, String> {
    let r = Registry::standard();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get("prioritize.zonation").unwrap().run(&mut Ctx::default(), &args).map_err(|e| e.to_string())
}

/// Every case of the core group: every rule, 16 bits, weights (negative ones and groups), thresholds, ABF's z per
/// feature, the zero modes, presence and counts, value extraction, costs, hierarchic masks, analysis areas,
/// smoothing, few iterations, a coarser error, curves, LOAD and a larger stack.
#[test]
fn rank_is_zonations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inputs = terraflux_raster::npy::read_npz(root.join("tests/data/reference/zonation_inputs.npz")).unwrap();
    let all: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("tests/data/reference/zonation_inputs.json")).unwrap())
            .unwrap();
    let cases: BTreeMap<String, serde_json::Value> = serde_json::from_value(all["cases"].clone()).unwrap();
    let refdir = root.join("extern/landschaft/tests/data/reference/prioritize");
    let reference = terraflux_raster::npy::read_npz(refdir.join("core.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(refdir.join("core.json")).unwrap()).unwrap();
    let map_of = |name: &str| {
        let a = &inputs[name];
        Map::new(Arc::new(Grid::new(a.shape[1], a.shape[0])), a.data.clone())
    };
    let mut checked = 0;
    for (name, case) in &cases {
        let layers: Vec<String> =
            case["layers"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()).collect();
        let first = map_of(&layers[0]);
        let stack = Stack {
            grid: first.grid.clone(),
            keys: layers.iter().map(|l| Key::Name(l.clone())).collect(),
            layers: layers.iter().map(|l| Arc::new(inputs[l].data.clone())).collect(),
        };
        let mut params = case["params"].as_object().unwrap().clone();
        // the NoData values of the features' files
        let nodata: Vec<serde_json::Value> = layers.iter().map(|l| all["nodata"][l].clone()).collect();
        if nodata.iter().any(|v| !v.is_null()) {
            params.insert("nodata".into(), nodata.into());
        }
        let mut ins = vec![("features", Value::Stack(stack))];
        for port in ["cost", "hierarchic_mask", "analysis_area", "solution"] {
            if let Some(serde_json::Value::String(r)) = params.remove(port) {
                ins.push((port, Value::Map(map_of(&r))));
            }
        }
        let out = run(ins, params).unwrap_or_else(|e| panic!("{name}: {e}"));
        let Value::Map(rank) = &out["rank"] else { panic!() };
        let want = &reference[&format!("{name}/rank")];
        let differ = rank
            .data
            .iter()
            .zip(&want.data)
            .filter(|(a, b)| !((**a as f32).to_bits() == (**b as f32).to_bits() || (a.is_nan() && b.is_nan())))
            .count();
        assert_eq!(differ, 0, "{name}: {differ} cells differ from Zonation's rank");
        let stats = &meta["cases"][format!("{name}/stats")];
        let iterations: f64 =
            stats.get("Number of iterations").and_then(|v| v.as_str()).map_or(0.0, |v| v.parse().unwrap());
        let Value::Number(it) = out["iterations"] else { panic!() };
        assert_eq!(it, iterations, "{name}: iterations");
        checked += 1;
    }
    assert_eq!(checked, 41);
}

/// The rule's forms, empty features left out, and the settings checked.
#[test]
fn settings() {
    let grid = Arc::new(Grid::new(9, 8));
    let n = 72;
    let layer = |k: usize| Arc::new((0..n).map(|i| ((i * 7 + k * 13) % 17) as f64 / 17.0).collect::<Vec<_>>());
    let stack = |empty: bool| {
        Value::Stack(Stack {
            grid: grid.clone(),
            keys: (0..3).map(|k| Key::Name(format!("f{k}"))).collect(),
            layers: (0..3).map(|k| if empty && k == 1 { Arc::new(vec![0.0; n]) } else { layer(k) }).collect(),
        })
    };
    let p = |v: serde_json::Value| v.as_object().unwrap().clone();
    let a = run(vec![("features", stack(false))], p(serde_json::json!({"rule": "ABF 0.4"}))).unwrap();
    let b = run(vec![("features", stack(false))], p(serde_json::json!({"rule": "ABF", "rule_param": 0.4}))).unwrap();
    let (Value::Map(ra), Value::Map(rb)) = (&a["rank"], &b["rank"]) else { panic!() };
    assert_eq!(ra.data, rb.data);
    let mut ranks: Vec<f64> = ra.data.to_vec();
    ranks.sort_by(f64::total_cmp);
    assert_eq!(ranks[0], 0.0, "a permutation of 0..1");
    let out = run(vec![("features", stack(true))], p(serde_json::json!({}))).unwrap();
    let Value::Table(f) = &out["features"] else { panic!() };
    assert_eq!(f.rows.iter().map(|r| r[0]).collect::<Vec<_>>(), [1.0, 3.0], "the empty feature left out");
    for bad in [
        serde_json::json!({"rule": "XYZ"}),
        serde_json::json!({"rule": "ABF 0.3", "rule_param": 0.2}),
        serde_json::json!({"rule": "LOAD"}),
        serde_json::json!({"bits": 12}),
        serde_json::json!({"weights": [1, 2]}),
    ] {
        assert!(run(vec![("features", stack(false))], p(bad.clone())).is_err(), "{bad}");
    }
}
