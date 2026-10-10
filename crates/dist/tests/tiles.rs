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
    };
    m.inputs.insert("t0".into(), map("1997"));
    m.inputs.insert("t1".into(), map("2011"));
    m.inputs.insert(
        "drivers".into(),
        Input::Stack { path: data("sibuyan_drivers.tif"), bands: None, nodata: None, note: None },
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
fn maps_in_memory_do_not_go_to_workers() {
    terraflux_ops::threads::ensure();
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let local = rt.block_on(Local::start(1, tmp("tiles-cache-memory"), CoordinatorOptions::default())).unwrap();
    let mut opt = options(64, PathBuf::new(), Some(Arc::new(Remote::connect(&local.url).unwrap())));
    opt.disk = None;
    let e = run(&model(), Path::new(""), &tmp("tiles-memory"), &opt, |_| {}).err().expect("an error");
    assert!(e.to_string().contains("shared disk"), "{e}");
    local.shutdown();
}
