//! Compare segmenters side by side: a window opened from the live segmentation's panel that segments the same window of
//! the image (the whole image when small, else the part in view, as [`live_segment::plan_for`] decides) with each
//! segmenter chosen, in thumbnails with the objects' outlines over the image, the number of objects and the seconds.
//! A segmenter runs with the settings of a node of its operator on the same image when the model has one, else with the
//! sliders' defaults (see [`live_segment::knobs`]). *Use* puts the segmenter in place of the one shown: what read the
//! shown node's objects reads the new one's (the old node stays, with no readers), as one step of undo.
//!
//! One worker thread at a time runs the segmenters that have no thumbnail yet one after another; when the window, the
//! image or the settings change, it is told to stop (it leaves out what it has not made) and the missing ones are asked
//! again when it ends, so the window never waits.

use std::collections::{BTreeSet, HashMap};
use std::hash::Hash;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use egui::{Color32, ColorImage, RichText, TextureHandle, TextureOptions, Ui};
use terraflux_graph::{Node, Source};
use terraflux_ops::Registry;
use terraflux_ops::objects::{Objects, Whole, Window, WindowSettings, segment_window, segment_window_op};

use crate::app::Session;
use crate::live_segment::{self, Image, PREVIEW, knobs};
use crate::map_view::MapView;

type Params = serde_json::Map<String, serde_json::Value>;

/// The segmenters that can be compared, in the order they are shown, and whether each is chosen at the start.
pub const OPS: [(&str, bool); 8] = [
    ("segment.multiresolution", true),
    ("segment.slic", true),
    ("segment.watershed", true),
    ("segment.felzenszwalb", true),
    ("segment.threshold", false),
    ("segment.quickshift", false),
    ("segment.region_growing", false),
    ("segment.meanshift", false),
];

/// The longest side of a thumbnail in pixels (an image of fewer cells is enlarged to it).
const SIDE: usize = 230;

/// What the worker runs for one segmenter.
#[derive(Clone)]
struct Item {
    op: String,
    key: u64,
    params: Params,
    /// The multiresolution's own settings (it does not run through the registry).
    set: Option<WindowSettings>,
}

/// A segmenter's result: its outlines over the image, the objects and the seconds.
struct Thumb {
    image: ColorImage,
    count: usize,
    secs: f64,
    tex: Option<TextureHandle>,
}

struct Msg {
    op: String,
    key: u64,
    result: Result<Thumb, String>,
}

/// What the Viewer keeps of the comparison of a node's segmentation: the choice, the worker and what it made.
pub struct Compare {
    pub open: bool,
    /// The segmenters chosen.
    pub chosen: BTreeSet<String>,
    items: Vec<Item>,
    sig: u64,
    results: HashMap<String, (u64, Result<Thumb, String>)>,
    inflight: Vec<(String, u64)>,
    worker: Option<Receiver<Msg>>,
    stale: Arc<AtomicBool>,
    whole: Option<(usize, Arc<OnceLock<Whole>>)>,
    pub error: Option<String>,
    /// How many worker threads were started (a test counts them).
    pub started: usize,
}

impl Default for Compare {
    fn default() -> Self {
        Compare {
            open: false,
            chosen: OPS.iter().filter(|o| o.1).map(|o| o.0.to_string()).collect(),
            items: Vec::new(),
            sig: 0,
            results: HashMap::new(),
            inflight: Vec::new(),
            worker: None,
            stale: Arc::default(),
            whole: None,
            error: None,
            started: 0,
        }
    }
}

/// The comparison of a node's segmentation (kept in the context, by the view's name and the node).
pub fn state(ctx: &egui::Context, salt: &str, node: &str) -> Arc<Mutex<Compare>> {
    let id = egui::Id::new(("segment comparison", salt, node));
    ctx.data_mut(|d| d.get_temp_mut_or_insert_with::<Arc<Mutex<Compare>>>(id, Default::default).clone())
}

/// The node of the model's top level that gives the settings of `op` on the image `image`, when there is one.
fn existing<'a>(s: &'a Session, op: &str, image: Option<&Source>) -> Option<&'a Node> {
    s.source.nodes.iter().find(|n| n.op == op && n.inputs.get("image") == image)
}

/// The settings `op` runs with: those of a node of the operator on the same image, else the sliders' defaults (the
/// settings with no default of the operator's are always given, from the image).
pub(crate) fn params_for(s: &Session, op: &str, image_ref: Option<&Source>, image: &Image) -> Params {
    let node = existing(s, op, image_ref);
    let temp = node.cloned().unwrap_or_else(|| Node::new("compare", op));
    let base = live_segment::base_of(s, &temp, image);
    let mut p = temp.params.clone();
    for (k, &v) in knobs(op).iter().zip(&base) {
        if !p.contains_key(k.param) && (node.is_none() || !k.op_default) {
            p.insert(k.param.into(), k.json(v));
        }
    }
    p
}

/// The multiresolution's settings from the node's settings.
fn multires_set(params: &Params, image: &Image, s: &Session) -> WindowSettings {
    let mut n = Node::new("compare", live_segment::OP);
    n.params = params.clone();
    let values: Vec<f64> = knobs(live_segment::OP)
        .iter()
        .map(|k| n.params.get(k.param).and_then(|v| v.as_f64()).unwrap_or(k.start))
        .collect();
    let _ = (image, s);
    live_segment::settings_of(&n, &values)
}

/// What to run for the segmenters chosen, over the window `win` read every `step`-th cell.
fn items_for(
    s: &Session,
    chosen: &BTreeSet<String>,
    image_ref: Option<&Source>,
    image: &Image,
    win: Window,
    step: usize,
) -> Vec<Item> {
    OPS.iter()
        .filter(|(op, _)| chosen.contains(*op))
        .map(|(op, _)| {
            let params = params_for(s, op, image_ref, image);
            let set = (*op == live_segment::OP).then(|| multires_set(&params, image, s));
            let key = live_segment::hashed(|h| {
                (image.key(), win, step, op).hash(h);
                serde_json::to_string(&params).unwrap_or_default().hash(h);
                format!("{set:?}").hash(h);
            });
            Item { op: op.to_string(), key, params, set }
        })
        .collect()
}

/// Which of everything the items depend on changed.
fn signature(
    s: &Session,
    chosen: &BTreeSet<String>,
    image_ref: Option<&Source>,
    image: &Image,
    win: Window,
    step: usize,
) -> u64 {
    live_segment::hashed(|h| {
        (image.key(), win, step, chosen).hash(h);
        for (op, _) in OPS {
            if let Some(n) = existing(s, op, image_ref) {
                (&n.id, serde_json::to_string(&n.params).unwrap_or_default()).hash(h);
            }
        }
    })
}

/// A thumbnail: the window's cells (every `step`-th) as the image (grey, or the first three bands stretched from the
/// 2nd to the 98th percentile) with the outlines of `o` over it.
fn thumbnail(image: &Image, win: Window, step: usize, o: &Objects) -> ColorImage {
    let (gw, gh) = (o.grid.width.max(1), o.grid.height.max(1));
    let f = SIDE as f32 / gw.max(gh) as f32;
    let (pw, ph) = (((gw as f32 * f).round() as usize).max(1), ((gh as f32 * f).round() as usize).max(1));
    let cols: Vec<i32> = (0..pw).map(|x| (((x as f32 + 0.5) / f) as usize).min(gw - 1) as i32).collect();
    let rows: Vec<i32> = (0..ph).map(|y| (((y as f32 + 0.5) / f) as usize).min(gh - 1) as i32).collect();
    // the band's value at each cell of the window's grid
    let at = |b: &[f64], gx: usize, gy: usize| {
        let (x, y) = ((win.x0 + gx * step).min(image.grid.width - 1), (win.y0 + gy * step).min(image.grid.height - 1));
        b[y * image.grid.width + x]
    };
    let bands = if image.data.len() >= 3 { 3 } else { 1 };
    let stretched: Vec<Vec<u8>> = image.data[..bands]
        .iter()
        .map(|b| {
            let v: Vec<f64> =
                (0..gh).flat_map(|gy| (0..gw).map(move |gx| (gx, gy))).map(|(gx, gy)| at(b, gx, gy)).collect();
            let (lo, hi) = crate::palette::percentiles_2_98(&v).filter(|(l, h)| h > l).unwrap_or((0.0, 1.0));
            v.iter().map(|&x| (((x - lo) / (hi - lo)).clamp(0.0, 1.0) * 255.0).round() as u8).collect()
        })
        .collect();
    let mut img = crate::objects_view::outline_image(&o.labels, gw, (&cols, &rows), PREVIEW, 1.0, 1.0);
    for (y, &r) in rows.iter().enumerate() {
        for (x, &c) in cols.iter().enumerate() {
            let cell = r as usize * gw + c as usize;
            let grey = |k: usize| stretched[if bands == 3 { k } else { 0 }][cell];
            let p = img.pixels[y * pw + x];
            let a = u32::from(p.a());
            let mix = |base: u8, over: u8| (u32::from(base) * (255 - a) / 255 + u32::from(over)).min(255) as u8;
            img.pixels[y * pw + x] = Color32::from_rgb(mix(grey(0), p.r()), mix(grey(1), p.g()), mix(grey(2), p.b()));
        }
    }
    img
}

impl Compare {
    /// How many segmenters have a thumbnail for what is asked now.
    pub fn ready(&self) -> usize {
        self.items.iter().filter(|i| self.valid(i) && self.results[&i.op].1.is_ok()).count()
    }

    /// The objects found by a segmenter, for what is asked now.
    pub fn count(&self, op: &str) -> Option<usize> {
        let i = self.items.iter().find(|i| i.op == op)?;
        match self.results.get(op) {
            Some((k, Ok(t))) if *k == i.key => Some(t.count),
            _ => None,
        }
    }

    /// Whether the worker is at work.
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }

    fn valid(&self, i: &Item) -> bool {
        self.results.get(&i.op).is_some_and(|(k, _)| *k == i.key)
    }

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

    /// Take what the worker made; stop it when it works for what is not asked now; start it for what is missing.
    fn poll(&mut self, ctx: &egui::Context, image: &Image, win: Window, step: usize) {
        let mut ended = false;
        if let Some(rx) = &self.worker {
            loop {
                match rx.try_recv() {
                    Ok(m) => {
                        self.inflight.retain(|(o, _)| *o != m.op);
                        self.results.insert(m.op, (m.key, m.result));
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        ended = true;
                        break;
                    }
                }
            }
        }
        if ended {
            self.worker = None;
            self.inflight.clear();
        }
        if self.inflight.iter().any(|f| !self.items.iter().any(|i| (&i.op, i.key) == (&f.0, f.1))) {
            self.stale.store(true, Ordering::Relaxed);
        }
        let missing: Vec<Item> = self
            .items
            .iter()
            .filter(|i| !self.valid(i) && !self.inflight.iter().any(|f| (&f.0, f.1) == (&i.op, i.key)))
            .cloned()
            .collect();
        if self.worker.is_none() && !missing.is_empty() {
            self.spawn(missing, image, win, step);
        }
        if self.worker.is_some() {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }

    fn spawn(&mut self, items: Vec<Item>, image: &Image, win: Window, step: usize) {
        let (tx, rx) = mpsc::channel();
        self.stale = Arc::new(AtomicBool::new(false));
        let stale = self.stale.clone();
        let whole = self.whole_of(image);
        let image = image.clone();
        let inflight: Vec<(String, u64)> = items.iter().map(|i| (i.op.clone(), i.key)).collect();
        let spawned =
            std::thread::Builder::new().name("segment comparison".into()).stack_size(16 << 20).spawn(move || {
                for item in items {
                    if stale.load(Ordering::Relaxed) {
                        return;
                    }
                    let t = Instant::now();
                    let made = match &item.set {
                        Some(set) => segment_window(&image.grid, &image.data, win, step, set).map(Arc::new),
                        None => {
                            let whole = whole.get_or_init(|| Whole::of(&image.data));
                            segment_window_op(
                                live_segment::registry(),
                                &item.op,
                                &item.params,
                                &image.grid,
                                &image.data,
                                &image.names,
                                whole,
                                win,
                                step,
                                &[],
                            )
                        }
                    };
                    let secs = t.elapsed().as_secs_f64();
                    let result = made.map_err(|e| e.to_string()).map(|o| Thumb {
                        image: thumbnail(&image, win, step, &o),
                        count: o.count,
                        secs,
                        tex: None,
                    });
                    if tx.send(Msg { op: item.op, key: item.key, result }).is_err() {
                        return;
                    }
                }
            });
        match spawned {
            Ok(_) => {
                self.worker = Some(rx);
                self.inflight = inflight;
                self.started += 1;
            }
            Err(e) => self.error = Some(format!("no thread for the comparison: {e}")),
        }
    }
}

/// *Use*: the segmenter `op` in place of the node `shown` (a segmenter of the model's top level): the nodes that read
/// the shown node's objects read the objects of a node of `op` on the same image (the one the model has, else a new
/// node connected to the image, given `params`). The shown node stays, with no readers. One step of undo (named by
/// them); the model is put back on an error.
pub fn use_op(s: &mut Session, r: &Registry, shown: &str, op: &str, params: &Params) -> Result<String, String> {
    let before = s.source.clone();
    match write(s, r, shown, op, params) {
        Ok((msg, id)) => {
            s.history.name(&format!("Use {op} for {shown}: {id}"));
            s.edited();
            Ok(msg)
        }
        Err(e) => {
            s.source = before;
            Err(e)
        }
    }
}

fn write(s: &mut Session, r: &Registry, shown: &str, op: &str, params: &Params) -> Result<(String, String), String> {
    let top: &[String] = &[];
    let node = s
        .source
        .nodes
        .iter()
        .find(|n| n.id == shown && !knobs(&n.op).is_empty())
        .cloned()
        .ok_or_else(|| format!("{shown} is not a segmenter of the model's top level"))?;
    if node.op == op {
        return Err(format!("{shown} is already a {op}"));
    }
    let Some(Source::Ref(image)) = node.inputs.get("image").cloned() else {
        return Err(format!("the image of {shown} is not a node's output: no node can read it"));
    };
    let found = s
        .source
        .nodes
        .iter()
        .find(|n| n.op == op && n.id != shown && n.inputs.get("image") == Some(&Source::Ref(image.clone())))
        .map(|n| n.id.clone());
    let (id, made) = match found {
        Some(id) => (id, false),
        None => {
            let at = crate::builder::beside(&s.source.nodes, Some(shown));
            let name = op.strip_prefix("segment.").unwrap_or(op);
            let id = crate::editor::add_node_named(&mut s.source, r, top, op, Some(name), at)?;
            crate::editor::connect(&mut s.source, r, top, &image, &id, "image")?;
            if let Some(n) = s.source.nodes.iter_mut().find(|n| n.id == id) {
                for (k, v) in params {
                    n.params.insert(k.clone(), v.clone());
                }
            }
            (id, true)
        }
    };
    let (from, to) = (format!("{shown}.objects"), format!("{id}.objects"));
    let readers: Vec<(String, String)> = s
        .source
        .nodes
        .iter()
        .filter(|n| n.id != id)
        .flat_map(|n| {
            n.inputs
                .iter()
                .filter(|(_, src)| matches!(src, Source::Ref(x) if *x == from))
                .map(|(port, _)| (n.id.clone(), port.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    for (reader, port) in &readers {
        crate::editor::connect(&mut s.source, r, top, &to, reader, port)?;
    }
    let msg = format!(
        "{id}{} in place of {shown}: {}",
        if made { " (a new node)" } else { "" },
        match readers.len() {
            0 => "nothing read it".to_string(),
            1 => "1 node reads it now".to_string(),
            n => format!("{n} nodes read it now"),
        }
    );
    Ok((msg, id))
}

/// The *Compare...* button of the live segmentation's panel and the window it opens. `node` is the segmenter shown,
/// `image` the image it reads, `view` the map's view.
pub(crate) fn section(ui: &mut Ui, s: &mut Session, node: &Node, image: &Image, view: &MapView, salt: &str) {
    let st = state(ui.ctx(), salt, &node.id);
    let mut c = st.lock().unwrap_or_else(|e| e.into_inner());
    let c = &mut *c;
    ui.toggle_value(&mut c.open, "Compare...")
        .on_hover_text("the same window of the image segmented by several segmenters, side by side");
    if c.open {
        window(ui.ctx(), s, node, image, view, c);
    }
}

fn window(ctx: &egui::Context, s: &mut Session, node: &Node, image: &Image, view: &MapView, c: &mut Compare) {
    let image_ref = node.inputs.get("image").cloned();
    // (every segmenter on the same window: the one the slowest does at once)
    let planned = live_segment::plan_for("segment.quickshift", view.visible(), &image.grid);
    match planned {
        Some((win, step)) => {
            let sig = signature(s, &c.chosen, image_ref.as_ref(), image, win, step);
            if sig != c.sig || c.items.is_empty() != c.chosen.is_empty() {
                c.sig = sig;
                c.items = items_for(s, &c.chosen, image_ref.as_ref(), image, win, step);
            }
            c.poll(ctx, image, win, step);
        }
        None => c.items.clear(),
    }
    let mut open = true;
    let mut chosen_now = None;
    egui::Window::new("Compare segmenters")
        .id(egui::Id::new(("segment comparison window", &node.id)))
        .open(&mut open)
        .default_width(4.0 * (SIDE as f32 + 16.0))
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (op, _) in OPS {
                    let mut on = c.chosen.contains(op);
                    let name = op.strip_prefix("segment.").unwrap_or(op);
                    if ui.checkbox(&mut on, name).changed() {
                        if on {
                            c.chosen.insert(op.to_string());
                        } else {
                            c.chosen.remove(op);
                        }
                    }
                }
            });
            if planned.is_none() {
                ui.label(RichText::new("nothing of the image is in view").weak().size(11.0));
            } else if let Some((win, step)) = planned {
                let every = if step > 1 { format!(", every {} cell", step) } else { String::new() };
                ui.label(
                    RichText::new(format!("window: {} x {} cells{every}; settings of the model's nodes, else the defaults", win.width, win.height))
                        .weak()
                        .size(11.0),
                );
            }
            if let Some(e) = &c.error {
                ui.label(RichText::new(e).color(ui.visuals().error_fg_color).size(11.0));
            }
            ui.horizontal_wrapped(|ui| {
                let ops: Vec<(String, u64)> = c.items.iter().map(|i| (i.op.clone(), i.key)).collect();
                for (op, key) in ops {
                    ui.vertical(|ui| {
                        ui.set_width(SIDE as f32);
                        let name = op.strip_prefix("segment.").unwrap_or(&op).to_string();
                        ui.label(RichText::new(&name).strong());
                        match c.results.get_mut(&op) {
                            Some((k, Ok(t))) if *k == key => {
                                let tex = t.tex.get_or_insert_with(|| {
                                    ctx.load_texture(format!("compare {op}"), t.image.clone(), TextureOptions::NEAREST)
                                });
                                let [w, h] = t.image.size;
                                ui.image((tex.id(), egui::vec2(w as f32, h as f32)));
                                ui.label(RichText::new(format!("{} objects, {:.2} s", t.count, t.secs)).size(11.0));
                                if op == node.op {
                                    ui.label(RichText::new("shown").weak().size(11.0));
                                } else if ui
                                    .button("Use")
                                    .on_hover_text(format!(
                                        "{name} in place of {}: what reads its objects reads this one's (one step of undo)",
                                        node.id
                                    ))
                                    .clicked()
                                {
                                    chosen_now = Some(op.clone());
                                }
                            }
                            Some((k, Err(e))) if *k == key => {
                                ui.label(RichText::new(e.as_str()).color(ui.visuals().error_fg_color).size(11.0));
                            }
                            _ => {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(RichText::new("segmenting...").weak().size(11.0));
                                });
                            }
                        }
                    });
                }
            });
        });
    c.open = open;
    if let Some(op) = chosen_now {
        let params = c.items.iter().find(|i| i.op == op).map(|i| i.params.clone()).unwrap_or_default();
        match use_op(s, live_segment::registry(), &node.id, &op, &params) {
            Ok(m) => {
                c.error = None;
                s.notice = Some((m, Instant::now()));
            }
            Err(e) => c.error = Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_graph::Model;

    fn session() -> Session {
        session_with(false)
    }

    fn session_with(slic: bool) -> Session {
        let mut m = Model::new("compare");
        m.nodes.push(Node::new("land", "nlm.random").param("rows", 10).param("columns", 10).param("seed", 1));
        m.nodes.push(Node::new("fine", "segment.multiresolution").input("image", "land.map").param("scale", 0.5));
        m.nodes.push(Node::new("rules", "objects.rules").input("objects", "fine.objects"));
        m.nodes.push(Node::new("more", "objects.rules").input("objects", "fine.objects"));
        m.nodes.push(Node::new("tidy", "objects.rules").input("objects", "rules.objects"));
        if slic {
            m.nodes.push(Node::new("slic", "segment.slic").input("image", "land.map").param("size", 7));
        }
        Session::from_model(m).unwrap()
    }

    fn node<'a>(s: &'a Session, id: &str) -> &'a Node {
        s.source.nodes.iter().find(|n| n.id == id).unwrap()
    }

    fn image() -> Image {
        let g = terraflux_raster::Grid::new(10, 10);
        let data: Vec<f64> = (0..100).map(|i| (i % 10) as f64).collect();
        Image { grid: Arc::new(g), names: vec!["image".into()], data: vec![Arc::new(data)] }
    }

    #[test]
    fn every_op_has_defaults_that_are_its_sliders() {
        let s = session();
        let img = image();
        let none = Some(Source::Ref("other.map".into()));
        for (op, _) in OPS {
            assert!(!knobs(op).is_empty(), "{op}");
            let p = params_for(&s, op, none.as_ref(), &img);
            for k in knobs(op) {
                assert!(p.contains_key(k.param), "{op}: {} is given", k.param);
            }
        }
        let p = params_for(&s, "segment.slic", none.as_ref(), &img);
        assert_eq!(p["size"], serde_json::json!(10));
        let p = params_for(&s, "segment.watershed", none.as_ref(), &img);
        assert_eq!(p["h"], serde_json::json!(0.1));
        // (the multiresolution's scale: a twentieth of the band's range)
        let p = params_for(&s, live_segment::OP, none.as_ref(), &img);
        assert!((p["scale"].as_f64().unwrap() - 9.0 / 20.0).abs() < 1e-9, "{}", p["scale"]);
    }

    #[test]
    fn a_node_of_the_operator_on_the_same_image_gives_the_settings() {
        let s = session();
        let img = image();
        let image_ref = Some(Source::Ref("land.map".into()));
        let p = params_for(&s, live_segment::OP, image_ref.as_ref(), &img);
        assert_eq!(p["scale"], serde_json::json!(0.5), "the node's own");
        assert!(!p.contains_key("shape"), "what the node does not give is left to the operator");
        let set = multires_set(&p, &img, &s);
        assert_eq!((set.scale, set.shape), (0.5, 0.1));
    }

    #[test]
    fn using_a_new_segmenter_rewires_the_readers_and_undo_puts_them_back() {
        let r = Registry::standard();
        let mut s = session();
        let before = s.source.clone();
        let img = image();
        let params = params_for(&s, "segment.slic", Some(&Source::Ref("land.map".into())), &img);
        let msg = use_op(&mut s, &r, "fine", "segment.slic", &params).unwrap();
        assert!(msg.contains("a new node") && msg.contains("2 nodes"), "{msg}");
        let slic = node(&s, "slic");
        assert_eq!(slic.op, "segment.slic");
        assert_eq!(slic.inputs.get("image"), Some(&Source::Ref("land.map".into())));
        assert_eq!(slic.params["size"], serde_json::json!(10));
        for id in ["rules", "more"] {
            assert_eq!(node(&s, id).inputs.get("objects"), Some(&Source::Ref("slic.objects".into())), "{id}");
        }
        assert_eq!(node(&s, "tidy").inputs.get("objects"), Some(&Source::Ref("rules.objects".into())), "not a reader");
        assert!(node(&s, "fine").params.contains_key("scale"), "the old node stays");
        assert!(s.history.can_undo());
        crate::builder::undo_redo(&mut s, false);
        assert_eq!(s.source, before, "one undo puts everything back");
        // (the shown one is not put in its own place; an image that is no node's output has no node to read it)
        assert!(use_op(&mut s, &r, "fine", live_segment::OP, &Params::new()).is_err());
        assert_eq!(s.source, before);
    }

    #[test]
    fn using_a_segmenter_the_model_has_reuses_it() {
        let r = Registry::standard();
        let mut s = session_with(true);
        let before = s.source.clone();
        use_op(&mut s, &r, "fine", "segment.slic", &Params::new()).unwrap();
        assert_eq!(s.source.nodes.len(), before.nodes.len(), "no new node");
        assert_eq!(node(&s, "slic").params["size"], serde_json::json!(7), "its own settings");
        assert_eq!(node(&s, "rules").inputs.get("objects"), Some(&Source::Ref("slic.objects".into())));
        crate::builder::undo_redo(&mut s, false);
        assert_eq!(s.source, before);
    }

    #[test]
    fn a_thumbnail_is_the_image_with_the_outlines_over_it() {
        let img = image();
        let win = Window { x0: 0, y0: 0, width: 10, height: 10 };
        let labels: Vec<u32> = (0..100).map(|i| if i % 10 < 5 { 1 } else { 2 }).collect();
        let o = Objects::from_labels(img.grid.clone(), labels, 2).unwrap();
        let t = thumbnail(&img, win, 1, &o);
        assert_eq!(t.size, [SIDE, SIDE]);
        assert!(t.pixels.contains(&PREVIEW), "the outline is drawn");
        assert!(t.pixels.iter().any(|p| p.r() == p.g() && p.g() == p.b()), "the image is grey under it");
    }
}
