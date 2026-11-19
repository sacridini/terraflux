//! The tiled engine on a coordinator's workers: the maps of a model run with its tiles spread over workers
//! (sharing this machine's disk) are those of the same model run on this machine's threads.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_dist::Local;
use terraflux_dist::coordinator::Options as CoordinatorOptions;
use terraflux_dist::tiles::Remote;
use terraflux_engine::dinamica::Settings;
use terraflux_engine::graph::{Options, run};
use terraflux_graph::{Input, Model};
use terraflux_ops::presets;
use terraflux_raster::Raster;

fn data(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name).to_string_lossy().into_owned()
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Dinamica's model of Sibuyan, 3 steps.
fn model() -> Model {
    let mut m = Model::new("dinamica");
    let map = |band: &str| Input::Map {
        path: data("sibuyan_landuse.tif"),
        band: Some(band.into()),
        nodata: None,
        note: None,
        data: None,
    };
    m.inputs.insert("t0".into(), map("1997"));
    m.inputs.insert("t1".into(), map("2011"));
    m.inputs.insert(
        "drivers".into(),
        Input::Stack { path: data("sibuyan_drivers.tif"), bands: None, nodata: None, note: None, data: None },
    );
    m.inputs.insert("seed".into(), Input::Value { value: 1.into(), note: None });
    presets::dinamica(
        &mut m,
        &presets::Dinamica { steps: 14, simulate: 3, increment: Some(250.0), ..Default::default() },
    );
    m
}

/// The maps a run wrote, step by step.
fn maps(out: &Path) -> Vec<Vec<f64>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(out.join("maps"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "tif"))
        .collect();
    files.sort();
    files.iter().map(|f| Raster::read(f).unwrap().bands[0].data.clone()).collect()
}

fn options(tile: usize, disk: PathBuf, runner: Option<Arc<Remote>>) -> Options {
    Options {
        tile,
        settings: Settings { radius: 8, ..Default::default() },
        disk: Some(disk),
        runner: runner.map(|r| r as Arc<dyn terraflux_engine::pass::Runner>),
    }
}

#[test]
fn tiles_on_workers_give_the_maps_of_this_machine() {
    terraflux_ops::threads::ensure();
    let m = model();
    // on this machine's threads
    let here = tmp("tiles-here");
    run(&m, Path::new(""), &here.join("out"), &options(64, here.join("layers"), None), |_| {}).unwrap();
    let want = maps(&here.join("out"));
    assert_eq!(want.len(), 3);

    // on two workers (a coordinator in this process, the disk shared)
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let local = rt.block_on(Local::start(2, tmp("tiles-cache"), CoordinatorOptions::default())).unwrap();
    // the same tiles, cut in runs of any length among the workers (the tile size changes the patches
    // measured, so it stays)
    let tile = 64;
    for per_worker in [4, 1, 16] {
        let mut remote = Remote::connect(&local.url).unwrap();
        remote.per_worker = per_worker;
        remote.shared = true;
        let there = tmp(&format!("tiles-there-{per_worker}"));
        let opt = options(tile, there.join("layers"), Some(Arc::new(remote)));
        run(&m, Path::new(""), &there.join("out"), &opt, |_| {}).unwrap();
        let got = maps(&there.join("out"));
        assert_eq!(got.len(), want.len());
        for (k, (a, b)) in got.iter().zip(&want).enumerate() {
            let differ = a.iter().zip(b).filter(|(x, y)| x != y && !(x.is_nan() && y.is_nan())).count();
            assert_eq!(differ, 0, "{per_worker} runs per worker, step {}: {differ} cells differ", k + 1);
        }
    }
    local.shutdown();
}

#[test]
fn tiles_on_workers_without_a_shared_disk_give_the_maps_of_this_machine() {
    terraflux_ops::threads::ensure();
    let m = model();
    let here = tmp("tiles-sent-here");
    run(&m, Path::new(""), &here.join("out"), &options(64, here.join("layers"), None), |_| {}).unwrap();
    let want = maps(&here.join("out"));
    // three workers with caches of their own: the maps and the layers' tiles go to them through the
    // coordinator, the tiles they write come back here
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let local = rt.block_on(Local::start(0, tmp("tiles-sent-cache"), CoordinatorOptions::default())).unwrap();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    for k in 0..3 {
        let mut o = terraflux_dist::worker::Options::new(&format!("w{k}"), tmp(&format!("tiles-sent-w{k}")));
        o.threads = 2;
        let (u, s) = (local.url.clone(), stop.clone());
        rt.spawn(async move { terraflux_dist::worker::run(&u, o, s).await });
    }
    let remote = Remote::connect(&local.url).unwrap();
    assert!(!remote.shared, "nothing shared by default");
    let there = tmp("tiles-sent-there");
    let opt = options(64, there.join("layers"), Some(Arc::new(remote)));
    run(&m, Path::new(""), &there.join("out"), &opt, |_| {}).unwrap();
    let got = maps(&there.join("out"));
    assert_eq!(got.len(), want.len());
    for (k, (a, b)) in got.iter().zip(&want).enumerate() {
        let differ = a.iter().zip(b).filter(|(x, y)| x != y && !(x.is_nan() && y.is_nan())).count();
        assert_eq!(differ, 0, "step {}: {differ} cells differ", k + 1);
    }
    // the workers read nothing of this machine's folders: their own caches had the layers' tiles
    let cached = |k: usize| {
        tmp_files(&PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("tiles-sent-w{k}")).join("layers"))
    };
    assert!((0..3).map(cached).sum::<usize>() > 0);
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    local.shutdown();
}

/// The files under a folder.
fn tmp_files(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| if e.path().is_dir() { tmp_files(&e.path()) } else { 1 })
        .sum()
}

#[test]
fn maps_in_memory_do_not_go_to_workers() {
    terraflux_ops::threads::ensure();
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let local = rt.block_on(Local::start(1, tmp("tiles-cache-memory"), CoordinatorOptions::default())).unwrap();
    let mut opt = options(64, PathBuf::new(), Some(Arc::new(Remote::connect(&local.url).unwrap())));
    opt.disk = None;
    let e = run(&model(), Path::new(""), &tmp("tiles-memory"), &opt, |_| {}).err().expect("an error");
    assert!(e.to_string().contains("on disk"), "{e}");
    local.shutdown();
}

/// A GeoTIFF of three f32 bands: blocks of two levels plus noise, a few cells without data.
fn write_image(path: &Path, w: usize, h: usize) {
    use terraflux_raster::{Band, DataType, Grid};
    let mut s = 5u64;
    let mut lcg = move || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((s >> 11) as f64) / (1u64 << 53) as f64
    };
    let bands: Vec<Band> = (0..3)
        .map(|b| {
            let data = (0..w * h)
                .map(|i| {
                    let (r, c) = (i / w, i % w);
                    let level = if (r / 37 + c / 41) % 2 == 0 { 10.0 } else { 70.0 };
                    let noise = (lcg() + lcg() - 1.0) * 6.0;
                    if b == 0 && i % 211 == 0 { -9999.0 } else { (level + 3.0 * b as f64 + noise) as f32 as f64 }
                })
                .collect();
            Band { name: Some(format!("B{}", b + 2)), data }
        })
        .collect();
    let mut grid = Grid::new(w, h);
    grid.transform = [500_000.0, 10.0, 0.0, 5_000_000.0, 0.0, -10.0];
    Raster { grid, bands, dtype: DataType::F32, nodata: Some(-9999.0) }.write(path).unwrap();
}

/// The objects `segment.multiresolution` makes of the rasters in `path`, by tiles of 64, with the tiles run by
/// `runner` when there is one.
fn segment(path: &Path, runner: Option<Arc<Remote>>) -> Vec<u32> {
    segment_bounded(path, runner, None)
}

/// The same with `bounds` (a map of the image's grid) when given.
fn segment_bounded(path: &Path, runner: Option<Arc<Remote>>, bounds: Option<terraflux_ops::Map>) -> Vec<u32> {
    use terraflux_ops::files::Rasters;
    use terraflux_ops::{Args, Ctx, Value};
    let r = Arc::new(Rasters::open(&path.to_string_lossy(), None, None).unwrap());
    let params = serde_json::json!({ "scale": 25, "shape": 0.1, "tile": 64, "margin": 12, "seed": 3 });
    let params = params.as_object().cloned().unwrap();
    let mut inputs: std::collections::BTreeMap<String, Value> = [("image".to_string(), Value::Raster(r))].into();
    if let Some(m) = bounds {
        inputs.insert("bounds".into(), Value::Map(m));
    }
    let args = Args { node: "n", inputs, params: &params };
    let registry = terraflux_ops::Registry::standard();
    let mut ctx = Ctx::default().with_tile_runner(runner.map(|r| r as Arc<dyn terraflux_ops::objects::TileCores>));
    let out = registry.get("segment.multiresolution").unwrap().run(&mut ctx, &args).unwrap();
    assert!(ctx.warnings.is_empty(), "{:?}", ctx.warnings);
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    assert!(o.count > 4, "{} objects", o.count);
    o.labels.to_vec()
}

#[test]
fn segmented_tiles_with_bounds_on_workers_give_the_objects_of_this_machine() {
    terraflux_ops::threads::ensure();
    let d = tmp("segment-bounds-data");
    let path = d.join("img.tif");
    write_image(&path, 256, 256);
    // zones of 100 x 90 cells: their borders cross the tiles of 64 (the image's own blocks are 37 x 41)
    let grid = terraflux_ops::files::Rasters::open(&path.to_string_lossy(), None, None).unwrap().grid.clone();
    let zone: Vec<f64> = (0..256 * 256).map(|i| ((i % 256) / 100 + 3 * ((i / 256) / 90)) as f64).collect();
    let bounds = terraflux_ops::Map::new(grid, zone.clone());
    let want = segment_bounded(&path, None, Some(bounds.clone()));
    let mut of = std::collections::HashMap::new();
    for (i, &l) in want.iter().enumerate() {
        if l != 0 {
            assert_eq!(*of.entry(l).or_insert(zone[i]), zone[i], "object {l} crosses a zone at cell {i}");
        }
    }
    assert_ne!(want, segment(&path, None), "the zones change the objects");

    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let local = rt.block_on(Local::start(2, tmp("segment-bounds-cache"), CoordinatorOptions::default())).unwrap();
    for per_worker in [4, 1] {
        let mut remote = Remote::connect(&local.url).unwrap();
        remote.per_worker = per_worker;
        remote.shared = true;
        let got = segment_bounded(&path, Some(Arc::new(remote)), Some(bounds.clone()));
        assert_eq!(got, want, "{per_worker} runs per worker, with bounds");
    }
    local.shutdown();
}

#[test]
fn segmented_tiles_on_workers_give_the_objects_of_this_machine() {
    terraflux_ops::threads::ensure();
    let d = tmp("segment-data");
    let path = d.join("img.tif");
    write_image(&path, 256, 256);
    let want = segment(&path, None);

    // two workers sharing this machine's disk, the tiles cut in runs of any length
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let local = rt.block_on(Local::start(2, tmp("segment-cache"), CoordinatorOptions::default())).unwrap();
    for per_worker in [4, 1, 16] {
        let mut remote = Remote::connect(&local.url).unwrap();
        remote.per_worker = per_worker;
        remote.shared = true;
        assert_eq!(segment(&path, Some(Arc::new(remote))), want, "{per_worker} runs per worker, shared disk");
    }
    local.shutdown();

    // three workers with caches of their own: the file goes to them through the coordinator
    let local = rt.block_on(Local::start(0, tmp("segment-sent-cache"), CoordinatorOptions::default())).unwrap();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    for k in 0..3 {
        let mut o = terraflux_dist::worker::Options::new(&format!("w{k}"), tmp(&format!("segment-w{k}")));
        o.threads = 2;
        let (u, s) = (local.url.clone(), stop.clone());
        rt.spawn(async move { terraflux_dist::worker::run(&u, o, s).await });
    }
    let remote = Remote::connect(&local.url).unwrap();
    assert_eq!(segment(&path, Some(Arc::new(remote))), want, "files through the coordinator");
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    local.shutdown();
}

/// The objects of `segment.multiresolution` over the rasters in `path` (whole, no tiles).
fn objects_of(path: &Path) -> Arc<terraflux_ops::objects::Objects> {
    use terraflux_ops::files::Rasters;
    use terraflux_ops::{Args, Ctx, Value};
    let r = Arc::new(Rasters::open(&path.to_string_lossy(), None, None).unwrap());
    let params = serde_json::json!({ "scale": 25, "shape": 0.1, "seed": 3 });
    let params = params.as_object().cloned().unwrap();
    let args = Args { node: "n", inputs: [("image".to_string(), Value::Raster(r))].into(), params: &params };
    let out = terraflux_ops::Registry::standard()
        .get("segment.multiresolution")
        .unwrap()
        .run(&mut Ctx::default(), &args)
        .unwrap();
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

/// The table `objects.features` gives (every group), with the work run by `runner` when there is one.
fn features(
    path: &Path,
    o: &Arc<terraflux_ops::objects::Objects>,
    runner: Option<Arc<Remote>>,
) -> terraflux_ops::Table {
    use terraflux_ops::files::Rasters;
    use terraflux_ops::{Args, Ctx, Value};
    let r = Arc::new(Rasters::open(&path.to_string_lossy(), None, None).unwrap());
    let params = serde_json::json!({ "features": "all", "levels": 16 });
    let params = params.as_object().cloned().unwrap();
    let inputs = [("objects".to_string(), Value::Objects(o.clone())), ("image".to_string(), Value::Raster(r))];
    let args = Args { node: "n", inputs: inputs.into(), params: &params };
    let mut ctx = Ctx::default().with_tile_runner(runner.map(|r| r as Arc<dyn terraflux_ops::objects::TileCores>));
    let out = terraflux_ops::Registry::standard().get("objects.features").unwrap().run(&mut ctx, &args).unwrap();
    // (the advice on the image comes only from a run that reads it whole; a failure on the cluster must not pass)
    assert!(ctx.warnings.iter().all(|w| !w.contains("cluster")), "{:?}", ctx.warnings);
    let Value::Table(t) = &out["table"] else { panic!("table") };
    t.clone()
}

fn same_table(a: &terraflux_ops::Table, b: &terraflux_ops::Table, what: &str) {
    assert_eq!(a.columns, b.columns, "{what}: columns");
    assert_eq!(a.rows.len(), b.rows.len(), "{what}: rows");
    for (j, name) in a.columns.iter().enumerate() {
        for (i, (x, y)) in a.rows.iter().zip(&b.rows).enumerate() {
            assert!(
                x[j] == y[j] || (x[j].is_nan() && y[j].is_nan()),
                "{what}: {name} of object {}: {} {}",
                i + 1,
                x[j],
                y[j]
            );
        }
    }
}

#[test]
fn features_on_workers_give_the_table_of_this_machine() {
    terraflux_ops::threads::ensure();
    let d = tmp("features-data");
    let path = d.join("img.tif");
    write_image(&path, 160, 120);
    let o = objects_of(&path);
    assert!(o.count > 6, "{} objects", o.count);
    let want = features(&path, &o, None);
    assert!(want.columns.len() > 60, "{} columns", want.columns.len());

    // two workers sharing this machine's disk
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let local = rt.block_on(Local::start(2, tmp("features-cache"), CoordinatorOptions::default())).unwrap();
    let mut remote = Remote::connect(&local.url).unwrap();
    remote.shared = true;
    same_table(&features(&path, &o, Some(Arc::new(remote))), &want, "shared disk");
    local.shutdown();

    // three workers with caches of their own: the rasters and the labels go through the coordinator
    let local = rt.block_on(Local::start(0, tmp("features-sent-cache"), CoordinatorOptions::default())).unwrap();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    for k in 0..3 {
        let mut w = terraflux_dist::worker::Options::new(&format!("f{k}"), tmp(&format!("features-w{k}")));
        w.threads = 2;
        let (u, s) = (local.url.clone(), stop.clone());
        rt.spawn(async move { terraflux_dist::worker::run(&u, w, s).await });
    }
    let remote = Remote::connect(&local.url).unwrap();
    same_table(&features(&path, &o, Some(Arc::new(remote))), &want, "files through the coordinator");
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    local.shutdown();
}
