//! Multiresolution segmentation by tiles (Happ et al. 2010, in the spirit of the OTB's LSMS): each tile is segmented
//! with a margin around it, all tiles in parallel; the objects are then stitched at the tile borders.
//!
//! 1. Each tile's window (its core plus the margin, within the image) is segmented on its own by the ordinary
//!    single-threaded algorithm, with a seed made of the run's seed and the tile's index. The margin gives the objects
//!    near the border the context they would have in the whole image.
//! 2. Only the core of each tile is kept: the cells of the objects of the window that lie in the core, each piece in
//!    its connected parts, are the regions of the second pass.
//! 3. The second pass is the ordinary merging over the whole image that starts from those pieces (the `from` of
//!    [`merge_level`]) with the same criterion and scale. A piece far from a tile border is at a fixpoint already (its
//!    neighbours are in its tile), so only the pieces along the borders merge, across them: an object cut by a border
//!    is one again whenever the criterion allows. The cost of this pass is linear in the cells, one thread.
//!
//! Nothing depends on the order the tiles finish in: the result is the same for a seed at any number of threads.
//!
//! The image comes from a [`Source`]: in memory, or rasters on disk. Step 1 reads each tile's window when its turn
//! comes (a few windows in memory, one per thread); step 3 reads the image once more by strips of rows, for the
//! sums of the pieces (count, sum, sum of squares per band), never the whole. Only the labels (4 bytes a cell) are
//! whole in memory. From disk or from memory the objects are the same.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use terraflux_raster::Grid;

use super::big::{Layered, Part, Source, TILES_BYTES};
use super::multiresolution::{Cost, merge_level, merge_level_src};
use super::{Labels, Objects};
use crate::files::Rasters;
use crate::{Error, Result};

/// The cells above which a run is tiled by default, and the side of the tiles then.
const AUTO_CELLS: usize = 4_000_000;
const AUTO_TILE: usize = 1024;
/// The margin by default (cells), at most half the tile.
const AUTO_MARGIN: usize = 64;

/// The side of the tiles of a run (`tile` as given, 0 for automatic), or `None` for the whole image at once.
pub(crate) fn tile_side(w: usize, h: usize, tile: usize) -> Option<usize> {
    let t = match tile {
        0 if w * h <= AUTO_CELLS => return None,
        0 => AUTO_TILE,
        t => t,
    };
    (t < w.max(h)).then_some(t)
}

/// The margin of a run (`margin` as given, 0 for automatic) for tiles of side `t`.
pub(crate) fn margin_of(t: usize, margin: usize) -> usize {
    if margin > 0 { margin } else { AUTO_MARGIN.min(t / 2).max(1) }
}

/// The seed of tile `k`.
fn tile_seed(seed: u64, k: usize) -> u64 {
    seed ^ (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// The core of tile `k` (labels of the objects of its window, row by row), the window read from `src`.
#[allow(clippy::too_many_arguments)]
fn tile_core(
    src: &dyn Source,
    (w, h): (usize, usize),
    cost: &Cost,
    scale: f64,
    mutual: bool,
    seed: u64,
    max_passes: u32,
    t: usize,
    margin: usize,
    k: usize,
) -> Result<Vec<u32>> {
    let nx = w.div_ceil(t);
    let (r0, c0) = ((k / nx) * t, (k % nx) * t);
    let (r1, c1) = ((r0 + t).min(h), (c0 + t).min(w));
    let (wr0, wc0) = (r0.saturating_sub(margin), c0.saturating_sub(margin));
    let (wr1, wc1) = ((r1 + margin).min(h), (c1 + margin).min(w));
    let ww = wc1 - wc0;
    let sub = src.window(wr0, wr1, wc0, wc1)?;
    let sub_grid = Arc::new(Grid::new(ww, wr1 - wr0));
    let o = merge_level(&sub_grid, &sub, cost, scale, mutual, tile_seed(seed, k), max_passes, None).objects;
    let mut core = Vec::with_capacity((r1 - r0) * (c1 - c0));
    for r in r0..r1 {
        let at = (r - wr0) * ww + (c0 - wc0);
        core.extend_from_slice(&o.labels[at..at + (c1 - c0)]);
    }
    Ok(core)
}

/// The first pass of a run on rasters on disk, as another machine can do it: where the files are, which layers make
/// the image, and the settings of the segmentation. Its tiles are numbered row by row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileJob {
    pub files: Vec<PathBuf>,
    /// The layers of the image, by their number among the layers of the files.
    pub layers: Vec<usize>,
    /// The no-data value of every layer of the files.
    pub nodata: Vec<Option<f64>>,
    pub width: usize,
    pub height: usize,
    pub colour: f64,
    pub compact: f64,
    pub weights: Vec<f64>,
    pub scale: f64,
    pub mutual: bool,
    pub seed: u64,
    pub max_passes: u32,
    pub tile: usize,
    pub margin: usize,
}

impl TileJob {
    /// How many tiles the job has.
    pub fn tiles(&self) -> usize {
        self.width.div_ceil(self.tile) * self.height.div_ceil(self.tile)
    }

    /// The cores of some tiles (by number), in parallel on this machine: what a worker does.
    pub fn cores(&self, tiles: &[usize]) -> Result<Vec<Vec<u32>>> {
        crate::threads::ensure();
        let name = self.files.first().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let mut r = Rasters::open_files(&name, self.files.clone(), None, None)?;
        if r.layers.len() != self.nodata.len() || self.layers.iter().any(|&l| l >= r.layers.len()) {
            return Err(Error::msg(format!("{name}: not the rasters the job was made for")));
        }
        for (l, nd) in r.layers.iter_mut().zip(&self.nodata) {
            l.nodata = *nd;
        }
        let r = Arc::new(r);
        if (r.grid.width, r.grid.height) != (self.width, self.height) {
            return Err(Error::msg(format!("{name}: not the grid the job was made for")));
        }
        let parts = self.layers.iter().map(|&l| Part::Disk(r.clone(), l)).collect();
        let src = Layered::new(parts, self.width, self.height);
        let cost = Cost { colour: self.colour, compact: self.compact, weights: self.weights.clone() };
        let n = self.tiles();
        tiles
            .par_iter()
            .map(|&k| {
                if k >= n {
                    return Err(Error::msg(format!("no tile {k}")));
                }
                let dims = (self.width, self.height);
                tile_core(
                    &src,
                    dims,
                    &cost,
                    self.scale,
                    self.mutual,
                    self.seed,
                    self.max_passes,
                    self.tile,
                    self.margin,
                    k,
                )
            })
            .collect()
    }
}

/// What runs the first pass of the tiles elsewhere (a cluster): the cores of every tile of the job, in order.
pub trait TileCores: Send + Sync {
    fn cores(&self, job: &TileJob) -> Result<Vec<Vec<u32>>>;

    /// The columns of every task of an `objects.features` job, in order of task; `None` when this runner takes no such
    /// job (the features are then worked out here).
    fn features(&self, _job: &super::FeatureJob) -> Result<Option<Vec<super::FeatureColumns>>> {
        Ok(None)
    }
}

/// The image segmented by tiles of side `t` with `margin` cells around each. The objects carry no table. The image
/// is read from `src` by windows (a tile's with its margin) and by strips (the stitching): from disk, never whole.
#[allow(clippy::too_many_arguments)]
pub(crate) fn merge_tiled(
    grid: &Arc<Grid>,
    src: &dyn Source,
    cost: &Cost,
    scale: f64,
    mutual: bool,
    seed: u64,
    max_passes: u32,
    t: usize,
    margin: usize,
    remote: Option<(&dyn TileCores, TileJob)>,
) -> Result<Objects> {
    let (w, h) = (grid.width, grid.height);
    let (nx, ny) = (w.div_ceil(t), h.div_ceil(t));
    crate::threads::ensure();
    // each tile's core, as the labels of the objects of its window (in order of tile, so any thread count is one run),
    // written into one map of labels as it is made (in order of tile), its labels offset by the labels of the tiles
    // before it so no two tiles share one: the cores are never all in memory
    let mut lab = Labels::zeros(w * h);
    let mut off = 0u64;
    let mut put = |k: usize, core: &[u32]| -> Result<()> {
        let (r0, c0) = ((k / nx) * t, (k % nx) * t);
        let cw = (c0 + t).min(w) - c0;
        let most = core.iter().copied().max().unwrap_or(0) as u64;
        if off + most >= u32::MAX as u64 {
            return Err(Error::msg("the image has more pieces of objects than the labels can number (use a bigger scale)"));
        }
        let o = off as u32;
        for (j, row) in core.chunks(cw).enumerate() {
            let at = (r0 + j) * w + c0;
            for (to, &l) in lab[at..at + cw].iter_mut().zip(row) {
                if l != 0 {
                    *to = o + l;
                }
            }
        }
        off += most;
        Ok(())
    };
    match remote {
        Some((runner, job)) => {
            let cores = runner.cores(&job)?;
            if cores.len() != nx * ny {
                return Err(Error::msg(format!("the cluster gave {} tiles of {}", cores.len(), nx * ny)));
            }
            for (k, core) in cores.into_iter().enumerate() {
                put(k, &core)?;
            }
        }
        None => {
            // a few tiles at a time, in parallel (as many as the memory for tiles allows, up to the threads)
            let side = (t + 2 * margin) as u64;
            let per = side.min(w as u64) * side.min(h as u64) * (src.nb() as u64 * 40 + 200);
            let fit = (TILES_BYTES.load(Ordering::Relaxed) / per.max(1)) as usize;
            let step = fit.clamp(1, rayon::current_num_threads());
            for first in (0..nx * ny).step_by(step) {
                let cores = (first..(first + step).min(nx * ny))
                    .into_par_iter()
                    .map(|k| tile_core(src, (w, h), cost, scale, mutual, seed, max_passes, t, margin, k))
                    .collect::<Result<Vec<_>>>()?;
                for (j, core) in cores.iter().enumerate() {
                    put(first + j, core)?;
                }
            }
        }
    }
    let pieces = Objects::from_fn(grid.clone(), |i| lab[i], 0u32, 4);
    drop(lab);
    Ok(merge_level_src(grid, src, cost, scale, mutual, seed, max_passes, Some(&pieces))?.objects)
}
