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
}

impl Options {
    pub fn new(name: &str, cache: impl Into<PathBuf>) -> Self {
        Options { name: name.to_string(), cache: cache.into(), heartbeat: Duration::from_secs(5) }
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
    if let Some(e) = eval
        && let Some(h) = e.observed.strip_prefix("blob:")
    {
        e.observed = blob(client, cache, h).await?.to_string_lossy().into_owned();
    }
    Ok(())
}

/// Run one task (blocking): the model with its overrides, then the evaluation or the outputs' files.
pub fn run_task(
    mut model: Model,
    overrides: &BTreeMap<String, serde_json::Value>,
    eval: Option<&Evaluation>,
    files: Option<&Path>,
) -> Result<(String, Vec<OutputFile>, Vec<String>)> {
    model.apply(overrides).map_err(|e| Error::Other(e.to_string()))?;
    let inputs = load_inputs(&model, Path::new(""), &BTreeMap::new()).map_err(|e| Error::Other(e.to_string()))?;
    let results = Engine::default().run(&model, &inputs, &mut Quiet).map_err(|e| Error::Other(e.to_string()))?;
    let metrics = match eval {
        Some(e) => serde_json::to_string(&evaluate(e, &results, &inputs)?)?,
        None => String::new(),
    };
    let mut out = Vec::new();
    if let Some(dir) = files {
        std::fs::create_dir_all(dir)?;
        for f in write_outputs(&model, &results, dir).map_err(|e| Error::Other(e.to_string()))? {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            out.push(OutputFile { name, data: std::fs::read(&f)? });
        }
        let _ = std::fs::remove_dir_all(dir);
    }
    Ok((metrics, out, results.warnings))
}

/// Ask for tasks and run them until `stop`; reconnects when the coordinator goes away.
pub async fn run(url: &str, opts: Options, stop: Arc<AtomicBool>) -> Result<()> {
    let threads = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1);
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
            let result = work(&mut client, &opts, &id, &task).await;
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
            };
            if client.complete(r).await.is_err() {
                break;
            }
        }
    }
    Ok(())
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
    let done = Arc::new(AtomicBool::new(false));
    let (d, mut hb, wid, every) = (done.clone(), client.clone(), id.to_string(), opts.heartbeat);
    let beat = tokio::spawn(async move {
        while !d.load(Ordering::Relaxed) {
            tokio::time::sleep(every).await;
            let _ = hb.heartbeat(WorkerId { id: wid.clone() }).await;
        }
    });
    let files = task.return_files.then(|| opts.cache.join("out").join(task.task_id.replace('/', "_")));
    let r = tokio::task::spawn_blocking(move || run_task(model, &overrides, eval.as_ref(), files.as_deref()))
        .await
        .map_err(|e| Error::Other(format!("the task stopped: {e}")))?;
    done.store(true, Ordering::Relaxed);
    beat.abort();
    r
}
