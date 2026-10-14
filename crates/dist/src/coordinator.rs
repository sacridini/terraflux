//! The coordinator: the queue of the tasks of the jobs submitted, the data they need and the files of their
//! results (blobs by hash, on disk), the workers and what each is doing. A task whose worker stops sending
//! heartbeats goes back to the queue. Its jobs are kept on disk as they change: a coordinator stopped and started
//! again on the same folder goes on with them (the tasks that were running run again).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use prost::Message;
use tokio::sync::Notify;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming};

use crate::proto::coordinator_server::{Coordinator, CoordinatorServer};
use crate::proto::*;
use crate::store::{Store, file_chunks};

/// The largest message (a task's payload, a job's model, a part of the tiled engine).
pub const MAX_MESSAGE: usize = 1 << 30;

/// The version of Terraflux: workers and clients of another are turned away.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How the coordinator treats its workers, and where it keeps its jobs.
#[derive(Debug, Clone)]
pub struct Options {
    /// A running task goes back to the queue when its worker has not been heard of for this long.
    pub timeout: Duration,
    /// Times a task is tried before it fails.
    pub attempts: u32,
    /// Where the blobs and the jobs are kept (none: a folder of its own in the system's temporary folder, let go
    /// when the coordinator stops).
    pub dir: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Options { timeout: Duration::from_secs(60), attempts: 3, dir: None }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum State {
    Pending,
    Running { worker: String },
    Done,
    Failed,
}

struct Task {
    label: String,
    overrides_json: String,
    payload: Vec<u8>,
    state: State,
    attempts: u32,
    result: Option<TaskResult>,
}

struct Job {
    name: String,
    kind: String,
    payload: Arc<Vec<u8>>,
    model_json: Arc<String>,
    evaluation_json: String,
    return_files: bool,
    tasks: Vec<Task>,
    errors: Vec<String>,
    submitted: u64,
}

struct Worker {
    name: String,
    threads: u32,
    version: String,
    seen: Instant,
    done: u32,
    task: Option<(String, usize)>,
}

#[derive(Default)]
struct Inner {
    jobs: BTreeMap<String, Job>,
    queue: VecDeque<(String, usize)>,
    workers: BTreeMap<String, Worker>,
    next: u64,
    /// The jobs changed since they were last kept on disk.
    dirty: bool,
}

impl Inner {
    fn id(&mut self, prefix: &str) -> String {
        self.next += 1;
        format!("{prefix}-{}", self.next)
    }

    /// Put back in the queue the tasks of workers not heard of for `timeout`.
    fn reap(&mut self, timeout: Duration, attempts: u32) {
        let lost: Vec<String> = self
            .workers
            .iter()
            .filter(|(_, w)| w.seen.elapsed() > timeout && w.task.is_some())
            .map(|(k, _)| k.clone())
            .collect();
        for wid in lost {
            let Some((job, i)) = self.workers.get_mut(&wid).and_then(|w| w.task.take()) else { continue };
            let Some(j) = self.jobs.get_mut(&job) else { continue };
            let t = &mut j.tasks[i];
            if t.state == (State::Running { worker: wid.clone() }) {
                if t.attempts >= attempts {
                    t.state = State::Failed;
                    j.errors.push(format!("{}: its workers stopped answering ({} attempts)", t.label, t.attempts));
                } else {
                    t.state = State::Pending;
                    self.queue.push_front((job, i));
                }
                self.dirty = true;
            }
        }
        // workers gone for long are forgotten
        self.workers.retain(|_, w| w.task.is_some() || w.seen.elapsed() < timeout * 10);
    }

    fn saved(&self) -> SavedState {
        SavedState {
            next: self.next,
            jobs: self
                .jobs
                .iter()
                .map(|(id, j)| SavedJob {
                    id: id.clone(),
                    name: j.name.clone(),
                    kind: j.kind.clone(),
                    payload: (*j.payload).clone(),
                    model_json: (*j.model_json).clone(),
                    evaluation_json: j.evaluation_json.clone(),
                    return_files: j.return_files,
                    errors: j.errors.clone(),
                    submitted: j.submitted,
                    tasks: j
                        .tasks
                        .iter()
                        .map(|t| SavedTask {
                            label: t.label.clone(),
                            overrides_json: t.overrides_json.clone(),
                            payload: t.payload.clone(),
                            state: match t.state {
                                State::Pending | State::Running { .. } => 0,
                                State::Done => 1,
                                State::Failed => 2,
                            },
                            attempts: t.attempts,
                            result: t.result.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    fn restore(s: SavedState) -> Inner {
        let mut inner = Inner { next: s.next, ..Default::default() };
        for j in s.jobs {
            let tasks: Vec<Task> = j
                .tasks
                .into_iter()
                .map(|t| Task {
                    label: t.label,
                    overrides_json: t.overrides_json,
                    payload: t.payload,
                    state: match t.state {
                        1 => State::Done,
                        2 => State::Failed,
                        _ => State::Pending,
                    },
                    attempts: t.attempts,
                    result: t.result,
                })
                .collect();
            for (i, t) in tasks.iter().enumerate() {
                if t.state == State::Pending {
                    inner.queue.push_back((j.id.clone(), i));
                }
            }
            inner.jobs.insert(
                j.id,
                Job {
                    name: j.name,
                    kind: j.kind,
                    payload: Arc::new(j.payload),
                    model_json: Arc::new(j.model_json),
                    evaluation_json: j.evaluation_json,
                    return_files: j.return_files,
                    tasks,
                    errors: j.errors,
                    submitted: j.submitted,
                },
            );
        }
        inner
    }
}

/// The coordinator's service.
#[derive(Clone)]
pub struct Service {
    inner: Arc<Mutex<Inner>>,
    notify: Arc<Notify>,
    options: Options,
    store: Store,
    dir: PathBuf,
    /// The folder is the coordinator's own, let go when it stops.
    temporary: bool,
}

const STATE: &str = "state.pb";

impl Service {
    /// A coordinator on its folder: the jobs kept there taken up again.
    pub fn new(options: Options) -> crate::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let (dir, temporary) = match &options.dir {
            Some(d) => (d.clone(), false),
            None => (
                std::env::temp_dir().join(format!(
                    "terraflux-coordinator-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                )),
                true,
            ),
        };
        let store = Store::open(dir.join("blobs"))?;
        let inner = match std::fs::read(dir.join(STATE)) {
            Ok(bytes) => Inner::restore(
                SavedState::decode(bytes.as_slice())
                    .map_err(|e| crate::Error::Other(format!("{}: {e}", dir.join(STATE).display())))?,
            ),
            Err(_) => Inner::default(),
        };
        Ok(Service {
            inner: Arc::new(Mutex::new(inner)),
            notify: Arc::new(Notify::new()),
            options,
            store,
            dir,
            temporary,
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The folder of its blobs and jobs.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Check the workers now (also done every second by the server).
    pub fn reap(&self) {
        let (t, a) = (self.options.timeout, self.options.attempts);
        self.lock().reap(t, a);
        self.notify.notify_waiters();
    }

    /// Keep the jobs on disk if they changed (done every second by the server).
    pub fn save(&self) -> crate::Result<()> {
        let bytes = {
            let mut g = self.lock();
            if !g.dirty {
                return Ok(());
            }
            g.dirty = false;
            g.saved().encode_to_vec()
        };
        let tmp = self.dir.join(format!("{STATE}.part"));
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, self.dir.join(STATE))?;
        Ok(())
    }

    /// Stop: the jobs kept, or the folder let go when it was the coordinator's own.
    pub fn close(&self) {
        if self.temporary {
            let _ = std::fs::remove_dir_all(&self.dir);
        } else {
            let _ = self.save();
        }
    }
}

fn status_of(id: &str, j: &Job) -> JobStatus {
    let count = |f: &dyn Fn(&State) -> bool| j.tasks.iter().filter(|t| f(&t.state)).count() as u32;
    JobStatus {
        id: id.to_string(),
        total: j.tasks.len() as u32,
        done: count(&|s| *s == State::Done),
        failed: count(&|s| *s == State::Failed),
        running: count(&|s| matches!(s, State::Running { .. })),
        errors: j.errors.clone(),
        name: j.name.clone(),
        kind: j.kind.clone(),
        submitted: j.submitted,
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

type ChunkStream = Pin<Box<dyn Stream<Item = Result<Chunk, Status>> + Send>>;

#[tonic::async_trait]
impl Coordinator for Service {
    async fn hello(&self, _r: Request<Empty>) -> Result<Response<CoordinatorHello>, Status> {
        Ok(Response::new(CoordinatorHello { version: VERSION.into() }))
    }

    async fn register(&self, r: Request<WorkerHello>) -> Result<Response<WorkerId>, Status> {
        let h = r.into_inner();
        if h.version != VERSION {
            return Err(Status::failed_precondition(format!(
                "this worker is Terraflux {}, the coordinator {VERSION}: install Terraflux {VERSION} on this \
                 machine",
                if h.version.is_empty() { "of an unknown version" } else { h.version.as_str() }
            )));
        }
        let mut g = self.lock();
        let id = g.id("worker");
        g.workers.insert(
            id.clone(),
            Worker { name: h.name, threads: h.threads, version: h.version, seen: Instant::now(), done: 0, task: None },
        );
        Ok(Response::new(WorkerId { id }))
    }

    async fn next_task(&self, r: Request<TaskRequest>) -> Result<Response<TaskAssignment>, Status> {
        let r = r.into_inner();
        let deadline = Instant::now() + Duration::from_millis(u64::from(r.wait_ms.min(60_000)));
        loop {
            let notified = self.notify.notified();
            {
                let mut g = self.lock();
                let Some(w) = g.workers.get_mut(&r.worker) else {
                    return Err(Status::not_found("unknown worker: register again"));
                };
                w.seen = Instant::now();
                while let Some((job, i)) = g.queue.pop_front() {
                    let Some(j) = g.jobs.get_mut(&job) else { continue };
                    let t = &mut j.tasks[i];
                    if t.state != State::Pending {
                        continue;
                    }
                    t.state = State::Running { worker: r.worker.clone() };
                    t.attempts += 1;
                    let a = TaskAssignment {
                        task_id: format!("{job}/{i}"),
                        job_id: job.clone(),
                        model_json: (*j.model_json).clone(),
                        overrides_json: t.overrides_json.clone(),
                        evaluation_json: j.evaluation_json.clone(),
                        return_files: j.return_files,
                        kind: j.kind.clone(),
                        payload: (*j.payload).clone(),
                        task_payload: t.payload.clone(),
                    };
                    if let Some(w) = g.workers.get_mut(&r.worker) {
                        w.task = Some((job, i));
                    }
                    return Ok(Response::new(a));
                }
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || tokio::time::timeout(left, notified).await.is_err() {
                return Ok(Response::new(TaskAssignment::default()));
            }
        }
    }

    type GetBlobStream = ChunkStream;

    async fn get_blob(&self, r: Request<BlobId>) -> Result<Response<ChunkStream>, Status> {
        let hash = r.into_inner().hash;
        if !self.store.has(&hash) {
            return Err(Status::not_found(format!("no blob {hash}")));
        }
        let path = self.store.path(&hash);
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        Ok(Response::new(Box::pin(file_chunks(path, hash, size).map(Ok))))
    }

    async fn heartbeat(&self, r: Request<WorkerId>) -> Result<Response<Ack>, Status> {
        let id = r.into_inner().id;
        let mut g = self.lock();
        let w = g.workers.get_mut(&id).ok_or_else(|| Status::not_found("unknown worker"))?;
        w.seen = Instant::now();
        Ok(Response::new(Ack { ok: true }))
    }

    async fn complete(&self, r: Request<TaskResult>) -> Result<Response<Ack>, Status> {
        let mut res = r.into_inner();
        if let Some(f) = res.files.iter().find(|f| !self.store.has(&f.hash)) {
            return Err(Status::failed_precondition(format!("the file {} was not sent", f.name)));
        }
        let mut g = self.lock();
        if let Some(w) = g.workers.get_mut(&res.worker) {
            w.seen = Instant::now();
            w.task = None;
            w.done += 1;
        }
        let (job, i) = res.task_id.rsplit_once('/').ok_or_else(|| Status::invalid_argument("bad task id"))?;
        let i: usize = i.parse().map_err(|_| Status::invalid_argument("bad task id"))?;
        let j = g.jobs.get_mut(job).ok_or_else(|| Status::not_found("unknown job"))?;
        let t = j.tasks.get_mut(i).ok_or_else(|| Status::not_found("unknown task"))?;
        // a late result of a task given again: the first one counts
        if matches!(t.state, State::Done | State::Failed) {
            return Ok(Response::new(Ack { ok: false }));
        }
        res.label = t.label.clone();
        if res.ok {
            t.state = State::Done;
        } else {
            t.state = State::Failed;
            j.errors.push(format!("{}: {}", t.label, res.error));
        }
        t.result = Some(res);
        g.dirty = true;
        drop(g);
        self.notify.notify_waiters();
        Ok(Response::new(Ack { ok: true }))
    }

    async fn put_blob(&self, r: Request<Streaming<Chunk>>) -> Result<Response<BlobId>, Status> {
        let mut s = r.into_inner();
        let mut w = self.store.writer().map_err(|e| Status::internal(e.to_string()))?;
        let mut expected = None;
        while let Some(c) = s.message().await? {
            if expected.is_none() {
                expected = Some(c.hash.clone());
            }
            w.write(&c.data).map_err(|e| Status::internal(e.to_string()))?;
        }
        let hash = w.finish(&self.store, expected.as_deref()).map_err(|e| Status::invalid_argument(e.to_string()))?;
        Ok(Response::new(BlobId { hash }))
    }

    async fn has_blob(&self, r: Request<BlobId>) -> Result<Response<Ack>, Status> {
        Ok(Response::new(Ack { ok: self.store.has(&r.into_inner().hash) }))
    }

    async fn submit(&self, r: Request<JobSpec>) -> Result<Response<JobId>, Status> {
        let s = r.into_inner();
        if s.tasks.is_empty() {
            return Err(Status::invalid_argument("a job needs a task"));
        }
        let mut g = self.lock();
        let id = g.id("job");
        let n = s.tasks.len();
        g.jobs.insert(
            id.clone(),
            Job {
                name: s.name,
                kind: s.kind,
                payload: Arc::new(s.payload),
                model_json: Arc::new(s.model_json),
                evaluation_json: s.evaluation_json,
                return_files: s.return_files,
                tasks: s
                    .tasks
                    .into_iter()
                    .map(|t| Task {
                        label: t.label,
                        overrides_json: t.overrides_json,
                        payload: t.payload,
                        state: State::Pending,
                        attempts: 0,
                        result: None,
                    })
                    .collect(),
                errors: Vec::new(),
                submitted: now(),
            },
        );
        for i in 0..n {
            g.queue.push_back((id.clone(), i));
        }
        g.dirty = true;
        drop(g);
        self.notify.notify_waiters();
        Ok(Response::new(JobId { id }))
    }

    async fn status(&self, r: Request<JobId>) -> Result<Response<JobStatus>, Status> {
        let id = r.into_inner().id;
        let g = self.lock();
        let j = g.jobs.get(&id).ok_or_else(|| Status::not_found("unknown job"))?;
        Ok(Response::new(status_of(&id, j)))
    }

    async fn wait(&self, r: Request<WaitRequest>) -> Result<Response<JobStatus>, Status> {
        let r = r.into_inner();
        let deadline = Instant::now() + Duration::from_millis(u64::from(r.wait_ms.min(60_000)));
        loop {
            let notified = self.notify.notified();
            let s = {
                let g = self.lock();
                let j = g.jobs.get(&r.id).ok_or_else(|| Status::not_found("unknown job"))?;
                status_of(&r.id, j)
            };
            let left = deadline.saturating_duration_since(Instant::now());
            if s.done + s.failed == s.total || left.is_zero() || tokio::time::timeout(left, notified).await.is_err() {
                return Ok(Response::new(s));
            }
        }
    }

    async fn forget(&self, r: Request<JobId>) -> Result<Response<Ack>, Status> {
        let id = r.into_inner().id;
        let mut g = self.lock();
        let Some(job) = g.jobs.remove(&id) else { return Ok(Response::new(Ack { ok: false })) };
        g.queue.retain(|(j, _)| *j != id);
        g.dirty = true;
        // the files of its results, unless another job's results have them too
        let mine: BTreeSet<String> = job
            .tasks
            .iter()
            .flat_map(|t| t.result.iter().flat_map(|r| r.files.iter().map(|f| f.hash.clone())))
            .collect();
        let others: BTreeSet<&String> = g
            .jobs
            .values()
            .flat_map(|j| j.tasks.iter().flat_map(|t| t.result.iter().flat_map(|r| r.files.iter().map(|f| &f.hash))))
            .collect();
        for h in mine.iter().filter(|h| !others.contains(h)) {
            self.store.remove(h);
        }
        Ok(Response::new(Ack { ok: true }))
    }

    async fn results(&self, r: Request<JobId>) -> Result<Response<JobResults>, Status> {
        let id = r.into_inner().id;
        let g = self.lock();
        let j = g.jobs.get(&id).ok_or_else(|| Status::not_found("unknown job"))?;
        Ok(Response::new(JobResults { results: j.tasks.iter().filter_map(|t| t.result.clone()).collect() }))
    }

    async fn jobs(&self, _r: Request<Empty>) -> Result<Response<JobList>, Status> {
        let g = self.lock();
        Ok(Response::new(JobList { jobs: g.jobs.iter().map(|(id, j)| status_of(id, j)).collect() }))
    }

    async fn workers(&self, _r: Request<Empty>) -> Result<Response<WorkerList>, Status> {
        let g = self.lock();
        Ok(Response::new(WorkerList {
            workers: g
                .workers
                .iter()
                .map(|(id, w)| WorkerState {
                    id: id.clone(),
                    name: w.name.clone(),
                    threads: w.threads,
                    done: w.done,
                    task: w.task.as_ref().map(|(j, i)| format!("{j}/{i}")).unwrap_or_default(),
                    seen_seconds_ago: w.seen.elapsed().as_secs_f64(),
                    version: w.version.clone(),
                })
                .collect(),
        }))
    }
}

/// A coordinator serving on an address (port 0: any free one): its address, the service and the server's task
/// (the jobs kept on disk every second).
pub async fn serve(
    addr: SocketAddr,
    options: Options,
) -> crate::Result<(SocketAddr, Service, tokio::task::JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local = listener.local_addr()?;
    let service = Service::new(options)?;
    let s = service.clone();
    let keeper = service.clone();
    let handle = tokio::spawn(async move {
        let server =
            CoordinatorServer::new(s).max_decoding_message_size(MAX_MESSAGE).max_encoding_message_size(MAX_MESSAGE);
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
        let serving = tonic::transport::Server::builder().add_service(server).serve_with_incoming(incoming);
        let keeping = async move {
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            loop {
                tick.tick().await;
                keeper.reap();
                let k = keeper.clone();
                let _ = tokio::task::spawn_blocking(move || k.save()).await;
            }
        };
        tokio::select! {
            _ = serving => {}
            _ = keeping => {}
        }
    });
    Ok((local, service, handle))
}
