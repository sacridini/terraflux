//! Terraflux on many machines: a coordinator ([`coordinator`]), headless workers ([`worker`]) and the clients
//! that submit batches to them ([`client`], [`batch`]).
//!
//! A job is a model and the overrides of each of its tasks (replicates, scenarios, the candidates of a
//! calibration). The data travels once, by hash; workers keep it in a cache. Workers only call the
//! coordinator, so they run behind firewalls, on machines started by hand, in the cloud or by a cluster's
//! scheduler. Every task runs the exact engine, so a task gives the same maps on any worker as on the machine
//! that submitted it.

pub mod awake;
pub mod batch;
pub mod bundle;
pub mod client;
pub mod cloud;
pub mod coordinator;
pub mod discover;
pub mod evaluate;
pub mod idle;
pub mod net;
pub mod pool;
pub mod remote;
pub mod run;
pub mod secure;
pub mod store;
pub mod tiles;
pub mod worker;

pub mod proto {
    tonic::include_proto!("terraflux.v1");
}

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("connecting: {0}")]
    Transport(#[from] tonic::transport::Error),
    #[error("{}", .0.message())]
    Status(#[from] tonic::Status),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A number not given before in this process (for names of files of its own).
pub fn unique() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// The SHA-256 of some bytes, in hexadecimal: the name of a blob.
pub fn hash(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// A coordinator and `workers` workers in this process (on 127.0.0.1): the coordinator's URL and what stops
/// them. One machine's cores used for a batch, with the same code as a cluster.
pub struct Local {
    pub url: String,
    pub stop: Arc<AtomicBool>,
    pub service: coordinator::Service,
    handles: Vec<tokio::task::JoinHandle<()>>,
}

impl Local {
    pub async fn start(workers: usize, cache: PathBuf, options: coordinator::Options) -> Result<Local> {
        let (addr, service, server) = coordinator::serve(SocketAddr::from(([127, 0, 0, 1], 0)), options).await?;
        let url = format!("http://{addr}");
        let stop = Arc::new(AtomicBool::new(false));
        let mut handles = vec![server];
        // the workers share the machine's threads (never every core)
        let per = (terraflux_ops::threads::most() / workers.max(1)).max(1);
        for k in 0..workers {
            let (u, s, c) = (url.clone(), stop.clone(), cache.clone());
            handles.push(tokio::spawn(async move {
                let mut o = worker::Options::new(&format!("local-{k}"), c);
                o.threads = per;
                let _ = worker::run(&u, o, s).await;
            }));
        }
        Ok(Local { url, stop, service, handles })
    }

    /// Stop the workers and the coordinator (its jobs kept on disk, when it has a folder of its own).
    pub fn shutdown(self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        for h in self.handles {
            h.abort();
        }
        self.service.close();
    }
}
