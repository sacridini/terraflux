//! A headless worker: it asks the coordinator for tasks, fetches their data once (a cache by hash), runs the
//! model with the task's overrides (heartbeats while it runs) and sends back the files or the metrics.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use terraflux_exec::{Engine, Quiet, load_inputs, write_outputs};
use terraflux_graph::{Input, Model};

use crate::evaluate::{Evaluation, evaluate};
use crate::net::Conn;
use crate::proto::*;
use crate::secure::Code;
use crate::{Error, Result, bundle};

#[derive(Debug, Clone)]
pub struct Options {
    pub name: String,
    /// Where the data of the tasks is kept (by hash) and their outputs written.
    pub cache: PathBuf,
    /// Seconds between heartbeats while a task runs.
    pub heartbeat: Duration,
    /// The kernels' threads of each task (never every core).
    pub threads: usize,
    /// Take tasks only when the computer's user has been away this long (none: always). A task taken runs to
    /// its end.
    pub idle: Option<Duration>,
    /// Asked between tasks: whether to take no task for now.
    pub paused: Option<Arc<AtomicBool>>,
    /// The coordinator's pairing code (none: a plain connection).
    pub code: Option<Code>,
    /// Where to tell what it does.
    pub status: Option<Arc<std::sync::Mutex<Status>>>,
}

impl Options {
    pub fn new(name: &str, cache: impl Into<PathBuf>) -> Self {
        Options {
            name: name.to_string(),
            cache: cache.into(),
            heartbeat: Duration::from_secs(5),
            threads: terraflux_ops::threads::default_threads(),
            idle: None,
            paused: None,
            code: None,
            status: None,
        }
    }

    /// Whether to wait before taking a task: paused, or the user at the computer.
    fn waiting(&self) -> bool {
        self.paused.as_ref().is_some_and(|p| p.load(Ordering::Relaxed))
            || self.idle.is_some_and(|d| crate::idle::seconds().is_some_and(|s| s < d.as_secs_f64()))
    }
}

/// Connect to a coordinator (plain, or with a pairing code).
pub async fn connect(url: &str, code: Option<&Code>) -> Result<Conn> {
    crate::net::connect(url, code).await
}

/// A file a task wrote: its name and its bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub name: String,
    pub data: Vec<u8>,
}

/// The paths sent with a model (`bundle:<hash>/...`) as files of the cache: its inputs, the files its nodes
/// read, an evaluation's observed map and the files a task's overrides give.
pub async fn localize(
    client: &mut Conn,
    cache: &Path,
    model: &mut Model,
    eval: Option<&mut Evaluation>,
    overrides: &mut BTreeMap<String, serde_json::Value>,
) -> Result<()> {
    for input in model.inputs.values_mut() {
        if let Input::Map { path, .. } | Input::Stack { path, .. } = input {
            *path = bundle::local(client, cache, path).await?;
        }
    }
    let mut paths = Vec::new();
    crate::client::read_paths(&mut model.nodes, &mut |p| paths.push(p.clone()));
    let mut local = BTreeMap::new();
    for p in paths {
        if let std::collections::btree_map::Entry::Vacant(e) = local.entry(p) {
            let l = bundle::local(client, cache, e.key()).await?;
            e.insert(l);
        }
    }
    crate::client::read_paths(&mut model.nodes, &mut |p| {
        if let Some(f) = local.get(p.as_str()) {
            *p = f.clone();
        }
    });
    if let Some(e) = eval {
        e.observed = bundle::local(client, cache, &e.observed).await?;
    }
    for v in overrides.values_mut() {
        if let serde_json::Value::String(p) = v
            && p.starts_with("bundle:")
        {
            *p = bundle::local(client, cache, p).await?;
        }
    }
    Ok(())
}

/// The operators that write files: in a batch, each run writes them in a folder of its own.
pub const WRITE_OPS: [&str; 3] = ["raster.write", "vector.write", "landscape.mspa_tiled"];

/// The files the nodes write put in `dir`, by their names (a name taken already gets the node's id before it).
pub fn write_into(nodes: &mut [terraflux_graph::Node], dir: &Path, taken: &mut std::collections::BTreeSet<String>) {
    for n in nodes {
        if WRITE_OPS.contains(&n.op.as_str())
            && let Some(serde_json::Value::String(p)) = n.params.get_mut("path")
        {
            let name = Path::new(p.as_str()).file_name().map(|f| f.to_string_lossy().into_owned());
            let mut name = name.unwrap_or_else(|| format!("{}.tif", n.id));
            if !taken.insert(name.clone()) {
                name = format!("{}_{name}", n.id);
                taken.insert(name.clone());
            }
            *p = dir.join(name).to_string_lossy().into_owned();
        }
        if let Some(b) = &mut n.body {
            write_into(&mut b.nodes, dir, taken);
        }
    }
}

/// Every file under a folder, named by its path in it (with `/`).
fn files_under(dir: &Path, prefix: &str, out: &mut Vec<File>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = format!("{prefix}{}", e.file_name().to_string_lossy());
        if e.path().is_dir() {
            files_under(&e.path(), &format!("{name}/"), out)?;
        } else {
            out.push(File { name, data: std::fs::read(e.path())? });
        }
    }
    Ok(())
}

/// Run one task (blocking) in the folder `dir`: the model with its overrides, the files its nodes write put in
/// the folder, then the evaluation, and the outputs' files and the nodes' (when `keep`).
pub fn run_task(
    mut model: Model,
    overrides: &BTreeMap<String, serde_json::Value>,
    eval: Option<&Evaluation>,
    dir: &Path,
    keep: bool,
) -> Result<(String, Vec<File>, Vec<String>)> {
    model.apply(overrides).map_err(|e| Error::Other(e.to_string()))?;
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir)?;
    write_into(&mut model.nodes, dir, &mut Default::default());
    let r = (|| {
        let inputs = load_inputs(&model, Path::new(""), &BTreeMap::new()).map_err(|e| Error::Other(e.to_string()))?;
        let results = Engine::default().run(&model, &inputs, &mut Quiet).map_err(|e| Error::Other(e.to_string()))?;
        let metrics = match eval {
            Some(e) => serde_json::to_string(&evaluate(e, &results, &inputs)?)?,
            None => String::new(),
        };
        let mut out = Vec::new();
        if keep {
            write_outputs(&model, &results, dir).map_err(|e| Error::Other(e.to_string()))?;
            files_under(dir, "", &mut out)?;
        }
        Ok((metrics, out, results.warnings))
    })();
    let _ = std::fs::remove_dir_all(dir);
    r
}

/// What a worker is doing, for whoever shows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Status {
    /// The coordinator it works for (its address), if connected.
    pub coordinator: Option<String>,
    /// The task it runs now.
    pub task: Option<String>,
    /// The tasks it did.
    pub done: u32,
    /// Why it does not work, if it does not (looking for its coordinator, another version...).
    pub note: Option<String>,
}

fn tell(opts: &Options, f: impl FnOnce(&mut Status)) {
    if let Some(s) = &opts.status {
        f(&mut s.lock().unwrap_or_else(|e| e.into_inner()));
    }
}

/// Ask for tasks and run them until `stop`; reconnects when the coordinator goes away.
pub async fn run(url: &str, opts: Options, stop: Arc<AtomicBool>) -> Result<()> {
    let mut told = String::new();
    while !stop.load(Ordering::Relaxed) {
        if let Ended::Refused(why) = serve(url, &opts, &stop).await
            && told != why
        {
            eprintln!("worker {}: {why}", opts.name);
            told = why;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Ok(())
}

/// Find the coordinator of a pairing code on the local network (its beacons heard at `on`) and work for it
/// until `stop`; found again when it goes away.
pub async fn run_found(code: Code, mut opts: Options, stop: Arc<AtomicBool>, on: std::net::SocketAddr) -> Result<()> {
    opts.code = Some(code.clone());
    while !stop.load(Ordering::Relaxed) {
        tell(&opts, |s| {
            s.coordinator = None;
            s.note = Some("looking for the computer of this code on the network".into());
        });
        let found = match crate::discover::find(&code, Duration::from_secs(3), on).await {
            Ok(f) => f,
            Err(e) => {
                tell(&opts, |s| s.note = Some(e.to_string()));
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        };
        let Some(f) = found.iter().find(|f| f.version == crate::coordinator::VERSION) else {
            if let Some(f) = found.first() {
                let why = format!(
                    "{} has Terraflux {}, this computer {}: install the same version on both",
                    f.name,
                    f.version,
                    crate::coordinator::VERSION
                );
                tell(&opts, |s| s.note = Some(why));
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            continue;
        };
        if let Ended::Refused(why) = serve(&f.addr, &opts, &stop).await {
            tell(&opts, |s| s.note = Some(why));
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
    tell(&opts, |s| *s = Status::default());
    Ok(())
}

/// How a worker's time with a coordinator ended.
enum Ended {
    /// It could not be reached, or went away.
    Lost,
    /// It turned the worker away: why.
    Refused(String),
    Stopped,
}

/// Work for a coordinator until it goes away, turns the worker away, or `stop`.
async fn serve(url: &str, opts: &Options, stop: &AtomicBool) -> Ended {
    let threads = opts.threads as u32;
    let mut client = match connect(url, opts.code.as_ref()).await {
        Ok(c) => c,
        Err(e) => {
            let why = e.to_string();
            return if why.contains("does not know this code") { Ended::Refused(why) } else { Ended::Lost };
        }
    };
    let hello = WorkerHello { name: opts.name.clone(), threads, version: crate::coordinator::VERSION.into() };
    let id = match client.register(hello).await {
        Ok(r) => r.into_inner().id,
        Err(e) if matches!(e.code(), tonic::Code::FailedPrecondition | tonic::Code::Unauthenticated) => {
            return Ended::Refused(e.message().to_string());
        }
        Err(_) => return Ended::Lost,
    };
    tell(opts, |s| {
        s.coordinator = Some(crate::net::host_port(url));
        s.note = None;
    });
    while !stop.load(Ordering::Relaxed) {
        if opts.waiting() {
            tell(opts, |s| s.note = Some("waiting: paused, or someone at this computer".into()));
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        let task = match client.next_task(TaskRequest { worker: id.clone(), wait_ms: 1000 }).await {
            Ok(t) => t.into_inner(),
            Err(_) => return Ended::Lost, // register again
        };
        tell(opts, |s| s.note = None);
        if task.task_id.is_empty() {
            continue;
        }
        tell(opts, |s| s.task = Some(task.task_id.clone()));
        // the computer kept awake while it works
        let _awake = crate::awake::Awake::hold();
        let t0 = Instant::now();
        let (result, payload) = if task.kind == "tiles" || task.kind == crate::tiles::SENT {
            let r = if task.kind == "tiles" {
                tiles(&mut client, opts, &id, &task).await
            } else {
                tiles_sent(&mut client, opts, &id, &task).await
            };
            match r {
                Ok(p) => (Ok((String::new(), Vec::new(), Vec::new())), p),
                Err(e) => (Err(e), Vec::new()),
            }
        } else {
            (work(&mut client, opts, &id, &task).await, Vec::new())
        };
        let (ok, mut error, metrics, written, warnings) = match result {
            Ok((m, f, w)) => (true, String::new(), m, f, w),
            Err(e) => (false, e.to_string(), String::new(), Vec::new(), Vec::new()),
        };
        // the files go as blobs, the result names them
        let mut files = Vec::new();
        let mut sent = ok;
        for f in written {
            let size = f.data.len() as u64;
            match bundle::send_bytes(&mut client, f.data).await {
                Ok(hash) => files.push(OutputFile { name: f.name, hash, size }),
                Err(e) => {
                    sent = false;
                    error = format!("sending {}: {e}", f.name);
                    break;
                }
            }
        }
        let r = TaskResult {
            task_id: task.task_id.clone(),
            worker: id.clone(),
            ok: ok && sent,
            error,
            metrics_json: metrics,
            files: if sent { files } else { Vec::new() },
            seconds: t0.elapsed().as_secs_f64(),
            warnings,
            label: String::new(),
            payload,
        };
        tell(opts, |s| {
            s.task = None;
            s.done += 1;
        });
        if client.complete(r).await.is_err() {
            return Ended::Lost;
        }
    }
    Ended::Stopped
}

/// Heartbeats to the coordinator until the returned flag is set.
fn heartbeats(client: &Conn, id: &str, every: Duration) -> (Arc<AtomicBool>, tokio::task::JoinHandle<()>) {
    let done = Arc::new(AtomicBool::new(false));
    let (d, mut hb, wid) = (done.clone(), client.clone(), id.to_string());
    let beat = tokio::spawn(async move {
        while !d.load(Ordering::Relaxed) {
            tokio::time::sleep(every).await;
            let _ = hb.heartbeat(WorkerId { id: wid.clone() }).await;
        }
    });
    (done, beat)
}

/// A pass of the tiled engine on some tiles (the maps on shared storage): its part, encoded.
async fn tiles(client: &mut Conn, opts: &Options, id: &str, task: &TaskAssignment) -> Result<Vec<u8>> {
    let (done, beat) = heartbeats(client, id, opts.heartbeat);
    let (job, chosen, threads) = (task.payload.clone(), task.task_payload.clone(), opts.threads);
    let r = tokio::task::spawn_blocking(move || {
        terraflux_ops::threads::ensure();
        terraflux_ops::threads::set_kernel_threads(threads);
        terraflux_engine::pass::run_encoded(&job, &chosen).map_err(|e| Error::Other(e.to_string()))
    })
    .await
    .map_err(|e| Error::Other(format!("the task stopped: {e}")))?;
    done.store(true, Ordering::Relaxed);
    beat.abort();
    r
}

/// A tile of a layer in a worker's folder of the layer, fetched by its hash unless it is there (a `<i>.hash`
/// beside it says which it is).
async fn tile_here(client: &mut Conn, store: &crate::store::Store, dir: &Path, i: usize, hash: &str) -> Result<()> {
    let mark = dir.join(format!("{i}.hash"));
    if std::fs::read_to_string(&mark).is_ok_and(|h| h == hash) && dir.join(format!("{i}.bin")).exists() {
        return Ok(());
    }
    let blob = bundle::fetch(client, store, hash).await?;
    let tmp = dir.join(format!("{i}.{}-{}.part", std::process::id(), crate::unique()));
    std::fs::copy(&blob, &tmp)?;
    std::fs::rename(&tmp, dir.join(format!("{i}.bin")))?;
    std::fs::write(mark, hash)?;
    Ok(())
}

/// A pass of the tiled engine whose maps and layers come through the coordinator: the job's files from the
/// cache, the layers' tiles of these tiles and their neighbours fetched into folders of the cache, the pass run,
/// and the tiles it wrote sent back.
async fn tiles_sent(client: &mut Conn, opts: &Options, id: &str, task: &TaskAssignment) -> Result<Vec<u8>> {
    use terraflux_engine::pass::{Job, decode, encode};
    use terraflux_engine::tiles::Spec;
    let e = |x: terraflux_engine::Error| Error::Other(x.to_string());
    let sent: crate::tiles::SentJob = decode(&task.payload).map_err(e)?;
    let mut job: Job = decode(&sent.job).map_err(e)?;
    let chosen: Vec<usize> = decode(&task.task_payload).map_err(e)?;
    let store = crate::store::Store::open(opts.cache.join("blobs"))?;
    let tiling = job.tiling();
    let near = crate::tiles::around(&chosen, &tiling, 1);
    let mut layers = sent.layers.iter();
    let mut dirs: Vec<(String, PathBuf, BTreeMap<usize, String>)> = Vec::new();
    for s in &mut job.sources {
        match s {
            Spec::File { path, .. } => {
                *path = PathBuf::from(bundle::local(client, &opts.cache, &path.to_string_lossy()).await?);
            }
            Spec::Layer { dir, .. } => {
                let l = layers.next().ok_or_else(|| Error::Other("a layer of the pass was not listed".into()))?;
                // a folder of this worker's (workers may share a cache), with no tile the layer does not have
                let mine = crate::hash(opts.name.as_bytes());
                let here = opts.cache.join("layers").join(&mine[..12]).join(&crate::hash(l.key.as_bytes())[..20]);
                std::fs::create_dir_all(&here)?;
                let tiles: BTreeMap<usize, String> = l.tiles.iter().cloned().collect();
                for (i, f) in crate::tiles::tile_files(&here) {
                    if !tiles.contains_key(&i) {
                        let _ = std::fs::remove_file(f);
                        let _ = std::fs::remove_file(here.join(format!("{i}.hash")));
                    }
                }
                for i in &near {
                    if let Some(h) = tiles.get(i) {
                        tile_here(client, &store, &here, *i, h).await?;
                    }
                }
                *dir = here.clone();
                dirs.push((l.key.clone(), here, tiles));
            }
        }
    }
    let (done, beat) = heartbeats(client, id, opts.heartbeat);
    let threads = opts.threads;
    let run = |job: Job, chosen: Vec<usize>| {
        tokio::task::spawn_blocking(move || {
            terraflux_ops::threads::ensure();
            terraflux_ops::threads::set_kernel_threads(threads);
            job.run(&chosen)
        })
    };
    let mut part =
        run(job.clone(), chosen.clone()).await.map_err(|x| Error::Other(format!("the task stopped: {x}")))?;
    // a halo wider than a tile: every tile of the layers, and again
    if let Err(x) = &part
        && x.to_string().contains("was not computed")
    {
        for (_, here, tiles) in &dirs {
            for (i, h) in tiles {
                tile_here(client, &store, here, *i, h).await?;
            }
        }
        part = run(job, chosen).await.map_err(|x| Error::Other(format!("the task stopped: {x}")))?;
    }
    let part = part.map_err(e)?;
    // the tiles written here: those without the mark of a tile fetched
    let mut written = Vec::new();
    for (key, here, _) in &dirs {
        for (i, f) in crate::tiles::tile_files(here) {
            let mark = here.join(format!("{i}.hash"));
            if mark.exists() {
                continue;
            }
            let h = bundle::send_file(client, &f).await?;
            std::fs::write(&mark, &h)?;
            written.push((key.clone(), i, h));
        }
    }
    done.store(true, Ordering::Relaxed);
    beat.abort();
    encode(&crate::tiles::SentPart { part: encode(&part).map_err(e)?, written }).map_err(e)
}

async fn work(
    client: &mut Conn,
    opts: &Options,
    id: &str,
    task: &TaskAssignment,
) -> Result<(String, Vec<File>, Vec<String>)> {
    let mut model = Model::from_json(&task.model_json).map_err(|e| Error::Other(e.to_string()))?;
    let mut overrides: BTreeMap<String, serde_json::Value> =
        if task.overrides_json.is_empty() { BTreeMap::new() } else { serde_json::from_str(&task.overrides_json)? };
    let mut eval: Option<Evaluation> =
        if task.evaluation_json.is_empty() { None } else { Some(serde_json::from_str(&task.evaluation_json)?) };
    localize(client, &opts.cache, &mut model, eval.as_mut(), &mut overrides).await?;
    // heartbeats while the task runs
    let (done, beat) = heartbeats(client, id, opts.heartbeat);
    let dir = opts.cache.join("out").join(format!("{}-{}", opts.name, task.task_id.replace('/', "_")));
    let (threads, keep) = (opts.threads, task.return_files);
    let r = tokio::task::spawn_blocking(move || {
        terraflux_ops::threads::ensure();
        terraflux_ops::threads::set_kernel_threads(threads);
        run_task(model, &overrides, eval.as_ref(), &dir, keep)
    })
    .await
    .map_err(|e| Error::Other(format!("the task stopped: {e}")))?;
    done.store(true, Ordering::Relaxed);
    beat.abort();
    r
}
