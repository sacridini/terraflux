//! prioritize.optimize with management zones against prioritizr 8.0.6: the models its compile() gives
//! (extern/landschaft/tests/data/reference/optimize/zones) bit for bit, and their optima (Gurobi's at gap 0); on the
//! zone problems of landschaft's cases (tests/data/reference/optimize_zones.npz and optimize_zone_tables.json,
//! tools/make_optimize_inputs.py): three zones (protect, use, restore) on a 12 x 15 grid, every zone's own features.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use terraflux_ops::{Args, Key, Stack, Table, Value};
use terraflux_raster::npy::Array;

struct Reference {
    arrays: BTreeMap<String, Array>,
    meta: serde_json::Value,
}

fn reference() -> Reference {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extern/landschaft/tests/data/reference/optimize");
    Reference {
        arrays: terraflux_raster::npy::read_npz(dir.join("zones.npz")).unwrap(),
        meta: serde_json::from_str(&std::fs::read_to_string(dir.join("zones.json")).unwrap()).unwrap(),
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

fn data(file: &str) -> BTreeMap<String, Array> {
    terraflux_raster::npy::read_npz(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference").join(file))
        .unwrap()
}

/// The tables (null: no value).
fn tables() -> BTreeMap<String, Table> {
    #[derive(serde::Deserialize)]
    struct Raw {
        columns: Vec<String>,
        rows: Vec<Vec<Option<f64>>>,
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/optimize_zone_tables.json");
    let raw: BTreeMap<String, Raw> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    raw.into_iter()
        .map(|(k, r)| {
            let rows = r.rows.into_iter().map(|row| row.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect()).collect();
            (k, Table { columns: r.columns, rows, ..Default::default() })
        })
        .collect()
}

const ZONES: [&str; 3] = ["protect", "use", "restore"];
const FEATURES: [&str; 5] = ["f1", "f2", "f3", "f4", "f5"];
const TREE_SHUFFLED: &str = "(((f3:1.5,f1:0.5):1,f5:2):0.75,(f2:1.25,f4:3):0.5);";

/// The landschaft cases' inputs: the zones' costs and features as stacks on the 12 x 15 grid of 100 m cells.
struct Inputs {
    zones: BTreeMap<String, Array>,
    base: BTreeMap<String, Array>,
    tables: BTreeMap<String, Table>,
    grid: Arc<terraflux_raster::Grid>,
}

impl Inputs {
    fn new() -> Inputs {
        let zones = data("optimize_zones.npz");
        let (ny, nx) = (zones["zone_cost"].shape[1], zones["zone_cost"].shape[2]);
        let mut grid = terraflux_raster::Grid::new(nx, ny);
        grid.transform = [0.0, 100.0, 0.0, ny as f64 * 100.0, 0.0, -100.0];
        Inputs { zones, base: data("optimize_inputs.npz"), tables: tables(), grid: Arc::new(grid) }
    }

    fn stack(&self, a: &Array, keys: Vec<String>) -> Value {
        let n = self.grid.width * self.grid.height;
        Value::Stack(Stack {
            grid: self.grid.clone(),
            keys: keys.into_iter().map(Key::Name).collect(),
            layers: a.data.chunks(n).map(|c| Arc::new(c.to_vec())).collect(),
        })
    }

    /// The zone problem of rasters: the costs of the first `nz` zones and their features.
    fn raster(&self, nz: usize) -> BTreeMap<String, Value> {
        let n = self.grid.width * self.grid.height;
        let cost = &self.zones["zone_cost"];
        let cost = Array { shape: vec![nz, cost.shape[1], cost.shape[2]], data: cost.data[..nz * n].to_vec() };
        let zf = &self.zones["zone_features"];
        let zf = Array { shape: vec![nz * 5, zf.shape[1], zf.shape[2]], data: zf.data[..nz * 5 * n].to_vec() };
        let keys = (0..nz).flat_map(|z| FEATURES.iter().map(move |f| format!("{}_{f}", ZONES[z]))).collect();
        [
            ("zone_costs".to_string(), self.stack(&cost, ZONES[..nz].iter().map(|z| z.to_string()).collect())),
            ("zone_features".to_string(), self.stack(&zf, keys)),
        ]
        .into()
    }

    fn with(&self, mut ins: BTreeMap<String, Value>, extra: &[(&str, &str)]) -> BTreeMap<String, Value> {
        for (port, name) in extra {
            let v = match self.zones.get(*name) {
                Some(a) => self.stack(a, ZONES.iter().map(|z| z.to_string()).collect()),
                None => Value::Table(self.tables[*name].clone()),
            };
            ins.insert(port.to_string(), v);
        }
        ins
    }

    /// landschaft's sum_targets: an absolute target per feature, a share of its total (terra's global sum of the
    /// base feature), met by the sum over the zones.
    fn sum_targets(&self, share: f64, zones: &[&str]) -> serde_json::Value {
        let f = &self.base["features"];
        let layers: Vec<&[f64]> = f.data.chunks(f.shape[1] * f.shape[2]).collect();
        let u = terraflux_ops::milp::Units::raster(
            &self.base["pu_cost"].data,
            &layers,
            FEATURES.iter().map(|s| s.to_string()).collect(),
            (f.shape[1], f.shape[2]),
            [0.0; 6],
        )
        .unwrap();
        json!(
            FEATURES
                .iter()
                .zip(&u.totals)
                .map(|(f, t)| json!({"feature": f, "zone": zones, "type": "absolute", "target": share * t}))
                .collect::<Vec<_>>()
        )
    }
}

/// R's matrix(c(...), ncol = 3) as features x zones.
fn matrix(v: [f64; 15]) -> serde_json::Value {
    json!((0..5).map(|f| (0..3).map(|z| v[z * 5 + f]).collect::<Vec<_>>()).collect::<Vec<_>>())
}

const ZONE_MATRIX: [f64; 15] = [0.2, 0.1, 0.15, 0.3, 0.1, 0.1, 0.1, 0.05, 0.2, 0.1, 0.05, 0.0, 0.1, 0.05, 0.05];
const ZONE_WEIGHTS: [f64; 15] = [5.0, 1.0, 2.0, 0.5, 1.0, 4.0, 1.0, 1.0, 0.5, 2.0, 3.0, 0.5, 2.0, 1.0, 1.0];

fn manual_targets() -> serde_json::Value {
    json!([
        {"feature": "f1", "zone": ["protect", "use"], "type": "relative", "sense": ">=", "target": 0.2},
        {"feature": "f2", "zone": "protect", "type": "absolute", "sense": ">=", "target": 1},
        {"feature": "f3", "zone": ZONES, "type": "relative", "sense": ">=", "target": 0.25},
        {"feature": "f4", "zone": "use", "type": "relative", "sense": "<=", "target": 0.4},
        {"feature": "f4", "zone": ["protect", "restore"], "type": "relative", "sense": ">=", "target": 0.2},
        {"feature": "f5", "zone": "restore", "type": "absolute", "sense": ">=", "target": 2},
        {"feature": "f5", "zone": "use", "type": "relative", "sense": "<=", "target": 0.3}
    ])
}

/// The cases without spatial terms: name, inputs, settings.
fn cases(i: &Inputs) -> Vec<(&'static str, BTreeMap<String, Value>, serde_json::Value)> {
    let names = json!(FEATURES);
    let sum = |t: f64| i.sum_targets(t, &ZONES);
    let with = |extra: serde_json::Value| {
        let mut p = json!({"feature_names": names});
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        p
    };
    let table_features: serde_json::Map<String, serde_json::Value> =
        ZONES.iter().map(|z| (z.to_string(), json!(FEATURES.map(|f| format!("{}_{f}", &z[..1]))))).collect();
    let cost_columns = json!(ZONES.map(|z| format!("cost_{z}")));
    let zone_table = |cols: bool| {
        let mut t = i.tables["zone_table"].clone();
        if !cols {
            let keep: Vec<usize> = t
                .columns
                .iter()
                .enumerate()
                .filter(|(_, c)| *c == "id" || c.starts_with("cost_"))
                .map(|(k, _)| k)
                .collect();
            t.rows = t.rows.iter().map(|r| keep.iter().map(|&k| r[k]).collect()).collect();
            t.columns = keep.iter().map(|&k| t.columns[k].clone()).collect();
        }
        t
    };
    let mut spec = Table::new(vec!["id".into()]);
    spec.rows = (1..=5).map(|k| vec![k as f64]).collect();
    let rij_targets: Vec<serde_json::Value> = [0.2, 0.3, 0.25, 0.1, 0.3]
        .iter()
        .enumerate()
        .map(|(k, t)| json!({"feature": k + 1, "zone": ZONES, "type": "relative", "target": t}))
        .collect();
    vec![
        ("zones_minset_sum", i.raster(3), with(json!({"manual_targets": sum(0.3)}))),
        ("zones_minset_2z", i.raster(2), with(json!({"manual_targets": i.sum_targets(0.3, &ZONES[..2])}))),
        ("zones_minset_matrix", i.raster(3), with(json!({"relative_targets": matrix(ZONE_MATRIX)}))),
        (
            "zones_minset_mandatory",
            i.raster(3),
            with(json!({"manual_targets": sum(0.3), "mandatory_allocation": true})),
        ),
        ("zones_manual_targets", i.raster(3), with(json!({"manual_targets": manual_targets()}))),
        (
            "zones_locked_raster",
            i.with(i.raster(3), &[("zone_locked_in", "zone_locked_in"), ("zone_locked_out", "zone_locked_out")]),
            with(json!({"manual_targets": sum(0.3)})),
        ),
        (
            "zones_manual_bounded",
            i.with(i.raster(3), &[("bounds", "zone_bounds_cells")]),
            with(json!({"manual_targets": sum(0.3), "decisions": "proportion"})),
        ),
        (
            "zones_manual_locked",
            i.with(i.raster(3), &[("zone_locked_in", "zone_locked_in"), ("status", "zone_status_cells")]),
            with(json!({"manual_targets": sum(0.3)})),
        ),
        ("zones_proportion", i.raster(3), with(json!({"manual_targets": sum(0.3), "decisions": "proportion"}))),
        (
            "zones_semicontinuous",
            i.raster(3),
            with(json!({"manual_targets": sum(0.3), "decisions": "semicontinuous", "upper_limit": 0.6})),
        ),
        (
            "zones_table",
            [("pu".to_string(), Value::Table(zone_table(true)))].into(),
            with(json!({"features": table_features, "cost_column": cost_columns, "zones": ZONES,
                        "relative_targets": matrix(ZONE_MATRIX),
                        "locked_in_column": ZONES.map(|z| format!("locked_in_{z}")),
                        "locked_out_column": ZONES.map(|z| format!("locked_out_{z}"))})),
        ),
        (
            "zones_rij",
            [
                ("pu".to_string(), Value::Table(zone_table(false))),
                ("spec".to_string(), Value::Table(spec)),
                ("rij".to_string(), Value::Table(i.tables["zone_rij"].clone())),
            ]
            .into(),
            json!({"cost_column": cost_columns,
                   "zones": [{"id": 10, "name": "protect"}, {"id": 20, "name": "use"}, {"id": 30, "name": "restore"}],
                   "manual_targets": rij_targets}),
        ),
        (
            "zones_min_shortfall",
            i.raster(3),
            with(json!({"objective": "min_shortfall", "budget": 2500, "manual_targets": sum(0.4)})),
        ),
        (
            "zones_min_shortfall_budgets",
            i.raster(3),
            with(
                json!({"objective": "min_shortfall", "budget": [1200, 800, 600], "relative_targets": matrix(ZONE_MATRIX)}),
            ),
        ),
        (
            "zones_min_shortfall_weights",
            i.raster(3),
            with(json!({"objective": "min_shortfall", "budget": 2500, "manual_targets": sum(0.4),
                        "feature_weights": [[1], [2], [0.5], [3], [1]]})),
        ),
        (
            "zones_min_largest_shortfall",
            i.with(i.raster(3), &[("zone_locked_in", "zone_locked_in")]),
            with(json!({"objective": "min_largest_shortfall", "budget": 2500, "manual_targets": sum(0.4)})),
        ),
        ("zones_max_utility", i.raster(3), with(json!({"objective": "max_utility", "budget": 2500}))),
        (
            "zones_max_utility_weights",
            i.raster(3),
            with(
                json!({"objective": "max_utility", "budget": [1200, 800, 600], "feature_weights": matrix(ZONE_WEIGHTS),
                        "decisions": "proportion"}),
            ),
        ),
        ("zones_max_cover", i.raster(3), with(json!({"objective": "max_cover", "budget": 1500}))),
        (
            "zones_max_features",
            i.raster(3),
            with(
                json!({"objective": "max_features", "budget": [900, 600, 500], "relative_targets": matrix(ZONE_MATRIX)}),
            ),
        ),
        (
            "zones_max_features_weights",
            i.raster(3),
            with(json!({"objective": "max_features", "budget": 2000, "manual_targets": sum(0.3),
                        "feature_weights": [[1], [2], [0.5], [3], [1]]})),
        ),
        (
            "zones_max_phylo_div",
            i.raster(3),
            with(
                json!({"objective": "max_phylo_div", "budget": 2000, "tree": TREE_SHUFFLED, "manual_targets": sum(0.3)}),
            ),
        ),
        (
            "zones_max_phylo_end",
            i.raster(3),
            with(json!({"objective": "max_phylo_end", "budget": [900, 600, 500], "tree": TREE_SHUFFLED,
                        "relative_targets": matrix(ZONE_MATRIX)})),
        ),
    ]
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

/// prioritizr's compile() with zones, bit for bit: units of rasters and tables (every zone's features, Marxan's rij
/// with a zone column), targets summed over zones, per zone and by hand with lists of zones, mandatory allocation,
/// locked units and manual bounds per zone, the decisions, every objective with one budget or one per zone, weights
/// per target or per feature and zone.
#[test]
fn zone_compile_is_prioritizrs() {
    let i = Inputs::new();
    let r = reference();
    let cases = cases(&i);
    assert_eq!(cases.len(), 23);
    for (name, ins, params) in cases {
        let args = Args { node: "n", inputs: ins, params: params.as_object().unwrap() };
        let p = terraflux_ops::prioritize::problem_of(&args).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (m, _) = terraflux_ops::milp::compile(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_model(name, &m, &r);
    }
}

/// The cases whose optimum takes a few seconds at most.
const QUICK: [&str; 10] = [
    "zones_minset_mandatory",
    "zones_manual_targets",
    "zones_manual_bounded",
    "zones_manual_locked",
    "zones_proportion",
    "zones_semicontinuous",
    "zones_min_largest_shortfall",
    "zones_max_utility",
    "zones_max_utility_weights",
    "zones_max_phylo_end",
];

/// The zone problems' optima are prioritizr's (Gurobi's at gap 0), on landschaft's zones engines; the solution's
/// maps (the zone of every unit, a layer per zone).
#[test]
fn zone_optima_are_prioritizrs() {
    optima(|name| QUICK.contains(&name));
}

/// Every zone case's optimum (about 9 minutes: cargo test -- --ignored).
#[test]
#[ignore]
fn every_zone_optimum_is_prioritizrs() {
    optima(|_| true);
}

fn optima(which: impl Fn(&str) -> bool) {
    let i = Inputs::new();
    let r = reference();
    let registry = terraflux_ops::Registry::standard();
    for (name, ins, params) in cases(&i).into_iter().filter(|c| which(c.0)) {
        let mut params = params.as_object().unwrap().clone();
        params.insert("gap".into(), 0.0.into());
        let maximize = params.get("objective").and_then(|o| o.as_str()).is_some_and(|o| o.starts_with("max"));
        let raster = ins.contains_key("zone_costs");
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
        if raster {
            let (Value::Map(zone), Value::Stack(alloc)) = (&out["solution"], &out["allocation"]) else { panic!() };
            assert_eq!(alloc.layers.len(), if name == "zones_minset_2z" { 2 } else { 3 });
            for (c, &z) in zone.data.iter().enumerate() {
                if z >= 1.0 {
                    assert!(alloc.layers[z as usize - 1][c] > 0.0, "{name}: the zone of cell {c}");
                }
            }
        }
    }
}

/// R's matrix(c(...), ncol = 3) of zones as rows.
fn zones3(v: [f64; 9]) -> serde_json::Value {
    json!((0..3).map(|r| (0..3).map(|c| v[c * 3 + r]).collect::<Vec<_>>()).collect::<Vec<_>>())
}

const ZONES_B: [f64; 9] = [1.0, 0.4, -0.2, 0.4, 0.8, 0.0, -0.2, 0.0, 1.0];
const ZONES_C: [f64; 9] = [1.0, 0.5, 0.0, 0.5, 1.0, 0.3, 0.0, 0.3, 0.2];
const ZONES_A: [f64; 9] = [1.0, 0.2, 0.0, 0.6, 1.0, 0.5, 0.0, 0.1, 0.8];
const ZONES_N: [f64; 9] = [1.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0];

/// The cases with spatial penalties and constraints (but contiguity): name, inputs, settings.
fn spatial_cases(i: &Inputs) -> Vec<(&'static str, BTreeMap<String, Value>, serde_json::Value)> {
    let sum = i.sum_targets(0.3, &ZONES);
    let with = |extra: serde_json::Value| {
        let mut p = json!({"feature_names": FEATURES, "manual_targets": sum});
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        p
    };
    let utility = |extra: serde_json::Value| {
        let mut p = json!({"feature_names": FEATURES, "objective": "max_utility", "budget": 2500});
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        p
    };
    vec![
        ("zones_boundary", i.raster(3), with(json!({"boundary_penalty": 0.5}))),
        (
            "zones_boundary_matrix",
            i.raster(3),
            with(json!({"boundary_penalty": 0.3, "edge_factor": [0.5, 1, 0.2], "boundary_zones": zones3(ZONES_B)})),
        ),
        ("zones_boundary_max", i.raster(3), utility(json!({"boundary_penalty": 0.01}))),
        (
            "zones_connectivity",
            i.with(i.raster(3), &[("connectivity_data", "cell_conn")]),
            with(json!({"connectivity_penalty": 5, "connectivity_zones": zones3(ZONES_C)})),
        ),
        (
            "zones_asym_connectivity",
            i.with(i.raster(3), &[("asym_connectivity_data", "cell_asym")]),
            with(json!({"asym_connectivity_penalty": 3, "asym_connectivity_zones": zones3(ZONES_A)})),
        ),
        (
            "zones_linear_penalties",
            i.with(i.raster(3), &[("zone_penalty_data", "zone_linear")]),
            with(json!({"linear_penalty": [20, 10, 5]})),
        ),
        (
            "zones_linear_penalties_zero",
            i.with(i.raster(3), &[("zone_penalty_data", "zone_linear")]),
            with(json!({"linear_penalty": [20, 0, 5]})),
        ),
        (
            "zones_linear_constraints",
            i.with(i.raster(3), &[("zone_constraint_data", "zone_linear")]),
            with(json!({"linear_threshold": 50, "linear_sense": "<="})),
        ),
        ("zones_neighbors", i.raster(3), with(json!({"neighbors": [2, 1, 2]}))),
        (
            "zones_neighbors_matrix",
            i.raster(3),
            utility(json!({"neighbors": [1, 2, 1], "neighbors_zones": zones3(ZONES_N)})),
        ),
    ]
}

/// prioritizr's compile() with zones and spatial terms, bit for bit: boundary penalties (the identity zones matrix,
/// one between zones with a negative cell and edge factors per zone, a maximisation), connectivity and asymmetric
/// connectivity penalties with zones matrices, linear penalties and constraints with data per zone (a zone with
/// penalty 0), neighbour constraints with k per zone and a zones matrix.
#[test]
fn zone_spatial_compile_is_prioritizrs() {
    let i = Inputs::new();
    let r = reference();
    let cases = spatial_cases(&i);
    assert_eq!(cases.len(), 10);
    for (name, ins, params) in cases {
        let args = Args { node: "n", inputs: ins, params: params.as_object().unwrap() };
        let p = terraflux_ops::prioritize::problem_of(&args).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (m, _) = terraflux_ops::milp::compile(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_model(name, &m, &r);
    }
}

/// The spatial zone problems' optima are prioritizr's (Gurobi's at gap 0; cargo test -- --ignored). The boundary
/// penalties with zones take minutes on HiGHS's MIP: 15 seconds there (as landschaft's test), the bound no worse
/// than prioritizr's optimum and the solution no better.
#[test]
#[ignore]
fn every_zone_spatial_optimum_is_prioritizrs() {
    use terraflux_ops::milp;
    let i = Inputs::new();
    let r = reference();
    for (name, ins, params) in spatial_cases(&i) {
        let args = Args { node: "n", inputs: ins, params: params.as_object().unwrap() };
        let p = terraflux_ops::prioritize::problem_of(&args).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (m, _) = milp::compile(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        let slow = name == "zones_boundary" || name == "zones_boundary_matrix";
        let s = milp::solve(&m, p.units.n(), 0.0, if slow { 15.0 } else { f64::INFINITY }).unwrap();
        let want = r.floats(&format!("{name}/objective"))[0];
        if slow {
            assert!(s.status == "optimal" || s.status == "time_limit", "{name}: {}", s.status);
            assert!(s.bound <= want * (1.0 + 1e-9) + 1e-9, "{name}: bound {} > {want}", s.bound);
            assert!(s.objective >= want * (1.0 - 1e-9) - 1e-9, "{name}: {} < {want}", s.objective);
            continue;
        }
        assert_eq!(s.status, "optimal", "{name}");
        let ours = s.objective;
        let close = (ours - want).abs() <= 1e-9 * want.abs().max(1.0);
        let better = if m.maximize { ours > want } else { ours < want };
        assert!(close || (better && (ours - want).abs() <= 1e-5 * want.abs().max(1.0)), "{name}: {ours} != {want}");
    }
}

/// Contiguity with zones (protect and use one cluster, restore another; the zones matrix's components).
fn zone_contiguity_model() -> (terraflux_ops::milp::Model, usize) {
    let i = Inputs::new();
    let params = json!({"feature_names": FEATURES, "manual_targets": i.sum_targets(0.15, &ZONES), "contiguity": true,
                        "contiguity_zones": zones3(ZONES_N)});
    let args = Args { node: "n", inputs: i.raster(3), params: params.as_object().unwrap() };
    let p = terraflux_ops::prioritize::problem_of(&args).unwrap();
    let (m, _) = terraflux_ops::milp::compile(&p).unwrap();
    (m, p.units.n())
}

/// Contiguity with zones: a patch per cluster of zones, the cluster of two zones on columns holding the sum of the
/// unit's columns in them.
#[test]
fn zone_contiguity_has_a_patch_per_cluster() {
    let (m, n) = zone_contiguity_model();
    assert_eq!(m.connected.len(), 2, "two clusters");
    let units = n / 3;
    // restore alone: its own unit columns; protect and use: the cluster columns after the units'
    assert_eq!(m.connected[1].nodes, (2 * units..3 * units).collect::<Vec<_>>());
    assert!(m.connected[0].nodes.iter().all(|&j| m.col_ids[j] == "cluster"));
    assert_eq!(m.row_ids.iter().filter(|r| *r == "cluster").count(), units);
}

/// Contiguity with zones on HiGHS (a time limit, as landschaft's test): prioritizr's formulation is a restriction,
/// ours exact, so the bound is no worse than prioritizr's optimum (cargo test -- --ignored).
#[test]
#[ignore]
fn zone_contiguity_bound_is_below_prioritizrs() {
    let (m, n) = zone_contiguity_model();
    let want = reference().floats("zones_contiguity/objective")[0];
    let s = terraflux_ops::milp::solve(&m, n, 0.0, 120.0).unwrap();
    assert!(s.status == "optimal" || s.status == "time_limit", "{}", s.status);
    assert!(s.bound <= want * (1.0 + 1e-9) + 1e-9, "{} > {want}", s.bound);
}

const ZONES_E: [f64; 9] = [1.0, 0.5, 0.2, 0.5, 1.0, 0.0, 0.2, 0.0, 0.8];

/// prioritizr's evaluation with zones, bit for bit: the units and cost overall and per zone, the boundary (edge
/// factors per zone, a zones matrix and a boundary matrix), the connectivity and asymmetric connectivity with zones
/// matrices, every feature's representation overall and per zone, the targets' coverage (per zone, and by hand with
/// lists of zones); of a binary and a proportional allocation.
#[test]
fn zone_evaluation_is_prioritizrs() {
    let i = Inputs::new();
    let r = reference();
    let registry = terraflux_ops::Registry::standard();
    let bits = |what: &str, a: &[f64], b: &[f64]| {
        assert_eq!(a.len(), b.len(), "{what}");
        for (k, (x, y)) in a.iter().zip(b).enumerate() {
            assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{what}[{k}]: {x} != {y}");
        }
    };
    let run = |sol: &str, extra: &[(&str, &str)], params: serde_json::Value| {
        let mut ins = i.with(i.raster(3), extra);
        ins.insert("zone_solution".into(), i.with(BTreeMap::new(), &[("s", sol)]).remove("s").unwrap());
        let mut p = json!({"feature_names": FEATURES, "relative_targets": matrix([0.2; 15])});
        p.as_object_mut().unwrap().extend(params.as_object().unwrap().clone());
        let args = Args { node: "n", inputs: ins, params: p.as_object().unwrap() };
        registry.get("prioritize.evaluate").unwrap().run(&mut terraflux_ops::Ctx::default(), &args).unwrap()
    };
    let col = |out: &terraflux_ops::Outputs, port: &str, c: &str| {
        let Value::Table(t) = &out[port] else { panic!("{port}") };
        t.column(c).unwrap_or_else(|| panic!("{port}.{c}"))
    };
    let coverage = |name: &str, out: &terraflux_ops::Outputs| {
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
            bits(&format!("{name} {c}"), &col(out, "coverage", c), &r.floats(&format!("{name}/coverage/{c}")));
        }
        let senses: Vec<f64> = r
            .texts(&format!("{name}/coverage/sense"))
            .iter()
            .map(|s| match s.as_str() {
                ">=" => 1.0,
                "<=" => -1.0,
                _ => 0.0,
            })
            .collect();
        assert_eq!(col(out, "coverage", "sense"), senses, "{name}: senses");
    };
    let representation = |name: &str, out: &terraflux_ops::Outputs| {
        for c in ["total_amount", "absolute_held", "relative_held"] {
            bits(
                &format!("{name} {c}"),
                &col(out, "representation", c),
                &r.floats(&format!("{name}/representation/{c}")),
            );
        }
    };

    let k = "zones_eval_summaries";
    let out = run("zone_sol", &[], json!({}));
    bits("n", &col(&out, "summary", "n"), &r.floats(&format!("{k}/n/n")));
    bits("cost", &col(&out, "summary", "cost"), &r.floats(&format!("{k}/cost/cost")));
    bits("boundary", &col(&out, "summary", "boundary"), &r.floats(&format!("{k}/boundary/boundary")));
    representation(k, &out);
    coverage(k, &out);

    let k = "zones_eval_proportion";
    let out = run("zone_solp", &[], json!({"decisions": "proportion", "edge_factor": [0.5, 1, 0.2]}));
    bits("n", &col(&out, "summary", "n"), &r.floats(&format!("{k}/n/n")));
    bits("cost", &col(&out, "summary", "cost"), &r.floats(&format!("{k}/cost/cost")));
    bits("boundary", &col(&out, "summary", "boundary"), &r.floats(&format!("{k}/boundary/boundary")));
    representation(k, &out);

    let k = "zones_eval_spatial";
    let out = run(
        "zone_sol",
        &[("boundary_data", "zone_bnd"), ("connectivity_data", "cell_conn"), ("asym_connectivity_data", "cell_asym")],
        json!({"edge_factor": [0.5, 1, 0.2], "boundary_zones": zones3(ZONES_B), "connectivity_zones": zones3(ZONES_E),
               "asym_connectivity_zones": zones3(ZONES_A)}),
    );
    bits("boundary", &col(&out, "summary", "boundary"), &r.floats(&format!("{k}/boundary/boundary")));
    bits("connectivity", &col(&out, "summary", "connectivity"), &r.floats(&format!("{k}/connectivity/connectivity")));
    bits("asym", &col(&out, "summary", "asym_connectivity"), &r.floats(&format!("{k}/asym/asym_connectivity")));

    let k = "zones_eval_coverage_lists";
    let mut p = json!({"manual_targets": manual_targets()});
    p.as_object_mut().unwrap().insert("relative_targets".into(), serde_json::Value::Null);
    let out = run("zone_sol", &[], p);
    coverage(k, &out);
}
