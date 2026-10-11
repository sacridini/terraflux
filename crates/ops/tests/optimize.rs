//! prioritize.optimize against prioritizr 8.0.6: the models its compile() gives (extern/landschaft/tests/data/
//! reference/optimize, core and objectives) for raster planning units, bit for bit, and their optima (Gurobi at
//! gap 0) reached by HiGHS; on the rasters of landschaft's cases (tests/data/reference/optimize_inputs.npz,
//! tools/make_optimize_inputs.py).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Key, Map, Stack, Table, Value};
use terraflux_raster::npy::Array;

struct Reference {
    arrays: BTreeMap<String, Array>,
    meta: serde_json::Value,
}

fn reference(group: &str) -> Reference {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extern/landschaft/tests/data/reference/optimize");
    Reference {
        arrays: terraflux_raster::npy::read_npz(dir.join(format!("{group}.npz"))).unwrap(),
        meta: serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{group}.json"))).unwrap()).unwrap(),
    }
}

impl Reference {
    fn floats(&self, key: &str) -> Vec<f64> {
        self.arrays[key].data.clone()
    }
    fn texts(&self, key: &str) -> Vec<String> {
        self.meta["arrays"][key]["values"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect()
    }
}

fn inputs() -> BTreeMap<String, Array> {
    terraflux_raster::npy::read_npz(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/optimize_inputs.npz"),
    )
    .unwrap()
}

/// The cases of raster units (prioritizr's R calls in landschaft's optimize_cases.py) as the node's settings.
fn cases() -> Vec<(&'static str, &'static str, serde_json::Value, bool, Option<Table>)> {
    use serde_json::json;
    let abs = json!([1.2, 0.9, 1, 120, 4]);
    let manual = |rows: Vec<[f64; 4]>| {
        let mut t = Table::new(["feature", "relative", "sense", "target"].map(String::from).to_vec());
        t.rows = rows.into_iter().map(|r| r.to_vec()).collect();
        Some(t)
    };
    vec![
        ("core", "minset_raster_relative", json!({"relative_targets": 0.3}), false, None),
        ("core", "minset_raster_relative_each", json!({"relative_targets": [0.1, 0.25, 0.5, 0.2, 0.35]}), false, None),
        ("core", "minset_raster_absolute", json!({"absolute_targets": abs}), false, None),
        ("core", "minset_raster_locked", json!({"relative_targets": 0.3}), true, None),
        ("core", "minset_raster_proportion", json!({"relative_targets": 0.3, "decisions": "proportion"}), false, None),
        (
            "core",
            "minset_raster_semicontinuous",
            json!({"relative_targets": 0.3, "decisions": "semicontinuous", "upper_limit": 0.6}),
            false,
            None,
        ),
        (
            "core",
            "minset_raster_locked_proportion",
            json!({"relative_targets": 0.3, "decisions": "proportion"}),
            true,
            None,
        ),
        (
            "objectives",
            "min_shortfall",
            json!({"objective": "min_shortfall", "budget": 2500, "relative_targets": 0.4}),
            false,
            None,
        ),
        (
            "objectives",
            "min_shortfall_weights_tiny",
            json!({"objective": "min_shortfall", "budget": 2500, "absolute_targets": [1.2, 1e-5, 1, 120, 4],
                   "feature_weights": [1, 2, 0.5, 3, 1]}),
            false,
            None,
        ),
        (
            "objectives",
            "min_shortfall_tiny",
            json!({"objective": "min_shortfall", "budget": 2500, "absolute_targets": [1.2, 1e-5, 1, 120, 4]}),
            false,
            None,
        ),
        (
            "objectives",
            "min_largest_shortfall",
            json!({"objective": "min_largest_shortfall", "budget": 2500, "relative_targets": [0.3, 0.4, 0.5, 0.2, 0.35]}),
            true,
            None,
        ),
        ("objectives", "max_utility", json!({"objective": "max_utility", "budget": 2500}), false, None),
        (
            "objectives",
            "max_utility_weights",
            json!({"objective": "max_utility", "budget": 2500, "feature_weights": [5, 1, 2, 0.01, 0.5],
                   "decisions": "proportion"}),
            false,
            None,
        ),
        (
            "objectives",
            "max_features",
            json!({"objective": "max_features", "budget": 2500, "relative_targets": [0.3, 0.4, 0.5, 0.2, 0.35],
                   "feature_weights": [1, 2, 3, 1, 2]}),
            false,
            None,
        ),
        ("objectives", "max_cover", json!({"objective": "max_cover", "budget": 800}), false, None),
        (
            "objectives",
            "min_penalties",
            json!({"objective": "min_penalties", "budget": 2500, "relative_targets": 0.2}),
            false,
            None,
        ),
        (
            "objectives",
            "targets_manual",
            json!({}),
            false,
            manual(vec![
                [1.0, 1.0, 1.0, 0.3],
                [2.0, 0.0, -1.0, 2.0],
                [4.0, 1.0, 1.0, 0.2],
                [4.0, 0.0, -1.0, 120.0],
                [5.0, 1.0, 1.0, 0.25],
            ]),
        ),
        (
            "objectives",
            "targets_manual_equal",
            json!({"decisions": "proportion"}),
            false,
            manual(vec![[1.0, 1.0, 0.0, 0.3], [3.0, 0.0, 1.0, 1.1], [5.0, 1.0, 0.0, 0.25]]),
        ),
        ("objectives", "targets_loglinear", json!({"loglinear_targets": [3, 0.6, 300, 0.2]}), false, None),
        ("objectives", "targets_loglinear_cap", json!({"loglinear_targets": [3, 0.6, 300, 0.2, 100, 50]}), false, None),
    ]
}

fn args_of(inputs: &BTreeMap<String, Array>, locked: bool, targets: Option<Table>) -> BTreeMap<String, Value> {
    let f = &inputs["features"];
    let (ny, nx) = (f.shape[1], f.shape[2]);
    let grid = Arc::new(terraflux_raster::Grid::new(nx, ny));
    let mut out = BTreeMap::new();
    out.insert(
        "features".to_string(),
        Value::Stack(Stack {
            grid: grid.clone(),
            keys: (1..=5).map(|k| Key::Name(format!("f{k}"))).collect(),
            layers: f.data.chunks(ny * nx).map(|c| Arc::new(c.to_vec())).collect(),
        }),
    );
    out.insert("cost".into(), Value::Map(Map::new(grid.clone(), inputs["pu_cost"].data.clone())));
    if locked {
        for k in ["locked_in", "locked_out"] {
            out.insert(k.into(), Value::Map(Map::new(grid.clone(), inputs[k].data.clone())));
        }
    }
    if let Some(t) = targets {
        out.insert("targets".into(), Value::Table(t));
    }
    out
}

/// The same model as prioritizr's compile(), bit for bit (the matrix in canonical order, without the zeros
/// prioritizr stores), with the same row and column ids.
#[test]
fn compile_is_prioritizrs() {
    let inputs = inputs();
    let refs: BTreeMap<&str, Reference> = [("core", reference("core")), ("objectives", reference("objectives"))].into();
    for (group, name, params, locked, targets) in cases() {
        let r = &refs[group];
        let ins = args_of(&inputs, locked, targets);
        let params = params.as_object().unwrap().clone();
        let args = Args { node: "n", inputs: ins, params: &params };
        let p = terraflux_ops::prioritize::problem_of(&args).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (m, _) = terraflux_ops::milp::compile(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        let k = |s: &str| format!("{name}/model/{s}");
        // the reference's entries without zeros, by row then column
        let (ai, aj, ax) = (r.floats(&k("A_i")), r.floats(&k("A_j")), r.floats(&k("A_x")));
        let mut entries: Vec<(usize, usize, f64)> =
            (0..ax.len()).filter(|&e| ax[e] != 0.0).map(|e| (ai[e] as usize, aj[e] as usize, ax[e])).collect();
        entries.sort_by_key(|&(i, j, _)| (i, j));
        let mut ours = Vec::new();
        for i in 0..m.a.nrow {
            let (idx, v) = m.a.row(i);
            ours.extend(idx.iter().zip(v).map(|(&j, &x)| (i, j, x)));
        }
        assert_eq!(ours.len(), entries.len(), "{name}: entries");
        for (a, b) in ours.iter().zip(&entries) {
            assert!(a.0 == b.0 && a.1 == b.1 && a.2.to_bits() == b.2.to_bits(), "{name}: {a:?} != {b:?}");
        }
        let same = |what: &str, a: &[f64], b: &[f64]| {
            assert_eq!(a.len(), b.len(), "{name}: {what}");
            for (x, y) in a.iter().zip(b) {
                assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{name}: {what} {x} != {y}");
            }
        };
        same("obj", &m.obj, &r.floats(&k("obj")));
        same("lb", &m.lb, &r.floats(&k("lb")));
        same("ub", &m.ub, &r.floats(&k("ub")));
        same("rhs", &m.rhs, &r.floats(&k("rhs")));
        assert_eq!(m.sense, r.texts(&k("sense")), "{name}: sense");
        assert_eq!(m.vtype.iter().map(|c| c.to_string()).collect::<Vec<_>>(), r.texts(&k("vtype")), "{name}: vtype");
        assert_eq!(if m.maximize { "max" } else { "min" }, r.texts(&k("modelsense"))[0], "{name}: sense");
        assert_eq!(m.row_ids, r.texts(&k("row_ids")), "{name}: row ids");
        assert_eq!(m.col_ids, r.texts(&k("col_ids")), "{name}: column ids");
    }
}

/// The cases HiGHS solves at gap 0 in under a few seconds (the others take up to half a minute).
const QUICK: [&str; 10] = [
    "minset_raster_relative",
    "minset_raster_absolute",
    "minset_raster_proportion",
    "minset_raster_semicontinuous",
    "minset_raster_locked_proportion",
    "min_largest_shortfall",
    "max_utility",
    "max_utility_weights",
    "min_penalties",
    "targets_manual_equal",
];

/// HiGHS at gap 0 reaches prioritizr's optimum on the quick cases.
#[test]
fn optimum_is_prioritizrs() {
    optima(|name| QUICK.contains(&name));
}

/// ... and on every case (minutes: cargo test --release -p terraflux-ops --test optimize -- --ignored).
#[test]
#[ignore = "minutes at gap 0"]
fn every_optimum_is_prioritizrs() {
    optima(|_| true);
}

/// HiGHS at gap 0 reaches prioritizr's optimum (Gurobi's), or one better by up to 1e-5 in the objectives that
/// maximise (where the cost only breaks ties and Gurobi stops within its tolerances), never worse.
fn optima(which: impl Fn(&str) -> bool) {
    let inputs = inputs();
    let refs: BTreeMap<&str, Reference> = [("core", reference("core")), ("objectives", reference("objectives"))].into();
    let registry = terraflux_ops::Registry::standard();
    for (group, name, params, locked, targets) in cases().into_iter().filter(|c| which(c.1)) {
        let mut params = params.as_object().unwrap().clone();
        params.insert("gap".into(), 0.0.into());
        let args = Args { node: "n", inputs: args_of(&inputs, locked, targets), params: &params };
        let out = registry
            .get("prioritize.optimize")
            .unwrap()
            .run(&mut terraflux_ops::Ctx::default(), &args)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let Value::Table(s) = &out["summary"] else { panic!() };
        let ours = s.column("objective").unwrap()[0];
        let want = refs[group].floats(&format!("{name}/objective"))[0];
        let maximize = params.get("objective").and_then(|o| o.as_str()).is_some_and(|o| o.starts_with("max"));
        let close = (ours - want).abs() <= 1e-9 * want.abs().max(1.0);
        let better = if maximize { ours > want } else { ours < want };
        assert!(close || (better && (ours - want).abs() <= 1e-5 * want.abs().max(1.0)), "{name}: {ours} != {want}");
        assert_eq!(s.column("optimal").unwrap()[0], 1.0, "{name}");
    }
}
