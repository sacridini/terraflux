//! prioritize.optimize's minimum extinction risk (landschaft's add_min_extinction_risk_objective, its extension of
//! prioritizr) and prioritize.evaluate's extinction risk against landschaft (tests/data/reference/
//! optimize_extinction.json, tools/make_extinction_reference.py): the models bit for bit, the optima, the risks;
//! and the objective against its definition by enumeration.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use terraflux_ops::{Args, Ctx, Key, Map, Registry, Stack, Table, Value};
use terraflux_raster::npy::Array;

const FEATURES: [&str; 5] = ["f1", "f2", "f3", "f4", "f5"];
const ZONES: [&str; 3] = ["protect", "use", "restore"];

fn data(file: &str) -> BTreeMap<String, Array> {
    terraflux_raster::npy::read_npz(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference").join(file))
        .unwrap()
}

fn grid(ny: usize, nx: usize) -> Arc<terraflux_raster::Grid> {
    let mut grid = terraflux_raster::Grid::new(nx, ny);
    grid.transform = [0.0, 100.0, 0.0, ny as f64 * 100.0, 0.0, -100.0];
    Arc::new(grid)
}

fn stack(g: &Arc<terraflux_raster::Grid>, a: &Array, keys: Vec<String>) -> Value {
    let n = g.width * g.height;
    Value::Stack(Stack {
        grid: g.clone(),
        keys: keys.into_iter().map(Key::Name).collect(),
        layers: a.data.chunks(n).map(|c| Arc::new(c[..n].to_vec())).collect(),
    })
}

/// The landschaft cases' problems: rasters of one cost and five features, or of three zones with their own.
fn inputs(zoned: bool) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    if zoned {
        let z = data("optimize_zones.npz");
        let g = grid(z["zone_cost"].shape[1], z["zone_cost"].shape[2]);
        let keys = ZONES.iter().flat_map(|z| FEATURES.iter().map(move |f| format!("{z}_{f}"))).collect();
        out.insert("zone_costs".into(), stack(&g, &z["zone_cost"], ZONES.map(String::from).to_vec()));
        out.insert("zone_features".into(), stack(&g, &z["zone_features"], keys));
    } else {
        let d = data("optimize_inputs.npz");
        let f = &d["features"];
        let g = grid(f.shape[1], f.shape[2]);
        out.insert("features".into(), stack(&g, f, FEATURES.map(String::from).to_vec()));
        out.insert("cost".into(), Value::Map(Map::new(g, d["pu_cost"].data.clone())));
    }
    out
}

fn run(op: &str, ins: BTreeMap<String, Value>, params: &serde_json::Value) -> terraflux_ops::Outputs {
    let args = Args { node: "n", inputs: ins, params: params.as_object().unwrap() };
    Registry::standard().get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn floats(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect()
}

fn texts(v: &serde_json::Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

/// The cases: the node's settings, whether with zones.
fn cases() -> Vec<(&'static str, serde_json::Value, bool)> {
    vec![
        ("raster_default", json!({"objective": "min_extinction_risk", "budget": 2500}), false),
        (
            "raster_z_weights",
            json!({"objective": "min_extinction_risk", "budget": 2500, "z": [0.25, 0.3, 0.2, 0.15, 0.35],
                   "segments": 8, "feature_weights": [1.0, 2.0, 1.0, 0.5, 1.0]}),
            false,
        ),
        (
            "raster_proportion",
            json!({"objective": "min_extinction_risk", "budget": 1800, "z": 0.4, "decisions": "proportion"}),
            false,
        ),
        ("zones", json!({"objective": "min_extinction_risk", "budget": 2500, "feature_names": FEATURES}), true),
    ]
}

/// The models bit for bit, the optima (HiGHS at gap 0 on both sides), the risks of the solutions.
#[test]
fn extinction_risk_is_landschafts() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/optimize_extinction.json");
    let all: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for (name, params, zoned) in cases() {
        let r = &all[name];
        let w = &r["model"];
        let args = Args { node: "n", inputs: inputs(zoned), params: params.as_object().unwrap() };
        let p = terraflux_ops::prioritize::problem_of(&args).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (m, _) = terraflux_ops::milp::compile(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut ours = Vec::new();
        for i in 0..m.a.nrow {
            let (idx, v) = m.a.row(i);
            ours.extend(idx.iter().zip(v).map(|(&j, &x)| (i, j, x)));
        }
        let (ai, aj, ax) = (floats(&w["A_i"]), floats(&w["A_j"]), floats(&w["A_x"]));
        let want: Vec<(usize, usize, f64)> =
            (0..ax.len()).filter(|&e| ax[e] != 0.0).map(|e| (ai[e] as usize, aj[e] as usize, ax[e])).collect();
        assert_eq!(ours.len(), want.len(), "{name}: entries");
        for (a, b) in ours.iter().zip(&want) {
            assert!(a.0 == b.0 && a.1 == b.1 && a.2.to_bits() == b.2.to_bits(), "{name}: {a:?} != {b:?}");
        }
        for (what, v) in [("obj", &m.obj), ("lb", &m.lb), ("ub", &m.ub), ("rhs", &m.rhs)] {
            let b = floats(&w[what]);
            assert_eq!(v.len(), b.len(), "{name}: {what}");
            for (i, (x, y)) in v.iter().zip(&b).enumerate() {
                assert!(
                    x.to_bits() == y.to_bits() || (x.is_infinite() && y.is_nan()),
                    "{name}: {what}[{i}] {x} != {y}"
                );
            }
        }
        assert_eq!(m.sense, texts(&w["sense"]), "{name}: sense");
        assert_eq!(m.vtype.iter().map(|c| c.to_string()).collect::<Vec<_>>(), texts(&w["vtype"]), "{name}: vtype");
        assert_eq!(m.row_ids, texts(&w["row_ids"]), "{name}: row ids");
        assert_eq!(m.col_ids, texts(&w["col_ids"]), "{name}: column ids");
        assert!(!m.maximize, "{name}: minimized");
        // the optimum and its risks
        let mut params = params.clone();
        params["gap"] = json!(0.0);
        let out = run("prioritize.optimize", inputs(zoned), &params);
        let Value::Table(s) = &out["summary"] else { panic!() };
        let objective = s.column("objective").unwrap()[0];
        let want = r["objective"].as_f64().unwrap();
        assert!((objective - want).abs() <= 1e-7 * want.abs().max(1.0), "{name}: {objective} != {want}");
        let Value::Table(risk) = &out["extinction_risk"] else { panic!("{name}: no risk") };
        let got = risk.column("extinction_risk").unwrap();
        let want = floats(&r["risk"]);
        assert_eq!(got.len(), want.len());
        for (a, b) in got.iter().zip(&want) {
            assert!((a - b).abs() <= 1e-6, "{name}: risk {a} != {b}");
        }
    }
}

/// The optimum is the least risk over every selection within the budget (enumerated), the risk the piecewise-linear
/// 1 - r^z through the curve's points, plus the tie-break 0.01 x cost / total cost; the evaluation the exact risk.
#[test]
fn extinction_risk_is_its_definition() {
    // landschaft's _sar_table: 11 units, 3 features
    let cost = [3.0, 1.0, 5.0, 2.0, 4.0, 1.0, 2.0, 5.0, 3.0, 2.0, 4.0];
    let amounts = [
        [2.0, 0.0, 1.0, 4.0, 0.0, 3.0, 0.0, 2.0, 1.0, 0.0, 3.0],
        [0.0, 3.0, 2.0, 0.0, 1.0, 0.0, 4.0, 0.0, 2.0, 3.0, 1.0],
        [1.0, 1.0, 0.0, 2.0, 3.0, 0.0, 1.0, 4.0, 0.0, 2.0, 0.0],
    ];
    let (budget, z, segs) = (12.0, [0.25, 0.4, 0.15], 8usize);
    let mut pu = Table::new(["id", "cost", "f1", "f2", "f3"].map(String::from).to_vec());
    pu.rows = (0..11).map(|i| vec![(i + 1) as f64, cost[i], amounts[0][i], amounts[1][i], amounts[2][i]]).collect();
    let ins: BTreeMap<String, Value> = [("pu".to_string(), Value::Table(pu))].into();
    let params = json!({"objective": "min_extinction_risk", "budget": budget, "z": z, "segments": segs, "gap": 0.0,
                        "features": ["f1", "f2", "f3"]});
    let out = run("prioritize.optimize", ins.clone(), &params);
    let Value::Table(s) = &out["summary"] else { panic!() };
    let objective = s.column("objective").unwrap()[0];
    let total: Vec<f64> = amounts.iter().map(|a| a.iter().sum()).collect();
    let all_cost: f64 = cost.iter().sum();
    let mut best = f64::INFINITY;
    for bits in 0u32..(1 << 11) {
        let x: Vec<f64> = (0..11).map(|i| f64::from((bits >> i) & 1)).collect();
        let c: f64 = cost.iter().zip(&x).map(|(c, x)| c * x).sum();
        if c > budget + 1e-9 {
            continue;
        }
        let mut risk = 0.0;
        for f in 0..3 {
            let r = amounts[f].iter().zip(&x).map(|(a, x)| a * x).sum::<f64>() / total[f];
            let (ci, sl) = terraflux_ops::milp::sar_lines(z[f], segs);
            let line = (0..segs).map(|k| 1.0 - (ci[k] + sl[k] * r)).fold(f64::NEG_INFINITY, f64::max);
            risk += line.max(0.0);
        }
        best = best.min(risk + 0.01 * c / all_cost);
    }
    assert!((objective - best).abs() <= 1e-7, "{objective} != {best}");
    // the evaluation: 1 - r^z exactly, below the piecewise-linear risk (the chords lie below r^z)
    let Value::Table(sel) = &out["selection"] else { panic!() };
    let mut ev = params.clone();
    ev["extinction_z"] = json!(z);
    let mut ins = ins;
    ins.insert("selection".into(), Value::Table(sel.clone()));
    let e = run("prioritize.evaluate", ins, &ev);
    let Value::Table(risk) = &e["extinction_risk"] else { panic!() };
    let rel = risk.column("relative_held").unwrap();
    let r = risk.column("extinction_risk").unwrap();
    for f in 0..3 {
        assert!((r[f] - (1.0 - rel[f].powf(z[f]))).abs() < 1e-15);
    }
    let spent = s.column("cost").unwrap()[0];
    let pwl = objective - 0.01 * spent / all_cost;
    assert!(r[3] <= pwl + 1e-9 && pwl - r[3] < 0.15, "{} {pwl}", r[3]);
    assert_eq!(r[3], r[..3].iter().sum::<f64>());
    // z outside (0, 1]
    let mut bad = params.clone();
    bad["z"] = json!(0);
    let args = Args { node: "n", inputs: BTreeMap::new(), params: bad.as_object().unwrap() };
    assert!(Registry::standard().get("prioritize.optimize").unwrap().run(&mut Ctx::default(), &args).is_err());
}
