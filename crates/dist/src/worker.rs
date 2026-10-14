//! A headless worker: it asks the coordinator for tasks, fetches their data once (a cache by hash), runs the
//! model with the task's overrides (heartbeats while it runs) and sends back the files or the metrics.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use terraflux_exec::{Engine, Quiet, load_inputs, write_outputs};
use terraflux_graph::{Input, Model};
use tonic::transport::Channel;

use crate::coordinator::MAX_MESSAGE;
use crate::evaluate::{Evaluation, evaluate};
use crate::proto::coordinator_client::CoordinatorClient;
use crate::proto::*;
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct Options {
    pub name: String,
    /// Where the data of the tasks is kept (by hash) and their outputs written.
    pub cache: PathBuf,
    /// Seconds between heartbeats while a task runs.
    pub heartbeat: Duration,
    /// The kernels' threads of each task (never every core).
    pub threads: usize,
}

impl Options {
    pub fn new(name: &str, cache: impl Into<PathBuf>) -> Self {
        Options {
            name: name.to_string(),
            cache: cache.into(),
            heartbeat: Duration::from_secs(5),
            threads: terraflux_ops::threads::default_threads(),
        }
    }
}

pub async fn connect(url: &str) -> Result<CoordinatorClient<Channel>> {
    let c = CoordinatorClient::connect(url.to_string()).await?;
    Ok(c.max_decoding_message_size(MAX_MESSAGE).max_encoding_message_size(MAX_MESSAGE))
}

/// The local file of a blob, fetched when the cache does not have it.
async fn blob(client: &mut CoordinatorClient<Channel>, cache: &Path, hash: &str) -> Result<PathBuf> {
    let path = cache.join("blobs").join(format!("{hash}.tif"));
    if path.exists() {
        return Ok(path);
    }
    let b = client.get_blob(BlobId { hash: hash.to_string() }).await?.into_inner();
    if crate::hash(&b.data) != hash {
        return Err(Error::Other(format!("the blob {hash} arrived damaged")));
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    // workers may share a cache (several in one process, or on one disk): a file of their own, then renamed
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("{}-{n}.part", std::process::id()));
    std::fs::write(&tmp, &b.data)?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        if !path.exists() {
            return Err(e.into());
        }
    }
    Ok(path)
}

/// The paths `blob:<hash>` of a model (its inputs and an evaluation's observed map) as local files.
pub async fn localize(
    client: &mut CoordinatorClient<Channel>,
    cache: &Path,
    model: &mut Model,
    eval: Option<&mut Evaluation>,
) -> Result<()> {
    for input in model.inputs.values_mut() {
        if let Input::Map { path, .. } | Input::Stack { path, .. } = input
            && let Some(h) = path.strip_prefix("blob:")
        {
            *path = blob(client, cache, h).await?.to_string_lossy().into_owned();
        }
    }
    let mut hashes = Vec::new();
    crate::client::plugin_paths(&mut model.nodes, &mut |p| {
        if let Some(h) = p.strip_prefix("blob:") {
            hashes.push(h.to_string());
        }
    });
    let mut local = std::collections::BTreeMap::new();
    for h in hashes {
        let f = blob(client, cache, &h).await?;
        local.insert(format!("blob:{h}"), f.to_string_lossy().into_owned());
    }
    crate::client::plugin_paths(&mut model.nodes, &mut |p| {
        if let Some(f) = local.get(p.as_str()) {
            *p = f.clone();
        }
    });
    if let Some(e) = eval
        && let Some(h) = e.observed.strip_prefix("blob:")
    {
        e.observed = blob(client, cache, h).await?.to_string_lossy().into_owned();
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
fn files_under(dir: &Path, prefix: &str, out: &mut Vec<OutputFile>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = format!("{prefix}{}", e.file_name().to_string_lossy());
        if e.path().is_dir() {
            files_under(&e.path(), &format!("{name}/"), out)?;
        } else {
            out.push(OutputFile { name, data: std::fs::read(e.path())? });
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
) -> Result<(String, Vec<OutputFile>, Vec<String>)> {
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

/// Ask for tasks and run them until `stop`; reconnects when the coordinator goes away.
pub async fn run(url: &str, opts: Options, stop: Arc<AtomicBool>) -> Result<()> {
    let threads = opts.threads as u32;
    while !stop.load(Ordering::Relaxed) {
        let mut client = match connect(url).await {
            Ok(c) => c,
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };
        let id = match client
            .register(WorkerHello { name: opts.name.clone(), threads, version: env!("CARGO_PKG_VERSION").into() })
            .await
        {
            Ok(r) => r.into_inner().id,
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };
        while !stop.load(Ordering::Relaxed) {
            let task = match client.next_task(TaskRequest { worker: id.clone(), wait_ms: 1000 }).await {
                Ok(t) => t.into_inner(),
                Err(_) => break, // register again
            };
            if task.task_id.is_empty() {
                continue;
            }
            let t0 = Instant::now();
            let (result, payload) = if task.kind == "tiles" {
                match tiles(&mut client, &opts, &id, &task).await {
                    Ok(p) => (Ok((String::new(), Vec::new(), Vec::new())), p),
                    Err(e) => (Err(e), Vec::new()),
                }
            } else {
                (work(&mut client, &opts, &id, &task).await, Vec::new())
            };
            let (ok, error, metrics, files, warnings) = match result {
                Ok((m, f, w)) => (true, String::new(), m, f, w),
                Err(e) => (false, e.to_string(), String::new(), Vec::new(), Vec::new()),
            };
            let r = TaskResult {
                task_id: task.task_id.clone(),
                worker: id.clone(),
                ok,
                error,
                metrics_json: metrics,
                files,
                seconds: t0.elapsed().as_secs_f64(),
                warnings,
                label: String::new(),
                payload,
            };
            if client.complete(r).await.is_err() {
                break;
            }
        }
    }
    Ok(())
}

/// Heartbeats to the coordinator until the returned flag is set.
fn heartbeats(
    client: &CoordinatorClient<Channel>,
    id: &str,
    every: Duration,
) -> (Arc<AtomicBool>, tokio::task::JoinHandle<()>) {
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
async fn tiles(
    client: &mut CoordinatorClient<Channel>,
    opts: &Options,
    id: &str,
    task: &TaskAssignment,
) -> Result<Vec<u8>> {
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

async fn work(
    client: &mut CoordinatorClient<Channel>,
    opts: &Options,
    id: &str,
    task: &TaskAssignment,
) -> Result<(String, Vec<OutputFile>, Vec<String>)> {
    let mut model = Model::from_json(&task.model_json).map_err(|e| Error::Other(e.to_string()))?;
    let overrides: BTreeMap<String, serde_json::Value> =
        if task.overrides_json.is_empty() { BTreeMap::new() } else { serde_json::from_str(&task.overrides_json)? };
    let mut eval: Option<Evaluation> =
        if task.evaluation_json.is_empty() { None } else { Some(serde_json::from_str(&task.evaluation_json)?) };
    localize(client, &opts.cache, &mut model, eval.as_mut()).await?;
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
