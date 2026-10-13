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
    // by default, the metrics by blocks only
    let out = run("metrics.class", disk.clone(), serde_json::json!({"extras": true, "block_rows": 64}));
    assert!(!table(&out).columns.iter().any(|c| c.starts_with("prox") || c.starts_with("econ")));
    let r = Registry::standard();
    let params = serde_json::json!({"metrics": ["prox_mn"], "block_rows": 64}).as_object().cloned().unwrap();
    let args = Args { node: "n", inputs: [("map".to_string(), disk)].into_iter().collect(), params: &params };
    assert!(r.get("metrics.class").unwrap().run(&mut Ctx::default(), &args).is_err());
    std::fs::remove_dir_all(&dir).ok();
}
