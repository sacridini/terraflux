//! The tiled engine on many machines: each pass over the tiles is a job of the coordinator, its tasks the
//! tiles cut in contiguous runs (a few per worker), its results the runs' parts, merged here.
//!
//! What the workers read goes to them through the coordinator, so they need no storage of their own in common
//! with this machine: the model's maps once (by hash, as a batch's files), and the layers of the steps by their
//! tiles (each tile file a blob: a pass says which tiles each layer has, and their hashes; a worker fetches those
//! of its tiles and of their neighbours, the halos, and sends back by hash the tiles it wrote, fetched here into
//! the layer's folder). With storage shared at the same paths ([`Remote::shared`]), a pass sends only what to
//! compute and gets back sums (histograms, counts, keys).
//!
//! The merge does not depend on which worker computed which tiles, nor in which order they came back: the
//! maps are those of the same model run on one machine.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use terraflux_engine::pass::{Job, Part, Runner, Task, decode, encode};
use terraflux_engine::tiles::{Source, Spec, Tiling};

use crate::client::Client;

/// The kind of the jobs whose layers travel by their tiles.
pub const SENT: &str = "tiles-sent";

/// A layer of a pass: its key (its folder on the machine that runs the model) and its tiles (index, hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerSent {
    pub key: String,
    pub tiles: Vec<(usize, String)>,
}

/// A pass whose layers travel by their tiles: the engine's job (its files sent, `bundle:`) and its layers, in
/// the order of the job's layer sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentJob {
    pub job: Vec<u8>,
    pub layers: Vec<LayerSent>,
}

/// What a worker gives back of such a pass: the part, and the tiles it wrote (layer's key, index, hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentPart {
    pub part: Vec<u8>,
    pub written: Vec<(String, usize, String)>,
}

/// The tiles of a layer's folder (`<index>.bin`).
pub fn tile_files(dir: &Path) -> Vec<(usize, PathBuf)> {
    let mut out: Vec<(usize, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let i = name.strip_suffix(".bin")?.parse::<usize>().ok()?;
            Some((i, e.path()))
        })
        .collect();
    out.sort();
    out
}

/// A runner of passes on a coordinator's workers.
pub struct Remote {
    url: String,
    rt: tokio::runtime::Runtime,
    client: Mutex<Client>,
    /// Runs of tiles per worker in each pass.
    pub per_worker: usize,
    /// Whether the workers read and write the maps and layers on storage shared with this machine, at the same
    /// paths (else they go through the coordinator).
    pub shared: bool,
    /// The files sent (their paths, as sent), and the hashes of the tiles (by file, with its size and time).
    sent: Mutex<HashMap<PathBuf, String>>,
    hashes: Mutex<HashMap<PathBuf, (u64, SystemTime, String)>>,
}

impl std::fmt::Debug for Remote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Remote({})", self.url)
    }
}

impl Remote {
    /// Connect to a coordinator (from outside any async runtime: the runner has its own).
    pub fn connect(url: &str) -> crate::Result<Remote> {
        Remote::connect_with(url, None)
    }

    /// Connect to a coordinator with its pairing code, if it has one.
    pub fn connect_with(url: &str, code: Option<&crate::secure::Code>) -> crate::Result<Remote> {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let client = rt.block_on(Client::connect_with(url, code))?;
        Ok(Remote {
            url: url.to_string(),
            rt,
            client: Mutex::new(client),
            per_worker: 4,
            shared: false,
            sent: Mutex::new(HashMap::new()),
            hashes: Mutex::new(HashMap::new()),
        })
    }

    /// A tile file's hash (sent to the coordinator unless it has it), hashed again only when it changed.
    async fn put_tile(&self, c: &mut Client, path: &Path) -> crate::Result<String> {
        let meta = std::fs::metadata(path)?;
        let (len, time) = (meta.len(), meta.modified()?);
        if let Some((l, t, h)) = self.hashes.lock().unwrap().get(path)
            && *l == len
            && *t == time
        {
            return Ok(h.clone());
        }
        let h = c.put_file(path).await?;
        self.hashes.lock().unwrap().insert(path.to_path_buf(), (len, time, h.clone()));
        Ok(h)
    }

    /// The job with its files sent and its layers' tiles listed (and sent): what a worker needs.
    async fn sent(&self, c: &mut Client, mut job: Job) -> crate::Result<(SentJob, Vec<PathBuf>)> {
        let mut layers = Vec::new();
        let mut dirs = Vec::new();
        for s in &mut job.sources {
            match s {
                Spec::File { path, .. } => {
                    let known = self.sent.lock().unwrap().get(path.as_path()).cloned();
                    let b = match known {
                        Some(b) => b,
                        None => {
                            let b = c.send(&path.to_string_lossy(), Path::new("")).await?;
                            self.sent.lock().unwrap().insert(path.clone(), b.clone());
                            b
                        }
                    };
                    *path = PathBuf::from(b);
                }
                Spec::Layer { dir, .. } => {
                    let mut tiles = Vec::new();
                    for (i, f) in tile_files(dir) {
                        tiles.push((i, self.put_tile(c, &f).await?));
                    }
                    layers.push(LayerSent { key: dir.to_string_lossy().into_owned(), tiles });
                    dirs.push(dir.clone());
                }
            }
        }
        let job = encode(&job).map_err(other)?;
        Ok((SentJob { job, layers }, dirs))
    }

    fn pass(&self, job: Job, tiles: usize) -> crate::Result<Vec<Part>> {
        let mut c = self.client.lock().unwrap_or_else(|e| e.into_inner());
        self.rt.block_on(async {
            let workers = c.workers().await?.len().max(1);
            // contiguous runs of tiles: neighbouring tiles share their halos' reading
            let runs = (workers * self.per_worker).clamp(1, tiles.max(1));
            let per = tiles.div_ceil(runs);
            let chunks: Vec<Vec<usize>> =
                (0..tiles).collect::<Vec<_>>().chunks(per.max(1)).map(<[usize]>::to_vec).collect();
            let tasks = chunks.iter().map(encode).collect::<terraflux_engine::Result<Vec<_>>>().map_err(other)?;
            let (kind, payload, dirs) = if self.shared {
                ("tiles", encode(&job).map_err(other)?, Vec::new())
            } else {
                let (s, dirs) = self.sent(&mut c, job).await?;
                (SENT, encode(&s).map_err(other)?, dirs)
            };
            let id = c.submit_payload(kind, payload, tasks).await?;
            let status = c.wait_done(&id).await?;
            if status.failed > 0 {
                let _ = c.forget(&id).await;
                return Err(crate::Error::Other(format!("a pass failed on the workers: {}", status.errors.join("; "))));
            }
            let results = c.results(&id).await?;
            let mut parts = Vec::new();
            for r in &results {
                if kind == SENT {
                    let s: SentPart = decode(&r.payload).map_err(other)?;
                    // the tiles the workers wrote, in the layers' folders here
                    for (key, i, hash) in &s.written {
                        let dir =
                            dirs.iter().find(|d| d.to_string_lossy() == *key).cloned().unwrap_or(PathBuf::from(key));
                        c.download(hash, &dir.join(format!("{i}.bin"))).await?;
                    }
                    parts.push(decode::<Part>(&s.part).map_err(other)?);
                } else {
                    parts.push(decode::<Part>(&r.payload).map_err(other)?);
                }
            }
            c.forget(&id).await?;
            Ok(parts)
        })
    }
}

fn other(e: terraflux_engine::Error) -> crate::Error {
    crate::Error::Other(e.to_string())
}

impl Runner for Remote {
    fn run(&self, tiling: &Tiling, sources: &[&dyn Source], task: &Task) -> terraflux_engine::Result<Part> {
        let job = Job::new(tiling, sources, task)?;
        let parts = self.pass(job, tiling.tiles().len()).map_err(|e| terraflux_engine::Error::Msg(e.to_string()))?;
        parts.into_iter().try_fold(Part::Nothing, |a, b| a.merge(b))
    }
}

/// The tiles within `rings` tiles of some tiles (theirs, and their neighbours').
pub fn around(tiles: &[usize], tiling: &Tiling, rings: usize) -> Vec<usize> {
    let (across, down) = (tiling.across(), tiling.down());
    let mut out = std::collections::BTreeSet::new();
    for &t in tiles {
        let (row, col) = (t / across.max(1), t % across.max(1));
        for r in row.saturating_sub(rings)..(row + rings + 1).min(down) {
            for c in col.saturating_sub(rings)..(col + rings + 1).min(across) {
                out.insert(r * across + c);
            }
        }
    }
    out.into_iter().collect()
}
