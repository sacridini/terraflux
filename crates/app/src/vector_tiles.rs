//! Vector features drawn over a grid as tiles, so that layers of millions of features draw and move as smoothly as
//! a map: a layer is prepared once on a thread of its own (reprojected to the grid's coordinate system, its
//! vertices put in the grid's cells, an R-tree of its features' boxes), then threads rasterize the tiles in view
//! (256 x 256 pixels, antialiased by tiny-skia) from the features the tree finds over each, every vertex closer than
//! a pixel to the last drawn skipped and features smaller than a pixel drawn as a dot. The screen only draws
//! textures; until a tile comes the finest coarser one kept is shown, stretched. The feature under the pointer is
//! found by the tree too.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};

use egui::{Color32, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, Envelope, RTree, RTreeObject};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

/// The side of a tile, pixels.
const TILE: usize = 256;
/// The tiles kept, textures of 256 KB: 128 MB at most.
const KEPT: usize = 512;
/// The finest level: a tile's pixel 1/256 of a cell.
const FINEST: i32 = -8;

/// How a layer is drawn: a colour, or the colours of an attribute's values (classes, or a ramp of numbers).
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub colour: Color32,
    pub by: Option<String>,
    /// The polygons' fill, 0 (none) to 1.
    pub fill: f32,
    /// The lines' and the polygons' outlines' width, pixels (0: polygons without outline); points are a circle of
    /// 2 + width pixels across.
    pub width: f32,
}

impl Default for Style {
    fn default() -> Self {
        Style { colour: crate::theme::ACCENT, by: None, fill: 0.2, width: 1.2 }
    }
}

impl Style {
    fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.colour.to_array().hash(&mut h);
        self.by.hash(&mut h);
        self.fill.to_bits().hash(&mut h);
        self.width.to_bits().hash(&mut h);
        h.finish()
    }
}

/// What a feature is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Point,
    Line,
    Polygon,
}

/// A feature: its kind, its parts (rings, lines or points: `parts[k]..parts[k + 1]` of the vertices) and box.
#[derive(Debug, Clone)]
struct Feature {
    kind: Kind,
    first: u32,
    last: u32,
    bbox: [f64; 4],
}

/// A layer ready to draw over a grid: its features in the grid's cells (x the column, y the row, from the corner).
pub struct Layer {
    pub vector: Arc<Vector>,
    features: Vec<Feature>,
    parts: Vec<u32>,
    xy: Vec<[f64; 2]>,
    tree: RTree<GeomWithData<Rectangle<[f64; 2]>, u32>>,
    /// The box of every feature, cells.
    pub bounds: Option<[f64; 4]>,
}

/// A layer's features in a grid's cells: reprojected to its coordinate system when both are known, through the
/// inverse of its geotransform.
pub fn prepare(v: &Arc<Vector>, grid: &Grid) -> Result<Layer, String> {
    let projected = match (&v.crs, crs_of(grid)) {
        (Some(from), Some(to)) if !terraflux_gdal::same_crs(from, &to).unwrap_or(true) => {
            Arc::new(terraflux_gdal::reproject_vector(v, &to)?)
        }
        _ => v.clone(),
    };
    let t = grid.transform;
    let det = t[1] * t[5] - t[2] * t[4];
    if det == 0.0 {
        return Err("the grid's geotransform cannot be inverted".into());
    }
    let to_cell = |p: &[f64; 2]| {
        let (dx, dy) = (p[0] - t[0], p[1] - t[3]);
        [(dx * t[5] - dy * t[2]) / det, (dy * t[1] - dx * t[4]) / det]
    };
    let mut features = Vec::with_capacity(projected.len());
    let mut parts: Vec<u32> = vec![0];
    let mut xy: Vec<[f64; 2]> = Vec::new();
    let mut boxes = Vec::with_capacity(projected.len());
    for g in &projected.geometries {
        let first = (parts.len() - 1) as u32;
        let push = |pts: &[[f64; 2]], xy: &mut Vec<[f64; 2]>, parts: &mut Vec<u32>| {
            xy.extend(pts.iter().map(to_cell));
            parts.push(xy.len() as u32);
        };
        let kind = match g {
            Geometry::Empty => Kind::Point,
            Geometry::Point(p) => {
                push(&[*p], &mut xy, &mut parts);
                Kind::Point
            }
            Geometry::MultiPoint(ps) => {
                for p in ps {
                    push(&[*p], &mut xy, &mut parts);
                }
                Kind::Point
            }
            Geometry::LineString(l) => {
                push(l, &mut xy, &mut parts);
                Kind::Line
            }
            Geometry::MultiLineString(ls) => {
                for l in ls {
                    push(l, &mut xy, &mut parts);
                }
                Kind::Line
            }
            Geometry::Polygon(_) | Geometry::MultiPolygon(_) => {
                for poly in g.polygons() {
                    for ring in poly {
                        push(ring, &mut xy, &mut parts);
                    }
                }
                Kind::Polygon
            }
        };
        let last = (parts.len() - 1) as u32;
        let a = parts[first as usize] as usize;
        let b = parts[last as usize] as usize;
        let mut bbox = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for p in &xy[a..b] {
            bbox = [bbox[0].min(p[0]), bbox[1].min(p[1]), bbox[2].max(p[0]), bbox[3].max(p[1])];
        }
        let k = features.len() as u32;
        if a < b && bbox.iter().all(|v| v.is_finite()) {
            boxes.push(GeomWithData::new(Rectangle::from_corners([bbox[0], bbox[1]], [bbox[2], bbox[3]]), k));
        }
        features.push(Feature { kind, first, last, bbox });
    }
    let bounds = boxes.iter().map(|b| b.geom().envelope()).reduce(|a, b| a.merged(&b)).map(|e| {
        let (l, u) = (e.lower(), e.upper());
        [l[0], l[1], u[0], u[1]]
    });
    let tree = RTree::bulk_load(boxes);
    Ok(Layer { vector: v.clone(), features, parts, xy, tree, bounds })
}

fn crs_of(g: &Grid) -> Option<String> {
    g.geo.wkt.clone().or_else(|| g.geo.epsg().map(|c| format!("EPSG:{c}")))
}

/// Every feature's colour in a style: one, or by the classes or the range of an attribute.
pub fn colours(v: &Vector, style: &Style) -> Vec<Color32> {
    let n = v.len();
    let Some(column) = style.by.as_ref().and_then(|b| v.column(b)) else { return vec![style.colour; n] };
    match column {
        Column::Numbers(x) => {
            let mut distinct: Vec<f64> = x.iter().copied().filter(|v| !v.is_nan()).collect();
            distinct.sort_by(f64::total_cmp);
            distinct.dedup();
            if distinct.len() <= 24 && distinct.iter().all(|v| v.fract() == 0.0) {
                x.iter()
                    .map(|v| match distinct.binary_search_by(|d| d.total_cmp(v)) {
                        Ok(i) => crate::palette::class_colour(i),
                        Err(_) => Color32::GRAY,
                    })
                    .collect()
            } else {
                let (lo, hi) = (distinct.first().copied().unwrap_or(0.0), distinct.last().copied().unwrap_or(1.0));
                let span = if hi > lo { hi - lo } else { 1.0 };
                x.iter()
                    .map(
                        |v| {
                            if v.is_nan() {
                                Color32::GRAY
                            } else {
                                crate::palette::sequential(((v - lo) / span) as f32)
                            }
                        },
                    )
                    .collect()
            }
        }
        Column::Texts(t) => {
            let mut distinct: Vec<&String> = t.iter().collect();
            distinct.sort();
            distinct.dedup();
            t.iter().map(|s| crate::palette::class_colour(distinct.binary_search(&s).unwrap_or(0))).collect()
        }
    }
}

/// A layer over a grid: its features' and the grid's keys.
type LayerKey = (usize, u64);

/// A tile of a layer: (layer, style, level, column, row), at level `l` a pixel is 2^l cells.
type Key = (LayerKey, u64, i32, i64, i64);

/// The cells a tile spans at a level.
fn span(level: i32) -> f64 {
    TILE as f64 * 2f64.powi(level)
}

/// A tile's picture: the features over it, drawn; none when no feature is.
pub fn rasterize(
    layer: &Layer,
    colours: &[Color32],
    style: &Style,
    level: i32,
    tx: i64,
    ty: i64,
) -> Option<egui::ColorImage> {
    let s = span(level);
    let px = 2f64.powi(-level); // pixels per cell
    let (ox, oy) = (tx as f64 * s, ty as f64 * s);
    // the features near, a few pixels around for the strokes and dots
    let margin = (4.0 + 2.0 * style.width as f64) / px;
    let env = AABB::from_corners([ox - margin, oy - margin], [ox + s + margin, oy + s + margin]);
    let mut found: Vec<u32> = layer.tree.locate_in_envelope_intersecting(&env).map(|g| g.data).collect();
    if found.is_empty() {
        return None;
    }
    // in their order, the later over the earlier
    found.sort_unstable();
    let mut pixmap = tiny_skia::Pixmap::new(TILE as u32, TILE as u32)?;
    let at = |p: &[f64; 2]| (((p[0] - ox) * px) as f32, ((p[1] - oy) * px) as f32);
    let paint = |c: Color32, alpha: f32| {
        let mut p = tiny_skia::Paint::default();
        p.set_color_rgba8(c.r(), c.g(), c.b(), (f32::from(c.a()) * alpha).round() as u8);
        p.anti_alias = true;
        p
    };
    let stroke = tiny_skia::Stroke { width: style.width, ..tiny_skia::Stroke::default() };
    let id = tiny_skia::Transform::identity();
    for k in found {
        let f = &layer.features[k as usize];
        let c = colours.get(k as usize).copied().unwrap_or(style.colour);
        let w = (f.bbox[2] - f.bbox[0]) * px;
        let h = (f.bbox[3] - f.bbox[1]) * px;
        // smaller than a pixel: a dot
        if f.kind != Kind::Point && w < 1.0 && h < 1.0 {
            let (x, y) = at(&[(f.bbox[0] + f.bbox[2]) / 2.0, (f.bbox[1] + f.bbox[3]) / 2.0]);
            if let Some(r) = tiny_skia::Rect::from_xywh(x - 0.5, y - 0.5, 1.0, 1.0) {
                pixmap.fill_rect(r, &paint(c, 1.0), id, None);
            }
            continue;
        }
        let mut pb = tiny_skia::PathBuilder::new();
        for part in f.first..f.last {
            let (a, b) = (layer.parts[part as usize] as usize, layer.parts[part as usize + 1] as usize);
            let pts = &layer.xy[a..b];
            if pts.is_empty() {
                continue;
            }
            if f.kind == Kind::Point {
                let (x, y) = at(&pts[0]);
                pb.push_circle(x, y, 2.0 + style.width);
                continue;
            }
            // the vertices a pixel apart at least
            let (x, y) = at(&pts[0]);
            pb.move_to(x, y);
            let mut last = (x, y);
            for (i, p) in pts.iter().enumerate().skip(1) {
                let (x, y) = at(p);
                if (x - last.0).abs() >= 0.5 || (y - last.1).abs() >= 0.5 || i == pts.len() - 1 {
                    pb.line_to(x, y);
                    last = (x, y);
                }
            }
            if f.kind == Kind::Polygon {
                pb.close();
            }
        }
        let Some(path) = pb.finish() else { continue };
        match f.kind {
            Kind::Point => {
                pixmap.fill_path(&path, &paint(c, 1.0), tiny_skia::FillRule::Winding, id, None);
                let ring = tiny_skia::Stroke { width: 1.0, ..tiny_skia::Stroke::default() };
                pixmap.stroke_path(&path, &paint(Color32::from_black_alpha(160), 1.0), &ring, id, None);
            }
            Kind::Line => pixmap.stroke_path(&path, &paint(c, 1.0), &stroke, id, None),
            Kind::Polygon if style.width <= 0.0 => {
                pixmap.fill_path(&path, &paint(c, style.fill), tiny_skia::FillRule::EvenOdd, id, None);
            }
            Kind::Polygon => {
                if style.fill > 0.0 {
                    pixmap.fill_path(&path, &paint(c, style.fill), tiny_skia::FillRule::EvenOdd, id, None);
                }
                pixmap.stroke_path(&path, &paint(c, 1.0), &stroke, id, None);
            }
        }
    }
    Some(egui::ColorImage::from_rgba_premultiplied([TILE, TILE], pixmap.data()))
}

/// The feature under a point (cells) within `tolerance` cells: the last drawn there.
pub fn pick(layer: &Layer, p: [f64; 2], tolerance: f64) -> Option<usize> {
    let env = AABB::from_corners([p[0] - tolerance, p[1] - tolerance], [p[0] + tolerance, p[1] + tolerance]);
    let mut found: Vec<u32> = layer.tree.locate_in_envelope_intersecting(&env).map(|g| g.data).collect();
    found.sort_unstable();
    let near_segment = |a: &[f64; 2], b: &[f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let (x, y) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
        x * x + y * y <= tolerance * tolerance
    };
    found.into_iter().rev().map(|k| k as usize).find(|&k| {
        let f = &layer.features[k];
        let parts = (f.first..f.last).map(|part| {
            let (a, b) = (layer.parts[part as usize] as usize, layer.parts[part as usize + 1] as usize);
            &layer.xy[a..b]
        });
        match f.kind {
            Kind::Point => parts.flatten().any(|q| (q[0] - p[0]).hypot(q[1] - p[1]) <= tolerance),
            Kind::Line => parts.into_iter().any(|l| l.windows(2).any(|w| near_segment(&w[0], &w[1]))),
            Kind::Polygon => {
                let mut inside = false;
                for ring in parts {
                    let n = ring.len();
                    for i in 0..n {
                        let (a, b) = (ring[i], ring[(i + 1) % n]);
                        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
                        {
                            inside = !inside;
                        }
                        if near_segment(&a, &b) {
                            return true;
                        }
                    }
                }
                inside
            }
        }
    })
}

struct Job {
    key: Key,
    layer: Arc<Layer>,
    colours: Arc<Vec<Color32>>,
    style: Style,
}

struct Made {
    key: Key,
    image: Option<egui::ColorImage>,
}

/// The threads that rasterize tiles: a queue replaced at every frame (the tiles in view, the nearest the centre
/// first), the tiles being made.
struct Workers {
    queue: Arc<(Mutex<VecDeque<Job>>, Condvar)>,
    started: Arc<Mutex<HashSet<Key>>>,
    done: Receiver<Made>,
}

impl Workers {
    fn new(ctx: egui::Context) -> Workers {
        let queue: Arc<(Mutex<VecDeque<Job>>, Condvar)> = Arc::new((Mutex::new(VecDeque::new()), Condvar::new()));
        let started: Arc<Mutex<HashSet<Key>>> = Arc::new(Mutex::new(HashSet::new()));
        let (tx, done): (Sender<Made>, _) = channel();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 6);
        for k in 0..threads {
            let (queue, started, tx, ctx) = (queue.clone(), started.clone(), tx.clone(), ctx.clone());
            let _ = std::thread::Builder::new().name(format!("terraflux-vector-tiles-{k}")).spawn(move || {
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
                    started.lock().unwrap().insert(job.key);
                    let (_, _, level, tx_, ty) = job.key;
                    let image = rasterize(&job.layer, &job.colours, &job.style, level, tx_, ty);
                    started.lock().unwrap().remove(&job.key);
                    if tx.send(Made { key: job.key, image }).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            });
        }
        Workers { queue, started, done }
    }
}

/// A layer being prepared, ready, or failed.
enum Prep {
    Pending(Receiver<Result<Layer, String>>),
    Ready(Arc<Layer>),
    Failed(String),
}

/// The vector layers of a view: prepared once per grid, their tiles made by threads and kept (the least used let
/// go), each style's colours.
#[derive(Default)]
pub struct VectorTiles {
    layers: HashMap<LayerKey, Prep>,
    colours: HashMap<(LayerKey, u64), Arc<Vec<Color32>>>,
    /// a tile's texture, or none (no feature over it); when last shown
    tiles: HashMap<Key, (Option<TextureHandle>, u64)>,
    workers: Option<Workers>,
    asked: HashSet<Key>,
    clock: u64,
}

/// A grid's key: its geotransform and coordinate system.
fn grid_key(g: &Grid) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    g.transform.iter().for_each(|v| v.to_bits().hash(&mut h));
    crs_of(g).hash(&mut h);
    h.finish()
}

/// A layer to draw: the features and their style.
#[derive(Clone)]
pub struct Overlay {
    pub vector: Arc<Vector>,
    pub style: Style,
}

/// How the view maps cells to the screen: the cell at the centre, screen points per cell, the rectangle.
#[derive(Clone, Copy)]
pub struct Camera {
    pub center: Vec2,
    pub zoom: f32,
    pub rect: Rect,
    pub pixels_per_point: f32,
}

impl VectorTiles {
    /// Whether layers are being prepared or tiles made.
    pub fn busy(&self) -> bool {
        !self.asked.is_empty() || self.layers.values().any(|p| matches!(p, Prep::Pending(_)))
    }

    /// A layer over a grid, prepared (none while it is, or when it failed: why).
    pub fn layer(&mut self, ctx: &egui::Context, v: &Arc<Vector>, grid: &Grid) -> Result<Option<Arc<Layer>>, String> {
        let key = (Arc::as_ptr(v) as usize, grid_key(grid));
        let prep = self.layers.entry(key).or_insert_with(|| {
            let (tx, rx) = channel();
            let (v, grid, ctx) = (v.clone(), grid.clone(), ctx.clone());
            let _ = std::thread::Builder::new().name("terraflux-vector-index".into()).spawn(move || {
                let _ = tx.send(prepare(&v, &grid));
                ctx.request_repaint();
            });
            Prep::Pending(rx)
        });
        if let Prep::Pending(rx) = prep {
            match rx.try_recv() {
                Ok(Ok(l)) => *prep = Prep::Ready(Arc::new(l)),
                Ok(Err(e)) => *prep = Prep::Failed(e),
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(_) => *prep = Prep::Failed("the layer could not be prepared".into()),
            }
        }
        match prep {
            Prep::Ready(l) => Ok(Some(l.clone())),
            Prep::Failed(e) => Err(e.clone()),
            Prep::Pending(_) => Ok(None),
        }
    }

    /// Draw the layers over the view, asking the threads for the tiles missing.
    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        cam: Camera,
        grid: &Grid,
        overlays: &[Overlay],
    ) -> Vec<String> {
        let workers = self.workers.get_or_insert_with(|| Workers::new(ctx.clone()));
        for m in workers.done.try_iter().collect::<Vec<_>>() {
            self.asked.remove(&m.key);
            self.clock += 1;
            let tex = m.image.map(|img| ctx.load_texture(format!("vtile-{}", self.clock), img, TextureOptions::LINEAR));
            self.tiles.insert(m.key, (tex, self.clock));
        }
        let mut errors = Vec::new();
        let ppp = cam.pixels_per_point;
        let cells_per_pixel = 1.0 / (f64::from(cam.zoom) * f64::from(ppp));
        let (a, b) = (
            cam.center + (cam.rect.min - cam.rect.center()) / cam.zoom,
            cam.center + (cam.rect.max - cam.rect.center()) / cam.zoom,
        );
        self.clock += 1;
        let clock = self.clock;
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let mut wanted: Vec<(f32, Job)> = Vec::new();
        for o in overlays {
            let layer = match self.layer(ctx, &o.vector, grid) {
                Ok(Some(l)) => l,
                Ok(None) => continue,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            let Some(bounds) = layer.bounds else { continue };
            let lkey = (Arc::as_ptr(&o.vector) as usize, grid_key(grid));
            let skey = o.style.key();
            let colours =
                self.colours.entry((lkey, skey)).or_insert_with(|| Arc::new(colours(&o.vector, &o.style))).clone();
            // the level: a tile's pixel about a pixel of the screen; the coarsest that holds the layer in a tile
            let mut top = FINEST;
            while span(top) < (bounds[2] - bounds[0]).max(bounds[3] - bounds[1]).max(1e-9) && top < 40 {
                top += 1;
            }
            let level = (cells_per_pixel.log2().floor() as i32).clamp(FINEST, top.max(FINEST));
            let screen = |l: i32, tx: i64, ty: i64| -> Rect {
                let s = span(l) as f32;
                let min = cam.rect.center() + (Vec2::new(tx as f32 * s, ty as f32 * s) - cam.center) * cam.zoom;
                Rect::from_min_size(min, Vec2::splat(s * cam.zoom))
            };
            let s = span(level);
            // the tiles in view over the layer's box
            let x0 = (f64::from(a.x).max(bounds[0] - 1.0) / s).floor() as i64;
            let y0 = (f64::from(a.y).max(bounds[1] - 1.0) / s).floor() as i64;
            let x1 = (f64::from(b.x).min(bounds[2] + 1.0) / s).floor() as i64;
            let y1 = (f64::from(b.y).min(bounds[3] + 1.0) / s).floor() as i64;
            if x1 < x0 || y1 < y0 || (x1 - x0 + 1) * (y1 - y0 + 1) > 1024 {
                continue;
            }
            // the top tiles first: something at once
            let ts = span(top);
            for ty in (bounds[1] / ts).floor() as i64..=(bounds[3] / ts).floor() as i64 {
                for tx in (bounds[0] / ts).floor() as i64..=(bounds[2] / ts).floor() as i64 {
                    let k = (lkey, skey, top, tx, ty);
                    if !self.tiles.contains_key(&k) {
                        wanted.push((
                            -1.0,
                            Job { key: k, layer: layer.clone(), colours: colours.clone(), style: o.style.clone() },
                        ));
                    }
                }
            }
            for ty in y0..=y1 {
                for tx in x0..=x1 {
                    let k = (lkey, skey, level, tx, ty);
                    let r = screen(level, tx, ty);
                    if let Some((tex, used)) = self.tiles.get_mut(&k) {
                        *used = clock;
                        if let Some(t) = tex {
                            painter.image(t.id(), r, full, Color32::WHITE);
                        }
                        continue;
                    }
                    wanted.push((
                        (r.center() - cam.rect.center()).length(),
                        Job { key: k, layer: layer.clone(), colours: colours.clone(), style: o.style.clone() },
                    ));
                    // meanwhile the finest coarser tile kept, stretched
                    for up in level + 1..=top.max(level + 1) {
                        let shift = (up - level) as u32;
                        let (ax, ay) = (tx >> shift, ty >> shift);
                        if let Some((tex, used)) = self.tiles.get_mut(&(lkey, skey, up, ax, ay)) {
                            *used = clock;
                            if let Some(t) = tex {
                                let f = 1.0 / (1u32 << shift) as f32;
                                let (u, v) = ((tx - (ax << shift)) as f32 * f, (ty - (ay << shift)) as f32 * f);
                                painter.image(
                                    t.id(),
                                    r,
                                    Rect::from_min_size(Pos2::new(u, v), Vec2::splat(f)),
                                    Color32::WHITE,
                                );
                            }
                            break;
                        }
                    }
                }
            }
        }
        wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
        let workers = self.workers.as_ref().unwrap();
        let started = workers.started.lock().unwrap().clone();
        let (lock, cv) = &*workers.queue;
        let mut q = lock.lock().unwrap();
        q.clear();
        self.asked.retain(|k| started.contains(k));
        for (_, j) in wanted {
            if !started.contains(&j.key) && self.asked.insert(j.key) {
                q.push_back(j);
            } else if started.contains(&j.key) {
                self.asked.insert(j.key);
            }
        }
        cv.notify_all();
        drop(q);
        if self.tiles.len() > KEPT {
            let mut by_age: Vec<(u64, Key)> = self.tiles.iter().map(|(k, (_, u))| (*u, *k)).collect();
            by_age.sort_unstable_by_key(|(u, _)| *u);
            for (_, k) in by_age.into_iter().take(self.tiles.len() - KEPT) {
                self.tiles.remove(&k);
            }
        }
        errors
    }

    /// The feature under a cell position, of the topmost layer that has one there: (layer, feature).
    pub fn pick(
        &mut self,
        ctx: &egui::Context,
        overlays: &[Overlay],
        grid: &Grid,
        cell: [f64; 2],
        tolerance: f64,
    ) -> Option<(usize, usize)> {
        for (i, o) in overlays.iter().enumerate().rev() {
            if let Ok(Some(l)) = self.layer(ctx, &o.vector, grid)
                && let Some(k) = pick(&l, cell, tolerance)
            {
                return Some((i, k));
            }
        }
        None
    }
}
