//! MSPA by tiles against landschaft's mspa_tiled (GuidosToolbox's MSPA Tiling; tests/data/reference/mspa_tiled.json,
//! tools/make_mspa_tiled_reference.py): the same maps, in memory and from rasters on disk to a GeoTIFF.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use terraflux_ops::files::Rasters;
use terraflux_ops::mspa_tiled::tile_layout;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::{Band, DataType, Raster};

fn reference() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/mspa_tiled.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn input() -> Map {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extern/landschaft/src/landschaft/data/input.tif");
    let r = Raster::read(&path).unwrap();
    Map::new(Arc::new(r.grid.clone()), r.bands[0].data.clone())
}

/// input.tif's top-left 300 x 450 cells with bands whose tiles take GTB's shortcuts (the reference's
/// `shortcuts`).
fn shortcuts(a: &Map) -> Map {
    let nx = a.grid.width;
    let mut grid = (*a.grid).clone();
    (grid.width, grid.height) = (450, 300);
    let data = (0..300 * 450)
        .map(|k| {
            let (i, j) = (k / 450, k % 450);
            let v = a.data[i * nx + j];
            if i < 100 {
                0.0
            } else if j < 150 {
                2.0
            } else if j < 250 {
                if v == 0.0 { 0.0 } else { 1.0 }
            } else if i < 200 && j < 350 {
                if ((i - 100) + (j - 250)) % 7 == 0 { 0.0 } else { 2.0 }
            } else {
                v
            }
        })
        .collect();
    Map::new(Arc::new(grid), data)
}

fn run(input: Value, params: &serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: [("map".to_string(), input)].into_iter().collect(), params: &params };
    r.get("landscape.mspa_tiled").unwrap().run(&mut Ctx::default(), &args).unwrap()
}

fn digest(values: &[f64]) -> String {
    let bytes: Vec<u8> = values.iter().map(|&v| v as u8).collect();
    hex::encode(Sha256::digest(&bytes))
}

/// The settings of a case (its tiling and MSPA's).
fn params(case: &serde_json::Value) -> serde_json::Value {
    let mut p = case.clone();
    p.as_object_mut().unwrap().retain(|k, _| !["map", "digest"].contains(&k.as_str()));
    p
}

#[test]
fn layout_is_gtbs() {
    assert_eq!(tile_layout(10_000, 6500, 2000).unwrap(), vec![(0, 6500, 0, 5500), (4500, 10000, 5500, 10000)]);
    assert_eq!(tile_layout(100, 6500, 2000).unwrap(), vec![(0, 100, 0, 100)]);
    assert!(tile_layout(100, 200, 200).is_err());
}

#[test]
fn tiles_in_memory_are_landschafts() {
    let a = input();
    let s = shortcuts(&a);
    for case in reference()["cases"].as_array().unwrap() {
        let map = if case["map"] == "input" { &a } else { &s };
        let out = run(Value::Map(map.clone()), &params(case));
        let Value::Map(m) = &out["map"] else { panic!() };
        assert_eq!(digest(&m.data), case["digest"].as_str().unwrap(), "{case}");
    }
}

/// Rasters on disk: read by windows, written by tiles to a GeoTIFF (GTB's name by default), the same maps.
#[test]
fn tiles_on_disk_are_landschafts() {
    let dir = std::env::temp_dir().join(format!("terraflux-mspa-tiled-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = input();
    let s = shortcuts(&a);
    for (name, map) in [("input", &a), ("shortcuts", &s)] {
        let src = dir.join(format!("{name}.tif"));
        let r = Raster {
            grid: (*map.grid).clone(),
            bands: vec![Band { name: None, data: map.data.to_vec() }],
            dtype: DataType::U8,
            nodata: None,
        };
        r.write(&src).unwrap();
        for case in reference()["cases"].as_array().unwrap().iter().filter(|c| c["map"] == name) {
            let disk = Value::Raster(Arc::new(Rasters::open(src.to_str().unwrap(), None, None).unwrap()));
            let mut p = params(case);
            let target: PathBuf = dir.join(format!("{name}_{}.tif", case["tile_size"]));
            p["path"] = target.to_string_lossy().into_owned().into();
            let out = run(disk, &p);
            let Value::Raster(written) = &out["map"] else { panic!("a map on disk") };
            assert_eq!(written.images[0].path(), target);
            let back = Raster::read(&target).unwrap();
            assert_eq!(back.dtype, DataType::U8);
            assert_eq!(digest(&back.bands[0].data), case["digest"].as_str().unwrap(), "disk {case}");
        }
    }
    // GTB's name next to the input by default
    let disk = Value::Raster(Arc::new(Rasters::open(dir.join("input.tif").to_str().unwrap(), None, None).unwrap()));
    run(disk, &serde_json::json!({"tile_size": 600, "overlap": 200}));
    assert!(dir.join("input_8_1_1_1.tif").exists());
    // a folder: each file, by its name, the same maps as one by one
    let folder = dir.join("folder");
    std::fs::create_dir_all(&folder).unwrap();
    for name in ["input", "shortcuts"] {
        std::fs::copy(dir.join(format!("{name}.tif")), folder.join(format!("{name}.tif"))).unwrap();
    }
    let disk = Value::Raster(Arc::new(Rasters::open(folder.to_str().unwrap(), None, None).unwrap()));
    let out_path = dir.join("out").join("{name}_mspa.tif");
    let p = serde_json::json!({"tile_size": 600, "overlap": 200, "path": out_path.to_string_lossy()});
    let Value::Raster(written) = &run(disk, &p)["map"] else { panic!("maps on disk") };
    assert_eq!(written.images.len(), 2);
    let names: Vec<&str> = written.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["input_mspa", "shortcuts_mspa"]);
    let alone = Raster::read(dir.join("input_8_1_1_1.tif")).unwrap();
    let each = Raster::read(dir.join("out").join("input_mspa.tif")).unwrap();
    assert_eq!(alone.bands[0].data, each.bands[0].data);
    std::fs::remove_dir_all(&dir).ok();
}
