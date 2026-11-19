//! Live segmentation in the Viewer: while the shown objects are a segmenter's (`segment.multiresolution`, `slic`,
//! `snic`, `felzenszwalb`, `watershed`, `threshold`, `spectral_difference`, `region_growing`, `quickshift` or
//! `meanshift`), sliders for its main settings (see [`knobs`]) segment the part of the image in view (with a small
//! margin, read every second, third... cell when it has more than [`MOST_CELLS`]) in a thread of its own, and the
//! objects found are drawn over the map as a temporary preview (see [`draw`]). *Apply* writes the values in the node's
//! settings, one step of undo.
//!
//! One worker thread at a time: a new value is kept (the latest only) while it runs and made when it ends, so the
//! sliders never wait and never pile threads up. For the multiresolution the worker segments, then (when the model has
//! no `segment.scale` node of the same image to read it from) runs ESP over the same window for the curve beside the
//! sliders. The other segmenters run through the registry (`segment_window_op`), on the whole image when it has no
//! more cells than they can do in a second (so the preview is the node's own result), else on the part in view.

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Ui};
use terraflux_graph::{Node, Source};
use terraflux_ops::objects::{Objects, Whole, Window, WindowSettings, esp_window, segment_window, segment_window_op};
use terraflux_ops::{Registry, Table, Value};
use terraflux_raster::Grid;

use crate::app::Session;
use crate::map_view::{MapView, ObjectsDraw};

/// The most cells of the window segmented at once: a window with more is read every second, third... cell.
pub const MOST_CELLS: usize = 1_000_000;
/// The same for the segmenters that take seconds on a million cells (region growing, quickshift, mean shift).
pub const SLOW_CELLS: usize = 250_000;
/// The most cells (read) the ESP curve is made on, and its levels.
const ESP_CELLS: usize = 100_000;
const ESP_LEVELS: usize = 20;
/// The margin around the part in view, a share of its side (and at least [`MIN_MARGIN`] cells).
const MARGIN: f32 = 0.1;
const MIN_MARGIN: f32 = 4.0;
/// The colour the preview's outlines are drawn in.
pub const PREVIEW: Color32 = Color32::from_rgb(255, 64, 200);

/// The op the panel's ESP curve is for.
pub const OP: &str = "segment.multiresolution";

/// Where a slider's range comes from.
#[derive(Clone, Copy)]
pub enum Span {
    /// From the first number to the second.
    Fixed(f64, f64),
    /// From a twentieth to twenty times the node's value (the slider's when it goes beyond).
    Around,
    /// From the first number to the second times the range of the first band (a difference in the image's units).
    Band(f64, f64),
}

/// A setting of a segmenter that has a slider.
pub struct Knob {
    pub param: &'static str,
    pub span: Span,
    pub log: bool,
    /// A whole number (the setting is an integer one, written as such).
    pub integer: bool,
    /// What the number counts, shown after it ("cells"; none: the image's units or no unit).
    pub unit: &'static str,
    /// The slider's start when the node does not give the setting.
    pub start: f64,
    /// Whether `start` is the operator's own default (a node without the setting keeps it, so *Apply* does not write it).
    pub op_default: bool,
    /// The node's settings that give the same thing another way, dropped when this one is written.
    pub drops: &'static [&'static str],
    pub about: &'static str,
}

const fn knob(param: &'static str, span: Span, start: f64, about: &'static str) -> Knob {
    Knob { param, span, log: false, integer: false, unit: "", start, op_default: true, drops: &[], about }
}

impl Knob {
    const fn log(mut self) -> Knob {
        self.log = true;
        self
    }

    const fn whole(mut self) -> Knob {
        self.integer = true;
        self
    }

    const fn cells(mut self) -> Knob {
        self.unit = "cells";
        self
    }

    /// The setting has no default of the operator's: it is always written.
    const fn needed(mut self) -> Knob {
        self.op_default = false;
        self
    }

    const fn drops(mut self, d: &'static [&'static str]) -> Knob {
        self.drops = d;
        self
    }

    /// The range of the slider when the node's value is `base`, the slider's `v` and the first band's range `band`.
    fn bounds(&self, base: f64, v: f64, band: f64) -> (f64, f64) {
        let (lo, hi) = match self.span {
            Span::Fixed(a, b) => (a.min(base).min(v), b.max(base).max(v)),
            Span::Around => (base.min(v) / 20.0, base.max(v) * 20.0),
            Span::Band(a, b) => ((a * band).min(base).min(v), (b * band).max(base).max(v)),
        };
        // (a logarithmic slider needs a lowest above 0)
        if self.log && lo <= 0.0 { (hi / 1e4, hi) } else { (lo, hi) }
    }

    /// The value as the node's setting.
    pub(crate) fn json(&self, v: f64) -> serde_json::Value {
        if self.integer { serde_json::json!(v.round() as i64) } else { serde_json::json!(v) }
    }
}

const MULTIRES: [Knob; 3] = [
    knob("scale", Span::Around, 1.0, "objects merge while their cost is under the scale squared (the image's units)")
        .log()
        .needed(),
    knob("shape", Span::Fixed(0.0, 0.9), 0.1, "the weight of shape against colour"),
    knob("compactness", Span::Fixed(0.0, 1.0), 0.5, "in the shape, compactness against smoothness"),
];
const LATTICE: [Knob; 2] = [
    knob("size", Span::Fixed(2.0, 200.0), 10.0, "the approximate side of a superpixel")
        .log()
        .whole()
        .cells()
        .drops(&["count"]),
    knob("compactness", Span::Fixed(0.0, 10.0), 1.0, "the weight of space against colour"),
];
const FELZENSZWALB: [Knob; 2] = [
    knob("k", Span::Around, 1.0, "higher gives larger segments (in standard deviations of the bands)").log(),
    knob("sigma", Span::Fixed(0.0, 5.0), 0.8, "the smoothing's Gaussian").cells(),
];
const WATERSHED: [Knob; 2] = [
    knob("h", Span::Fixed(0.0, 2.0), 0.1, "the least depth of a minimum of the gradient to be a marker").needed(),
    knob("compactness", Span::Fixed(0.0, 0.2), 0.0, "0 is the plain watershed; above it the basins are more regular"),
];
const THRESHOLD: [Knob; 1] = [knob("classes", Span::Fixed(2.0, 8.0), 2.0, "the classes the band is cut into (Otsu)")
    .whole()
    .drops(&["thresholds"])];
const SPECTRAL: [Knob; 1] = [knob(
    "max_difference",
    Span::Band(0.001, 0.5),
    1.0,
    "neighbours merge while their difference is below this (the image's units)",
)
.log()
.needed()];
const GROWING: [Knob; 1] =
    [knob("threshold", Span::Fixed(0.001, 1.0), 0.03, "regions merge while their distance (0 to 1) is at or under it")
        .log()
        .needed()];
const QUICKSHIFT: [Knob; 2] = [
    knob("kernel_size", Span::Fixed(1.0, 30.0), 5.0, "sigma of the density's Gaussian: higher, fewer modes")
        .log()
        .cells(),
    knob("max_dist", Span::Fixed(1.0, 100.0), 10.0, "the longest link: higher, larger segments").log().cells(),
];
const MEANSHIFT: [Knob; 2] = [
    knob("spatial_radius", Span::Fixed(1.0, 20.0), 5.0, "the radius of the filter's window").log().cells(),
    knob("range_radius", Span::Fixed(0.05, 3.0), 0.5, "the distance in the bands within which cells are alike").log(),
];

/// The sliders of a segmenter: its main settings, each with its range. Empty for an operator with no live
/// segmentation (SAM and the `segment.from_*` among them).
pub fn knobs(op: &str) -> &'static [Knob] {
    match op {
        OP => &MULTIRES,
        "segment.slic" | "segment.snic" => &LATTICE,
        "segment.felzenszwalb" => &FELZENSZWALB,
        "segment.watershed" => &WATERSHED,
        "segment.threshold" => &THRESHOLD,
        "segment.spectral_difference" => &SPECTRAL,
        "segment.region_growing" => &GROWING,
        "segment.quickshift" => &QUICKSHIFT,
        "segment.meanshift" => &MEANSHIFT,
        _ => &[],
    }
}

/// The most cells a segmenter does at once for a preview.
fn most_cells(op: &str) -> usize {
    match op {
        "segment.region_growing" | "segment.quickshift" | "segment.meanshift" => SLOW_CELLS,
        _ => MOST_CELLS,
    }
}

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
    plan_most(shown, b, image, MOST_CELLS)
}

/// [`plan`] with no more than `most` cells read.
fn plan_most(shown: &Grid, b: [f32; 4], image: &Grid, most: usize) -> Option<(Window, usize)> {
    let cells = if (shown.width, shown.height) == (image.width, image.height) {
        b
    } else {
        let place = crate::map_view::place_on(image, shown)?;
        let (a, z) = (place.to_source(egui::vec2(b[0], b[1])), place.to_source(egui::vec2(b[2], b[3])));
        [a.x.min(z.x), a.y.min(z.y), a.x.max(z.x), a.y.max(z.y)]
    };
    let win = window_cells(cells, image.width, image.height)?;
    Some((win, win.step_for(most)))
}

/// The plan for the segmenter `op`: the whole image when it has no more cells than the segmenter does at once (the
/// preview is then what the node makes, whatever the view), else the part in view.
pub(crate) fn plan_for(op: &str, view: Option<(&Grid, [f32; 4])>, image: &Grid) -> Option<(Window, usize)> {
    if op == OP {
        return view.and_then(|(g, b)| plan(g, b, image));
    }
    let most = most_cells(op);
    if image.cells() <= most {
        return Some((Window { x0: 0, y0: 0, width: image.width, height: image.height }, 1));
    }
    view.and_then(|(g, b)| plan_most(g, b, image, most))
}

/// What the node segments: its image's bands.
#[derive(Clone)]
pub(crate) struct Image {
    pub(crate) grid: Arc<Grid>,
    pub(crate) names: Vec<String>,
    pub(crate) data: Vec<Arc<Vec<f64>>>,
}

impl Image {
    /// A map or a stack in memory (rasters on disk are not segmented here).
    pub(crate) fn of(v: &Value) -> Option<Image> {
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

    pub(crate) fn key(&self) -> usize {
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

/// The inputs of the node besides its image that the segmenter reads over the cells (objects, markers, seeds,
/// bounds), as the model gives them; an error says which has not run.
fn extras_of(s: &Session, n: &Node) -> Result<Vec<(String, Value)>, String> {
    let mut out = Vec::new();
    for port in ["objects", "markers", "seeds", "bounds"] {
        if n.inputs.contains_key(port) {
            match input_of(s, n, port) {
                Some(v @ (Value::Objects(_) | Value::Map(_))) => out.push((port.to_string(), v)),
                _ => return Err(format!("the input {port} of the node has not run (or is not objects or a map)")),
            }
        }
    }
    Ok(out)
}

/// The identity of an input's cells, to tell a new one from the one asked for.
fn identity(v: &Value) -> usize {
    match v {
        Value::Objects(o) => Arc::as_ptr(o) as usize,
        Value::Map(m) => Arc::as_ptr(&m.data) as usize,
        _ => 0,
    }
}

/// The node `id` of the model's top level when it is a segmenter that has sliders.
fn node_of<'a>(s: &'a Session, id: &str) -> Option<&'a Node> {
    s.source.nodes.iter().find(|n| n.id == id && !knobs(&n.op).is_empty())
}

/// Whether the live segmentation is for the node `id`.
pub fn is_target(s: &Session, id: &str) -> bool {
    node_of(s, id).is_some()
}

/// The settings a node has, one for each of its knobs (the scale of its output when it has none given, and a rough
/// one when it has no output either).
pub(crate) fn base_of(s: &Session, n: &Node, image: &Image) -> Vec<f64> {
    let band = band_range(image);
    knobs(&n.op)
        .iter()
        .map(|k| {
            let given = n.params.get(k.param).and_then(|v| v.as_f64()).filter(|x| x.is_finite());
            if let Some(x) = given {
                return x;
            }
            match (k.param, k.span) {
                ("scale", _) if n.op == OP => match s.value_of(&n.id, "scale") {
                    Some(Value::Number(x)) if x.is_finite() && x > 0.0 => x,
                    _ => rough_scale(image),
                },
                (_, Span::Band(_, _)) => band * 0.05,
                _ => k.start,
            }
        })
        .collect()
}

/// The range of the first band (1 when it has none).
fn band_range(image: &Image) -> f64 {
    let Some(b) = image.data.first() else { return 1.0 };
    let step = b.len().div_ceil(100_000).max(1);
    let (lo, hi) =
        b.iter().step_by(step).filter(|v| v.is_finite()).fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    let r = hi - lo;
    if r.is_finite() && r > 0.0 { r } else { 1.0 }
}

/// A scale to start from when the node has none: a twentieth of the range of the first band.
fn rough_scale(image: &Image) -> f64 {
    band_range(image) / 20.0
}

/// The settings of a segmentation as the node has them (its weights, mutual and seed with the sliders' values).
pub(crate) fn settings_of(n: &Node, values: &[f64]) -> WindowSettings {
    let weights: Vec<f64> = n
        .params
        .get("weights")
        .and_then(|w| w.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
        .unwrap_or_default();
    WindowSettings {
        scale: values[0],
        shape: values[1],
        compactness: values[2],
        weights,
        mutual: n.params.get("mutual").and_then(|v| v.as_bool()).unwrap_or(true),
        seed: n.params.get("seed").and_then(|v| v.as_u64()).unwrap_or(0),
    }
}

/// The node's settings with the sliders' values written over them (and what they replace dropped).
fn params_with(n: &Node, values: &[f64]) -> serde_json::Map<String, serde_json::Value> {
    let mut p = n.params.clone();
    for (k, &v) in knobs(&n.op).iter().zip(values) {
        for d in k.drops {
            p.remove(*d);
        }
        p.insert(k.param.into(), k.json(v));
    }
    p
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

/// What a request has the worker run.
enum Job {
    /// The multiresolution, then (with a key) the curve of ESP over the same window.
    Multires { esp_key: Option<u64>, set: WindowSettings },
    /// Any other segmenter through the registry: the node's settings with the sliders' values, the inputs besides the
    /// image (cut to the window), and what the whole image gives the window (made once, by the first job).
    Op {
        op: String,
        params: serde_json::Map<String, serde_json::Value>,
        extra: Vec<(String, Value)>,
        whole: Arc<OnceLock<Whole>>,
    },
}

/// A request to the worker.
struct Request {
    key: u64,
    image: Image,
    win: Window,
    step: usize,
    job: Job,
}

enum Msg {
    Seg(Result<Preview, String>),
    Esp(u64, Result<Table, String>),
}

/// The operators of the registry (made once).
pub(crate) fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(Registry::standard)
}

/// What the Viewer keeps of the live segmentation of a node: the sliders, the worker and what it made.
#[derive(Default)]
pub struct Live {
    pub node: String,
    /// The node's operator (the sliders are its knobs').
    pub op: String,
    /// The node's own values (the sliders' start), set when the node or its settings change.
    base: Vec<f64>,
    /// The sliders' values, one for each knob of the operator.
    pub values: Vec<f64>,
    worker: Option<Receiver<Msg>>,
    /// Told to the worker that its result will not be wanted (it leaves out what it has not made yet).
    stale: Arc<AtomicBool>,
    want: Option<Request>,
    asked: Option<u64>,
    pub preview: Option<Preview>,
    esp: Option<(u64, Table)>,
    esp_asked: Option<u64>,
    /// What the whole image gives a window, for the image with this key.
    whole: Option<(usize, Arc<OnceLock<Whole>>)>,
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
        self.values != self.base
    }

    /// Whether the worker is at work or has work waiting.
    pub fn busy(&self) -> bool {
        self.worker.is_some() || self.want.is_some()
    }

    /// The node's values taken as the sliders' (the node was edited, or is another).
    fn reset(&mut self, node: &str, op: &str, base: Vec<f64>) {
        self.stale.store(true, Ordering::Relaxed);
        *self = Live { node: node.to_string(), op: op.to_string(), values: base.clone(), base, ..Live::default() };
    }

    fn ask(&mut self, req: Request) {
        self.asked = Some(req.key);
        self.want = Some(req);
    }

    /// What the whole image gives a window, shared by the jobs on this image.
    fn whole_of(&mut self, image: &Image) -> Arc<OnceLock<Whole>> {
        match &self.whole {
            Some((k, w)) if *k == image.key() => w.clone(),
            _ => {
                let w = Arc::new(OnceLock::new());
                self.whole = Some((image.key(), w.clone()));
                w
            }
        }
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
        if let Job::Multires { esp_key: Some(k), .. } = &req.job {
            self.esp_asked = Some(*k);
        }
        let spawned =
            std::thread::Builder::new().name("live segmentation".into()).stack_size(16 << 20).spawn(move || {
                let Request { image, win, step, job, .. } = req;
                let t = Instant::now();
                let (made, esp) = match job {
                    Job::Multires { esp_key, set } => {
                        let made = segment_window(&image.grid, &image.data, win, step, &set)
                            .map(Arc::new)
                            .map_err(|e| e.to_string());
                        (made, esp_key.map(|k| (k, set)))
                    }
                    Job::Op { op, params, extra, whole } => {
                        let whole = whole.get_or_init(|| Whole::of(&image.data));
                        let made = segment_window_op(
                            registry(),
                            &op,
                            &params,
                            &image.grid,
                            &image.data,
                            &image.names,
                            whole,
                            win,
                            step,
                            &extra,
                        )
                        .map_err(|e| e.to_string());
                        (made, None)
                    }
                };
                let secs = t.elapsed().as_secs_f64();
                let seg = made.map(|objects| Preview { objects, win, step, secs });
                if tx.send(Msg::Seg(seg)).is_err() {
                    return;
                }
                if let Some((k, set)) = esp
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

pub(crate) fn hashed(parts: impl FnOnce(&mut std::hash::DefaultHasher)) -> u64 {
    let mut h = std::hash::DefaultHasher::new();
    parts(&mut h);
    h.finish()
}

/// *Apply*: the values (one for each knob of the node's operator) written into the node's settings, as one step of
/// undo (named by them). The node must be in the model's top level.
pub fn apply(s: &mut Session, id: &str, values: &[f64]) -> Result<String, String> {
    let Some(n) = s.source.nodes.iter_mut().find(|n| n.id == id && !knobs(&n.op).is_empty()) else {
        return Err(format!("{id} is not a segmenter of the model's top level with sliders"));
    };
    let ks = knobs(&n.op);
    if values.len() != ks.len() {
        return Err(format!("{id}: {} values for {} settings", values.len(), ks.len()));
    }
    for (k, &v) in ks.iter().zip(values) {
        // (a default the node did not have is not written)
        if !n.params.contains_key(k.param) && k.op_default && v == k.start {
            continue;
        }
        for d in k.drops {
            n.params.remove(*d);
        }
        n.params.insert(k.param.into(), k.json(v));
    }
    let said: Vec<String> = ks.iter().zip(values).map(|(k, &v)| format!("{} {}", k.param, short(v))).collect();
    let msg = format!("{id}: {}", said.join(", "));
    s.history.name(&format!("Segmentation of {id}: {}", said[0]));
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
    let said: Vec<String> =
        knobs(&live.op).iter().zip(&live.values).map(|(k, &v)| format!("{} {}", k.param, short(v))).collect();
    let text = format!("PREVIEW (not applied): {}", said.join(", "));
    let font = egui::FontId::proportional(12.0);
    let galley = painter.layout_no_wrap(text, font, Color32::WHITE);
    let at = rect.left_top() + egui::vec2(10.0, 10.0);
    painter.rect_filled(egui::Rect::from_min_size(at, galley.size()).expand(4.0), 4.0, PREVIEW.gamma_multiply(0.85));
    painter.galley(at, galley, Color32::WHITE);
}

/// The side panel's section: the sliders, the preview's state, *Apply*, and (for the multiresolution) the ESP curve.
/// `id` is the node of the segmenter shown; `view` the map's view (the part in view is what is segmented).
pub fn panel(ui: &mut Ui, s: &mut Session, id: &str, view: &MapView, salt: &str) {
    let Some(n) = node_of(s, id) else { return };
    let node = n.clone();
    let ks = knobs(&node.op);
    let st = state(ui.ctx(), salt, id);
    let mut live = st.lock().unwrap_or_else(|e| e.into_inner());
    let live = &mut *live;
    crate::theme::section(ui, "Live segmentation");
    let Some(image) = input_of(s, &node, "image").as_ref().and_then(Image::of) else {
        ui.label(RichText::new("the node's image has not run (or is on disk: not segmented live)").weak().size(11.0));
        return;
    };
    let base = base_of(s, &node, &image);
    if live.node != id || live.op != node.op || live.base != base {
        live.reset(id, &node.op, base);
    }
    let band = band_range(&image);
    for (k, knob) in ks.iter().enumerate() {
        let (lo, hi) = knob.bounds(live.base[k], live.values[k], band);
        let mut slider = egui::Slider::new(&mut live.values[k], lo..=hi).text(knob.param);
        if knob.log {
            slider = slider.logarithmic(true);
        }
        if knob.integer {
            slider = slider.integer();
        }
        if !knob.unit.is_empty() {
            slider = slider.suffix(format!(" {}", knob.unit));
        }
        ui.add(slider).on_hover_text(knob.about);
    }
    if live.touched() {
        // the part in view (or the whole image): asked again when the view, the image or a value changes
        let planned = plan_for(&node.op, view.visible(), &image.grid);
        let extra = if node.op == OP { Ok(Vec::new()) } else { extras_of(s, &node) };
        if let Err(e) = &extra {
            live.error = Some(e.clone());
        }
        if let (Some((win, step)), Ok(extra)) = (planned, extra) {
            let values = live.values.clone();
            let params = params_with(&node, &values);
            let key = hashed(|h| {
                (image.key(), win, step, &node.op).hash(h);
                for x in &values {
                    x.to_bits().hash(h);
                }
                serde_json::to_string(&params).unwrap_or_default().hash(h);
                for (port, v) in &extra {
                    (port, identity(v)).hash(h);
                }
            });
            if live.asked != Some(key) {
                let job = if node.op == OP {
                    let set = settings_of(&node, &values);
                    // (ESP: from the model's segment.scale of this image when it has one, else made here for the window)
                    let esp_key = hashed(|h| {
                        (image.key(), win, step, set.shape.to_bits(), set.compactness.to_bits(), set.seed).hash(h);
                    });
                    let need = esp_node(s, &node).is_none() && live.esp_asked != Some(esp_key);
                    Job::Multires { esp_key: need.then_some(esp_key), set }
                } else {
                    Job::Op { op: node.op.clone(), params, extra, whole: live.whole_of(&image) }
                };
                live.ask(Request { key, image: image.clone(), win, step, job });
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
    crate::segment_compare::section(ui, s, &node, &image, view, salt);
    let mut apply_now = false;
    let mut reset = false;
    ui.horizontal(|ui| {
        apply_now = ui
            .add_enabled(live.touched(), egui::Button::new("Apply"))
            .on_hover_text("write these values into the node (one step of undo, in the Changes)")
            .clicked();
        reset = ui.add_enabled(live.touched(), egui::Button::new("Reset")).clicked();
    });
    if node.op == OP {
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
            live.values[0] = m;
        }
    }
    if reset {
        live.values = live.base.clone();
    }
    if apply_now {
        match apply(s, id, &live.values) {
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
    fn a_small_image_is_segmented_whole_and_a_big_one_by_the_part_in_view() {
        let small = Grid::new(400, 300);
        let (win, step) = plan_for("segment.slic", None, &small).unwrap();
        assert_eq!((win, step), (Window { x0: 0, y0: 0, width: 400, height: 300 }, 1), "no view needed");
        // (the slow ones have fewer cells: 600 x 500 is a million's third)
        let mid = Grid::new(600, 500);
        assert_eq!(plan_for("segment.slic", None, &mid).unwrap().1, 1);
        assert!(plan_for("segment.quickshift", None, &mid).is_none(), "the part in view: no view, no plan");
        let (win, step) = plan_for("segment.quickshift", Some((&mid, [0.0, 0.0, 100.0, 100.0])), &mid).unwrap();
        assert_eq!((win.width, win.height, step), (110, 110, 1));
        // (the multiresolution keeps its way: always the part in view)
        assert!(plan_for(OP, None, &small).is_none());
    }

    #[test]
    fn every_segmenter_has_sliders_that_are_its_settings() {
        let registry = Registry::standard();
        // what segment.optimize varies by default for each segmenter of an image
        let main = [
            ("segment.multiresolution", "scale"),
            ("segment.region_growing", "threshold"),
            ("segment.slic", "size"),
            ("segment.snic", "size"),
            ("segment.felzenszwalb", "k"),
            ("segment.watershed", "h"),
            ("segment.quickshift", "kernel_size"),
            ("segment.meanshift", "range_radius"),
            ("segment.spectral_difference", "max_difference"),
        ];
        for (op, param) in main {
            assert!(knobs(op).iter().any(|k| k.param == param), "{op} has a slider for {param}");
        }
        assert!(knobs("segment.threshold").iter().any(|k| k.param == "classes"));
        for op in main.iter().map(|m| m.0).chain(["segment.threshold"]) {
            let sig = registry.sig(op).unwrap();
            for k in knobs(op) {
                let p = sig.params.iter().find(|p| p.name == k.param);
                let p = p.unwrap_or_else(|| panic!("{op} has no setting {}", k.param));
                assert_eq!(k.integer, p.ty == terraflux_graph::Type::Integer, "{op} {}: whole numbers", k.param);
                let (lo, hi) = k.bounds(k.start, k.start, 10.0);
                assert!(lo < hi && lo.is_finite() && hi.is_finite(), "{op} {}: {lo} to {hi}", k.param);
                assert!(!k.log || lo > 0.0, "{op} {}: a log slider starts above 0", k.param);
                for d in k.drops {
                    assert!(sig.params.iter().any(|p| p.name == *d), "{op} drops {d}");
                }
            }
            if op != OP {
                assert!(terraflux_ops::objects::window_op(op), "{op} runs on a window");
            }
        }
        // (no sliders for SAM, the conversions and the segmenters that need more)
        for op in ["segment.sam", "segment.from_map", "segment.optimize", "nlm.random"] {
            assert!(knobs(op).is_empty(), "{op}");
        }
    }

    #[test]
    fn applying_writes_the_settings_and_undo_puts_them_back() {
        let mut m = terraflux_graph::Model::new("live");
        m.nodes.push(Node::new("land", "nlm.random").param("rows", 10).param("columns", 10).param("seed", 1));
        m.nodes.push(Node::new("seg", OP).input("image", "land.map").param("scale", 5.0));
        let mut s = Session::from_model(m).unwrap();
        let before = s.source.clone();
        let msg = apply(&mut s, "seg", &[12.5, 0.3, 0.5]).unwrap();
        assert!(msg.contains("scale 12.5") && msg.contains("shape 0.3"), "{msg}");
        let n = s.source.nodes.iter().find(|n| n.id == "seg").unwrap();
        assert_eq!(n.params["scale"], serde_json::json!(12.5));
        assert_eq!(n.params["shape"], serde_json::json!(0.3));
        assert!(!n.params.contains_key("compactness"), "a default the node did not have is not written");
        assert!(s.history.can_undo());
        crate::builder::undo_redo(&mut s, false);
        assert_eq!(s.source, before, "one undo puts the node back");
        assert!(apply(&mut s, "land", &[1.0]).is_err(), "only a segmenter with sliders");
        assert!(apply(&mut s, "seg", &[1.0]).is_err(), "one value for each slider");
    }

    #[test]
    fn applying_to_slic_writes_a_whole_size_in_one_step_and_drops_the_count() {
        let mut m = terraflux_graph::Model::new("live");
        m.nodes.push(Node::new("land", "nlm.random").param("rows", 10).param("columns", 10).param("seed", 1));
        m.nodes.push(Node::new("seg", "segment.slic").input("image", "land.map").param("count", 50));
        let mut s = Session::from_model(m).unwrap();
        let before = s.source.clone();
        // (size from a slider is a float: written as a whole number; the compactness at its default is left out)
        apply(&mut s, "seg", &[7.0000001, 1.0]).unwrap();
        let n = s.source.nodes.iter().find(|n| n.id == "seg").unwrap();
        assert_eq!(n.params["size"], serde_json::json!(7));
        assert!(n.params["size"].is_i64() || n.params["size"].is_u64(), "an integer: {:?}", n.params["size"]);
        assert!(!n.params.contains_key("count") && !n.params.contains_key("compactness"));
        // (and the node runs: the settings are valid)
        let p = params_with(n, &[7.2, 2.5]);
        assert_eq!((&p["size"], &p["compactness"]), (&serde_json::json!(7), &serde_json::json!(2.5)));
        crate::builder::undo_redo(&mut s, false);
        assert_eq!(s.source, before, "one undo puts the node back");
        // a threshold given its thresholds loses them for the classes
        let mut n = Node::new("t", "segment.threshold").input("image", "land.map");
        n.params.insert("thresholds".into(), serde_json::json!([0.5]));
        assert!(!params_with(&n, &[3.0]).contains_key("thresholds"));
    }
}
