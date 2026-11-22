//! The SAM tool of the Viewer: Segment Anything on the image of the objects shown, one click at a time. With the tool on,
//! the part of the image in view (with a margin) goes through SAM's encoder once, in a thread of its own (the status bar
//! says so); then each click runs only the decoder and draws the mask it gives over the map: a left click is inside the
//! object, a right click outside it, a drag a box. Enter keeps the object (its prompts are written into the model, in one
//! step of undo, see [`write`]); Esc lets it go.
//!
//! What is kept is the prompts, not the masks: they go to a GeoPackage beside the model (a name from their content, so
//! that undo finds the file it had) read by a `vector.read` that feeds the `prompts` of a `segment.sam` node of the same
//! image, which cuts the objects when the model runs. Those objects are the SAM node's own: they are not printed into
//! another segmentation. With a class chosen the object takes it (the `class` of its group), so it is a sample of that
//! class for what reads the objects' classes.
//!
//! The model is a [`Backend`] (the real one loads ONNX Runtime and the cached weights; the tests give a fake).

use std::hash::{Hash, Hasher};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Ui};
use terraflux_graph::{Model, Node, Source};
use terraflux_ops::objects::deep::{Embedded, Group, Prompt, Tile, decode_group, encode_tile, stretch_range};
use terraflux_ops::objects::onnx::{Ort, Session};
use terraflux_ops::{Registry, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::app::Session as App;

/// The name the node that reads the prompts takes, and the one of the SAM node.
const READER: &str = "sam_prompts";
const NODE: &str = "sam";
/// A drag shorter than this (screen points) is a click, not a box.
pub const BOX_LEAST: f32 = 4.0;
/// The colour the mask is drawn in, and the one of the objects already kept.
pub const MASK: Color32 = Color32::from_rgb(40, 200, 255);
pub const KEPT: Color32 = Color32::from_rgb(255, 190, 40);

// ---- what the map tells and is told

/// What the pointer did on the map in SAM mode, in cells of the map shown (a fraction inside a cell).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Point { x: f32, y: f32, positive: bool },
    Box { x0: f32, y0: f32, x1: f32, y1: f32 },
}

/// A mask to draw: the cells of a window, with an id that changes when they do.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskView {
    pub id: u64,
    pub r0: usize,
    pub c0: usize,
    pub h: usize,
    pub w: usize,
    pub cells: Vec<bool>,
    /// The cells as runs along a row: (row, first column, one past the last), in the cells of the window.
    pub runs: Vec<[u32; 3]>,
}

impl MaskView {
    pub fn new(id: u64, t: Tile, cells: Vec<bool>) -> MaskView {
        let mut runs = Vec::new();
        for r in 0..t.h {
            let mut c = 0;
            while c < t.w {
                if cells[r * t.w + c] {
                    let from = c;
                    while c < t.w && cells[r * t.w + c] {
                        c += 1;
                    }
                    runs.push([r as u32, from as u32, c as u32]);
                } else {
                    c += 1;
                }
            }
        }
        MaskView { id, r0: t.r0, c0: t.c0, h: t.h, w: t.w, cells, runs }
    }
}

/// What the map draws over the data for the tool: the masks (and whether each is an object already kept), the points
/// (cell, inside or not) and the box.
#[derive(Clone, Default)]
pub struct Paint {
    pub masks: Vec<(Arc<MaskView>, bool)>,
    pub points: Vec<([f32; 2], bool)>,
    pub boxed: Option<[f32; 4]>,
}

// ---- the model

/// The three bands SAM is given, and which cells have a value.
pub struct Image3 {
    pub grid: Arc<Grid>,
    pub bands: [Arc<Vec<f64>>; 3],
    pub valid: Vec<bool>,
    pub key: usize,
}

/// The three bands of a stack to give SAM: the first three, or the ones named in `names`.
type Bands = (Arc<Grid>, [Arc<Vec<f64>>; 3]);

fn pick(v: &Value, names: Option<&[String]>) -> Result<Bands, String> {
    {
        let Value::Stack(st) = v else { return Err("SAM needs an image of three bands (red, green, blue)".into()) };
        let have: Vec<String> = st.keys.iter().map(|k| k.to_string()).collect();
        let at: Vec<usize> = match names {
            Some(n) if n.len() == 3 => n
                .iter()
                .map(|x| have.iter().position(|h| h.eq_ignore_ascii_case(x)).ok_or(format!("no layer {x:?}")))
                .collect::<Result<_, _>>()?,
            _ => (0..3).collect(),
        };
        if at.iter().any(|&k| k >= st.layers.len()) {
            return Err("SAM needs an image of three bands (red, green, blue)".into());
        }
        Ok((st.grid.clone(), [st.layers[at[0]].clone(), st.layers[at[1]].clone(), st.layers[at[2]].clone()]))
    }
}

impl Image3 {
    /// The image of three bands, with the cells that have a value.
    pub fn new(grid: Arc<Grid>, bands: [Arc<Vec<f64>>; 3]) -> Image3 {
        let valid = (0..grid.cells()).map(|i| bands.iter().any(|b| !b[i].is_nan())).collect();
        let key = Arc::as_ptr(&bands[0]) as usize;
        Image3 { grid, bands, valid, key }
    }
}

/// SAM's two halves as the tool needs them: the encoder on a window, the decoder on a group of prompts.
pub trait Backend: Send + Sync {
    fn encode(&self, image: &Image3, tile: &Tile) -> Result<Embedded, String>;
    /// The cells of the window's object for the prompts (none: the prompts give no mask).
    fn decode(&self, e: &Embedded, g: &Group, image: &Image3) -> Result<Option<Vec<bool>>, String>;
}

/// The real model: ONNX Runtime and the weights in the cache, loaded the first time they are needed.
pub struct Real {
    paths: (std::path::PathBuf, std::path::PathBuf),
    loaded: Mutex<Option<(Session, Session)>>,
}

impl Real {
    /// The cached weights (the quantised ones, else the full), or what to do to have them.
    pub fn new() -> Result<Real, String> {
        use terraflux_ops::objects::weights;
        let cache = terraflux_ops::cache::root();
        let src = [&weights::QUANT, &weights::FULL].into_iter().find(|s| weights::cached(&cache, s)).ok_or(
            "the SAM weights are not downloaded yet: run a segment.sam node once (it asks before downloading 75 MB)",
        )?;
        let dir = cache.join("sam").join(src.name);
        Ok(Real { paths: (dir.join("encoder.onnx"), dir.join("decoder.onnx")), loaded: Mutex::new(None) })
    }

    fn with<R>(&self, f: impl FnOnce(&Session, &Session) -> Result<R, String>) -> Result<R, String> {
        let mut g = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            let ort = Ort::load().map_err(|e| e.to_string())?;
            let open = |p: &std::path::Path| ort.session(p).map_err(|e| e.to_string());
            *g = Some((open(&self.paths.0)?, open(&self.paths.1)?));
        }
        let (e, d) = g.as_ref().expect("loaded above");
        f(e, d)
    }
}

impl Backend for Real {
    fn encode(&self, image: &Image3, tile: &Tile) -> Result<Embedded, String> {
        self.with(|enc, _| {
            let rgb = [image.bands[0].as_slice(), image.bands[1].as_slice(), image.bands[2].as_slice()];
            let ranges = [stretch_range(rgb[0]), stretch_range(rgb[1]), stretch_range(rgb[2])];
            encode_tile(enc, rgb, image.grid.width, tile, ranges).map_err(|e| e.to_string())
        })
    }

    fn decode(&self, e: &Embedded, g: &Group, image: &Image3) -> Result<Option<Vec<bool>>, String> {
        self.with(|_, dec| decode_group(dec, e, g, &image.valid, image.grid.width).map_err(|e| e.to_string()))
    }
}

// ---- the state

/// What the Viewer keeps of the tool.
#[derive(Default)]
pub struct SamTool {
    pub on: bool,
    /// The model to use (none: the real one); the tests give a fake.
    pub backend: Option<Arc<dyn Backend>>,
    real: Option<Arc<dyn Backend>>,
    /// The class the objects kept take (a name of the palette; none: no class).
    pub class: Option<String>,
    pub image: Option<Arc<Image3>>,
    embedding: Option<Arc<Embedded>>,
    encoding: Option<(Receiver<Result<Embedded, String>>, Instant)>,
    /// The encoder failed for this image: not tried again until the tool is turned off and on.
    failed: bool,
    /// The prompts of the object being cut.
    pub group: Group,
    pub mask: Option<Arc<MaskView>>,
    decoding: Option<Receiver<Result<Option<Vec<bool>>, String>>>,
    want: bool,
    next_id: u64,
    /// The objects kept since the tool was turned on (drawn until the model runs).
    pub kept: Vec<Arc<MaskView>>,
    pub error: Option<String>,
    /// Enter was pressed: the object is kept as soon as its mask is made.
    pub accepting: bool,
    /// How many encoder and decoder runs were started (the tests count them).
    pub encoded: usize,
    pub decoded: usize,
    /// What the status bar was last told.
    told: Option<&'static str>,
}

impl SamTool {
    /// Whether the encoder is at work.
    pub fn encoding(&self) -> bool {
        self.encoding.is_some()
    }

    pub fn ready(&self) -> bool {
        self.embedding.is_some()
    }

    /// The window the embedding is of.
    pub fn window(&self) -> Option<Tile> {
        self.embedding.as_ref().map(|e| e.tile)
    }

    /// Turned off: nothing is kept but the class.
    pub fn off(&mut self) {
        let (backend, class) = (self.backend.take(), self.class.take());
        *self = SamTool { backend, class, ..Default::default() };
    }

    fn model(&mut self) -> Result<Arc<dyn Backend>, String> {
        if let Some(b) = &self.backend {
            return Ok(b.clone());
        }
        if self.real.is_none() {
            self.real = Some(Arc::new(Real::new()?));
        }
        Ok(self.real.clone().expect("made above"))
    }

    /// The image given (another image lets everything go) and, when the part in view is not in the window encoded, the
    /// encoder started on it.
    pub fn sync(&mut self, ctx: &egui::Context, image: Option<Arc<Image3>>, view: Option<(&Grid, [f32; 4])>) {
        let Some(image) = image else {
            self.image = None;
            return;
        };
        if self.image.as_ref().is_none_or(|i| i.key != image.key) {
            let (backend, class, on) = (self.backend.take(), self.class.take(), self.on);
            *self = SamTool { backend, class, on, image: Some(image.clone()), ..Default::default() };
        }
        let Some((g, b)) = view else { return };
        if (g.width, g.height) != (image.grid.width, image.grid.height) {
            self.error = Some("the map shown is not on the grid of the image: show the image's own objects".into());
            return;
        }
        if self.failed || self.encoding.is_some() {
            return;
        }
        let need = self.embedding.as_ref().is_none_or(|e| !covers(&e.tile, b));
        if need && let Some(win) = crate::live_segment::window_cells(b, g.width, g.height) {
            let tile = Tile { r0: win.y0, c0: win.x0, h: win.height, w: win.width };
            self.encode(ctx, tile);
        }
    }

    fn encode(&mut self, ctx: &egui::Context, tile: Tile) {
        let backend = match self.model() {
            Ok(b) => b,
            Err(e) => {
                self.error = Some(e);
                self.failed = true;
                return;
            }
        };
        let Some(image) = self.image.clone() else { return };
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new().name("sam encoder".into()).stack_size(16 << 20).spawn(move || {
            let _ = tx.send(backend.encode(&image, &tile));
            ctx.request_repaint();
        });
        match spawned {
            Ok(_) => {
                self.encoding = Some((rx, Instant::now()));
                self.encoded += 1;
                self.error = None;
            }
            Err(e) => {
                self.error = Some(format!("no thread for the encoder: {e}"));
                self.failed = true;
            }
        }
    }

    /// Take what the threads made, and start the decoder when the prompts changed.
    pub fn poll(&mut self, ctx: &egui::Context) {
        if let Some((rx, _)) = &self.encoding {
            match rx.try_recv() {
                Ok(Ok(e)) => {
                    self.embedding = Some(Arc::new(e));
                    self.encoding = None;
                    self.want = true;
                }
                Ok(Err(e)) => {
                    self.error = Some(e);
                    self.failed = true;
                    self.encoding = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.encoding = None,
            }
        }
        if let Some(rx) = &self.decoding {
            match rx.try_recv() {
                Ok(Ok(cells)) => {
                    self.decoding = None;
                    self.error = None;
                    self.mask = match (cells, self.embedding.as_ref()) {
                        (Some(cells), Some(e)) if cells.iter().any(|&c| c) => {
                            self.next_id += 1;
                            Some(Arc::new(MaskView::new(self.next_id, e.tile, cells)))
                        }
                        _ => None,
                    };
                }
                Ok(Err(e)) => {
                    self.decoding = None;
                    self.error = Some(e);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.decoding = None,
            }
        }
        if self.want && self.decoding.is_none() {
            self.decode(ctx);
        }
        if self.encoding.is_some() || self.decoding.is_some() {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }

    fn decode(&mut self, ctx: &egui::Context) {
        let (Some(e), Some(image), Some(backend)) = (self.embedding.clone(), self.image.clone(), self.model().ok())
        else {
            return;
        };
        self.want = false;
        if self.group.anchor().is_none() {
            self.mask = None;
            return;
        }
        let group = self.group.clone();
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new().name("sam decoder".into()).stack_size(16 << 20).spawn(move || {
            let _ = tx.send(backend.decode(&e, &group, &image));
            ctx.request_repaint();
        });
        if spawned.is_ok() {
            self.decoding = Some(rx);
            self.decoded += 1;
        }
    }

    /// What the pointer did: a click adds a point, a drag sets the box (a new one replaces it).
    pub fn event(&mut self, e: Event) {
        match e {
            Event::Point { x, y, positive } => {
                self.group.prompts.push(Prompt::Point { x: f64::from(x), y: f64::from(y), positive })
            }
            Event::Box { x0, y0, x1, y1 } => {
                self.group.prompts.retain(|p| !matches!(p, Prompt::Box { .. }));
                self.group.prompts.push(Prompt::Box {
                    x0: f64::from(x0.min(x1)),
                    y0: f64::from(y0.min(y1)),
                    x1: f64::from(x0.max(x1)),
                    y1: f64::from(y0.max(y1)),
                });
            }
        }
        self.want = true;
    }

    /// The prompt let go (Esc): the object being cut is dropped.
    pub fn drop_prompt(&mut self) {
        self.group = Group::default();
        self.mask = None;
        self.want = false;
        self.accepting = false;
    }

    /// The last point taken away (a click on the mask's own point is not needed: ctrl+Z is the model's).
    pub fn undo_point(&mut self) {
        self.group.prompts.pop();
        self.want = true;
    }

    /// Whether there is a prompt, or a mask, to keep or drop.
    pub fn has_prompt(&self) -> bool {
        !self.group.prompts.is_empty()
    }

    /// The object, when it was asked to be kept and its mask is made for the prompts as they are now.
    pub fn take(&mut self) -> Option<(Group, Arc<MaskView>)> {
        if !self.accepting || self.want || self.decoding.is_some() {
            return None;
        }
        self.accepting = false;
        let mask = self.mask.clone()?;
        let mut g = std::mem::take(&mut self.group);
        g.class = self.class.clone();
        self.mask = None;
        self.kept.push(mask.clone());
        Some((g, mask))
    }

    /// What the map draws.
    pub fn paint(&self) -> Paint {
        let mut p = Paint { masks: self.kept.iter().map(|m| (m.clone(), true)).collect(), ..Default::default() };
        p.masks.extend(self.mask.iter().map(|m| (m.clone(), false)));
        for q in &self.group.prompts {
            match *q {
                Prompt::Point { x, y, positive } => p.points.push(([x as f32, y as f32], positive)),
                Prompt::Box { x0, y0, x1, y1 } => p.boxed = Some([x0 as f32, y0 as f32, x1 as f32, y1 as f32]),
            }
        }
        p
    }

    /// The word for the status bar when what the tool is doing changed.
    pub fn status(&mut self) -> Option<String> {
        let now = if self.encoding.is_some() {
            Some("SAM: the encoder is reading the window...")
        } else if self.embedding.is_some() {
            Some("SAM: ready, click the object")
        } else {
            None
        };
        if now == self.told {
            return None;
        }
        self.told = now;
        now.map(String::from)
    }
}

/// Whether the window encoded holds the part in view (`b`: columns and rows seen) and is not much larger than it.
fn covers(t: &Tile, b: [f32; 4]) -> bool {
    let (x0, y0, x1, y1) = (t.c0 as f32, t.r0 as f32, (t.c0 + t.w) as f32, (t.r0 + t.h) as f32);
    let holds = x0 <= b[0] + 1.0 && y0 <= b[1] + 1.0 && x1 >= b[2] - 1.0 && y1 >= b[3] - 1.0;
    // (zoomed far in: the window has too few cells of the view)
    let big = (b[2] - b[0]) * 3.0 >= t.w as f32 || (b[3] - b[1]) * 3.0 >= t.h as f32;
    holds && big
}

// ---- the panel

/// Whether the tool can work on the objects of a node: it reads an image.
pub fn can(s: &App, node: &str, tool: &SamTool) -> Result<(), String> {
    if tool.on {
        return image_of(s, node, tool).map(|_| ());
    }
    let n = s.source.nodes.iter().find(|n| n.id == node).ok_or("the objects' node is not in the model's top level")?;
    match n.inputs.get("image") {
        Some(Source::Ref(_)) => Ok(()),
        _ => Err("the node of these objects reads no image: SAM works on a segmenter's image".into()),
    }
}

/// The image a node reads (its `image` input), as SAM takes it: the reference it comes from and the three bands (the
/// ones of the tool when they are still the same).
pub fn image_of(s: &App, node: &str, tool: &SamTool) -> Result<(String, Arc<Image3>), String> {
    let n = s.source.nodes.iter().find(|n| n.id == node).ok_or("the objects' node is not in the model's top level")?;
    let Some(Source::Ref(r)) = n.inputs.get("image") else {
        return Err("the node of these objects reads no image".into());
    };
    let v = crate::live_segment::input_of(s, n, "image").ok_or("the image has not run")?;
    let names: Option<Vec<String>> = n.params.get("bands").and_then(|b| serde_json::from_value(b.clone()).ok());
    let (grid, bands) = pick(&v, names.as_deref())?;
    match &tool.image {
        Some(i) if i.key == Arc::as_ptr(&bands[0]) as usize => Ok((r.clone(), i.clone())),
        _ => Ok((r.clone(), Arc::new(Image3::new(grid, bands)))),
    }
}

/// What the tool does at a frame, once the map has been drawn: its keys, the image and the window encoded, what the
/// pointer did, the threads' results, a word for the status bar and, when asked, the object kept in the model.
pub fn frame(
    ui: &Ui,
    s: &mut App,
    r: &Registry,
    st: &mut crate::objects_inspect::ObjectsUi,
    events: &[Event],
    node: &str,
    view: Option<(&Grid, [f32; 4])>,
) {
    if !st.sam.on {
        return;
    }
    let ctx = ui.ctx();
    keys(ui, &mut st.sam);
    let image = image_of(s, node, &st.sam);
    match &image {
        Ok((_, img)) => st.sam.sync(ctx, Some(img.clone()), view),
        Err(_) => st.sam.sync(ctx, None, None),
    }
    for e in events {
        st.sam.event(*e);
    }
    st.sam.poll(ctx);
    if let Some(w) = st.sam.status() {
        s.notice = Some((w, Instant::now()));
    }
    if let Ok((from, img)) = &image
        && let Some((g, _)) = st.sam.take()
    {
        match write(s, r, from, &img.grid, &g) {
            Ok(id) => s.notice = Some((format!("SAM: an object kept in {id}"), Instant::now())),
            Err(e) => st.sam.error = Some(e),
        }
    }
}

/// The tool's block in the objects' side panel: the switch, how to use it, the class and what it has done.
pub fn panel(ui: &mut Ui, st: &mut crate::objects_inspect::ObjectsUi, image: Result<(), String>, node: &str) {
    crate::theme::section(ui, "SAM");
    let mut on = st.sam.on;
    ui.horizontal(|ui| {
        let b = ui.selectable_label(on, "SAM tool").on_hover_text(
            "Segment Anything: click an object in the map to cut it, with the encoder run once on the view",
        );
        if b.clicked() {
            on = !on;
        }
    });
    if on != st.sam.on {
        if on {
            st.sampling = false;
            st.sam.on = true;
        } else {
            st.sam.off();
        }
    }
    if !st.sam.on {
        if let Err(e) = image {
            ui.label(RichText::new(e).weak().size(11.0));
        }
        return;
    }
    if let Err(e) = image {
        ui.label(RichText::new(e).color(ui.visuals().warn_fg_color).size(11.0));
        return;
    }
    ui.label(
        RichText::new("left click: inside; right click: outside; drag: a box (middle button moves the map)")
            .weak()
            .size(11.0),
    );
    ui.label(RichText::new("Enter keeps the object, Esc drops it").weak().size(11.0));
    ui.horizontal(|ui| {
        if st.sam.encoding() {
            ui.spinner();
            ui.label(RichText::new("the encoder is reading the window").size(11.0));
        } else if st.sam.ready() {
            ui.label(RichText::new("ready").color(crate::theme::accent()).size(11.0));
        }
    });
    if let Some(e) = &st.sam.error {
        ui.label(RichText::new(e).color(ui.visuals().warn_fg_color).size(11.0));
    }
    // the class the objects kept take
    let classes = st.classes.clone();
    if !classes.is_empty() {
        let shown = st.sam.class.clone().unwrap_or_else(|| "no class".into());
        egui::ComboBox::from_label("class").selected_text(shown).show_ui(ui, |ui| {
            ui.selectable_value(&mut st.sam.class, None, "no class");
            for c in &classes {
                ui.selectable_value(&mut st.sam.class, Some(c.clone()), c);
            }
        });
    }
    ui.horizontal(|ui| {
        let busy = st.sam.mask.is_none() || st.sam.accepting;
        if ui.add_enabled(!busy, egui::Button::new("Keep (Enter)")).clicked() {
            st.sam.accepting = true;
        }
        if ui.add_enabled(st.sam.has_prompt(), egui::Button::new("Drop (Esc)")).clicked() {
            st.sam.drop_prompt();
        }
    });
    if !st.sam.kept.is_empty() {
        let n = st.sam.kept.len();
        ui.label(RichText::new(format!("{n} object{} kept", if n == 1 { "" } else { "s" })).weak().size(11.0))
            .on_hover_text(format!("their prompts are in {node}'s SAM node"));
    }
}

/// The keys of the tool: Enter keeps the object, Esc drops it (not while a text is being typed).
pub fn keys(ui: &Ui, st: &mut SamTool) {
    if !st.on || ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    let (enter, esc) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if enter && st.has_prompt() {
        st.accepting = true;
    }
    if esc && st.has_prompt() {
        st.drop_prompt();
    }
}

// ---- writing the prompts into the model

/// One row of the prompts' file.
struct Row {
    geometry: Geometry,
    label: f64,
    group: String,
    class: String,
}

fn text_of(c: &Column, k: usize) -> String {
    match c {
        Column::Texts(t) => t[k].clone(),
        Column::Numbers(x) => {
            if x[k].is_nan() {
                String::new()
            } else {
                format!("{}", x[k])
            }
        }
    }
}

/// The rows of a prompts file as it is.
fn rows_of(v: &Vector) -> Vec<Row> {
    let col = |n: &str| v.attributes.iter().find(|(a, _)| a == n).map(|(_, c)| c);
    (0..v.geometries.len())
        .map(|k| Row {
            geometry: v.geometries[k].clone(),
            label: col("label").map(|c| text_of(c, k)).and_then(|t| t.trim().parse().ok()).unwrap_or(1.0),
            group: col("group").map(|c| text_of(c, k)).unwrap_or_default(),
            class: col("class").map(|c| text_of(c, k)).unwrap_or_default(),
        })
        .collect()
}

/// The prompts of a group as rows, in the map's coordinates (a box is a polygon of its corners).
fn rows_for(g: &Group, grid: &Grid, name: &str) -> Vec<Row> {
    let t = grid.transform;
    let xy = |x: f64, y: f64| [t[0] + x * t[1] + y * t[2], t[3] + x * t[4] + y * t[5]];
    let class = g.class.clone().unwrap_or_default();
    g.prompts
        .iter()
        .map(|p| {
            let (geometry, label) = match *p {
                Prompt::Point { x, y, positive } => (Geometry::Point(xy(x, y)), if positive { 1.0 } else { 0.0 }),
                Prompt::Box { x0, y0, x1, y1 } => {
                    (Geometry::Polygon(vec![vec![xy(x0, y0), xy(x1, y0), xy(x1, y1), xy(x0, y1), xy(x0, y0)]]), 1.0)
                }
            };
            Row { geometry, label, group: name.to_string(), class: class.clone() }
        })
        .collect()
}

fn vector_of(rows: &[Row], grid: &Grid) -> Vector {
    let crs = grid
        .geo
        .wkt
        .clone()
        .or_else(|| grid.geo.epsg().and_then(|c| terraflux_gdal::crs_wkt(&format!("EPSG:{c}")).ok()));
    Vector {
        geometries: rows.iter().map(|r| r.geometry.clone()).collect(),
        attributes: vec![
            ("label".into(), Column::Numbers(rows.iter().map(|r| r.label).collect())),
            ("group".into(), Column::Texts(rows.iter().map(|r| r.group.clone()).collect())),
            ("class".into(), Column::Texts(rows.iter().map(|r| r.class.clone()).collect())),
        ],
        crs,
    }
}

/// The SAM node of the image `image` that has prompts read from a file, and the node that reads them.
fn target(m: &Model, image: &str) -> Option<(String, String)> {
    m.nodes.iter().filter(|n| n.op == "segment.sam").find_map(|n| {
        let same = matches!(n.inputs.get("image"), Some(Source::Ref(r)) if r == image);
        let Some(Source::Ref(p)) = n.inputs.get("prompts") else { return None };
        let reader = p.split_once('.')?.0;
        (same && m.nodes.iter().any(|x| x.id == reader && x.op == "vector.read"))
            .then(|| (n.id.clone(), reader.to_string()))
    })
}

/// The folder the prompts' files go in: beside the model, else the cache.
fn folder(s: &App) -> std::path::PathBuf {
    match s.path.as_ref().and_then(|p| p.parent()) {
        Some(d) => d.join("drawn"),
        None => terraflux_ops::cache::root().join("sam_prompts"),
    }
}

/// A group of prompts kept: written (with the ones kept before, when the model has a SAM node of this image already)
/// to a GeoPackage named by its content, read by the `vector.read` that feeds the node's `prompts`; the nodes are made
/// when there are none. One step of undo. The SAM node's id.
pub fn write(s: &mut App, r: &Registry, image: &str, grid: &Grid, g: &Group) -> Result<String, String> {
    let before = s.source.clone();
    let made = write_in(s, r, image, grid, g);
    match made {
        Ok(id) => {
            let what = g.class.as_deref().map_or(String::new(), |c| format!(" ({c})"));
            s.history.name(&format!("SAM object{what} in {id}"));
            s.edited();
            Ok(id)
        }
        Err(e) => {
            s.source = before;
            Err(e)
        }
    }
}

fn write_in(s: &mut App, r: &Registry, image: &str, grid: &Grid, g: &Group) -> Result<String, String> {
    let top: &[String] = &[];
    let found = target(&s.source, image);
    // (the rows kept before)
    let mut rows: Vec<Row> = match &found {
        Some((_, reader)) => {
            let path = s.source.nodes.iter().find(|n| n.id == *reader).and_then(|n| n.params.get("path")?.as_str());
            path.and_then(|p| terraflux_gdal::read_vector(p, None).ok()).map(|v| rows_of(&v)).unwrap_or_default()
        }
        None => Vec::new(),
    };
    let mut names: Vec<String> = rows.iter().map(|r| r.group.clone()).collect();
    names.sort();
    names.dedup();
    let mut k = names.len() + 1;
    while names.contains(&format!("g{k}")) {
        k += 1;
    }
    rows.extend(rows_for(g, grid, &format!("g{k}")));
    // the file: its name from what it holds
    let key = {
        let mut h = std::hash::DefaultHasher::new();
        for r in &rows {
            format!("{:?}|{}|{}|{}", r.geometry, r.label, r.group, r.class).hash(&mut h);
        }
        h.finish()
    };
    let dir = folder(s);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = format!("sam_prompts_{:08x}", key as u32);
    let path = dir.join(format!("{name}.gpkg"));
    if !path.exists() {
        terraflux_gdal::write_vector(&path, &name, &vector_of(&rows, grid))?;
    }
    let text = path.to_string_lossy().into_owned();
    match found {
        Some((sam, reader)) => {
            crate::editor::set_param(&mut s.source, top, &reader, "path", &text)?;
            Ok(sam)
        }
        None => {
            let from = image.split_once('.').map(|x| x.0);
            let at = crate::builder::beside(&s.source.nodes, from);
            let reader = crate::editor::add_node_named(&mut s.source, r, top, "vector.read", Some(READER), at)?;
            crate::editor::set_param(&mut s.source, top, &reader, "path", &text)?;
            if let Some(n) = s.source.nodes.iter_mut().find(|n| n.id == reader) {
                n.note = Some("The prompts of the SAM tool in the Viewer: points (label 1 inside, 0 outside) and boxes, a group each object.".into());
            }
            let at = crate::builder::beside(&s.source.nodes, Some(&reader));
            let sam = crate::editor::add_node_named(&mut s.source, r, top, "segment.sam", Some(NODE), at)?;
            crate::editor::connect(&mut s.source, r, top, image, &sam, "image")?;
            crate::editor::connect(&mut s.source, r, top, &format!("{reader}.vector"), &sam, "prompts")?;
            if let Some(n) = s.source.nodes.iter_mut().find(|n| n.id == sam) {
                n.note =
                    Some("The objects cut by SAM from the prompts clicked in the Viewer: one object a group.".into());
            }
            Ok(sam)
        }
    }
}

/// The nodes of the model that read the prompts (the tests look at them).
pub fn nodes(m: &Model) -> Vec<&Node> {
    m.nodes.iter().filter(|n| n.op == "segment.sam" && n.inputs.contains_key("prompts")).collect()
}
