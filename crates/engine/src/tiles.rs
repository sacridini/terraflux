//! Maps cut in tiles: the tiling of a grid, the sources maps are read from by windows (files, or layers the
//! engine computes), and the layers it writes tile by tile (in memory, or on disk for maps larger than
//! memory).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use terraflux_raster::tiff::{Reader, Window};
use terraflux_raster::{Grid, Raster};

use serde::{Deserialize, Serialize};

use crate::pass::{Local, Part, Runner, Task};
use crate::{Error, Result};

/// One tile: its place in the tiling and its cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    pub index: usize,
    pub row: usize,
    pub col: usize,
    pub x0: usize,
    pub y0: usize,
    pub width: usize,
    pub height: usize,
}

impl Tile {
    /// The tile and `halo` cells around it.
    pub fn window(&self, halo: usize) -> Window {
        Window {
            x0: self.x0 as i64 - halo as i64,
            y0: self.y0 as i64 - halo as i64,
            width: self.width + 2 * halo,
            height: self.height + 2 * halo,
        }
    }

    pub fn cells(&self) -> usize {
        self.width * self.height
    }
}

/// A grid cut in square tiles (smaller at the right and bottom edges), and what runs the passes over them
/// (this machine's threads, or workers elsewhere).
#[derive(Clone)]
pub struct Tiling {
    pub width: usize,
    pub height: usize,
    pub size: usize,
    pub runner: Arc<dyn Runner>,
}

impl std::fmt::Debug for Tiling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tiling({} x {} by {}, {:?})", self.width, self.height, self.size, self.runner)
    }
}

impl PartialEq for Tiling {
    fn eq(&self, o: &Tiling) -> bool {
        (self.width, self.height, self.size) == (o.width, o.height, o.size)
    }
}

impl Tiling {
    /// Tiles run by this machine's threads.
    pub fn new(width: usize, height: usize, size: usize) -> Self {
        Tiling { width, height, size: size.max(1), runner: Arc::new(Local) }
    }

    /// The same tiles run by `runner`.
    pub fn with_runner(mut self, runner: Arc<dyn Runner>) -> Self {
        self.runner = runner;
        self
    }

    /// A pass over every tile: the sum of the tiles' parts.
    pub fn run(&self, sources: &[&dyn Source], task: &Task) -> Result<Part> {
        self.runner.run(self, sources, task)
    }

    pub fn across(&self) -> usize {
        self.width.div_ceil(self.size)
    }

    pub fn down(&self) -> usize {
        self.height.div_ceil(self.size)
    }

    pub fn tiles(&self) -> Vec<Tile> {
        let mut out = Vec::with_capacity(self.across() * self.down());
        for row in 0..self.down() {
            for col in 0..self.across() {
                let (x0, y0) = (col * self.size, row * self.size);
                out.push(Tile {
                    index: out.len(),
                    row,
                    col,
                    x0,
                    y0,
                    width: self.size.min(self.width - x0),
                    height: self.size.min(self.height - y0),
                });
            }
        }
        out
    }
}

/// Somewhere a map is read from, window by window (NaN outside the map and at missing cells).
pub trait Source: Send + Sync {
    fn grid(&self) -> &Grid;
    fn read(&self, win: Window) -> Result<Vec<f64>>;
    /// Where another machine finds the same map (none for maps in memory).
    fn spec(&self) -> Option<Spec> {
        None
    }
    /// The layer, when the source is one (passes write layers).
    fn as_layer(&self) -> Option<&Layer> {
        None
    }
}

/// Where a map is, for another machine: a band of a file, or a layer of tiles on disk (shared storage).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)] // a few per model
pub enum Spec {
    File { path: PathBuf, band: usize, nodata: Option<f64> },
    Layer { dir: PathBuf, grid: Grid, size: usize },
}

/// Open the sources of some specs (the bands of a file share its reading).
pub fn open_specs(specs: &[Spec]) -> Result<Vec<Box<dyn Source>>> {
    let mut files: std::collections::HashMap<&Path, (Arc<OpenFile>, Raster)> = std::collections::HashMap::new();
    for s in specs {
        if let Spec::File { path, .. } = s
            && !files.contains_key(path.as_path())
        {
            let n = specs.iter().filter(|o| matches!(o, Spec::File { path: q, .. } if q == path)).count();
            files.insert(path.as_path(), OpenFile::open(path, n)?);
        }
    }
    specs
        .iter()
        .map(|s| -> Result<Box<dyn Source>> {
            Ok(match s {
                Spec::File { path, band, nodata } => {
                    let (file, meta) = &files[path.as_path()];
                    if *band >= meta.bands.len() {
                        return Err(Error::Msg(format!("{} has no band {band}", path.display())));
                    }
                    Box::new(FileSource {
                        file: file.clone(),
                        band: *band,
                        nodata: *nodata,
                        grid: meta.grid.clone(),
                        name: meta.bands[*band].name.clone(),
                    })
                }
                Spec::Layer { dir, grid, size } => {
                    Box::new(Layer::open(grid.clone(), Tiling::new(grid.width, grid.height, *size), dir.clone()))
                }
            })
        })
        .collect()
}

/// An open GeoTIFF shared by the sources of its bands: readers kept for reuse (one per reading thread, the
/// file's directory parsed once) and, when several of its bands are read, the bands of the windows read
/// lately, so that reading every band of a window decodes it once.
struct OpenFile {
    path: PathBuf,
    readers: Mutex<Vec<Reader>>,
    /// bands read by sources (the windows' bands are kept until each of them took its own)
    sharing: usize,
    cache: Mutex<VecDeque<Cached>>,
}

struct Cached {
    win: Window,
    bands: Vec<Option<Vec<f64>>>,
    left: usize,
}

/// Windows kept at most (bands read by no one go first): two per reading thread.
fn cached() -> usize {
    2 * rayon::current_num_threads()
}

impl OpenFile {
    fn open(path: &Path, sharing: usize) -> Result<(Arc<OpenFile>, Raster)> {
        let r = Reader::open(path).map_err(|e| Error::Msg(format!("{}: {e}", path.display())))?;
        let meta = r.meta.clone();
        let f = OpenFile {
            path: path.to_path_buf(),
            readers: Mutex::new(vec![r]),
            sharing,
            cache: Mutex::new(VecDeque::new()),
        };
        Ok((Arc::new(f), meta))
    }

    fn read_all(&self, win: Window) -> Result<Vec<Vec<f64>>> {
        let pooled = self.readers.lock().unwrap().pop();
        let mut r = match pooled {
            Some(r) => r,
            None => {
                let first = self.readers.lock().unwrap().first().map(Reader::reopen);
                match first {
                    Some(r) => r,
                    None => Reader::open(&self.path),
                }
                .map_err(|e| Error::Msg(format!("{}: {e}", self.path.display())))?
            }
        };
        let bands = r.window(win).map_err(|e| Error::Msg(format!("{}: {e}", self.path.display())));
        self.readers.lock().unwrap().push(r);
        bands
    }

    fn read(&self, band: usize, win: Window) -> Result<Vec<f64>> {
        if self.sharing > 1 {
            let mut cache = self.cache.lock().unwrap();
            if let Some(i) = cache.iter().position(|c| c.win == win && c.bands[band].is_some()) {
                let v = cache[i].bands[band].take().unwrap();
                cache[i].left -= 1;
                if cache[i].left == 0 {
                    cache.remove(i);
                }
                return Ok(v);
            }
        }
        let mut bands = self.read_all(win)?;
        let v = std::mem::take(&mut bands[band]);
        if self.sharing > 1 {
            let mut kept: Vec<Option<Vec<f64>>> = bands.into_iter().map(Some).collect();
            kept[band] = None;
            let mut cache = self.cache.lock().unwrap();
            cache.push_back(Cached { win, bands: kept, left: self.sharing - 1 });
            while cache.len() > cached() {
                cache.pop_front();
            }
        }
        Ok(v)
    }
}

/// A band of a GeoTIFF file.
pub struct FileSource {
    file: Arc<OpenFile>,
    band: usize,
    nodata: Option<f64>,
    grid: Grid,
    name: Option<String>,
}

impl FileSource {
    fn band_index(path: &Path, meta: &Raster, band: Option<&str>) -> Result<usize> {
        let names = meta.band_names();
        match band {
            None => Ok(0),
            Some(b) => names
                .iter()
                .position(|n| n == b)
                .or_else(|| b.parse::<usize>().ok().filter(|i| *i < names.len()))
                .ok_or_else(|| Error::Msg(format!("{} has no band {b:?}", path.display()))),
        }
    }

    /// The band of a name or a 0-based index; `nodata` replaces the file's.
    pub fn open(path: &Path, band: Option<&str>, nodata: Option<f64>) -> Result<FileSource> {
        let (file, meta) = OpenFile::open(path, 1)?;
        let band = Self::band_index(path, &meta, band)?;
        Ok(FileSource {
            file,
            band,
            nodata: nodata.or(meta.nodata),
            grid: meta.grid.clone(),
            name: meta.bands[band].name.clone(),
        })
    }

    /// The band's name in the file.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Every band of a file, by name (each window decoded once for all of them).
    pub fn bands(path: &Path, nodata: Option<f64>) -> Result<Vec<(String, FileSource)>> {
        Self::some_bands(path, nodata, None)
    }

    /// The bands of a file of the given names (all of them when `None`), in the file's order.
    pub fn some_bands(path: &Path, nodata: Option<f64>, keep: Option<&[String]>) -> Result<Vec<(String, FileSource)>> {
        let meta = Reader::open(path).map_err(|e| Error::Msg(format!("{}: {e}", path.display())))?.meta;
        let names: Vec<String> =
            meta.bands.iter().enumerate().map(|(i, b)| b.name.clone().unwrap_or_else(|| i.to_string())).collect();
        let chosen: Vec<usize> = (0..names.len()).filter(|&i| keep.is_none_or(|k| k.contains(&names[i]))).collect();
        let (file, meta) = OpenFile::open(path, chosen.len())?;
        Ok(chosen
            .into_iter()
            .map(|i| {
                let s = FileSource {
                    file: file.clone(),
                    band: i,
                    nodata: nodata.or(meta.nodata),
                    grid: meta.grid.clone(),
                    name: meta.bands[i].name.clone(),
                };
                (names[i].clone(), s)
            })
            .collect())
    }
}

impl Source for FileSource {
    fn grid(&self) -> &Grid {
        &self.grid
    }

    fn spec(&self) -> Option<Spec> {
        Some(Spec::File { path: self.file.path.clone(), band: self.band, nodata: self.nodata })
    }

    fn read(&self, win: Window) -> Result<Vec<f64>> {
        let mut v = self.file.read(self.band, win)?;
        if let Some(nd) = self.nodata.filter(|x| !x.is_nan()) {
            for x in v.iter_mut() {
                if *x == nd {
                    *x = f64::NAN;
                }
            }
        }
        Ok(v)
    }
}

/// A whole raster band in memory as a source (small maps, tests).
pub struct MemorySource {
    grid: Grid,
    data: Arc<Vec<f64>>,
}

impl MemorySource {
    pub fn new(grid: Grid, data: Arc<Vec<f64>>) -> Self {
        MemorySource { grid, data }
    }

    pub fn band(r: &Raster, i: usize) -> Self {
        MemorySource { grid: r.grid.clone(), data: Arc::new(r.bands[i].masked(r.nodata)) }
    }
}

/// The cells of a window from a whole map in memory.
pub fn window_of(data: &[f64], width: usize, height: usize, win: Window) -> Vec<f64> {
    let mut out = vec![f64::NAN; win.width * win.height];
    for y in 0..win.height {
        let gy = win.y0 + y as i64;
        if gy < 0 || gy >= height as i64 {
            continue;
        }
        for x in 0..win.width {
            let gx = win.x0 + x as i64;
            if gx < 0 || gx >= width as i64 {
                continue;
            }
            out[y * win.width + x] = data[gy as usize * width + gx as usize];
        }
    }
    out
}

impl Source for MemorySource {
    fn grid(&self) -> &Grid {
        &self.grid
    }

    fn read(&self, win: Window) -> Result<Vec<f64>> {
        Ok(window_of(&self.data, self.grid.width, self.grid.height, win))
    }
}

/// The tiles of the layers on disk read lately, decoded, shared by every layer: reading a window with a halo
/// touches up to nine tiles, and the passes of a step read each tile many times. Its size is
/// `TERRAFLUX_CACHE_MB` (2048 by default); the tiles read least lately go first.
struct TileCache {
    budget: usize,
    inner: Mutex<CacheInner>,
}

type Cached32 = (Arc<Vec<f32>>, u64);

#[derive(Default)]
struct CacheInner {
    /// (layer, tile) -> (cells, when last read)
    map: std::collections::HashMap<(u64, usize), Cached32>,
    bytes: usize,
    clock: u64,
}

fn tile_cache() -> &'static TileCache {
    static CACHE: std::sync::OnceLock<TileCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        let mb = std::env::var("TERRAFLUX_CACHE_MB").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(2048);
        TileCache { budget: mb << 20, inner: Mutex::new(CacheInner::default()) }
    })
}

impl TileCache {
    fn get(&self, key: (u64, usize)) -> Option<Arc<Vec<f32>>> {
        let mut c = self.inner.lock().unwrap();
        c.clock += 1;
        let now = c.clock;
        c.map.get_mut(&key).map(|(v, used)| {
            *used = now;
            v.clone()
        })
    }

    fn put(&self, key: (u64, usize), v: Arc<Vec<f32>>) {
        let size = v.len() * 4;
        if size > self.budget {
            return;
        }
        let mut c = self.inner.lock().unwrap();
        c.clock += 1;
        let now = c.clock;
        if let Some((old, _)) = c.map.insert(key, (v, now)) {
            c.bytes -= old.len() * 4;
        }
        c.bytes += size;
        while c.bytes > self.budget {
            let Some((&oldest, _)) = c.map.iter().min_by_key(|(_, (_, used))| *used) else { break };
            let (gone, _) = c.map.remove(&oldest).unwrap();
            c.bytes -= gone.len() * 4;
        }
    }

    fn forget(&self, layer: u64) {
        let mut c = self.inner.lock().unwrap();
        let keys: Vec<(u64, usize)> = c.map.keys().filter(|k| k.0 == layer).copied().collect();
        for k in keys {
            let (gone, _) = c.map.remove(&k).unwrap();
            c.bytes -= gone.len() * 4;
        }
    }
}

fn next_layer_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// A layer the engine computes, kept tile by tile as f32 (exact for classes; probabilities to 7 digits): in
/// memory, or compressed (LZ4) on disk.
pub struct Layer {
    grid: Grid,
    tiling: Tiling,
    tiles: Vec<RwLock<Option<Arc<Vec<f32>>>>>,
    disk: Option<PathBuf>,
    /// tiles written to disk
    on_disk: Mutex<Vec<bool>>,
    /// the layer's number in the cache of tiles
    id: u64,
    /// Whether this process made it ([`Layer::new`]): its folder then goes with it (a step's layers lived for
    /// the run only, and stayed on disk: hundreds of GB of a long run); not one written by others.
    owned: bool,
}

impl Drop for Layer {
    fn drop(&mut self) {
        if let Some(d) = &self.disk {
            tile_cache().forget(self.id);
            if self.owned {
                let _ = std::fs::remove_dir_all(d);
                // the step's folders it was in, once empty (`step_001/probability`)
                for up in d.ancestors().skip(1).take(2) {
                    if std::fs::remove_dir(up).is_err() {
                        break;
                    }
                }
            }
        }
    }
}

impl Layer {
    /// A layer of the grid cut by `tiling`; on disk in `disk` (a folder of its own) when given.
    pub fn new(grid: Grid, tiling: Tiling, disk: Option<PathBuf>) -> Result<Layer> {
        let n = tiling.across() * tiling.down();
        if let Some(d) = &disk {
            // the tiles of an earlier run go
            if d.exists() {
                std::fs::remove_dir_all(d)?;
            }
            std::fs::create_dir_all(d)?;
        }
        Ok(Layer {
            grid,
            tiling,
            tiles: (0..n).map(|_| RwLock::new(None)).collect(),
            disk,
            on_disk: Mutex::new(vec![false; n]),
            id: next_layer_id(),
            owned: true,
        })
    }

    /// A layer on disk written by others (other processes or machines): its tiles are read as they appear.
    pub fn open(grid: Grid, tiling: Tiling, dir: PathBuf) -> Layer {
        let n = tiling.across() * tiling.down();
        Layer {
            grid,
            tiling,
            tiles: (0..n).map(|_| RwLock::new(None)).collect(),
            disk: Some(dir),
            on_disk: Mutex::new(vec![false; n]),
            id: next_layer_id(),
            owned: false,
        }
    }

    pub fn tiling(&self) -> &Tiling {
        &self.tiling
    }

    /// Whether a tile is on disk (written here, or by another process).
    fn stored(&self, index: usize, path: &Path) -> bool {
        let mut on = self.on_disk.lock().unwrap();
        if !on[index] && path.exists() {
            on[index] = true;
        }
        on[index]
    }

    fn file(&self, i: usize) -> Option<PathBuf> {
        self.disk.as_ref().map(|d| d.join(format!("{i}.bin")))
    }

    /// Store a tile's cells (row by row, the tile's size).
    pub fn put(&self, tile: &Tile, values: Vec<f32>) -> Result<()> {
        if values.len() != tile.cells() {
            return Err(Error::Msg("a tile's cells differ in number from the tile".into()));
        }
        match self.file(tile.index) {
            Some(path) => {
                // LZ4: fast to write and read again (the steps' layers live only for the run)
                let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
                let tmp = path.with_extension(format!("{}.part", std::process::id()));
                std::fs::write(&tmp, lz4_flex::compress_prepend_size(&bytes))?;
                std::fs::rename(&tmp, &path)?;
                self.on_disk.lock().unwrap()[tile.index] = true;
                tile_cache().put((self.id, tile.index), Arc::new(values));
            }
            None => *self.tiles[tile.index].write().unwrap() = Some(Arc::new(values)),
        }
        Ok(())
    }

    /// A tile's cells (None if never stored).
    pub fn get(&self, index: usize) -> Result<Option<Arc<Vec<f32>>>> {
        match self.file(index) {
            Some(path) => {
                if !self.stored(index, &path) {
                    return Ok(None);
                }
                if let Some(v) = tile_cache().get((self.id, index)) {
                    return Ok(Some(v));
                }
                let raw = std::fs::read(&path)?;
                let bytes = lz4_flex::decompress_size_prepended(&raw)
                    .map_err(|e| Error::Msg(format!("{}: {e}", path.display())))?;
                let v: Arc<Vec<f32>> = Arc::new(
                    bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect(),
                );
                tile_cache().put((self.id, index), v.clone());
                Ok(Some(v))
            }
            None => Ok(self.tiles[index].read().unwrap().clone()),
        }
    }

    pub fn complete(&self) -> bool {
        match &self.disk {
            Some(_) => (0..self.tiles.len()).all(|i| self.stored(i, &self.file(i).unwrap())),
            None => self.tiles.iter().all(|t| t.read().unwrap().is_some()),
        }
    }
}

impl Source for Layer {
    fn grid(&self) -> &Grid {
        &self.grid
    }

    fn spec(&self) -> Option<Spec> {
        self.disk.as_ref().map(|d| Spec::Layer { dir: d.clone(), grid: self.grid.clone(), size: self.tiling.size })
    }

    fn as_layer(&self) -> Option<&Layer> {
        Some(self)
    }

    fn read(&self, win: Window) -> Result<Vec<f64>> {
        let mut out = vec![f64::NAN; win.width * win.height];
        let t = &self.tiling;
        let (x1, y1) = (win.x0 + win.width as i64, win.y0 + win.height as i64);
        let (gx0, gy0) = (win.x0.max(0) as usize, win.y0.max(0) as usize);
        let gx1 = x1.clamp(0, t.width as i64) as usize;
        let gy1 = y1.clamp(0, t.height as i64) as usize;
        if gx0 >= gx1 || gy0 >= gy1 {
            return Ok(out);
        }
        for row in gy0 / t.size..gy1.div_ceil(t.size) {
            for col in gx0 / t.size..gx1.div_ceil(t.size) {
                let index = row * t.across() + col;
                let Some(v) = self.get(index)? else {
                    return Err(Error::Msg(format!("tile {index} was not computed")));
                };
                let (tx0, ty0) = (col * t.size, row * t.size);
                let tw = t.size.min(t.width - tx0);
                let th = t.size.min(t.height - ty0);
                for y in ty0.max(gy0)..(ty0 + th).min(gy1) {
                    for x in tx0.max(gx0)..(tx0 + tw).min(gx1) {
                        out[(y as i64 - win.y0) as usize * win.width + (x as i64 - win.x0) as usize] =
                            v[(y - ty0) * tw + (x - tx0)] as f64;
                    }
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_cover_the_grid_once() {
        let t = Tiling::new(10, 7, 4);
        let tiles = t.tiles();
        assert_eq!(tiles.len(), 3 * 2);
        assert_eq!(tiles.iter().map(Tile::cells).sum::<usize>(), 70);
        assert_eq!((tiles[5].width, tiles[5].height), (2, 3));
    }

    fn check_layer(disk: Option<PathBuf>) {
        let g = Grid::new(10, 7);
        let data: Vec<f64> = (0..70).map(f64::from).collect();
        let tiling = Tiling::new(10, 7, 4);
        let layer = Layer::new(g.clone(), tiling.clone(), disk).unwrap();
        for t in tiling.tiles() {
            let v = window_of(&data, 10, 7, t.window(0)).into_iter().map(|x| x as f32).collect();
            layer.put(&t, v).unwrap();
        }
        assert!(layer.complete());
        let win = Window { x0: -2, y0: 1, width: 9, height: 8 };
        let got = layer.read(win).unwrap();
        let want = window_of(&data, 10, 7, win);
        for (a, b) in got.iter().zip(&want) {
            assert!(a == b || (a.is_nan() && b.is_nan()));
        }
    }

    #[test]
    fn layers_read_windows_across_tiles() {
        check_layer(None);
        let d = std::env::temp_dir().join(format!("terraflux-layer-{}", std::process::id()));
        check_layer(Some(d.clone()));
        let _ = std::fs::remove_dir_all(d);
    }
}
