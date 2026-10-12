//! The model builder: the operators to add (a library by subject, with a search), the canvas of the graph, the
//! node selected in an inspector, and under the canvas what a node gives (a map, the maps of a stack or of a
//! loop's steps, a table, a value).
//!
//! A model is run as it is built: a run stops at the node shown and takes what did not change from the cache
//! (ctrl+enter, or by itself after every edit), so that changing a setting shows its result at once. What
//! takes long is never done in a frame: the colours and areas of a map are worked out in a thread, the maps
//! coloured in another.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{Margin, RichText, Ui, Vec2};
use terraflux_graph::{Node, Type};
use terraflux_ops::landscape::Product;
use terraflux_ops::{Map, Registry, Value};

use crate::app::{Look, Session, accent_button, node_panel};
use crate::editor_view::{Action, NodeStatus};
use crate::map_view::{Hover, Source, SourceKey, class_counts, legend};
use crate::palette::{Coloring, Ramp};
use crate::theme;

/// The subjects of the library: landschaft's (the sections of its reference, in its order), then the
/// model's own (map algebra, the flow of a model); with the operators' prefixes in each and, within a subject,
/// the groups of its operators.
const SUBJECTS: [(&str, &[&str]); 20] = [
    ("Data & configuration", &["raster", "vector"]),
    ("Habitat patterns", &["landscape", "patterns"]),
    ("Fragmentation", &["fragmentation"]),
    ("Landscape metrics", &["metrics"]),
    ("Habitat networks", &["networks"]),
    ("Circuit connectivity", &["circuit"]),
    ("Surface metrics", &["surface"]),
    ("Grid distances", &["cost"]),
    ("Distance & restoration", &["distance", "restoration"]),
    ("Change detection", &["change"]),
    ("Large data & batch", &["batch"]),
    ("Neutral landscape models", &["nlm"]),
    ("Biodiversity analysis", &["bio"]),
    ("Spatial prioritization", &["prioritize"]),
    ("Species distributions", &["sdm"]),
    (
        "Land change simulation",
        &[
            "markov", "woe", "patches", "dinamica", "glm", "lulcc", "demand", "plus", "cars", "dynamic", "agents",
            "crafty", "validate",
        ],
    ),
    ("Uncertainty", &["uncertainty"]),
    ("Map algebra", &["map", "stack"]),
    ("Visualization", &["view"]),
    ("Model flow", &["control", "rng", "plugin"]),
];

/// The groups of the operators of a subject, by prefix (land change simulation's models).
fn group(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "markov" | "woe" | "patches" | "dinamica" => "Dinamica EGO",
        "glm" | "lulcc" | "demand" => "lulcc and CLUE-S",
        "plus" | "cars" => "PLUS",
        "dynamic" => "Dynamic variables",
        "agents" | "crafty" => "Agents",
        "validate" => "Validation",
        _ => return None,
    })
}

/// An operator of the library.
pub struct Entry {
    pub name: String,
    pub doc: String,
    pub subject: &'static str,
    pub group: Option<&'static str>,
}

/// The operators by subject, made once.
pub struct Library {
    pub entries: Vec<Entry>,
}

impl Library {
    pub fn new(r: &Registry) -> Library {
        let mut entries: Vec<Entry> = r
            .signatures()
            .into_iter()
            .map(|g| {
                let prefix = g.name.split('.').next().unwrap_or("");
                let subject = SUBJECTS.iter().find(|(_, p)| p.contains(&prefix)).map_or("Other", |(s, _)| s);
                Entry { group: group(prefix), name: g.name, doc: g.doc, subject }
            })
            .collect();
        let rank = |s: &str| SUBJECTS.iter().position(|(x, _)| *x == s).unwrap_or(SUBJECTS.len());
        entries.sort_by(|a, b| (rank(a.subject), a.group, &a.name).cmp(&(rank(b.subject), b.group, &b.name)));
        Library { entries }
    }

    /// The operators a search finds, the best first: every word of it in the name or in what the operator does;
    /// those whose name starts with it, then whose name has it, before those whose description has it.
    pub fn find(&self, query: &str) -> Vec<&Entry> {
        let query = query.trim().to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let mut found: Vec<(u8, &Entry)> = self
            .entries
            .iter()
            .filter_map(|e| {
                let (name, doc) = (e.name.to_lowercase(), e.doc.to_lowercase());
                let short = name.split_once('.').map_or(name.as_str(), |(_, b)| b);
                if !words.iter().all(|w| name.contains(w) || doc.contains(w)) {
                    return None;
                }
                let rank = if short.starts_with(&query) || name.starts_with(&query) {
                    0
                } else if name.contains(&query) {
                    1
                } else if words.iter().all(|w| name.contains(w)) {
                    2
                } else {
                    3
                };
                Some((rank, e))
            })
            .collect();
        // (the library's order within a rank)
        found.sort_by_key(|(rank, _)| *rank);
        found.into_iter().map(|(_, e)| e).collect()
    }
}

/// The search of the operators from the keyboard: "/" opens it, typing searches, the arrows choose, enter adds
/// the operator chosen (connected to the node selected, as a click in the library), escape closes it.
#[derive(Debug, Default)]
pub struct Quick {
    pub query: String,
    /// The operator chosen among those found.
    pub at: usize,
    /// Whether the search was given the keyboard's focus.
    focused: bool,
}

/// The search of the operators from the keyboard, over the panes, while it is open.
pub fn quick_add(ctx: &egui::Context, s: &mut Session, r: &Registry, lib: &Library) {
    let Some(mut q) = s.editor.quick.take() else { return };
    let found = lib.find(&q.query);
    let (down, up, enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if down {
        q.at = (q.at + 1).min(found.len().saturating_sub(1));
    }
    if up {
        q.at = q.at.saturating_sub(1);
    }
    let mut chosen = (enter && !found.is_empty()).then(|| found[q.at.min(found.len() - 1)].name.clone());
    let mut close = escape || chosen.is_some();
    let screen = ctx.content_rect();
    let width = 520.0_f32.min(screen.width() - 40.0);
    let area = egui::Area::new(egui::Id::new("quick add"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(screen.center().x - width / 2.0, screen.top() + 90.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(Margin::same(10)).show(ui, |ui| {
                ui.set_width(width);
                let to = s.selected.as_ref().map_or("a new node".to_string(), |id| format!("after {id}"));
                ui.label(RichText::new(format!("Add an operator · {to}")).strong());
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut q.query)
                        .hint_text("its name or what it does (mspa, reclassify, distance...)")
                        .desired_width(f32::INFINITY),
                );
                if !q.focused {
                    edit.request_focus();
                    q.focused = true;
                }
                if edit.changed() {
                    q.at = 0;
                }
                ui.add_space(4.0);
                if found.is_empty() {
                    ui.label(RichText::new("no operator has it").weak());
                }
                let at = q.at.min(found.len().saturating_sub(1));
                egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                    for (k, e) in found.iter().enumerate() {
                        let short = e.name.split_once('.').map_or(e.name.as_str(), |(_, b)| b);
                        let resp = ui.add(
                            egui::Button::selectable(k == at, RichText::new(short).strong())
                                .right_text(RichText::new(&e.name).size(10.5).color(ui.visuals().weak_text_color()))
                                .min_size(Vec2::new(ui.available_width(), 22.0)),
                        );
                        if k == at && (up || down) {
                            resp.scroll_to_me(None);
                        }
                        if resp.on_hover_text(&e.doc).clicked() {
                            chosen = Some(e.name.clone());
                            close = true;
                        }
                    }
                });
                // what the operator chosen does
                if let Some(e) = found.get(at) {
                    ui.separator();
                    ui.label(RichText::new(&e.doc).size(11.5).color(ui.visuals().weak_text_color()));
                }
                ui.add_space(2.0);
                ui.label(
                    RichText::new("up, down: choose · enter: add · esc: close")
                        .size(10.5)
                        .color(ui.visuals().weak_text_color()),
                );
            });
        });
    // a click elsewhere closes it
    if ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer() {
        close = true;
    }
    if let Some(op) = chosen
        && let Err(e) = add(s, r, &op, &[])
    {
        s.editor.message = Some(e);
    }
    if close {
        ctx.memory_mut(|m| {
            if let Some(f) = m.focused() {
                m.surrender_focus(f);
            }
        });
    } else {
        s.editor.quick = Some(q);
    }
}

/// What the values shown are, as far as their colours go (as landschaft's plot tells): from the operator that
/// made them (through the plots that show them) and the names of the nodes, files and bands they come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    /// By the values: classes in order, or a ramp (centred on 0 for values on both sides of it).
    Auto,
    /// What GuidosToolbox gives (MSPA's classes, FAD's densities...) in its colours, with the names of its values
    /// (when the values are the product's).
    Product(Product),
    /// Elevations: a ramp from lowlands to peaks.
    Elevation,
    /// A hillshade: black to white.
    Shade,
    /// NDVI, EVI: red to green.
    Vegetation,
    /// A priority rank (Zonation's): dark to red.
    Rank,
}

impl Palette {
    /// The palette of what an operator gives (`made`, when it is a product of GuidosToolbox), `names` those of the
    /// nodes, files and bands it comes from.
    pub fn of(made: Option<Product>, names: &str) -> Palette {
        let names = names.to_lowercase();
        // whole words of the names (so that "demand" is no "dem")
        let words: Vec<&str> = names.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
        let has = |w: &str| words.contains(&w);
        let starts = |w: &str| words.iter().any(|x| x.starts_with(w));
        if let Some(p) = made {
            Palette::Product(p)
        } else if starts("mspa") {
            Palette::Product(Product::Mspa)
        // GTB's names of its files: x_fad_27.tif, x_fos_fad_6class.tif, x_spa5.tif, x_acc.tif
        } else if starts("fad") || has("fos") {
            Palette::Product(Product::Density { five: has("5class") })
        } else if has("spa") || ["spa2", "spa3", "spa5", "spa6"].iter().any(|w| has(w)) {
            Palette::Product(Product::Spa)
        } else if has("acc") || has("accounting") {
            Palette::Product(Product::Accounting)
        } else if has("rank") || starts("priorit") {
            Palette::Rank
        } else if starts("hillshade") || has("shade") {
            Palette::Shade
        } else if has("dem")
            || has("dtm")
            || has("dsm")
            || starts("elev")
            || has("height")
            || starts("altitud")
            || has("srtm")
            || has("topography")
        {
            Palette::Elevation
        } else if has("ndvi") || has("evi") || has("savi") {
            Palette::Vegetation
        } else {
            Palette::Auto
        }
    }

    pub fn look(self, values: &[f64]) -> Look {
        let auto = Look::of(values, None);
        let ramp = |r| Look { coloring: auto.coloring.clone().with_ramp(r), ..auto.clone() };
        match self {
            Palette::Auto => auto,
            Palette::Elevation => ramp(Ramp::Terrain),
            Palette::Shade => ramp(Ramp::Grey),
            Palette::Vegetation => ramp(Ramp::Vegetation),
            Palette::Rank => ramp(Ramp::Rank),
            Palette::Product(p) => {
                let counts = class_counts(values);
                // a file named so that is not the product's values: by its values
                let colours: Option<Vec<(f64, egui::Color32)>> = counts
                    .iter()
                    .map(|(v, _)| p.colour(*v).map(|[r, g, b]| (*v, egui::Color32::from_rgb(r, g, b))))
                    .collect();
                match colours {
                    Some(c) if !c.is_empty() => Look { coloring: Coloring::Classes(c), counts, product: Some(p) },
                    _ => auto,
                }
            }
        }
    }
}

/// The name of a value in a legend: its name in the product shown, or the value.
pub fn class_name(look: &Look, v: f64) -> String {
    match look.product.and_then(|p| p.name(v)) {
        Some(n) => n,
        None => terraflux_ops::value::class_label(v),
    }
}

/// What a node gives at a port, through the plots that show it (they give what they are given): the product of
/// GuidosToolbox it is, if one, and the ids of the nodes on the way.
fn producer(s: &Session, node: &str, port: &str) -> (Option<Product>, String) {
    let (mut id, mut port) = (node.to_string(), port.to_string());
    let mut names = String::new();
    for _ in 0..32 {
        let Some(n) = s.model.nodes.iter().find(|n| n.id == id) else { break };
        names.push_str(&n.id);
        names.push(' ');
        let from = match n.inputs.get("value") {
            Some(terraflux_graph::Source::Ref(r)) if n.op == "view.plot" => terraflux_graph::Ref::parse(r).ok(),
            _ => None,
        };
        match from {
            Some(terraflux_graph::Ref::Port { node, port: p }) => (id, port) = (node, p),
            _ => return (Product::of(&n.op, &port, &n.params), names),
        }
    }
    (None, names)
}

type Worked = (SourceKey, Palette, Look);
/// A source's key, the source (held so that its key stays its own), its palette, its look once worked out and
/// when it was last asked for.
type Kept = (SourceKey, Source, Palette, Option<Look>, u64);

/// The cells a look of rasters on disk is worked out from (a sample of the whole, as a thumbnail).
const LOOK_SAMPLE: usize = 1024;

/// The look of a source: of all its cells (a map in memory), or of a sample of about a million cells (rasters
/// on disk, of any size), its counts scaled to the whole.
fn look_of(source: &Source, palette: Palette) -> Look {
    match source {
        Source::Map(m) => palette.look(&m.data),
        Source::Rasters(r, l) => {
            let (w, h) = (r.grid_of(*l).width, r.grid_of(*l).height);
            let step = w.max(h).div_ceil(LOOK_SAMPLE).max(1);
            let win = terraflux_raster::tiff::Window { x0: 0, y0: 0, width: w, height: h };
            let values: Vec<f64> =
                r.sample(*l, win, step).map(|v| v.into_iter().map(f64::from).collect()).unwrap_or_default();
            let mut look = palette.look(&values);
            for (_, n) in look.counts.iter_mut() {
                *n *= step * step;
            }
            look
        }
    }
}

/// The looks of the maps shown, each worked out once in a thread.
#[derive(Default)]
pub struct Looks {
    kept: Vec<Kept>,
    channel: Option<(Sender<Worked>, Receiver<Worked>)>,
    clock: u64,
}

impl Looks {
    /// The look of a source worked out already (of any palette), without asking for one.
    pub fn peek(&self, source: &Source) -> Option<&Look> {
        let key = source.key();
        self.kept.iter().find(|e| e.0 == key && e.3.is_some()).and_then(|e| e.3.as_ref())
    }

    /// The look of a source, or none while it is worked out.
    pub fn get(&mut self, ctx: &egui::Context, source: &Source, palette: Palette) -> Option<Look> {
        let (tx, rx) = self.channel.get_or_insert_with(channel);
        while let Ok((key, p, look)) = rx.try_recv() {
            if let Some(e) = self.kept.iter_mut().find(|e| e.0 == key && e.2 == p) {
                e.3 = Some(look);
            }
        }
        self.clock += 1;
        let key = source.key();
        if let Some(e) = self.kept.iter_mut().find(|e| e.0 == key && e.2 == palette) {
            e.4 = self.clock;
            return e.3.clone();
        }
        let (tx, src, c) = (tx.clone(), source.clone(), ctx.clone());
        std::thread::spawn(move || {
            let look = look_of(&src, palette);
            let _ = tx.send((key, palette, look));
            c.request_repaint();
        });
        self.kept.push((key, source.clone(), palette, None, self.clock));
        // the sources are held while their looks are: keep a few
        while self.kept.len() > 24 {
            let Some(i) = (0..self.kept.len()).filter(|&i| self.kept[i].3.is_some()).min_by_key(|&i| self.kept[i].4)
            else {
                break;
            };
            self.kept.swap_remove(i);
        }
        None
    }
}

// The panes of the builder, each in a tab of the dock (see `app::Pane`).

/// The operators' library.
pub fn operators_pane(ui: &mut Ui, s: &mut Session, r: &Registry, lib: &Library) {
    egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| library(ui, s, r, lib));
}

/// The node selected: what it does, its inputs and settings.
pub fn node_pane(ui: &mut Ui, s: &mut Session, r: &Registry) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| node_panel(ui, s, r));
    });
}

/// The model's graph: where in it, run up to the node, and the canvas.
pub fn model_pane(ui: &mut Ui, s: &mut Session, r: &Registry) {
    egui::Panel::top("builder_bar")
        .frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::symmetric(8, 5)))
        .show(ui, |ui| bar(ui, s, r));
    egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::CANVAS)).show(ui, |ui| canvas(ui, s, r));
}

/// What the node shown gives; the cell under the pointer.
pub fn viewer_pane(ui: &mut Ui, s: &mut Session, r: &Registry) -> Option<Hover> {
    preview(ui, s, r)
}

/// Run up to a node (none: the whole model).
fn run_to(ui: &Ui, s: &mut Session, target: Option<String>) {
    let ctx = ui.ctx().clone();
    s.start_to(target, Box::new(move || ctx.request_repaint()));
}

/// The selected node, when it is a top-level node (those run on their own).
fn runnable(s: &Session) -> Option<String> {
    s.selected.clone().filter(|id| s.editor.path.is_empty() && s.model.nodes.iter().any(|n| &n.id == id))
}

/// ctrl+enter runs up to the node selected, del deletes the nodes selected, ctrl+L arranges the nodes, "/"
/// searches the operators to add one.
pub fn keys(ui: &Ui, s: &mut Session, r: &Registry) {
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    let (run, delete, tidy, search) = ui.input_mut(|i| {
        // "/" as typed, whatever the keyboard's layout puts it on (taken away, so that it is not typed in the
        // search it opens)
        let slash = !typing && i.events.iter().any(|e| matches!(e, egui::Event::Text(t) if t == "/"));
        if slash {
            i.events.retain(|e| !matches!(e, egui::Event::Text(t) if t == "/"));
        }
        (
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter),
            !typing && i.consume_key(egui::Modifiers::NONE, egui::Key::Delete),
            !typing && i.consume_key(egui::Modifiers::COMMAND, egui::Key::L),
            slash,
        )
    });
    if search && s.editor.quick.is_none() {
        s.editor.quick = Some(Quick::default());
    }
    if run && s.can_run_to() {
        let target = runnable(s);
        if target.is_some() {
            run_to(ui, s, target);
        }
    }
    if tidy {
        arrange(s, r);
    }
    if delete {
        let mut gone: Vec<String> = s.editor.chosen.iter().cloned().collect();
        if gone.is_empty() {
            gone.extend(s.selected.clone());
        }
        let path = s.editor.path.clone();
        let mut removed = false;
        for id in &gone {
            match crate::editor::remove_node(&mut s.source, &path, id) {
                Ok(()) => removed = true,
                Err(e) => s.editor.message = Some(e),
            }
        }
        if removed {
            if s.selected.as_ref().is_some_and(|x| gone.contains(x)) {
                s.selected = None;
            }
            if s.preview.as_ref().is_some_and(|(n, _)| gone.contains(n)) {
                s.preview = None;
            }
            s.editor.chosen.clear();
            s.edited();
        }
    }
}

/// The output of a node to show first: a map, a stack or a series, else the first.
fn first_output(n: &Node, r: &Registry) -> Option<String> {
    let outs = crate::editor::output_ports(n, r);
    outs.iter()
        .find(|(_, t)| matches!(t, Type::Raster | Type::Map | Type::Stack | Type::Series))
        .or(outs.first())
        .map(|(p, _)| p.clone())
}

/// Select a node and show its first output.
pub fn select(s: &mut Session, r: &Registry, id: &str) {
    if s.selected.as_deref() != Some(id) {
        s.fields.clear();
    }
    s.selected = Some(id.to_string());
    if s.editor.path.is_empty()
        && let Some(n) = s.source.nodes.iter().find(|n| n.id == id)
        && let Some(port) = first_output(n, r)
    {
        if s.preview.as_ref().is_none_or(|(p, _)| p != id) {
            s.preview_band = 0;
            s.preview_chosen = false;
        }
        s.preview = Some((id.to_string(), port));
    }
}

/// Add a node of an operator next to the one selected, its first input fed by the selected node's first output
/// that fits; it becomes the node selected.
pub fn add(s: &mut Session, r: &Registry, op: &str, params: &[(&str, serde_json::Value)]) -> Result<String, String> {
    let path = s.editor.path.clone();
    let nodes = s.source.scope(&path).cloned().unwrap_or_default();
    let auto = crate::editor_view::auto_positions(&nodes);
    let at = |n: &Node| n.pos.unwrap_or(auto[&n.id]);
    let from = s.selected.as_ref().and_then(|id| nodes.iter().find(|n| &n.id == id)).cloned();
    let pos = match &from {
        Some(n) => {
            let p = at(n);
            let right = [p[0] + crate::editor_view::WIDTH + 70.0, p[1]];
            // below what is there already
            let taken =
                |q: [f32; 2]| nodes.iter().any(|m| (at(m)[0] - q[0]).abs() < 100.0 && (at(m)[1] - q[1]).abs() < 90.0);
            let mut q = right;
            while taken(q) {
                q[1] += 110.0;
            }
            q
        }
        None => [0.0, nodes.iter().map(|n| at(n)[1] + 130.0).fold(0.0, f32::max)],
    };
    let id = crate::editor::add_node(&mut s.source, r, &path, op, pos)?;
    if let Some(n) = s.source.scope(&path).and_then(|v| v.iter().find(|n| n.id == id)).cloned() {
        for (k, v) in params {
            crate::editor::set_param(
                &mut s.source,
                &path,
                &id,
                k,
                &match v {
                    serde_json::Value::String(t) => t.clone(),
                    v => v.to_string(),
                },
            )?;
        }
        if let Some(f) = &from {
            let outs = crate::editor::output_ports(f, r);
            if let Some((port, ty, _)) = crate::editor::input_ports(&n, r).into_iter().find(|(_, _, req)| *req)
                && let Some((out, _)) = outs.iter().find(|(_, t)| t.fits(ty))
            {
                let _ = crate::editor::connect(&mut s.source, r, &path, &format!("{}.{out}", f.id), &id, &port);
            }
        }
    }
    s.edited();
    select(s, r, &id);
    Ok(id)
}

fn library(ui: &mut Ui, s: &mut Session, r: &Registry, lib: &Library) {
    theme::section(ui, "Data");
    let wide = Vec2::new(ui.available_width(), 24.0);
    if ui.add(accent_button("Open a raster...", true).min_size(wide)).clicked()
        && let Some(p) = rfd::FileDialog::new()
            .set_title("A raster")
            .add_filter("Rasters", &terraflux_ops::files::RASTER_EXTENSIONS)
            .add_filter("Every file", &["*"])
            .pick_file()
    {
        let path = p.to_string_lossy().into_owned();
        if let Err(e) = add(s, r, "raster.read", &[("path", path.into())]) {
            s.editor.message = Some(e);
        }
    }
    if ui
        .add(egui::Button::new("Open vectors...").min_size(wide))
        .on_hover_text("vector features (GeoPackage, Shapefile, GeoJSON...), read by GDAL")
        .clicked()
        && let Some(p) = rfd::FileDialog::new()
            .set_title("Vector features")
            .add_filter("Vectors", &["gpkg", "shp", "geojson", "json", "fgb", "kml", "gml"])
            .add_filter("Every file", &["*"])
            .pick_file()
    {
        let path = p.to_string_lossy().into_owned();
        if let Err(e) = add(s, r, "vector.read", &[("path", path.into())]) {
            s.editor.message = Some(e);
        }
    }
    if ui
        .add(egui::Button::new("Open a folder as a stack...").min_size(wide))
        .on_hover_text(
            "every raster of a folder as one stack: a layer per band of each file, by the files' names (of one grid \
             to be a stack; else shown one by one)",
        )
        .clicked()
        && let Some(p) = rfd::FileDialog::new().set_title("A folder of rasters").pick_folder()
    {
        let path = p.to_string_lossy().into_owned();
        if let Err(e) = add(s, r, "raster.read", &[("path", path.into())]) {
            s.editor.message = Some(e);
        }
    }
    let can_save = s.selected.is_some();
    if ui
        .add_enabled(can_save, egui::Button::new("Save as a raster...").min_size(wide))
        .on_hover_text("write what the node selected gives to a GeoTIFF file")
        .on_disabled_hover_text("select the node whose map to save")
        .clicked()
        && let Some(p) = rfd::FileDialog::new()
            .set_title("Save the map")
            .add_filter("GeoTIFF", &["tif"])
            .set_file_name("map.tif")
            .save_file()
    {
        let path = p.to_string_lossy().into_owned();
        if let Err(e) = add(s, r, "raster.write", &[("path", path.into())]) {
            s.editor.message = Some(e);
        }
    }
    // which GDAL reads the other formats and does the warping, if any
    let gdal = match terraflux_gdal::gdal() {
        Ok(g) => RichText::new(format!("GDAL {} · {} formats", g.version, g.drivers)).weak(),
        Err(_) => RichText::new("no GDAL: GeoTIFF only").color(ui.visuals().warn_fg_color),
    };
    ui.label(gdal.size(10.5)).on_hover_text(match terraflux_gdal::gdal() {
        Ok(g) => g.library.display().to_string(),
        Err(e) => e.to_string(),
    });
    ui.add_space(6.0);
    theme::section(ui, "Operators");
    ui.add(
        egui::TextEdit::singleline(&mut s.editor.search)
            .hint_text("search (or \"/\" anywhere)")
            .desired_width(f32::INFINITY),
    );
    ui.add_space(2.0);
    let mut chosen = None;
    let query = s.editor.search.trim().to_lowercase();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let entry = |ui: &mut Ui, e: &Entry, chosen: &mut Option<String>| {
            // the name in one line, its prefix beside it (the whole name is both)
            let (prefix, short) = e.name.split_once('.').unwrap_or(("", e.name.as_str()));
            let resp = ui.add(
                egui::Button::selectable(false, short)
                    .wrap_mode(egui::TextWrapMode::Extend)
                    .right_text(RichText::new(prefix).size(10.5).color(ui.visuals().weak_text_color()))
                    .min_size(Vec2::new(ui.available_width(), 20.0)),
            );
            if resp.on_hover_text(&e.doc).clicked() {
                *chosen = Some(e.name.clone());
            }
        };
        if query.is_empty() {
            for (subject, _) in SUBJECTS.iter().chain([("Other", &[][..])].iter()) {
                let in_it: Vec<&Entry> = lib.entries.iter().filter(|e| e.subject == *subject).collect();
                if in_it.is_empty() {
                    // a subject of landschaft not here yet: its name, to see where things will be
                    if *subject != "Other" {
                        ui.add_enabled(
                            false,
                            egui::Button::new(RichText::new(format!("    {subject}")).size(12.5)).frame(false),
                        );
                    }
                    continue;
                }
                egui::CollapsingHeader::new(RichText::new(*subject).strong())
                    .default_open(matches!(
                        *subject,
                        "Data & configuration" | "Habitat patterns" | "Fragmentation" | "Visualization"
                    ))
                    .show(ui, |ui| {
                        // the operators of no group, then each group a level below, closed until opened
                        for e in in_it.iter().filter(|e| e.group.is_none()) {
                            entry(ui, e, &mut chosen);
                        }
                        let mut groups: Vec<&str> = in_it.iter().filter_map(|e| e.group).collect();
                        groups.dedup();
                        for g in groups {
                            egui::CollapsingHeader::new(g).id_salt((*subject, g)).default_open(false).show(ui, |ui| {
                                for e in in_it.iter().filter(|e| e.group == Some(g)) {
                                    entry(ui, e, &mut chosen);
                                }
                            });
                        }
                    });
            }
        } else {
            for e in lib.find(&query) {
                entry(ui, e, &mut chosen);
            }
        }
    });
    if let Some(op) = chosen
        && let Err(e) = add(s, r, &op, &[])
    {
        s.editor.message = Some(e);
    }
}

/// A node's output in a map tab of its own, run first when it has not run yet.
pub fn open_map(ui: &Ui, s: &mut Session, r: &Registry, id: &str) {
    if !s.editor.path.is_empty() {
        return;
    }
    let Some(n) = s.source.nodes.iter().find(|n| n.id == id) else { return };
    let Some(port) = first_output(n, r) else { return };
    if s.value_of(id, &port).is_none() && s.can_run_to() {
        run_to(ui, s, Some(id.to_string()));
    }
    s.open_map(id, &port);
}

/// Put the nodes of the scope shown in tidy places, and look at them all.
pub fn arrange(s: &mut Session, r: &Registry) {
    let path = s.editor.path.clone();
    let Some(nodes) = s.source.scope(&path).cloned() else { return };
    for (id, p) in crate::editor_view::arrange(&nodes, r) {
        let _ = crate::editor::move_node(&mut s.source, &path, &id, p);
    }
    s.dirty = true;
    s.revision += 1;
    s.editor.fit();
}

/// The bar above the canvas: where in the model, run up to the node, the preview kept up to date.
fn bar(ui: &mut Ui, s: &mut Session, r: &Registry) {
    ui.horizontal(|ui| {
        if ui.selectable_label(s.editor.path.is_empty(), "Top level").clicked() {
            s.editor.path.clear();
            s.selected = None;
        }
        for k in 0..s.editor.path.len() {
            ui.label("/");
            if ui.selectable_label(k + 1 == s.editor.path.len(), &s.editor.path[k]).clicked() {
                s.editor.path.truncate(k + 1);
                s.selected = None;
            }
        }
        ui.separator();
        let target = runnable(s);
        let can = target.is_some() && s.can_run_to();
        if ui
            .add_enabled(can, accent_button("Run to here", can))
            .on_hover_text(
                "run the node selected and what it needs; what did not change comes from the cache (ctrl+enter)",
            )
            .clicked()
        {
            run_to(ui, s, target);
        }
        if ui.button("Arrange").on_hover_text("put the nodes in tidy columns, by what they read (ctrl+L)").clicked() {
            arrange(s, r);
        }
        ui.checkbox(&mut s.auto, "update after every edit")
            .on_hover_text("run up to the node shown after every change of the model");
        if let Some(m) = &s.editor.message {
            ui.colored_label(ui.visuals().warn_fg_color, m);
        }
    });
}

fn canvas(ui: &mut Ui, s: &mut Session, r: &Registry) {
    let shown = s.preview.clone();
    let actions = crate::editor_view::show(
        ui,
        &s.source,
        r,
        &mut s.editor,
        s.selected.as_deref(),
        &s.status,
        s.revision,
        s.status_rev,
        shown.as_ref().map(|(n, p)| (n.as_str(), p.as_str())),
    );
    if s.source.nodes.is_empty() && s.editor.path.is_empty() {
        ui.painter().text(
            ui.max_rect().center(),
            egui::Align2::CENTER_CENTER,
            "Open a raster, click an operator of the library or press / to start; or begin from a template (Templates)",
            egui::FontId::proportional(15.0),
            ui.visuals().weak_text_color(),
        );
    }
    for a in actions {
        let path = s.editor.path.clone();
        let result = match a {
            Action::Select(id) => {
                select(s, r, &id);
                Ok(false)
            }
            Action::Preview(id, port) => {
                s.selected = Some(id.clone());
                if s.editor.path.is_empty() {
                    if s.preview.as_ref().is_none_or(|(n, p)| *n != id || *p != port) {
                        s.preview_band = 0;
                        s.preview_chosen = false;
                    }
                    s.preview = Some((id, port));
                }
                Ok(false)
            }
            Action::Open(id) => {
                open_map(ui, s, r, &id);
                Ok(false)
            }
            Action::Enter(id) => {
                s.editor.path.push(id);
                s.selected = None;
                s.fields.clear();
                Ok(false)
            }
            // the canvas moved the node in its scene already: no new layout
            Action::Move(id, p) => crate::editor::move_node(&mut s.source, &path, &id, p).map(|_| {
                s.dirty = true;
                false
            }),
            Action::Connect { reference, node, port } => {
                crate::editor::connect(&mut s.source, r, &path, &reference, &node, &port).map(|_| true)
            }
        };
        match result {
            Ok(true) => {
                s.editor.message = None;
                s.edited();
            }
            Ok(false) => {}
            Err(e) => s.editor.message = Some(e),
        }
    }
}

/// What the node shown gives: a map (with its legend), the maps of a stack or of a loop's steps one at a time,
/// a table, or a value.
fn preview(ui: &mut Ui, s: &mut Session, r: &Registry) -> Option<Hover> {
    let Some((node, port)) = s.preview.clone() else {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("Select a node to see what it gives").color(ui.visuals().weak_text_color()));
        });
        return None;
    };
    let mut shown =
        Shown { node, port, band: s.preview_band, chosen: s.preview_chosen, view: std::mem::take(&mut s.preview_view) };
    let hover = output(ui, s, r, &mut shown, "viewer");
    s.preview = Some((shown.node, shown.port));
    s.preview_band = shown.band;
    s.preview_chosen = shown.chosen;
    s.preview_view = shown.view;
    hover
}

/// An output of a node shown: which, the map of a stack or series shown, whether the band chosen of rasters
/// was shown first already, and the view.
#[derive(Default)]
pub struct Shown {
    pub node: String,
    pub port: String,
    pub band: usize,
    pub chosen: bool,
    pub view: crate::map_view::MapView,
}

/// Where to save an output: a GeoTIFF for maps, a GeoPackage for vectors, else what the value is written as (CSV,
/// JSON).
fn save_dialog(node: &str, maps: bool, vector: bool) -> Option<PathBuf> {
    let d = rfd::FileDialog::new().set_title("Save");
    let d = if vector {
        d.add_filter("GeoPackage", &["gpkg"])
            .add_filter("GeoJSON", &["geojson"])
            .add_filter("Shapefile", &["shp"])
            .set_file_name(format!("{node}.gpkg"))
    } else if maps {
        d.add_filter("GeoTIFF", &["tif"]).set_file_name(format!("{node}.tif"))
    } else {
        d.add_filter("Table or value", &["csv", "json"]).set_file_name(node)
    };
    d.save_file()
}

/// A `raster.write` of a node's output in the model (beside the node), writing to `path` at every run.
pub fn add_write(s: &mut Session, r: &Registry, node: &str, port: &str, path: &Path) -> Result<String, String> {
    let at = s.source.nodes.iter().find(|n| n.id == node).and_then(|n| n.pos).unwrap_or([0.0, 0.0]);
    let pos = [at[0] + crate::editor_view::WIDTH + 70.0, at[1] + 140.0];
    let id = crate::editor::add_node(&mut s.source, r, &[], "raster.write", pos)?;
    crate::editor::set_param(&mut s.source, &[], &id, "path", &path.to_string_lossy())?;
    crate::editor::connect(&mut s.source, r, &[], &format!("{node}.{port}"), &id, "value")?;
    s.edited();
    Ok(id)
}

/// What a node's output is: a map (with its legend), the maps of a stack or of a loop's steps one at a time, a
/// table, or a value; with a header to show its other outputs. `salt` tells apart the panes that show outputs.
pub fn output(ui: &mut Ui, s: &mut Session, r: &Registry, sh: &mut Shown, salt: &str) -> Option<Hover> {
    let (node, port) = (sh.node.clone(), sh.port.clone());
    let op = s.model.nodes.iter().find(|n| n.id == node).map(|n| n.op.clone()).unwrap_or_default();
    let ports: Vec<String> = s
        .source
        .nodes
        .iter()
        .find(|n| n.id == node)
        .map(|n| crate::editor::output_ports(n, r))
        .unwrap_or_default()
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    let value = s.value_of(&node, &port);
    // the header: the node, its outputs, how it did
    egui::Frame::new().fill(theme::BAR).inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&node).strong());
            ui.label(RichText::new(&op).monospace().weak());
            ui.separator();
            for p in &ports {
                if ui.selectable_label(*p == port, p).clicked() {
                    sh.port = p.clone();
                    sh.band = 0;
                    sh.chosen = false;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // what is shown, saved: a file only, or a file and a node that writes it at every run
                let maps = matches!(&value, Some(Value::Map(_) | Value::Stack(_) | Value::Raster(_)))
                    || matches!(&value, Some(Value::Series(v)) if v.iter().all(|x| matches!(x, Value::Map(_))));
                let can = value.as_ref().is_some_and(|v| !matches!(v, Value::Rng(_)));
                ui.add_enabled_ui(can, |ui| {
                    ui.menu_button("Save output", |ui| {
                        if ui.button("As a file...").clicked() {
                            if let Some(p) = save_dialog(&node, maps, matches!(&value, Some(Value::Vector(_)))) {
                                s.save_output(&node, &port, &p);
                            }
                            ui.close();
                        }
                        if maps
                            && ui
                                .button("As a file, and a write node in the model...")
                                .on_hover_text("the model writes it again at every run")
                                .clicked()
                        {
                            if let Some(p) = save_dialog(&node, maps, matches!(&value, Some(Value::Vector(_)))) {
                                s.save_output(&node, &port, &p);
                                if let Err(e) = add_write(s, r, &node, &port, &p) {
                                    s.editor.message = Some(e);
                                }
                            }
                            ui.close();
                        }
                    });
                });
                ui.separator();
                let weak = ui.visuals().weak_text_color();
                match s.status.get(&node) {
                    Some(NodeStatus::Running) => {
                        ui.label(RichText::new("running").color(theme::ACCENT));
                        ui.spinner();
                    }
                    Some(NodeStatus::Done { cached: true }) => {
                        ui.label(RichText::new("nothing changed: from the cache").color(weak));
                    }
                    Some(NodeStatus::Done { cached: false }) => {
                        ui.label(RichText::new("run").color(weak));
                    }
                    Some(NodeStatus::Failed) => {
                        ui.label(RichText::new("failed").color(ui.visuals().error_fg_color));
                    }
                    None if value.is_some() => {
                        ui.label(RichText::new("from an earlier run").color(weak));
                    }
                    None => {}
                }
            });
        });
    });
    let Some(value) = value else {
        ui.centered_and_justified(|ui| {
            let text = match (&s.error, s.status.get(&node)) {
                (Some(e), Some(NodeStatus::Failed)) => RichText::new(e).color(ui.visuals().error_fg_color),
                _ if s.running() => RichText::new("running...").color(ui.visuals().weak_text_color()),
                _ => RichText::new("Not run yet: ctrl+enter runs it, and what it needs")
                    .color(ui.visuals().weak_text_color()),
            };
            ui.label(text);
        });
        return None;
    };
    // a map to show, of the maps of a stack, a series or rasters on disk
    let (maps, names): (Vec<Source>, Vec<String>) = match &value {
        Value::Map(m) => (vec![Source::from(m)], vec![]),
        Value::Stack(st) => (
            st.layers.iter().map(|l| Source::Map(Map { grid: st.grid.clone(), data: l.clone() })).collect(),
            st.keys.iter().map(|k| k.to_string()).collect(),
        ),
        Value::Series(items) if items.iter().all(|v| matches!(v, Value::Map(_))) && !items.is_empty() => (
            items.iter().filter_map(|v| if let Value::Map(m) = v { Some(Source::from(m)) } else { None }).collect(),
            (1..=items.len()).map(|k| format!("step {k}")).collect(),
        ),
        Value::Raster(r) => (
            (0..r.layers.len()).map(|l| Source::Rasters(r.clone(), l)).collect(),
            r.layers.iter().map(|l| l.name.clone()).collect(),
        ),
        _ => (vec![], vec![]),
    };
    // a raster's chosen band is shown first
    if let Value::Raster(r) = &value
        && sh.band == 0
        && r.chosen > 0
        && !sh.chosen
    {
        sh.band = r.chosen;
        sh.chosen = true;
    }
    if maps.is_empty() {
        if let Value::Vector(v) = &value {
            crate::vector_view::show(ui, v, egui::Id::new(("vector", salt, &node, &port)));
            return None;
        }
        if let Value::Table(t) = &value {
            // sorted, filtered and drawn as bars, the rows scrolled within
            let id = egui::Id::new(("table", salt, &node, &port));
            egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| crate::table_view::show(ui, t, id));
            return None;
        }
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| other(ui, &value));
        });
        return None;
    }
    let k = sh.band.min(maps.len() - 1);
    // the colours by what the values are: the operator that made them, the names they come by
    let (made, mut hint) = producer(s, &node, &port);
    hint.push_str(&port);
    if let Source::Rasters(r, l) = &maps[k] {
        hint.push(' ');
        hint.push_str(&r.layers[*l].name);
        hint.push(' ');
        hint.push_str(&r.images[r.layers[*l].image].path().to_string_lossy());
    }
    let palette = Palette::of(made, &hint);
    let look = s.looks.get(ui.ctx(), &maps[k], palette);
    egui::Panel::right(egui::Id::new(("output side", salt)))
        .resizable(true)
        .default_size(260.0)
        .frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::same(8)))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let (gw, gh) = maps[k].size();
                ui.label(RichText::new(format!("{gw} x {gh} cells")).weak());
                if gw * gh > 100_000_000 {
                    ui.label(RichText::new("on disk: read where it is shown").weak().size(11.0));
                }
                if maps.len() > 1 {
                    ui.add_space(4.0);
                    let mut b = k;
                    ui.horizontal(|ui| {
                        if ui.button("<").clicked() {
                            b = b.saturating_sub(1);
                        }
                        if ui.button(">").clicked() {
                            b = (b + 1).min(maps.len() - 1);
                        }
                        ui.label(&names[k]);
                    });
                    ui.spacing_mut().slider_width = ui.available_width() - 8.0;
                    ui.add(egui::Slider::new(&mut b, 0..=maps.len() - 1).show_value(false));
                    sh.band = b;
                }
                ui.add_space(4.0);
                match &look {
                    Some(l) => legend(ui, &l.coloring, &l.counts, &|v| class_name(l, v)),
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new("the legend...").weak());
                        });
                    }
                }
                // the world under the data (web maps, by GDAL), the data over it
                ui.add_space(8.0);
                theme::section(ui, "Basemap");
                let v = &mut sh.view;
                egui::ComboBox::from_id_salt(("basemap", salt)).selected_text(v.basemap.name()).width(150.0).show_ui(
                    ui,
                    |ui| {
                        for b in crate::map_view::Basemap::ALL {
                            ui.selectable_value(&mut v.basemap, b, b.name());
                        }
                    },
                );
                if v.basemap != crate::map_view::Basemap::None {
                    let mut show = !v.hide_data;
                    ui.checkbox(&mut show, "show the data");
                    v.hide_data = !show;
                    let mut opacity = 1.0 - v.transparency;
                    // (a slider no wider than the panel, its value and name beside it)
                    ui.spacing_mut().slider_width = (ui.available_width() - 110.0).clamp(40.0, 160.0);
                    ui.add_enabled(show, egui::Slider::new(&mut opacity, 0.0..=1.0).text("opacity"));
                    v.transparency = 1.0 - opacity;
                }
            });
        });
    let mut hover = None;
    egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::WELL)).show(ui, |ui| {
        if let Some(l) = &look {
            hover = sh.view.show(ui, &maps[k], &l.coloring);
        } else {
            ui.centered_and_justified(|ui| ui.spinner());
        }
    });
    hover
}

/// A value that is not a map: a table, a number, a text, or what it is.
fn other(ui: &mut Ui, v: &Value) {
    match v {
        Value::Number(x) => {
            ui.label(RichText::new(format!("{x}")).size(20.0).strong());
        }
        Value::Integer(x) => {
            ui.label(RichText::new(format!("{x}")).size(20.0).strong());
        }
        Value::Bool(x) => {
            ui.label(RichText::new(format!("{x}")).size(20.0).strong());
        }
        Value::Text(t) => {
            ui.label(RichText::new(t).monospace());
        }
        Value::Json(j) => {
            ui.label(RichText::new(serde_json::to_string_pretty(j).unwrap_or_default()).monospace());
        }
        v => {
            ui.label(RichText::new(format!("a {:?}", v.ty())).weak());
        }
    }
}

/// The rules of a reclassification as rows `(from, to)`, from its text (a rule per line or `;`).
pub fn rule_rows(text: &str) -> Vec<(String, String)> {
    text.split(['\n', ';'])
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| match l.split_once("->") {
            Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
            None => (l.to_string(), String::new()),
        })
        .collect()
}

/// The text of rows of rules (rows with nothing in them left out).
pub fn rule_text(rows: &[(String, String)]) -> String {
    rows.iter()
        .filter(|(a, b)| !a.trim().is_empty() || !b.trim().is_empty())
        .map(|(a, b)| format!("{} -> {}", a.trim(), b.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A number as rules write it (whole numbers without a decimal point).
fn number(v: f64) -> String {
    let t = format!("{v:.6}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The rules of a `raster.reclassify` as a table: the values a rule takes, the value they get; the values of
/// the map it reads (its classes and their cells, or its range) to start from. What to change, if anything.
pub(crate) fn reclassify_rules(ui: &mut Ui, s: &mut Session, n: &Node, path: &[String]) -> Option<Result<(), String>> {
    let committed = match n.params.get("rules") {
        Some(serde_json::Value::String(t)) => t.clone(),
        Some(v) => v.to_string(),
        None => String::new(),
    };
    let key = "param.rules".to_string();
    let text = s.fields.entry(key.clone()).or_insert(committed.clone()).clone();
    let mut rows = rule_rows(&text);
    let mut commit = false;
    let weak = ui.visuals().weak_text_color();
    ui.label(RichText::new("Rules").strong());
    ui.label(
        RichText::new(
            "Each rule gives a new value to the values it takes; the first rule that takes a value gives it, and \
             values no rule takes stay as they are.",
        )
        .size(11.5)
        .color(weak),
    );
    // the values of the map read, to start from
    let input = match n.inputs.get("map") {
        Some(terraflux_graph::Source::Ref(reference)) => match terraflux_graph::Ref::parse(reference) {
            Ok(terraflux_graph::Ref::Port { node, port }) if path.is_empty() => s.value_of(&node, &port),
            _ => None,
        },
        _ => None,
    };
    let source = match &input {
        Some(Value::Map(m)) => Some(Source::from(m)),
        Some(Value::Raster(r)) => Some(Source::Rasters(r.clone(), r.chosen)),
        _ => None,
    };
    let look = source.as_ref().and_then(|src| s.looks.get(ui.ctx(), src, Palette::Auto));
    egui::Frame::new().fill(theme::WELL).corner_radius(4.0).inner_margin(Margin::same(6)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match (&source, &look) {
            (None, _) => {
                ui.label(
                    RichText::new("Connect a map and run it (ctrl+enter) to see its values here.")
                        .size(11.5)
                        .color(weak),
                );
            }
            (Some(_), None) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("the values of the map...").size(11.5).color(weak));
                });
            }
            (Some(_), Some(l)) => match &l.coloring {
                Coloring::Classes(classes) => {
                    let total: usize = l.counts.iter().map(|(_, c)| c).sum::<usize>().max(1);
                    ui.label(RichText::new(format!("The map has {} classes:", classes.len())).size(11.5));
                    ui.horizontal_wrapped(|ui| {
                        for (v, col) in classes {
                            let cells = l.counts.iter().find(|(k, _)| k == v).map_or(0, |(_, c)| *c);
                            let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, *col);
                            ui.label(
                                RichText::new(format!("{} ({:.1}%)", number(*v), 100.0 * cells as f64 / total as f64))
                                    .size(11.5)
                                    .monospace(),
                            );
                            ui.add_space(4.0);
                        }
                    });
                    if ui
                        .small_button("A rule per class")
                        .on_hover_text(
                            "a row for every class of the map no rule takes yet, giving it its own value: then change \
                             the new values you want changed",
                        )
                        .clicked()
                    {
                        let taken: Vec<String> = rows.iter().map(|(a, _)| a.clone()).collect();
                        for (v, _) in classes {
                            let t = number(*v);
                            if !taken.contains(&t) {
                                rows.push((t.clone(), t));
                            }
                        }
                        commit = true;
                    }
                }
                Coloring::Continuous { min, max, .. } => {
                    ui.label(
                        RichText::new(format!(
                            "The map's values go from {} to {}: take them by ranges (from..to).",
                            number(*min),
                            number(*max)
                        ))
                        .size(11.5),
                    );
                    if ui
                        .small_button("4 equal ranges")
                        .on_hover_text("four rules of ranges of the same width, giving 1 to 4")
                        .clicked()
                    {
                        let w = (max - min) / 4.0;
                        rows.retain(|(a, b)| !a.trim().is_empty() || !b.trim().is_empty());
                        for k in 0..4 {
                            let (a, b) = (min + w * k as f64, if k == 3 { *max } else { min + w * (k + 1) as f64 });
                            rows.push((format!("{}..{}", number(a), number(b)), (k + 1).to_string()));
                        }
                        commit = true;
                    }
                }
            },
        }
    });
    ui.add_space(4.0);
    // the rules, a row each
    let mut gone = None;
    egui::Grid::new(("rules", &n.id)).num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
        ui.label(RichText::new("the values").size(11.0).color(weak));
        ui.label("");
        ui.label(RichText::new("become").size(11.0).color(weak));
        ui.end_row();
        for (k, (from, to)) in rows.iter_mut().enumerate() {
            let a = ui.add(egui::TextEdit::singleline(from).desired_width(110.0).hint_text("1 2 3, 0..10 or *"));
            ui.label("->");
            let b = ui.add(egui::TextEdit::singleline(to).desired_width(60.0).hint_text("value"));
            if ui.small_button("x").on_hover_text("take this rule away").clicked() {
                gone = Some(k);
            }
            commit |= a.lost_focus() || b.lost_focus();
            ui.end_row();
        }
    });
    if let Some(k) = gone {
        rows.remove(k);
        commit = true;
    }
    ui.horizontal(|ui| {
        if ui.button("+ a rule").clicked() {
            rows.push((String::new(), String::new()));
        }
        if ui.button("+ every other value").on_hover_text("a rule `* -> 0`: what no rule above takes").clicked() {
            rows.push(("*".into(), "0".into()));
            commit = true;
        }
    });
    egui::CollapsingHeader::new(RichText::new("How to write the values").size(11.5))
        .id_salt(("rules help", &n.id))
        .show(ui, |ui| {
            egui::Grid::new(("rules examples", &n.id)).num_columns(2).spacing([12.0, 2.0]).show(ui, |ui| {
                for (rule, says) in [
                    ("3 -> 1", "3 becomes 1"),
                    ("1 2 3 -> 1", "1, 2 and 3 become 1"),
                    ("0..10 -> 2", "from 0 to 10 (both in) become 2"),
                    ("* -> 0", "every value no rule above takes becomes 0"),
                    ("5 -> nan", "5 becomes no data"),
                ] {
                    ui.label(RichText::new(rule).monospace());
                    ui.label(RichText::new(says).size(11.5));
                    ui.end_row();
                }
            });
            ui.label(
                RichText::new(
                    "For MSPA, say: the forest's classes -> 2 and * -> 1 (the background); cells without a value \
                     stay missing.",
                )
                .size(11.0)
                .color(weak),
            );
        });
    // the rows as they are being written (empty ones too), until the next change of the model
    let shown: String = rows.iter().map(|(a, b)| format!("{a} -> {b}")).collect::<Vec<_>>().join("\n");
    s.fields.insert(key, shown);
    let new = rule_text(&rows);
    (commit && new != committed).then(|| crate::editor::set_param(&mut s.source, path, &n.id, "rules", &new))
}
