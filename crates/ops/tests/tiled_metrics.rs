//! Landscape metrics by blocks of rows (landschaft's out-of-core landscape, `block_rows`): the same tables as the
//! map in memory, to the bit, as landschaft's tests/test_lsm_tiled.py asks of its own; of maps in memory and of
//! rasters on disk.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::files::Rasters;
use terraflux_ops::lsm_catalog::METRICS;
use terraflux_ops::metrics::by_blocks;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
use terraflux_raster::{Band, DataType, Grid, Raster};

fn run(op: &str, input: Value, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: [("map".to_string(), input)].into_iter().collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
}

fn table(out: &Outputs) -> Table {
    let Value::Table(t) = &out["table"] else { panic!("no table") };
    t.clone()
}

fn landschaft(name: &str) -> Map {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extern/landschaft/tests/data/lsm").join(name);
    let r = Raster::read(&path).unwrap();
    Map::new(Arc::new(r.grid.clone()), r.bands[0].masked(r.nodata))
}

/// A map of `ny` x `nx` cells of `res`, from a function of the row and column (NaN: no value).
fn synthetic(ny: usize, nx: usize, res: (f64, f64), f: impl Fn(usize, usize) -> f64) -> Map {
    let mut grid = Grid::new(nx, ny);
    grid.transform = [0.0, res.0, 0.0, ny as f64 * res.1, 0.0, -res.1];
    Map::new(Arc::new(grid), (0..ny * nx).map(|i| f(i / nx, i % nx)).collect())
}

/// A deterministic draw in [0, 1) of a cell.
fn draw(seed: u64, i: usize, j: usize) -> f64 {
    let mut z = seed ^ ((i as u64) << 32 | j as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

/// Stripes and a comb: patches through every block, joined across the seams (landschaft's test).
fn comb() -> Map {
    synthetic(400, 150, (1.0, 1.0), |i, j| {
        if draw(1, i, j) < 0.02 {
            5.0
        } else if (200..260).contains(&i) && (40..90).contains(&j) {
            4.0
        } else if j % 9 == 0 {
            3.0
        } else if i % 5 == 0 {
            2.0
        } else {
            1.0
        }
    })
}

/// Clumps of five classes in squares of 6 x 6 cells, some cells changed and 5 % without a value, cells of 30 x 20.
fn clustered() -> Map {
    synthetic(330, 170, (30.0, 20.0), |i, j| {
        if draw(7, i, j) < 0.05 {
            f64::NAN
        } else if draw(8, i, j) < 0.1 {
            1.0 + (draw(9, i, j) * 5.0).floor()
        } else {
            1.0 + (draw(3, i / 6, j / 6) * 5.0).floor()
        }
    })
}

/// Every metric of a level that can be measured by blocks.
fn all(level: &str, fragstats: bool) -> Vec<&'static str> {
    METRICS.iter().filter(|m| m.level == level && by_blocks(m.metric, fragstats)).map(|m| m.metric).collect()
}

fn same(a: &Table, b: &Table, what: &str) {
    assert_eq!(a.columns, b.columns, "{what}");
    assert_eq!(a.rows.len(), b.rows.len(), "{what}: rows");
    for (i, (x, y)) in a.rows.iter().zip(&b.rows).enumerate() {
        for (j, (p, q)) in x.iter().zip(y).enumerate() {
            assert!(
                p.to_bits() == q.to_bits() || (p.is_nan() && q.is_nan()),
                "{what}: row {i} {}: {p} {q}",
                a.columns[j]
            );
        }
    }
}

/// The tables of every level of a map, in memory and by blocks of `rows` rows.
fn same_as_memory(map: &Map, rows: u64, options: serde_json::Value, what: &str) {
    let fragstats = options.get("convention").is_some_and(|c| c == "fragstats");
    for level in ["patch", "class", "landscape"] {
        let mut p = options.clone();
        p["metrics"] = serde_json::json!(all(level, fragstats));
        let memory = table(&run(&format!("metrics.{level}"), Value::Map(map.clone()), p.clone()));
        p["block_rows"] = rows.into();
        let blocks = run(&format!("metrics.{level}"), Value::Map(map.clone()), p);
        same(&table(&blocks), &memory, &format!("{what} {level}"));
        if level == "patch" {
            let (Value::Map(a), Value::Map(b)) =
                (&blocks["patches"], &run("metrics.patch", Value::Map(map.clone()), options.clone())["patches"])
            else {
                panic!()
            };
            assert!(a.data.iter().zip(b.data.iter()).all(|(x, y)| x.to_bits() == y.to_bits()), "{what}: patches");
        }
    }
}

#[test]
fn landschaft_maps_by_blocks() {
    for name in ["augusta.tif", "podlasie.tif", "sparse.tif", "sparse_na.tif", "synthetic_na.tif"] {
        same_as_memory(&landschaft(name), 64, serde_json::json!({}), name);
    }
}

#[test]
fn clustered_map_by_blocks() {
    let map = clustered();
    let cases = [
        (64, serde_json::json!({})),
        (128, serde_json::json!({"directions": 4})),
        (192, serde_json::json!({"edge_depth": 3, "consider_boundary": true, "count_boundary": true})),
        // context deeper than a block
        (64, serde_json::json!({"edge_depth": 70, "directions": 4})),
        (64, serde_json::json!({"cell_center": true, "neighbourhood": 8, "ordered": false, "base": "log10"})),
        (1, serde_json::json!({"classes_max": 9})),
        (64, serde_json::json!({"convention": "fragstats"})),
    ];
    for (rows, options) in cases {
        same_as_memory(&map, rows, options.clone(), &format!("clustered {rows} {options}"));
    }
}

#[test]
fn patches_across_many_blocks() {
    for directions in [4, 8] {
        same_as_memory(&comb(), 64, serde_json::json!({"directions": directions, "edge_depth": 2}), "comb");
    }
}

/// A raster on disk, read by blocks: the same tables as the map in memory; and the metrics that need the whole
/// map refused.
#[test]
fn rasters_on_disk_by_blocks() {
    let map = clustered();
    let dir = std::env::temp_dir().join(format!("terraflux-tiled-metrics-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("clustered.tif");
    let data: Vec<f64> = map.data.iter().map(|v| if v.is_nan() { -9.0 } else { *v }).collect();
    Raster {
        grid: (*map.grid).clone(),
        bands: vec![Band { name: None, data }],
        dtype: DataType::I16,
        nodata: Some(-9.0),
    }
    .write(&path)
    .unwrap();
    let disk = Value::Raster(Arc::new(Rasters::open(path.to_str().unwrap(), None, None).unwrap()));
    for level in ["patch", "class", "landscape"] {
        let p = serde_json::json!({"metrics": all(level, false), "edge_depth": 2});
        let memory = table(&run(&format!("metrics.{level}"), Value::Map(map.clone()), p.clone()));
        let mut q = p.clone();
        q["block_rows"] = 64.into();
        let out = run(&format!("metrics.{level}"), disk.clone(), q);
        same(&table(&out), &memory, &format!("disk {level}"));
        // the maps of patches only of a map in memory
        assert!(!out.contains_key("patches"));
    }
    // the patches written block by block, the same as in memory
    let patches = dir.join("patches.tif");
    let q = serde_json::json!({"metrics": ["area"], "block_rows": 64, "patches_path": patches.to_string_lossy()});
    let out = run("metrics.patch", disk.clone(), q);
    assert!(matches!(out["patches"], Value::Raster(_)));
    let back = Raster::read(&patches).unwrap();
    let back = back.bands[0].masked(back.nodata);
    let Value::Map(memory) =
        &run("metrics.patch", Value::Map(map.clone()), serde_json::json!({"metrics": ["area"]}))["patches"]
    else {
        panic!()
    };
    assert_eq!(back.len(), memory.data.len());
    assert!(back.iter().zip(memory.data.iter()).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())), "patches on disk");
    // by default, the metrics by blocks only
    let out = run("metrics.class", disk.clone(), serde_json::json!({"extras": true, "block_rows": 64}));
    assert!(!table(&out).columns.iter().any(|c| c.starts_with("prox") || c.starts_with("econ")));
    let r = Registry::standard();
    let params = serde_json::json!({"metrics": ["prox_mn"], "block_rows": 64}).as_object().cloned().unwrap();
    let args = Args { node: "n", inputs: [("map".to_string(), disk)].into_iter().collect(), params: &params };
    assert!(r.get("metrics.class").unwrap().run(&mut Ctx::default(), &args).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

/// Maps written to a GeoTIFF of a band each (no value: -9), as rasters on disk.
fn on_disk(maps: &[&Map], path: &Path) -> Value {
    let bands = maps
        .iter()
        .map(|m| Band { name: None, data: m.data.iter().map(|v| if v.is_nan() { -9.0 } else { *v }).collect() })
        .collect();
    Raster { grid: (*maps[0].grid).clone(), bands, dtype: DataType::I16, nodata: Some(-9.0) }.write(path).unwrap();
    Value::Raster(Arc::new(Rasters::open(path.to_str().unwrap(), None, None).unwrap()))
}

fn temp_dir(what: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("terraflux-tiled-{what}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_with(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
}

fn same_bits(a: &[f64], b: &[f64], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}");
    for (i, (p, q)) in a.iter().zip(b).enumerate() {
        assert!(p.to_bits() == q.to_bits() || (p.is_nan() && q.is_nan()), "{what}: cell {i}: {p} {q}");
    }
}

/// The moving window by strips of rows (with the window's half side above and below): the same maps as the map
/// whole, in memory and from rasters on disk (written to a GeoTIFF, a band per metric).
#[test]
fn windows_by_blocks() {
    let map = clustered();
    let dir = temp_dir("window");
    let disk = on_disk(&[&map], &dir.join("clustered.tif"));
    let metrics = ["shdi", "ed", "contag", "np", "ta", "lpi", "pafrac", "ai"];
    let cases = [
        serde_json::json!({"window": 5}),
        serde_json::json!({"window": 11, "shape": "circle", "directions": 4}),
        // a window deeper than the strips asked for
        serde_json::json!({"window": 81, "block_rows": 64}),
    ];
    for case in cases {
        let mut p = case.clone();
        p["metrics"] = serde_json::json!(metrics);
        if let Some(o) = p.as_object_mut() {
            o.remove("block_rows");
        }
        let Value::Stack(whole) = &run("metrics.window", Value::Map(map.clone()), p.clone())["maps"] else { panic!() };
        p["block_rows"] = case.get("block_rows").cloned().unwrap_or(64.into());
        let Value::Stack(blocks) = &run("metrics.window", Value::Map(map.clone()), p.clone())["maps"] else { panic!() };
        for (k, m) in metrics.iter().enumerate() {
            same_bits(&blocks.layers[k], &whole.layers[k], &format!("{case} {m}"));
        }
        let path = dir.join("window.tif");
        p["path"] = path.to_string_lossy().into();
        let Value::Raster(written) = &run("metrics.window", disk.clone(), p)["maps"] else { panic!() };
        assert_eq!(written.layers.len(), metrics.len());
        let back = Raster::read(&path).unwrap();
        for (k, m) in metrics.iter().enumerate() {
            assert_eq!(back.bands[k].name.as_deref(), Some(*m));
            same_bits(&back.bands[k].data, &whole.layers[k], &format!("{case} {m} on disk"));
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Points spread over the clustered map (its cells of 30 x 20), a few near its edges and one outside.
fn points() -> terraflux_raster::vector::Vector {
    use terraflux_raster::vector::{Column, Geometry, Vector};
    let xy = [[700.0, 900.0], [2600.0, 3300.0], [5000.0, 6500.0], [60.0, 3000.0], [4100.0, 1500.0], [9000.0, 9000.0]];
    Vector {
        geometries: xy.iter().map(|p| Geometry::Point(*p)).collect(),
        attributes: vec![("id".into(), Column::Numbers((1..=xy.len()).map(|i| i as f64).collect()))],
        crs: None,
    }
}

/// Samples of rasters on disk, each plot's window read alone: the same tables as the map in memory.
#[test]
fn samples_from_disk() {
    let map = clustered();
    let dir = temp_dir("sample");
    let disk = on_disk(&[&map], &dir.join("clustered.tif"));
    let plots = Value::Vector(Arc::new(points()));
    for (level, extra) in [
        ("landscape", serde_json::json!({})),
        ("class", serde_json::json!({"all_classes": true, "shape": "circle"})),
        ("patch", serde_json::json!({"size": [150, 400]})),
    ] {
        let mut p = extra.clone();
        p["level"] = level.into();
        if p.get("size").is_none() {
            p["size"] = 300.into();
        }
        let memory =
            run_with("metrics.sample", vec![("map", Value::Map(map.clone())), ("plots", plots.clone())], p.clone());
        p["block_rows"] = 64.into();
        let blocks = run_with("metrics.sample", vec![("map", disk.clone()), ("plots", plots.clone())], p);
        same(&table(&blocks), &table(&memory), &format!("sample {level}"));
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A series of two dates, and its signatures, from rasters on disk by blocks: the same tables as the stack in
/// memory, of whole maps and of zones.
#[test]
fn series_and_signatures_from_disk() {
    let first = clustered();
    let second = Map::new(
        first.grid.clone(),
        first.data.iter().enumerate().map(|(i, v)| if i % 7 == 0 && !v.is_nan() { 2.0 } else { *v }).collect(),
    );
    let dir = temp_dir("series");
    let disk = on_disk(&[&first, &second], &dir.join("dates.tif"));
    let stack = terraflux_ops::Stack {
        grid: first.grid.clone(),
        keys: vec![terraflux_ops::Key::Name("2000".into()), terraflux_ops::Key::Name("2010".into())],
        layers: vec![first.data.clone(), second.data.clone()],
    };
    for (level, extra) in [
        ("landscape", serde_json::json!({"metrics": ["shdi", "ed", "np", "lpi", "area_mn", "enn_mn"]})),
        ("class", serde_json::json!({"metrics": ["pland", "np", "core_mn"], "grid": 1500})),
    ] {
        let mut p = extra.clone();
        p["level"] = level.into();
        p["dates"] = serde_json::json!([2000, 2010]);
        let memory = run_with("metrics.series", vec![("maps", Value::Stack(stack.clone()))], p.clone());
        p["block_rows"] = 64.into();
        let blocks = run_with("metrics.series", vec![("maps", disk.clone())], p.clone());
        same(&table(&blocks), &table(&memory), &format!("series {level} on disk"));
        let blocks = run_with("metrics.series", vec![("maps", Value::Stack(stack.clone()))], p);
        same(&table(&blocks), &table(&memory), &format!("series {level} by blocks"));
    }
    let p = serde_json::json!({"landscape_metrics": ["shdi", "contag"], "class_metrics": ["pland", "ed"]});
    let memory = run_with("metrics.signatures", vec![("maps", Value::Stack(stack.clone()))], p.clone());
    let mut q = p.clone();
    q["block_rows"] = 64.into();
    let blocks = run_with("metrics.signatures", vec![("maps", disk.clone())], q);
    same(&table(&blocks), &table(&memory), "signatures of layers");
    let one = on_disk(&[&first], &dir.join("first.tif"));
    let mut p = p.clone();
    p["grid"] = 1200.into();
    let memory = run_with("metrics.signatures", vec![("maps", Value::Map(first.clone()))], p.clone());
    p["block_rows"] = 64.into();
    let blocks = run_with("metrics.signatures", vec![("maps", one)], p);
    same(&table(&blocks), &table(&memory), "signatures of zones");
    std::fs::remove_dir_all(&dir).ok();
}
