//! batch.run: an operator on many rasters, its outputs written with landschaft's (and GuidosToolbox's) naming,
//! the same as running the operator on each.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_ops::files::Rasters;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

/// The top-left `n` x `n` cells of a map, written as a GeoTIFF.
fn tile(m: &Map, row: usize, n: usize, path: &Path) -> Map {
    let mut g = (*m.grid).clone();
    let nx = g.width;
    (g.width, g.height) = (n, n);
    g.transform[3] += row as f64 * g.transform[5];
    let data: Vec<f64> = (row..row + n).flat_map(|i| m.data[i * nx..i * nx + n].to_vec()).collect();
    let t = Map::new(Arc::new(g), data);
    let raster = terraflux_raster::Raster {
        grid: (*t.grid).clone(),
        bands: vec![terraflux_raster::Band { name: None, data: t.data.to_vec() }],
        dtype: terraflux_raster::DataType::U8,
        nodata: None,
    };
    terraflux_ops::prepare::write_with_crs(&raster, path).unwrap();
    t
}

#[test]
fn batch_runs_an_operator_on_every_raster() {
    let dir = root().join("target/tmp/batch");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("tiles")).unwrap();
    let src = root().join("extern/landschaft/src/landschaft/data/input.tif");
    let m = Rasters::open(src.to_str().unwrap(), None, None).unwrap().map().unwrap();
    let tiles: Vec<Map> =
        [0, 300].iter().map(|&r| tile(&m, r, 300, &dir.join(format!("tiles/forest_{r}.tif")))).collect();

    // MSPA with GuidosToolbox's naming
    let out = run(
        "batch.run",
        vec![],
        serde_json::json!({"operator": "landscape.mspa", "inputs": dir.join("tiles").to_str().unwrap(),
            "output_dir": dir.join("mspa").to_str().unwrap(), "params": {"edge_width": 2}, "workers": 2}),
    );
    let Value::Table(t) = &out["table"] else { panic!() };
    assert!(t.rows.iter().all(|r| r[1] == 1.0), "{:?}", out["report"]);
    for (r, tile) in [0, 300].iter().zip(&tiles) {
        let path = dir.join(format!("mspa/forest_{r}_8_2_1_1.tif"));
        let got = Rasters::open(path.to_str().unwrap(), None, None).unwrap().map().unwrap();
        let want = run("landscape.mspa", vec![("map", Value::Map(tile.clone()))], serde_json::json!({"edge_width": 2}));
        let Value::Map(want) = &want["map"] else { panic!() };
        assert_eq!(got.data.len(), want.data.len());
        assert!(got.data.iter().zip(want.data.iter()).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())), "tile {r}");
    }

    // another operator: its outputs and a JSON per input; done inputs skipped; errors reported per file
    let params = serde_json::json!({"operator": "landscape.accounting",
        "inputs": [dir.join("tiles/*.tif").to_str().unwrap(), dir.join("tiles/missing.tif").to_str().unwrap()],
        "output_dir": dir.join("acc").to_str().unwrap()});
    let out = run("batch.run", vec![], params.clone());
    let Value::Table(t) = &out["table"] else { panic!() };
    let ok: Vec<f64> = t.rows.iter().map(|r| r[1]).collect();
    assert_eq!(ok, [1.0, 1.0, 0.0], "{:?}", out["report"]);
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("acc/forest_0_landscape_accounting.json")).unwrap())
            .unwrap();
    assert_eq!(meta["tool"], "landscape.accounting");
    let files = meta["outputs"].as_array().unwrap();
    assert!(!files.is_empty() && files.iter().all(|f| Path::new(f.as_str().unwrap()).exists()));
    let again = run("batch.run", vec![], params);
    let Value::Table(t2) = &again["table"] else { panic!() };
    assert!(t2.rows[..2].iter().all(|r| r[1] == 1.0 && r[2] < t.rows[0][2].max(0.01)));
}
