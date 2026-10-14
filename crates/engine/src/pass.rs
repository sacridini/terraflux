//! Passes over the tiles: what the engine computes tile by tile and sums over the tiles, described as data
//! ([`Task`] in, [`Part`] out) so that the tiles may run on this machine's threads ([`Local`]) or be spread
//! over many machines (a [`Runner`] that sends [`Job`]s to workers sharing the storage of the maps).
//!
//! The parts of tiles merge in any order into the same result (sums of counts, minima and maxima, sets;
//! the patches' sizes are combined in the order of the tiles), so the maps are the same whichever runner,
//! however many workers and whatever the order the tiles come back in.

use std::collections::BTreeMap;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use terraflux_ops::woe::{Extent, Layout, VarSpec, WoeModel};

use crate::dinamica::{Hist, Made, Phase};
use crate::tiles::{Source, Spec, Tile, Tiling, open_specs};
use crate::{Error, Result};

/// What a pass computes on each tile (its sources are given with it, in the order each task says).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Task {
    /// The distinct values of the maps.
    Classes,
    /// The cells of each class of the first map holding each class of the second.
    Crosstab { classes: Vec<f64> },
    /// The cells of each value of a map.
    Counts { values: Vec<f64> },
    /// The extents of the drivers over the cells of each initial class (sources: t0, t1, the drivers in the
    /// order of `specs`).
    WoeExtents { froms: Vec<f64>, specs: Vec<VarSpec> },
    /// The cells of each bucket of the drivers by final class (the sources of `WoeExtents`).
    WoeCounts { froms: Vec<f64>, specs: Vec<VarSpec>, layouts: Vec<Layout>, fin: Vec<f64> },
    /// The patches of each observed transition (sources: t0, t1).
    Patches { transitions: Vec<(f64, f64)> },
    /// The probability of each transition, written (sources: the land, the drivers named `names`, then a layer
    /// per transition of the model).
    Probabilities { model: WoeModel, names: Vec<String> },
    /// Histograms of the weights (or the keys) of each transition's cells (sources: the land, the land at the
    /// start of the step, a probability per transition).
    Hists { phase: Phase, ranges: Vec<(f64, f64)>, keys: bool },
    /// The keys of each active transition's candidates of at least `loose[k]` (the sources of `Hists`).
    Keys { phase: Phase, loose: Vec<f64> },
    /// The patches grown, their changes kept from `trim[k]` on (the sources of `Hists`, then the layer written
    /// when `write`).
    Grow { phase: Phase, trim: Vec<f64>, write: bool, scores: bool },
}

/// The changes of a transition observed in a tile: cells, cells in expansions, and (patches, mean size,
/// variance) of the expansions and of the new patches.
pub type PatchPart = (i64, i64, (i64, f64, f64), (i64, f64, f64));

/// What a pass computed: a tile's, or the merge of many tiles'.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Part {
    Nothing,
    Values(Vec<f64>),
    Counts(Vec<i64>),
    ByValue(BTreeMap<u64, i64>),
    Extents(Vec<Extent>),
    Buckets(Vec<Vec<i64>>),
    /// (tile, a part per transition)
    Patches(Vec<(usize, Vec<PatchPart>)>),
    Hists(Vec<Hist>),
    Keys(Vec<Vec<f64>>),
    Made(Made),
}

fn add(a: &mut [i64], b: &[i64]) {
    for (x, y) in a.iter_mut().zip(b) {
        *x += y;
    }
}

impl Part {
    /// The merge of two parts of the same task.
    pub fn merge(self, other: Part) -> Result<Part> {
        Ok(match (self, other) {
            (Part::Nothing, p) | (p, Part::Nothing) => p,
            (Part::Values(mut a), Part::Values(b)) => {
                a.extend(b);
                a.sort_by(f64::total_cmp);
                a.dedup();
                Part::Values(a)
            }
            (Part::Counts(mut a), Part::Counts(b)) => {
                add(&mut a, &b);
                Part::Counts(a)
            }
            (Part::ByValue(mut a), Part::ByValue(b)) => {
                for (k, v) in b {
                    *a.entry(k).or_insert(0) += v;
                }
                Part::ByValue(a)
            }
            (Part::Extents(mut a), Part::Extents(b)) => {
                for (x, y) in a.iter_mut().zip(&b) {
                    x.merge(y);
                }
                Part::Extents(a)
            }
            (Part::Buckets(mut a), Part::Buckets(b)) => {
                for (x, y) in a.iter_mut().zip(&b) {
                    add(x, y);
                }
                Part::Buckets(a)
            }
            (Part::Patches(mut a), Part::Patches(b)) => {
                a.extend(b);
                a.sort_by_key(|p| p.0);
                Part::Patches(a)
            }
            (Part::Hists(a), Part::Hists(b)) => Part::Hists(a.into_iter().zip(b).map(|(x, y)| x.merge(y)).collect()),
            (Part::Keys(mut a), Part::Keys(b)) => {
                for (x, y) in a.iter_mut().zip(b) {
                    x.extend(y);
                }
                Part::Keys(a)
            }
            (Part::Made(a), Part::Made(b)) => Part::Made(a.merge(b)),
            _ => return Err(Error::Msg("parts of different passes".into())),
        })
    }
}

/// A task on one tile.
fn on_tile(task: &Task, tile: &Tile, sources: &[&dyn Source]) -> Result<Part> {
    match task {
        Task::Classes | Task::Crosstab { .. } | Task::Counts { .. } => crate::fit::on_tile(task, tile, sources),
        Task::WoeExtents { .. } | Task::WoeCounts { .. } | Task::Patches { .. } => {
            crate::fit::on_tile(task, tile, sources)
        }
        Task::Probabilities { .. } | Task::Hists { .. } | Task::Keys { .. } | Task::Grow { .. } => {
            crate::dinamica::on_tile(task, tile, sources)
        }
    }
}

/// A task on some tiles, in parallel on this machine's threads.
pub fn run_tiles(tiles: &[Tile], sources: &[&dyn Source], task: &Task) -> Result<Part> {
    tiles.par_iter().map(|t| on_tile(task, t, sources)).try_reduce(|| Part::Nothing, |a, b| a.merge(b))
}

/// What runs the passes over the tiles.
pub trait Runner: Send + Sync + std::fmt::Debug {
    /// A pass over every tile of `tiling`: the merge of the tiles' parts.
    fn run(&self, tiling: &Tiling, sources: &[&dyn Source], task: &Task) -> Result<Part>;
}

/// This machine's threads.
#[derive(Debug, Clone, Copy, Default)]
pub struct Local;

impl Runner for Local {
    fn run(&self, tiling: &Tiling, sources: &[&dyn Source], task: &Task) -> Result<Part> {
        run_tiles(&tiling.tiles(), sources, task)
    }
}

/// A pass as another machine runs it: the tiling, where its maps are and the task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub width: usize,
    pub height: usize,
    pub size: usize,
    pub sources: Vec<Spec>,
    pub task: Task,
}

fn config() -> bincode::config::Configuration {
    bincode::config::standard()
}

pub fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    bincode::serde::encode_to_vec(v, config()).map_err(|e| Error::Msg(format!("encoding: {e}")))
}

pub fn decode<T: serde::de::DeserializeOwned>(b: &[u8]) -> Result<T> {
    bincode::serde::decode_from_slice(b, config()).map(|(v, _)| v).map_err(|e| Error::Msg(format!("decoding: {e}")))
}

impl Job {
    /// The job of a pass; every source must be somewhere other machines see (files, layers on disk).
    pub fn new(tiling: &Tiling, sources: &[&dyn Source], task: &Task) -> Result<Job> {
        let specs = sources
            .iter()
            .map(|s| {
                s.spec().ok_or_else(|| {
                    Error::Msg("a map in memory cannot go to other machines: keep the layers on disk (--disk)".into())
                })
            })
            .collect::<Result<_>>()?;
        Ok(Job { width: tiling.width, height: tiling.height, size: tiling.size, sources: specs, task: task.clone() })
    }

    pub fn tiling(&self) -> Tiling {
        Tiling::new(self.width, self.height, self.size)
    }

    /// Run the job's task on some of its tiles (by index) on this machine.
    pub fn run(&self, tiles: &[usize]) -> Result<Part> {
        terraflux_ops::threads::ensure();
        let all = self.tiling().tiles();
        let chosen: Vec<Tile> = tiles
            .iter()
            .map(|&i| all.get(i).copied().ok_or_else(|| Error::Msg(format!("no tile {i}"))))
            .collect::<Result<_>>()?;
        let open = open_specs(&self.sources)?;
        let refs: Vec<&dyn Source> = open.iter().map(|s| s.as_ref()).collect();
        run_tiles(&chosen, &refs, &self.task)
    }
}

/// A worker's side of a remote pass: the job and its tiles encoded, the part encoded.
pub fn run_encoded(job: &[u8], tiles: &[u8]) -> Result<Vec<u8>> {
    let job: Job = decode(job)?;
    let tiles: Vec<usize> = decode(tiles)?;
    encode(&job.run(&tiles)?)
}
