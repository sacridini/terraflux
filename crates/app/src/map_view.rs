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

use crate::blend::Blend;
use crate::palette::{Coloring, image, image_f32};

/// The side of a tile, pixels.
pub const TILE: usize = 256;
/// The tiles made taken in a frame at most (their textures sent to the GPU).
const MADE_PER_FRAME: usize = 32;
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
    /// Three maps drawn as one colour image (see [`crate::rgb`]): its tiles hold packed colours.
    Rgb(Arc<crate::rgb::RgbSource>),
    /// A map's hillshade (see [`crate::shade`]): its tiles hold the brightness, 0 to 1.
    Shade(Arc<crate::shade::ShadeSource>),
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
            Source::Rgb(r) => r.key(),
            Source::Shade(s) => s.key(),
        }
    }

    pub fn grid(&self) -> &terraflux_raster::Grid {
        match self {
            Source::Map(m) => &m.grid,
            Source::Rasters(r, l) => r.grid_of(*l),
            Source::Blank(g) => g,
            Source::Rgb(r) => r.bands[0].grid(),
            Source::Shade(s) => s.base.grid(),
        }
    }

    pub fn size(&self) -> (usize, usize) {
        let g = self.grid();
        (g.width, g.height)
    }

    /// Whether the cells are in a file on the web (read by windows, over the network).
    pub fn is_remote(&self) -> bool {
        match self {
            Source::Rasters(r, _) => r.is_remote(),
            Source::Rgb(r) => r.bands[0].is_remote(),
            Source::Shade(s) => s.base.is_remote(),
            Source::Map(_) | Source::Blank(_) => false,
        }
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
            // (the three bands' tiles, as the threads make any: packed into colours)
            Source::Rgb(r) => {
                let t =
                    [r.bands[0].tile(level, tx, ty)?, r.bands[1].tile(level, tx, ty)?, r.bands[2].tile(level, tx, ty)?];
                Ok(r.pack_values([&t[0], &t[1], &t[2]]))
            }
            // (the brightness of the map's cells, from a window with a cell around the tile)
            Source::Shade(s) => s.tile(level, tx, ty),
        }
    }

    /// A tile's values with `halo` cells more on every side (of the same level): `(TILE + 2 * halo)` squared, by rows,
    /// NaN outside the map. For what is worked out of a cell and its neighbours (a shade), so that tiles join.
    pub fn halo(&self, level: u32, tx: usize, ty: usize, halo: usize) -> Result<Vec<f32>, String> {
        let step = 1usize << level;
        let side = TILE + 2 * halo;
        let (x0, y0) =
            ((tx * TILE * step) as i64 - (halo * step) as i64, (ty * TILE * step) as i64 - (halo * step) as i64);
        match self {
            Source::Map(m) => {
                let (w, h) = (m.grid.width as i64, m.grid.height as i64);
                let mut v = vec![f32::NAN; side * side];
                for i in 0..side {
                    let y = y0 + (i * step) as i64;
                    if y < 0 || y >= h {
                        continue;
                    }
                    let row = &m.data[(y * w) as usize..((y + 1) * w) as usize];
                    for j in 0..side {
                        let x = x0 + (j * step) as i64;
                        if x >= 0 && x < w {
                            v[i * side + j] = row[x as usize] as f32;
                        }
                    }
                }
                Ok(v)
            }
            Source::Rasters(r, l) => {
                let win = Window { x0, y0, width: side * step, height: side * step };
                r.sample(*l, win, step).map_err(|e| e.to_string())
            }
            Source::Blank(_) => Ok(vec![f32::NAN; side * side]),
            Source::Rgb(_) | Source::Shade(_) => Err("no window of a derived map".into()),
        }
    }

    /// A cell's value, when it can be had at once (a map in memory).
    fn value(&self, x: usize, y: usize) -> Option<f64> {
        match self {
            Source::Map(m) => m.data.get(y * m.grid.width + x).copied(),
            Source::Rasters(..) | Source::Blank(_) | Source::Rgb(_) | Source::Shade(_) => None,
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
    opaque: bool,
    colouring: u64,
}

/// Whether an image has no pixel seen through (no cell without a value, no colour made transparent).
fn opaque(img: &egui::ColorImage) -> bool {
    img.pixels.iter().all(|p| p.a() == 255)
}

/// What the layers over a source hide of the view: the screen rectangles of the tiles they drew opaque. A tile of
/// the source there is neither drawn nor read (a stack of 40 years shows its top year alone), except the tile under
/// the pointer (its value told). `opaque` gathers those the source draws, for the layers under it.
#[derive(Default)]
struct Cover {
    hidden: Vec<Rect>,
    opaque: Vec<Rect>,
    pointer: Option<Pos2>,
    /// the part of the view the source is drawn in (left or right of a curtain)
    clip: Option<Rect>,
}

impl Cover {
    /// Whether the part `r` of the view is hidden (inside one rectangle drawn opaque over it).
    fn hides(&self, r: Rect) -> bool {
        r.is_positive()
            && !self.pointer.is_some_and(|p| r.contains(p))
            && self.hidden.iter().any(|c| c.expand(0.5).contains_rect(r))
    }

    /// The source drawn: what it drew opaque hides what is under it too.
    fn drawn(&mut self) {
        self.hidden.append(&mut self.opaque);
    }
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
        let threads = terraflux_ops::threads::most().min(8);
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
                    let opaque = image.as_ref().is_some_and(opaque);
                    started.lock().unwrap().remove(&(job.key, job.colouring));
                    let (key, source, colouring) = (job.key, job.source, job.colouring);
                    let made = Made { key, source, values, image, opaque, colouring };
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
    /// A service of the user's: its address template is kept in [`crate::basemap`] under this number (a hash of
    /// the template, which also names its folder of tiles on disk).
    Custom(u64),
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

    pub fn name(self) -> String {
        match self {
            Basemap::None => "None",
            Basemap::EsriImagery => "Satellite (Esri)",
            Basemap::EsriTopographic => "Topographic (Esri)",
            Basemap::OpenStreetMap => "OpenStreetMap",
            Basemap::OpenTopoMap => "OpenTopoMap",
            Basemap::CartoLight => "Light (Carto)",
            Basemap::Custom(id) => return crate::basemap::custom(id).map_or("Custom".into(), |c| c.name()),
        }
        .into()
    }

    /// The tiles' address, with `${z}`, `${x}` and `${y}`; the deepest zoom level.
    pub(crate) fn url(self) -> Option<(String, u32)> {
        if let Basemap::Custom(id) = self {
            return crate::basemap::custom(id).map(|c| (c.template, c.max_zoom));
        }
        let (url, levels) = match self {
            Basemap::None | Basemap::Custom(_) => return None,
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
        };
        Some((url.to_string(), levels))
    }

    /// Whose map it is, written on it as the services ask.
    pub fn attribution(self) -> String {
        match self {
            Basemap::None => "",
            Basemap::EsriImagery => "Esri, Maxar, Earthstar Geographics, and the GIS User Community",
            Basemap::EsriTopographic => "Esri, HERE, Garmin, USGS, NGA, and others",
            Basemap::OpenStreetMap => "© OpenStreetMap contributors",
            Basemap::OpenTopoMap => "© OpenStreetMap contributors, SRTM; style © OpenTopoMap (CC-BY-SA)",
            Basemap::CartoLight => "© OpenStreetMap contributors © CARTO",
            Basemap::Custom(id) => return crate::basemap::custom(id).map_or(String::new(), |c| c.attribution),
        }
        .into()
    }
}

/// The CRS of a grid as GDAL takes it: the WKT GDAL read, else the EPSG code of the GeoTIFF keys.
pub(crate) fn crs_of(g: &terraflux_raster::Grid) -> Option<String> {
    g.geo.wkt.clone().or_else(|| g.geo.epsg().map(|c| format!("EPSG:{c}")))
}

/// What the pointer on a mark is told: the place and, when it is off the map, so.
fn mark_text(m: &Mark) -> String {
    let out = if m.inside { "" } else { " (outside the map)" };
    format!("Go to: {}{out}", m.text)
}

/// A map drawn with another: its colours, how opaque it is (0 to 1), whether it is drawn under the map shown
/// (else over it), its name (its value under the pointer told by it) and how it is mixed with what is under it.
#[derive(Clone)]
pub struct RasterLayer {
    pub source: Source,
    pub coloring: Coloring,
    pub opacity: f32,
    pub below: bool,
    pub name: String,
    pub blend: crate::blend::Blend,
}

/// Another map compared with the one shown, right of a curtain dragged across the view: the map, and the names
/// of the two (written at the top of each side).
pub struct Curtain {
    pub layer: RasterLayer,
    pub names: [String; 2],
}

/// Where a source's cells are on the view's grid: a cell (x, y) at `offset + (x, y) * scale` cells of the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub offset: Vec2,
    pub scale: Vec2,
}

impl Place {
    /// A cell of the source as a cell of the view.
    pub fn to_view(self, c: Vec2) -> Vec2 {
        self.offset + Vec2::new(c.x * self.scale.x, c.y * self.scale.y)
    }

    /// A cell of the view as a cell of the source.
    pub fn to_source(self, c: Vec2) -> Vec2 {
        Vec2::new((c.x - self.offset.x) / self.scale.x, (c.y - self.offset.y) / self.scale.y)
    }
}

/// Where a layer is on the view's grid: placed by its coordinates (the same coordinate system), or carried there
/// from another ([`crate::reproject`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum At {
    Place(Place),
    Warp,
}

/// A feature of a vector layer as the pointer tells it: the layer's name (when it has one), its number, its
/// attributes a line each.
pub fn feature_text(o: &crate::vector_tiles::Overlay, k: usize) -> String {
    let mut text =
        if o.name.is_empty() { format!("feature {}", k + 1) } else { format!("{}: feature {}", o.name, k + 1) };
    for (n, col) in &o.vector.attributes {
        let value = match col {
            terraflux_raster::vector::Column::Numbers(x) => crate::table_view::cell(x[k]),
            terraflux_raster::vector::Column::Texts(t) => t[k].clone(),
        };
        text.push_str(&format!("\n{n}: {value}"));
    }
    text
}

/// A grid's place on another: by their transforms, when they are of the same coordinate system (or neither has
/// one) and neither is rotated (a rotated one is carried by [`crate::reproject`]).
pub fn place_on(g: &terraflux_raster::Grid, base: &terraflux_raster::Grid) -> Option<Place> {
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

/// Why a map drawn with the one shown is not: its coordinate system is not told or cannot be
/// carried to the view's.
const OTHER_GRID: &str = "the coordinate system of a map drawn with it is unknown or cannot be carried \
                          to this map's: put it on this map's grid (raster.match)";

/// A tile made: its values, its texture in a colouring, when it was last shown.
struct Tile {
    values: Arc<Vec<f32>>,
    texture: Option<(u64, TextureHandle)>,
    /// whether the texture has no pixel seen through
    opaque: bool,
    used: u64,
    /// the source, held so that its key is not another's while the tile is kept
    _source: Source,
}

/// A patch's outline on the map: the sides of its cells on its border, from a corner to another (x a column, y a
/// row), and the rectangle of the rows and columns it spans (drawn instead when it has too many sides).
#[derive(Debug, Clone, PartialEq)]
pub struct Outline {
    pub edges: Vec<[(f32, f32); 2]>,
    pub bounds: (f32, f32, f32, f32),
}

/// The sides of an outline drawn one by one, at most (more: its rectangle).
const OUTLINE_SIDES: usize = 60_000;
/// The most features selected outlined at once (those in view).
const SELECTED_DRAWN: usize = 500;

/// What is drawn over the map to find one's way: the scale bar and the coordinate grid. They start as the settings
/// have them (the bar on, the grid off unless turned on before).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Aids {
    pub scale_bar: bool,
    pub grid: bool,
}

impl Default for Aids {
    fn default() -> Aids {
        let s = crate::settings::get();
        Aids { scale_bar: s.scale_bar, grid: s.map_grid }
    }
}

/// A place *Go to* found, marked on the map until escape or another place: a cell of the map shown (fractional),
/// its coordinates as text, and whether it is inside the map.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub cell: [f64; 2],
    pub text: String,
    pub inside: bool,
}

/// How near the pointer must be to a mark for its coordinates to be told, screen points.
const MARK_NEAR: f32 = 14.0;

#[derive(Default)]
pub struct MapView {
    /// The scale bar and the coordinate grid over the map.
    pub aids: Aids,
    /// The views the map settled on, to go back and forward along (alt+left, alt+right, the mouse's buttons).
    history: crate::nav::History,
    /// The scale bar drawn at the last frame (none: not drawn).
    scale_drawn: Option<crate::nav::Bar>,
    /// The box of *Go to* while it is open, and the place it found (marked until escape).
    pub goto: Option<crate::goto::GoTo>,
    pub mark: Option<Mark>,
    /// The mark was just set here (views side by side drop the marks of the others), escape was pressed over this view
    /// (they drop it everywhere), and a view beside has a mark (escape lets it go from here too).
    pub mark_new: bool,
    pub mark_stopped: bool,
    pub mark_beside: bool,
    tiles: HashMap<TileKey, Tile>,
    workers: Option<Workers>,
    /// the tiles asked of the workers and not come yet
    asked: HashSet<(TileKey, u64)>,
    /// the pass of the context the view was last drawn in
    drawn_pass: u64,
    clock: u64,
    /// the clock when the last frame was drawn: the tiles it used are kept
    frame_clock: u64,
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
    /// How opaque the map shown is as a layer, 0 to 1 (none: opaque; 0: hidden in the Layers).
    pub opacity: Option<f32>,
    base: crate::basemap::Layer,
    /// The vector layers drawn over the map.
    pub vectors: crate::vector_tiles::VectorTiles,
    /// The grid (its transform's bits and size) and how many of its cells the world spans (none: no CRS).
    world: Option<(u64, Option<f64>)>,
    /// The cell clicked (row, column): its values along a stack's layers are shown beside the map.
    pub picked: Option<(usize, usize)>,
    /// The patch of the cell traced drawn around it (a trace's card of a patch's operator selected); let go when
    /// another cell is picked.
    pub patch: Option<Outline>,
    /// The features selected in a table of attributes (layer, features), outlined in the accent; and the features
    /// (layer, feature) under the pointer at the last click, to select rows by (`feature_toggle`: with ctrl, to
    /// toggle them instead).
    pub selected: Option<(usize, Vec<usize>)>,
    pub feature_clicked: Vec<(usize, usize)>,
    pub feature_toggle: bool,
    /// A shape being dragged to select features by area (shift+drag a rectangle, shift+alt+drag a lasso), the
    /// shape finished and not yet used (`area_done`), whether the drag in hand is one (escape lets the shape go, the
    /// drag goes on without moving the map), and the keys held at the last press.
    pub area: Option<crate::area::Drag>,
    pub area_done: Option<crate::area::Done>,
    selecting: bool,
    press: egui::Modifiers,
    /// The trace of the cell picked asked for from its menu (a right click), and who explains it.
    pub trace_request: Option<crate::trace_view::Explain>,
    /// Where the view was drawn last.
    rect: Option<Rect>,
    /// Where the curtain of a comparison is, a share of the view's width (none: in the middle).
    curtain: Option<f32>,
    /// The cell under the pointer at the last frame (none when the pointer is elsewhere).
    pointer: Option<Vec2>,
    /// A cell marked by a cross: where the pointer is in a view beside this one (the same place).
    pub ghost: Option<Vec2>,
    /// Features being drawn on the map (clicks add their vertices instead of picking a cell).
    pub drawing: Option<crate::draw::Sketch>,
    /// A line being measured (clicks add its points instead of picking a cell); not while drawing.
    pub measuring: Option<crate::measure::Measure>,
    /// Without the box of numbers while measuring (of views side by side, those not being measured), and Esc
    /// pressed over the view (read and cleared by whoever keeps the views together: every view then stops).
    pub hide_measure_box: bool,
    pub measure_stopped: bool,
    /// The name of the map shown, its value under the pointer told by it beside those of the maps drawn with it.
    pub name: String,
    /// The map drawn at the last frame and its colouring; the one shown before the map now, while it fades in.
    last: Option<(Source, u64)>,
    prior: Option<Prior>,
    /// The layers of another coordinate system than the view's, carried to it.
    reproj: HashMap<SourceKey, crate::reproject::Reproj>,
    /// The grid of the map drawn at the last frame.
    shown_grid: Option<terraflux_raster::Grid>,
    /// The layers drawn with the map at the last frame: the maps and the vector layers.
    layers_drawn: (usize, usize),
    /// A box (west, south, east, north) to bring the view to when the next map is drawn: a Plot's saved view.
    frame: Option<[f64; 4]>,
    /// The bands' values at the cell last read of a colour composite (its key, column, row, values).
    rgb_memo: Mutex<Option<RgbCell>>,
}

/// A colour composite's bands' values at a cell: its key, column, row and the values.
type RgbCell = (SourceKey, usize, usize, [f64; 3]);

/// How long a map takes to fade in over the one shown before it (a series' next step), seconds.
const FADE: f32 = 0.18;

/// The map shown before the one now (of the same size), held under it until its tiles have come and it has
/// faded in: no blank between two steps of a series.
struct Prior {
    source: Source,
    colouring: u64,
    since: std::time::Instant,
}

/// A tile drawn in a blend mode by the GPU ([`crate::blend::Tile`]): its values in the colouring, the part `seen` of
/// its screen rectangle `r` (where `uv` of its texture is), at the opacity of `tint`.
fn blend_tile(
    mode: Blend,
    coloring: Arc<Coloring>,
    (values, key): (&Arc<Vec<f32>>, (TileKey, u64)),
    (r, uv, seen): (Rect, Rect, Rect),
    tint: Color32,
    frame: u64,
) -> egui::Shape {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    key.hash(&mut h);
    // (the part seen of the tile's rectangle: its place on the texture)
    let at = |v: f32, lo: f32, size: f32, a: f32, b: f32| a + (b - a) * ((v - lo) / size);
    let (u0, u1) = (
        at(seen.min.x, r.min.x, r.width(), uv.min.x, uv.max.x),
        at(seen.max.x, r.min.x, r.width(), uv.min.x, uv.max.x),
    );
    let (v0, v1) = (
        at(seen.min.y, r.min.y, r.height(), uv.min.y, uv.max.y),
        at(seen.max.y, r.min.y, r.height(), uv.min.y, uv.max.y),
    );
    let tile = crate::blend::Tile {
        id: h.finish(),
        frame,
        values: values.clone(),
        coloring,
        side: TILE,
        uv: [u0, v0, u1, v1],
        opacity: f32::from(tint.a()) / 255.0,
        mode,
    };
    egui::Shape::from(eframe::egui_wgpu::Callback::new_paint_callback(seen, tile))
}

/// How a source's tiles are drawn: in a colouring, the tiles missing asked for; or only those kept in a colouring.
#[derive(Clone, Copy)]
enum Paint<'a> {
    Asked(&'a Coloring),
    Held(u64),
}

impl MapView {
    /// The part of the map drawn at the last frame that is in view: its grid, and the columns and rows seen
    /// (`[x0, y0, x1, y1]`, in cells of the grid, within it); none before a map was drawn.
    pub fn visible(&self) -> Option<(&terraflux_raster::Grid, [f32; 4])> {
        let (g, rect) = (self.shown_grid.as_ref()?, self.rect?);
        if !self.fitted || self.zoom <= 0.0 || g.width == 0 || g.height == 0 {
            return None;
        }
        let half = rect.size() / (2.0 * self.zoom);
        let (w, h) = (g.width as f32, g.height as f32);
        let b = [
            (self.center.x - half.x).clamp(0.0, w),
            (self.center.y - half.y).clamp(0.0, h),
            (self.center.x + half.x).clamp(0.0, w),
            (self.center.y + half.y).clamp(0.0, h),
        ];
        (b[2] > b[0] && b[3] > b[1]).then_some((g, b))
    }

    /// The layers drawn with the map at the last frame: how many maps (over or under it) and how many vector layers.
    pub fn layers_drawn(&self) -> (usize, usize) {
        self.layers_drawn
    }

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

    /// Why the map's tiles could not be read (a file that cannot be read, a server that failed), if so.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Screen points per cell.
    /// Where the view was drawn last, on the screen.
    pub fn rect(&self) -> Option<Rect> {
        self.rect
    }

    /// The line measured in the view beside (its points as cells of that view's grid), drawn over this one: each
    /// carried by `link` (this view's grid on that one's) to this view's cells. Call it after the view is drawn.
    pub fn paint_measure(&self, ui: &Ui, line: &crate::measure::Measure, link: &crate::reproject::Link) {
        let Some(rect) = self.rect else { return };
        let pts: Vec<Pos2> = line
            .points
            .iter()
            .map(|c| link.to_own(Vec2::new(c[0] as f32, c[1] as f32)))
            .filter(|c| c.x.is_finite() && c.y.is_finite())
            .map(|c| rect.center() + (c - self.center) * self.zoom)
            .collect();
        crate::measure::paint_line(&ui.painter_at(rect), &pts, line.ended);
    }

    /// Measuring turned on or off (the line lost); refused while features are being drawn.
    pub fn set_measuring(&mut self, on: bool) {
        self.measuring = (on && self.drawing.is_none()).then(Default::default);
    }

    /// What the line measured tells (none: not measuring, or no points yet).
    pub fn measured(&self) -> Option<crate::measure::Report> {
        let line = self.measuring.as_ref().filter(|l| !l.points.is_empty())?;
        Some(line.report(self.shown_grid.as_ref()?))
    }

    /// A point added to the line measured, a cell of the map shown (as a click would).
    pub fn measure_click(&mut self, cell: [f64; 2]) {
        if let Some(l) = &mut self.measuring {
            l.click(cell);
        }
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// The zoom (screen points per cell) and the cell at the centre of the view.
    pub fn camera(&self) -> (f32, Vec2) {
        (self.zoom, self.center)
    }

    /// The view at this zoom and centre (a view beside another following it), not fitted again.
    pub fn set_camera(&mut self, zoom: f32, center: Vec2) {
        self.zoom = zoom;
        self.center = center;
        self.fitted = true;
    }

    /// Whether there is a view to go back to.
    pub fn can_back(&self) -> bool {
        self.history.can_back(self.camera())
    }

    /// Whether there is a view to go forward to.
    pub fn can_forward(&self) -> bool {
        self.history.can_forward()
    }

    /// The view before this one (the last that settled); nothing pushed.
    pub fn back(&mut self) {
        if let Some((zoom, center)) = self.history.back(self.camera()) {
            self.set_camera(zoom, center);
        }
    }

    /// The view after, when one went back.
    pub fn forward(&mut self) {
        if let Some((zoom, center)) = self.history.forward() {
            self.set_camera(zoom, center);
        }
    }

    /// How many views the history keeps.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// The scale bar drawn at the last frame: its length on the screen and its text.
    pub fn scale_bar(&self) -> Option<&crate::nav::Bar> {
        self.scale_drawn.as_ref()
    }

    /// The *Go to* box open (or closed, when it is).
    pub fn toggle_goto(&mut self) {
        self.goto = match self.goto {
            Some(_) => None,
            None => Some(crate::goto::GoTo::new(self.shown_grid.as_ref())),
        };
    }

    /// Go to the place a text names (degrees, or the map's units): the view centred on it at the same zoom, a mark
    /// on it. Whether it is inside the map; a place outside it is gone to all the same.
    pub fn go_to(&mut self, text: &str, degrees: bool) -> Result<bool, String> {
        self.go_to_zoom(text, degrees, crate::goto::Zoom::Keep)
    }

    /// `go_to` at a zoom: the map's scale or the width of ground in view (`zoom`, unless the text has its own after an
    /// `@`), else the zoom as it is. The jump (centre and zoom) is one view of the history.
    pub fn go_to_zoom(&mut self, text: &str, degrees: bool, zoom: crate::goto::Zoom) -> Result<bool, String> {
        let grid = self.shown_grid.as_ref().ok_or("no map is shown")?;
        let (target, typed) = crate::goto::parse_with_zoom(text, degrees)?;
        let zoom = if typed == crate::goto::Zoom::Keep { zoom } else { typed };
        let (cell, inside) = crate::goto::locate(grid, target)?;
        let new_zoom = match zoom.metres_per_point(self.rect.map_or(800.0, |r| r.width())) {
            None => None,
            Some(per_pt) => {
                let at = Vec2::new(cell[0] as f32, cell[1] as f32);
                let per_cell = crate::nav::metres_per_point(grid, at, 1.0)
                    .ok_or("the map has no units to take a scale or a width in: keep the zoom")?;
                Some(((per_cell / per_pt) as f32).clamp(1e-6, MAX_ZOOM))
            }
        };
        let label = crate::goto::place_text(grid, cell);
        let text = match target {
            // (what was typed, as the map's system has it, and in degrees when they were typed)
            crate::goto::Target::Degrees { lon, lat }
                if crate::measure::Frame::of(grid) != crate::measure::Frame::Geographic =>
            {
                format!("{}  ({label})", crate::goto::degrees_text(lon, lat))
            }
            _ => label,
        };
        self.mark = Some(Mark { cell, text, inside });
        (self.mark_new, self.mark_stopped) = (true, false);
        if let Some(z) = new_zoom {
            self.zoom = z;
        }
        self.center = Vec2::new(cell[0] as f32, cell[1] as f32);
        self.fitted = true;
        Ok(inside)
    }

    /// The view on a box of cells of the map shown (`[x0, y0, x1, y1]`), at the size the view was drawn last;
    /// nothing before it was drawn.
    pub fn zoom_to(&mut self, cells: [f32; 4]) {
        let Some(rect) = self.rect else { return };
        let (w, h) = ((cells[2] - cells[0]).abs().max(1e-3), (cells[3] - cells[1]).abs().max(1e-3));
        let zoom = (rect.width() / w).min(rect.height() / h).max(1e-6);
        self.set_camera(zoom, Vec2::new((cells[0] + cells[2]) / 2.0, (cells[1] + cells[3]) / 2.0));
    }

    /// The view on a box of the map's coordinates (west, south, east, north) when the map is drawn next, whichever
    /// map and window size that is; kept until a map the box touches is drawn.
    pub fn show_extent(&mut self, extent: [f64; 4]) {
        self.frame = Some(extent);
    }

    /// The grid of the map drawn at the last frame.
    pub fn shown_grid(&self) -> Option<&terraflux_raster::Grid> {
        self.shown_grid.as_ref()
    }

    /// A map of this size shown next where the view is (a view following another), not fitted to it again.
    pub fn hold(&mut self, size: (usize, usize)) {
        self.size = size;
    }

    /// The cell under the pointer at the last frame, if it was over the view.
    pub fn pointer(&self) -> Option<Vec2> {
        self.pointer
    }

    /// Where the curtain of a comparison is, a share of the view's width.
    pub fn curtain(&self) -> f32 {
        self.curtain.unwrap_or(0.5)
    }

    /// Whether tiles of the view (of the map or of its vector layers) are being made.
    pub fn busy(&self) -> bool {
        !self.asked.is_empty() || self.vectors.busy()
    }

    /// Not drawn in the pass of `ctx` (it shows something else now: a table, a value): the tiles asked not waited
    /// for (those that come are kept when it is drawn again), so that it is not busy.
    pub fn rest_unless_drawn(&mut self, ctx: &egui::Context) {
        if self.drawn_pass != ctx.cumulative_pass_nr() {
            self.asked.clear();
        }
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
    /// stretched; a few tiles kept in another colouring coloured again); the tiles missing asked for. Held: only
    /// the tiles kept in that colouring, nothing asked. Whether every tile in view was drawn (stretched or not).
    /// The tiles hidden by the layers over it ([`Cover`]) neither drawn nor asked; those it draws opaque told.
    #[allow(clippy::too_many_arguments)]
    fn draw_source(
        &mut self,
        ctx: &egui::Context,
        out: &mut Vec<egui::Shape>,
        rect: Rect,
        source: &Source,
        paint: Paint<'_>,
        tint: Color32,
        place: Place,
        (show, blend): (bool, crate::blend::Blend),
        wanted: &mut Vec<(f32, Job)>,
        cover: &mut Cover,
    ) -> bool {
        let (w, h) = source.size();
        if w == 0 || h == 0 {
            return true;
        }
        let ppp = ctx.pixels_per_point();
        let top = source.top();
        // the level: a tile's pixel about a pixel of the screen
        let cells_per_pixel = 1.0 / (self.zoom * place.scale.x.abs().min(place.scale.y.abs()) * ppp);
        let level = (cells_per_pixel.max(1.0).log2().floor() as u32).min(top);
        let src = source.key();
        let (coloring, colouring) = match paint {
            Paint::Asked(c) => (Some(c), c.key()),
            Paint::Held(k) => (None, k),
        };
        let mut complete = true;
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
        let job = |key: TileKey, coloring: &Coloring| Job {
            key,
            source: source.clone(),
            coloring: coloring.clone(),
            colouring,
        };
        let mut recolour = if coloring.is_some() { 8 } else { 0 };
        // a layer in a blend mode is drawn by the GPU from the tiles' values in its colouring (a tile drawn in the
        // colouring it was not made in, or kept without one, is drawn the normal way)
        let blended = match (blend, coloring) {
            (Blend::Normal, _) | (_, None) => None,
            (mode, Some(c)) => Some((mode, Arc::new(c.clone()))),
        };
        // the top tiles over the view first: something to show at once
        let (tx0, ty0, tx1, ty1) = range(top);
        for ty in ty0..=(ty1.max(0) as usize) {
            for tx in tx0..=(tx1.max(0) as usize) {
                let k = (src, top, tx, ty);
                if cover.hides(screen(top, tx, ty).intersect(rect)) {
                    continue;
                }
                // (kept while in view: what is drawn while finer tiles come, never let go and asked again)
                match self.tiles.get_mut(&k) {
                    Some(t) if t.texture.as_ref().is_some_and(|(c, _)| *c == colouring) => t.used = clock,
                    _ => {
                        if let Some(c) = coloring {
                            wanted.push((-1.0, job(k, c)));
                        }
                    }
                }
            }
        }
        if x1 < x0 as i64 || y1 < y0 as i64 {
            return complete;
        }
        for ty in y0..=y1 as usize {
            for tx in x0..=x1 as usize {
                let k = (src, level, tx, ty);
                let r = screen(level, tx, ty);
                // (hidden by the layers over it: neither drawn nor read)
                let seen = r.intersect(rect);
                if cover.hides(seen) {
                    continue;
                }
                let seen = cover.clip.map_or(seen, |c| seen.intersect(c));
                let covers = show && tint.a() == 255 && blend == Blend::Normal;
                // a tile kept in another colouring is coloured again here (a few per frame)
                if let Some(t) = self.tiles.get_mut(&k)
                    && t.texture.as_ref().is_none_or(|(c, _)| *c != colouring)
                    && recolour > 0
                    && let Some(coloring) = coloring
                {
                    recolour -= 1;
                    let img = image_f32(&t.values, TILE, TILE, coloring);
                    t.opaque = opaque(&img);
                    t.texture = Some((
                        colouring,
                        ctx.load_texture(format!("tile-r-{clock}-{tx}-{ty}"), img, TextureOptions::NEAREST),
                    ));
                }
                let drawn = match self.tiles.get_mut(&k) {
                    Some(t) if t.texture.as_ref().is_some_and(|(c, _)| *c == colouring) => {
                        t.used = clock;
                        if show {
                            let id = t.texture.as_ref().unwrap().1.id();
                            match blended.as_ref().filter(|_| seen.is_positive()) {
                                Some((mode, c)) => out.push(blend_tile(
                                    *mode,
                                    c.clone(),
                                    (&t.values, (k, colouring)),
                                    (r, full, seen),
                                    tint,
                                    clock,
                                )),
                                None if blended.is_some() => {}
                                None => out.push(egui::Shape::image(id, r, full, tint)),
                            }
                        }
                        if covers && t.opaque {
                            cover.opaque.push(seen);
                        }
                        true
                    }
                    _ => false,
                };
                if drawn {
                    continue;
                }
                if let Some(c) = coloring {
                    let d = (r.center() - rect.center()).length();
                    wanted.push((d, job(k, c)));
                }
                // meanwhile the finest coarser tile kept over it, stretched
                let mut stretched = false;
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
                            match blended.as_ref().filter(|_| seen.is_positive()) {
                                Some((mode, c)) => {
                                    let key = ((src, up, ax, ay), colouring);
                                    out.push(blend_tile(*mode, c.clone(), (&t.values, key), (r, uv, seen), tint, clock))
                                }
                                None if blended.is_some() => {}
                                None => out.push(egui::Shape::image(tex.id(), r, uv, tint)),
                            }
                        }
                        if covers && t.opaque {
                            cover.opaque.push(seen);
                        }
                        stretched = true;
                        break;
                    }
                }
                complete &= stretched;
            }
        }
        complete
    }

    /// Where a layer is on the view's grid, if it is: placed by its coordinates, or carried from another
    /// coordinate system (the way to carry it already made).
    fn placed(&self, l: &RasterLayer, base: &terraflux_raster::Grid) -> Option<At> {
        if !self.reproj.is_empty()
            && self.reproj.get(&l.source.key()).is_some_and(|re| re.is_for(l.source.grid(), base))
        {
            return Some(At::Warp);
        }
        place_on(l.source.grid(), base).map(At::Place)
    }

    /// Where a layer is on the view's grid; when it is of another coordinate system, the way to carry it made
    /// (once). Why not, if so.
    fn layer_at(&mut self, l: &RasterLayer, base: &terraflux_raster::Grid) -> Result<At, &'static str> {
        if let Some(at) = self.placed(l, base) {
            return Ok(at);
        }
        match crate::reproject::Reproj::new(l.source.grid(), base) {
            Ok(re) => {
                if self.reproj.len() > 64 {
                    self.reproj.clear();
                }
                self.reproj.insert(l.source.key(), re);
                Ok(At::Warp)
            }
            Err(_) => Err(OTHER_GRID),
        }
    }

    /// [`MapView::draw_source`] for a layer wherever it is on the view's grid.
    #[allow(clippy::too_many_arguments)]
    fn draw_at(
        &mut self,
        ctx: &egui::Context,
        out: &mut Vec<egui::Shape>,
        rect: Rect,
        source: &Source,
        paint: Paint<'_>,
        tint: Color32,
        at: At,
        (show, blend): (bool, crate::blend::Blend),
        wanted: &mut Vec<(f32, Job)>,
        cover: &mut Cover,
    ) -> bool {
        match at {
            At::Place(place) => {
                self.draw_source(ctx, out, rect, source, paint, tint, place, (show, blend), wanted, cover)
            }
            // (a layer carried from another coordinate system is drawn as a mesh, mixed in the normal way)
            At::Warp => {
                let Some(mut re) = self.reproj.remove(&source.key()) else { return true };
                let done = self.draw_warped(ctx, out, rect, source, paint, tint, &mut re, show, wanted);
                self.reproj.insert(source.key(), re);
                done
            }
        }
    }

    /// [`MapView::draw_source`] for a layer of another coordinate system: each tile in view a mesh on the view
    /// ([`crate::reproject`]), the tiles made as they are for any layer.
    #[allow(clippy::too_many_arguments)]
    fn draw_warped(
        &mut self,
        ctx: &egui::Context,
        out: &mut Vec<egui::Shape>,
        rect: Rect,
        source: &Source,
        paint: Paint<'_>,
        tint: Color32,
        re: &mut crate::reproject::Reproj,
        show: bool,
        wanted: &mut Vec<(f32, Job)>,
    ) -> bool {
        let (w, h) = source.size();
        if w == 0 || h == 0 {
            return true;
        }
        let Some([bx0, by0, bx1, by1]) = re.seen(self.to_cell(rect, rect.min), self.to_cell(rect, rect.max)) else {
            return true;
        };
        let top = source.top();
        let cells_per_pixel = 1.0 / (self.zoom * re.scale * ctx.pixels_per_point());
        let mut level = (cells_per_pixel.max(1.0).log2().floor() as u32).min(top);
        let range = |level: u32| {
            let span = (TILE << level) as f32;
            let x0 = (bx0 / span).floor().max(0.0) as usize;
            let y0 = (by0 / span).floor().max(0.0) as usize;
            let x1 = ((bx1 / span).floor() as i64).min(w.div_ceil(TILE << level) as i64 - 1);
            let y1 = ((by1 / span).floor() as i64).min(h.div_ceil(TILE << level) as i64 - 1);
            (x0, y0, x1, y1)
        };
        // (a view far out: coarser tiles, not a thousand)
        while level < top {
            let (x0, y0, x1, y1) = range(level);
            if (x1 - x0 as i64 + 1).max(0) * (y1 - y0 as i64 + 1).max(0) <= 400 {
                break;
            }
            level += 1;
        }
        let src = source.key();
        let (coloring, colouring) = match paint {
            Paint::Asked(c) => (Some(c), c.key()),
            Paint::Held(k) => (None, k),
        };
        let job = |key: TileKey, coloring: &Coloring| Job {
            key,
            source: source.clone(),
            coloring: coloring.clone(),
            colouring,
        };
        let (center, zoom, clock) = (self.center, self.zoom, self.clock);
        let screen = |c: Vec2| rect.center() + (c - center) * zoom;
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let mut complete = true;
        let mut recolour = if coloring.is_some() { 8 } else { 0 };
        // the top tiles over the layer's cells in view first: something to show at once
        let (tx0, ty0, tx1, ty1) = range(top);
        for ty in ty0..=(ty1.max(0) as usize) {
            for tx in tx0..=(tx1.max(0) as usize) {
                let k = (src, top, tx, ty);
                // (kept while in view: what is drawn while finer tiles come, never let go and asked again)
                match self.tiles.get_mut(&k) {
                    Some(t) if t.texture.as_ref().is_some_and(|(c, _)| *c == colouring) => t.used = clock,
                    _ => {
                        if let Some(c) = coloring {
                            wanted.push((-1.0, job(k, c)));
                        }
                    }
                }
            }
        }
        let (x0, y0, x1, y1) = range(level);
        if x1 < x0 as i64 || y1 < y0 as i64 {
            return complete;
        }
        for ty in y0..=y1 as usize {
            for tx in x0..=x1 as usize {
                let k = (src, level, tx, ty);
                let vs = re.vertices(level, tx, ty);
                // a tile kept in another colouring is coloured again here (a few per frame)
                if let Some(t) = self.tiles.get_mut(&k)
                    && t.texture.as_ref().is_none_or(|(c, _)| *c != colouring)
                    && recolour > 0
                    && let Some(coloring) = coloring
                {
                    recolour -= 1;
                    let img = image_f32(&t.values, TILE, TILE, coloring);
                    t.opaque = opaque(&img);
                    t.texture = Some((
                        colouring,
                        ctx.load_texture(format!("tile-r-{clock}-{tx}-{ty}"), img, TextureOptions::NEAREST),
                    ));
                }
                let drawn = match self.tiles.get_mut(&k) {
                    Some(t) if t.texture.as_ref().is_some_and(|(c, _)| *c == colouring) => {
                        t.used = clock;
                        if show {
                            let id = t.texture.as_ref().unwrap().1.id();
                            out.push(crate::reproject::mesh(&vs, id, full, tint, re.half, screen).into());
                        }
                        true
                    }
                    _ => false,
                };
                if drawn {
                    continue;
                }
                if let Some(c) = coloring {
                    // (how far its middle is from the view's)
                    let mid = vs[vs.len() / 2];
                    let d = if mid[0].is_finite() && mid[1].is_finite() {
                        (screen(Vec2::new(mid[0], mid[1])) - rect.center()).length()
                    } else {
                        f32::MAX
                    };
                    wanted.push((d, job(k, c)));
                }
                // meanwhile the finest coarser tile kept over it, stretched
                let mut stretched = false;
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
                            out.push(crate::reproject::mesh(&vs, tex.id(), uv, tint, re.half, screen).into());
                        }
                        stretched = true;
                        break;
                    }
                }
                complete &= stretched;
            }
        }
        complete
    }

    /// The cell under a screen point.
    fn to_cell(&self, rect: Rect, p: Pos2) -> Vec2 {
        self.center + (p - rect.center()) / self.zoom
    }

    /// Selecting by area: a drag with shift down begins a rectangle (with alt too: a lasso), the pointer shapes
    /// it, a release ends it (`area_done`: with ctrl held, to add), escape lets it go. `allowed`: no drawing or
    /// measuring, and vector layers to select from.
    fn select_area(&mut self, ui: &Ui, response: &egui::Response, rect: Rect, allowed: bool) {
        // (the keys held at the last press on the map: the events' own, as a click's)
        let pressed = ui.input(|i| {
            i.events.iter().rev().find_map(|e| match e {
                egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers }
                    if rect.contains(*pos) =>
                {
                    Some(*modifiers)
                }
                _ => None,
            })
        });
        if let Some(m) = pressed {
            self.press = m;
        }
        let now = ui.input(|i| i.modifiers);
        let cell = |s: &Self, p: Pos2| {
            let c = s.to_cell(rect, p);
            [f64::from(c.x), f64::from(c.y)]
        };
        if allowed && response.drag_started_by(egui::PointerButton::Primary) && (self.press.shift || now.shift) {
            let origin = ui.input(|i| i.pointer.press_origin()).or_else(|| response.interact_pointer_pos());
            if let Some(p) = origin {
                self.selecting = true;
                self.area = Some(crate::area::Drag::new(self.press.alt || now.alt, cell(self, p)));
            }
        }
        if !self.selecting {
            return;
        }
        if self.area.is_some() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.area = None;
        }
        if let Some(p) = response.interact_pointer_pos() {
            let (c, step) = (cell(self, p), f64::from(2.0 / self.zoom.max(1e-6)));
            if let Some(d) = self.area.as_mut() {
                d.push(c, step);
            }
        }
        if response.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
            self.selecting = false;
            if let Some(shape) = self.area.take().and_then(|d| d.polygon()) {
                self.area_done = Some(crate::area::Done { shape, add: self.press.command || now.command });
            }
        }
    }

    /// The coordinate grid over the map: thin light lines at round values of its own coordinates, their values at
    /// the view's top (columns) and left (rows) edges, in white with a dark halo.
    fn paint_graticule(&self, painter: &egui::Painter, rect: Rect, grid: &terraflux_raster::Grid) {
        let Some(g) = crate::nav::graticule(grid, self.center, self.zoom, rect.size()) else { return };
        let (dark, light) = (Color32::from_black_alpha(70), Color32::from_white_alpha(150));
        let font = egui::FontId::proportional(10.5);
        let c = rect.center();
        let label = |at: Pos2, text: &str, anchor: egui::Align2| {
            let halo = painter.layout_no_wrap(text.to_string(), font.clone(), Color32::from_black_alpha(210));
            let ink = painter.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE);
            let min = anchor.anchor_size(at, ink.size()).min;
            for d in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                painter.galley(min + Vec2::new(d.0, d.1), halo.clone(), Color32::BLACK);
            }
            painter.galley(min, ink, Color32::WHITE);
        };
        for (dx, text) in &g.xs {
            let x = c.x + dx;
            let line = [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())];
            painter.line_segment(line, egui::Stroke::new(2.0, dark));
            painter.line_segment(line, egui::Stroke::new(0.8, light));
            label(Pos2::new(x + 2.0, rect.top() + 2.0), text, egui::Align2::LEFT_TOP);
        }
        for (dy, text) in &g.ys {
            let y = c.y + dy;
            let line = [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)];
            painter.line_segment(line, egui::Stroke::new(2.0, dark));
            painter.line_segment(line, egui::Stroke::new(0.8, light));
            label(Pos2::new(rect.left() + 3.0, y - 1.0), text, egui::Align2::LEFT_BOTTOM);
        }
    }

    /// The place found by *Go to*: a ring and a crosshair in the accent over a white and dark outline, seen on any
    /// colour; a word beside it when it is outside the map.
    fn paint_mark(&self, painter: &egui::Painter, rect: Rect) {
        let Some(m) = &self.mark else { return };
        self.paint_mark_at(painter, rect, m.cell, m.inside);
    }

    /// The mark of a view beside, at the same ground: its cell carried to this view's grid by the link between them.
    pub fn paint_mark_beside(&self, ui: &Ui, mark: &Mark, link: &crate::reproject::Link) {
        let Some(rect) = self.rect else { return };
        let c = link.to_own(Vec2::new(mark.cell[0] as f32, mark.cell[1] as f32));
        if c.x.is_finite() && c.y.is_finite() {
            let inside = self.shown_grid.as_ref().is_none_or(|g| {
                c.x >= 0.0 && c.y >= 0.0 && (c.x as f64) < g.width as f64 && (c.y as f64) < g.height as f64
            });
            self.paint_mark_at(&ui.painter_at(rect), rect, [f64::from(c.x), f64::from(c.y)], inside);
        }
    }

    fn paint_mark_at(&self, painter: &egui::Painter, rect: Rect, cell: [f64; 2], inside: bool) {
        let p = rect.center() + (Vec2::new(cell[0] as f32, cell[1] as f32) - self.center) * self.zoom;
        if !rect.expand(20.0).contains(p) {
            return;
        }
        for stroke in [
            egui::Stroke::new(4.5, Color32::from_black_alpha(160)),
            egui::Stroke::new(3.0, Color32::WHITE),
            egui::Stroke::new(1.5, crate::theme::accent()),
        ] {
            painter.circle_stroke(p, 7.0, stroke);
            for s in [-1.0f32, 1.0] {
                painter.line_segment([p + Vec2::new(s * 7.0, 0.0), p + Vec2::new(s * 15.0, 0.0)], stroke);
                painter.line_segment([p + Vec2::new(0.0, s * 7.0), p + Vec2::new(0.0, s * 15.0)], stroke);
            }
        }
        if !inside {
            let galley =
                painter.layout_no_wrap("outside the map".into(), egui::FontId::proportional(11.0), Color32::WHITE);
            let at = p + Vec2::new(18.0, -galley.size().y / 2.0);
            let back = Rect::from_min_size(at, galley.size()).expand2(Vec2::new(4.0, 2.0));
            painter.rect_filled(back, 3.0, Color32::from_black_alpha(170));
            painter.galley(at, galley, Color32::WHITE);
        }
    }

    /// The scale bar, at the view's lower left (the right while a line is measured: its numbers are at the left),
    /// on the theme's colours over a translucent ground: a round length of 60 to 150 points, in kilometres, metres
    /// or cells.
    fn paint_scale(&mut self, ui: &Ui, painter: &egui::Painter, rect: Rect, grid: &terraflux_raster::Grid) {
        self.scale_drawn = None;
        if !self.aids.scale_bar || rect.width() < 200.0 || rect.height() < 120.0 {
            return;
        }
        let Some(bar) = crate::nav::scale_bar(grid, self.center, self.zoom) else { return };
        let visuals = ui.visuals();
        let ink = visuals.text_color();
        let galley = painter.layout_no_wrap(bar.label.clone(), egui::FontId::proportional(11.0), ink);
        let width = bar.px.max(galley.size().x) + 12.0;
        let height = galley.size().y + 14.0;
        // (above the pointer's coordinates, which are at the very bottom)
        let corner = if self.measuring.is_some() {
            Pos2::new(rect.right() - 8.0 - width, rect.bottom() - 26.0 - height)
        } else {
            Pos2::new(rect.left() + 8.0, rect.bottom() - 26.0 - height)
        };
        let back = Rect::from_min_size(corner, Vec2::new(width, height));
        painter.rect_filled(back, 3.0, visuals.window_fill().gamma_multiply(0.78));
        painter.galley(corner + Vec2::new(6.0, 3.0), galley, ink);
        let y = back.bottom() - 6.0;
        let (x0, x1) = (corner.x + 6.0, corner.x + 6.0 + bar.px);
        let stroke = egui::Stroke::new(2.0, ink);
        painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], stroke);
        for x in [x0, x1] {
            painter.line_segment([Pos2::new(x, y - 5.0), Pos2::new(x, y + 1.0)], stroke);
        }
        self.scale_drawn = Some(bar);
    }

    /// The box of *Go to*, at the view's top while it is open: a place typed, in degrees or the map's units; enter
    /// or Go goes there, escape closes it.
    fn goto_box(&mut self, ui: &Ui, rect: Rect, grid: &terraflux_raster::Grid) {
        let Some(mut b) = self.goto.take() else { return };
        // (escape first, so that the text field does not take it)
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            return;
        }
        let plain = crate::measure::Frame::of(grid) == crate::measure::Frame::Plain;
        if plain {
            b.degrees = false;
        }
        let (mut go, mut close) = (false, false);
        egui::Area::new(ui.id().with("goto box"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(rect.center_top() + Vec2::new(0.0, 10.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(400.0);
                    ui.strong("Go to");
                    ui.horizontal(|ui| {
                        let hint = if b.degrees { "12.5, 122.25" } else { "x y" };
                        let edit = ui.add(egui::TextEdit::singleline(&mut b.text).desired_width(240.0).hint_text(hint));
                        if b.focus {
                            edit.request_focus();
                            b.focus = false;
                        }
                        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            go = true;
                        }
                        if ui.button("Go").clicked() {
                            go = true;
                        }
                        if ui.small_button("Close").clicked() {
                            close = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(!plain, |ui| {
                            ui.selectable_value(&mut b.degrees, true, "degrees");
                        });
                        ui.selectable_value(&mut b.degrees, false, "map units");
                        // (a scale needs units on the ground)
                        ui.add_enabled_ui(!plain, |ui| {
                            egui::ComboBox::from_id_salt("goto zoom")
                                .selected_text(b.zoom.label())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut b.zoom, crate::goto::Zoom::Keep, "keep the zoom");
                                    for n in crate::goto::SCALES {
                                        let z = crate::goto::Zoom::Scale(n);
                                        ui.selectable_value(&mut b.zoom, z, z.label());
                                    }
                                })
                                .response
                                .on_hover_text(
                                    "the zoom after going: as it is, or a scale (a width of view or another scale \
                                     can be typed after an @: -15.8, -47.9 @ 1:50000, or @ 5 km)",
                                );
                        });
                    });
                    let help = if b.degrees {
                        crate::goto::HINT.to_string()
                    } else {
                        "x y in the coordinates of the map (its system's units)".to_string()
                    };
                    ui.label(egui::RichText::new(help).weak().size(10.5));
                    if let Some(e) = &b.error {
                        ui.colored_label(ui.visuals().error_fg_color, e);
                    }
                    if let Some(n) = &b.note {
                        ui.label(egui::RichText::new(n).size(11.0));
                    }
                });
            });
        if go {
            match self.go_to_zoom(&b.text, b.degrees, b.zoom) {
                Ok(true) => close = true,
                Ok(false) => {
                    b.error = None;
                    b.note = Some("outside the map: gone there all the same".into());
                }
                Err(e) => {
                    b.error = Some(e);
                    b.note = None;
                    b.focus = true;
                }
            }
        }
        if !close {
            self.goto = Some(b);
        }
    }

    fn take_made(&mut self, ctx: &egui::Context) {
        let Some(w) = &self.workers else { return };
        // (a few textures a frame: hundreds made at once, a folder of rasters opened, would hold the window)
        let made: Vec<Made> = w.done.try_iter().take(MADE_PER_FRAME).collect();
        if made.len() == MADE_PER_FRAME {
            ctx.request_repaint();
        }
        for m in made {
            self.asked.remove(&(m.key, m.colouring));
            match m.values {
                Err(e) => self.error = Some(e),
                Ok(values) => {
                    self.clock += 1;
                    let opaque = m.opaque;
                    let texture = m.image.map(|img| {
                        (m.colouring, ctx.load_texture(format!("tile-{}", self.clock), img, TextureOptions::NEAREST))
                    });
                    match self.tiles.get_mut(&m.key) {
                        Some(t) => {
                            t.values = values;
                            if texture.is_some() {
                                t.texture = texture;
                                t.opaque = opaque;
                            }
                        }
                        None => {
                            let tile = Tile { values, texture, opaque, used: self.clock, _source: m.source };
                            self.tiles.insert(m.key, tile);
                        }
                    }
                }
            }
        }
        // the oldest let go, never those of the last frame (many layers in view need more than KEPT: letting
        // those go asked them again at once, without end)
        if self.tiles.len() > KEPT {
            let frame = self.frame_clock;
            let mut by_age: Vec<(u64, TileKey)> =
                self.tiles.iter().filter(|(_, t)| t.used < frame).map(|(k, t)| (t.used, *k)).collect();
            by_age.sort_unstable();
            let over = self.tiles.len() - KEPT;
            for (_, k) in by_age.into_iter().take(over) {
                self.tiles.remove(&k);
            }
        }
    }

    /// The values of a colour composite's three bands at a cell, read once for the cell the pointer stays on (on
    /// disk each is a window read).
    fn rgb_cell(&self, r: &crate::rgb::RgbSource, x: usize, y: usize) -> [f64; 3] {
        let key = r.key();
        let mut m = self.rgb_memo.lock().unwrap();
        if let Some((k, mx, my, v)) = *m
            && k == key
            && (mx, my) == (x, y)
        {
            return v;
        }
        let v = r.cell(x, y);
        *m = Some((key, x, y, v));
        v
    }

    /// The value of a cell from the finest tile kept over it (a map in memory: the cell itself).
    fn value_at(&self, source: &Source, x: usize, y: usize) -> f64 {
        // (a colour composite: the red channel's value, none when a band has none)
        if let Source::Rgb(r) = source {
            let v = self.rgb_cell(r, x, y);
            return if v.iter().any(|x| x.is_nan()) { f64::NAN } else { v[0] };
        }
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

    /// The value of a map drawn over the view's at a cell of the view (none outside it).
    fn layer_value(&self, l: &RasterLayer, how: At, at: Vec2) -> f64 {
        let c = match how {
            At::Place(place) => place.to_source(at),
            At::Warp => match self.reproj.get(&l.source.key()).and_then(|re| re.to_source(at)) {
                Some(c) => c,
                None => return f64::NAN,
            },
        };
        let (w, h) = l.source.size();
        if c.x < 0.0 || c.y < 0.0 || c.x >= w as f32 || c.y >= h as f32 {
            return f64::NAN;
        }
        self.value_at(&l.source, c.x as usize, c.y as usize)
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
        self.show_compared(ui, source, coloring, rasters, overlays, None)
    }

    /// [`MapView::show_stacked`], and another map compared with it right of a curtain (dragged across the view by
    /// its handle): the map shown at the left, the other at the right, the values of both under the pointer.
    pub fn show_compared(
        &mut self,
        ui: &mut Ui,
        source: &Source,
        coloring: &Coloring,
        rasters: &[RasterLayer],
        overlays: &[crate::vector_tiles::Overlay],
        curtain: Option<&Curtain>,
    ) -> Option<Hover> {
        let ctx = ui.ctx().clone();
        self.drawn_pass = ctx.cumulative_pass_nr();
        self.layers_drawn = (rasters.len(), overlays.len());
        self.pointer = None;
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
            // (another map: its views, and the place found on the last, are not this one's)
            self.history.clear();
            self.mark = None;
        }
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.rect = Some(rect);
        if self.shown_grid.as_ref() != Some(source.grid()) {
            self.shown_grid = Some(source.grid().clone());
        }
        if w == 0 || h == 0 {
            return None;
        }
        // (a double click while drawing ends a feature)
        if !self.fitted || (response.double_clicked() && self.drawing.is_none() && self.measuring.is_none()) {
            self.zoom = (rect.width() / w as f32).min(rect.height() / h as f32).max(1e-6);
            self.center = Vec2::new(w as f32 / 2.0, h as f32 / 2.0);
            self.fitted = true;
        }
        // a Plot's saved view asked for: the view on that box (a box away from the map is let go)
        if let Some(e) = self.frame.take()
            && let Some(cells) = crate::frame::view_cells(source.grid(), e)
        {
            self.zoom_to(cells);
        }
        // a comparison: the curtain's handle (over the map, so that dragging it does not move the map), and the
        // other map's place on this one (none: of a coordinate system that cannot be carried to this one)
        let compared = curtain.and_then(|c| self.layer_at(&c.layer, source.grid()).ok().map(|at| (c, at)));
        let split = compared.is_some().then(|| {
            let x = rect.left() + rect.width() * self.curtain();
            let handle = ui
                .interact(
                    Rect::from_center_size(Pos2::new(x, rect.center().y), Vec2::new(16.0, rect.height())),
                    ui.id().with("curtain"),
                    Sense::drag(),
                )
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if handle.dragged()
                && let Some(p) = handle.interact_pointer_pos()
            {
                self.curtain = Some(((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0));
            }
            rect.left() + rect.width() * self.curtain()
        });
        // a vertex of a feature drawn dragged: the map stays
        let editing = match self.drawing.as_mut() {
            Some(sketch) => {
                let (zoom, center) = (self.zoom, self.center);
                let to_cell = |p: Pos2| center + (p - rect.center()) / zoom;
                crate::draw::drag(sketch, ui, &response, to_cell, |c| rect.center() + (c - center) * zoom)
            }
            None => false,
        };
        // shift+drag: a rectangle of features to select (shift+alt: a lasso); not while features are drawn or a
        // line measured; ctrl adds them to the selection
        self.select_area(
            ui,
            &response,
            rect,
            !overlays.is_empty() && self.drawing.is_none() && self.measuring.is_none(),
        );
        if response.dragged() && !editing && !self.selecting {
            self.center -= response.drag_delta() / self.zoom;
        }
        // B over the map: the satellite's imagery under it, or none
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        if response.hovered() && !typing && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::B)) {
            self.basemap = if self.basemap == Basemap::None { Basemap::EsriImagery } else { Basemap::None };
        }
        // (features drawn take the place of the line measured)
        if self.drawing.is_some() {
            self.measuring = None;
        }
        // M over the map: measuring, or not (not while drawing)
        // (what was asked of the views side by side by the last frame has been done by now)
        self.measure_stopped = false;
        if response.hovered() && !typing && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::M)) {
            self.set_measuring(self.measuring.is_none());
            self.measure_stopped = self.measuring.is_none();
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
        // alt+left, alt+right, the mouse's back and forward buttons: the view before, the view after
        let (back, forward) = if response.hovered() && !typing {
            ui.input_mut(|i| {
                let back = i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowLeft)
                    | i.pointer.button_pressed(egui::PointerButton::Extra1);
                let forward = i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowRight)
                    | i.pointer.button_pressed(egui::PointerButton::Extra2);
                (back, forward)
            })
        } else {
            (false, false)
        };
        if back {
            self.back();
        }
        if forward {
            self.forward();
        }
        // Ctrl+G over the map: the box to go to a place
        if response.hovered() && !typing && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::G)) {
            self.toggle_goto();
        }
        // escape over the map lets the place found go (not while the box, a line or a shape wants it)
        if (self.mark.is_some() || self.mark_beside)
            && self.goto.is_none()
            && self.measuring.is_none()
            && self.drawing.is_none()
            && !self.selecting
            && response.hovered()
            && !typing
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.mark = None;
            self.mark_stopped = true;
        }
        // (set again every frame by views side by side)
        self.mark_beside = false;
        // the view as it stands: kept in the history once it has been still for a while
        if self.history.watch((self.zoom, self.center), crate::nav::now()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(450));
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        // the world under the data, then the data over it as transparent as asked
        if self.basemap != Basemap::None {
            self.draw_base(&ctx, &painter, rect, source.grid());
        }
        let tint = Color32::from_white_alpha(((1.0 - self.transparency.clamp(0.0, 1.0)) * 255.0).round() as u8);
        let show_data = !self.hide_data && !blank;
        // (the map shown as opaque as its layer is: the maps compared with it keep the data's)
        let own = tint.gamma_multiply(self.opacity.unwrap_or(1.0).clamp(0.0, 1.0));
        let ppp = ctx.pixels_per_point();
        self.clock += 1;
        self.frame_clock = self.clock;
        // The maps are drawn from the top down, each into the place kept for it in the painter (the lowest
        // first), so that what a map hides of those under it ([`Cover`]) is known before they are drawn: a
        // stack of years shows its top one, and only its tiles are read. The tiles wanted from the top map down,
        // each map's the nearest the centre first.
        let mut cover = Cover { pointer: response.hover_pos(), ..Default::default() };
        let mut ranked: Vec<(usize, f32, Job)> = Vec::new();
        let mut wanted: Vec<(f32, Job)> = Vec::new();
        let below: Vec<&RasterLayer> = rasters.iter().filter(|l| l.below).collect();
        let over: Vec<&RasterLayer> = rasters.iter().filter(|l| !l.below).collect();
        let below_at: Vec<egui::layers::ShapeIdx> = below.iter().map(|_| painter.add(egui::Shape::Noop)).collect();
        // compared: this map left of the curtain, the other right of it, as opaque as this one
        let (left_clip, right_clip) = match split {
            Some(x) => (
                Some(Rect::from_min_max(rect.min, Pos2::new(x, rect.max.y))),
                Some(Rect::from_min_max(Pos2::new(x, rect.min.y), rect.max)),
            ),
            None => (None, None),
        };
        let left = left_clip.map_or_else(|| painter.clone(), |c| painter.with_clip_rect(c));
        let right = right_clip.map_or_else(|| painter.clone(), |c| painter.with_clip_rect(c));
        let (prior_at, source_at, compared_at) =
            (left.add(egui::Shape::Noop), left.add(egui::Shape::Noop), right.add(egui::Shape::Noop));
        let over_at: Vec<egui::layers::ShapeIdx> = over.iter().map(|_| painter.add(egui::Shape::Noop)).collect();
        let mut rank = 0;
        let mut rank_wanted = |wanted: &mut Vec<(f32, Job)>, ranked: &mut Vec<(usize, f32, Job)>| {
            ranked.extend(wanted.drain(..).map(|(d, j)| (rank, d, j)));
            rank += 1;
        };
        // the maps over it, the last on top
        let (mut over_errors, mut below_errors) = (Vec::new(), Vec::new());
        for (l, &at_slot) in over.iter().zip(&over_at).rev() {
            match self.layer_at(l, source.grid()) {
                Ok(at) => {
                    let tint = Color32::from_white_alpha((l.opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
                    let paint = Paint::Asked(&l.coloring);
                    let mut out = Vec::new();
                    self.draw_at(
                        &ctx,
                        &mut out,
                        rect,
                        &l.source,
                        paint,
                        tint,
                        at,
                        (true, l.blend),
                        &mut wanted,
                        &mut cover,
                    );
                    painter.set(at_slot, egui::Shape::Vec(out));
                }
                Err(e) => over_errors.push(e),
            }
            cover.drawn();
            rank_wanted(&mut wanted, &mut ranked);
        }
        if !blank {
            let base = Place { offset: Vec2::ZERO, scale: Vec2::splat(1.0) };
            let mut hidden_by_compared = Vec::new();
            if let (Some((c, at)), Some(_)) = (compared, split) {
                let l = &c.layer;
                let paint = Paint::Asked(&l.coloring);
                let mut out = Vec::new();
                cover.clip = right_clip;
                self.draw_at(
                    &ctx,
                    &mut out,
                    rect,
                    &l.source,
                    paint,
                    tint,
                    at,
                    (show_data, Blend::Normal),
                    &mut wanted,
                    &mut cover,
                );
                right.set(compared_at, egui::Shape::Vec(out));
                // (what the other map hides is right of the curtain: not over this one)
                hidden_by_compared = std::mem::take(&mut cover.opaque);
                rank_wanted(&mut wanted, &mut ranked);
            }
            cover.clip = left_clip;
            // another map than at the last frame (a series' next step), or other colours: the last one kept under
            // it while its tiles come, the new one fading in over it
            let now = (source.key(), coloring.key());
            if let Some((was, colouring)) = self.last.take()
                && (was.key(), colouring) != now
                && was.size() == (w, h)
            {
                self.prior = Some(Prior { source: was, colouring, since: std::time::Instant::now() });
            }
            let under = match &self.prior {
                Some(p) if (p.source.key(), p.colouring) != now => {
                    Some((p.source.clone(), p.colouring, (p.since.elapsed().as_secs_f32() / FADE).min(1.0)))
                }
                _ => None,
            };
            let fade = under.as_ref().map_or(1.0, |u| u.2);
            let asked = Paint::Asked(coloring);
            let faded = own.gamma_multiply(fade);
            let mut out = Vec::new();
            let complete = self.draw_source(
                &ctx,
                &mut out,
                rect,
                source,
                asked,
                faded,
                base,
                (show_data, Blend::Normal),
                &mut wanted,
                &mut cover,
            );
            left.set(source_at, egui::Shape::Vec(out));
            cover.drawn();
            if let Some((was, colouring, _)) = under {
                let held = Paint::Held(colouring);
                let mut out = Vec::new();
                self.draw_source(
                    &ctx,
                    &mut out,
                    rect,
                    &was,
                    held,
                    own,
                    base,
                    (show_data, Blend::Normal),
                    &mut wanted,
                    &mut cover,
                );
                left.set(prior_at, egui::Shape::Vec(out));
                cover.opaque.clear();
            }
            if self.prior.is_some() {
                if complete && fade >= 1.0 {
                    self.prior = None;
                } else {
                    ctx.request_repaint();
                }
            }
            self.last = Some((source.clone(), now.1));
            cover.clip = None;
            cover.hidden.append(&mut hidden_by_compared);
            rank_wanted(&mut wanted, &mut ranked);
        }
        // the maps under it, placed by their coordinates on its grid (of its coordinate system)
        for (l, &at_slot) in below.iter().zip(&below_at).rev() {
            match self.layer_at(l, source.grid()) {
                Ok(at) => {
                    let tint = Color32::from_white_alpha((l.opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
                    let paint = Paint::Asked(&l.coloring);
                    let mut out = Vec::new();
                    self.draw_at(
                        &ctx,
                        &mut out,
                        rect,
                        &l.source,
                        paint,
                        tint,
                        at,
                        (true, l.blend),
                        &mut wanted,
                        &mut cover,
                    );
                    painter.set(at_slot, egui::Shape::Vec(out));
                }
                Err(e) => below_errors.push(e),
            }
            cover.drawn();
            rank_wanted(&mut wanted, &mut ranked);
        }
        below_errors.reverse();
        over_errors.reverse();
        let mut layer_errors = below_errors;
        layer_errors.append(&mut over_errors);
        if curtain.is_some() && compared.is_none() {
            layer_errors.push(
                "the coordinate system of the map compared is unknown or cannot be carried to this map's: put it on \
                 this map's grid (raster.match)",
            );
        }
        // the workers' queue: what the view needs, the top map's first, each map's nearest the centre first; the
        // rest let go
        ranked.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let wanted = ranked.into_iter().map(|(_, d, j)| (d, j));
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
        // the curtain: a line with a handle, the names of the two maps at the top of their sides
        if let (Some((c, _)), Some(x)) = (compared, split) {
            let accent = crate::theme::accent();
            painter.vline(x, rect.y_range(), egui::Stroke::new(3.0, Color32::from_black_alpha(120)));
            painter.vline(x, rect.y_range(), egui::Stroke::new(1.5, Color32::WHITE));
            let knob = Pos2::new(x, rect.center().y);
            painter.circle(knob, 11.0, accent, egui::Stroke::new(1.5, Color32::WHITE));
            for side in [-1.0f32, 1.0] {
                let tip = knob + Vec2::new(side * 7.0, 0.0);
                let back = knob + Vec2::new(side * 2.5, 0.0);
                painter.add(egui::Shape::convex_polygon(
                    vec![tip, back + Vec2::new(0.0, -4.0), back + Vec2::new(0.0, 4.0)],
                    Color32::WHITE,
                    egui::Stroke::NONE,
                ));
            }
            let font = egui::FontId::proportional(12.0);
            for (k, name) in c.names.iter().enumerate() {
                let galley = painter.layout_no_wrap(name.clone(), font.clone(), Color32::WHITE);
                let at = if k == 0 {
                    Pos2::new(x - 10.0 - galley.size().x, rect.top() + 8.0)
                } else {
                    Pos2::new(x + 10.0, rect.top() + 8.0)
                };
                let back = Rect::from_min_size(at, galley.size()).expand2(Vec2::new(5.0, 2.0));
                painter.rect_filled(back, 3.0, Color32::from_black_alpha(160));
                painter.galley(at, galley, Color32::WHITE);
            }
        }
        // the place of the pointer in the view beside this one: a cross
        if let Some(g) = self.ghost {
            let p = rect.center() + (g - self.center) * self.zoom;
            if rect.contains(p) {
                for stroke in [
                    egui::Stroke::new(3.5, Color32::from_black_alpha(140)),
                    egui::Stroke::new(1.5, crate::theme::accent()),
                ] {
                    painter.line_segment([p - Vec2::new(9.0, 0.0), p + Vec2::new(9.0, 0.0)], stroke);
                    painter.line_segment([p - Vec2::new(0.0, 9.0), p + Vec2::new(0.0, 9.0)], stroke);
                }
            }
        }
        // finding one's way: the coordinate grid, the place found, the scale bar
        if self.aids.grid {
            self.paint_graticule(&painter, rect, source.grid());
        }
        self.paint_mark(&painter, rect);
        self.paint_scale(ui, &painter, rect, source.grid());
        self.goto_box(ui, rect, source.grid());
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
            let text = self.base.error.clone().unwrap_or_else(|| self.basemap.attribution());
            // (a service of the user's may have no attribution to write)
            if !text.is_empty() {
                let at = rect.right_bottom() - Vec2::new(6.0, 4.0);
                let galley = painter.layout_no_wrap(text, egui::FontId::proportional(10.5), Color32::WHITE);
                let back = Rect::from_min_size(at - galley.size(), galley.size()).expand2(Vec2::new(4.0, 2.0));
                painter.rect_filled(back, 2.0, Color32::from_black_alpha(140));
                painter.galley(at - galley.size(), galley, Color32::WHITE);
            }
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
                    let cell = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let centre = rect.center() + (cell - self.center) * self.zoom;
                    // (right of a curtain: the other map's value, in its colours)
                    let (v, colours) = match (compared, split) {
                        (Some((c, how)), Some(at)) if centre.x > at => {
                            (self.layer_value(&c.layer, how, cell), &c.layer.coloring)
                        }
                        _ => (self.value_at(source, x, y), coloring),
                    };
                    if v.is_nan() {
                        continue;
                    }
                    // dark on a light cell, light on a dark one
                    let c = colours.colour(v);
                    let light = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32 > 140.0;
                    let ink = if light && self.transparency < 0.5 { Color32::BLACK } else { Color32::WHITE };
                    painter.text(centre, egui::Align2::CENTER_CENTER, cell_text(v), font.clone(), ink);
                }
            }
        }
        // the patch of the cell traced, its border drawn (white under the accent, seen on any colour)
        if let Some(o) = self.patch.as_ref().filter(|_| !blank) {
            let at = |(x, y): (f32, f32)| rect.center() + (Vec2::new(x, y) - self.center) * self.zoom;
            let clip = painter.clip_rect();
            let strokes = [egui::Stroke::new(3.0, Color32::WHITE), egui::Stroke::new(1.5, crate::theme::accent())];
            if o.edges.is_empty() || o.edges.len() > OUTLINE_SIDES {
                let (x0, y0, x1, y1) = o.bounds;
                let r = Rect::from_two_pos(at((x0, y0)), at((x1, y1)));
                for s in strokes {
                    painter.rect_stroke(r, 0.0, s, egui::StrokeKind::Middle);
                }
            } else {
                for s in strokes {
                    for [a, b] in &o.edges {
                        let (p, q) = (at(*a), at(*b));
                        if clip.intersects(Rect::from_two_pos(p, q).expand(1.0)) {
                            painter.line_segment([p, q], s);
                        }
                    }
                }
            }
        }
        // the feature selected in the table of attributes, outlined (white under the accent), a point ringed
        if let Some((l, ks)) = &self.selected
            && let Some(o) = overlays.get(*l)
            && let Ok(Some(layer)) = self.vectors.layer(&ctx, &o.vector, source.grid())
        {
            let at = |p: &[f64; 2]| rect.center() + (Vec2::new(p[0] as f32, p[1] as f32) - self.center) * self.zoom;
            let clip = painter.clip_rect();
            let strokes = [egui::Stroke::new(3.5, Color32::WHITE), egui::Stroke::new(2.0, crate::theme::accent())];
            // only those in view (by their boxes), and no more than a few hundred of them
            let seen = ks
                .iter()
                .filter(|&&k| {
                    layer.feature_box(k).is_some_and(|b| {
                        clip.expand(8.0).intersects(Rect::from_two_pos(at(&[b[0], b[1]]), at(&[b[2], b[3]])))
                    })
                })
                .filter_map(|&k| layer.shape(k))
                .take(SELECTED_DRAWN)
                .collect::<Vec<_>>();
            for s in strokes {
                for (kind, parts) in &seen {
                    let kind = *kind;
                    for part in parts {
                        if kind == crate::vector_tiles::Kind::Point {
                            for p in part.iter() {
                                painter.circle_stroke(at(p), 7.0, s);
                            }
                            continue;
                        }
                        let n = part.len();
                        let closed = kind == crate::vector_tiles::Kind::Polygon;
                        for i in 0..n.saturating_sub(1) + usize::from(closed && n > 2) {
                            let (a, b) = (at(&part[i]), at(&part[(i + 1) % n]));
                            if clip.intersects(Rect::from_two_pos(a, b).expand(2.0)) {
                                painter.line_segment([a, b], s);
                            }
                        }
                    }
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
        // a right click: why the cell has its value (its menu drawn whether or not the pointer is over the map:
        // it stays open while the pointer goes to it; the cell picked by the click that opens it, below)
        if !blank {
            response.context_menu(|ui| {
                if let Some((row, col)) = self.picked {
                    ui.label(egui::RichText::new(format!("row {row}, column {col}")).weak().small());
                }
                if ui.button("Trace this cell").on_hover_text("the chain of values that made it").clicked() {
                    self.trace_request = Some(crate::trace_view::Explain::Here);
                    ui.close();
                }
                if ui
                    .button("Ask the assistant why")
                    .on_hover_text("its trace explained in words by the application's assistant")
                    .clicked()
                {
                    self.trace_request = Some(crate::trace_view::Explain::Assistant);
                    ui.close();
                }
                if ui
                    .button("Explain with an MCP assistant")
                    .on_hover_text("for an assistant through terraflux mcp (Claude Code, Claude Desktop...): the trace kept for it, and a request to paste copied (or its prompt explain_cell)")
                    .clicked()
                {
                    self.trace_request = Some(crate::trace_view::Explain::Mcp);
                    ui.close();
                }
            });
        }
        // the shape being dragged, translucent in the accent
        if let Some(d) = &self.area {
            let (zoom, center) = (self.zoom, self.center);
            d.paint(&painter, |p| rect.center() + (Vec2::new(p[0] as f32, p[1] as f32) - center) * zoom);
        }
        if let Some(mut sketch) = self.drawing.take() {
            let (zoom, center) = (self.zoom, self.center);
            crate::draw::interact(
                &mut sketch,
                ui,
                &response,
                |p| self.to_cell(rect, p),
                |c| rect.center() + (c - center) * zoom,
            );
            self.drawing = Some(sketch);
        }
        if let Some(mut line) = self.measuring.take() {
            let (zoom, center) = (self.zoom, self.center);
            let wish = line.interact(
                ui,
                &response,
                source.grid(),
                typing,
                !self.hide_measure_box,
                |p| self.to_cell(rect, p),
                |c| rect.center() + (c - center) * zoom,
            );
            if wish == crate::measure::Wish::Keep {
                self.measuring = Some(line);
            } else {
                self.measure_stopped = true;
            }
        }
        let p = response.hover_pos()?;
        let c = self.to_cell(rect, p);
        self.pointer = Some(c);
        let g = &source.grid().transform;
        let inside = c.x >= 0.0 && c.y >= 0.0 && c.x < w as f32 && c.y < h as f32;
        // the features under the pointer (4 points around), of each vector layer from the top, their attributes
        // beside them
        let features = if overlays.is_empty() {
            Vec::new()
        } else {
            let tolerance = f64::from(4.0 / self.zoom);
            self.vectors.pick_all(&ctx, overlays, source.grid(), [f64::from(c.x), f64::from(c.y)], tolerance)
        };
        let feature = features.first().copied();
        // (the place found, when the pointer is on its mark, told before the features')
        let mut texts: Vec<String> = Vec::new();
        if let Some(m) = &self.mark {
            let at = rect.center() + (Vec2::new(m.cell[0] as f32, m.cell[1] as f32) - self.center) * self.zoom;
            if at.distance(p) <= MARK_NEAR {
                texts.push(mark_text(m));
            }
        }
        texts.extend(features.iter().map(|&(l, k)| feature_text(&overlays[l], k)));
        if !texts.is_empty() {
            response.clone().on_hover_text_at_pointer(texts.join("\n\n"));
        }
        if response.clicked() {
            self.feature_clicked = features.clone();
            self.feature_toggle = crate::table_view::click_modifiers(ui).command;
        }
        if (response.clicked() || response.secondary_clicked())
            && !blank
            && self.drawing.is_none()
            && self.measuring.is_none()
        {
            let picked = inside.then_some((c.y as usize, c.x as usize));
            if picked != self.picked {
                self.patch = None;
            }
            self.picked = picked;
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
            let said = |v: f64| if v.is_nan() { "no value".to_string() } else { terraflux_ops::value::class_label(v) };
            if inside && !blank {
                let v = match source {
                    // (a colour composite: each channel's band and value)
                    Source::Rgb(r) => r.text(self.rgb_cell(r, c.x as usize, c.y as usize)),
                    _ => said(self.value_at(source, c.x as usize, c.y as usize)),
                };
                text.push_str(&format!("   ·   row {}  column {}", c.y as usize, c.x as usize));
                // compared: the values of both, by their names
                match compared {
                    Some((cmp, how)) => {
                        let w = said(self.layer_value(&cmp.layer, how, c));
                        text.push_str(&format!("   ·   {} {v}   ·   {} {w}", cmp.names[0], cmp.names[1]));
                    }
                    // the maps drawn with it: each one's value by its name, from the top
                    None if rasters.iter().any(|l| !l.name.is_empty()) => {
                        let named = |l: &&RasterLayer| !l.name.is_empty();
                        for l in rasters.iter().rev().filter(|l| !l.below).filter(named) {
                            if let Some(how) = self.placed(l, source.grid()) {
                                text.push_str(&format!("   ·   {} {}", l.name, said(self.layer_value(l, how, c))));
                            }
                        }
                        let name = if self.name.is_empty() { "shown" } else { self.name.as_str() };
                        text.push_str(&format!("   ·   {name} {v}"));
                        for l in rasters.iter().rev().filter(|l| l.below).filter(named) {
                            if let Some(how) = self.placed(l, source.grid()) {
                                text.push_str(&format!("   ·   {} {}", l.name, said(self.layer_value(l, how, c))));
                            }
                        }
                    }
                    None => text.push_str(&format!("   ·   {v}")),
                }
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
            let lines = legend_lines(c, counts, names);
            let top = lines.iter().map(|l| l.2).max().unwrap_or(0).max(1);
            for (col, name, n) in lines {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                    ui.painter().rect_filled(r, 2.0, crate::palette::seen(col));
                    // the count on the right, the name in what is left (cut short when long)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(n.to_string()).weak().monospace());
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(egui::Label::new(name).truncate());
                        });
                    });
                });
                // a thin bar under the row: the class's cells against the largest class's
                let room = ui.available_width();
                let (bar, _) = ui.allocate_exact_size(Vec2::new(room, 3.0), Sense::hover());
                let full = Rect::from_min_size(bar.min, Vec2::new(room * n as f32 / top as f32, 3.0));
                ui.painter().rect_filled(full, 1.0, crate::palette::seen(col));
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
                    crate::palette::seen(ramp.colour(i as f32 / (n - 1) as f32)),
                );
            }
            let scale = if *log { "   (logarithmic)" } else { "" };
            ui.label(format!("{min:.4} to {max:.4}{scale}"));
        }
        // (a colour composite: its channels are told by its layer's legend)
        Coloring::Packed => {
            ui.label("a colour composite of three bands");
        }
        Coloring::Shade { .. } => {}
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
