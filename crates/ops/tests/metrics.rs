//! Landscape metrics against the R package landscapemetrics: every value of calculate_lsm that landschaft keeps
//! (extern/landschaft/tests/data/reference/lsm, made by its tools/make_reference_lsm.py) for its test rasters
//! and option sets, and window_lsm's maps (reference/lsm_window).

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_ops::lsm_catalog::METRICS;
use terraflux_ops::metrics::{LEVELS, Row, default_options, measure};
use terraflux_ops::{Args, Ctx, Map, Registry, Value};

/// Tolerances of landschaft's own comparison (tests/test_lsm_reference.py).
const RTOL: f64 = 1e-10;
const ATOL: f64 = 1e-14;

/// calculate_lsm builds te, ed, iji and contag from the neighbourhood of the complexity metrics; landscapemetrics'
/// own functions (and landschaft) use 4 neighbours: with 8 they are compared with the 4-neighbour reference.
const NEIGHBOURHOOD_QUIRK: [&str; 4] = ["te", "ed", "iji", "contag"];

fn landschaft() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extern/landschaft/tests/data")
}

fn read_map(path: &Path) -> Map {
    let r = terraflux_raster::Raster::read(path).unwrap();
    Map::new(Arc::new(r.grid.clone()), r.bands[0].masked(r.nodata))
}

type Key = (String, Option<i32>, Option<i32>, String);

/// The rows of a reference (gzipped CSV: level, class, id, metric, value; NA where none).
fn read_reference(path: &Path) -> BTreeMap<Key, f64> {
    let mut text = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_string(&mut text).unwrap();
    let int = |s: &str| if s.is_empty() || s == "NA" { None } else { Some(s.parse::<i32>().unwrap()) };
    text.lines()
        .skip(1)
        .filter(|l| !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            let value = if f[4] == "NA" || f[4].is_empty() { f64::NAN } else { f[4].parse().unwrap() };
            ((f[0].to_string(), int(f[1]), int(f[2]), f[3].to_string()), value)
        })
        .collect()
}

/// Only the metrics landschaft has (those of the catalogue that are landscapemetrics').
fn implemented(rows: BTreeMap<Key, f64>) -> BTreeMap<Key, f64> {
    rows.into_iter()
        .filter(|((l, _, _, m), _)| METRICS.iter().any(|x| !x.extra && x.level == *l && x.metric == *m))
        .collect()
}

fn options(o: &serde_json::Value) -> terraflux_ls_sys::ffi::LsmOptions {
    let mut opt = default_options();
    let o = o.as_object().unwrap();
    for (k, v) in o {
        match k.as_str() {
            "directions" => opt.directions = v.as_i64().unwrap() as i32,
            "count_boundary" => opt.count_boundary = v.as_bool().unwrap(),
            "consider_boundary" => opt.consider_boundary = v.as_bool().unwrap(),
            "edge_depth" => opt.edge_depth = v.as_i64().unwrap() as i32,
            "cell_center" => opt.cell_center = v.as_bool().unwrap(),
            "classes_max" => opt.classes_max = v.as_f64().unwrap(),
            "neighbourhood" => opt.neighbourhood = v.as_i64().unwrap() as i32,
            "ordered" => opt.ordered = v.as_bool().unwrap(),
            "base" => opt.base = v.as_str().unwrap().into(),
            k => panic!("option {k}"),
        }
    }
    opt
}

fn close(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b || (a - b).abs() <= ATOL + RTOL * b.abs()
}

/// Every landscapemetrics metric landschaft has, at every level, on the 14 rasters and 5 option sets kept: the
/// same rows (levels, classes, patches) and values.
#[test]
fn metrics_are_landscapemetrics() {
    let dir = landschaft().join("reference/lsm");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    let specs: Vec<(&'static str, &str)> = METRICS
        .iter()
        .filter(|m| !m.extra)
        .map(|m| (*LEVELS.iter().find(|l| **l == m.level).unwrap(), m.metric))
        .collect();
    let mut files = 0;
    let mut values = 0;
    for (fname, info) in manifest["files"].as_object().unwrap() {
        let map = read_map(&landschaft().join("lsm").join(info["input"].as_str().unwrap()));
        let opt = options(&info["options"]);
        let ours: Vec<Row> = measure(&map, &opt, &specs, false).unwrap().rows;
        let mut ours: BTreeMap<Key, f64> =
            ours.into_iter().map(|r| ((r.level.to_string(), r.class, r.id, r.metric), r.value)).collect();
        let mut theirs = implemented(read_reference(&dir.join(fname)));
        // circle is not measured on cells that are not square
        if !theirs.keys().any(|k| k.3.starts_with("circle")) {
            ours.retain(|k, _| !k.3.starts_with("circle"));
        }
        if opt.neighbourhood == 8 {
            let default = implemented(read_reference(&dir.join(fname.replace("__complexity", "__default"))));
            theirs.retain(|k, _| !NEIGHBOURHOOD_QUIRK.contains(&k.3.as_str()));
            theirs.extend(default.into_iter().filter(|(k, _)| NEIGHBOURHOOD_QUIRK.contains(&k.3.as_str())));
        }
        let only_ours: Vec<_> = ours.keys().filter(|k| !theirs.contains_key(*k)).take(5).collect();
        let only_theirs: Vec<_> = theirs.keys().filter(|k| !ours.contains_key(*k)).take(5).collect();
        assert!(
            only_ours.is_empty() && only_theirs.is_empty(),
            "{fname}: ours only {only_ours:?}, R's only {only_theirs:?}"
        );
        let wrong: Vec<_> =
            ours.iter().filter(|(k, v)| !close(**v, theirs[*k])).map(|(k, v)| (k, *v, theirs[k])).take(5).collect();
        assert!(wrong.is_empty(), "{fname}: {wrong:?}");
        files += 1;
        values += ours.len();
    }
    assert_eq!(files, 70);
    assert!(values > 100_000, "{values} values");
}

fn run(op: &str, map: Map, params: serde_json::Value) -> terraflux_ops::Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: [("map".to_string(), Value::Map(map))].into(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
}

/// window_lsm's maps of every landscape-level metric, in square and circular windows.
#[test]
fn windows_are_landscapemetrics() {
    let dir = landschaft().join("reference/lsm_window");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    for (fname, info) in manifest["files"].as_object().unwrap() {
        let want = terraflux_raster::npy::read_npz(dir.join(fname)).unwrap();
        let w = &want["window"];
        let side = w.shape[0];
        // the windows kept are squares and discs
        let full = w.data.iter().all(|&v| v != 0.0);
        let mut params = info["options"].clone();
        params["window"] = side.into();
        params["shape"] = if full { "square" } else { "circle" }.into();
        params["metrics"] = info["metrics"].clone();
        let shape: Vec<f64> = (0..side * side)
            .map(|i| {
                let r = (side / 2) as f64;
                let (y, x) = ((i / side) as f64 - r, (i % side) as f64 - r);
                if full || x.hypot(y) <= r { 1.0 } else { 0.0 }
            })
            .collect();
        assert_eq!(shape, w.data, "{fname}: the window");
        let map = read_map(&dir.join(info["input"].as_str().unwrap()));
        let out = run("metrics.window", map, params);
        let Value::Stack(s) = &out["maps"] else { panic!() };
        for (k, layer) in s.keys.iter().zip(&s.layers) {
            let theirs = &want[&k.to_string()];
            let bad = layer.iter().zip(&theirs.data).filter(|(a, b)| !close(**a, **b)).count();
            assert_eq!(bad, 0, "{fname} {k}: {bad} cells differ");
        }
    }
}

/// The operators give the metrics as tables of a row per patch, class or the map, and maps of them.
#[test]
fn tables_and_maps() {
    // two classes: a block of 2 in 1, and a lone cell of 2
    let mut data = vec![1.0; 36];
    for i in [7, 8, 13, 14] {
        data[i] = 2.0;
    }
    data[35] = 2.0;
    data[0] = f64::NAN;
    let map = Map::new(Arc::new(terraflux_raster::Grid::new(6, 6)), data);
    let out = run("metrics.patch", map.clone(), serde_json::json!({"metrics": ["area", "perim"], "show": "area"}));
    let Value::Table(t) = &out["table"] else { panic!() };
    assert_eq!(t.columns, ["class", "patch", "area", "perim"]);
    assert_eq!(t.rows.len(), 3);
    // landscapemetrics' areas are in hectares of cells of side 1 m
    let areas: Vec<(f64, f64)> = t.rows.iter().map(|r| (r[0], r[2] * 10_000.0)).collect();
    assert_eq!(areas, [(1.0, 30.0), (2.0, 4.0), (2.0, 1.0)]);
    let (Value::Map(p), Value::Map(a)) = (&out["patches"], &out["map"]) else { panic!() };
    assert!(p.data[0].is_nan() && a.data[0].is_nan(), "NA");
    assert_eq!(a.data[7] * 10_000.0, 4.0);
    let out = run("metrics.class", map.clone(), serde_json::json!({"metrics": "np", "show": "lsm_c_pland"}));
    let Value::Table(t) = &out["table"] else { panic!() };
    assert_eq!(t.columns, ["class", "np", "pland"]);
    assert_eq!(t.rows, [vec![1.0, 1.0, 30.0 / 35.0 * 100.0], vec![2.0, 2.0, 5.0 / 35.0 * 100.0]]);
    let out = run("metrics.landscape", map.clone(), serde_json::json!({"type": "diversity"}));
    let Value::Table(t) = &out["table"] else { panic!() };
    assert!(t.columns.contains(&"shdi".to_string()) && t.rows.len() == 1);
    let r = Registry::standard();
    let params = serde_json::json!({"metrics": ["nonsense"]}).as_object().cloned().unwrap();
    let args = Args { node: "n", inputs: [("map".to_string(), Value::Map(map))].into(), params: &params };
    assert!(r.get("metrics.class").unwrap().run(&mut Ctx::default(), &args).is_err());
}
