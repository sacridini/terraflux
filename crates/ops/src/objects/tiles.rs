//! Multiresolution segmentation by tiles (Happ et al. 2010, in the spirit of the OTB's LSMS): each tile is segmented
//! with a margin around it, all tiles in parallel; the objects are then stitched at the tile borders.
//!
//! 1. Each tile's window (its core plus the margin, within the image) is segmented on its own by the ordinary
//!    single-threaded algorithm, with a seed made of the run's seed and the tile's index. The margin gives the objects
//!    near the border the context they would have in the whole image.
//! 2. Only the core of each tile is kept: the cells of the objects of the window that lie in the core, each piece in
//!    its connected parts, are the regions of the second pass.
//! 3. The pieces that touch a tile border are taken apart into single cells ([`dissolve_at_borders`]): each window
//!    segmented the area around a border on its own, so two pieces across it are two unrelated partitions cut by a
//!    straight line, and big pieces do not merge across it (the cost of merging two grows with their size, however
//!    alike they are): the border would stay an object edge (36-44 % of the cells' pairs across it in different
//!    objects against 22-27 % elsewhere; 71 % on a Sentinel-2 clip).
//! 4. The second pass is the ordinary merging over the whole image that starts from the pieces away from the borders
//!    and those cells (the `from` of [`merge_level`]) with the same criterion and scale (and the zones of the whole
//!    image, when there are `bounds`: each tile gets those of its window). The cells grow across the border as they do
//!    anywhere, so no line cuts the objects: the share of cut pairs on the borders is the share elsewhere. A piece far
//!    from a border is at a fixpoint already (its neighbours are in its tile). The cost of this pass is linear in the
//!    cells it has (about 5 % of them, a tile's borders), one thread.
//!
//! Nothing depends on the order the tiles finish in: the result is the same for a seed at any number of threads.
//!
//! The image comes from a [`Source`]: in memory, or rasters on disk. Step 1 reads each tile's window when its turn
//! comes (a few windows in memory, one per thread); step 3 reads the image once more by strips of rows, for the
//! sums of the pieces (count, sum, sum of squares per band), never the whole. Only the labels (4 bytes a cell) are
//! whole in memory. From disk or from memory the objects are the same.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

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
pub(super) fn tile_seed(seed: u64, k: usize) -> u64 {
    seed ^ (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Where tile `k` is: its core (rows `r0..r1`, columns `c0..c1`) and its window, the core plus the margin within the
/// image. The tiles are numbered row by row.
pub(super) struct Tile {
    pub r0: usize,
    pub r1: usize,
    pub c0: usize,
    pub c1: usize,
    pub wr0: usize,
    pub wr1: usize,
    pub wc0: usize,
    pub wc1: usize,
}

impl Tile {
    pub fn new((w, h): (usize, usize), t: usize, margin: usize, k: usize) -> Tile {
        let nx = w.div_ceil(t);
        let (r0, c0) = ((k / nx) * t, (k % nx) * t);
        let (r1, c1) = ((r0 + t).min(h), (c0 + t).min(w));
        Tile {
            r0,
            r1,
            c0,
            c1,
            wr0: r0.saturating_sub(margin),
            wc0: c0.saturating_sub(margin),
            wr1: (r1 + margin).min(h),
            wc1: (c1 + margin).min(w),
        }
    }

    /// The width of the window.
    pub fn width(&self) -> usize {
        self.wc1 - self.wc0
    }

    /// The window of a map of the whole image (`w` wide), row by row.
    pub fn cut<T: Copy>(&self, full: &[T], w: usize) -> Vec<T> {
        let mut out = Vec::with_capacity((self.wr1 - self.wr0) * self.width());
        for r in self.wr0..self.wr1 {
            out.extend_from_slice(&full[r * w + self.wc0..r * w + self.wc1]);
        }
        out
    }

    /// The core of a map of the window (`win`, row by row), row by row.
    pub fn core(&self, win: &[u32]) -> Vec<u32> {
        let ww = self.width();
        let mut core = Vec::with_capacity((self.r1 - self.r0) * (self.c1 - self.c0));
        for r in self.r0..self.r1 {
            let at = (r - self.wr0) * ww + (self.c0 - self.wc0);
            core.extend_from_slice(&win[at..at + (self.c1 - self.c0)]);
        }
        core
    }
}

/// The core of tile `k` (labels of the objects of its window, row by row), the window read from `src`. `zone` gives
/// the zones of a tile's window (empty for none).
#[allow(clippy::too_many_arguments)]
fn tile_core(
    src: &dyn Source,
    dims: (usize, usize),
    cost: &Cost,
    scale: f64,
    mutual: bool,
    seed: u64,
    max_passes: u32,
    t: usize,
    margin: usize,
    k: usize,
    zone: &dyn Fn(&Tile) -> Vec<u64>,
) -> Result<Vec<u32>> {
    let tl = Tile::new(dims, t, margin, k);
    let sub = src.window(tl.wr0, tl.wr1, tl.wc0, tl.wc1)?;
    let sub_grid = Arc::new(Grid::new(tl.width(), tl.wr1 - tl.wr0));
    let z = zone(&tl);
    let o = merge_level(&sub_grid, &sub, cost, scale, mutual, tile_seed(seed, k), max_passes, None, &z).objects;
    Ok(tl.core(&o.labels))
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
    /// The zones no object crosses (`bounds`) of every cell of the image, row by row, numbered from 0; empty for none.
    #[serde(default)]
    pub zones: Vec<u32>,
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
                let zone = |tl: &Tile| -> Vec<u64> {
                    if self.zones.is_empty() {
                        Vec::new()
                    } else {
                        tl.cut(&self.zones, self.width).into_iter().map(u64::from).collect()
                    }
                };
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
                    &zone,
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

/// The tiles of a run in flight: what each needs, and the cores made but not put yet.
struct Run<'a> {
    /// The core of a tile, by its number.
    core: &'a (dyn Fn(usize) -> Result<Vec<u32>> + Sync),
    total: usize,
    /// The most tiles worked at once.
    step: usize,
    state: Mutex<Ordered<'a>>,
}

/// Where the tiles have got to: the next one to put, how many were started and finished, the cores that wait for
/// those before them.
struct Ordered<'a> {
    next: usize,
    started: usize,
    done: usize,
    ready: BTreeMap<usize, Vec<u32>>,
    err: Option<Error>,
    put: &'a mut (dyn FnMut(usize, &[u32]) -> Result<()> + Send),
}

/// Starts tile `k` as a task of the scope. When a tile is done its core is put (with those waiting before it) and the
/// next tiles start, so there is no wait for the others; no task ever blocks (a task that waited could be one that a
/// thread took up while it held the tile the others wait for). A tile starts only while fewer than `step` are at work
/// and the tile before it is less than `2 * step` back from the last one put, so few cores wait in memory.
fn spawn_tile<'s, 'a: 's>(sc: &rayon::Scope<'s>, run: &'s Run<'a>, k: usize) {
    sc.spawn(move |sc| {
        let core = (run.core)(k);
        let mut guard = run.state.lock().unwrap();
        let st = &mut *guard;
        st.done += 1;
        match core {
            Ok(core) => {
                st.ready.insert(k, core);
                while let Some(core) = st.ready.remove(&st.next) {
                    if let Err(e) = (st.put)(st.next, &core) {
                        st.err = Some(e);
                        break;
                    }
                    st.next += 1;
                }
            }
            Err(e) => {
                if st.err.is_none() {
                    st.err = Some(e);
                }
            }
        }
        let mut more = Vec::new();
        while st.err.is_none()
            && st.started < run.total
            && st.started - st.done < run.step
            && st.started < st.next + 2 * run.step
        {
            more.push(st.started);
            st.started += 1;
        }
        drop(guard);
        for k in more {
            spawn_tile(sc, run, k);
        }
    });
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
    zones: &[u64],
) -> Result<Objects> {
    let dims = (grid.width, grid.height);
    let tiles = dims.0.div_ceil(t) * dims.1.div_ceil(t);
    let given = match remote {
        Some((runner, job)) => {
            let cores = runner.cores(&job)?;
            if cores.len() != tiles {
                return Err(Error::msg(format!("the cluster gave {} tiles of {tiles}", cores.len())));
            }
            Some(cores)
        }
        None => None,
    };
    let zone = |tl: &Tile| if zones.is_empty() { Vec::new() } else { tl.cut(zones, dims.0) };
    let core = |k: usize| tile_core(src, dims, cost, scale, mutual, seed, max_passes, t, margin, k, &zone);
    let per_cell = src.nb() as u64 * 40 + 200;
    merge_tiled_with(grid, t, margin, per_cell, 4, given, &core, |pieces| {
        Ok(merge_level_src(grid, src, cost, scale, mutual, seed, max_passes, Some(&pieces), zones)?.objects)
    })
}

/// A run by tiles of side `t` with `margin` cells around each, for any segmenter: `core(k)` segments tile `k` (the
/// labels of the objects of its window that lie in its core, row by row, 0 for none, 1 to the count) and `stitch` makes
/// the objects of the image from the pieces (the cores put together, each in its parts connected by `conn` cells).
/// `given` has the cores already (made elsewhere), all of them in order. A tile's window takes `per_cell` bytes a
/// cell, which with `TILES_BYTES` limits the tiles at once. The cores are put in order of tile, so the pieces never
/// depend on the order the tiles finish in, and so on the number of threads.
#[allow(clippy::too_many_arguments)]
pub(crate) fn merge_tiled_with(
    grid: &Arc<Grid>,
    t: usize,
    margin: usize,
    per_cell: u64,
    conn: usize,
    given: Option<Vec<Vec<u32>>>,
    core: &(dyn Fn(usize) -> Result<Vec<u32>> + Sync),
    stitch: impl FnOnce(Objects) -> Result<Objects>,
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
    match given {
        Some(cores) => {
            for (k, c) in cores.into_iter().enumerate() {
                put(k, &c)?;
            }
        }
        None => {
            // As many tiles at once as the memory for tiles allows (up to the threads), each worker taking the next as soon
            // as it is done (see `spawn_tile`). A core is put into the labels when all those before it have been, so the
            // labels are the same whatever the order the tiles finish in.
            let side = (t + 2 * margin) as u64;
            let per = side.min(w as u64) * side.min(h as u64) * per_cell;
            let fit = (TILES_BYTES.load(Ordering::Relaxed) / per.max(1)) as usize;
            let step = fit.clamp(1, rayon::current_num_threads());
            let total = nx * ny;
            let first = step.min(total);
            let run = Run {
                core,
                total,
                step,
                state: Mutex::new(Ordered {
                    next: 0,
                    started: first,
                    done: 0,
                    ready: BTreeMap::new(),
                    err: None,
                    put: &mut put,
                }),
            };
            rayon::scope(|sc| {
                for k in 0..first {
                    spawn_tile(sc, &run, k);
                }
            });
            if let Some(e) = run.state.into_inner().unwrap().err {
                return Err(e);
            }
        }
    }
    let pieces = Objects::from_fn(grid.clone(), |i| lab[i], 0u32, conn);
    drop(lab);
    let pieces = dissolve_at_borders(grid, pieces, t, conn);
    stitch(pieces)
}

/// The pieces that touch a tile border, in single cells again. Each window segmented the area around the border on its
/// own, so the pieces on the two sides of it are two unrelated partitions cut by a straight line, and pieces that big
/// do not merge across it (the merge cost of two big pieces grows with their size, however alike): the border stays an
/// object edge. The whole image's objects grew from cells, so no line cut them. With those pieces as cells the
/// stitching grows them across the border as the merging does anywhere. The pieces away from the borders are kept
/// (they are the speed of the tiles).
fn dissolve_at_borders(grid: &Arc<Grid>, pieces: Objects, t: usize, conn: usize) -> Objects {
    let (w, h) = (grid.width, grid.height);
    let mut touch = vec![false; pieces.count + 1];
    let mut any = false;
    let mut mark = |i: usize| {
        let l = pieces.labels[i] as usize;
        any |= l != 0;
        touch[l] = true;
    };
    // the cells beside a border line, on both sides of it: the whole rows next to a border row, the columns beside the
    // border columns in the others
    for r in 0..h {
        let row = (r % t == 0 && r > 0) || ((r + 1) % t == 0 && r + 1 < h);
        if row {
            (0..w).for_each(|c| mark(r * w + c));
        } else {
            // (the two columns of each border: only those are visited)
            for c in (t..w).step_by(t) {
                mark(r * w + c - 1);
                mark(r * w + c);
            }
        }
    }
    touch[0] = false;
    if !any {
        return pieces;
    }
    // (a cell's number is above every piece's: u64 holds both)
    Objects::from_fn(
        grid.clone(),
        |i| {
            let l = pieces.labels[i];
            if l != 0 && touch[l as usize] { (1u64 << 32) + i as u64 } else { l as u64 }
        },
        0u64,
        conn,
    )
}
