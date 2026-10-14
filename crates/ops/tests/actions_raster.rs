//! prioritize.actions_raster against landschaft's actions_raster_data: the tables it builds from the rasters of
//! its tests (tests/data/reference/actions_raster.npz; numbers and maps as actions' costs, absolute and relative
//! targets, locked units, a sensitivity, the boundary) and of its example (prioritize/data), the same; the optima
//! of their problems the same (tools/make_actions_raster_reference.py); the solution as maps.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Map, Registry, Stack, Table, Value};
use terraflux_raster::Grid;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference() -> serde_json::Value {
    let p = root().join("tests/data/reference/actions_raster.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// The grid of landschaft's test rasters: 100 m cells, top-left at (0, rows * 100).
fn grid(ny: usize, nx: usize) -> Arc<Grid> {
    let mut g = Grid::new(nx, ny);
    g.transform = [0.0, 100.0, 0.0, ny as f64 * 100.0, 0.0, -100.0];
    Arc::new(g)
}

#[derive(Clone)]
struct Inputs {
    cost: Map,
    features: Stack,
    threats: Stack,
    maps: Stack,
    locked_in: Map,
    locked_out: Map,
}

fn small() -> Inputs {
    let a = terraflux_raster::npy::read_npz(root().join("tests/data/reference/actions_raster.npz")).unwrap();
    let (ny, nx) = (a["cost"].shape[0], a["cost"].shape[1]);
    let g = grid(ny, nx);
    let n = ny * nx;
    let stack = |name: &str, keys: &[&str]| Stack {
        grid: g.clone(),
        keys: keys.iter().map(|k| Key::Name(k.to_string())).collect(),
        layers: (0..keys.len()).map(|k| Arc::new(a[name].data[k * n..(k + 1) * n].to_vec())).collect(),
    };
    let map = |name: &str| Map::new(g.clone(), a[name].data.clone());
    Inputs {
        cost: map("cost"),
        features: stack("features", &["sp1", "sp2", "sp3"]),
        threats: stack("threats", &["fire", "cattle"]),
        maps: stack("action_cost", &["action_cost"]),
        locked_in: map("locked_in"),
        locked_out: map("locked_out"),
    }
}

/// landschaft's example: the cost, the eight features and two threats of its prioritize data.
fn example() -> Inputs {
    let data = root().join("extern/landschaft/src/landschaft/prioritize/data");
    let read = |f: &str| terraflux_raster::Raster::read(data.join(f)).unwrap();
    let as_stack = |r: &terraflux_raster::Raster, g: &Arc<Grid>| Stack {
        grid: g.clone(),
        keys: r.band_names().into_iter().map(Key::Name).collect(),
        layers: r.bands.iter().map(|b| Arc::new(r.masked_band(b))).collect(),
    };
    let cost = read("cost.tif");
    let g = Arc::new(cost.grid.clone());
    let (f, t) = (read("features.tif"), read("threats.tif"));
    let empty = Map::new(g.clone(), vec![0.0; g.cells()]);
    Inputs {
        cost: Map::new(g.clone(), cost.masked_band(&cost.bands[0])),
        features: as_stack(&f, &g),
        threats: as_stack(&t, &g),
        maps: Stack { grid: g.clone(), keys: Vec::new(), layers: Vec::new() },
        locked_in: empty.clone(),
        locked_out: empty,
    }
}

trait Masked {
    fn masked_band(&self, b: &terraflux_raster::Band) -> Vec<f64>;
}

impl Masked for terraflux_raster::Raster {
    fn masked_band(&self, b: &terraflux_raster::Band) -> Vec<f64> {
        b.data.iter().map(|&v| if Some(v) == self.nodata || v.is_nan() { f64::NAN } else { v }).collect()
    }
}

/// landschaft's sensitivity of the locked case, by positions.
fn sensitivity() -> Table {
    let mut t = Table::new(["feature", "threat", "delta1", "delta2"].map(String::from).to_vec());
    t.rows =
        vec![vec![1.0, 1.0, 0.0, 1.0], vec![2.0, 1.0, 0.1, 0.9], vec![3.0, 2.0, 0.0, 0.8], vec![3.0, 2.0, 0.2, 1.0]];
    t
}

fn run(ins: &Inputs, case: &serde_json::Value) -> terraflux_ops::Outputs {
    let mut params = case["settings"].as_object().unwrap().clone();
    params.extend(case["problem"].as_object().unwrap().clone());
    params.insert("gap".into(), case["gap"].clone());
    let mut inputs: BTreeMap<String, Value> = BTreeMap::new();
    inputs.insert("monitoring_cost".into(), Value::Map(ins.cost.clone()));
    inputs.insert("features".into(), Value::Stack(ins.features.clone()));
    inputs.insert("threats".into(), Value::Stack(ins.threats.clone()));
    inputs.insert("maps".into(), Value::Stack(ins.maps.clone()));
    if params.remove("locked_in").is_some() {
        inputs.insert("locked_in".into(), Value::Map(ins.locked_in.clone()));
    }
    if params.remove("locked_out").is_some() {
        inputs.insert("locked_out".into(), Value::Map(ins.locked_out.clone()));
    }
    if params.remove("sensitivity").is_some() {
        inputs.insert("sensitivity".into(), Value::Table(sensitivity()));
    }
    let r = Registry::standard();
    let args = Args { node: "n", inputs, params: &params };
    r.get("prioritize.actions_raster").unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{e}"))
}

/// A table's rows, their first columns (landschaft's tables carry its internal ids after them).
fn rows(t: &serde_json::Value, columns: usize) -> Vec<Vec<f64>> {
    let all: Vec<Vec<f64>> = serde_json::from_value(t["rows"].clone()).unwrap();
    all.into_iter().map(|r| r[..columns].to_vec()).collect()
}

/// The tables the same (prioriactions's inputData() rounds the costs and intensities landschaft's tables show: the
/// costs of a float32 raster to 3 decimals).
fn close(a: &[Vec<f64>], b: &[Vec<f64>], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: rows");
    for (x, y) in a.iter().zip(b) {
        assert!(x.iter().zip(y).all(|(u, v)| (u - v).abs() <= 1e-6 * v.abs().max(1.0)), "{what}: {x:?} != {y:?}");
    }
}

/// The tables built from the rasters (the units, the actions and their costs) and the optimum, as landschaft's;
/// the maps of the solution its monitored units and actions.
#[test]
fn rasters_are_landschafts() {
    let refs = reference();
    let small = small();
    for (name, case) in refs.as_object().unwrap() {
        let ins = if name == "example" { example() } else { small.clone() };
        let out = run(&ins, case);
        let Value::Table(pu) = &out["pu"] else { panic!() };
        close(&pu.rows, &rows(&case["tables"]["pu"], 3), &format!("{name}: pu"));
        let Value::Table(dt) = &out["dist_threats"] else { panic!() };
        close(&dt.rows, &rows(&case["tables"]["dist_threats"], 4), &format!("{name}: dist_threats"));
        let Value::Table(result) = &out["result"] else { panic!() };
        let objective = case["objective"].as_f64().unwrap();
        assert!(
            (result.rows[0][0] - objective).abs() <= 1e-9 * objective.abs().max(1.0),
            "{name}: objective {} != {objective}",
            result.rows[0][0]
        );
        // the maps: the monitored units and the actions of the solution
        let Value::Stack(maps) = &out["maps"] else { panic!() };
        let Value::Table(mon) = &out["monitoring"] else { panic!() };
        let Value::Table(act) = &out["actions"] else { panic!() };
        let sum = |l: &Arc<Vec<f64>>| l.iter().filter(|v| !v.is_nan()).sum::<f64>();
        assert_eq!(sum(&maps.layers[0]), mon.rows.iter().map(|r| r[1]).sum::<f64>(), "{name}: monitoring");
        let acted: f64 = maps.layers[1..].iter().map(sum).sum();
        assert_eq!(acted, act.rows.iter().map(|r| r[2]).sum::<f64>(), "{name}: actions");
        assert_eq!(maps.layers[0].iter().filter(|v| !v.is_nan()).count(), pu.rows.len(), "{name}: units");
    }
}

/// The targets and the boundary built, as landschaft's (through the model they give: the same optimum is checked
/// above; here the features' targets and the boundary rows).
#[test]
fn targets_and_boundary() {
    let refs = reference();
    let ins = small();
    // relative targets: shares of the amounts where threats harm the features
    let r = terraflux_ops::actions_raster::RasterSettings {
        action_cost: Some(&serde_json::json!(1.0)),
        target_recovery: Some(&serde_json::json!(0.5)),
        target_conservation: Some(&serde_json::json!(0.25)),
        relative: true,
        blm_actions: None,
        boundary: true,
    };
    let none = |_: &str| -> Result<Vec<f64>, String> { Err("no maps".into()) };
    let (_, t) = terraflux_ops::actions_raster::raster_data(
        &ins.cost,
        &ins.features,
        &ins.threats,
        &none,
        [None, None],
        None,
        &r,
    )
    .unwrap();
    assert_eq!(t.features.rows, rows(&refs["relative"]["tables"]["features"], 3));
    assert_eq!(t.dist_features.rows, rows(&refs["relative"]["tables"]["dist_features"], 3));
    // the boundary's pairs (prioriactions's inputData() sorts them)
    let sorted = |mut v: Vec<Vec<f64>>| {
        v.sort_by(|x, y| x.partial_cmp(y).unwrap());
        v
    };
    assert_eq!(sorted(t.boundary.unwrap().rows), sorted(rows(&refs["boundary"]["tables"]["boundary"], 3)));
    // errors
    let bad = |s: terraflux_ops::actions_raster::RasterSettings| {
        terraflux_ops::actions_raster::raster_data(
            &ins.cost,
            &ins.features,
            &ins.threats,
            &none,
            [None, None],
            None,
            &s,
        )
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default()
    };
    let s = |cost: serde_json::Value, rec: f64| terraflux_ops::actions_raster::RasterSettings {
        action_cost: Some(Box::leak(Box::new(cost))),
        target_recovery: Some(Box::leak(Box::new(serde_json::json!(rec)))),
        target_conservation: None,
        relative: true,
        blm_actions: None,
        boundary: false,
    };
    assert!(bad(s(serde_json::json!(1.0), 30.0)).contains("between 0 and 1"));
    assert!(bad(s(serde_json::json!({"fire": 1.0}), 0.2)).contains("no value for"));
    assert!(bad(s(serde_json::json!([1.0, "holes"]), 0.2)).contains("no maps"));
}
