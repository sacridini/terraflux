//! A map on the screen, of any size: zoom with the wheel around the pointer, pan by dragging, a double click
//! fits it, B puts the satellite's imagery under it (or takes it away), space hides the map (or shows it again);
//! the cell under the pointer is read out (row, column, map coordinates, value).
//!
//! The map is drawn from a pyramid of tiles (256 x 256 pixels; at level `L` a pixel is every `2^L`-th cell):
//! only the tiles in view, at the level of the screen's resolution, are made, by threads, the last asked first
//! (tiles that left the view while waiting are let go); until a tile comes, the coarsest tile already made
//! over its place is shown, stretched. A map in memory and rasters on disk (read where and as finely as the
//! tiles need, see `terraflux_raster::image`) are drawn the same way, so that a map of billions of cells
//! shows at once and moves as smoothly as a small one. Vector layers are drawn over it the same way, as tiles
//! threads rasterize ([`crate::vector_tiles`]); with no map under them (a grid only), the view shows vectors alone.

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
/// The closest zoom, screen points per cell: the cells' values are written in them.
pub const MAX_ZOOM: f32 = 64.0;

/// A cell's value short enough for its square: a whole number as it is, else four significant digits (an
/// exponent for the very large and the very small).
pub fn cell_text(v: f64) -> String {
    let a = v.abs();
    if v.fract() == 0.0 && a < 1e7 {
        return format!("{}", v as i64);
    }
    if a >= 1e5 || a < 1e-3 {
        return format!("{v:.2e}");
    }
    let whole = if a < 1.0 { 1 } else { a.log10().floor() as usize + 1 };
    let t = format!("{v:.*}", 4usize.saturating_sub(whole).max(1));
    let t = t.trim_end_matches('0');
    t.strip_suffix('.').unwrap_or(t).to_string()
}

/// The cell under the pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hover {
    pub row: usize,
    pub col: usize,
    pub x: f64,
    pub y: f64,
    pub value: f64,
    /// The feature of a vector layer under the pointer: (layer, feature).
    pub feature: Option<(usize, usize)>,
}

/// What a view draws: a map in memory, a layer of rasters on disk, or no map (a grid for vector layers).
#[derive(Clone)]
pub enum Source {
    Map(Map),
    Rasters(Arc<Rasters>, usize),
    Blank(Arc<terraflux_raster::Grid>),
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
            Source::Blank(g) => (Arc::as_ptr(g) as usize, usize::MAX),
        }
    }

    pub fn grid(&self) -> &terraflux_raster::Grid {
        match self {
            Source::Map(m) => &m.grid,
            Source::Rasters(r, l) => r.grid_of(*l),
            Source::Blank(g) => g,
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
            Source::Blank(_) => Ok(vec![f32::NAN; TILE * TILE]),
        }
    }

    /// A cell's value, when it can be had at once (a map in memory).
    fn value(&self, x: usize, y: usize) -> Option<f64> {
        match self {
            Source::Map(m) => m.data.get(y * m.grid.width + x).copied(),
            Source::Rasters(..) | Source::Blank(_) => None,
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

/// A map of the world under the data (web tiles drawn on the data's grid, see [`crate::basemap`]): free services.
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
    pub(crate) fn url(self) -> Option<(&'static str, u32)> {
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
}

/// The CRS of a grid as GDAL takes it: the WKT GDAL read, else the EPSG code of the GeoTIFF keys.
fn crs_of(g: &terraflux_raster::Grid) -> Option<String> {
    g.geo.wkt.clone().or_else(|| g.geo.epsg().map(|c| format!("EPSG:{c}")))
}

/// A map drawn over another: its colours and how opaque it is (0 to 1).
#[derive(Clone)]
pub struct RasterLayer {
    pub source: Source,
    pub coloring: Coloring,
    pub opacity: f32,
}

/// Where a source's cells are on the view's grid: a cell (x, y) at `offset + (x, y) * scale` cells of the view.
#[derive(Clone, Copy)]
struct Place {
    offset: Vec2,
    scale: Vec2,
}

/// A grid's place on another: by their transforms, when they are of the same coordinate system (or neither has
/// one) and neither is rotated.
fn place_on(g: &terraflux_raster::Grid, base: &terraflux_raster::Grid) -> Option<Place> {
    let (t, b) = (g.transform, base.transform);
    if t[2] != 0.0 || t[4] != 0.0 || b[2] != 0.0 || b[4] != 0.0 {
        return None;
    }
    match (crs_of(g), crs_of(base)) {
        (Some(x), Some(y)) if x != y && !terraflux_gdal::same_crs(&x, &y).unwrap_or(false) => return None,
        (Some(_), None) | (None, Some(_)) => return None,
        _ => {}
    }
    let offset = Vec2::new(((t[0] - b[0]) / b[1]) as f32, ((t[3] - b[3]) / b[5]) as f32);
    let scale = Vec2::new((t[1] / b[1]) as f32, (t[5] / b[5]) as f32);
    Some(Place { offset, scale })
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
    base: crate::basemap::Layer,
    /// The vector layers drawn over the map.
    pub vectors: crate::vector_tiles::VectorTiles,
    /// The grid (its transform's bits and size) and how many of its cells the world spans (none: no CRS).
    world: Option<(u64, Option<f64>)>,
    /// The cell clicked (row, column): its values along a stack's layers are shown beside the map.
    pub picked: Option<(usize, usize)>,
    /// The trace of the cell picked asked for from its menu (a right click): `true` to have the assistant explain
    /// it.
    pub trace_request: Option<bool>,
    /// Where the view was drawn last.
    rect: Option<Rect>,
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

    /// Draw the basemap under the view (web tiles drawn as meshes by the GPU, see [`crate::basemap`]); nothing
    /// when the grid has no CRS.
    fn draw_base(&mut self, ctx: &egui::Context, painter: &egui::Painter, rect: Rect, grid: &terraflux_raster::Grid) {
        if self.basemap == Basemap::None {
            return;
        }
        const NO_CRS: &str = "the map has no CRS: no basemap";
        let Some(crs) = crs_of(grid) else {
            self.base.error = Some(NO_CRS.into());
            return;
        };
        // (a map without one shown before: its word goes)
        if self.base.error.as_deref() == Some(NO_CRS) {
            self.base.error = None;
        }
        let view = crate::basemap::View {
            transform: grid.transform,
            rect,
            center: self.center,
            zoom: self.zoom,
            pixels_per_point: ctx.pixels_per_point(),
        };
        self.base.draw(ctx, painter, self.basemap, &crs, &view);
    }

    /// The basemap tiles kept; why there is no basemap, if so.
    pub fn base_tiles(&self) -> (usize, Option<String>) {
        self.base.tiles()
    }

    /// Whether basemap tiles are being fetched.
    pub fn base_busy(&self) -> bool {
        self.base.busy()
    }

    /// Screen points per cell.
    /// Where the view was drawn last, on the screen.
    pub fn rect(&self) -> Option<Rect> {
        self.rect
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Whether tiles of the view (of the map or of its vector layers) are being made.
    pub fn busy(&self) -> bool {
        !self.asked.is_empty() || self.vectors.busy()
    }

    /// How far out the view zooms: the whole world in the view when the grid has a coordinate system, else the
    /// map 32 points across.
    fn least_zoom(&mut self, rect: Rect, grid: &terraflux_raster::Grid, w: usize, h: usize) -> f32 {
        let key = {
            use std::hash::{Hash, Hasher};
            let mut k = std::collections::hash_map::DefaultHasher::new();
            grid.transform.iter().for_each(|v| v.to_bits().hash(&mut k));
            (w, h).hash(&mut k);
            crs_of(grid).hash(&mut k);
            k.finish()
        };
        if self.world.is_none_or(|(k, _)| k != key) {
            let cells = crs_of(grid)
                .and_then(|c| crate::basemap::cell_metres(&c, grid.transform, w, h))
                .map(crate::basemap::world_cells);
            self.world = Some((key, cells));
        }
        match self.world.and_then(|(_, c)| c) {
            Some(cells) => (rect.width().max(rect.height()) as f64 / cells) as f32,
            None => 32.0 / w.max(h).max(1) as f32,
        }
    }

    /// Draw a source's tiles in view, placed on the view's grid (the finest kept over a tile still to come,
    /// stretched; a few tiles kept in another colouring coloured again); the tiles missing asked for.
    #[allow(clippy::too_many_arguments)]
    fn draw_source(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        rect: Rect,
        source: &Source,
        coloring: &Coloring,
        tint: Color32,
        place: Place,
        show: bool,
        wanted: &mut Vec<(f32, Job)>,
    ) {
        let (w, h) = source.size();
        if w == 0 || h == 0 {
            return;
        }
        let ppp = ctx.pixels_per_point();
        let top = source.top();
        // the level: a tile's pixel about a pixel of the screen
        let cells_per_pixel = 1.0 / (self.zoom * place.scale.x.abs().min(place.scale.y.abs()) * ppp);
        let level = (cells_per_pixel.max(1.0).log2().floor() as u32).min(top);
        let src = source.key();
        let colouring = coloring.key();
        let (center, zoom) = (self.center, self.zoom);
        let screen = |level: u32, tx: usize, ty: usize| -> Rect {
            let span = (TILE << level) as f32;
            let a = place.offset + Vec2::new(tx as f32 * span * place.scale.x, ty as f32 * span * place.scale.y);
            let b = a + Vec2::new(span * place.scale.x, span * place.scale.y);
            Rect::from_two_pos(rect.center() + (a - center) * zoom, rect.center() + (b - center) * zoom)
        };
        // the view in the source's cells
        let to_source =
            |p: Vec2| Vec2::new((p.x - place.offset.x) / place.scale.x, (p.y - place.offset.y) / place.scale.y);
        let (pa, pb) = (to_source(self.to_cell(rect, rect.min)), to_source(self.to_cell(rect, rect.max)));
        let (a, b) = (Vec2::new(pa.x.min(pb.x), pa.y.min(pb.y)), Vec2::new(pa.x.max(pb.x), pa.y.max(pb.y)));
        let range = |level: u32| {
            let span = (TILE << level) as f32;
            let x0 = (a.x / span).floor().max(0.0) as usize;
            let y0 = (a.y / span).floor().max(0.0) as usize;
            let x1 = ((b.x / span).floor() as i64).min(w.div_ceil(TILE << level) as i64 - 1);
            let y1 = ((b.y / span).floor() as i64).min(h.div_ceil(TILE << level) as i64 - 1);
            (x0, y0, x1, y1)
        };
        let (x0, y0, x1, y1) = range(level);
        let clock = self.clock;
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let job = |key: TileKey| Job { key, source: source.clone(), coloring: coloring.clone(), colouring };
        let mut recolour = 8;
        // the top tiles over the view first: something to show at once
        let (tx0, ty0, tx1, ty1) = range(top);
        for ty in ty0..=(ty1.max(0) as usize) {
            for tx in tx0..=(tx1.max(0) as usize) {
                let k = (src, top, tx, ty);
                if self.tiles.get(&k).is_none_or(|t| t.texture.as_ref().is_none_or(|(c, _)| *c != colouring)) {
                    wanted.push((-1.0, job(k)));
                }
            }
        }
        if x1 < x0 as i64 || y1 < y0 as i64 {
            return;
        }
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
                        if show {
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
                wanted.push((d, job(k)));
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
                        if show {
                            painter.image(tex.id(), r, uv, tint);
                        }
                        break;
                    }
                }
            }
        }
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
        self.show_layers(ui, source, coloring, &[])
    }

    /// The map and vector layers over it (the last on top); the feature under the pointer named with its
    /// attributes.
    pub fn show_layers(
        &mut self,
        ui: &mut Ui,
        source: &Source,
        coloring: &Coloring,
        overlays: &[crate::vector_tiles::Overlay],
    ) -> Option<Hover> {
        self.show_stacked(ui, source, coloring, &[], overlays)
    }

    /// A map with other maps over it (in their order, each as opaque as asked, placed by its coordinates) and
    /// vector layers over them all.
    pub fn show_stacked(
        &mut self,
        ui: &mut Ui,
        source: &Source,
        coloring: &Coloring,
        rasters: &[RasterLayer],
        overlays: &[crate::vector_tiles::Overlay],
    ) -> Option<Hover> {
        let ctx = ui.ctx().clone();
        let blank = matches!(source, Source::Blank(_));
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
        self.rect = Some(rect);
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
        // B over the map: the satellite's imagery under it, or none
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        if response.hovered() && !typing && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::B)) {
            self.basemap = if self.basemap == Basemap::None { Basemap::EsriImagery } else { Basemap::None };
        }
        // space over the map: the map hidden (what is under it and over it seen alone), or shown again
        if response.hovered() && !typing && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Space)) {
            self.hide_data = !self.hide_data;
        }
        if let Some(p) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            let pinch = ui.input(|i| i.zoom_delta());
            let factor = pinch * (scroll / 400.0).exp();
            if factor != 1.0 {
                // keep the cell under the pointer in place
                let under = self.to_cell(rect, p);
                let least = self.least_zoom(rect, source.grid(), w, h);
                self.zoom = (self.zoom * factor).clamp(least.min(self.zoom), MAX_ZOOM);
                self.center = under - (p - rect.center()) / self.zoom;
            }
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        // the world under the data, then the data over it as transparent as asked
        if self.basemap != Basemap::None {
            self.draw_base(&ctx, &painter, rect, source.grid());
        }
        let tint = Color32::from_white_alpha(((1.0 - self.transparency.clamp(0.0, 1.0)) * 255.0).round() as u8);
        let show_data = !self.hide_data && !blank;
        let ppp = ctx.pixels_per_point();
        let mut wanted: Vec<(f32, Job)> = Vec::new();
        self.clock += 1;
        if !blank {
            let base = Place { offset: Vec2::ZERO, scale: Vec2::splat(1.0) };
            self.draw_source(&ctx, &painter, rect, source, coloring, tint, base, show_data, &mut wanted);
        }
        // the maps over it, placed by their coordinates on its grid (of its coordinate system)
        let mut layer_errors = Vec::new();
        for l in rasters {
            match place_on(l.source.grid(), source.grid()) {
                Some(place) => {
                    let tint = Color32::from_white_alpha((l.opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
                    self.draw_source(&ctx, &painter, rect, &l.source, &l.coloring, tint, place, true, &mut wanted);
                }
                None => layer_errors.push(
                    "a map over it is of another coordinate system or rotated: put it on this \
                                           map's grid (raster.match)",
                ),
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
            for (_, job) in wanted {
                if started.contains(&(job.key, job.colouring)) {
                    continue;
                }
                self.asked.insert((job.key, job.colouring));
                q.push_back(job);
            }
            cv.notify_all();
        }
        if let Some(e) = layer_errors.first() {
            painter.text(
                rect.left_top() + Vec2::new(8.0, 44.0),
                egui::Align2::LEFT_TOP,
                e,
                egui::FontId::proportional(12.0),
                ui.visuals().error_fg_color,
            );
        }
        // the vector layers over the map
        let cam = crate::vector_tiles::Camera { center: self.center, zoom: self.zoom, rect, pixels_per_point: ppp };
        let vector_errors = self.vectors.draw(&ctx, &painter, cam, source.grid(), overlays);
        if let Some(e) = vector_errors.first() {
            painter.text(
                rect.left_top() + Vec2::new(8.0, 26.0),
                egui::Align2::LEFT_TOP,
                e,
                egui::FontId::proportional(12.0),
                ui.visuals().error_fg_color,
            );
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
        // at the closest zoom, each cell's value written in it: only the cells in view (a few hundred at most at
        // 64 points a cell), their values from the map in memory or the tiles kept, the texts' layouts cached by
        // egui from frame to frame
        if show_data && self.zoom >= MAX_ZOOM {
            let (a, b) = (self.to_cell(rect, rect.min), self.to_cell(rect, rect.max));
            let (x0, y0) = (a.x.floor().max(0.0) as usize, a.y.floor().max(0.0) as usize);
            let (x1, y1) = ((b.x.ceil().max(0.0) as usize).min(w), (b.y.ceil().max(0.0) as usize).min(h));
            let font = egui::FontId::monospace((self.zoom / 6.0).clamp(9.0, 12.0));
            for y in y0..y1 {
                for x in x0..x1 {
                    let v = self.value_at(source, x, y);
                    if v.is_nan() {
                        continue;
                    }
                    let centre = rect.center() + (Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - self.center) * self.zoom;
                    // dark on a light cell, light on a dark one
                    let c = coloring.colour(v);
                    let light = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32 > 140.0;
                    let ink = if light && self.transparency < 0.5 { Color32::BLACK } else { Color32::WHITE };
                    painter.text(centre, egui::Align2::CENTER_CENTER, cell_text(v), font.clone(), ink);
                }
            }
        }
        // the cell clicked, outlined in the accent (its values along the layers beside the map)
        if let Some((row, col)) = self.picked.filter(|&(r, c)| r < h && c < w && !blank) {
            let cell = Vec2::new(col as f32, row as f32);
            let r = Rect::from_min_size(rect.center() + (cell - self.center) * self.zoom, Vec2::splat(self.zoom));
            let r = if r.width() < 6.0 { Rect::from_center_size(r.center(), Vec2::splat(6.0)) } else { r };
            painter.rect_stroke(r, 0.0, egui::Stroke::new(2.0, crate::theme::accent()), egui::StrokeKind::Outside);
        }
        let p = response.hover_pos()?;
        let c = self.to_cell(rect, p);
        let g = &source.grid().transform;
        let inside = c.x >= 0.0 && c.y >= 0.0 && c.x < w as f32 && c.y < h as f32;
        // the feature under the pointer (4 points around), its attributes beside it
        let feature = if overlays.is_empty() {
            None
        } else {
            let tolerance = f64::from(4.0 / self.zoom);
            self.vectors.pick(&ctx, overlays, source.grid(), [f64::from(c.x), f64::from(c.y)], tolerance)
        };
        if let Some((l, k)) = feature {
            let v = &overlays[l].vector;
            let mut text = format!("feature {}", k + 1);
            for (n, col) in &v.attributes {
                let value = match col {
                    terraflux_raster::vector::Column::Numbers(x) => crate::table_view::cell(x[k]),
                    terraflux_raster::vector::Column::Texts(t) => t[k].clone(),
                };
                text.push_str(&format!("\n{n}: {value}"));
            }
            response.clone().on_hover_text_at_pointer(text);
        }
        if (response.clicked() || response.secondary_clicked()) && !blank {
            self.picked = inside.then_some((c.y as usize, c.x as usize));
        }
        // a right click: why the cell has its value
        if self.picked.is_some() && !blank {
            response.context_menu(|ui| {
                if let Some((row, col)) = self.picked {
                    ui.label(egui::RichText::new(format!("row {row}, column {col}")).weak().small());
                }
                if ui.button("Trace this cell").on_hover_text("the chain of values that made it").clicked() {
                    self.trace_request = Some(false);
                    ui.close();
                }
                if ui
                    .button("Ask the assistant why")
                    .on_hover_text("its trace explained in words by the assistant (an assistant through terraflux mcp reads it with last_trace)")
                    .clicked()
                {
                    self.trace_request = Some(true);
                    ui.close();
                }
            });
        }
        // close enough to see the cells: the one under the pointer outlined
        if inside && !blank && self.zoom >= 6.0 {
            let cell = Vec2::new(c.x.floor(), c.y.floor());
            let r = Rect::from_min_size(rect.center() + (cell - self.center) * self.zoom, Vec2::splat(self.zoom));
            painter.rect_stroke(
                r,
                0.0,
                egui::Stroke::new(2.5, Color32::from_black_alpha(110)),
                egui::StrokeKind::Outside,
            );
            painter.rect_stroke(r, 0.0, egui::Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Inside);
        }
        // the coordinates under the pointer, in the view's corner (and the cell's value over the map)
        {
            let (px, py) = (f64::from(c.x), f64::from(c.y));
            let (x, y) = (g[0] + px * g[1] + py * g[2], g[3] + px * g[4] + py * g[5]);
            // degrees to a hundred-thousandth, metres to a centimetre
            let digits = if g[1].abs() < 0.01 { 6 } else { 2 };
            let mut text = format!("x {x:.digits$}   y {y:.digits$}");
            if inside && !blank {
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
            return feature.map(|f| Hover {
                row: 0,
                col: 0,
                x: f64::NAN,
                y: f64::NAN,
                value: f64::NAN,
                feature: Some(f),
            });
        }
        let (col, row) = (c.x as usize, c.y as usize);
        let (cx, cy) = (col as f64 + 0.5, row as f64 + 0.5);
        Some(Hover {
            row,
            col,
            x: g[0] + cx * g[1] + cy * g[2],
            y: g[3] + cx * g[4] + cy * g[5],
            value: self.value_at(source, col, row),
            feature,
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
        Coloring::Continuous { min, max, ramp, log } => {
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
            let scale = if *log { "   (logarithmic)" } else { "" };
            ui.label(format!("{min:.4} to {max:.4}{scale}"));
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
    fn cell_texts() {
        assert_eq!(cell_text(3.0), "3");
        assert_eq!(cell_text(-1250.0), "-1250");
        assert_eq!(cell_text(0.25), "0.25");
        assert_eq!(cell_text(0.123456), "0.123");
        assert_eq!(cell_text(12.3456), "12.35");
        assert_eq!(cell_text(1234.56), "1234.6");
        assert_eq!(cell_text(123456.7), "1.23e5");
        assert_eq!(cell_text(0.00012), "1.20e-4");
    }

    #[test]
    fn counts() {
        assert_eq!(class_counts(&[2.0, 1.0, f64::NAN, 2.0]), vec![(1.0, 1), (2.0, 2)]);
        assert_eq!(class_counts(&[0.5, -1.0, 300.0, 0.5, 0.0]), vec![(-1.0, 1), (0.0, 1), (0.5, 2), (300.0, 1)]);
    }
}
