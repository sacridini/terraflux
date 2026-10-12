//! A map on the screen, of any size: zoom with the wheel around the pointer, pan by dragging, a double click
//! fits it; the cell under the pointer is read out (row, column, map coordinates, value).
//!
//! The map is drawn from a pyramid of tiles (256 x 256 pixels; at level `L` a pixel is every `2^L`-th cell):
//! only the tiles in view, at the level of the screen's resolution, are made, by threads, the last asked first
//! (tiles that left the view while waiting are let go); until a tile comes, the coarsest tile already made
//! over its place is shown, stretched. A map in memory and rasters on disk (read where and as finely as the
//! tiles need, see `terraflux_raster::image`) are drawn the same way, so that a map of billions of cells
//! shows at once and moves as smoothly as a small one.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};

use egui::{Color32, Pos2, Rect, Sense, TextureHandle, TextureOptions, Ui, Vec2};
use terraflux_ops::Map;
use terraflux_ops::files::Rasters;
use terraflux_raster::tiff::Window;

use crate::palette::{Coloring, image, image_f32};

/// The side of a tile, pixels.
pub const TILE: usize = 256;
/// The tiles kept (textures and values): 128 MB of each at most.
const KEPT: usize = 512;

/// The cell under the pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hover {
    pub row: usize,
    pub col: usize,
    pub x: f64,
    pub y: f64,
    pub value: f64,
}

/// What a view draws: a map in memory, or a layer of rasters on disk.
#[derive(Clone)]
pub enum Source {
    Map(Map),
    Rasters(Arc<Rasters>, usize),
}

impl From<&Map> for Source {
    fn from(m: &Map) -> Source {
        Source::Map(m.clone())
    }
}

/// What tells sources apart: a map's cells, a file's band.
pub type SourceKey = (usize, usize);

impl Source {
    pub fn key(&self) -> SourceKey {
        match self {
            Source::Map(m) => (Arc::as_ptr(&m.data) as usize, 0),
            Source::Rasters(r, l) => {
                let layer = &r.layers[*l];
                (Arc::as_ptr(&r.images[layer.image]) as usize, layer.band)
            }
        }
    }

    pub fn grid(&self) -> &terraflux_raster::Grid {
        match self {
            Source::Map(m) => &m.grid,
            Source::Rasters(r, l) => r.grid_of(*l),
        }
    }

    pub fn size(&self) -> (usize, usize) {
        let g = self.grid();
        (g.width, g.height)
    }

    /// The level at which the whole map fits a tile.
    fn top(&self) -> u32 {
        let (w, h) = self.size();
        let mut l = 0;
        while (TILE << l) < w.max(h) {
            l += 1;
        }
        l
    }

    /// A tile's values: at level `level`, every `2^level`-th cell from the tile's corner; NaN outside the map.
    pub fn tile(&self, level: u32, tx: usize, ty: usize) -> Result<Vec<f32>, String> {
        let step = 1usize << level;
        let (x0, y0) = (tx * TILE * step, ty * TILE * step);
        match self {
            Source::Map(m) => {
                let (w, h) = (m.grid.width, m.grid.height);
                let mut v = vec![f32::NAN; TILE * TILE];
                for i in 0..TILE {
                    let y = y0 + i * step;
                    if y >= h {
                        break;
                    }
                    let row = &m.data[y * w..(y + 1) * w];
                    for j in 0..TILE {
                        let x = x0 + j * step;
                        if x >= w {
                            break;
                        }
                        v[i * TILE + j] = row[x] as f32;
                    }
                }
                Ok(v)
            }
            Source::Rasters(r, l) => {
                let win = Window { x0: x0 as i64, y0: y0 as i64, width: TILE * step, height: TILE * step };
                r.sample(*l, win, step).map_err(|e| e.to_string())
            }
        }
    }

    /// A cell's value, when it can be had at once (a map in memory).
    fn value(&self, x: usize, y: usize) -> Option<f64> {
        match self {
            Source::Map(m) => m.data.get(y * m.grid.width + x).copied(),
            Source::Rasters(..) => None,
        }
    }
}

/// A tile: its source, level and place.
type TileKey = (SourceKey, u32, usize, usize);

struct Job {
    key: TileKey,
    source: Source,
    coloring: Coloring,
    colouring: u64,
}

struct Made {
    key: TileKey,
    source: Source,
    values: Result<Arc<Vec<f32>>, String>,
    image: Option<egui::ColorImage>,
    colouring: u64,
}

/// The threads that make tiles: a queue the view replaces at every frame (what is in view, the most needed
/// first), the tiles being made.
struct Workers {
    queue: Arc<(Mutex<VecDeque<Job>>, Condvar)>,
    started: Arc<Mutex<HashSet<(TileKey, u64)>>>,
    done: Receiver<Made>,
}

impl Workers {
    fn new(ctx: egui::Context) -> Workers {
        let queue: Arc<(Mutex<VecDeque<Job>>, Condvar)> = Arc::new((Mutex::new(VecDeque::new()), Condvar::new()));
        let started: Arc<Mutex<HashSet<(TileKey, u64)>>> = Arc::new(Mutex::new(HashSet::new()));
        let (tx, done): (Sender<Made>, _) = channel();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 8);
        for k in 0..threads {
            let (queue, started, tx, ctx) = (queue.clone(), started.clone(), tx.clone(), ctx.clone());
            let _ = std::thread::Builder::new().name(format!("terraflux-tiles-{k}")).spawn(move || {
                loop {
                    let job = {
                        let (lock, cv) = &*queue;
                        let mut q = lock.lock().unwrap();
                        loop {
                            if let Some(j) = q.pop_front() {
                                break j;
                            }
                            q = cv.wait(q).unwrap();
                        }
                    };
                    started.lock().unwrap().insert((job.key, job.colouring));
                    let (_, level, tx_, ty) = job.key;
                    let values = job.source.tile(level, tx_, ty).map(Arc::new);
                    let image = values.as_ref().ok().map(|v| image_f32(v, TILE, TILE, &job.coloring));
                    started.lock().unwrap().remove(&(job.key, job.colouring));
                    let made = Made { key: job.key, source: job.source, values, image, colouring: job.colouring };
                    if tx.send(made).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            });
        }
        Workers { queue, started, done }
    }
}

/// A map of the world under the data (web tiles, warped by GDAL onto the data's grid): free services.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Basemap {
    #[default]
    None,
    EsriImagery,
    EsriTopographic,
    OpenStreetMap,
    OpenTopoMap,
    CartoLight,
}

impl Basemap {
    pub const ALL: [Basemap; 6] = [
        Basemap::None,
        Basemap::EsriImagery,
        Basemap::EsriTopographic,
        Basemap::OpenStreetMap,
        Basemap::OpenTopoMap,
        Basemap::CartoLight,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Basemap::None => "None",
            Basemap::EsriImagery => "Satellite (Esri)",
            Basemap::EsriTopographic => "Topographic (Esri)",
            Basemap::OpenStreetMap => "OpenStreetMap",
            Basemap::OpenTopoMap => "OpenTopoMap",
            Basemap::CartoLight => "Light (Carto)",
        }
    }

    /// The tiles' address, with `${z}`, `${x}` and `${y}`; the deepest zoom level.
    fn url(self) -> Option<(&'static str, u32)> {
        Some(match self {
            Basemap::None => return None,
            Basemap::EsriImagery => {
                ("https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/${z}/${y}/${x}", 19)
            }
            Basemap::EsriTopographic => (
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Topo_Map/MapServer/tile/${z}/${y}/${x}",
                19,
            ),
            Basemap::OpenStreetMap => ("https://tile.openstreetmap.org/${z}/${x}/${y}.png", 19),
            Basemap::OpenTopoMap => ("https://tile.opentopomap.org/${z}/${x}/${y}.png", 17),
            Basemap::CartoLight => ("https://basemaps.cartocdn.com/light_all/${z}/${x}/${y}.png", 19),
        })
    }

    /// Whose map it is, written on it as the services ask.
    pub fn attribution(self) -> &'static str {
        match self {
            Basemap::None => "",
            Basemap::EsriImagery => "Esri, Maxar, Earthstar Geographics, and the GIS User Community",
            Basemap::EsriTopographic => "Esri, HERE, Garmin, USGS, NGA, and others",
            Basemap::OpenStreetMap => "© OpenStreetMap contributors",
            Basemap::OpenTopoMap => "© OpenStreetMap contributors, SRTM; style © OpenTopoMap (CC-BY-SA)",
            Basemap::CartoLight => "© OpenStreetMap contributors © CARTO",
        }
    }

    /// The service as GDAL opens it, its tiles kept in the user's cache.
    fn service(self) -> Option<String> {
        let (url, levels) = self.url()?;
        let root = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let cache = root.join("Terraflux").join("tiles").join(format!("{self:?}"));
        Some(terraflux_gdal::tile_service(url, levels, &cache))
    }
}

/// The CRS of a grid as GDAL takes it: the WKT GDAL read, else the EPSG code of the GeoTIFF keys.
fn crs_of(g: &terraflux_raster::Grid) -> Option<String> {
    g.geo.wkt.clone().or_else(|| g.geo.epsg().map(|c| format!("EPSG:{c}")))
}

/// A tile of a basemap: the service, the grid it is warped onto (a hash of its CRS and transform), the level
/// (a tile spans 256 x 2^level cells; below 0, finer than the cells) and its place.
type BaseKey = (Basemap, u64, i32, i64, i64);

struct BaseJob {
    key: BaseKey,
    service: Arc<String>,
    crs: Arc<String>,
    extent: [f64; 4],
}

struct BaseMade {
    key: BaseKey,
    image: Result<egui::ColorImage, String>,
}

/// The threads that fetch and warp basemap tiles (the web is slow: never in the data's threads).
struct BaseWorkers {
    queue: Arc<(Mutex<VecDeque<BaseJob>>, Condvar)>,
    started: Arc<Mutex<HashSet<BaseKey>>>,
    done: Receiver<BaseMade>,
}

impl BaseWorkers {
    fn new(ctx: egui::Context) -> BaseWorkers {
        let queue: Arc<(Mutex<VecDeque<BaseJob>>, Condvar)> = Arc::new((Mutex::new(VecDeque::new()), Condvar::new()));
        let started: Arc<Mutex<HashSet<BaseKey>>> = Arc::new(Mutex::new(HashSet::new()));
        let (tx, done) = channel();
        for k in 0..4 {
            let (queue, started, tx, ctx) = (queue.clone(), started.clone(), tx.clone(), ctx.clone());
            let _ = std::thread::Builder::new().name(format!("terraflux-basemap-{k}")).spawn(move || {
                loop {
                    let job: BaseJob = {
                        let (lock, cv) = &*queue;
                        let mut q = lock.lock().unwrap();
                        loop {
                            if let Some(j) = q.pop_front() {
                                break j;
                            }
                            q = cv.wait(q).unwrap();
                        }
                    };
                    started.lock().unwrap().insert(job.key);
                    let image = terraflux_gdal::warp_image(&job.service, &job.crs, job.extent, TILE, TILE).map(|px| {
                        let pixels =
                            px.into_iter().map(|[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a)).collect();
                        egui::ColorImage::new([TILE, TILE], pixels)
                    });
                    started.lock().unwrap().remove(&job.key);
                    if tx.send(BaseMade { key: job.key, image }).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            });
        }
        BaseWorkers { queue, started, done }
    }
}

/// The basemap under the data and how the data shows over it.
#[derive(Default)]
struct Base {
    tiles: HashMap<BaseKey, (TextureHandle, u64)>,
    workers: Option<BaseWorkers>,
    asked: HashSet<BaseKey>,
    error: Option<String>,
}

/// A tile made: its values, its texture in a colouring, when it was last shown.
struct Tile {
    values: Arc<Vec<f32>>,
    texture: Option<(u64, TextureHandle)>,
    used: u64,
    /// the source, held so that its key is not another's while the tile is kept
    _source: Source,
}

#[derive(Default)]
pub struct MapView {
    tiles: HashMap<TileKey, Tile>,
    workers: Option<Workers>,
    /// the tiles asked of the workers and not come yet
    asked: HashSet<(TileKey, u64)>,
    clock: u64,
    /// the size of the map shown (cells), to fit it again when another comes
    size: (usize, usize),
    /// screen points per cell, and the cell at the centre of the view
    zoom: f32,
    center: Vec2,
    fitted: bool,
    error: Option<String>,
    /// The map of the world under the data; how transparent the data is over it (0: opaque), or hidden.
    pub basemap: Basemap,
    pub transparency: f32,
    pub hide_data: bool,
    base: Base,
}

impl MapView {
    /// Fit the map in the view at the next frame.
    pub fn fit(&mut self) {
        self.fitted = false;
    }

    /// The tiles kept.
    pub fn textures(&self) -> usize {
        self.tiles.len()
    }

    /// Draw the basemap tiles over the view (the finest kept over a tile still to come, stretched), asking the
    /// missing ones of its threads; nothing when the grid has no CRS.
    fn draw_base(&mut self, ctx: &egui::Context, painter: &egui::Painter, rect: Rect, grid: &terraflux_raster::Grid) {
        let Some(service) = self.basemap.service() else { return };
        let Some(crs) = crs_of(grid) else {
            self.base.error = Some("the map has no CRS: no basemap".into());
            return;
        };
        let workers = self.base.workers.get_or_insert_with(|| BaseWorkers::new(ctx.clone()));
        for m in workers.done.try_iter().collect::<Vec<_>>() {
            self.base.asked.remove(&m.key);
            match m.image {
                Ok(img) => {
                    self.clock += 1;
                    let t = ctx.load_texture(format!("base-{}", self.clock), img, TextureOptions::LINEAR);
                    self.base.tiles.insert(m.key, (t, self.clock));
                }
                // a tile the service has not (beyond its projection's bounds): none there
                Err(e) => {
                    if self.base.tiles.is_empty() {
                        self.base.error = Some(e);
                    }
                }
            }
        }
        let gkey = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            crs.hash(&mut h);
            grid.transform.iter().for_each(|v| v.to_bits().hash(&mut h));
            h.finish()
        };
        let (w, hgt) = (grid.width as f64, grid.height as f64);
        let mut top = 0;
        while ((TILE as f64) * 2f64.powi(top)) < w.max(hgt) {
            top += 1;
        }
        let ppp = ctx.pixels_per_point();
        let cells_per_pixel = 1.0 / (self.zoom as f64 * ppp as f64);
        let level = (cells_per_pixel.log2().floor() as i32).clamp(-6, top);
        let span = |l: i32| TILE as f64 * 2f64.powi(l);
        let (a, b) = (self.to_cell(rect, rect.min), self.to_cell(rect, rect.max));
        let s = span(level);
        // the whole view, around the data too (a few hundred tiles at most)
        let (x0, y0) = ((a.x as f64 / s).floor() as i64, (a.y as f64 / s).floor() as i64);
        let (x1, y1) = ((b.x as f64 / s).floor() as i64, (b.y as f64 / s).floor() as i64);
        if (x1 - x0 + 1) * (y1 - y0 + 1) > 400 {
            return;
        }
        let t = grid.transform;
        let screen = |l: i32, tx: i64, ty: i64| -> Rect {
            let sp = span(l) as f32;
            let min = rect.center() + (Vec2::new(tx as f32 * sp, ty as f32 * sp) - self.center) * self.zoom;
            Rect::from_min_size(min, Vec2::splat(sp * self.zoom))
        };
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        self.clock += 1;
        let clock = self.clock;
        let mut wanted: Vec<(f32, BaseJob)> = Vec::new();
        let (service, crs) = (Arc::new(service), Arc::new(crs));
        for ty in y0..=y1.max(y0 - 1) {
            for tx in x0..=x1.max(x0 - 1) {
                let key = (self.basemap, gkey, level, tx, ty);
                let r = screen(level, tx, ty);
                if let Some((tex, used)) = self.base.tiles.get_mut(&key) {
                    *used = clock;
                    painter.image(tex.id(), r, full, Color32::WHITE);
                    continue;
                }
                // the cells the tile spans, in the grid's coordinates (a north-up grid)
                let (cx0, cy0) = (tx as f64 * s, ty as f64 * s);
                let (xa, xb) = (t[0] + cx0 * t[1], t[0] + (cx0 + s) * t[1]);
                let (ya, yb) = (t[3] + cy0 * t[5], t[3] + (cy0 + s) * t[5]);
                let extent = [xa.min(xb), ya.min(yb), xa.max(xb), ya.max(yb)];
                wanted.push((
                    (r.center() - rect.center()).length(),
                    BaseJob { key, service: service.clone(), crs: crs.clone(), extent },
                ));
                for up in level + 1..=top {
                    let shift = (up - level) as u32;
                    let (ax, ay) = (tx >> shift, ty >> shift);
                    if let Some((tex, used)) = self.base.tiles.get_mut(&(self.basemap, gkey, up, ax, ay)) {
                        *used = clock;
                        let f = 1.0 / (1u32 << shift) as f32;
                        let (u, v) = ((tx - (ax << shift)) as f32 * f, (ty - (ay << shift)) as f32 * f);
                        painter.image(
                            tex.id(),
                            r,
                            Rect::from_min_size(Pos2::new(u, v), Vec2::splat(f)),
                            Color32::WHITE,
                        );
                        break;
                    }
                }
            }
        }
        wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
        let workers = self.base.workers.as_ref().unwrap();
        let started = workers.started.lock().unwrap().clone();
        let (lock, cv) = &*workers.queue;
        let mut q = lock.lock().unwrap();
        q.clear();
        self.base.asked.retain(|k| started.contains(k));
        for (_, j) in wanted {
            if !started.contains(&j.key) {
                self.base.asked.insert(j.key);
                q.push_back(j);
            }
        }
        cv.notify_all();
        drop(q);
        if self.base.tiles.len() > KEPT {
            let mut by_age: Vec<(u64, BaseKey)> = self.base.tiles.iter().map(|(k, (_, u))| (*u, *k)).collect();
            by_age.sort_unstable_by_key(|(u, _)| *u);
            for (_, k) in by_age.into_iter().take(self.base.tiles.len() - KEPT) {
                self.base.tiles.remove(&k);
            }
        }
    }

    /// The basemap tiles kept; why there is no basemap, if so.
    pub fn base_tiles(&self) -> (usize, Option<String>) {
        (self.base.tiles.len(), self.base.error.clone())
    }

    /// Whether basemap tiles are being fetched.
    pub fn base_busy(&self) -> bool {
        !self.base.asked.is_empty()
    }

    /// Screen points per cell.
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Whether tiles of the view are being made.
    pub fn busy(&self) -> bool {
        !self.asked.is_empty()
    }

    /// The cell under a screen point.
    fn to_cell(&self, rect: Rect, p: Pos2) -> Vec2 {
        self.center + (p - rect.center()) / self.zoom
    }

    fn take_made(&mut self, ctx: &egui::Context) {
        let Some(w) = &self.workers else { return };
        let made: Vec<Made> = w.done.try_iter().collect();
        for m in made {
            self.asked.remove(&(m.key, m.colouring));
            match m.values {
                Err(e) => self.error = Some(e),
                Ok(values) => {
                    self.clock += 1;
                    let texture = m.image.map(|img| {
                        (m.colouring, ctx.load_texture(format!("tile-{}", self.clock), img, TextureOptions::NEAREST))
                    });
                    match self.tiles.get_mut(&m.key) {
                        Some(t) => {
                            t.values = values;
                            if texture.is_some() {
                                t.texture = texture;
                            }
                        }
                        None => {
                            self.tiles.insert(m.key, Tile { values, texture, used: self.clock, _source: m.source });
                        }
                    }
                }
            }
        }
        if self.tiles.len() > KEPT {
            let mut by_age: Vec<(u64, TileKey)> = self.tiles.iter().map(|(k, t)| (t.used, *k)).collect();
            by_age.sort_unstable();
            for (_, k) in by_age.into_iter().take(self.tiles.len() - KEPT) {
                self.tiles.remove(&k);
            }
        }
    }

    /// The value of a cell from the finest tile kept over it (a map in memory: the cell itself).
    fn value_at(&self, source: &Source, x: usize, y: usize) -> f64 {
        if let Some(v) = source.value(x, y) {
            return v;
        }
        let key = source.key();
        for level in 0..=source.top() {
            let span = TILE << level;
            if let Some(t) = self.tiles.get(&(key, level, x / span, y / span)) {
                let (i, j) = ((y % span) >> level, (x % span) >> level);
                return t.values.get(i * TILE + j).map_or(f64::NAN, |&v| v as f64);
            }
        }
        f64::NAN
    }

    pub fn show(&mut self, ui: &mut Ui, source: &Source, coloring: &Coloring) -> Option<Hover> {
        let ctx = ui.ctx().clone();
        if self.workers.is_none() {
            self.workers = Some(Workers::new(ctx.clone()));
        }
        self.take_made(&ctx);
        let (w, h) = source.size();
        if self.size != (w, h) {
            self.size = (w, h);
            self.fitted = false;
            self.error = None;
        }
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        if w == 0 || h == 0 {
            return None;
        }
        if !self.fitted || response.double_clicked() {
            self.zoom = (rect.width() / w as f32).min(rect.height() / h as f32).max(1e-6);
            self.center = Vec2::new(w as f32 / 2.0, h as f32 / 2.0);
            self.fitted = true;
        }
        if response.dragged() {
            self.center -= response.drag_delta() / self.zoom;
        }
        if let Some(p) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            let pinch = ui.input(|i| i.zoom_delta());
            let factor = pinch * (scroll / 400.0).exp();
            if factor != 1.0 {
                // keep the cell under the pointer in place
                let under = self.to_cell(rect, p);
                self.zoom = (self.zoom * factor).clamp(1e-6, 64.0);
                self.center = under - (p - rect.center()) / self.zoom;
            }
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        // the world under the data, then the data over it as transparent as asked
        if self.base.tiles.values().len() > 0 {
            self.base.error = None;
        }
        if self.basemap != Basemap::None {
            self.draw_base(&ctx, &painter, rect, source.grid());
        }
        let tint = Color32::from_white_alpha(((1.0 - self.transparency.clamp(0.0, 1.0)) * 255.0).round() as u8);
        let show_data = !self.hide_data;
        // the level: a tile's pixel about a pixel of the screen
        let ppp = ctx.pixels_per_point();
        let top = source.top();
        let cells_per_pixel = 1.0 / (self.zoom * ppp);
        let level = (cells_per_pixel.max(1.0).log2().floor() as u32).min(top);
        let src = source.key();
        let colouring = coloring.key();
        let screen = |level: u32, tx: usize, ty: usize| -> Rect {
            let span = (TILE << level) as f32;
            let min = rect.center() + (Vec2::new(tx as f32 * span, ty as f32 * span) - self.center) * self.zoom;
            Rect::from_min_size(min, Vec2::splat(span * self.zoom))
        };
        let (a, b) = (self.to_cell(rect, rect.min), self.to_cell(rect, rect.max));
        let range = |level: u32| {
            let span = (TILE << level) as f32;
            let x0 = (a.x / span).floor().max(0.0) as usize;
            let y0 = (a.y / span).floor().max(0.0) as usize;
            let x1 = ((b.x / span).floor() as i64).min(w.div_ceil(TILE << level) as i64 - 1);
            let y1 = ((b.y / span).floor() as i64).min(h.div_ceil(TILE << level) as i64 - 1);
            (x0, y0, x1, y1)
        };
        let (x0, y0, x1, y1) = range(level);
        self.clock += 1;
        let clock = self.clock;
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let mut wanted: Vec<(f32, TileKey)> = Vec::new();
        let mut recolour = 8;
        // the top tiles over the view first: something to show at once
        let (tx0, ty0, tx1, ty1) = range(top);
        for ty in ty0..=(ty1.max(0) as usize) {
            for tx in tx0..=(tx1.max(0) as usize) {
                let k = (src, top, tx, ty);
                if self.tiles.get(&k).is_none_or(|t| t.texture.as_ref().is_none_or(|(c, _)| *c != colouring)) {
                    wanted.push((-1.0, k));
                }
            }
        }
        if x1 >= x0 as i64 && y1 >= y0 as i64 {
            for ty in y0..=y1 as usize {
                for tx in x0..=x1 as usize {
                    let k = (src, level, tx, ty);
                    let r = screen(level, tx, ty);
                    // a tile kept in another colouring is coloured again here (a few per frame)
                    if let Some(t) = self.tiles.get_mut(&k)
                        && t.texture.as_ref().is_none_or(|(c, _)| *c != colouring)
                        && recolour > 0
                    {
                        recolour -= 1;
                        let img = image_f32(&t.values, TILE, TILE, coloring);
                        t.texture = Some((
                            colouring,
                            ctx.load_texture(format!("tile-r-{clock}-{tx}-{ty}"), img, TextureOptions::NEAREST),
                        ));
                    }
                    let drawn = match self.tiles.get_mut(&k) {
                        Some(t) if t.texture.as_ref().is_some_and(|(c, _)| *c == colouring) => {
                            t.used = clock;
                            if show_data {
                                painter.image(t.texture.as_ref().unwrap().1.id(), r, full, tint);
                            }
                            true
                        }
                        _ => false,
                    };
                    if drawn {
                        continue;
                    }
                    let d = (r.center() - rect.center()).length();
                    wanted.push((d, k));
                    // meanwhile the finest coarser tile kept over it, stretched
                    for up in level + 1..=top {
                        let shift = up - level;
                        let (ax, ay) = (tx >> shift, ty >> shift);
                        if let Some(t) = self.tiles.get_mut(&(src, up, ax, ay))
                            && let Some((c, tex)) = &t.texture
                            && *c == colouring
                        {
                            t.used = clock;
                            let f = 1.0 / (1u32 << shift) as f32;
                            let (u, v) = ((tx - (ax << shift)) as f32 * f, (ty - (ay << shift)) as f32 * f);
                            let uv = Rect::from_min_size(Pos2::new(u, v), Vec2::splat(f));
                            if show_data {
                                painter.image(tex.id(), r, uv, tint);
                            }
                            break;
                        }
                    }
                }
            }
        }
        // the workers' queue: what the view needs, the nearest the centre first; the rest let go
        wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some(wk) = &self.workers {
            let started = wk.started.lock().unwrap().clone();
            let (lock, cv) = &*wk.queue;
            let mut q = lock.lock().unwrap();
            q.clear();
            self.asked.retain(|a| started.contains(a));
            for (_, k) in wanted {
                if started.contains(&(k, colouring)) {
                    continue;
                }
                q.push_back(Job { key: k, source: source.clone(), coloring: coloring.clone(), colouring });
                self.asked.insert((k, colouring));
            }
            cv.notify_all();
        }
        // whose map the basemap is, as the services ask; why there is none
        if self.basemap != Basemap::None {
            let text = self.base.error.clone().unwrap_or_else(|| self.basemap.attribution().to_string());
            let at = rect.right_bottom() - Vec2::new(6.0, 4.0);
            let galley = painter.layout_no_wrap(text, egui::FontId::proportional(10.5), Color32::WHITE);
            let back = Rect::from_min_size(at - galley.size(), galley.size()).expand2(Vec2::new(4.0, 2.0));
            painter.rect_filled(back, 2.0, Color32::from_black_alpha(140));
            painter.galley(at - galley.size(), galley, Color32::WHITE);
        }
        if let Some(e) = &self.error {
            painter.text(
                rect.left_top() + Vec2::splat(8.0),
                egui::Align2::LEFT_TOP,
                e,
                egui::FontId::proportional(12.0),
                ui.visuals().error_fg_color,
            );
        }
        let p = response.hover_pos()?;
        let c = self.to_cell(rect, p);
        let g = &source.grid().transform;
        let inside = c.x >= 0.0 && c.y >= 0.0 && c.x < w as f32 && c.y < h as f32;
        // the coordinates under the pointer, in the view's corner (and the cell's value over the map)
        {
            let (px, py) = (f64::from(c.x), f64::from(c.y));
            let (x, y) = (g[0] + px * g[1] + py * g[2], g[3] + px * g[4] + py * g[5]);
            // degrees to a hundred-thousandth, metres to a centimetre
            let digits = if g[1].abs() < 0.01 { 6 } else { 2 };
            let mut text = format!("x {x:.digits$}   y {y:.digits$}");
            if inside {
                let v = self.value_at(source, c.x as usize, c.y as usize);
                let v = if v.is_nan() { "no value".to_string() } else { terraflux_ops::value::class_label(v) };
                text.push_str(&format!("   ·   row {}  column {}   ·   {v}", c.y as usize, c.x as usize));
            }
            let at = rect.left_bottom() + Vec2::new(6.0, -4.0);
            let galley = painter.layout_no_wrap(text, egui::FontId::monospace(11.0), Color32::WHITE);
            let back =
                Rect::from_min_size(at - Vec2::new(0.0, galley.size().y), galley.size()).expand2(Vec2::new(4.0, 2.0));
            painter.rect_filled(back, 2.0, Color32::from_black_alpha(150));
            painter.galley(at - Vec2::new(0.0, galley.size().y), galley, Color32::WHITE);
        }
        if !inside {
            return None;
        }
        let (col, row) = (c.x as usize, c.y as usize);
        let (cx, cy) = (col as f64 + 0.5, row as f64 + 0.5);
        Some(Hover {
            row,
            col,
            x: g[0] + cx * g[1] + cy * g[2],
            y: g[3] + cx * g[4] + cy * g[5],
            value: self.value_at(source, col, row),
        })
    }
}

/// Small images of maps, to see the maps of a stack at once.
#[derive(Default)]
pub struct Thumbs {
    list: Vec<(Arc<Vec<f64>>, u64, TextureHandle)>,
    made: usize,
}

impl Thumbs {
    /// The largest side of a small image, pixels.
    pub const SIDE: usize = 160;

    /// The small image of a map, made once.
    pub fn get(&mut self, ctx: &egui::Context, map: &Map, coloring: &Coloring) -> (egui::TextureId, Vec2) {
        let key = coloring.key();
        if let Some((_, _, t)) = self.list.iter().find(|(d, k, _)| Arc::ptr_eq(d, &map.data) && *k == key) {
            return (t.id(), t.size_vec2());
        }
        let (img, _) = image(&map.data, map.grid.width, map.grid.height, Self::SIDE, coloring);
        self.made += 1;
        let t = ctx.load_texture(format!("thumb-{}", self.made), img, TextureOptions::LINEAR);
        let out = (t.id(), t.size_vec2());
        self.list.push((map.data.clone(), key, t));
        if self.list.len() > 512 {
            self.list.drain(..1);
        }
        out
    }
}

/// The lines of a legend of classes: a colour, a name and the cells, the classes in a row of the same name in one
/// line in the colour of the first (the values of a density in its classes: Rare (0-9%)...; the steps of a ramp
/// in its bands: 10-19%...).
pub fn legend_lines(
    classes: &[(f64, Color32)],
    counts: &[(f64, usize)],
    names: &dyn Fn(f64) -> String,
) -> Vec<(Color32, String, usize)> {
    let mut lines: Vec<(Color32, String, usize)> = Vec::new();
    for (v, col) in classes {
        let n = counts.iter().find(|(k, _)| k == v).map_or(0, |(_, n)| *n);
        let name = names(*v);
        match lines.last_mut() {
            Some((_, last, m)) if *last == name => *m += n,
            _ => lines.push((*col, name, n)),
        }
    }
    lines
}

/// The legend of a colouring: a swatch and the name of each class (with its cells), or the range.
pub fn legend(ui: &mut Ui, coloring: &Coloring, counts: &[(f64, usize)], names: &dyn Fn(f64) -> String) {
    match coloring {
        Coloring::Classes(c) => {
            for (col, name, n) in legend_lines(c, counts, names) {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                    ui.painter().rect_filled(r, 2.0, col);
                    // the count on the right, the name in what is left (cut short when long)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(n.to_string()).weak().monospace());
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(egui::Label::new(name).truncate());
                        });
                    });
                });
            }
        }
        Coloring::Continuous { min, max, ramp } => {
            let (r, _) = ui.allocate_exact_size(Vec2::new(160.0, 12.0), Sense::hover());
            let n = 32;
            for i in 0..n {
                let x0 = r.left() + r.width() * i as f32 / n as f32;
                let x1 = r.left() + r.width() * (i + 1) as f32 / n as f32;
                ui.painter().rect_filled(
                    Rect::from_min_max(Pos2::new(x0, r.top()), Pos2::new(x1, r.bottom())),
                    0.0,
                    ramp.colour(i as f32 / (n - 1) as f32),
                );
            }
            ui.label(format!("{min:.4} to {max:.4}"));
        }
    }
}

/// The cells of each class of a map (the classes 0 to 255 counted in a table, the others, up to 256 of them,
/// in a list).
pub fn class_counts(values: &[f64]) -> Vec<(f64, usize)> {
    let mut table = [0usize; 256];
    let mut out: Vec<(f64, usize)> = Vec::new();
    for &v in values {
        if v.is_nan() {
            continue;
        }
        if (0.0..256.0).contains(&v) && v.fract() == 0.0 {
            table[v as usize] += 1;
            continue;
        }
        match out.iter_mut().find(|(k, _)| *k == v) {
            Some(e) => e.1 += 1,
            None => {
                if out.len() < 256 {
                    out.push((v, 1));
                }
            }
        }
    }
    out.extend(table.iter().enumerate().filter(|(_, n)| **n > 0).map(|(v, n)| (v as f64, *n)));
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts() {
        assert_eq!(class_counts(&[2.0, 1.0, f64::NAN, 2.0]), vec![(1.0, 1), (2.0, 2)]);
        assert_eq!(class_counts(&[0.5, -1.0, 300.0, 0.5, 0.0]), vec![(-1.0, 1), (0.0, 1), (0.5, 2), (300.0, 1)]);
    }
}
