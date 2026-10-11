//! prioritize.marxan against Marxan 4.0.6: the runs landschaft keeps (extern/landschaft/tests/data/reference/
//! optimize/marxan.npz) of the tables of its cases (tests/data/reference/marxan_inputs.json,
//! tools/make_marxan_inputs.py), compared as landschaft's test_prioritize_marxan.py does: the reserves of every
//! run exactly; the summary exactly for the corrected executable, to its 6 printed digits for the released one.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Map, Registry, Stack, Table, Value};

fn run(inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Result<terraflux_ops::Outputs, String> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get("prioritize.marxan").unwrap().run(&mut Ctx::default(), &args).map_err(|e| e.to_string())
}

fn table(out: &terraflux_ops::Outputs, port: &str) -> Table {
    let Value::Table(t) = &out[port] else { panic!("{port}") };
    t.clone()
}

#[test]
fn runs_are_marxans() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("tests/data/reference/marxan_inputs.json")).unwrap())
            .unwrap();
    let reference =
        terraflux_raster::npy::read_npz(root.join("extern/landschaft/tests/data/reference/optimize/marxan.npz"))
            .unwrap();
    let mut checked = 0;
    for (name, case) in cases.as_object().unwrap() {
        let tables: Vec<(&str, Value)> = ["pu", "spec", "puvspr", "bound"]
            .into_iter()
            .map(|t| (t, Value::Table(serde_json::from_value::<Table>(case["tables"][t].clone()).unwrap())))
            .collect();
        let out = run(tables, case["options"].clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let sol = table(&out, "solutions");
        let want = &reference[&format!("{name}/solutions")];
        let (runs, npu) = (want.shape[0], want.shape[1]);
        assert_eq!(sol.rows.len(), npu, "{name}");
        for r in 0..runs {
            for i in 0..npu {
                assert_eq!(sol.rows[i][r + 1], want.data[r * npu + i], "{name}: run {} unit {i}", r + 1);
            }
        }
        let full = case["exe"] == "fixed";
        let s = table(&out, "summary");
        for (ours, theirs) in [
            ("score", "Score"),
            ("cost", "Cost"),
            ("penalty", "Penalty"),
            ("connectivity", "Connectivity"),
            ("shortfall", "Shortfall"),
            ("mpm", "MPM"),
            ("planning_units", "Planning_Units"),
            ("missing", "Missing_Values"),
        ] {
            let (a, b) = (s.column(ours).unwrap(), &reference[&format!("{name}/{theirs}")].data);
            for (x, y) in a.iter().zip(b) {
                let exact = full || matches!(theirs, "Planning_Units" | "Missing_Values");
                let ok = if exact { x == y } else { (x - y).abs() <= 1e-9 + 5e-6 * y.abs() };
                assert!(ok, "{name}: {ours} {x} != {y}");
            }
        }
        let p = table(&out, "penalties").column("penalty").unwrap();
        for (x, y) in p.iter().zip(&reference[&format!("{name}/penalty")].data) {
            assert!(if full { x == y } else { (x - y).abs() <= 5e-6 * y.abs() }, "{name}: penalty {x} != {y}");
        }
        checked += 1;
    }
    assert_eq!(checked, 8);
}

/// Marxan from maps: every cell a unit, its shared edges its boundaries; the reserve on the grid.
#[test]
fn from_maps() {
    let (ny, nx) = (12, 15);
    let grid = Arc::new(terraflux_raster::Grid::new(nx, ny));
    let f = |k: usize| Arc::new((0..ny * nx).map(|i| ((i * (3 + k) + k) % 11) as f64).collect::<Vec<_>>());
    let features = Stack {
        grid: grid.clone(),
        keys: vec![Key::Name("a".into()), Key::Name("b".into())],
        layers: vec![f(0), f(1)],
    };
    let mut cost = vec![1.0; ny * nx];
    cost[0] = f64::NAN;
    let cost = Map::new(grid.clone(), cost);
    let out = run(
        vec![("features", Value::Stack(features)), ("cost", Value::Map(cost))],
        serde_json::json!({"prop": [0.3, 0.3], "blm": 0.5, "num_reps": 3, "num_iters": 20000, "num_temp": 100, "seed": 5}),
    )
    .unwrap();
    let Value::Map(m) = &out["map"] else { panic!() };
    assert!(m.data[0].is_nan(), "no unit where no cost");
    let p = table(&out, "penalties");
    assert_eq!(p.rows.len(), 2);
    // near the targets (30% of each feature's total; annealing of few iterations may fall a little short)
    let Value::Table(s) = &out["summary"] else { panic!() };
    assert_eq!(s.rows.len(), 3);
    let best = table(&out, "best");
    assert_eq!(best.rows.len(), ny * nx - 1);
    for k in 0..2 {
        let layer = f(k);
        let total: f64 = layer.iter().skip(1).sum();
        let kept: f64 = (1..ny * nx).filter(|&c| m.data[c] == 1.0).map(|c| layer[c]).sum();
        assert!(kept >= 0.25 * total, "feature {k}: {kept} of {total}");
    }
    assert!(run(vec![], serde_json::json!({})).is_err());
}
