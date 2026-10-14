//! A batch from start to end: where it runs (workers on this machine, or a coordinator's), the model's data sent,
//! the job submitted and followed, its results. The command line and the application run batches with it; the
//! application in a thread of its own ([`spawn`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use terraflux_graph::Model;

use crate::client::{Client, TaskOf};
use crate::coordinator::Options;
use crate::evaluate::Evaluation;
use crate::proto::{JobStatus, TaskResult};
use crate::{Error, Local, Result};

/// Where a batch runs.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    /// A coordinator and `workers` workers started here for the batch (their data kept in `cache`).
    Here { workers: usize, cache: PathBuf },
    /// A coordinator's workers (`http://host:7700`).
    At(String),
}

/// A batch: a model (its paths relative to `base`) run once per task.
#[derive(Debug, Clone)]
pub struct Batch {
    pub name: String,
    pub model: Model,
    pub base: PathBuf,
    pub tasks: Vec<TaskOf>,
    /// Whether the tasks' files come back (else only their metrics).
    pub files: bool,
    /// A calibration's evaluation, its observed map a file here (sent with the model's).
    pub evaluation: Option<Evaluation>,
}

/// How a batch goes.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    /// Workers started on this machine for it.
    Workers(usize),
    /// The model's data is being sent.
    Sending,
    /// The job was submitted.
    Submitted { job: String, total: usize },
    /// Tasks done, running, failed.
    Status(JobStatus),
}

/// What a batch gave.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub status: JobStatus,
    /// The results of the tasks, in their order.
    pub results: Vec<TaskResult>,
}

/// The message of a batch stopped by hand.
pub const STOPPED: &str = "stopped";

/// Run a batch where it is placed, telling `progress` along the way; `stop` drops it.
pub async fn run(b: &Batch, place: &Place, stop: &AtomicBool, progress: &mut dyn FnMut(Progress)) -> Result<Outcome> {
    let (url, local) = match place {
        Place::At(u) => (u.clone(), None),
        Place::Here { workers, cache } => {
            let local = Local::start(*workers, cache.clone(), Options::default()).await?;
            progress(Progress::Workers(*workers));
            (local.url.clone(), Some(local))
        }
    };
    let r = follow(b, &url, stop, progress).await;
    if let Some(l) = local {
        l.shutdown();
    }
    r
}

async fn follow(b: &Batch, url: &str, stop: &AtomicBool, progress: &mut dyn FnMut(Progress)) -> Result<Outcome> {
    let mut c = Client::connect(url).await?;
    progress(Progress::Sending);
    let bundled = c.bundle(&b.model, &b.base).await?;
    let eval = match &b.evaluation {
        Some(e) => {
            let mut e = e.clone();
            let p = Path::new(&e.observed);
            let full = if p.is_absolute() { p.to_path_buf() } else { b.base.join(p) };
            e.observed = c.upload(&full).await?;
            Some(e)
        }
        None => None,
    };
    let job = c.submit(&b.name, &bundled, &b.tasks, b.files, eval.as_ref()).await?;
    progress(Progress::Submitted { job: job.clone(), total: b.tasks.len() });
    let mut last = None;
    let status = loop {
        if stop.load(Ordering::Relaxed) {
            let _ = c.forget(&job).await;
            return Err(Error::Other(STOPPED.into()));
        }
        let s = c.status(&job).await?;
        if last.as_ref() != Some(&(s.done, s.failed, s.running)) {
            last = Some((s.done, s.failed, s.running));
            progress(Progress::Status(s.clone()));
        }
        if s.done + s.failed == s.total {
            break s;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    };
    let results = c.results(&job).await?;
    let _ = c.forget(&job).await;
    Ok(Outcome { status, results })
}

/// Write each task's files in `out/<label>/` and `out/tasks.csv` (task, worker, seconds, ok, error).
pub fn write_results(out: &Path, results: &[TaskResult]) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut summary = String::from("task,worker,seconds,ok,error\n");
    for r in results {
        let dir = out.join(&r.label);
        for f in &r.files {
            let path = dir.join(&f.name);
            if let Some(d) = path.parent() {
                std::fs::create_dir_all(d)?;
            }
            std::fs::write(path, &f.data)?;
        }
        summary.push_str(&format!("{},{},{:.2},{},{:?}\n", r.label, r.worker, r.seconds, r.ok, r.error));
    }
    std::fs::write(out.join("tasks.csv"), summary)?;
    Ok(())
}

/// What a batch run in a thread sends.
#[derive(Debug)]
pub enum Event {
    Progress(Progress),
    Done(std::result::Result<Outcome, String>),
}

/// A batch running in a thread of its own.
pub struct Handle {
    pub rx: Receiver<Event>,
    stop: Arc<AtomicBool>,
}

impl Handle {
    /// Drop the batch (its tasks left undone).
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Run a batch in a thread of its own (with its own async runtime); `wake` is called after every event. With
/// `write_to`, the tasks' files are written there ([`write_results`]) before the batch is done, and only their
/// names kept in the outcome.
pub fn spawn(b: Batch, place: Place, write_to: Option<PathBuf>, wake: Box<dyn Fn() + Send>) -> Handle {
    let (tx, rx) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(Event::Done(Err(e.to_string())));
                wake();
                return;
            }
        };
        let r = rt.block_on(async {
            let mut tell = |p: Progress| {
                let _ = tx.send(Event::Progress(p));
                wake();
            };
            let mut o = run(&b, &place, &s, &mut tell).await?;
            if let Some(out) = &write_to {
                write_results(out, &o.results)?;
                for r in &mut o.results {
                    for f in &mut r.files {
                        f.data = Vec::new();
                    }
                }
            }
            Ok::<_, Error>(o)
        });
        let _ = tx.send(Event::Done(r.map_err(|e| e.to_string())));
        wake();
        rt.shutdown_timeout(Duration::from_secs(2));
    });
    Handle { rx, stop }
}
