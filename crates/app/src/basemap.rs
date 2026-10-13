//! Basemaps under the maps: the web map tiles of a service (Esri, OpenStreetMap...) fetched by a pool of threads
//! shared by every view (many at once, each tile once: kept on disk and, decoded, in memory), and drawn by the GPU
//! as textures on meshes whose vertices are the tiles' corners and edges carried to the map's coordinate system
//! (a 9 x 9 grid per tile, transformed once). Nothing is warped on the processor: a tile shows as soon as it
//! comes, the coarser tiles already there under those still to come.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use egui::{Color32, ColorImage, Mesh, Pos2, Rect, Shape, TextureHandle, TextureOptions, Vec2};

use crate::map_view::Basemap;

/// Half the side of the web Mercator world, metres.
const E: f64 = 20037508.342789244;
/// The side of a web tile, pixels.
const SIDE: f64 = 256.0;
/// The decoded tiles kept in memory (256 KB each).
const DECODED: usize = 600;
/// The threads fetching tiles.
const THREADS: usize = 12;
/// A tile's vertices along each side.
const STEPS: usize = 8;

/// A web tile: the service, its zoom level and place.
pub type WebKey = (Basemap, u8, u32, u32);

/// The pool of threads fetching tiles, the tiles each view wants (in order), those decoded.
struct Pool {
    state: Mutex<State>,
    wake: Condvar,
}

#[derive(Default)]
struct State {
    /// The tiles each view wants, first the most wanted.
    wanted: HashMap<u64, VecDeque<WebKey>>,
    /// The view served last (the views are served in turn).
    turn: u64,
    fetching: HashSet<WebKey>,
    /// The tiles decoded (none: the service has none there), and when each was last asked for.
    done: HashMap<WebKey, (Option<Arc<ColorImage>>, u64)>,
    clock: u64,
    contexts: Vec<egui::Context>,
    error: Option<String>,
}

fn pool() -> &'static Pool {
    static POOL: OnceLock<Pool> = OnceLock::new();
    POOL.get_or_init(|| {
        for k in 0..THREADS {
            let _ = std::thread::Builder::new().name(format!("terraflux-web-tiles-{k}")).spawn(work);
        }
        Pool { state: Mutex::new(State::default()), wake: Condvar::new() }
    })
}

/// A thread of the pool: the next tile wanted by the next view, fetched, decoded, kept.
fn work() {
    let p = pool();
    loop {
        let key = {
            let mut s = p.state.lock().unwrap();
            loop {
                if let Some(k) = next(&mut s) {
                    s.fetching.insert(k);
                    break k;
                }
                s = p.wake.wait(s).unwrap();
            }
        };
        let got = load(key);
        let mut s = p.state.lock().unwrap();
        s.fetching.remove(&key);
        s.clock += 1;
        let clock = s.clock;
        match got {
            Ok(img) => {
                s.done.insert(key, (img.map(Arc::new), clock));
                s.error = None;
            }
            Err(e) => s.error = Some(e),
        }
        if s.done.len() > DECODED {
            let mut by_age: Vec<(u64, WebKey)> = s.done.iter().map(|(k, (_, u))| (*u, *k)).collect();
            by_age.sort_unstable_by_key(|(u, _)| *u);
            for (_, k) in by_age.into_iter().take(s.done.len() - DECODED) {
                s.done.remove(&k);
            }
        }
        for c in &s.contexts {
            c.request_repaint();
        }
    }
}

/// The next tile to fetch: from the views in turn, the first each wants that is neither done nor being fetched.
fn next(s: &mut State) -> Option<WebKey> {
    let mut views: Vec<u64> = s.wanted.keys().copied().collect();
    views.sort_unstable();
    let start = views.iter().position(|&v| v > s.turn).unwrap_or(0);
    for i in 0..views.len() {
        let v = views[(start + i) % views.len()];
        let q = s.wanted.get_mut(&v).unwrap();
        while let Some(k) = q.pop_front() {
            if !s.done.contains_key(&k) && !s.fetching.contains(&k) {
                s.turn = v;
                return Some(k);
            }
        }
    }
    None
}

/// Where a service's tiles are kept on disk (on macOS `~/Library/Caches`).
fn cache_dir(b: Basemap) -> PathBuf {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            let home = std::env::var_os("HOME").filter(|_| cfg!(target_os = "macos"))?;
            Some(PathBuf::from(home).join("Library").join("Caches"))
        })
        .unwrap_or_else(std::env::temp_dir);
    root.join("Terraflux").join("web-tiles").join(format!("{b:?}"))
}

/// A tile from the disk, else from the web (then kept on disk), decoded; none where the service has none.
fn load((b, z, x, y): WebKey) -> Result<Option<ColorImage>, String> {
    let Some((url, _)) = b.url() else { return Ok(None) };
    let file = cache_dir(b).join(z.to_string()).join(x.to_string()).join(y.to_string());
    let bytes = match std::fs::read(&file) {
        Ok(bytes) => bytes,
        Err(_) => {
            let url =
                url.replace("${z}", &z.to_string()).replace("${x}", &x.to_string()).replace("${y}", &y.to_string());
            let agent = format!("USERAGENT=Terraflux/{}", env!("CARGO_PKG_VERSION"));
            let (status, bytes) = terraflux_gdal::fetch(&url, &[&agent, "TIMEOUT=20", "MAX_RETRY=2", "RETRY_DELAY=1"])?;
            if status == 404 || status == 204 || bytes.is_empty() {
                return Ok(None);
            }
            if status != 200 && status != 0 {
                return Err(format!("{url}: HTTP {status}"));
            }
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
                let _ = std::fs::write(&file, &bytes);
            }
            bytes
        }
    };
    let (w, h, px) = match terraflux_gdal::decode_image(&bytes) {
        Ok(d) => d,
        // a broken file kept: fetched again next time
        Err(e) => {
            let _ = std::fs::remove_file(&file);
            return Err(e);
        }
    };
    let pixels = px.into_iter().map(|[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a)).collect();
    Ok(Some(ColorImage::new([w, h], pixels)))
}

/// A grid's place on the screen: its transform, the cell at the view's centre and points per cell.
pub struct View {
    pub transform: [f64; 6],
    pub rect: Rect,
    pub center: Vec2,
    pub zoom: f32,
    pub pixels_per_point: f32,
}

impl View {
    /// The screen point of map coordinates.
    fn screen(&self, x: f64, y: f64) -> Pos2 {
        let t = self.transform;
        let (cx, cy) = ((x - t[0]) / t[1], (y - t[3]) / t[5]);
        self.rect.center() + (Vec2::new(cx as f32, cy as f32) - self.center) * self.zoom
    }

    /// The map coordinates of a screen point.
    fn map(&self, p: Pos2) -> (f64, f64) {
        let c = self.center + (p - self.rect.center()) / self.zoom;
        let t = self.transform;
        (t[0] + c.x as f64 * t[1], t[3] + c.y as f64 * t[5])
    }
}

/// A view's basemap: its coordinate system's transformations to and from web Mercator, the tiles' vertices in
/// map coordinates and their textures.
#[derive(Default)]
pub struct Layer {
    id: u64,
    crs: Option<String>,
    to_web: Option<terraflux_gdal::Transform>,
    from_web: Option<terraflux_gdal::Transform>,
    /// Each tile's (STEPS + 1)^2 vertices in map coordinates.
    vertices: HashMap<WebKey, Arc<Vec<[f64; 2]>>>,
    textures: HashMap<WebKey, (TextureHandle, u64)>,
    clock: u64,
    pub error: Option<String>,
    /// Tiles wanted and not come yet.
    missing: usize,
}

impl Layer {
    /// The tiles shown and kept; why there is none, if so.
    pub fn tiles(&self) -> (usize, Option<String>) {
        (self.textures.len(), self.error.clone().or_else(|| pool().state.lock().unwrap().error.clone()))
    }

    /// Whether tiles in view are still to come.
    pub fn busy(&self) -> bool {
        self.missing > 0
    }

    /// Draw the basemap `b` under a view of a grid of the coordinate system `crs`.
    pub fn draw(&mut self, ctx: &egui::Context, painter: &egui::Painter, b: Basemap, crs: &str, v: &View) {
        let Some((_, levels)) = b.url() else { return };
        if self.id == 0 {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            self.id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        if self.crs.as_deref() != Some(crs) {
            self.crs = Some(crs.to_string());
            self.vertices.clear();
            self.to_web = None;
            self.from_web = None;
            match (terraflux_gdal::Transform::new(crs, "EPSG:3857"), terraflux_gdal::Transform::new("EPSG:3857", crs)) {
                (Ok(a), Ok(b)) => {
                    self.to_web = Some(a);
                    self.from_web = Some(b);
                    self.error = None;
                }
                (Err(e), _) | (_, Err(e)) => self.error = Some(e),
            }
        }
        let (Some(to_web), Some(_)) = (&self.to_web, &self.from_web) else { return };
        // the view in web Mercator: a grid of its points carried there
        let r = v.rect;
        let n = 8;
        let (mut xs, mut ys) = (Vec::new(), Vec::new());
        for i in 0..=n {
            for j in 0..=n {
                let p =
                    Pos2::new(r.min.x + r.width() * j as f32 / n as f32, r.min.y + r.height() * i as f32 / n as f32);
                let (x, y) = v.map(p);
                xs.push(x);
                ys.push(y);
            }
        }
        to_web.points(&mut xs, &mut ys);
        let ok: Vec<(f64, f64)> = xs.iter().zip(&ys).filter(|(x, _)| x.is_finite()).map(|(x, y)| (*x, *y)).collect();
        if ok.is_empty() {
            return;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for &(x, y) in &ok {
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        let (x0, x1, y0, y1) = (x0.max(-E), x1.min(E), y0.max(-E), y1.min(E));
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        // the level whose pixels are about the screen's: metres per physical pixel along the view's middle row
        let mid = (n / 2) * (n + 1);
        let across = (0..n).filter_map(|j| {
            let (a, b) = ((xs[mid + j], ys[mid + j]), (xs[mid + j + 1], ys[mid + j + 1]));
            (a.0.is_finite() && b.0.is_finite()).then(|| (b.0 - a.0).hypot(b.1 - a.1))
        });
        let (sum, count) = across.fold((0.0, 0), |(s, c), d| (s + d, c + 1));
        let pixels = (r.width() * v.pixels_per_point) as f64 / n as f64;
        let mpp = if count > 0 { sum / count as f64 / pixels } else { (x1 - x0) / (r.width() as f64) };
        let mut z = ((2.0 * E / (SIDE * mpp)).log2() + 0.2).ceil().clamp(0.0, levels as f64) as u8;
        let range = |z: u8| {
            let size = 2.0 * E / (1u64 << z) as f64;
            let last = (1u64 << z) as f64 - 1.0;
            let tx = |m: f64| ((m + E) / size).floor().clamp(0.0, last) as u32;
            let ty = |m: f64| ((E - m) / size).floor().clamp(0.0, last) as u32;
            (tx(x0), tx(x1), ty(y1), ty(y0))
        };
        while z > 0 {
            let (a, b, c, d) = range(z);
            if (b - a + 1) as u64 * (d - c + 1) as u64 <= 300 {
                break;
            }
            z -= 1;
        }
        // the tiles wanted: two levels coarser first (few, they fill the view soon), then the level's, from the
        // centre out
        let centre = {
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            move |z: u8, x: u32, y: u32| {
                let size = 2.0 * E / (1u64 << z) as f64;
                let (mx, my) = (-E + (x as f64 + 0.5) * size, E - (y as f64 + 0.5) * size);
                (mx - cx).hypot(my - cy)
            }
        };
        let mut wanted: Vec<WebKey> = Vec::new();
        for level in [z.saturating_sub(2), z] {
            let (a, bx, c, d) = range(level);
            let mut tiles: Vec<WebKey> = (c..=d).flat_map(|y| (a..=bx).map(move |x| (b, level, x, y))).collect();
            tiles.sort_by(|p, q| centre(p.1, p.2, p.3).total_cmp(&centre(q.1, q.2, q.3)));
            wanted.extend(tiles);
            if level == z {
                break;
            }
        }
        wanted.dedup();
        // what has come, and the view's wishes
        let p = pool();
        let mut ready: HashMap<WebKey, Option<Arc<ColorImage>>> = HashMap::new();
        {
            let mut s = p.state.lock().unwrap();
            if !s.contexts.iter().any(|c| c == ctx) {
                s.contexts.push(ctx.clone());
            }
            s.clock += 1;
            let clock = s.clock;
            let mut queue = VecDeque::new();
            // the coarser levels kept, drawn under
            for level in z.saturating_sub(5)..=z {
                let (a, bx, c, d) = range(level);
                for y in c..=d {
                    for x in a..=bx {
                        let k = (b, level, x, y);
                        if let Some((img, used)) = s.done.get_mut(&k) {
                            *used = clock;
                            ready.insert(k, img.clone());
                        }
                    }
                }
            }
            self.missing = 0;
            for k in &wanted {
                if !s.done.contains_key(k) {
                    self.missing += 1;
                    queue.push_back(*k);
                }
            }
            s.wanted.insert(self.id, queue);
        }
        p.wake.notify_all();
        // drawn coarse to fine
        self.clock += 1;
        let clock = self.clock;
        let mut keys: Vec<WebKey> = ready.iter().filter(|(_, i)| i.is_some()).map(|(k, _)| *k).collect();
        keys.sort_by_key(|k| (k.1, k.2, k.3));
        for k in keys {
            let tex = match self.textures.get_mut(&k) {
                Some((t, used)) => {
                    *used = clock;
                    t.clone()
                }
                None => {
                    let img = ready[&k].as_ref().unwrap();
                    let t = ctx.load_texture(format!("web-{k:?}"), (**img).clone(), TextureOptions::LINEAR);
                    self.textures.insert(k, (t.clone(), clock));
                    t
                }
            };
            let vertices = self.vertices_of(k);
            painter.add(Shape::mesh(mesh(&vertices, tex.id(), v)));
        }
        if self.textures.len() > DECODED {
            let mut by_age: Vec<(u64, WebKey)> = self.textures.iter().map(|(k, (_, u))| (*u, *k)).collect();
            by_age.sort_unstable_by_key(|(u, _)| *u);
            for (_, k) in by_age.into_iter().take(self.textures.len() - DECODED) {
                self.textures.remove(&k);
            }
        }
        if self.vertices.len() > 4 * DECODED {
            self.vertices.clear();
        }
    }

    /// A tile's vertices in map coordinates (NaN where the map's system has no place for them).
    fn vertices_of(&mut self, k: WebKey) -> Arc<Vec<[f64; 2]>> {
        if let Some(v) = self.vertices.get(&k) {
            return v.clone();
        }
        let (_, z, x, y) = k;
        let size = 2.0 * E / (1u64 << z) as f64;
        let (mut xs, mut ys) = (Vec::new(), Vec::new());
        for i in 0..=STEPS {
            for j in 0..=STEPS {
                xs.push(-E + (x as f64 + j as f64 / STEPS as f64) * size);
                ys.push(E - (y as f64 + i as f64 / STEPS as f64) * size);
            }
        }
        if let Some(t) = &self.from_web {
            t.points(&mut xs, &mut ys);
        }
        let v = Arc::new(xs.into_iter().zip(ys).map(|(x, y)| [x, y]).collect::<Vec<_>>());
        self.vertices.insert(k, v.clone());
        v
    }
}

/// A tile's mesh on the screen: its quads whose four corners have a place.
fn mesh(vertices: &[[f64; 2]], texture: egui::TextureId, v: &View) -> Mesh {
    let mut m = Mesh::with_texture(texture);
    let side = STEPS + 1;
    let valid: Vec<bool> = vertices.iter().map(|p| p[0].is_finite() && p[1].is_finite()).collect();
    for (k, p) in vertices.iter().enumerate() {
        let (i, j) = (k / side, k % side);
        let pos = if valid[k] { v.screen(p[0], p[1]) } else { Pos2::ZERO };
        m.vertices.push(egui::epaint::Vertex {
            pos,
            uv: Pos2::new(j as f32 / STEPS as f32, i as f32 / STEPS as f32),
            color: Color32::WHITE,
        });
    }
    for i in 0..STEPS {
        for j in 0..STEPS {
            let a = i * side + j;
            let (b, c, d) = (a + 1, a + side, a + side + 1);
            if [a, b, c, d].iter().all(|&q| valid[q]) {
                m.indices.extend([a as u32, b as u32, d as u32, a as u32, d as u32, c as u32]);
            }
        }
    }
    m
}

/// The side of a grid's cell in web Mercator metres at its centre (how many cells the world spans), when its
/// coordinate system can be carried there; the transformations kept by the thread.
pub fn cell_metres(crs: &str, transform: [f64; 6], width: usize, height: usize) -> Option<f64> {
    thread_local! {
        static TO_WEB: std::cell::RefCell<HashMap<String, Option<terraflux_gdal::Transform>>> =
            std::cell::RefCell::new(HashMap::new());
    }
    TO_WEB.with(|cache| {
        let mut cache = cache.borrow_mut();
        let t = cache.entry(crs.to_string()).or_insert_with(|| terraflux_gdal::Transform::new(crs, "EPSG:3857").ok());
        let t = t.as_ref()?;
        let (cx, cy) = (width as f64 / 2.0, height as f64 / 2.0);
        let at = |c: f64, r: f64| (transform[0] + c * transform[1], transform[3] + r * transform[5]);
        let (a, b) = (at(cx, cy), at(cx + 1.0, cy));
        let (mut xs, mut ys) = (vec![a.0, b.0], vec![a.1, b.1]);
        t.points(&mut xs, &mut ys);
        let d = (xs[1] - xs[0]).hypot(ys[1] - ys[0]);
        (d.is_finite() && d > 0.0).then_some(d)
    })
}

/// The cells the world spans across in web Mercator, for a grid's cell of `metres`.
pub fn world_cells(metres: f64) -> f64 {
    2.0 * E / metres
}
