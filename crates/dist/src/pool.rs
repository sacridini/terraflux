//! What the application runs batches on, and how it lends its computer to another's.
//!
//! - [`Pool`]: a coordinator of the application's own (its jobs kept in a folder, so that a batch goes on after
//!   the application is opened again) with workers of this computer; shared or not. Shared, it listens on the
//!   network (the port 7700 if free) and says so with its beacons, and the computers given its code work for it.
//!   It always speaks TLS with its code: its own workers too.
//! - [`Lender`]: a worker that finds the coordinator of a code on the network and works for it, as long as the
//!   application wants (paused at will, or while someone uses the computer).
//!
//! Both run on a runtime of their own, so the application calls them from its thread.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::coordinator::{self, Options, Service};
use crate::proto::WorkerState;
use crate::run::Place;
use crate::secure::Code;
use crate::worker::{self, Status};
use crate::{Result, discover};

/// The port a shared coordinator takes when it is free.
pub const SHARED_PORT: u16 = 7700;

fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?)
}

struct Server {
    url: String,
    service: Service,
    handle: tokio::task::JoinHandle<()>,
    beacons: Option<tokio::task::JoinHandle<()>>,
}

/// A coordinator of the application's own, with workers of this computer.
pub struct Pool {
    rt: tokio::runtime::Runtime,
    dir: PathBuf,
    cache: PathBuf,
    code: Code,
    shared: bool,
    /// Where it listens when shared (every network, the port 7700 if free), and the port its beacons go to.
    ip: IpAddr,
    port: u16,
    beacons: u16,
    server: Option<Server>,
    workers: Vec<(Arc<AtomicBool>, tokio::task::JoinHandle<()>)>,
}

impl Pool {
    /// A pool on its folder (its jobs taken up again), with `workers` workers of this computer.
    pub fn start(dir: PathBuf, cache: PathBuf, code: Code, shared: bool, workers: usize) -> Result<Pool> {
        let mut p = Pool {
            rt: runtime()?,
            dir,
            cache,
            code,
            shared,
            ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: SHARED_PORT,
            beacons: discover::PORT,
            server: None,
            workers: Vec::new(),
        };
        p.serve()?;
        p.set_workers(workers);
        Ok(p)
    }

    /// Where it listens when shared (the port 0: any free one), and the port its beacons go to (tests keep to
    /// this computer).
    pub fn with_network(mut self, ip: IpAddr, port: u16, beacons: u16) -> Result<Pool> {
        self.ip = ip;
        self.port = port;
        self.beacons = beacons;
        let workers = self.workers.len();
        self.restart(workers)?;
        Ok(self)
    }

    fn serve(&mut self) -> Result<()> {
        let options = Options { dir: Some(self.dir.clone()), code: Some(self.code.clone()), ..Default::default() };
        let (code, shared, beacons, ip, port) = (self.code.clone(), self.shared, self.beacons, self.ip, self.port);
        let server = self.rt.block_on(async move {
            let at = |port| SocketAddr::new(ip, port);
            let (addr, service, handle) = if shared {
                match coordinator::serve(at(port), options.clone()).await {
                    Ok(s) => s,
                    Err(_) => coordinator::serve(at(0), options).await?,
                }
            } else {
                coordinator::serve(SocketAddr::from(([127, 0, 0, 1], 0)), options).await?
            };
            let beacons = if shared { Some(discover::announce(&code, addr.port(), beacons)?) } else { None };
            Ok::<_, crate::Error>(Server { url: format!("http://127.0.0.1:{}", addr.port()), service, handle, beacons })
        })?;
        self.server = Some(server);
        Ok(())
    }

    fn stop_server(&mut self) {
        if let Some(s) = self.server.take() {
            let _ = s.service.save();
            s.handle.abort();
            if let Some(b) = s.beacons {
                b.abort();
            }
        }
    }

    fn restart(&mut self, workers: usize) -> Result<()> {
        self.set_workers(0);
        self.stop_server();
        self.serve()?;
        self.set_workers(workers);
        Ok(())
    }

    /// Where its batches go: its address on this computer, and its code.
    pub fn place(&self) -> Place {
        Place::At { url: self.url(), code: Some(self.code.clone()) }
    }

    pub fn url(&self) -> String {
        self.server.as_ref().map(|s| s.url.clone()).unwrap_or_default()
    }

    /// The port it listens on.
    pub fn port(&self) -> u16 {
        self.url().rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or(0)
    }

    pub fn code(&self) -> &Code {
        &self.code
    }

    pub fn shared(&self) -> bool {
        self.shared
    }

    /// The workers of this computer: as many as `n` from now on (each with an equal share of its threads).
    pub fn set_workers(&mut self, n: usize) {
        if n == self.workers.len() {
            return;
        }
        for (stop, h) in self.workers.drain(..) {
            stop.store(true, Ordering::Relaxed);
            h.abort();
        }
        let per = (terraflux_ops::threads::most() / n.max(1)).max(1);
        for k in 0..n {
            let stop = Arc::new(AtomicBool::new(false));
            let mut o = worker::Options::new(&format!("{} {}", discover::hostname(), k + 1), self.cache.clone());
            o.threads = per;
            o.code = Some(self.code.clone());
            let (url, s) = (self.url(), stop.clone());
            let h = self.rt.spawn(async move {
                let _ = worker::run(&url, o, s).await;
            });
            self.workers.push((stop, h));
        }
    }

    pub fn local_workers(&self) -> usize {
        self.workers.len()
    }

    /// Share it with the computers given its code, or stop sharing (its jobs go on).
    pub fn set_shared(&mut self, shared: bool) -> Result<()> {
        if shared != self.shared {
            self.shared = shared;
            let n = self.workers.len();
            self.restart(n)?;
        }
        Ok(())
    }

    /// Another code (the computers given the old one cannot work for it any more).
    pub fn set_code(&mut self, code: Code) -> Result<()> {
        if code != self.code {
            self.code = code;
            let n = self.workers.len();
            // the old certificate goes with the old code
            let _ = std::fs::remove_file(self.dir.join("tls-cert.der"));
            let _ = std::fs::remove_file(self.dir.join("tls-key.der"));
            self.restart(n)?;
        }
        Ok(())
    }

    /// The workers that work for it, this computer's and the others'.
    pub fn workers(&self) -> Vec<WorkerState> {
        let Some(s) = &self.server else { return Vec::new() };
        let service = s.service.clone();
        service.workers_now()
    }

    /// Drop a job (and its files).
    pub fn forget(&self, job: &str) {
        if let Some(s) = &self.server {
            s.service.forget_now(job);
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.set_workers(0);
        self.stop_server();
    }
}

/// A worker that lends this computer to the coordinator of a code.
pub struct Lender {
    rt: tokio::runtime::Runtime,
    stop: Arc<AtomicBool>,
    pub status: Arc<Mutex<Status>>,
    /// Set: no task taken until it is let go.
    pub paused: Arc<AtomicBool>,
    code: Code,
}

impl Lender {
    /// Lend this computer: `threads` of its threads, tasks taken only when its user is away for `idle` (if
    /// given), the coordinator's beacons heard at `on` ([`discover::everywhere`]).
    pub fn start(code: Code, cache: PathBuf, threads: usize, idle: Option<Duration>, on: SocketAddr) -> Result<Lender> {
        let rt = runtime()?;
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(Status::default()));
        let paused = Arc::new(AtomicBool::new(false));
        let mut o = worker::Options::new(&discover::hostname(), cache);
        o.threads = threads.max(1);
        o.idle = idle;
        o.paused = Some(paused.clone());
        o.status = Some(status.clone());
        let (c, s) = (code.clone(), stop.clone());
        rt.spawn(async move {
            let _ = worker::run_found(c, o, s, on).await;
        });
        Ok(Lender { rt, stop, status, paused, code })
    }

    pub fn code(&self) -> &Code {
        &self.code
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl Drop for Lender {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = &self.rt;
    }
}
