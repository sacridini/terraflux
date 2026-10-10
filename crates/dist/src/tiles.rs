//! The tiled engine on many machines: each pass over the tiles is a job of the coordinator, its tasks the
//! tiles cut in contiguous runs (a few per worker), its results the runs' parts, merged here. The workers read
//! and write the maps on storage they share (the same paths everywhere), so a pass sends only what to compute
//! and gets back sums (histograms, counts, keys).
//!
//! The merge does not depend on which worker computed which tiles, nor in which order they came back: the
//! maps are those of the same model run on one machine.

use std::sync::Mutex;

use terraflux_engine::pass::{Job, Part, Runner, Task, decode, encode};
use terraflux_engine::tiles::{Source, Tiling};

use crate::client::Client;

/// A runner of passes on a coordinator's workers.
pub struct Remote {
    url: String,
    rt: tokio::runtime::Runtime,
    client: Mutex<Client>,
    /// Runs of tiles per worker in each pass.
    pub per_worker: usize,
}

impl std::fmt::Debug for Remote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Remote({})", self.url)
    }
}

impl Remote {
    /// Connect to a coordinator (from outside any async runtime: the runner has its own).
    pub fn connect(url: &str) -> crate::Result<Remote> {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let client = rt.block_on(Client::connect(url))?;
        Ok(Remote { url: url.to_string(), rt, client: Mutex::new(client), per_worker: 4 })
    }

    fn pass(&self, payload: Vec<u8>, tiles: usize) -> crate::Result<Vec<Part>> {
        let mut c = self.client.lock().unwrap_or_else(|e| e.into_inner());
        self.rt.block_on(async {
            let workers = c.workers().await?.len().max(1);
            // contiguous runs of tiles: neighbouring tiles share their halos' reading
            let runs = (workers * self.per_worker).clamp(1, tiles.max(1));
            let per = tiles.div_ceil(runs);
            let chunks: Vec<Vec<usize>> =
                (0..tiles).collect::<Vec<_>>().chunks(per.max(1)).map(<[usize]>::to_vec).collect();
            let tasks = chunks.iter().map(encode).collect::<terraflux_engine::Result<Vec<_>>>().map_err(other)?;
            let job = c.submit_payload("tiles", payload, tasks).await?;
            let status = c.wait_done(&job).await?;
            if status.failed > 0 {
                let _ = c.forget(&job).await;
                return Err(crate::Error::Other(format!("a pass failed on the workers: {}", status.errors.join("; "))));
            }
            let results = c.results(&job).await?;
            c.forget(&job).await?;
            results.iter().map(|r| decode::<Part>(&r.payload).map_err(other)).collect()
        })
    }
}

fn other(e: terraflux_engine::Error) -> crate::Error {
    crate::Error::Other(e.to_string())
}

impl Runner for Remote {
    fn run(&self, tiling: &Tiling, sources: &[&dyn Source], task: &Task) -> terraflux_engine::Result<Part> {
        let job = Job::new(tiling, sources, task)?;
        let parts =
            self.pass(encode(&job)?, tiling.tiles().len()).map_err(|e| terraflux_engine::Error::Msg(e.to_string()))?;
        parts.into_iter().try_fold(Part::Nothing, |a, b| a.merge(b))
    }
}
