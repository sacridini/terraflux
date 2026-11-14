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

const NONE: &[&str] = &[];
const BOTH: &[&str] = &["locked_in", "locked_out"];

/// The cases of raster units (prioritizr's R calls in landschaft's optimize_cases.py) as the node's settings.
/// A case: its reference group and name, the node's settings, the locked or data rasters it takes, manual targets.
type Case = (&'static str, &'static str, serde_json::Value, &'static [&'static str], Option<Table>);

/// The phylogenies of the phylogenetic objectives' cases: the tips in the features' order, and in another.
const TREE_ORDERED: &str = "(((f1:1.5,f2:0.5):1,f3:2):0.75,(f4:1.25,f5:3):0.5);";
const TREE_SHUFFLED: &str = "(((f3:1.5,f1:0.5):1,f5:2):0.75,(f2:1.25,f4:3):0.5);";

fn cases() -> Vec<Case> {
    use serde_json::json;
    let abs = json!([1.2, 0.9, 1, 120, 4]);
    let manual = |rows: Vec<[f64; 4]>| {
        let mut t = Table::new(["feature", "relative", "sense", "target"].map(String::from).to_vec());
        t.rows = rows.into_iter().map(|r| r.to_vec()).collect();
        Some(t)
    };
    vec![
        ("core", "minset_raster_relative", json!({"relative_targets": 0.3}), NONE, None),
        ("core", "minset_raster_relative_each", json!({"relative_targets": [0.1, 0.25, 0.5, 0.2, 0.35]}), NONE, None),
        ("core", "minset_raster_absolute", json!({"absolute_targets": abs}), NONE, None),
        ("core", "minset_raster_locked", json!({"relative_targets": 0.3}), BOTH, None),
        ("core", "minset_raster_proportion", json!({"relative_targets": 0.3, "decisions": "proportion"}), NONE, None),
        (
            "core",
            "minset_raster_semicontinuous",
            json!({"relative_targets": 0.3, "decisions": "semicontinuous", "upper_limit": 0.6}),
            NONE,
            None,
        ),
        (
            "core",
            "minset_raster_locked_proportion",
            json!({"relative_targets": 0.3, "decisions": "proportion"}),
            BOTH,
            None,
        ),
        (
            "objectives",
            "min_shortfall",
            json!({"objective": "min_shortfall", "budget": 2500, "relative_targets": 0.4}),
            NONE,
            None,
        ),
        (
            "objectives",
            "min_shortfall_weights_tiny",
            json!({"objective": "min_shortfall", "budget": 2500, "absolute_targets": [1.2, 1e-5, 1, 120, 4],
                   "feature_weights": [1, 2, 0.5, 3, 1]}),
            NONE,
            None,
        ),
        (
            "objectives",
            "min_shortfall_tiny",
            json!({"objective": "min_shortfall", "budget": 2500, "absolute_targets": [1.2, 1e-5, 1, 120, 4]}),
            NONE,
            None,
        ),
        (
            "objectives",
            "min_largest_shortfall",
            json!({"objective": "min_largest_shortfall", "budget": 2500, "relative_targets": [0.3, 0.4, 0.5, 0.2, 0.35]}),
            BOTH,
            None,
        ),
        ("objectives", "max_utility", json!({"objective": "max_utility", "budget": 2500}), NONE, None),
        (
            "objectives",
            "max_utility_weights",
            json!({"objective": "max_utility", "budget": 2500, "feature_weights": [5, 1, 2, 0.01, 0.5],
                   "decisions": "proportion"}),
            NONE,
            None,
        ),
        (
            "objectives",
            "max_features",
            json!({"objective": "max_features", "budget": 2500, "relative_targets": [0.3, 0.4, 0.5, 0.2, 0.35],
                   "feature_weights": [1, 2, 3, 1, 2]}),
            NONE,
            None,
        ),
        ("objectives", "max_cover", json!({"objective": "max_cover", "budget": 800}), NONE, None),
        (
            "objectives",
            "min_penalties",
            json!({"objective": "min_penalties", "budget": 2500, "relative_targets": 0.2}),
            NONE,
            None,
        ),
        (
            "objectives",
            "targets_manual",
            json!({}),
            NONE,
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
            NONE,
            manual(vec![[1.0, 1.0, 0.0, 0.3], [3.0, 0.0, 1.0, 1.1], [5.0, 1.0, 0.0, 0.25]]),
        ),
        ("objectives", "targets_loglinear", json!({"loglinear_targets": [3, 0.6, 300, 0.2]}), NONE, None),
        ("objectives", "targets_loglinear_cap", json!({"loglinear_targets": [3, 0.6, 300, 0.2, 100, 50]}), NONE, None),
        (
            "objectives",
            "max_phylo_div",
            json!({"objective": "max_phylo_div", "budget": 2500, "tree": TREE_ORDERED, "relative_targets": 0.3}),
            NONE,
            None,
        ),
        // prioritizr with its tip-order bug fixed
        (
            "objectives",
            "max_phylo_div_shuffled",
            json!({"objective": "max_phylo_div", "budget": 2500, "tree": TREE_SHUFFLED,
                   "relative_targets": [0.3, 0.4, 0.5, 0.2, 0.35], "feature_weights": [1, 2, 1, 3, 1]}),
            NONE,
            None,
        ),
        (
            "objectives",
            "max_phylo_end",
            json!({"objective": "max_phylo_end", "budget": 2500, "tree": TREE_SHUFFLED, "relative_targets": 0.3}),
            NONE,
            None,
        ),
        (
            "objectives",
            "max_phylo_end_ordered",
            json!({"objective": "max_phylo_end", "budget": 2500, "tree": TREE_ORDERED, "relative_targets": 0.3}),
            NONE,
            None,
        ),
        ("spatial", "boundary_raster", json!({"relative_targets": 0.3, "boundary_penalty": 0.5}), NONE, None),
        (
            "spatial",
            "boundary_raster_edge_max",
            json!({"objective": "max_utility", "budget": 2500, "boundary_penalty": 0.002, "edge_factor": 0.3}),
            &["locked_in"],
            None,
        ),
        (
            "spatial",
            "linear_penalties",
            json!({"relative_targets": 0.3, "linear_penalty": 7.5}),
            &["penalty_data"],
            None,
        ),
        (
            "spatial",
            "linear_constraints",
            json!({"relative_targets": 0.3, "linear_threshold": 250, "linear_sense": "<="}),
            &["constraint_data"],
            None,
        ),
        ("spatial", "neighbors_raster", json!({"relative_targets": 0.3, "neighbors": 2}), NONE, None),
    ]
}

fn args_of(inputs: &BTreeMap<String, Array>, locks: &[&str], targets: Option<Table>) -> BTreeMap<String, Value> {
    let f = &inputs["features"];
    let (ny, nx) = (f.shape[1], f.shape[2]);
    // the cases' grid: cells of 100 m, the top-left corner at (0, rows x 100)
    let mut grid = terraflux_raster::Grid::new(nx, ny);
    grid.transform = [0.0, 100.0, 0.0, ny as f64 * 100.0, 0.0, -100.0];
    let grid = Arc::new(grid);
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
    for &k in locks {
        // the penalty raster feeds the linear penalties and constraint
        let source = if k.ends_with("_data") { "penalty_raster" } else { k };
        out.insert(k.into(), Value::Map(Map::new(grid.clone(), inputs[source].data.clone())));
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
    let refs: BTreeMap<&str, Reference> =
        [("core", reference("core")), ("objectives", reference("objectives")), ("spatial", reference("spatial"))]
            .into();
    for (group, name, params, locks, targets) in cases() {
        let r = &refs[group];
        let ins = args_of(&inputs, locks, targets);
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
const QUICK: [&str; 12] = [
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
    "boundary_raster_edge_max",
    "linear_constraints",
];

/// HiGHS at gap 0 reaches prioritizr's optimum on the quick cases.
#[test]
fn optimum_is_prioritizrs() {
    optima(|name| QUICK.contains(&name));
}

/// Gurobi (`solver = "gurobi"`), where it is installed and licensed, reaches the same optima on the quick cases.
#[test]
fn gurobi_reaches_prioritizrs_optima() {
    if let Err(why) = terraflux_ops::gurobi::available() {
        eprintln!("no Gurobi here: {why}");
        return;
    }
    optima_by(|name| QUICK.contains(&name), "gurobi");
}

/// ... and on every case (minutes: cargo test --release -p terraflux-ops --test ops optimize:: -- --ignored).
#[test]
#[ignore = "minutes at gap 0"]
fn every_optimum_is_prioritizrs() {
    optima(|_| true);
}

/// HiGHS at gap 0 reaches prioritizr's optimum (Gurobi's), or one better by up to 1e-5 in the objectives that
/// maximise (where the cost only breaks ties and Gurobi stops within its tolerances), never worse.
fn optima(which: impl Fn(&str) -> bool) {
    optima_by(which, "highs");
}

/// A solve tells the node how it goes, and stops when the run is stopped: HiGHS and, where it is, Gurobi.
#[test]
fn a_solve_reports_and_stops() {
    let inputs = inputs();
    let registry = terraflux_ops::Registry::standard();
    let mut solvers = vec!["highs"];
    if terraflux_ops::gurobi::available().is_ok() {
        solvers.push("gurobi");
    }
    for solver in solvers {
        let params = serde_json::json!({"relative_targets": 0.3, "boundary_penalty": 0.5, "solver": solver});
        let params = params.as_object().unwrap().clone();
        let run = |watch: &mut dyn FnMut(Option<&terraflux_ops::Progress>) -> bool| {
            let args = Args { node: "n", inputs: args_of(&inputs, NONE, None), params: &params };
            let mut ctx = terraflux_ops::Ctx::watched(watch);
            let out = registry.get("prioritize.optimize").unwrap().run(&mut ctx, &args);
            (out, ctx.was_stopped())
        };
        // told how it goes
        let mut told = Vec::new();
        let (out, stopped) = run(&mut |p| {
            told.extend(p.cloned());
            true
        });
        assert!(out.is_ok() && !stopped, "{solver}");
        assert!(!told.is_empty(), "{solver}: no progress");
        // stopped at once: the node fails, stopped
        let (out, stopped) = run(&mut |_| false);
        let e = out.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(stopped && e.contains("stopped"), "{solver}: {e}");
    }
}

fn optima_by(which: impl Fn(&str) -> bool, solver: &str) {
    let inputs = inputs();
    let refs: BTreeMap<&str, Reference> =
        [("core", reference("core")), ("objectives", reference("objectives")), ("spatial", reference("spatial"))]
            .into();
    let registry = terraflux_ops::Registry::standard();
    for (group, name, params, locks, targets) in cases().into_iter().filter(|c| which(c.1)) {
        let mut params = params.as_object().unwrap().clone();
        params.insert("gap".into(), 0.0.into());
        params.insert("solver".into(), solver.into());
        let args = Args { node: "n", inputs: args_of(&inputs, locks, targets), params: &params };
        let mut ctx = terraflux_ops::Ctx::default();
        let out =
            registry.get("prioritize.optimize").unwrap().run(&mut ctx, &args).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(ctx.warnings.is_empty(), "{name}: {:?}", ctx.warnings);
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

/// prioritizr's evaluation of a solution (eval_*), bit for bit: its units, cost and boundary (with prioritizr's
/// edge factor and another), the features' representation, the targets' coverage, Ferrier's importance and the
/// rarity-weighted richness (rescaled and not), of a binary and a proportional solution.
#[test]
fn evaluation_is_prioritizrs() {
    let inputs = inputs();
    let r = reference("eval");
    let registry = terraflux_ops::Registry::standard();
    let bits = |what: &str, a: &[f64], b: &[f64]| {
        assert_eq!(a.len(), b.len(), "{what}");
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{what}[{i}]: {x} != {y}");
        }
    };
    let cases = [
        ("eval_raster", "sol_raster", serde_json::json!({"relative_targets": 0.3}), true),
        (
            "eval_raster_proportion",
            "solp_raster",
            serde_json::json!({"absolute_targets": [1.2, 0.9, 1, 120, 4], "decisions": "proportion"}),
            false,
        ),
    ];
    for (name, sol, params, full) in cases {
        let run = |extra: serde_json::Value| {
            let mut p = params.as_object().unwrap().clone();
            p.extend(extra.as_object().unwrap().clone());
            let mut ins = args_of(&inputs, NONE, None);
            let g = match &ins["cost"] {
                Value::Map(m) => m.grid.clone(),
                _ => unreachable!(),
            };
            ins.insert("solution".into(), Value::Map(Map::new(g, inputs[sol].data.clone())));
            let args = Args { node: "n", inputs: ins, params: &p };
            registry.get("prioritize.evaluate").unwrap().run(&mut terraflux_ops::Ctx::default(), &args).unwrap()
        };
        let table = |out: &terraflux_ops::Outputs, port: &str, col: &str| {
            let Value::Table(t) = &out[port] else { panic!("{port}") };
            t.column(col).unwrap()
        };
        let k = |s: &str| format!("{name}/{s}");
        let out = run(serde_json::json!({}));
        bits("n", &table(&out, "summary", "n"), &r.floats(&k("n/n")));
        bits("cost", &table(&out, "summary", "cost"), &r.floats(&k("cost/cost")));
        bits("boundary", &table(&out, "summary", "boundary"), &r.floats(&k("boundary/boundary")));
        for col in ["total_amount", "absolute_held", "relative_held"] {
            bits(col, &table(&out, "representation", col), &r.floats(&k(&format!("representation/{col}"))));
        }
        for col in [
            "met",
            "total_amount",
            "absolute_target",
            "absolute_held",
            "absolute_shortfall",
            "relative_target",
            "relative_held",
            "relative_shortfall",
        ] {
            bits(col, &table(&out, "coverage", col), &r.floats(&k(&format!("coverage/{col}"))));
        }
        if full {
            let edge = run(serde_json::json!({"edge_factor": 0.3}));
            bits("boundary 0.3", &table(&edge, "summary", "boundary"), &r.floats(&k("boundary_edge/boundary")));
            let Value::Stack(f) = &out["ferrier"] else { panic!() };
            for (key, layer) in f.keys.iter().zip(&f.layers) {
                bits(&format!("ferrier {key}"), layer, &r.floats(&k(&format!("ferrier/{key}"))));
            }
            let Value::Map(rare) = &out["rarity"] else { panic!() };
            bits("rarity", &rare.data, &r.floats(&k("rare/rwr")));
            let raw = run(serde_json::json!({"rescale": false}));
            let Value::Map(rare) = &raw["rarity"] else { panic!() };
            bits("rarity raw", &rare.data, &r.floats(&k("rare_raw/rwr")));
        }
    }
}

/// prioritizr's replacement importance (a solve without each unit selected) and rank importance (4 budgets) of a
/// min set problem's optimal solution: its numbers (rtol 1e-9, as landschaft's test) and ranks.
#[test]
fn importance_is_prioritizrs() {
    let inputs = inputs();
    let r = reference("eval");
    let registry = terraflux_ops::Registry::standard();
    let close = |what: &str, a: &[f64], b: &[f64]| {
        assert_eq!(a.len(), b.len(), "{what}");
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            let ok = (x.is_nan() && y.is_nan()) || (x - y).abs() <= 1e-9 + 1e-9 * y.abs();
            assert!(ok, "{what}[{i}]: {x} != {y}");
        }
    };
    // the solutions are per planning unit: put on the grid's cells with a cost
    let cost = &inputs["pu_cost"].data;
    let on_grid = |v: &[f64]| {
        let mut out = vec![f64::NAN; cost.len()];
        for (k, c) in (0..cost.len()).filter(|&c| cost[c].is_finite()).enumerate() {
            out[c] = v[k];
        }
        out
    };
    let run = |case: &str, extra: serde_json::Value| {
        let mut p = serde_json::json!({"relative_targets": 0.3, "gap": 0.0}).as_object().unwrap().clone();
        p.extend(extra.as_object().unwrap().clone());
        let mut ins = args_of(&inputs, NONE, None);
        let g = match &ins["cost"] {
            Value::Map(m) => m.grid.clone(),
            _ => unreachable!(),
        };
        ins.insert("solution".into(), Value::Map(Map::new(g, on_grid(&r.floats(&format!("{case}/solution"))))));
        let args = Args { node: "n", inputs: ins, params: &p };
        registry.get("prioritize.evaluate").unwrap().run(&mut terraflux_ops::Ctx::default(), &args).unwrap()
    };
    let map = |out: &terraflux_ops::Outputs, port: &str| match &out[port] {
        Value::Map(m) => m.data.to_vec(),
        _ => panic!("{port}"),
    };
    let raw = run("eval_replacement", serde_json::json!({"replacement": true, "rescale": false}));
    close("rc", &map(&raw, "replacement"), &r.floats("eval_replacement/rc"));
    let scaled = run("eval_replacement", serde_json::json!({"replacement": true}));
    close("rc scaled", &map(&scaled, "replacement"), &r.floats("eval_replacement/rc_scaled"));
    let rank = run("eval_rank", serde_json::json!({"rank": 4, "rescale": false}));
    let Value::Table(info) = &rank["rank_budgets"] else { panic!() };
    assert_eq!(info.column("budget").unwrap(), r.floats("eval_rank/budgets"), "budgets");
    close("objective", &info.column("objective").unwrap(), &r.floats("eval_rank/objective"));
    let ranks = map(&rank, "rank");
    let want = r.floats("eval_rank/rank");
    assert!(ranks.iter().zip(&want).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())), "ranks");
    // asked for Gurobi: its numbers where it solves here, else HiGHS's with the warning why
    let mut p = serde_json::json!({
        "relative_targets": 0.3, "gap": 0.0, "replacement": true, "rank": 4, "rescale": false, "solver": "gurobi",
        "threads": 2
    })
    .as_object()
    .unwrap()
    .clone();
    let mut ins = args_of(&inputs, NONE, None);
    let g = match &ins["cost"] {
        Value::Map(m) => m.grid.clone(),
        _ => unreachable!(),
    };
    ins.insert("solution".into(), Value::Map(Map::new(g, on_grid(&r.floats("eval_rank/solution")))));
    let args = Args { node: "n", inputs: ins, params: &p };
    let mut ctx = terraflux_ops::Ctx::default();
    let out = registry.get("prioritize.evaluate").unwrap().run(&mut ctx, &args).unwrap();
    match terraflux_ops::gurobi::available() {
        Ok(_) => assert!(ctx.warnings.is_empty(), "{:?}", ctx.warnings),
        Err(_) => {
            assert!(ctx.warnings.iter().any(|w| w.starts_with("solved by HiGHS: ")), "no warning: {:?}", ctx.warnings)
        }
    }
    // the same as HiGHS's on the same solution (the objectives at gap 0, the ranks and the replacement costs)
    let inputs = args.inputs;
    p.insert("solver".into(), "highs".into());
    let args = Args { node: "n", inputs, params: &p };
    let want = registry.get("prioritize.evaluate").unwrap().run(&mut terraflux_ops::Ctx::default(), &args).unwrap();
    close("gurobi replacement", &map(&out, "replacement"), &map(&want, "replacement"));
    close("gurobi rank", &map(&out, "rank"), &map(&want, "rank"));
}

/// The model compared with prioritizr's compile(), bit for bit.
fn assert_model(name: &str, m: &terraflux_ops::milp::Model, r: &Reference) {
    let k = |s: &str| format!("{name}/model/{s}");
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
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{name}: {what}[{i}] {x} != {y}");
        }
    };
    same("obj", &m.obj, &r.floats(&k("obj")));
    same("lb", &m.lb, &r.floats(&k("lb")));
    same("ub", &m.ub, &r.floats(&k("ub")));
    same("rhs", &m.rhs, &r.floats(&k("rhs")));
    assert_eq!(m.sense, r.texts(&k("sense")), "{name}: sense");
    assert_eq!(m.vtype.iter().map(|c| c.to_string()).collect::<Vec<_>>(), r.texts(&k("vtype")), "{name}: vtype");
    assert_eq!(m.row_ids, r.texts(&k("row_ids")), "{name}: row ids");
    assert_eq!(m.col_ids, r.texts(&k("col_ids")), "{name}: column ids");
}

fn tables() -> BTreeMap<String, Table> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/optimize_tables.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Table planning units (a table of feature columns, Marxan's rij), boundary, connectivity and adjacency matrices,
/// penalties and constraints in the order added: prioritizr's compile(), bit for bit.
#[test]
fn table_compile_is_prioritizrs() {
    use serde_json::json;
    let t = tables();
    let features = json!(["f1", "f2", "f3", "f4", "f5"]);
    let abs = json!([1.2, 0.9, 1, 120, 4]);
    // pu_table with a change: no cost in some rows, f3 less 0.01
    let pu_with = |f: &dyn Fn(&mut Table)| {
        let mut p = t["pu_table"].clone();
        f(&mut p);
        p
    };
    let na_cost = pu_with(&|p: &mut Table| {
        let c = p.columns.iter().position(|c| c == "cost").unwrap();
        for r in [2, 49, 50, 119] {
            p.rows[r][c] = f64::NAN;
        }
    });
    let negative = pu_with(&|p: &mut Table| {
        let c = p.columns.iter().position(|c| c == "f3").unwrap();
        p.rows.iter_mut().for_each(|r| r[c] -= 0.01);
    });
    let id_cost = pu_with(&|p: &mut Table| {
        let keep: Vec<usize> = ["id", "cost"].iter().map(|n| p.columns.iter().position(|c| c == n).unwrap()).collect();
        p.rows = p.rows.iter().map(|r| keep.iter().map(|&k| r[k]).collect()).collect();
        p.columns = vec!["id".into(), "cost".into()];
    });
    let pu = t["pu_table"].clone();
    type TableCase<'a> = (&'a str, &'a str, serde_json::Value, Table, Vec<(&'a str, &'a str)>);
    let cases: Vec<TableCase> = vec![
        (
            "core",
            "minset_table",
            json!({"features": features, "relative_targets": 0.3, "locked_in_column": "locked_in",
                   "locked_out_column": "locked_out"}),
            pu.clone(),
            vec![],
        ),
        (
            "core",
            "minset_rij",
            json!({"absolute_targets": abs}),
            id_cost.clone(),
            vec![("spec", "spec_table"), ("rij", "rij_table")],
        ),
        (
            "core",
            "minset_rij_relative",
            json!({"relative_targets": 0.3}),
            id_cost,
            vec![("spec", "spec_table"), ("rij", "rij_table")],
        ),
        ("core", "minset_table_na_cost", json!({"features": features, "relative_targets": 0.3}), na_cost, vec![]),
        (
            "objectives",
            "max_utility_negative",
            json!({"features": features, "objective": "max_utility", "budget": 2500}),
            negative,
            vec![],
        ),
        (
            "spatial",
            "boundary_matrix",
            json!({"features": features, "relative_targets": 0.3, "boundary_penalty": 3}),
            pu.clone(),
            vec![("boundary_data", "bnd_table")],
        ),
        (
            "spatial",
            "connectivity",
            json!({"features": features, "relative_targets": 0.3, "connectivity_penalty": 5}),
            pu.clone(),
            vec![("connectivity_data", "conn_table")],
        ),
        (
            "spatial",
            "connectivity_max",
            json!({"features": features, "objective": "max_features", "budget": 2500, "relative_targets": 0.3,
                   "connectivity_penalty": 0.01}),
            pu.clone(),
            vec![("connectivity_data", "conn_table")],
        ),
        (
            "spatial",
            "asym_connectivity",
            json!({"features": features, "relative_targets": 0.3, "asym_connectivity_penalty": 4}),
            pu.clone(),
            vec![("asym_connectivity_data", "asym_table")],
        ),
        (
            "spatial",
            "asym_connectivity_max",
            json!({"features": features, "objective": "max_utility", "budget": 2500, "asym_connectivity_penalty": 0.05}),
            pu.clone(),
            vec![("asym_connectivity_data", "asym_table")],
        ),
        (
            "spatial",
            "spatial_combined",
            json!({"features": features, "objective": "min_shortfall", "budget": 2500, "relative_targets": 0.3,
                   "locked_in_column": "locked_in",
                   "spatial": [
                       {"kind": "linear_constraint", "threshold": 30, "sense": ">=", "column": "f4"},
                       {"kind": "connectivity", "penalty": 0.02},
                       {"kind": "linear_penalty", "penalty": 0.001, "column": "f1"},
                       {"kind": "boundary", "penalty": 0.001},
                       {"kind": "linear_constraint", "threshold": 2, "sense": "<=", "column": "f2"}]}),
            pu.clone(),
            vec![("boundary_data", "bnd_table"), ("connectivity_data", "conn_table")],
        ),
        (
            "spatial",
            "neighbors_matrix",
            json!({"features": features, "objective": "max_utility", "budget": 2500, "neighbors": 5}),
            pu.clone(),
            vec![("adjacency_data", "adj_table")],
        ),
    ];
    let refs: BTreeMap<&str, Reference> =
        [("core", reference("core")), ("objectives", reference("objectives")), ("spatial", reference("spatial"))]
            .into();
    for (group, name, params, pu, extra) in cases {
        let mut ins: BTreeMap<String, Value> = [("pu".to_string(), Value::Table(pu))].into();
        for (port, table) in extra {
            ins.insert(port.into(), Value::Table(t[table].clone()));
        }
        let params = params.as_object().unwrap().clone();
        let args = Args { node: "n", inputs: ins, params: &params };
        let p = terraflux_ops::prioritize::problem_of(&args).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (m, _) = terraflux_ops::milp::compile(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_model(name, &m, &refs[group]);
    }
}

/// prioritizr's portfolios on its table of units: the cuts portfolio (with its no-good cut fixed), top and gap
/// (Gurobi's solution pool) reach the same objectives, distinct solutions; shuffle's are all optimal; the maps of
/// a raster problem's portfolio.
#[test]
fn portfolios_are_prioritizrs() {
    use serde_json::json;
    let t = tables();
    let r = reference("portfolio");
    let registry = terraflux_ops::Registry::standard();
    let try_run = |ins: BTreeMap<String, Value>, params: serde_json::Value| {
        let params = params.as_object().unwrap().clone();
        let args = Args { node: "n", inputs: ins, params: &params };
        registry.get("prioritize.optimize").unwrap().run(&mut terraflux_ops::Ctx::default(), &args)
    };
    let run = |ins, params| try_run(ins, params).unwrap();
    let table = |out: &BTreeMap<String, Value>, name: &str| match &out[name] {
        Value::Table(t) => t.clone(),
        _ => panic!("{name}"),
    };
    let base = json!({"features": ["f1", "f2", "f3", "f4", "f5"], "relative_targets": 0.3, "gap": 0});
    let with = |extra: serde_json::Value| {
        let mut p = base.clone();
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        p
    };
    let pu: BTreeMap<String, Value> = [("pu".to_string(), Value::Table(t["pu_table"].clone()))].into();
    for (name, params) in [
        ("portfolio_cuts_fixed", json!({"portfolio": "cuts", "number_solutions": 4})),
        ("portfolio_top", json!({"portfolio": "top", "number_solutions": 3})),
        ("portfolio_gap", json!({"portfolio": "gap", "number_solutions": 5, "pool_gap": 0.02})),
    ] {
        let out = run(pu.clone(), with(params));
        let summary = table(&out, "summary");
        let ours = summary.column("objective").unwrap();
        let want = r.floats(&format!("{name}/objective"));
        assert_eq!(ours.len(), want.len(), "{name}");
        for (a, b) in ours.iter().zip(&want) {
            assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{name}: {ours:?} != {want:?}");
        }
        let sel = table(&out, "selection");
        let k = want.len();
        assert_eq!(sel.columns.len(), k + 2, "{name}: unit, solution_1.., frequency");
        let mut seen = std::collections::BTreeSet::new();
        for c in 1..=k {
            let v: Vec<u8> = sel.rows.iter().map(|row| row[c].round() as u8).collect();
            assert!(seen.insert(v), "{name}: solution {c} is a duplicate");
        }
        for row in &sel.rows {
            let mean = row[1..=k].iter().sum::<f64>() / k as f64;
            assert!((row[k + 1] - mean).abs() < 1e-12, "{name}: frequency");
        }
    }
    let out = run(pu.clone(), with(json!({"portfolio": "shuffle", "number_solutions": 3, "seed": 2})));
    let obj = table(&out, "summary").column("objective").unwrap();
    assert!(!obj.is_empty() && obj.iter().all(|o| (o - obj[0]).abs() <= 1e-9 * obj[0].abs()), "shuffle: {obj:?}");
    let e = try_run(pu.clone(), with(json!({"portfolio": "cuts", "number_solutions": 2, "decisions": "proportion"})));
    assert!(e.unwrap_err().to_string().contains("binary"));
    let e = try_run(pu.clone(), with(json!({"portfolio": "cuts", "number_solutions": 0})));
    assert!(e.unwrap_err().to_string().contains("whole number"));
    // a raster problem's portfolio: a layer per solution, the frequency on the units only
    let inputs = inputs();
    let out =
        run(args_of(&inputs, NONE, None), json!({"relative_targets": 0.3, "portfolio": "cuts", "number_solutions": 3}));
    let (Value::Stack(st), Value::Map(freq), Value::Map(first)) =
        (&out["solutions"], &out["frequency"], &out["solution"])
    else {
        panic!("maps")
    };
    assert_eq!(st.layers.len(), 3);
    for (i, &f) in freq.data.iter().enumerate() {
        assert_eq!(f.is_nan(), first.data[i].is_nan());
        if !f.is_nan() {
            let mean = st.layers.iter().map(|l| l[i]).sum::<f64>() / 3.0;
            assert!((0.0..=1.0).contains(&f) && (f - mean).abs() < 1e-12);
            assert_eq!(st.layers[0][i], first.data[i]);
        }
    }
}

/// prioritizr's evaluation of a solution of table units, bit for bit: the units, cost, the boundary, connectivity
/// and asymmetric connectivity of matrices, the features' representation and manual targets' coverage; and the
/// replacement importance of a max utility problem with boundary penalties (rtol 1e-9).
#[test]
fn table_evaluation_is_prioritizrs() {
    use serde_json::json;
    let t = tables();
    let r = reference("eval");
    let registry = terraflux_ops::Registry::standard();
    let bits = |what: &str, a: &[f64], b: &[f64]| {
        assert_eq!(a.len(), b.len(), "{what}");
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{what}[{i}]: {x} != {y}");
        }
    };
    let run = |ins: BTreeMap<String, Value>, params: serde_json::Value| {
        let params = params.as_object().unwrap().clone();
        let args = Args { node: "n", inputs: ins, params: &params };
        let mut ctx = terraflux_ops::Ctx::default();
        let out = registry.get("prioritize.evaluate").unwrap().run(&mut ctx, &args).unwrap();
        (out, ctx.warnings)
    };
    let col = |out: &terraflux_ops::Outputs, port: &str, c: &str| {
        let Value::Table(t) = &out[port] else { panic!("{port}") };
        t.column(c).unwrap_or_else(|| panic!("{port}.{c}"))
    };
    let mut targets = Table::new(["feature", "relative", "sense", "target"].map(String::from).to_vec());
    targets.rows = vec![
        vec![1.0, 1.0, 1.0, 0.3],
        vec![2.0, 0.0, -1.0, 2.0],
        vec![4.0, 1.0, 0.0, 0.2],
        vec![4.0, 0.0, -1.0, 120.0],
        vec![5.0, 1.0, 1.0, 0.25],
    ];
    let mut ins: BTreeMap<String, Value> = [
        ("pu", "pu_table"),
        ("selection", "sol_table"),
        ("boundary_data", "bnd_table"),
        ("connectivity_data", "conn_table"),
        ("asym_connectivity_data", "asym_table"),
    ]
    .into_iter()
    .map(|(port, name)| (port.to_string(), Value::Table(t[name].clone())))
    .collect();
    ins.insert("targets".into(), Value::Table(targets));
    let (out, warnings) = run(ins.clone(), json!({"features": ["f1", "f2", "f3", "f4", "f5"]}));
    assert!(warnings.is_empty(), "{warnings:?}");
    let k = |s: &str| format!("eval_table/{s}");
    bits("n", &col(&out, "summary", "n"), &r.floats(&k("n/n")));
    bits("cost", &col(&out, "summary", "cost"), &r.floats(&k("cost/cost")));
    bits("boundary", &col(&out, "summary", "boundary"), &r.floats(&k("boundary/boundary")));
    bits("connectivity", &col(&out, "summary", "connectivity"), &r.floats(&k("connectivity/connectivity")));
    bits("asym", &col(&out, "summary", "asym_connectivity"), &r.floats(&k("asym/asym_connectivity")));
    for c in ["total_amount", "absolute_held", "relative_held"] {
        bits(c, &col(&out, "representation", c), &r.floats(&k(&format!("representation/{c}"))));
    }
    for c in [
        "met",
        "total_amount",
        "absolute_target",
        "absolute_held",
        "absolute_shortfall",
        "relative_target",
        "relative_held",
        "relative_shortfall",
    ] {
        bits(c, &col(&out, "coverage", c), &r.floats(&k(&format!("coverage/{c}"))));
    }
    assert!(!out.contains_key("rarity") && !out.contains_key("ferrier"), "no maps of tables");
    assert_eq!(col(&out, "units", "solution"), t["sol_table"].column("solution_1").unwrap());
    // the matrices swapped: prioritizr's warnings
    let mut swapped = ins.clone();
    swapped.insert("connectivity_data".into(), Value::Table(t["asym_table"].clone()));
    swapped.insert("asym_connectivity_data".into(), Value::Table(t["conn_table"].clone()));
    let (_, warnings) = run(swapped, json!({"features": ["f1", "f2", "f3", "f4", "f5"]}));
    assert_eq!(warnings.len(), 2, "{warnings:?}");

    let mut sol = Table::new(vec!["solution".into()]);
    sol.rows = r.floats("eval_replacement_max/solution").into_iter().map(|v| vec![v]).collect();
    let ins: BTreeMap<String, Value> = [
        ("pu".to_string(), Value::Table(t["pu_table"].clone())),
        ("boundary_data".to_string(), Value::Table(t["bnd_table"].clone())),
        ("selection".to_string(), Value::Table(sol)),
    ]
    .into();
    let (out, _) = run(
        ins,
        json!({"features": ["f1", "f2", "f3", "f4", "f5"], "objective": "max_utility", "budget": 1500,
               "boundary_penalty": 0.001, "replacement": true, "rescale": false, "gap": 0}),
    );
    let ours = col(&out, "units", "replacement");
    let want = r.floats("eval_replacement_max/rc");
    assert_eq!(ours.len(), want.len());
    for (i, (x, y)) in ours.iter().zip(&want).enumerate() {
        assert!((x.is_nan() && y.is_nan()) || (x - y).abs() <= 1e-9 + 1e-9 * y.abs(), "rc[{i}]: {x} != {y}");
    }
}

/// Contiguity: landschaft's exact formulation with the arcs from a lower unit to a higher one fixed at 0 is
/// prioritizr's, the same optimum (Gurobi's at gap 0) on HiGHS's MIP alone (about 2 minutes: cargo test --
/// --ignored).
#[test]
#[ignore]
fn contiguity_restriction_is_prioritizrs() {
    use serde_json::json;
    use terraflux_ops::milp;
    let inputs = inputs();
    let t = tables();
    let r = reference("spatial");
    let restricted = |p: &milp::Problem| {
        let (mut m, _) = milp::compile(p).unwrap();
        m.ub = milp::restricted_bounds(&m);
        m.connected.clear();
        milp::solve(&m, p.units.n(), 0.0, f64::INFINITY).unwrap()
    };
    let problem = |ins: BTreeMap<String, Value>, params: serde_json::Value| {
        let params = params.as_object().unwrap().clone();
        terraflux_ops::prioritize::problem_of(&Args { node: "n", inputs: ins, params: &params }).unwrap()
    };
    let raster = |layers: &[usize]| {
        let mut ins = args_of(&inputs, NONE, None);
        if let Value::Stack(s) = ins.get_mut("features").unwrap() {
            s.keys = layers.iter().map(|&k| s.keys[k].clone()).collect();
            s.layers = layers.iter().map(|&k| s.layers[k].clone()).collect();
        }
        ins
    };
    let table = || -> BTreeMap<String, Value> {
        [("pu", "pu_table"), ("adjacency_data", "adj_table")]
            .into_iter()
            .map(|(port, name)| (port.to_string(), Value::Table(t[name].clone())))
            .collect()
    };
    let cases = [
        ("contiguity_raster", raster(&[0, 1, 2, 3, 4]), json!({"relative_targets": 0.2, "contiguity": true}), false),
        (
            "contiguity_matrix",
            table(),
            json!({"features": ["f1", "f2", "f3", "f4", "f5"], "relative_targets": 0.2, "contiguity": true}),
            false,
        ),
        (
            "feature_contiguity_raster",
            raster(&[1, 2]),
            json!({"relative_targets": 0.2, "feature_contiguity": true}),
            false,
        ),
        (
            "feature_contiguity_matrix",
            table(),
            json!({"features": ["f2", "f3"], "objective": "max_features", "budget": 800, "relative_targets": 0.15,
                   "feature_contiguity": true}),
            true,
        ),
    ];
    for (name, ins, params, maximize) in cases {
        let p = problem(ins, params);
        let s = restricted(&p);
        assert_eq!(s.status, "optimal", "{name}");
        let want = r.floats(&format!("{name}/objective"))[0];
        let close = (s.objective - want).abs() <= 1e-9 * want.abs().max(1.0);
        let better = if maximize { s.objective > want } else { s.objective < want };
        assert!(
            close || (better && (s.objective - want).abs() <= 1e-5 * want.abs().max(1.0)),
            "{name}: {} != {want}",
            s.objective
        );
    }
}

/// Contiguity is exact: it finds the patches prioritizr's restriction misses (three units in a line, the middle one
/// first; the best patch of a 4 x 4 grid by enumeration); and the solve (a first patch, cuts, the MIP) gives one
/// patch.
#[test]
fn contiguity_is_exact() {
    use serde_json::json;
    use terraflux_ops::milp;
    let inputs = inputs();
    let restricted = |p: &milp::Problem| {
        let (mut m, _) = milp::compile(p).unwrap();
        m.ub = milp::restricted_bounds(&m);
        m.connected.clear();
        milp::solve(&m, p.units.n(), 0.0, f64::INFINITY).unwrap()
    };
    let problem = |ins: BTreeMap<String, Value>, params: serde_json::Value| {
        let params = params.as_object().unwrap().clone();
        terraflux_ops::prioritize::problem_of(&Args { node: "n", inputs: ins, params: &params }).unwrap()
    };
    // three units in a line, the middle one first: prioritizr's formulation has no patch
    let mut pu = Table::new(["id", "cost", "a", "b", "c"].map(String::from).to_vec());
    pu.rows = vec![vec![1.0, 1.0, 1.0, 0.0, 0.0], vec![2.0, 1.0, 0.0, 1.0, 0.0], vec![3.0, 1.0, 0.0, 0.0, 1.0]];
    let mut line = Table::new(["i", "j", "x"].map(String::from).to_vec());
    line.rows = vec![vec![1.0, 2.0, 1.0], vec![1.0, 3.0, 1.0], vec![2.0, 1.0, 1.0], vec![3.0, 1.0, 1.0]];
    let ins: BTreeMap<String, Value> =
        [("pu".to_string(), Value::Table(pu)), ("adjacency_data".to_string(), Value::Table(line))].into();
    let p = problem(ins, json!({"features": ["a", "b", "c"], "relative_targets": 1.0, "contiguity": true}));
    let (m, _) = milp::compile(&p).unwrap();
    let s = milp::solve(&m, 3, 0.0, f64::INFINITY).unwrap();
    assert_eq!((s.status.as_str(), s.objective), ("optimal", 3.0));
    assert_eq!(restricted(&p).status, "infeasible");

    // a 4 x 4 grid: the best patch by enumeration
    let side = 4;
    let n = side * side;
    let mut state = 3u64;
    let mut rand = || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    };
    let cost: Vec<f64> = (0..n).map(|_| 1.0 + 9.0 * rand()).collect();
    let mut feats = vec![vec![0.0; n]; 3];
    for f in feats.iter_mut() {
        for v in f.iter_mut() {
            *v = if rand() < 0.25 { 1.0 + rand() } else { 0.0 };
        }
        // opposite corners: the patch must cross the grid
        f[0] += 1.0;
        f[n - 1] += 1.0;
    }
    let mut adj = Vec::new();
    for i in 0..n {
        let (r, c) = (i / side, i % side);
        for (rr, cc) in [(r + 1, c), (r, c + 1)] {
            if rr < side && cc < side {
                let j = rr * side + cc;
                adj.push((i, j));
                adj.push((j, i));
            }
        }
    }
    let mut pu = Table::new(["id", "cost", "f0", "f1", "f2"].map(String::from).to_vec());
    pu.rows = (0..n).map(|i| vec![(i + 1) as f64, cost[i], feats[0][i], feats[1][i], feats[2][i]]).collect();
    let mut am = Table::new(["i", "j", "x"].map(String::from).to_vec());
    am.rows = adj.iter().map(|&(i, j)| vec![(i + 1) as f64, (j + 1) as f64, 1.0]).collect();
    let connected = |sel: &[usize]| {
        let inside: Vec<bool> = (0..n).map(|i| sel.contains(&i)).collect();
        let mut seen = vec![false; n];
        let mut stack = vec![sel[0]];
        seen[sel[0]] = true;
        while let Some(v) = stack.pop() {
            for &(a, b) in &adj {
                if a == v && inside[b] && !seen[b] {
                    seen[b] = true;
                    stack.push(b);
                }
            }
        }
        sel.iter().all(|&i| seen[i])
    };
    let need: Vec<f64> = feats.iter().map(|f| 0.5 * f.iter().sum::<f64>()).collect();
    let mut best = f64::INFINITY;
    for bits in 1u32..(1 << n) {
        let sel: Vec<usize> = (0..n).filter(|&i| bits >> i & 1 == 1).collect();
        let c: f64 = sel.iter().map(|&i| cost[i]).sum();
        if c >= best || feats.iter().zip(&need).any(|(f, &k)| sel.iter().map(|&i| f[i]).sum::<f64>() < k - 1e-9) {
            continue;
        }
        if connected(&sel) {
            best = c;
        }
    }
    let ins: BTreeMap<String, Value> =
        [("pu".to_string(), Value::Table(pu)), ("adjacency_data".to_string(), Value::Table(am))].into();
    let p = problem(ins, json!({"features": ["f0", "f1", "f2"], "relative_targets": 0.5, "contiguity": true}));
    let (m, _) = milp::compile(&p).unwrap();
    let s = milp::solve(&m, n, 0.0, f64::INFINITY).unwrap();
    assert_eq!(s.status, "optimal");
    assert!((s.objective - best).abs() <= 1e-9 * best, "{} != {best}", s.objective);
    assert!(restricted(&p).objective >= best - 1e-9);
    let sel: Vec<usize> = (0..n).filter(|&i| s.x[i] > 0.5).collect();
    assert!(connected(&sel));

    // the raster solved: one patch
    let registry = terraflux_ops::Registry::standard();
    let params = json!({"relative_targets": 0.2, "contiguity": true, "gap": 0.05});
    let args = Args { node: "n", inputs: args_of(&inputs, NONE, None), params: params.as_object().unwrap() };
    let out = registry.get("prioritize.optimize").unwrap().run(&mut terraflux_ops::Ctx::default(), &args).unwrap();
    let Value::Table(summary) = &out["summary"] else { panic!() };
    assert_eq!(summary.column("optimal").unwrap()[0], 1.0);
    let Value::Map(sol) = &out["solution"] else { panic!() };
    let (ny, nx) = (inputs["pu_cost"].shape[0], inputs["pu_cost"].shape[1]);
    let on: Vec<usize> = (0..ny * nx).filter(|&c| sol.data[c] > 0.5).collect();
    let mut seen = vec![false; ny * nx];
    let mut stack = vec![on[0]];
    seen[on[0]] = true;
    while let Some(c) = stack.pop() {
        let (r, k) = (c / nx, c % nx);
        let up = (r > 0).then(|| c - nx);
        let down = (r + 1 < ny).then_some(c + nx);
        let left = (k > 0).then(|| c - 1);
        let right = (k + 1 < nx).then_some(c + 1);
        for d in [up, down, left, right].into_iter().flatten() {
            if sol.data[d] > 0.5 && !seen[d] {
                seen[d] = true;
                stack.push(d);
            }
        }
    }
    assert!(on.iter().all(|&c| seen[c]), "one patch");
}

/// The manual bounds cases: name, inputs, settings.
fn bounds_cases() -> Vec<(&'static str, BTreeMap<String, Value>, serde_json::Value)> {
    use serde_json::json;
    let inputs = inputs();
    let t = tables();
    let mut na_cost = t["pu_table"].clone();
    let c = na_cost.columns.iter().position(|c| c == "cost").unwrap();
    for row in [2, 49, 50, 119] {
        na_cost.rows[row][c] = f64::NAN;
    }
    let raster = |locks: &[&str], extra: &[(&str, &str)]| {
        let mut ins = args_of(&inputs, locks, None);
        for (port, name) in extra {
            ins.insert(port.to_string(), Value::Table(t[*name].clone()));
        }
        ins
    };
    vec![
        (
            "bounds_raster",
            raster(&["locked_in"], &[("bounds", "bounds_cells")]),
            json!({"relative_targets": 0.3, "decisions": "proportion"}),
        ),
        (
            "bounds_locked_raster",
            raster(&["locked_out"], &[("status", "status_cells")]),
            json!({"relative_targets": 0.3}),
        ),
        (
            "bounds_twice",
            raster(&[], &[("bounds", "bounds_cells"), ("status", "status_cells")]),
            json!({"relative_targets": 0.3, "decisions": "proportion"}),
        ),
        (
            "bounds_table_semicontinuous",
            [("pu".to_string(), Value::Table(na_cost)), ("bounds".to_string(), Value::Table(t["bounds_rows"].clone()))]
                .into(),
            json!({"features": ["f1", "f2", "f3", "f4", "f5"], "relative_targets": 0.2, "decisions": "semicontinuous",
                   "upper_limit": 0.6}),
        ),
        (
            "bounds_max_utility",
            raster(&[], &[("status", "status_cells")]),
            json!({"objective": "max_utility", "budget": 2500}),
        ),
        (
            "bounds_boundary",
            raster(&[], &[("status", "status_cells")]),
            json!({"relative_targets": 0.3, "boundary_penalty": 0.5}),
        ),
        (
            "bounds_min_penalties",
            raster(&[], &[("status", "status_cells")]),
            json!({"objective": "min_penalties", "budget": 4000, "relative_targets": 0.2, "boundary_penalty": 0.5}),
        ),
    ]
}

/// prioritizr's manual bounded and locked constraints (after the locked units, the later winning; units without a
/// cost skipped): its compile() bit for bit on the 7 cases landschaft keeps.
#[test]
fn bounds_compile_is_prioritizrs() {
    let r = reference("bounds");
    for (name, ins, params) in bounds_cases() {
        let args = Args { node: "n", inputs: ins, params: params.as_object().unwrap() };
        let p = terraflux_ops::prioritize::problem_of(&args).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (m, _) = terraflux_ops::milp::compile(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_model(name, &m, &r);
    }
}

/// The manual bounds cases' optima are prioritizr's (Gurobi's at gap 0; about 90 s: cargo test -- --ignored).
#[test]
#[ignore]
fn bounds_optima_are_prioritizrs() {
    let r = reference("bounds");
    let registry = terraflux_ops::Registry::standard();
    for (name, ins, params) in bounds_cases() {
        let mut params = params.as_object().unwrap().clone();
        params.insert("gap".into(), 0.0.into());
        let maximize = params.get("objective").and_then(|o| o.as_str()).is_some_and(|o| o.starts_with("max"));
        let args = Args { node: "n", inputs: ins, params: &params };
        let out = registry
            .get("prioritize.optimize")
            .unwrap()
            .run(&mut terraflux_ops::Ctx::default(), &args)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let Value::Table(s) = &out["summary"] else { panic!() };
        let ours = s.column("objective").unwrap()[0];
        let want = r.floats(&format!("{name}/objective"))[0];
        let close = (ours - want).abs() <= 1e-9 * want.abs().max(1.0);
        let better = if maximize { ours > want } else { ours < want };
        assert!(close || (better && (ours - want).abs() <= 1e-5 * want.abs().max(1.0)), "{name}: {ours} != {want}");
    }
}

/// prioritize.calibrate on the raster cases: its candidates and their objectives, and the reference's.
fn calibrate_run(extra: serde_json::Value) -> terraflux_ops::Outputs {
    calibrate_ctx(extra).0
}

/// The same with the run's warnings.
fn calibrate_ctx(extra: serde_json::Value) -> (terraflux_ops::Outputs, Vec<String>) {
    let inputs = inputs();
    let mut params = serde_json::json!({"relative_targets": 0.3, "gap": 0.0}).as_object().unwrap().clone();
    params.extend(extra.as_object().unwrap().clone());
    let args = Args { node: "n", inputs: args_of(&inputs, NONE, None), params: &params };
    let registry = terraflux_ops::Registry::standard();
    let mut ctx = terraflux_ops::Ctx::default();
    let out = registry.get("prioritize.calibrate").unwrap().run(&mut ctx, &args).unwrap();
    (out, ctx.warnings.clone())
}

fn calibrate_column(out: &terraflux_ops::Outputs, c: &str) -> Vec<f64> {
    let Value::Table(t) = &out["table"] else { panic!() };
    t.column(c).unwrap_or_else(|| panic!("{c}"))
}

fn assert_objectives(name: &str, ours: &[f64], want: &[f64]) {
    assert_eq!(ours.len(), want.len(), "{name}");
    for (a, b) in ours.iter().zip(want) {
        assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{name}: {ours:?} != {want:?}");
    }
}

/// Asked for Gurobi (threads 2; the candidates solve in parallel, one thread each): its numbers where it solves
/// here, else HiGHS's with the warning why; the same candidates as HiGHS's at gap 0.
#[test]
fn calibrate_on_gurobi_is_highs() {
    let extra = |solver: &str| serde_json::json!({"approach": "pareto", "values": [500, 1000, 2000, 3000], "solver": solver, "threads": 2});
    let (out, warnings) = calibrate_ctx(extra("gurobi"));
    match terraflux_ops::gurobi::available() {
        Ok(_) => assert!(warnings.is_empty(), "{warnings:?}"),
        Err(_) => assert!(warnings.iter().any(|w| w.starts_with("solved by HiGHS: ")), "no warning: {warnings:?}"),
    }
    let (want, warnings) = calibrate_ctx(extra("highs"));
    assert!(warnings.is_empty(), "{warnings:?}");
    // (not the cost: equal shortfalls can have other optimal solutions)
    for c in ["value", "shortfall", "objective"] {
        assert_objectives(c, &calibrate_column(&out, c), &calibrate_column(&want, c));
    }
}

/// The Pareto front of cost and shortfall: prioritizr's min shortfall optima at its budgets; TOPSIS and the knee
/// of a curve as landschaft's (R's topsis package).
#[test]
fn calibrate_pareto_is_prioritizrs() {
    let r = reference("calibrate");
    let out = calibrate_run(serde_json::json!({"approach": "pareto", "values": [500, 1000, 2000, 3000]}));
    assert_eq!(calibrate_column(&out, "value"), r.floats("calibrate_pareto/budget"));
    assert_objectives("pareto", &calibrate_column(&out, "objective"), &r.floats("calibrate_pareto/objective"));
    // the shortfall is the objective (equal weights), every candidate efficient, one chosen by each method
    for (a, b) in calibrate_column(&out, "shortfall").iter().zip(calibrate_column(&out, "objective")) {
        assert!((a - b).abs() <= 1e-6 * b.max(1.0), "shortfall {a} != {b}");
    }
    assert!(calibrate_column(&out, "efficient").iter().all(|&e| e == 1.0));
    assert_eq!(calibrate_column(&out, "topsis").iter().sum::<f64>(), 1.0);
    assert_eq!(calibrate_column(&out, "knee").iter().sum::<f64>(), 1.0);
    let Value::Stack(s) = &out["solutions"] else { panic!() };
    assert_eq!(s.layers.len(), 4);

    let scores = terraflux_ops::prioritize::topsis(&[vec![1.0, 9.0], vec![4.0, 4.0], vec![9.0, 1.0]]);
    assert!(scores[1] > scores[0] && (scores[0] - scores[2]).abs() < 1e-12, "{scores:?}");
    let knee = terraflux_ops::prioritize::knee_point(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0], &[10.0, 4.0, 2.0, 1.5, 1.2, 1.0]);
    assert_eq!(knee, 2);
}

/// The blended approach (min set with boundary penalties of five weights) and the hierarchical one (the least
/// boundary within cost thresholds above the least cost, rounded up): prioritizr's optima, thresholds and least
/// cost (Gurobi's at gap 0; about 4 minutes: cargo test -- --ignored).
#[test]
#[ignore]
fn calibrate_blended_and_hierarchical_are_prioritizrs() {
    let r = reference("calibrate");
    let out = calibrate_run(serde_json::json!({"approach": "blended", "values": [0.001, 0.01, 0.1, 1, 10]}));
    assert_objectives("blended", &calibrate_column(&out, "objective"), &r.floats("calibrate_blended/objective"));
    let out = calibrate_run(serde_json::json!({"approach": "hierarchical"}));
    assert_eq!(calibrate_column(&out, "value"), r.floats("calibrate_hierarchical/threshold"));
    assert_objectives(
        "hierarchical",
        &calibrate_column(&out, "objective"),
        &r.floats("calibrate_hierarchical/objective"),
    );
    let cohon = calibrate_run(serde_json::json!({"approach": "cohon"}));
    let cost = calibrate_column(&cohon, "cost");
    assert_objectives("least cost", &cost[..1], &r.floats("calibrate_hierarchical/s1_cost"));
    assert!(calibrate_column(&cohon, "penalty")[0] > 0.0);
}

/// A synthetic table problem for the subset search: `n` units, `k` features of small whole amounts (an integer
/// generator: the same numbers on every machine), the targets and the budget whole numbers (a share of the totals,
/// rounded down) so that landschaft's and our models are the same to the last bit.
fn subset_problem(
    n: usize,
    k: usize,
    share: f64,
    budget_share: f64,
    objective: &str,
    tree: Option<String>,
    weights: bool,
) -> (terraflux_ops::milp::Model, usize) {
    let mut seed: u64 = 12345;
    let mut next = |m: u64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) % m
    };
    let names: Vec<String> = (1..=k).map(|f| format!("f{f}")).collect();
    let mut columns = vec!["id".to_string(), "cost".to_string()];
    columns.extend(names.iter().cloned());
    let mut pu = Table::new(columns);
    let mut totals = vec![0.0; k];
    let mut total_cost = 0.0;
    for i in 0..n {
        let cost = (1 + next(100)) as f64;
        total_cost += cost;
        let mut row = vec![(i + 1) as f64, cost];
        for t in totals.iter_mut() {
            let a = if next(100) < 20 { (1 + next(9)) as f64 } else { 0.0 };
            *t += a;
            row.push(a);
        }
        pu.rows.push(row);
    }
    let targets: Vec<f64> = totals.iter().map(|t| (t * share).floor().max(1.0)).collect();
    let mut params = serde_json::json!({"features": names, "objective": objective,
        "budget": (total_cost * budget_share).floor(), "absolute_targets": targets});
    if let Some(t) = tree {
        params["tree"] = t.into();
    }
    if weights {
        params["feature_weights"] = (0..k).map(|f| (1 + f % 4) as f64).collect::<Vec<_>>().into();
    }
    let params = params.as_object().unwrap().clone();
    let ins: BTreeMap<String, Value> = [("pu".to_string(), Value::Table(pu))].into();
    let args = Args { node: "n", inputs: ins, params: &params };
    let p = terraflux_ops::prioritize::problem_of(&args).unwrap();
    let (m, _) = terraflux_ops::milp::compile(&p).unwrap();
    (m, p.units.n())
}

/// A phylogeny of the features f1..fk: a ladder, the tips joined one after the other, with branch lengths that are
/// whole multiples of a quarter.
fn ladder_tree(k: usize) -> String {
    let mut t = "f1:1.5".to_string();
    for i in 2..=k {
        t = format!("({t},f{i}:{}):{}", 0.5 + 0.25 * (i % 5) as f64, 0.25 + 0.5 * (i % 3) as f64);
    }
    // (no length for the root)
    format!("{};", &t[..t.rfind(':').unwrap()])
}

/// The solution's number of targets met and of units chosen.
fn met_and_units(m: &terraflux_ops::milp::Model, n: usize, x: &[f64]) -> (usize, usize) {
    let met = (0..x.len()).filter(|&c| m.col_ids[c] == "spp_met" && x[c] > 0.5).count();
    (met, x[..n].iter().filter(|&&v| v > 0.5).count())
}

/// The subset problems of the tests below: 60 units, 20 features (weights 1 to 4), every target 30 % of the feature's
/// total and the budget 10 % of the total cost (whole numbers: the model is the same to the last bit in landschaft),
/// the objective, and for the phylogenetic ones a ladder tree.
fn subset_case(objective: &str) -> (terraflux_ops::milp::Model, usize) {
    let tree = objective.starts_with("max_phylo").then(|| ladder_tree(20));
    subset_problem(60, 20, 0.3, 0.1, objective, tree, true)
}

/// landschaft's subset search (prioritize/_milp_subset.py of 1.58.1) from a selection of the units, until no move
/// helps (no time limit: the search ends because no move helps, so the result does not depend on the machine's
/// speed): the same objective, targets met and units chosen. landschaft's numbers: the models of `subset_case` as
/// Terraflux compiles them (every array written with {:e} and read back: A, obj, lb, ub, rhs, sense, vtype, ids,
/// the same to the last bit) in landschaft's OptimizationProblem(modelsense="max"), sm = subset_model(m, n);
/// z = _complete(m, sm, xu) from the units' values (none chosen, or units 1, 9 and 17) then
/// improve_subset(m, sm, z, math.inf, 4) with OMP_NUM_THREADS=4: max_features 37.99900535833892 (14 targets, 15
/// units; 1.7 s there), max_phylo_div 66.74976054922973 (14, 15; 1.3 s), max_phylo_end 1603.4763773041454 (14, 13;
/// 1.6 s). Searching again from what it found, no move helps: None.
#[test]
fn subset_search_is_landschafts() {
    let cases = [
        ("max_features", 37.99900535833892, (14, 15)),
        ("max_phylo_div", 66.74976054922973, (14, 15)),
        ("max_phylo_end", 1603.4763773041454, (14, 13)),
    ];
    for (objective, want, counts) in cases {
        let (m, n) = subset_case(objective);
        for start in [vec![], vec![1, 9, 17]] {
            let mut xu = vec![0.0; n];
            start.iter().for_each(|&u| xu[u] = 1.0);
            let (x, obj) = terraflux_ops::milp::improve_subset(&m, n, &xu).unwrap().unwrap();
            assert!((obj - want).abs() <= 1e-9 * want, "{objective}: {obj} != {want}");
            assert_eq!(met_and_units(&m, n, &x), counts, "{objective}");
            assert!(terraflux_ops::milp::improve_subset(&m, n, &x[..n]).unwrap().is_none(), "{objective}");
        }
    }
}

/// Only a maximisation of the targets met under one budget takes the subset search: the other objectives, a model
/// with another budget row or continuous targets are refused.
#[test]
fn subset_search_takes_only_its_models() {
    let ok = |m: &terraflux_ops::milp::Model, n: usize| terraflux_ops::milp::improve_subset(m, n, &vec![0.0; n]);
    for objective in ["min_set", "min_shortfall", "min_largest_shortfall", "max_utility", "max_cover", "min_penalties"]
    {
        let (m, n) = subset_case(objective);
        assert!(ok(&m, n).is_err(), "{objective}");
    }
    let (m, n) = subset_case("max_features");
    assert!(ok(&m, n).is_ok());
    let mut broken = m.clone();
    let budget = broken.row_ids.iter().position(|r| r == "budget").unwrap();
    broken.sense[budget] = ">=".into();
    assert!(ok(&broken, n).is_err(), "a budget that is a minimum");
    let mut broken = m.clone();
    let flag = broken.col_ids.iter().position(|c| c == "spp_met").unwrap();
    broken.vtype[flag] = 'C';
    assert!(ok(&broken, n).is_err(), "a continuous target");
    let mut broken = m.clone();
    broken.maximize = false;
    assert!(ok(&broken, n).is_err(), "a minimisation");
    let mut broken = m.clone();
    broken.col_ids.iter_mut().filter(|c| *c == "spp_met").for_each(|c| *c = "other".into());
    assert!(ok(&broken, n).is_err(), "no targets met");
    let mut broken = m.clone();
    let k = broken.a.index.iter().position(|&c| c < n).unwrap();
    broken.a.value[k] = 1.0;
    broken.a.index[k] = n + 1;
    assert!(ok(&broken, n).is_err(), "a target's row that holds another target's flag");
    // and a model with a connected patch or zones goes the old way: no subset search there
    assert!(terraflux_ops::milp::improve_subset(&m, 0, &[]).is_err());
}

/// A model the wide engine proves optimal (the gap reached) is returned as it is, as landschaft's
/// _wide_then_subset does: landschaft's solve_model on the same model (subset_case("max_features"), gap 0.01,
/// time limit 60, n_threads 4): optimal, objective 37.99904219691896, bound 38.0, 14 targets, 15 units.
#[test]
fn subset_models_the_wide_engine_proves_come_back_as_they_are() {
    let (m, n) = subset_case("max_features");
    let s = terraflux_ops::milp::solve(&m, n, 0.01, 60.0).unwrap();
    assert_eq!(s.status, "optimal");
    let (objective, bound) = (37.99904219691896, 38.0);
    assert!((s.objective - objective).abs() <= 1e-9 * objective, "objective {} != {objective}", s.objective);
    assert!((s.bound - bound).abs() <= 1e-9 * bound, "bound {} != {bound}", s.bound);
    assert_eq!(met_and_units(&m, n, &s.x), (14, 15));
}

/// The wide engine for 40 % of the time limit, then the search for the rest: on the phylogenetic diversity model with
/// a time limit of 2 s the wide engine stops at 65.9997689216343 (its bound 68.4 there, 69.0 here: the bound depends
/// on the speed) and the search reaches 66.74976054922973 (14 targets, 15 units), landschaft's solve_model on the
/// same model (gap 0.01, time limit 2, n_threads 4): time_limit, objective 66.74976054922973, 15 units, 2.0 s. (On a
/// faster machine the wide engine alone gets there: the same objective.) Where the run ends depends on the speed (a
/// slow machine, the CI, may stop the search sooner): checked only that it ends no worse than the wide engine, the
/// search itself compared with landschaft's in subset_search_is_landschafts. Stopped, the search ends with the run.
#[test]
fn the_search_goes_on_from_the_wide_engine_and_stops_with_the_run() {
    let (m, n) = subset_case("max_phylo_div");
    let s = terraflux_ops::milp::solve(&m, n, 0.01, 2.0).unwrap();
    let (wide, here) = (65.9997689216343, 66.74976054922973);
    assert!(s.objective >= wide * (1.0 - 1e-9), "objective {} below the wide engine's {wide}", s.objective);
    if (s.objective - here).abs() <= 1e-9 * here {
        assert_eq!(met_and_units(&m, n, &s.x), (14, 15));
    }
    // stopped once the search asks (it asks whether to go on, telling nothing): the solve fails, stopped
    let mut asked = false;
    let r = terraflux_ops::milp::watching(
        &mut |p| {
            asked |= p.best.is_nan() && p.bound.is_nan();
            !asked
        },
        || terraflux_ops::milp::solve(&m, n, 0.01, 2.0),
    );
    match r {
        Err(e) => assert_eq!(e, terraflux_ops::milp::STOPPED),
        // (a machine fast enough for the wide engine to prove the gap alone: no search to stop)
        Ok(s) => assert!(!asked && s.status == "optimal"),
    }
}
