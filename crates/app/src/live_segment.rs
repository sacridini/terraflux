//! Live segmentation in the Viewer: while the shown objects are a `segment.multiresolution` node's, sliders for its
//! scale, shape and compactness segment the part of the image in view (with a small margin, read every second, third...
//! cell when it has more than [`MOST_CELLS`]) in a thread of its own, and the objects found are drawn over the map as
//! a temporary preview (see [`draw`]). *Apply* writes the values in the node's settings, one step of undo.
//!
//! One worker thread at a time: a new value is kept (the latest only) while it runs and made when it ends, so the
//! sliders never wait and never pile threads up. The worker segments, then (when the model has no `segment.scale`
//! node of the same image to read it from) runs ESP over the same window for the curve beside the sliders.

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Ui};
use terraflux_graph::{Node, Source};
use terraflux_ops::objects::{Objects, Window, WindowSettings, esp_window, segment_window};
use terraflux_ops::{Table, Value};
use terraflux_raster::Grid;

use crate::app::Session;
use crate::map_view::{MapView, ObjectsDraw};

/// The most cells of the window segmented at once: a window with more is read every second, third... cell.
pub const MOST_CELLS: usize = 1_000_000;
/// The most cells (read) the ESP curve is made on, and its levels.
const ESP_CELLS: usize = 100_000;
const ESP_LEVELS: usize = 20;
/// The margin around the part in view, a share of its side (and at least [`MIN_MARGIN`] cells).
const MARGIN: f32 = 0.1;
const MIN_MARGIN: f32 = 4.0;
/// The colour the preview's outlines are drawn in.
pub const PREVIEW: Color32 = Color32::from_rgb(255, 64, 200);

/// The op the panel is for.
pub const OP: &str = "segment.multiresolution";

/// The window of an image (`width` by `height` cells) that a view of the box `b` of its cells (`[x0, y0, x1, y1]`)
/// segments: the box with a margin, cut to the image; none when nothing of the image is in it.
pub fn window_cells(b: [f32; 4], width: usize, height: usize) -> Option<Window> {
    let (mx, my) = (((b[2] - b[0]) * MARGIN).max(MIN_MARGIN), ((b[3] - b[1]) * MARGIN).max(MIN_MARGIN));
    let x0 = (b[0] - mx).floor().max(0.0) as usize;
    let y0 = (b[1] - my).floor().max(0.0) as usize;
    let x1 = ((b[2] + mx).ceil().max(0.0) as usize).min(width);
    let y1 = ((b[3] + my).ceil().max(0.0) as usize).min(height);
    (x1 > x0 && y1 > y0).then(|| Window { x0, y0, width: x1 - x0, height: y1 - y0 })
}

/// The window segmented for what the view shows, and every how many cells it is read: the box in view is of the
/// grid shown (`shown`), the image is `image`.
pub fn plan(shown: &Grid, b: [f32; 4], image: &Grid) -> Option<(Window, usize)> {
    let cells = if (shown.width, shown.height) == (image.width, image.height) {
        b
    } else {
        let place = crate::map_view::place_on(image, shown)?;
        let (a, z) = (place.to_source(egui::vec2(b[0], b[1])), place.to_source(egui::vec2(b[2], b[3])));
        [a.x.min(z.x), a.y.min(z.y), a.x.max(z.x), a.y.max(z.y)]
    };
    let win = window_cells(cells, image.width, image.height)?;
    Some((win, win.step_for(MOST_CELLS)))
}

/// What the node segments: its image's bands.
#[derive(Clone)]
struct Image {
    grid: Arc<Grid>,
    names: Vec<String>,
    data: Vec<Arc<Vec<f64>>>,
}

impl Image {
    /// A map or a stack in memory (rasters on disk are not segmented here).
    fn of(v: &Value) -> Option<Image> {
        match v {
            Value::Map(m) => {
                Some(Image { grid: m.grid.clone(), names: vec!["image".into()], data: vec![m.data.clone()] })
            }
            Value::Stack(st) => Some(Image {
                grid: st.grid.clone(),
                names: st.keys.iter().map(|k| k.to_string()).collect(),
                data: st.layers.clone(),
            }),
            _ => None,
        }
    }

    fn key(&self) -> usize {
        self.data.first().map_or(0, |d| Arc::as_ptr(d) as usize)
    }
}

/// The value a node's input reads (`node.port`).
fn input_of(s: &Session, n: &Node, port: &str) -> Option<Value> {
    match n.inputs.get(port)? {
        Source::Ref(r) => {
            let (node, p) = r.split_once('.')?;
            s.value_of(node, p)
        }
        Source::Literal(_) => None,
    }
}

/// The node `id` of the model's top level when it is a `segment.multiresolution`.
fn node_of<'a>(s: &'a Session, id: &str) -> Option<&'a Node> {
    s.source.nodes.iter().find(|n| n.id == id && n.op == OP)
}

/// Whether the live segmentation is for the node `id`.
pub fn is_target(s: &Session, id: &str) -> bool {
    node_of(s, id).is_some()
}

/// The settings a node has (its scale, shape and compactness; the scale of its output when it has none given).
fn base_of(s: &Session, n: &Node, image: &Image) -> [f64; 3] {
    let num = |k: &str| n.params.get(k).and_then(|v| v.as_f64());
    let scale = num("scale")
        .or_else(|| match s.value_of(&n.id, "scale") {
            Some(Value::Number(x)) => Some(x),
            _ => None,
        })
        .filter(|x| x.is_finite() && *x > 0.0)
        .unwrap_or_else(|| rough_scale(image));
    [scale, num("shape").unwrap_or(0.1), num("compactness").unwrap_or(0.5)]
}

/// A scale to start from when the node has none: a twentieth of the range of the first band.
fn rough_scale(image: &Image) -> f64 {
    let Some(b) = image.data.first() else { return 1.0 };
    let step = b.len().div_ceil(100_000).max(1);
    let (lo, hi) =
        b.iter().step_by(step).filter(|v| v.is_finite()).fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    let r = (hi - lo) / 20.0;
    if r.is_finite() && r > 0.0 { r } else { 1.0 }
}

/// The settings of a segmentation as the node has them (its weights, mutual and seed with the sliders' values).
fn settings_of(n: &Node, scale: f64, shape: f64, compactness: f64) -> WindowSettings {
    let weights: Vec<f64> = n
        .params
        .get("weights")
        .and_then(|w| w.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
        .unwrap_or_default();
    WindowSettings {
        scale,
        shape,
        compactness,
        weights,
        mutual: n.params.get("mutual").and_then(|v| v.as_bool()).unwrap_or(true),
        seed: n.params.get("seed").and_then(|v| v.as_u64()).unwrap_or(0),
    }
}

/// The segmentation of a window, as the worker made it.
#[derive(Clone)]
pub struct Preview {
    pub objects: Arc<Objects>,
    pub win: Window,
    pub step: usize,
    pub secs: f64,
}

impl Preview {
    /// What the panel says of it.
    pub fn text(&self) -> String {
        let read = if self.step > 1 {
            let n = self.win.cells();
            format!(
                ", every {} cell (the window has {} cells)",
                ordinal(self.step),
                if n >= 1_000_000 { format!("{:.1} million", n as f64 / 1e6) } else { n.to_string() }
            )
        } else {
            String::new()
        };
        format!(
            "preview: {} x {} cells{read}, {} objects, {:.2} s",
            self.win.width, self.win.height, self.objects.count, self.secs
        )
    }
}

fn ordinal(n: usize) -> String {
    match n {
        2 => "second".into(),
        3 => "third".into(),
        n => format!("{n}th"),
    }
}

/// A request to the worker.
struct Request {
    key: u64,
    esp_key: Option<u64>,
    image: Image,
    win: Window,
    step: usize,
    set: WindowSettings,
}

enum Msg {
    Seg(Result<Preview, String>),
    Esp(u64, Result<Table, String>),
}

/// What the Viewer keeps of the live segmentation of a node: the sliders, the worker and what it made.
#[derive(Default)]
pub struct Live {
    pub node: String,
    /// The node's own values (the sliders' start), set when the node or its settings change.
    base: [f64; 3],
    pub scale: f64,
    pub shape: f64,
    pub compactness: f64,
    worker: Option<Receiver<Msg>>,
    /// Told to the worker that its result will not be wanted (it leaves out what it has not made yet).
    stale: Arc<AtomicBool>,
    want: Option<Request>,
    asked: Option<u64>,
    pub preview: Option<Preview>,
    esp: Option<(u64, Table)>,
    esp_asked: Option<u64>,
    pub error: Option<String>,
    /// How many worker threads were started (a test counts them).
    pub started: usize,
}

/// The live state of a node's segmentation (kept in the context, by the view's name and the node).
pub fn state(ctx: &egui::Context, salt: &str, node: &str) -> Arc<Mutex<Live>> {
    let id = egui::Id::new(("live segmentation", salt, node));
    ctx.data_mut(|d| d.get_temp_mut_or_insert_with::<Arc<Mutex<Live>>>(id, Default::default).clone())
}

impl Live {
    /// Whether a slider is not at the node's value (a preview is wanted).
    pub fn touched(&self) -> bool {
        (self.scale, self.shape, self.compactness) != (self.base[0], self.base[1], self.base[2])
    }

    /// Whether the worker is at work or has work waiting.
    pub fn busy(&self) -> bool {
        self.worker.is_some() || self.want.is_some()
    }

    /// The node's values taken as the sliders' (the node was edited, or is another).
    fn reset(&mut self, node: &str, base: [f64; 3]) {
        self.stale.store(true, Ordering::Relaxed);
        *self = Live {
            node: node.to_string(),
            base,
            scale: base[0],
            shape: base[1],
            compactness: base[2],
            ..Live::default()
        };
    }

    fn ask(&mut self, req: Request) {
        self.asked = Some(req.key);
        self.want = Some(req);
    }

    /// Take what the worker made, and start the request waiting when it is not at work.
    fn poll(&mut self, ctx: &egui::Context) {
        let mut ended = false;
        if let Some(rx) = &self.worker {
            loop {
                match rx.try_recv() {
                    Ok(Msg::Seg(Ok(p))) => {
                        self.error = None;
                        self.preview = Some(p);
                    }
                    Ok(Msg::Seg(Err(e))) => self.error = Some(e),
                    Ok(Msg::Esp(k, Ok(t))) => self.esp = Some((k, t)),
                    Ok(Msg::Esp(_, Err(_))) => {}
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        ended = true;
                        break;
                    }
                }
            }
            ctx.request_repaint();
        }
        if ended {
            self.worker = None;
        }
        if self.worker.is_none()
            && let Some(req) = self.want.take()
        {
            self.spawn(req);
        }
        if self.busy() {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }

    fn spawn(&mut self, req: Request) {
        let (tx, rx) = mpsc::channel();
        self.stale = Arc::new(AtomicBool::new(false));
        let stale = self.stale.clone();
        if req.esp_key.is_some() {
            self.esp_asked = req.esp_key;
        }
        let spawned =
            std::thread::Builder::new().name("live segmentation".into()).stack_size(16 << 20).spawn(move || {
                let Request { esp_key, image, win, step, set, .. } = req;
                let t = Instant::now();
                let made = segment_window(&image.grid, &image.data, win, step, &set).map_err(|e| e.to_string());
                let secs = t.elapsed().as_secs_f64();
                let seg = made.map(|o| Preview { objects: Arc::new(o), win, step, secs });
                if tx.send(Msg::Seg(seg)).is_err() {
                    return;
                }
                if let Some(k) = esp_key
                    && !stale.load(Ordering::Relaxed)
                {
                    let esp_step = win.step_for(ESP_CELLS).max(step);
                    let t = esp_window(&image.grid, &image.data, &image.names, win, esp_step, ESP_LEVELS, &set)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(Msg::Esp(k, t));
                }
            });
        match spawned {
            Ok(_) => {
                self.worker = Some(rx);
                self.started += 1;
            }
            Err(e) => self.error = Some(format!("no thread for the segmentation: {e}")),
        }
    }

    /// The ESP table for the sliders' window, when one was made.
    pub fn esp(&self) -> Option<&Table> {
        self.esp.as_ref().map(|(_, t)| t)
    }
}

fn hashed(parts: impl FnOnce(&mut std::hash::DefaultHasher)) -> u64 {
    let mut h = std::hash::DefaultHasher::new();
    parts(&mut h);
    h.finish()
}

/// *Apply*: the values written into the node's settings, as one step of undo (named by them). The node must be in
/// the model's top level.
pub fn apply(s: &mut Session, id: &str, scale: f64, shape: f64, compactness: f64) -> Result<String, String> {
    let Some(n) = s.source.nodes.iter_mut().find(|n| n.id == id && n.op == OP) else {
        return Err(format!("{id} is not a segment.multiresolution of the model's top level"));
    };
    n.params.insert("scale".into(), serde_json::json!(scale));
    // (shape and compactness are kept when the node has them or they are not the defaults)
    for (name, v, default) in [("shape", shape, 0.1), ("compactness", compactness, 0.5)] {
        if n.params.contains_key(name) || v != default {
            n.params.insert(name.into(), serde_json::json!(v));
        }
    }
    let msg = format!("{id}: scale {}, shape {}, compactness {}", short(scale), short(shape), short(compactness));
    s.history.name(&format!("Segmentation of {id}: scale {}", short(scale)));
    s.edited();
    Ok(msg)
}

fn short(x: f64) -> String {
    let t = format!("{x:.4}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The preview's outlines as objects to draw over the map (named "preview": the object under the pointer is told as
/// one of it), when there is one.
pub fn draw(live: &Live, id: &str) -> Option<ObjectsDraw> {
    let p = live.preview.as_ref().filter(|_| live.touched())?;
    Some(ObjectsDraw {
        objects: p.objects.clone(),
        colour: PREVIEW,
        width: 2.0,
        name: "preview".into(),
        shown: false,
        picked: None,
        key: (id.to_string(), "preview".into()),
        selected: Vec::new(),
        sampling: false,
        marks: Vec::new(),
    })
}

/// A badge over the map's corner saying what is drawn is a preview (not the node's objects).
pub fn badge(ctx: &egui::Context, rect: egui::Rect, live: &Live) {
    if draw(live, "").is_none() {
        return;
    }
    let painter =
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("live segmentation badge")));
    let text = format!("PREVIEW (not applied): scale {}", short(live.scale));
    let font = egui::FontId::proportional(12.0);
    let galley = painter.layout_no_wrap(text, font, Color32::WHITE);
    let at = rect.left_top() + egui::vec2(10.0, 10.0);
    painter.rect_filled(egui::Rect::from_min_size(at, galley.size()).expand(4.0), 4.0, PREVIEW.gamma_multiply(0.85));
    painter.galley(at, galley, Color32::WHITE);
}

/// The side panel's section: the sliders, the preview's state, *Apply*, and the ESP curve. `id` is the node of
/// `segment.multiresolution` shown; `view` the map's view (the part in view is what is segmented).
pub fn panel(ui: &mut Ui, s: &mut Session, id: &str, view: &MapView, salt: &str) {
    let Some(n) = node_of(s, id) else { return };
    let node = n.clone();
    let st = state(ui.ctx(), salt, id);
    let mut live = st.lock().unwrap_or_else(|e| e.into_inner());
    let live = &mut *live;
    crate::theme::section(ui, "Live segmentation");
    let Some(image) = input_of(s, &node, "image").as_ref().and_then(Image::of) else {
        ui.label(RichText::new("the node's image has not run (or is on disk: not segmented live)").weak().size(11.0));
        return;
    };
    let base = base_of(s, &node, &image);
    if live.node != id || live.base != base {
        live.reset(id, base);
    }
    let range = |b: f64, v: f64| (b.min(v) / 20.0, b.max(v) * 20.0);
    let (lo, hi) = range(live.base[0], live.scale);
    ui.add(egui::Slider::new(&mut live.scale, lo..=hi).logarithmic(true).text("scale"))
        .on_hover_text("objects merge while their cost is under the scale squared (the image's units)");
    ui.add(egui::Slider::new(&mut live.shape, 0.0..=0.9).text("shape"))
        .on_hover_text("the weight of shape against colour");
    ui.add(egui::Slider::new(&mut live.compactness, 0.0..=1.0).text("compactness"))
        .on_hover_text("in the shape, compactness against smoothness");
    let (scale, shape, compactness) = (live.scale, live.shape, live.compactness);
    if live.touched() {
        // the part in view: asked again when the view, the image or a value changes
        let shown = view.visible();
        if let Some((win, step)) = shown.and_then(|(g, b)| plan(g, b, &image.grid)) {
            let set = settings_of(&node, scale, shape, compactness);
            let key = hashed(|h| {
                (image.key(), win, step).hash(h);
                for x in [scale, shape, compactness] {
                    x.to_bits().hash(h);
                }
                (set.weights.iter().map(|w| w.to_bits()).collect::<Vec<_>>(), set.mutual, set.seed).hash(h);
            });
            if live.asked != Some(key) {
                // (ESP: from the model's segment.scale of this image when it has one, else made here for the window)
                let esp_key = hashed(|h| {
                    (image.key(), win, step, shape.to_bits(), compactness.to_bits(), set.seed).hash(h);
                });
                let from_node = esp_node(s, &node).is_some();
                let need = !from_node && live.esp_asked != Some(esp_key);
                live.ask(Request { key, esp_key: need.then_some(esp_key), image: image.clone(), win, step, set });
            }
        }
    } else if live.preview.is_some() || live.want.is_some() {
        live.stale.store(true, Ordering::Relaxed);
        live.preview = None;
        live.want = None;
        live.asked = None;
    }
    live.poll(ui.ctx());
    if let Some(e) = &live.error {
        ui.label(RichText::new(e).color(ui.visuals().error_fg_color).size(11.0));
    }
    ui.horizontal_wrapped(|ui| {
        if live.busy() {
            ui.spinner();
            ui.label(RichText::new("segmenting the part in view...").weak().size(11.0));
        }
        match (&live.preview, live.touched()) {
            (Some(p), true) => {
                ui.label(RichText::new(p.text()).color(PREVIEW).size(11.0));
            }
            (None, false) => {
                ui.label(RichText::new("move a slider to see the part in view segmented").weak().size(11.0));
            }
            _ => {}
        }
    });
    let mut apply_now = false;
    let mut reset = false;
    ui.horizontal(|ui| {
        apply_now = ui
            .add_enabled(live.touched(), egui::Button::new("Apply"))
            .on_hover_text("write these values into the node (one step of undo, in the Changes)")
            .clicked();
        reset = ui.add_enabled(live.touched(), egui::Button::new("Reset")).clicked();
    });
    // the ESP curve: the model's, else the window's
    let table: Option<Table> = match esp_node(s, &node) {
        Some(Value::Table(t)) => Some(t),
        _ => live.esp().cloned(),
    };
    let mut picked = None;
    if let Some(t) = &table {
        picked = crate::charts::esp_curve(ui, t, egui::Id::new(("live esp", salt, id)));
        let marks = crate::charts::suggested_scales(t);
        if !marks.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("suggested").weak().size(11.0));
                for m in marks {
                    if ui.small_button(short(m)).on_hover_text("use this scale").clicked() {
                        picked = Some(m);
                    }
                }
            });
        }
    }
    if let Some(m) = picked {
        live.scale = m;
    }
    if reset {
        let b = live.base;
        (live.scale, live.shape, live.compactness) = (b[0], b[1], b[2]);
    }
    if apply_now {
        match apply(s, id, live.scale, live.shape, live.compactness) {
            Ok(m) => s.notice = Some((m, Instant::now())),
            Err(e) => live.error = Some(e),
        }
    }
}

/// The table of a `segment.scale` node of the model that reads the image the node reads, when it has run.
fn esp_node(s: &Session, n: &Node) -> Option<Value> {
    let image = n.inputs.get("image")?;
    let other = s.source.nodes.iter().find(|m| m.op == "segment.scale" && m.inputs.get("image") == Some(image))?;
    s.value_of(&other.id, "table").filter(|v| matches!(v, Value::Table(t) if crate::charts::is_esp(t)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_is_the_view_with_a_margin_cut_to_the_image() {
        // 100 x 50 cells in view in the middle of a 1000 x 800 image: ten per cent more each side
        let w = window_cells([400.0, 300.0, 500.0, 350.0], 1000, 800).unwrap();
        assert_eq!(w, Window { x0: 390, y0: 295, width: 120, height: 60 });
        // (a view that spills over the image's corner is cut to it)
        let w = window_cells([-50.0, -50.0, 60.0, 60.0], 1000, 800).unwrap();
        assert_eq!((w.x0, w.y0, w.width, w.height), (0, 0, 71, 71));
        // (a view outside the image has no window)
        assert_eq!(window_cells([2000.0, 0.0, 2100.0, 50.0], 1000, 800), None);
        // (a tiny view still has the least margin)
        let w = window_cells([10.0, 10.0, 12.0, 12.0], 100, 100).unwrap();
        assert_eq!((w.x0, w.y0, w.width, w.height), (6, 6, 10, 10));
    }

    #[test]
    fn a_window_of_more_than_a_million_cells_is_read_every_few() {
        let big = Window { x0: 0, y0: 0, width: 2000, height: 1500 };
        let step = big.step_for(MOST_CELLS);
        assert_eq!(step, 2, "3 million cells: every second, 1000 x 750");
        assert!(big.width.div_ceil(step) * big.height.div_ceil(step) <= MOST_CELLS);
        let screen = Window { x0: 0, y0: 0, width: 800, height: 600 };
        assert_eq!(screen.step_for(MOST_CELLS), 1);
        // (the plan from a view of the image's own grid)
        let g = Grid::new(4000, 3000);
        let (win, step) = plan(&g, [0.0, 0.0, 2000.0, 1500.0], &g).unwrap();
        assert_eq!((win.width, win.height, step), (2200, 1650, 2));
        assert_eq!(ordinal(2), "second");
    }

    #[test]
    fn applying_writes_the_settings_and_undo_puts_them_back() {
        let mut m = terraflux_graph::Model::new("live");
        m.nodes.push(Node::new("land", "nlm.random").param("rows", 10).param("columns", 10).param("seed", 1));
        m.nodes.push(Node::new("seg", OP).input("image", "land.map").param("scale", 5.0));
        let mut s = Session::from_model(m).unwrap();
        let before = s.source.clone();
        let msg = apply(&mut s, "seg", 12.5, 0.3, 0.5).unwrap();
        assert!(msg.contains("scale 12.5") && msg.contains("shape 0.3"), "{msg}");
        let n = s.source.nodes.iter().find(|n| n.id == "seg").unwrap();
        assert_eq!(n.params["scale"], serde_json::json!(12.5));
        assert_eq!(n.params["shape"], serde_json::json!(0.3));
        assert!(!n.params.contains_key("compactness"), "a default the node did not have is not written");
        assert!(s.history.can_undo());
        crate::builder::undo_redo(&mut s, false);
        assert_eq!(s.source, before, "one undo puts the node back");
        assert!(apply(&mut s, "land", 1.0, 0.1, 0.5).is_err(), "only a segment.multiresolution");
    }
}
