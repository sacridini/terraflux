//! The tiled engine: models on maps larger than memory, cut in tiles that are read, computed and written
//! one at a time (in parallel), and the same maps whatever the tiling.
//!
//! - **Fits by reduction:** what a fit needs from the whole map (counts, extents) is summed over the tiles,
//!   so Markov chains and weights of evidence are the exact engine's to the bit.
//! - **Random numbers by counter** ([`rng`]): every draw is a function of what it is for, not of the draws
//!   before it.
//! - **Allocation by thresholds** ([`dinamica`]): the transition probabilities are computed tile by tile;
//!   the seeds of the Expander and the Patcher are the cells whose random keys pass thresholds found from
//!   histograms summed over the tiles; patches grow within a radius of their seed, so a tile with a halo of
//!   twice that radius grows every patch that may reach it as any other tile would; cells claimed by several
//!   patches go to the one of highest key. The seeds' thresholds are adjusted in rounds until the changes meet
//!   the demand. This is statistically equivalent to Dinamica EGO's sequential allocation, not the same draws.

// `!(x > 0.0)` is meant: it is true for NaN too.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

pub mod dinamica;
pub mod fit;
pub mod graph;
pub mod pass;
pub mod rng;
pub mod run;
pub mod tiles;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Msg(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl From<terraflux_ops::Error> for Error {
    fn from(e: terraflux_ops::Error) -> Self {
        Error::Msg(e.0)
    }
}

impl From<terraflux_ls_sys::Error> for Error {
    fn from(e: terraflux_ls_sys::Error) -> Self {
        Error::Msg(e.what().to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// With `TERRAFLUX_TRACE` set, a line on standard error with the time since the previous one.
pub(crate) fn trace(what: impl FnOnce() -> String) {
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    static ON: OnceLock<bool> = OnceLock::new();
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    if !*ON.get_or_init(|| std::env::var_os("TERRAFLUX_TRACE").is_some()) {
        return;
    }
    let mut last = LAST.lock().unwrap();
    let now = Instant::now();
    let dt = last.map(|t| (now - t).as_secs_f64()).unwrap_or(0.0);
    *last = Some(now);
    eprintln!("    [{dt:8.3} s] {}", what());
}

static SPENT: [std::sync::atomic::AtomicU64; 3] =
    [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];

/// Thread time spent in the parts of a pass (for the traces).
pub(crate) fn spent(d: &[std::time::Duration; 3]) {
    for (a, d) in SPENT.iter().zip(d) {
        a.fetch_add(d.as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The thread time spent since the last call, in seconds.
pub(crate) fn spent_take() -> [f64; 3] {
    SPENT.each_ref().map(|a| a.swap(0, std::sync::atomic::Ordering::Relaxed) as f64 * 1e-9)
}
