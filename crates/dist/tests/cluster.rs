//! A coordinator and workers on this machine: the tasks of a batch give what local runs give, a calibration
//! finds the setting that made the "observed" map, and a task whose worker vanishes is run by another.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use terraflux_dist::Local;
use terraflux_dist::batch::{Param, calibration_tasks, grid, rank, replicates};
use terraflux_dist::client::Client;
use terraflux_dist::coordinator::Options;
use terraflux_dist::evaluate::{Evaluation, Metrics};
use terraflux_dist::proto::{TaskRequest, WorkerHello};
use terraflux_dist::worker::{self, run_task};
use terraflux_graph::{Input, Model};
use terraflux_ops::presets;

fn data(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name).to_string_lossy().into_owned()
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Dinamica's model of Sibuyan, 4 steps.
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
        &presets::Dinamica { steps: 14, simulate: 4, increment: Some(250.0), ..Default::default() },
    );
    m
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn replicates_give_what_local_runs_give() {
    let local = Local::start(2, tmp("cache-replicates"), Options::default()).await.unwrap();
    let mut c = Client::connect(&local.url).await.unwrap();
    let m = model();
    let bundled = c.bundle(&m, std::path::Path::new("")).await.unwrap();
    assert!(bundled.inputs.values().all(|i| match i {
        Input::Map { path, .. } | Input::Stack { path, .. } => path.starts_with("blob:"),
        _ => true,
    }));
    let tasks = replicates(3, 11, "seed");
    let job = c.submit("replicates", &bundled, &tasks, true, None).await.unwrap();
    let s = c.wait(&job, |_| {}).await.unwrap();
    assert_eq!((s.done, s.failed), (3, 0), "{:?}", s.errors);
    let results = c.results(&job).await.unwrap();
    for (r, (label, overrides)) in results.iter().zip(&tasks) {
        assert_eq!(&r.label, label);
        let dir = tmp(&format!("local-{label}"));
        let (_, files, _) = run_task(m.clone(), overrides, None, Some(&dir)).unwrap();
        let mine = files.iter().find(|f| f.name == "maps.tif").unwrap();
        let theirs = r.files.iter().find(|f| f.name == "maps.tif").unwrap();
        assert_eq!(mine.data, theirs.data, "{label}: the maps of the worker differ from a local run");
    }
    let w = c.workers().await.unwrap();
    assert_eq!(w.len(), 2);
    assert_eq!(w.iter().map(|x| x.done).sum::<u32>(), 3);
    local.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn calibration_finds_the_setting_of_the_observed_map() {
    // the "observed" map: the model run with isometry 1.5, seed 1
    let m = model();
    let truth: BTreeMap<String, serde_json::Value> = [("patches.isometry".to_string(), serde_json::json!(1.5))].into();
    let dir = tmp("truth");
    let (_, files, _) = run_task(m.clone(), &truth, None, Some(&dir)).unwrap();
    let observed = tmp("observed").join("observed.tif");
    std::fs::write(&observed, &files.iter().find(|f| f.name == "maps.tif").unwrap().data).unwrap();

    let local = Local::start(3, tmp("cache-calibration"), Options::default()).await.unwrap();
    let mut c = Client::connect(&local.url).await.unwrap();
    let bundled = c.bundle(&m, std::path::Path::new("")).await.unwrap();
    let eval = Evaluation {
        output: "simulation.maps".into(),
        observed: c.upload(&observed).await.unwrap(),
        band: None,
        start: "t0".into(),
        windows: vec![1, 5],
    };
    let candidates = grid(&[Param::parse("patches.isometry=0.5:2.5:5", 5).unwrap()]);
    let tasks = calibration_tasks(&candidates, &[1], "seed");
    let job = c.submit("calibration", &bundled, &tasks, false, Some(&eval)).await.unwrap();
    let s = c.wait(&job, |_| {}).await.unwrap();
    assert_eq!(s.failed, 0, "{:?}", s.errors);
    let metrics: Vec<Option<Metrics>> =
        c.results(&job).await.unwrap().iter().map(|r| serde_json::from_str(&r.metrics_json).ok()).collect();
    assert!(metrics.iter().all(|m| m.is_some()));
    let ranked = rank(&candidates, 1, &metrics, "fom").unwrap();
    assert_eq!(ranked[0].settings["patches.isometry"], serde_json::json!(1.5));
    assert_eq!(ranked[0].score, 1.0, "the same map: every change a hit");
    assert!(ranked[1].score < 1.0);
    let sim = rank(&candidates, 1, &metrics, "similarity:5").unwrap();
    assert_eq!(sim[0].settings["patches.isometry"], serde_json::json!(1.5));
    local.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_task_of_a_vanished_worker_runs_again() {
    let local = Local::start(0, tmp("cache-vanish"), Options { timeout: Duration::from_millis(800), attempts: 3 })
        .await
        .unwrap();
    let mut c = Client::connect(&local.url).await.unwrap();
    let bundled = c.bundle(&model(), std::path::Path::new("")).await.unwrap();
    let job = c.submit("one", &bundled, &replicates(1, 5, "seed"), false, None).await.unwrap();
    // a worker takes the task and is never heard of again
    let mut ghost = worker::connect(&local.url).await.unwrap();
    let ghost_id = ghost
        .register(WorkerHello { name: "ghost".into(), threads: 1, version: String::new() })
        .await
        .unwrap()
        .into_inner()
        .id;
    let t = ghost.next_task(TaskRequest { worker: ghost_id.clone(), wait_ms: 2000 }).await.unwrap().into_inner();
    assert!(!t.task_id.is_empty());
    assert_eq!(c.status(&job).await.unwrap().running, 1);
    // a real worker joins: it gets the task once the ghost times out
    let (url, stop, cache) = (local.url.clone(), local.stop.clone(), tmp("cache-vanish-2"));
    tokio::spawn(async move { worker::run(&url, worker::Options::new("real", cache), stop).await });
    let s = tokio::time::timeout(Duration::from_secs(120), c.wait(&job, |_| {})).await.unwrap().unwrap();
    assert_eq!((s.done, s.failed), (1, 0), "{:?}", s.errors);
    let r = c.results(&job).await.unwrap();
    assert_ne!(r[0].worker, ghost_id, "done by the real worker");
    local.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workers_sharing_a_cache_fetch_the_data_together() {
    // six workers of one process, an empty cache: they fetch the same blobs at the same time
    let local = Local::start(6, tmp("cache-shared"), Options::default()).await.unwrap();
    let mut c = Client::connect(&local.url).await.unwrap();
    let mut m = model();
    m.apply(&[("simulation.times".to_string(), serde_json::json!(1))].into()).unwrap();
    let bundled = c.bundle(&m, std::path::Path::new("")).await.unwrap();
    let job = c.submit("shared", &bundled, &replicates(12, 1, "seed"), false, None).await.unwrap();
    let s = c.wait(&job, |_| {}).await.unwrap();
    assert_eq!((s.done, s.failed), (12, 0), "{:?}", s.errors);
    local.shutdown();
}
