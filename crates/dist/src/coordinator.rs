//! The coordinator: the queue of the tasks of the jobs submitted, the data they need (blobs by hash), the
//! workers and what each is doing. A task whose worker stops sending heartbeats goes back to the queue.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::Notify;
use tonic::{Request, Response, Status};

use crate::proto::coordinator_server::{Coordinator, CoordinatorServer};
use crate::proto::*;

/// The largest message (a blob, the files of a result).
pub const MAX_MESSAGE: usize = 1 << 30;

/// How the coordinator treats its workers.
#[derive(Debug, Clone)]
pub struct Options {
    /// A running task goes back to the queue when its worker has not been heard of for this long.
    pub timeout: Duration,
    /// Times a task is tried before it fails.
    pub attempts: u32,
}

impl Default for Options {
    fn default() -> Self {
        Options { timeout: Duration::from_secs(60), attempts: 3 }
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
    kind: String,
    payload: Arc<Vec<u8>>,
    model_json: String,
    evaluation_json: String,
    return_files: bool,
    tasks: Vec<Task>,
    errors: Vec<String>,
}

struct Worker {
    name: String,
    threads: u32,
    seen: Instant,
    done: u32,
    task: Option<(String, usize)>,
}

#[derive(Default)]
struct Inner {
    blobs: HashMap<String, Arc<Vec<u8>>>,
    jobs: BTreeMap<String, Job>,
    queue: VecDeque<(String, usize)>,
    workers: BTreeMap<String, Worker>,
    next: u64,
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
            }
        }
    }
}

/// The coordinator's service.
#[derive(Clone)]
pub struct Service {
    inner: Arc<Mutex<Inner>>,
    notify: Arc<Notify>,
    options: Options,
}

impl Service {
    pub fn new(options: Options) -> Self {
        Service { inner: Arc::new(Mutex::new(Inner::default())), notify: Arc::new(Notify::new()), options }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Check the workers now (also done every second by the server).
    pub fn reap(&self) {
        let (t, a) = (self.options.timeout, self.options.attempts);
        self.lock().reap(t, a);
        self.notify.notify_waiters();
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
    }
}

#[tonic::async_trait]
impl Coordinator for Service {
    async fn register(&self, r: Request<WorkerHello>) -> Result<Response<WorkerId>, Status> {
        let h = r.into_inner();
        let mut g = self.lock();
        let id = g.id("worker");
        g.workers
            .insert(id.clone(), Worker { name: h.name, threads: h.threads, seen: Instant::now(), done: 0, task: None });
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
                        model_json: j.model_json.clone(),
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

    async fn get_blob(&self, r: Request<BlobId>) -> Result<Response<Blob>, Status> {
        let hash = r.into_inner().hash;
        let data = self.lock().blobs.get(&hash).cloned().ok_or_else(|| Status::not_found(format!("no blob {hash}")))?;
        Ok(Response::new(Blob { hash, data: (*data).clone() }))
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
        drop(g);
        self.notify.notify_waiters();
        Ok(Response::new(Ack { ok: true }))
    }

    async fn put_blob(&self, r: Request<Blob>) -> Result<Response<BlobId>, Status> {
        let b = r.into_inner();
        let hash = crate::hash(&b.data);
        if !b.hash.is_empty() && b.hash != hash {
            return Err(Status::invalid_argument("the blob does not match its hash"));
        }
        self.lock().blobs.entry(hash.clone()).or_insert_with(|| Arc::new(b.data));
        Ok(Response::new(BlobId { hash }))
    }

    async fn has_blob(&self, r: Request<BlobId>) -> Result<Response<Ack>, Status> {
        Ok(Response::new(Ack { ok: self.lock().blobs.contains_key(&r.into_inner().hash) }))
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
                kind: s.kind,
                payload: Arc::new(s.payload),
                model_json: s.model_json,
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
            },
        );
        for i in 0..n {
            g.queue.push_back((id.clone(), i));
        }
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
        let ok = g.jobs.remove(&id).is_some();
        g.queue.retain(|(j, _)| *j != id);
        Ok(Response::new(Ack { ok }))
    }

    async fn results(&self, r: Request<JobId>) -> Result<Response<JobResults>, Status> {
        let id = r.into_inner().id;
        let g = self.lock();
        let j = g.jobs.get(&id).ok_or_else(|| Status::not_found("unknown job"))?;
        Ok(Response::new(JobResults { results: j.tasks.iter().filter_map(|t| t.result.clone()).collect() }))
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
                })
                .collect(),
        }))
    }
}

/// A coordinator serving on an address (port 0: any free one): its address and the server's task.
pub async fn serve(
    addr: SocketAddr,
    options: Options,
) -> std::io::Result<(SocketAddr, Service, tokio::task::JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local = listener.local_addr()?;
    let service = Service::new(options);
    let s = service.clone();
    let reaper = service.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            reaper.reap();
        }
    });
    let handle = tokio::spawn(async move {
        let server =
            CoordinatorServer::new(s).max_decoding_message_size(MAX_MESSAGE).max_encoding_message_size(MAX_MESSAGE);
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
        let _ = tonic::transport::Server::builder().add_service(server).serve_with_incoming(incoming).await;
    });
    Ok((local, service, handle))
}
