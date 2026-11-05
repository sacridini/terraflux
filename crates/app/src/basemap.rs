//! Basemaps under the maps: the web map tiles of a service (Esri, OpenStreetMap...) fetched by a pool of threads
//! shared by every view (many at once, each tile once: kept on disk and, decoded, in memory), and drawn by the GPU
//! as textures on meshes whose vertices are the tiles' corners and edges carried to the map's coordinate system
//! (a 9 x 9 grid per tile, transformed once). Nothing is warped on the processor: a tile shows as soon as it
//! comes, the coarser tiles already there under those still to come.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};

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

/// The most basemaps of the user's own kept in the settings.
pub const MOST_CUSTOM: usize = 5;

/// A tile's address: the template's `{z}`, `{x}` and `{y}` (also written `${z}`...) replaced by its level and place.
pub fn tile_url(template: &str, z: u8, x: u32, y: u32) -> String {
    template
        .replace("${z}", &z.to_string())
        .replace("${x}", &x.to_string())
        .replace("${y}", &y.to_string())
        .replace("{z}", &z.to_string())
        .replace("{x}", &x.to_string())
        .replace("{y}", &y.to_string())
}

/// A basemap of the user's own: web tiles by an address template.
#[derive(Debug, Clone, PartialEq)]
pub struct Custom {
    /// The address, with `{z}`, `{x}` and `{y}` (checked by [`validate`]).
    pub template: String,
    pub attribution: String,
    /// The deepest zoom level the service has.
    pub max_zoom: u32,
}

impl Custom {
    /// What names it in the basemap chooser: its host.
    pub fn name(&self) -> String {
        format!("Custom: {}", host_of(&self.template))
    }

    /// What tells it apart (FNV-1a of its template: stable, it names the folder of its tiles on disk).
    pub fn id(&self) -> u64 {
        self.template.bytes().fold(0xcbf29ce484222325u64, |h, c| (h ^ c as u64).wrapping_mul(0x100000001b3))
    }
}

/// The host of an address (`tiles.example.org` of `https://www.tiles.example.org:8080/{z}/{x}/{y}.png`).
pub fn host_of(address: &str) -> String {
    let rest = address.split_once("://").map_or(address, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = host.rsplit_once(':').filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit())).map_or(host, |(h, _)| h);
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

/// A template of tiles checked and made canonical: `http://` or `https://`, with `{z}`, `{x}` and `{y}` (`${z}`
/// too, as the services' documents write it). `{s}` (the subdomains of a service: a, b, c) is taken as `a`; any
/// other `{...}` is refused, not guessed.
pub fn validate(template: &str) -> Result<String, String> {
    let t = template.trim();
    let lower = t.to_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("the address starts with http:// or https://".into());
    }
    if t.chars().any(char::is_whitespace) {
        return Err("the address has a space in it".into());
    }
    let t = t.replace("${", "{").replace("{s}", "a");
    for (k, what) in [("{z}", "the zoom level"), ("{x}", "the column"), ("{y}", "the row")] {
        if !t.contains(k) {
            return Err(format!("the address has no {k} for {what}"));
        }
    }
    let rest = t.replace("{z}", "").replace("{x}", "").replace("{y}", "");
    if let Some(i) = rest.find('{') {
        let end = rest[i..].find('}').map_or(rest.len(), |e| i + e + 1);
        return Err(format!("{} is not known: only {{z}}, {{x}} and {{y}}", &rest[i..end]));
    }
    if rest.contains('}') {
        return Err("a } with no { before it".into());
    }
    if host_of(&t).is_empty() {
        return Err("the address has no host".into());
    }
    Ok(t)
}

fn registry() -> &'static RwLock<HashMap<u64, Custom>> {
    static R: OnceLock<RwLock<HashMap<u64, Custom>>> = OnceLock::new();
    R.get_or_init(|| RwLock::new(HashMap::new()))
}

/// The basemap of the user's with this number, when it is known (made by [`register`] or [`customs`]).
pub fn custom(id: u64) -> Option<Custom> {
    registry().read().unwrap().get(&id).cloned()
}

/// Make a basemap of the user's own: its template checked, the deepest zoom kept within 0 and 24; kept in the
/// settings as the first of at most [`MOST_CUSTOM`] (the same template again: moved first, its attribution and zoom
/// renewed).
pub fn register(template: &str, attribution: &str, max_zoom: u32) -> Result<Basemap, String> {
    let c = Custom {
        template: validate(template)?,
        attribution: attribution.trim().to_string(),
        max_zoom: max_zoom.min(24),
    };
    let id = c.id();
    registry().write().unwrap().insert(id, c.clone());
    crate::settings::update(|s| {
        s.custom_basemaps.retain(|k| k.template != c.template);
        s.custom_basemaps.insert(
            0,
            crate::settings::CustomBasemap {
                template: c.template.clone(),
                attribution: c.attribution.clone(),
                max_zoom: c.max_zoom,
            },
        );
        s.custom_basemaps.truncate(MOST_CUSTOM);
    });
    Ok(Basemap::Custom(id))
}

/// A basemap of the user's own for a test: known, but not kept in the settings.
#[cfg(test)]
pub(crate) fn known_for_test(template: &str, max_zoom: u32) -> Basemap {
    let c = Custom { template: validate(template).unwrap(), attribution: String::new(), max_zoom };
    let id = c.id();
    registry().write().unwrap().insert(id, c);
    Basemap::Custom(id)
}

/// The basemaps of the user's own in the settings (the last used first), known from now on.
pub fn customs() -> Vec<Basemap> {
    let kept = crate::settings::get().custom_basemaps;
    let mut r = registry().write().unwrap();
    kept.into_iter()
        .filter_map(|k| {
            let c = Custom {
                template: validate(&k.template).ok()?,
                attribution: k.attribution,
                max_zoom: k.max_zoom.min(24),
            };
            let id = c.id();
            r.insert(id, c);
            Some(Basemap::Custom(id))
        })
        .collect()
}

/// The form of a basemap of the user's own, as it is typed.
#[derive(Clone)]
struct Form {
    template: String,
    attribution: String,
    zoom: u32,
    error: Option<String>,
}

/// The choices of a basemap chooser (inside its combo box): none, the services, the user's own, and *Custom...*
/// that opens the window of [`custom_window`] (kept under `id`).
pub fn choices(ui: &mut egui::Ui, id: egui::Id, current: &mut Basemap) {
    for b in Basemap::ALL {
        ui.selectable_value(current, b, b.name());
    }
    let own = customs();
    if !own.is_empty() {
        ui.separator();
        for b in own {
            ui.selectable_value(current, b, b.name()).on_hover_text(b.url().map_or(String::new(), |u| u.0));
        }
    }
    ui.separator();
    if ui
        .button("Custom...")
        .on_hover_text("tiles by an address with {z}/{x}/{y} in it (the standard XYZ tiles of OpenStreetMap, QGIS...)")
        .clicked()
    {
        let form = Form { template: String::new(), attribution: String::new(), zoom: 19, error: None };
        ui.data_mut(|d| d.insert_temp(id.with("custom basemap"), form));
        ui.close();
    }
}

/// The window that makes a basemap of the user's own, open after *Custom...*: its address, its attribution (written
/// on the map), its deepest zoom; *Use* checks the address and makes it the basemap.
pub fn custom_window(ctx: &egui::Context, id: egui::Id, current: &mut Basemap) {
    let key = id.with("custom basemap");
    let Some(mut f) = ctx.data(|d| d.get_temp::<Form>(key)) else { return };
    let mut open = true;
    let mut done = false;
    egui::Window::new("Custom basemap").id(key).open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        ui.label("Tiles by an address, with {z} (zoom), {x} (column) and {y} (row) in it.");
        ui.add(
            egui::TextEdit::singleline(&mut f.template)
                .hint_text("https://tiles.example.org/{z}/{x}/{y}.png")
                .desired_width(380.0),
        );
        ui.horizontal(|ui| {
            ui.label("Attribution");
            ui.add(egui::TextEdit::singleline(&mut f.attribution).hint_text("whose map it is").desired_width(260.0));
        });
        ui.horizontal(|ui| {
            ui.label("Deepest zoom");
            ui.add(egui::DragValue::new(&mut f.zoom).range(0..=24));
        });
        if let Some(e) = &f.error {
            ui.label(egui::RichText::new(e.as_str()).color(ui.visuals().error_fg_color));
        }
        ui.horizontal(|ui| {
            if ui.button("Use").clicked() {
                match register(&f.template, &f.attribution, f.zoom) {
                    Ok(b) => {
                        *current = b;
                        done = true;
                    }
                    Err(e) => f.error = Some(e),
                }
            }
            if ui.button("Cancel").clicked() {
                done = true;
            }
        });
    });
    ctx.data_mut(|d| {
        if open && !done {
            d.insert_temp(key, f);
        } else {
            d.remove::<Form>(key);
        }
    });
}

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
pub fn cache_dir(b: Basemap) -> PathBuf {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            let home = std::env::var_os("HOME").filter(|_| cfg!(target_os = "macos"))?;
            Some(PathBuf::from(home).join("Library").join("Caches"))
        })
        .unwrap_or_else(std::env::temp_dir);
    // (a service of the user's: its folder named by a hash of its template, so another template is another cache)
    let name = match b {
        Basemap::Custom(id) => format!("custom-{id:016x}"),
        b => format!("{b:?}"),
    };
    root.join("Terraflux").join("web-tiles").join(name)
}

/// A tile from the disk, else from the web (then kept on disk), decoded; none where the service has none.
fn load((b, z, x, y): WebKey) -> Result<Option<ColorImage>, String> {
    let Some((url, _)) = b.url() else { return Ok(None) };
    let file = cache_dir(b).join(z.to_string()).join(x.to_string()).join(y.to_string());
    let bytes = match std::fs::read(&file) {
        Ok(bytes) => bytes,
        Err(_) => {
            let url = tile_url(&url, z, x, y);
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

/// The threads fetching the tiles of a picture of a basemap.
const PICTURE_THREADS: usize = 6;
/// The most tiles a picture of a basemap is made of (a deeper level is left for a coarser one beyond this).
const PICTURE_TILES: u64 = 400;
/// The pixels between the points of a picture carried to web Mercator (between them the places are interpolated).
const LATTICE: usize = 8;

/// A basemap as pixels: premultiplied RGBA, transparent where the service has nothing, and how many tiles were
/// wanted and not had.
pub struct Picture {
    pub rgba: Vec<u8>,
    pub missing: usize,
}

/// The basemap `b` as a picture of `w` x `h` pixels over the extent of `grid`, as the live view shows it under the
/// map: each pixel's place carried to web Mercator (exactly at a lattice of points, between them interpolated: the
/// live view does the same with its tiles' vertices) and the colour of the tile there, at the level whose pixels
/// are about the picture's. The tiles are read from the disk cache, else fetched (and kept), by a few threads.
pub fn picture(b: Basemap, grid: &terraflux_raster::Grid, w: usize, h: usize) -> Result<Picture, String> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let Some((_, levels)) = b.url() else { return Err("no basemap is chosen".into()) };
    let crs = crate::map_view::crs_of(grid).ok_or("the map has no CRS: no basemap")?;
    let to_web = terraflux_gdal::Transform::new(&crs, "EPSG:3857")?;
    let (w, h) = (w.max(1), h.max(1));
    // the lattice of the picture's points (in pixel edges), carried to web Mercator
    let (nx, ny) = (w.div_ceil(LATTICE) + 1, h.div_ceil(LATTICE) + 1);
    let t = grid.transform;
    let (fx, fy) = (grid.width as f64 / w as f64, grid.height as f64 / h as f64);
    let (mut xs, mut ys) = (Vec::with_capacity(nx * ny), Vec::with_capacity(nx * ny));
    for k in 0..ny {
        for i in 0..nx {
            let (cx, cy) = ((i * LATTICE) as f64 * fx, (k * LATTICE) as f64 * fy);
            xs.push(t[0] + cx * t[1] + cy * t[2]);
            ys.push(t[3] + cx * t[4] + cy * t[5]);
        }
    }
    to_web.points(&mut xs, &mut ys);
    let at = |i: usize, k: usize| (xs[k * nx + i], ys[k * nx + i]);
    let finite = |p: (f64, f64)| p.0.is_finite() && p.1.is_finite();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    let (mut across, mut pairs) = (0.0, 0usize);
    for k in 0..ny {
        for i in 0..nx {
            let p = at(i, k);
            if !finite(p) {
                continue;
            }
            (x0, x1, y0, y1) = (x0.min(p.0), x1.max(p.0), y0.min(p.1), y1.max(p.1));
            if i + 1 < nx && finite(at(i + 1, k)) {
                let q = at(i + 1, k);
                across += (q.0 - p.0).hypot(q.1 - p.1);
                pairs += 1;
            }
        }
    }
    let (x0, x1, y0, y1) = (x0.max(-E), x1.min(E), y0.max(-E), y1.min(E));
    if !(x1 > x0 && y1 > y0) {
        return Err("the map is outside the world of the basemap".into());
    }
    // the level whose pixels are about the picture's, as the live view picks it
    let mpp = if pairs > 0 { across / pairs as f64 / LATTICE as f64 } else { (x1 - x0) / w as f64 };
    let mut z = ((2.0 * E / (SIDE * mpp)).log2() + 0.2).ceil().clamp(0.0, levels as f64) as u8;
    let range = |z: u8| {
        let size = 2.0 * E / (1u64 << z) as f64;
        let last = (1u64 << z) as f64 - 1.0;
        let tx = |m: f64| ((m + E) / size).floor().clamp(0.0, last) as u32;
        let ty = |m: f64| ((E - m) / size).floor().clamp(0.0, last) as u32;
        (tx(x0), tx(x1), ty(y1), ty(y0))
    };
    while z > 0 {
        let (a, b2, c, d) = range(z);
        if u64::from(b2 - a + 1) * u64::from(d - c + 1) <= PICTURE_TILES {
            break;
        }
        z -= 1;
    }
    let (a, b2, c, d) = range(z);
    let keys: Vec<WebKey> = (c..=d).flat_map(|y| (a..=b2).map(move |x| (b, z, x, y))).collect();
    // the tiles: from the disk, else the web, a few at a time
    let (next, tiles, error) = (AtomicUsize::new(0), Mutex::new(HashMap::new()), Mutex::new(None::<String>));
    std::thread::scope(|s| {
        for _ in 0..PICTURE_THREADS.min(keys.len()) {
            s.spawn(|| {
                while let Some(&k) = keys.get(next.fetch_add(1, Ordering::Relaxed)) {
                    match load(k) {
                        Ok(img) => {
                            tiles.lock().unwrap().insert(k, img);
                        }
                        Err(e) => {
                            error.lock().unwrap().get_or_insert(e);
                        }
                    }
                }
            });
        }
    });
    let tiles = tiles.into_inner().unwrap();
    if tiles.values().all(Option::is_none)
        && let Some(e) = error.into_inner().unwrap()
    {
        return Err(e);
    }
    let missing = keys.len() - tiles.values().filter(|t| t.is_some()).count();
    // each pixel's place in web Mercator by the lattice, then its colour in the tile there
    let size = 2.0 * E / (1u64 << z) as f64;
    let last = (1u64 << z) as i64 - 1;
    let mut rgba = vec![0u8; w * h * 4];
    for j in 0..h {
        let v = (j as f64 + 0.5) / LATTICE as f64;
        let (k0, ty) = ((v as usize).min(ny - 2), v - (v as usize).min(ny - 2) as f64);
        for i in 0..w {
            let u = (i as f64 + 0.5) / LATTICE as f64;
            let (i0, tx) = ((u as usize).min(nx - 2), u - (u as usize).min(nx - 2) as f64);
            let (p00, p10, p01, p11) = (at(i0, k0), at(i0 + 1, k0), at(i0, k0 + 1), at(i0 + 1, k0 + 1));
            if ![p00, p10, p01, p11].into_iter().all(finite) {
                continue;
            }
            let mix = |f: fn(&(f64, f64)) -> f64| {
                (f(&p00) * (1.0 - tx) + f(&p10) * tx) * (1.0 - ty) + (f(&p01) * (1.0 - tx) + f(&p11) * tx) * ty
            };
            let (mx, my) = (mix(|p| p.0), mix(|p| p.1));
            let (fx, fy) = ((mx + E) / size, (E - my) / size);
            let (tile_x, tile_y) = (fx.floor() as i64, fy.floor() as i64);
            if !(0..=last).contains(&tile_x) || !(0..=last).contains(&tile_y) {
                continue;
            }
            let Some(Some(img)) = tiles.get(&(b, z, tile_x as u32, tile_y as u32)) else { continue };
            let [tw, th] = img.size;
            let px = (((fx - tile_x as f64) * tw as f64) as usize).min(tw - 1);
            let py = (((fy - tile_y as f64) * th as f64) as usize).min(th - 1);
            let o = (j * w + i) * 4;
            rgba[o..o + 4].copy_from_slice(&img.pixels[py * tw + px].to_array());
        }
    }
    Ok(Picture { rgba, missing })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_is_checked_and_made_canonical() {
        let ok = |t: &str| validate(t).unwrap();
        assert_eq!(ok("https://tiles.example.org/{z}/{x}/{y}.png"), "https://tiles.example.org/{z}/{x}/{y}.png");
        // the services' documents write ${z}; spaces around are not part of it; {s} is a
        assert_eq!(ok("  http://${s}.t.org/${z}/${y}/${x}  "), "http://a.t.org/{z}/{y}/{x}");
        assert_eq!(ok("HTTPS://t.org/{z}/{x}/{y}?key=1"), "HTTPS://t.org/{z}/{x}/{y}?key=1");
        for (bad, why) in [
            ("ftp://t.org/{z}/{x}/{y}", "http"),
            ("t.org/{z}/{x}/{y}", "http"),
            ("https://t.org/{z}/{x}.png", "{y}"),
            ("https://t.org/{x}/{y}", "{z}"),
            ("https://t.org/{z}/{x}/{y}/{r}.png", "{r}"),
            ("https://t.org/{z}/{x}/{y}/{", "{"),
            ("https://t.org/{z}/{x}/{y}}", "}"),
            ("https://t.org/ {z}/{x}/{y}", "space"),
            ("https:///{z}/{x}/{y}", "host"),
            ("", "http"),
        ] {
            let e = validate(bad).unwrap_err();
            assert!(e.contains(why), "{bad:?}: {e}");
        }
    }

    #[test]
    fn a_tile_has_its_place_in_the_address() {
        assert_eq!(tile_url("https://t.org/{z}/{x}/{y}.png", 7, 12, 99), "https://t.org/7/12/99.png");
        assert_eq!(tile_url("https://t.org/tile/${z}/${y}/${x}", 3, 1, 2), "https://t.org/tile/3/2/1");
        assert_eq!(tile_url("https://t.org/{z}/{z}/{x}/{y}", 3, 1, 2), "https://t.org/3/3/1/2");
    }

    #[test]
    fn a_custom_basemap_is_named_by_its_host() {
        assert_eq!(host_of("https://www.tiles.example.org:8080/{z}/{x}/{y}.png"), "tiles.example.org");
        assert_eq!(host_of("http://u:p@127.0.0.1:5000/t/{z}/{x}/{y}"), "127.0.0.1");
        assert_eq!(host_of("https://a.b?x=1"), "a.b");
        let c = Custom {
            template: "https://tiles.example.org/{z}/{x}/{y}".into(),
            attribution: String::new(),
            max_zoom: 12,
        };
        assert_eq!(c.name(), "Custom: tiles.example.org");
    }

    #[test]
    fn a_templates_number_is_stable_and_tells_templates_apart() {
        let of = |t: &str| Custom { template: t.into(), attribution: "a".into(), max_zoom: 5 }.id();
        assert_eq!(of("https://t.org/{z}/{x}/{y}"), of("https://t.org/{z}/{x}/{y}"));
        assert_ne!(of("https://t.org/{z}/{x}/{y}"), of("https://t.org/{z}/{y}/{x}"));
        // (FNV-1a of the text, fixed: it names a folder of tiles kept on disk between versions)
        assert_eq!(of("a"), 0xaf63dc4c8601ec8c);
        // the cache folder of a custom basemap is its own
        assert!(cache_dir(Basemap::Custom(255)).ends_with("custom-00000000000000ff"));
        assert!(cache_dir(Basemap::OpenStreetMap).ends_with("OpenStreetMap"));
    }

    #[test]
    fn the_custom_basemaps_round_trip_through_the_settings_file() {
        let mut s = crate::settings::Settings::default();
        assert!(s.custom_basemaps.is_empty());
        s.custom_basemaps.push(crate::settings::CustomBasemap {
            template: "https://t.org/{z}/{x}/{y}.png".into(),
            attribution: "© me".into(),
            max_zoom: 14,
        });
        let text = serde_json::to_string(&s).unwrap();
        let back: crate::settings::Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back, s);
        // settings written before there were custom basemaps still read
        let old: crate::settings::Settings = serde_json::from_str(r#"{ "theme": "Default" }"#).unwrap();
        assert!(old.custom_basemaps.is_empty());
        // an entry with no attribution reads too
        let some: crate::settings::Settings = serde_json::from_str(
            r#"{ "custom_basemaps": [ { "template": "https://t.org/{z}/{x}/{y}", "max_zoom": 9 } ] }"#,
        )
        .unwrap();
        assert_eq!((some.custom_basemaps[0].max_zoom, some.custom_basemaps[0].attribution.as_str()), (9, ""));
    }
}
