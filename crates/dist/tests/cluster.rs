//! A coordinator and workers on this machine: the tasks of a batch give what local runs give, a calibration
//! finds the setting that made the "observed" map, and a task whose worker vanishes is run by another.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use terraflux_dist::Local;
use terraflux_dist::batch::{Param, calibration_tasks, grid, rank, replicates, seed_keys};
use terraflux_dist::client::Client;
use terraflux_dist::coordinator::Options;
use terraflux_dist::coordinator::VERSION;
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
        Input::Map { path, .. } | Input::Stack { path, .. } => path.starts_with("bundle:"),
        _ => true,
    }));
    let tasks = replicates(3, 11, &seed_keys("seed"));
    let job = c.submit("replicates", &bundled, &tasks, true, None).await.unwrap();
    let s = c.wait(&job, |_| {}).await.unwrap();
    assert_eq!((s.done, s.failed), (3, 0), "{:?}", s.errors);
    let results = c.results(&job).await.unwrap();
    for (r, (label, overrides)) in results.iter().zip(&tasks) {
        assert_eq!(&r.label, label);
        let dir = tmp(&format!("local-{label}"));
        let (_, files, _) = run_task(m.clone(), overrides, None, &dir, true).unwrap();
        let mine = files.iter().find(|f| f.name == "maps.tif").unwrap();
        let theirs = c.fetch(&r.files.iter().find(|f| f.name == "maps.tif").unwrap().hash).await.unwrap();
        assert_eq!(mine.data, theirs, "{label}: the maps of the worker differ from a local run");
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
    let (_, files, _) = run_task(m.clone(), &truth, None, &dir, true).unwrap();
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
    let tasks = calibration_tasks(&candidates, &[1], &seed_keys("seed"));
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
    let local = Local::start(
        0,
        tmp("cache-vanish"),
        Options { timeout: Duration::from_millis(800), attempts: 3, ..Default::default() },
    )
    .await
    .unwrap();
    let mut c = Client::connect(&local.url).await.unwrap();
    let bundled = c.bundle(&model(), std::path::Path::new("")).await.unwrap();
    let job = c.submit("one", &bundled, &replicates(1, 5, &seed_keys("seed")), false, None).await.unwrap();
    // a worker takes the task and is never heard of again
    let mut ghost = worker::connect(&local.url, None).await.unwrap();
    let ghost_id = ghost
        .register(WorkerHello { name: "ghost".into(), threads: 1, version: VERSION.into() })
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
    let job = c.submit("shared", &bundled, &replicates(12, 1, &seed_keys("seed")), false, None).await.unwrap();
    let s = c.wait(&job, |_| {}).await.unwrap();
    assert_eq!((s.done, s.failed), (12, 0), "{:?}", s.errors);
    local.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plugins_travel_to_the_workers() {
    // a submodel of map algebra and a WebAssembly plugin on Sibuyan's land use; the module sent to the workers
    // by hash
    let dir = tmp("plugin-model");
    std::fs::write(
        dir.join("double.wat"),
        r#"(module (func (export "cell") (param $x f64) (result f64) (f64.mul (local.get $x) (f64.const 2))))"#,
    )
    .unwrap();
    let mut m = Model::new("plugin");
    m.inputs.insert(
        "t0".into(),
        Input::Map { path: data("sibuyan_landuse.tif"), band: Some("1997".into()), nodata: None, note: None },
    );
    // the map algebra in a submodel
    std::fs::write(
        dir.join("forest.toml"),
        "format = \"terraflux/1\"
[inputs.lu]
kind = \"map\"
path = \"none.tif\"
[[nodes]]
id = \"is\"
op = \"map.calc\"
inputs = { lu = \"$lu\" }
params = { expression = \"lu == 1\" }
[outputs.map]
from = \"is.map\"
",
    )
    .unwrap();
    m.nodes.push(
        terraflux_graph::Node::new("forest", terraflux_graph::SUBMODEL).input("lu", "$t0").param("path", "forest.toml"),
    );
    m.nodes.push(
        terraflux_graph::Node::new("twice", terraflux_graph::PLUGIN)
            .input("x", "forest.map")
            .param("path", "double.wat"),
    );
    m.outputs.insert(
        "twice".into(),
        terraflux_graph::Output { from: "twice.map".into(), path: None, dtype: Some("u8".into()), layer: None },
    );
    let path = dir.join("model.toml");
    m.save(&path).unwrap();
    let m = Model::load_expanded(&path).unwrap();

    let local = Local::start(2, tmp("cache-plugin"), Options::default()).await.unwrap();
    let mut c = Client::connect(&local.url).await.unwrap();
    let bundled = c.bundle(&m, &dir).await.unwrap();
    let p = bundled.node("twice").unwrap().params["path"].as_str().unwrap().to_string();
    assert!(p.starts_with("bundle:") && p.ends_with("/double.wat"), "{p}");
    let job = c.submit("plugin", &bundled, &[("one".to_string(), BTreeMap::new())], true, None).await.unwrap();
    let s = c.wait(&job, |_| {}).await.unwrap();
    assert_eq!((s.done, s.failed), (1, 0), "{:?}", s.errors);
    let hash = c.results(&job).await.unwrap()[0].files.iter().find(|f| f.name == "twice.tif").unwrap().hash.clone();
    let theirs = c.fetch(&hash).await.unwrap();
    let (_, files, _) = run_task(m.clone(), &BTreeMap::new(), None, &tmp("plugin-local"), true).unwrap();
    let mine = &files.iter().find(|f| f.name == "twice.tif").unwrap().data;
    assert_eq!(mine, &theirs);
    let f = tmp("plugin-out").join("twice.tif");
    std::fs::write(&f, &theirs).unwrap();
    let r = terraflux_raster::Raster::read(&f).unwrap();
    let values: std::collections::BTreeSet<u64> =
        r.bands[0].masked(r.nodata).iter().filter(|v| !v.is_nan()).map(|v| v.to_bits()).collect();
    assert_eq!(values, [0.0f64.to_bits(), 2.0f64.to_bits()].into(), "forest cells doubled, the others 0");
    local.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_batch_writes_what_its_nodes_write_in_a_folder_per_run() {
    // a model of the builder: a neutral map by a node's seed, written by a node to a file of this machine
    let dir = tmp("batch-writes");
    let target = dir.join("noise.tif");
    let mut m = Model::new("writes");
    m.nodes.push(
        terraflux_graph::Node::new("noise", "nlm.random").param("rows", 20).param("columns", 30).param("seed", 1),
    );
    m.nodes.push(
        terraflux_graph::Node::new("save", "raster.write")
            .input("value", "noise.map")
            .param("path", target.to_string_lossy().as_ref()),
    );
    let b = terraflux_dist::run::Batch {
        name: "writes".into(),
        model: m,
        base: dir.clone(),
        tasks: replicates(3, 1, &seed_keys("noise.seed")),
        files: true,
        evaluation: None,
    };
    let place = terraflux_dist::run::Place::Here { workers: 2, cache: tmp("cache-batch-writes"), state: None };
    let mut seen = Vec::new();
    let out = dir.join("out");
    let start = terraflux_dist::run::Start::New(Box::new(b));
    let stop = std::sync::atomic::AtomicBool::new(false);
    let o = terraflux_dist::run::run(&start, &place, Some(&out), &stop, &mut |p| seen.push(p)).await.unwrap();
    assert_eq!((o.status.done, o.status.failed), (3, 0), "{:?}", o.status.errors);
    assert!(seen.iter().any(|p| matches!(p, terraflux_dist::run::Progress::Submitted { total: 3, .. })));
    assert!(!target.exists(), "the runs wrote in their own folders, not in the model's file");
    let read =
        |label: &str| terraflux_raster::Raster::read(out.join(label).join("noise.tif")).unwrap().bands[0].clone();
    let (a, b2) = (read("seed_1"), read("seed_2"));
    assert_ne!(a.masked(None), b2.masked(None), "another seed, another map");
    assert!(std::fs::read_to_string(out.join("tasks.csv")).unwrap().lines().count() == 4);
    // the runs as one series: a pattern in the folders' names
    let s =
        terraflux_ops::files::Rasters::open(&out.join("*").join("noise.tif").to_string_lossy(), None, None).unwrap();
    assert_eq!(s.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["seed_1", "seed_2", "seed_3"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workers_and_clients_of_another_version_are_turned_away() {
    let local = Local::start(0, tmp("cache-version"), Options::default()).await.unwrap();
    let mut w = worker::connect(&local.url, None).await.unwrap();
    let e = w.register(WorkerHello { name: "old".into(), threads: 1, version: "0.1.0".into() }).await.unwrap_err();
    assert_eq!(e.code(), tonic::Code::FailedPrecondition);
    assert!(e.message().contains("0.1.0") && e.message().contains(VERSION), "{}", e.message());
    assert!(Client::connect(&local.url).await.is_ok());
    let jobs = Client::connect(&local.url).await.unwrap().workers().await.unwrap();
    assert!(jobs.is_empty(), "the old worker was not registered");
    local.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_coordinator_started_again_goes_on_with_its_jobs() {
    // a job of four runs, a coordinator with a folder of its own and no workers: stopped, then started again
    let state = tmp("coordinator-again");
    let options = || Options { dir: Some(state.clone()), ..Default::default() };
    let local = Local::start(0, tmp("cache-again"), options()).await.unwrap();
    let mut c = Client::connect(&local.url).await.unwrap();
    let mut m = model();
    m.apply(&[("simulation.times".to_string(), serde_json::json!(1))].into()).unwrap();
    let bundled = c.bundle(&m, std::path::Path::new("")).await.unwrap();
    let job = c.submit("again", &bundled, &replicates(4, 1, &seed_keys("seed")), true, None).await.unwrap();
    // one run done by hand before the stop
    let mut w = worker::connect(&local.url, None).await.unwrap();
    let wid = w
        .register(WorkerHello { name: "w".into(), threads: 1, version: VERSION.into() })
        .await
        .unwrap()
        .into_inner()
        .id;
    let t = w.next_task(TaskRequest { worker: wid.clone(), wait_ms: 2000 }).await.unwrap().into_inner();
    w.complete(terraflux_dist::proto::TaskResult {
        task_id: t.task_id.clone(),
        worker: wid.clone(),
        ok: true,
        ..Default::default()
    })
    .await
    .unwrap();
    // a second run taken and never finished: it runs again after the restart
    let t = w.next_task(TaskRequest { worker: wid, wait_ms: 2000 }).await.unwrap().into_inner();
    assert!(!t.task_id.is_empty());
    local.service.save().unwrap();
    local.shutdown();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let local = Local::start(2, tmp("cache-again-2"), options()).await.unwrap();
    let mut c = Client::connect(&local.url).await.unwrap();
    let jobs = c.jobs().await.unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!((jobs[0].id.as_str(), jobs[0].done, jobs[0].total), (job.as_str(), 1, 4));
    let out = tmp("again-out");
    let start = terraflux_dist::run::Start::Resume(job.clone());
    let place = terraflux_dist::run::Place::At { url: local.url.clone(), code: None };
    let stop = std::sync::atomic::AtomicBool::new(false);
    let o = terraflux_dist::run::run(&start, &place, Some(&out), &stop, &mut |_| {}).await.unwrap();
    assert_eq!((o.status.done, o.status.failed), (4, 0), "{:?}", o.status.errors);
    for label in ["seed_2", "seed_3", "seed_4"] {
        assert!(out.join(label).join("maps.tif").exists(), "{label}");
    }
    assert!(c.jobs().await.unwrap().is_empty(), "the job forgotten once fetched");
    local.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_files_nodes_read_and_scenarios_give_travel_with_the_model() {
    // a model that reads a raster and a shapefile by nodes, from a folder the workers never see; a scenario
    // that gives another raster
    let dir = tmp("travel");
    let src = terraflux_raster::Raster::read(data("sibuyan_landuse.tif")).unwrap();
    let mut one = src.clone();
    one.bands.truncate(1);
    one.write(dir.join("a.tif")).unwrap();
    let mut other = src.clone();
    other.bands.drain(..1);
    other.write(dir.join("b.tif")).unwrap();
    let mut m = Model::new("travel");
    m.nodes.push(
        terraflux_graph::Node::new("read", "raster.read").param("path", dir.join("a.tif").to_string_lossy().as_ref()),
    );
    m.nodes
        .push(terraflux_graph::Node::new("forest", "map.calc").input("lu", "read.map").param("expression", "lu == 1"));
    m.outputs.insert(
        "forest".into(),
        terraflux_graph::Output { from: "forest.map".into(), path: None, dtype: Some("u8".into()), layer: None },
    );
    let tasks: Vec<terraflux_dist::client::TaskOf> = vec![
        ("a".into(), BTreeMap::new()),
        ("b".into(), [("read.path".to_string(), serde_json::json!(dir.join("b.tif").to_string_lossy()))].into()),
    ];
    let local = Local::start(2, tmp("cache-travel"), Options::default()).await.unwrap();
    let mut c = Client::connect(&local.url).await.unwrap();
    let bundled = c.bundle(&m, &dir).await.unwrap();
    let p = bundled.node("read").unwrap().params["path"].as_str().unwrap().to_string();
    assert!(p.starts_with("bundle:") && p.ends_with("/a.tif"), "{p}");
    let sent = c.bundle_tasks(&m, &dir, &tasks).await.unwrap();
    assert!(sent[1].1["read.path"].as_str().unwrap().starts_with("bundle:"));
    let start = terraflux_dist::run::Start::New(Box::new(terraflux_dist::run::Batch {
        name: "travel".into(),
        model: m.clone(),
        base: dir.clone(),
        tasks,
        files: true,
        evaluation: None,
    }));
    let out = dir.join("out");
    let stop = std::sync::atomic::AtomicBool::new(false);
    let place = terraflux_dist::run::Place::At { url: local.url.clone(), code: None };
    let o = terraflux_dist::run::run(&start, &place, Some(&out), &stop, &mut |_| {}).await.unwrap();
    assert_eq!((o.status.done, o.status.failed), (2, 0), "{:?}", o.status.errors);
    let a = terraflux_raster::Raster::read(out.join("a").join("forest.tif")).unwrap();
    let b = terraflux_raster::Raster::read(out.join("b").join("forest.tif")).unwrap();
    assert_ne!(a.bands[0].data, b.bands[0].data, "the scenario read the other map");
    let forest = |r: &terraflux_raster::Raster| r.bands[0].data.iter().filter(|v| **v == 1.0).count();
    let (r1, r2) = (&src.bands[0], &src.bands[1]);
    assert_eq!(forest(&a), r1.data.iter().filter(|v| **v == 1.0).count());
    assert_eq!(forest(&b), r2.data.iter().filter(|v| **v == 1.0).count());
    local.shutdown();
}
