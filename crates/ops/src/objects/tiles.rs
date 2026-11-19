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

use std::sync::Arc;

use rayon::prelude::*;
use terraflux_raster::Grid;

use super::big::Source;
use super::multiresolution::{Cost, merge_level, merge_level_src};
use super::{NO_REGION, Objects};
use crate::Result;

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
) -> Result<Objects> {
    let (w, h) = (grid.width, grid.height);
    let (nx, ny) = (w.div_ceil(t), h.div_ceil(t));
    crate::threads::ensure();
    // each tile's core, as the labels of the objects of its window (in order of tile, so any thread count is one run)
    let cores: Vec<Vec<u32>> = (0..nx * ny)
        .into_par_iter()
        .map(|k| -> Result<Vec<u32>> {
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
        })
        .collect::<Result<_>>()?;
    let mut region = vec![NO_REGION; w * h];
    for (k, core) in cores.iter().enumerate() {
        let (r0, c0) = ((k / nx) * t, (k % nx) * t);
        let cw = (c0 + t).min(w) - c0;
        for (j, row) in core.chunks(cw).enumerate() {
            let at = (r0 + j) * w + c0;
            for (to, &l) in region[at..at + cw].iter_mut().zip(row) {
                if l != 0 {
                    *to = ((k as u64) << 32) | l as u64;
                }
            }
        }
    }
    drop(cores);
    let pieces = Objects::from_regions(grid.clone(), &region, 4);
    drop(region);
    Ok(merge_level_src(grid, src, cost, scale, mutual, seed, max_passes, Some(&pieces))?.objects)
}
