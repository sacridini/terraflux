//! A batch from start to end: where it runs (workers on this machine, or a coordinator's), the model's data sent,
//! the job submitted and followed, the files of its runs fetched as they are done, its results. A batch stopped
//! half way (the computer turned off, the application closed) is taken up again by its job ([`Start::Resume`]):
//! the coordinator kept it on disk, the runs done are not run again and their files not fetched again. The
//! command line and the application run batches with it; the application in a thread of its own ([`spawn`]).

use std::collections::BTreeSet;
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
    /// A coordinator and `workers` workers started here for the batch, their data kept in `cache`; the
    /// coordinator's jobs kept in `state` (none: let go when it stops; a folder: the batch may be taken up again).
    Here { workers: usize, cache: PathBuf, state: Option<PathBuf> },
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

/// A batch to start, or one to take up again (its job on the coordinator).
#[derive(Debug, Clone)]
pub enum Start {
    New(Box<Batch>),
    Resume(String),
}

/// How a batch goes.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    /// Workers started on this machine for it.
    Workers(usize),
    /// The model's data is being sent.
    Sending,
    /// The job was submitted (or found again).
    Submitted { job: String, total: usize },
    /// Tasks done, running, failed.
    Status(JobStatus),
}

/// What a batch gave.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub job: String,
    pub status: JobStatus,
    /// The results of the tasks, in their order (their files written in the folder given, if any).
    pub results: Vec<TaskResult>,
}

/// The message of a batch stopped by hand.
pub const STOPPED: &str = "stopped";

/// Run a batch where it is placed (or take one up again), telling `progress` along the way; the files of its
/// runs written to `out/<run>/` as they are done; `stop` drops it.
pub async fn run(
    start: &Start,
    place: &Place,
    out: Option<&Path>,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Outcome> {
    let (url, local) = match place {
        Place::At(u) => (u.clone(), None),
        Place::Here { workers, cache, state } => {
            let options = Options { dir: state.clone(), ..Default::default() };
            let local = Local::start(*workers, cache.clone(), options).await?;
            progress(Progress::Workers(*workers));
            (local.url.clone(), Some(local))
        }
    };
    let r = async {
        let mut c = Client::connect(&url).await?;
        let job = match start {
            Start::New(b) => submit(&mut c, b, progress).await?,
            Start::Resume(job) => job.clone(),
        };
        follow(&mut c, &job, out, stop, progress).await
    }
    .await;
    if let Some(l) = local {
        l.shutdown();
    }
    r
}

/// Send a batch's data and submit it: its job.
pub async fn submit(c: &mut Client, b: &Batch, progress: &mut dyn FnMut(Progress)) -> Result<String> {
    progress(Progress::Sending);
    let bundled = c.bundle(&b.model, &b.base).await?;
    let tasks = c.bundle_tasks(&b.model, &b.base, &b.tasks).await?;
    let eval = match &b.evaluation {
        Some(e) => {
            let mut e = e.clone();
            e.observed = c.send(&e.observed, &b.base).await?;
            Some(e)
        }
        None => None,
    };
    let job = c.submit(&b.name, &bundled, &tasks, b.files, eval.as_ref()).await?;
    progress(Progress::Submitted { job: job.clone(), total: tasks.len() });
    Ok(job)
}

/// Follow a job to its end, the files of its runs fetched to `out` as they are done; then forget it.
pub async fn follow(
    c: &mut Client,
    job: &str,
    out: Option<&Path>,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Outcome> {
    let mut last = None;
    let mut fetched = BTreeSet::new();
    let mut announced = false;
    let status = loop {
        if stop.load(Ordering::Relaxed) {
            let _ = c.forget(job).await;
            return Err(Error::Other(STOPPED.into()));
        }
        let s = c.status(job).await?;
        if !announced {
            progress(Progress::Submitted { job: job.to_string(), total: s.total as usize });
            announced = true;
        }
        let key = (s.done, s.failed, s.running);
        if last != Some(key) {
            last = Some(key);
            if let Some(out) = out {
                fetch_new(c, job, out, &mut fetched).await?;
            }
            progress(Progress::Status(s.clone()));
        }
        if s.done + s.failed == s.total {
            break s;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    };
    let results = c.results(job).await?;
    if let Some(out) = out {
        fetch_new(c, job, out, &mut fetched).await?;
        write_summary(out, &results)?;
    }
    let _ = c.forget(job).await;
    Ok(Outcome { job: job.to_string(), status, results })
}

/// The files of the runs done since the last time, written to `out/<run>/` (those there already, of their size,
/// left as they are).
async fn fetch_new(c: &mut Client, job: &str, out: &Path, fetched: &mut BTreeSet<String>) -> Result<()> {
    for r in c.results(job).await? {
        if !r.ok || fetched.contains(&r.task_id) {
            continue;
        }
        for f in &r.files {
            let dest = out.join(&r.label).join(&f.name);
            if std::fs::metadata(&dest).is_ok_and(|m| m.len() == f.size) {
                continue;
            }
            c.download(&f.hash, &dest).await?;
        }
        fetched.insert(r.task_id.clone());
    }
    Ok(())
}

/// `out/tasks.csv`: each task's worker, seconds, whether it went well and its error.
pub fn write_summary(out: &Path, results: &[TaskResult]) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut summary = String::from("task,worker,seconds,ok,error\n");
    for r in results {
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

/// Run a batch in a thread of its own (with its own async runtime); `wake` is called after every event. The
/// thread ends with the process: a batch placed here with a folder for its jobs may then be taken up again.
pub fn spawn(start: Start, place: Place, out: Option<PathBuf>, wake: Box<dyn Fn() + Send>) -> Handle {
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
            run(&start, &place, out.as_deref(), &s, &mut tell).await
        });
        let _ = tx.send(Event::Done(r.map_err(|e| e.to_string())));
        wake();
        rt.shutdown_timeout(Duration::from_secs(2));
    });
    Handle { rx, stop }
}
