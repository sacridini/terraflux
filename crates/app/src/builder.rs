//! The model builder: the operators to add (a library by subject, with a search), the canvas of the graph, the
//! node selected in an inspector, and under the canvas what a node gives (a map, the maps of a stack or of a
//! loop's steps, a table, a value).
//!
//! A model is run as it is built: a run stops at the node shown and takes what did not change from the cache
//! (ctrl+enter, or by itself after every edit), so that changing a setting shows its result at once. What
//! takes long is never done in a frame: the colours and areas of a map are worked out in a thread, the maps
//! coloured in another.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{Color32, Margin, Rect, RichText, Ui, Vec2};
use terraflux_graph::{Node, Type};
use terraflux_ops::landscape::Product;
use terraflux_ops::{Map, Registry, Value};

use crate::app::{Look, Session};
use crate::editor_view::{Action, NodeStatus};
use crate::map_view::{Hover, Source, SourceKey, class_counts, legend};
use crate::palette::{Coloring, Ramp};
use crate::theme;

/// The subjects of the library: landschaft's (the sections of its reference, in its order), then the
/// model's own (map algebra, the flow of a model); with the operators' prefixes in each and, within a subject,
/// the groups of its operators.
/// The subjects of the library and the operators of each: by the prefix of their names, or by name (before the
/// prefixes).
const SUBJECTS: [(&str, &[&str]); 32] = [
    ("IO", &[]),
    ("Data preparation", &["raster", "vector", "data"]),
    (
        "Raster analysis",
        &[
            "raster.stats",
            "stack.reduce",
            "raster.zonal",
            "raster.tabulate",
            "raster.correlation",
            "raster.local_correlation",
            "raster.standardize",
            "raster.focal",
            "raster.majority",
            "raster.sieve",
            "raster.label",
            "raster.expand",
            "raster.shrink",
            "raster.contour",
            "raster.viewshed",
            "raster.cell_area",
            "raster.classify",
        ],
    ),
    (
        "Vector analysis",
        &[
            "vector.polygonize",
            "vector.filter",
            "vector.calc",
            "vector.geometry",
            "vector.overlay",
            "vector.spatial_join",
            "vector.reproject",
            "vector.from_table",
            "vector.points",
            "vector.density",
        ],
    ),
    ("Habitat patterns", &["landscape", "patterns"]),
    ("Fragmentation", &["fragmentation"]),
    ("Landscape metrics", &["metrics"]),
    ("Habitat networks", &["networks", "conn"]),
    ("Circuit connectivity", &["circuit"]),
    ("Surface metrics", &["surface"]),
    ("Terrain hydrology", &["hydro", "swat"]),
    ("Grid distances", &["cost"]),
    ("Distance & restoration", &["distance", "restoration"]),
    ("Change detection", &["change"]),
    ("Map accuracy and area", &["accuracy"]),
    ("Spatial statistics", &["stats"]),
    ("Large data & batch", &["batch"]),
    ("Neutral landscape models", &["nlm"]),
    ("Biodiversity analysis", &["bio"]),
    ("Spatial prioritization", &["prioritize"]),
    ("Species distributions", &["sdm"]),
    ("Animal movement", &["move"]),
    ("Ecosystem services", &["services"]),
    ("Carbon and REDD+", &["carbon", "redd"]),
    ("Forest Code (Brazil)", &["forest_code"]),
    ("Machine learning", &["ml"]),
    (
        "Land change simulation",
        &[
            "markov", "woe", "patches", "dinamica", "glm", "lulcc", "demand", "plus", "cars", "dynamic", "agents",
            "crafty", "validate",
        ],
    ),
    ("Uncertainty", &["uncertainty", "sensitivity"]),
    ("Map algebra", &["map", "stack"]),
    ("Tables", &["table"]),
    ("Plot", &["view"]),
    ("Model flow", &["control", "rng", "plugin"]),
];

/// The operators that read and write files: the subject IO.
const IO: [&str; 6] = ["raster.read", "raster.write", "vector.read", "vector.write", "table.read", "table.write"];

/// The subjects open when the library is first shown (the others closed until opened).
const OPEN: [&str; 2] = ["IO", "Plot"];

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
                let subject = if IO.contains(&g.name.as_str()) {
                    "IO"
                } else {
                    let named = SUBJECTS.iter().find(|(_, p)| p.contains(&g.name.as_str()));
                    named.or_else(|| SUBJECTS.iter().find(|(_, p)| p.contains(&prefix))).map_or("Other", |(s, _)| s)
                };
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
    /// The folder to process (Process a folder...): the operators that take a map, the one chosen run on every
    /// raster of it ([`process_folder`]).
    pub folder: Option<String>,
}

/// Process a folder...: choose the folder of the rasters, then the operator (the quick search, [`Quick::folder`]).
pub fn process_folder_dialog(s: &mut Session) {
    if let Some(folder) = rfd::FileDialog::new().set_title("The folder of the rasters to process").pick_folder() {
        s.editor.quick = Some(Quick { folder: Some(folder.to_string_lossy().into_owned()), ..Default::default() });
    }
}

/// Every raster of a folder through an operator, written beside the folder ([`crate::editor::process_folder`]),
/// the operator's node selected to choose its settings.
pub fn process_folder(s: &mut Session, r: &Registry, folder: &str, op: &str) -> Result<String, String> {
    let path = s.editor.path.clone();
    let nodes = s.source.scope(&path).cloned().unwrap_or_default();
    let at = beside(&nodes, None);
    let id = crate::editor::process_folder(&mut s.source, r, &path, folder, op, at)?;
    s.edited();
    select(s, r, &id);
    Ok(id)
}

/// A node of the user's libraries as the quick search offers it: `my:name`, what it does and takes.
fn mine_entry(u: &terraflux_graph::UserNode) -> Entry {
    let mut doc = u.description.clone().unwrap_or_else(|| format!("{}: a node of your libraries", u.label()));
    let takes: Vec<String> = u.inputs.iter().map(|(k, i)| format!("{k}: {}", i.summary())).collect();
    if !takes.is_empty() {
        doc.push_str(&format!(" (takes {})", takes.join(", ")));
    }
    Entry { name: format!("{}{}", crate::my_nodes::PREFIX, u.name), doc, subject: "My nodes", group: None }
}

/// What Process a folder... does.
pub const PROCESS_HELP: &str = "Choose a folder, then an operator (MSPA, a metric, a reclassification...): it runs on \
     every raster of the folder, file by file, each written beside the folder by its name (a table of all of them \
     for the metrics). Choose its settings, then run.";

/// The search of the operators from the keyboard, over the panes, while it is open.
pub fn quick_add(ctx: &egui::Context, s: &mut Session, r: &Registry, lib: &Library) {
    let Some(mut q) = s.editor.quick.take() else { return };
    // the nodes of the user's libraries first (`my:name`), then the operators
    let mine = Library { entries: crate::my_nodes::nodes().iter().map(mine_entry).collect() };
    let mut found = mine.find(&q.query);
    found.extend(lib.find(&q.query));
    // a folder to process: the operators that take a map
    if q.folder.is_some() {
        found.retain(|e| crate::editor::folder_port(&terraflux_graph::Node::new("x", &e.name), r).is_some());
    }
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
                match &q.folder {
                    Some(f) => {
                        let name =
                            std::path::Path::new(f).file_name().map_or(f.clone(), |n| n.to_string_lossy().into());
                        ui.label(
                            RichText::new(format!("Process the folder {name} · the operator for every raster"))
                                .strong(),
                        )
                        .on_hover_text(f);
                    }
                    None => {
                        let to = s.selected.as_ref().map_or("a new node".to_string(), |id| format!("after {id}"));
                        ui.label(RichText::new(format!("Add an operator · {to}")).strong());
                    }
                }
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
                        let resp = ui.add(
                            egui::Button::selectable(k == at, op_name(ui, &e.name, k == at))
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
    if let (Some(op), Some(folder)) = (&chosen, &q.folder) {
        if let Err(e) = process_folder(s, r, folder, op) {
            s.editor.message = Some(e);
        }
    } else if let Some(op) = chosen {
        let added = match crate::my_nodes::named(&op) {
            Some((name, v)) => crate::my_nodes::get(name, v).and_then(|u| crate::my_nodes::add(s, r, &u)).map(|_| ()),
            None => add(s, r, &op, &[]).map(|_| ()),
        };
        if let Err(e) = added {
            s.editor.message = Some(e);
        }
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
    /// Circuit currents (Circuitscape's maps, flow potential): a ramp on a scale of logarithms, as landschaft's.
    Current,
    /// Chosen by hand: a ramp, on a scale of logarithms or not.
    Chosen(Ramp, bool),
}

/// The ramps a map can be shown in by hand, by name.
pub const RAMPS: [(&str, Ramp); 8] = [
    ("viridis", Ramp::Viridis),
    ("cividis (for colour-blind readers)", Ramp::Cividis),
    ("blue to red, centred on 0", Ramp::Diverging),
    ("orange to purple, centred on 0 (for colour-blind readers)", Ramp::PurpleOrange),
    ("terrain", Ramp::Terrain),
    ("grey", Ramp::Grey),
    ("red to green", Ramp::Vegetation),
    ("rank", Ramp::Rank),
];

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
        } else if !starts("voltage")
            && (starts("current") || starts("curmap") || starts("currmap") || names.contains("flow_potential"))
        {
            Palette::Current
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
            Palette::Current => {
                Look { coloring: Coloring::continuous(values, Ramp::Viridis, true), counts: Vec::new(), product: None }
            }
            Palette::Chosen(r, log) => {
                // centred on 0 for the diverging ramp, over the percentiles for the others
                let coloring = match (r, log, &auto.coloring) {
                    (r, false, Coloring::Continuous { ramp: Ramp::Diverging, .. }) if r.diverging() => {
                        auto.coloring.clone().with_ramp(r)
                    }
                    _ => Coloring::continuous(values, r, log),
                };
                Look { coloring, counts: Vec::new(), product: None }
            }
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
/// The palette of a node's output as the viewer shows it: the one chosen by hand, else by what made it.
pub fn palette_of(s: &Session, node: &str, port: &str) -> Palette {
    if let Some(p) = s.palettes.get(&(node.to_string(), port.to_string())) {
        return *p;
    }
    let (made, mut hint) = producer(s, node, port);
    hint.push_str(port);
    Palette::of(made, &hint)
}

/// The look of a map of a node's output as the viewer gives it (the document's pictures): its palette and the
/// classes' colours of the model's legend.
pub fn output_look(s: &Session, node: &str, port: &str, map: &terraflux_ops::Map) -> Look {
    let look = palette_of(s, node, port).look(&map.data);
    match crate::legend_view::legend_of(s, node, port, &Source::Map(map.clone())) {
        Some(c) => crate::legend_view::apply(&look, &c),
        None => look,
    }
}

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
pub(crate) fn look_of(source: &Source, palette: Palette) -> Look {
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
        Source::Blank(_) => palette.look(&[]),
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
        egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| crate::node_view::panel(ui, s, r));
    });
}

/// The model's graph: where in it, run up to the node, and the canvas (or the model as text).
pub fn model_pane(ui: &mut Ui, s: &mut Session, r: &Registry) {
    egui::Panel::top("builder_bar")
        .frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::symmetric(8, 5)))
        .show(ui, |ui| {
            theme::quiet(ui);
            bar(ui, s, r)
        });
    if s.editor.text {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::same(8)))
            .show(ui, |ui| text_view(ui, s));
    } else {
        // (the modern look: its corners rounded)
        let radius = if theme::modern() { 6 } else { 0 };
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::canvas()).corner_radius(radius))
            .show(ui, |ui| canvas(ui, s, r));
    }
    rename_box(ui.ctx(), s, r);
    crate::my_nodes::dialog(ui.ctx(), s, r);
}

/// The model as text, with its problems above: only the lines in view are laid out (a model of many thousands
/// of nodes is a long text); the text made when it is first shown after an edit.
fn text_view(ui: &mut Ui, s: &mut Session) {
    use terraflux_exec::lint::Level;
    use terraflux_graph::Severity;
    if s.toml.is_none() {
        s.toml = Some(s.model.to_toml().unwrap_or_else(|e| e.to_string()));
    }
    let s = &*s;
    egui::Panel::top("model_problems").frame(egui::Frame::new()).show(ui, |ui| {
        if let Some(p) = &s.path {
            ui.label(RichText::new(p.display().to_string()).monospace());
        }
        if s.loading() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Reading and checking the data...");
            });
        }
        if let Some(e) = &s.load_error {
            ui.colored_label(ui.visuals().error_fg_color, format!("the inputs could not be read: {e}"));
        }
        let problems = s.shown_issues().count() + s.findings.len();
        ui.label(
            RichText::new(if problems == 0 {
                "No problem found in the model or its data.".to_string()
            } else {
                format!("{problems} problems found")
            })
            .strong(),
        );
        egui::ScrollArea::vertical().id_salt("problems").max_height(200.0).show(ui, |ui| {
            for i in s.shown_issues() {
                let c = if i.severity == Severity::Error {
                    ui.visuals().error_fg_color
                } else {
                    ui.visuals().warn_fg_color
                };
                ui.colored_label(c, i.to_string());
            }
            for f in &s.findings {
                let c = match f.level {
                    Level::Error => ui.visuals().error_fg_color,
                    Level::Warning => ui.visuals().warn_fg_color,
                    Level::Info => ui.visuals().text_color(),
                };
                ui.colored_label(c, f.to_string());
            }
        });
        ui.separator();
    });
    let text = s.toml.as_deref().unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let height = ui.text_style_height(&egui::TextStyle::Monospace);
    egui::ScrollArea::both().auto_shrink([false, false]).show_rows(ui, height, lines.len(), |ui, range| {
        for l in &lines[range] {
            ui.label(RichText::new(*l).monospace());
        }
    });
}

/// The box to rename the node selected (ctrl+R): enter renames it, escape or a click elsewhere leaves it.
fn rename_box(ctx: &egui::Context, s: &mut Session, r: &Registry) {
    let Some((id, mut name, focused)) = s.editor.renaming.take() else { return };
    let (enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    let at = s.editor.screen_of(&id).unwrap_or(ctx.content_rect().center());
    let area = egui::Area::new(egui::Id::new("rename node"))
        .order(egui::Order::Foreground)
        .fixed_pos(at - Vec2::new(120.0, 30.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.label(RichText::new(format!("Rename {id}")).strong());
                let out = egui::TextEdit::singleline(&mut name).desired_width(240.0).show(ui);
                // at first: the focus, and the whole name selected (typing replaces it)
                if !focused {
                    out.response.request_focus();
                    let mut state = out.state;
                    let all = egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(name.chars().count()),
                    );
                    state.cursor.set_char_range(Some(all));
                    state.store(ui.ctx(), out.response.id);
                }
                ui.label(RichText::new("enter: rename · esc: leave").size(10.5).color(ui.visuals().weak_text_color()));
            });
        });
    let away = ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
    if enter {
        let path = s.editor.path.clone();
        let new = name.trim().to_string();
        if new != id {
            match crate::editor::rename_node(&mut s.source, &path, &id, &new) {
                Ok(()) => {
                    if s.selected.as_deref() == Some(id.as_str()) {
                        select(s, r, &new);
                    }
                    s.fields.clear();
                    s.editor.message = None;
                    s.edited();
                }
                Err(e) => {
                    s.editor.message = Some(e);
                    s.editor.renaming = Some((id, name, true));
                    return;
                }
            }
        }
    } else if !escape && !away {
        s.editor.renaming = Some((id, name, true));
        return;
    }
    ctx.memory_mut(|m| {
        if let Some(f) = m.focused() {
            m.surrender_focus(f);
        }
    });
}

/// What the node shown gives; the cell under the pointer.
pub fn viewer_pane(ui: &mut Ui, s: &mut Session, r: &Registry) -> Option<Hover> {
    preview(ui, s, r)
}

/// Run up to a node (none: the whole model).
pub(crate) fn run_to(ui: &Ui, s: &mut Session, target: Option<String>) {
    let ctx = ui.ctx().clone();
    s.start_to(target, Box::new(move || ctx.request_repaint()));
}

/// Run up to the node selected, when it is a top-level node.
pub fn run_selected(ctx: &egui::Context, s: &mut Session) {
    if let Some(target) = runnable(s)
        && s.can_run_to()
    {
        let ctx = ctx.clone();
        s.start_to(Some(target), Box::new(move || ctx.request_repaint()));
    }
}

/// The selected node, when it is a top-level node (those run on their own).
fn runnable(s: &Session) -> Option<String> {
    s.selected.clone().filter(|id| s.editor.path.is_empty() && s.model.nodes.iter().any(|n| &n.id == id))
}

/// ctrl+Z undoes an edit (ctrl+shift+Z, ctrl+Y: does it again), ctrl+enter runs up to the node selected, del
/// deletes the nodes selected, ctrl+D duplicates them, ctrl+C and ctrl+V copy and paste them, ctrl+L arranges the
/// nodes, ctrl+R
/// renames the node selected, "/" searches the operators to add one; with the model's canvas in focus and the
/// pointer over it the arrows go to the node beside the one selected (kept in view) and enter shows its output:
/// whether to show the viewer.
pub fn keys(ui: &Ui, s: &mut Session, r: &Registry, canvas: bool) -> bool {
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    let mut show = false;
    // ctrl+Z undoes the last edit of the model, ctrl+shift+Z or ctrl+Y does it again (a field typed in undoes its
    // own text)
    s.history.tick(&s.source, ui.input(|i| i.pointer.any_down()));
    let (undo, redo) = ui.input_mut(|i| {
        if typing {
            return (false, false);
        }
        let redo = i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z)
            || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y);
        (i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z), redo)
    });
    if undo || redo {
        undo_redo(s, redo);
    }
    // (the pointer over the canvas: elsewhere the arrows go through the maps shown)
    let over = ui.input(|i| i.pointer.latest_pos()).is_some_and(|p| s.editor.canvas().is_some_and(|r| r.contains(p)));
    if canvas && over && !typing && s.editor.quick.is_none() && s.editor.renaming.is_none() && !s.editor.text {
        let (left, right, up, down, enter) = ui.input_mut(|i| {
            let none = egui::Modifiers::NONE;
            (
                i.consume_key(none, egui::Key::ArrowLeft),
                i.consume_key(none, egui::Key::ArrowRight),
                i.consume_key(none, egui::Key::ArrowUp),
                i.consume_key(none, egui::Key::ArrowDown),
                i.consume_key(none, egui::Key::Enter),
            )
        });
        let dir = [
            (left, egui::vec2(-1.0, 0.0)),
            (right, egui::vec2(1.0, 0.0)),
            (up, egui::vec2(0.0, -1.0)),
            (down, egui::vec2(0.0, 1.0)),
        ]
        .into_iter()
        .find(|(k, _)| *k)
        .map(|(_, d)| d);
        if let Some(d) = dir
            && let Some(id) = s.editor.neighbour(s.selected.as_deref(), d)
        {
            select(s, r, &id);
            s.editor.chosen = [id.clone()].into();
            s.editor.reveal(&id);
        }
        show = enter && s.selected.is_some();
    }
    let rename = !typing && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::R));
    if rename && let Some(id) = s.selected.clone() {
        s.editor.text = false;
        s.editor.renaming = Some((id.clone(), id, false));
    }
    // (in the calculator's script ctrl+enter is its own, which gives the script to the node first)
    let calculator = crate::calc_view::editing(ui.ctx());
    let (run, delete, tidy, search) = ui.input_mut(|i| {
        // "/" as typed, whatever the keyboard's layout puts it on (taken away, so that it is not typed in the
        // search it opens)
        let slash = !typing && i.events.iter().any(|e| matches!(e, egui::Event::Text(t) if t == "/"));
        if slash {
            i.events.retain(|e| !matches!(e, egui::Event::Text(t) if t == "/"));
        }
        (
            !calculator && i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter),
            // delete, or backspace (the key macOS deletes with)
            !typing
                && (i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)),
            !typing && i.consume_key(egui::Modifiers::COMMAND, egui::Key::L),
            slash,
        )
    });
    // ctrl+D duplicates the nodes selected; ctrl+C copies them (as text, to the system's clipboard: to another
    // scope, model or window) and ctrl+V pastes them, with the model's tab in focus (the events taken, so that no
    // label copies its text then)
    let here = canvas && !typing && s.editor.quick.is_none() && s.editor.renaming.is_none() && !s.editor.text;
    // ctrl+G groups the nodes selected, ctrl+shift+G takes the group selected apart
    let (group, ungroup) = ui.input_mut(|i| {
        if !here {
            return (false, false);
        }
        let ungroup = i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::G);
        (i.consume_key(egui::Modifiers::COMMAND, egui::Key::G), ungroup)
    });
    if group {
        group_selected(s, r);
    }
    // B turns the nodes selected off (bypassed: their inputs passed on) or on again
    if here && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::B)) {
        bypass_selected(s);
    }
    if ungroup {
        ungroup_selected(s, r);
    }
    let (duplicate, copy, paste) = ui.input_mut(|i| {
        if !here {
            return (false, false, None);
        }
        let duplicate = i.consume_key(egui::Modifiers::COMMAND, egui::Key::D);
        let copy = i.events.iter().any(|e| matches!(e, egui::Event::Copy));
        let paste = i.events.iter().find_map(|e| match e {
            egui::Event::Paste(t) => crate::editor::clip_nodes(t),
            _ => None,
        });
        if copy && !chosen(s).is_empty() {
            i.events.retain(|e| !matches!(e, egui::Event::Copy));
        }
        if paste.is_some() {
            i.events.retain(|e| !matches!(e, egui::Event::Paste(_)));
        }
        (duplicate, copy, paste)
    });
    if duplicate {
        let nodes = chosen(s);
        put(s, r, &nodes, None, true);
    }
    if copy {
        let nodes = chosen(s);
        if !nodes.is_empty() {
            ui.ctx().copy_text(crate::editor::clip_text(&nodes));
        }
    }
    if let Some(nodes) = paste {
        // where the pointer is on the canvas, else beside where they were
        let at = ui.input(|i| i.pointer.latest_pos()).and_then(|p| s.editor.graph_at(p));
        put(s, r, &nodes, at.map(|p| [p.x, p.y]), false);
    }
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
            match crate::editor::remove_box(&mut s.source, &path, id) {
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
    show
}

/// The model as it was before its last edit (`redo`: the edit undone last made again); the nodes selected or shown
/// that are no more, let go.
pub fn undo_redo(s: &mut Session, redo: bool) {
    let done = if redo { s.history.redo(&mut s.source) } else { s.history.undo(&mut s.source) };
    if !done {
        return;
    }
    forget_missing(s);
    s.edited();
}

/// After the model was replaced (undone, read again from its file): the nodes selected or shown that are no more,
/// and the containers entered that are no more, let go.
pub fn forget_missing(s: &mut Session) {
    s.editor.renaming = None;
    s.editor.message = None;
    while s.source.scope(&s.editor.path).is_none() {
        s.editor.path.pop();
    }
    let ids: std::collections::HashSet<String> =
        s.source.scope(&s.editor.path).map(|n| n.iter().map(|n| n.id.clone()).collect()).unwrap_or_default();
    // (the box of something the scope reads: kept, unless it is an input of the model no more)
    let inputs = &s.source.inputs;
    let here = |id: &String| match id.strip_prefix('$') {
        Some(name) => inputs.contains_key(name),
        None => ids.contains(id) || crate::editor::is_source(id),
    };
    s.editor.chosen.retain(here);
    if s.selected.as_ref().is_some_and(|id| !here(id)) {
        s.selected = None;
    }
    if s.preview.as_ref().is_some_and(|(n, _)| s.source.node(n).is_none()) {
        s.preview = None;
    }
}

/// The nodes selected in the scope shown (else the node selected), each with its place on the canvas.
fn chosen(s: &Session) -> Vec<Node> {
    let Some(nodes) = s.source.scope(&s.editor.path) else { return Vec::new() };
    let mut ids: Vec<&String> = s.editor.chosen.iter().collect();
    if ids.is_empty() {
        ids.extend(s.selected.as_ref());
    }
    ids.retain(|id| !crate::editor::is_source(id));
    let auto = crate::editor_view::auto_positions(nodes);
    nodes
        .iter()
        .filter(|n| ids.contains(&&n.id))
        .map(|n| {
            let mut c = n.clone();
            c.pos = Some(n.pos.unwrap_or(auto[&n.id]));
            c
        })
        .collect()
}

/// Add nodes copied to the scope shown, as new nodes (new ids; their connections from outside them kept when
/// `outside`): the first of them at `at` (graph units) and the others as they were around it, else a step down
/// and to the right of where they were (more steps while a node is there). They become the nodes selected.
fn put(s: &mut Session, r: &Registry, copies: &[Node], at: Option<[f32; 2]>, outside: bool) {
    let Some(first) = copies.first() else { return };
    let from = first.pos.unwrap_or([0.0, 0.0]);
    let path = s.editor.path.clone();
    let offset = match at {
        Some(p) => [p[0] - from[0], p[1] - from[1]],
        None => {
            let places: Vec<[f32; 2]> =
                s.source.scope(&path).map(|n| n.iter().filter_map(|n| n.pos).collect()).unwrap_or_default();
            let mut d = [40.0, 40.0];
            while places.iter().any(|q| (q[0] - from[0] - d[0]).abs() < 5.0 && (q[1] - from[1] - d[1]).abs() < 5.0) {
                d = [d[0] + 40.0, d[1] + 40.0];
            }
            d
        }
    };
    match crate::editor::paste_nodes(&mut s.source, &path, copies, offset, outside) {
        Ok(ids) => {
            s.editor.chosen = ids.iter().cloned().collect();
            if let [id] = ids.as_slice() {
                select(s, r, id);
            }
            s.editor.message = None;
            s.edited();
        }
        Err(e) => s.editor.message = Some(e),
    }
}

/// The chart a plot's settings ask of a table: its kind (`chart`: bars, lines, points, boxes; else lines along a
/// date, layer, step or year when the table has one, bars when not), the columns drawn (`y`, comma-separated),
/// along `x`, a group by `by`; what is not given as the kind suggests.
pub fn plot_chart(n: &Node, t: &terraflux_ops::Table) -> crate::charts::Chart {
    use crate::charts::{Chart, Kind};
    let text = |k: &str| n.params.get(k).and_then(|v| v.as_str()).map(str::trim).filter(|v| !v.is_empty());
    let column = |name: &str| t.columns.iter().position(|c| c.eq_ignore_ascii_case(name.trim()));
    let along = ["date", "year", "step", "layer"].iter().any(|c| column(c).is_some());
    let kinds = Kind::of(t);
    let kind = match text("chart") {
        Some(c) => kinds.iter().copied().find(|k| k.name().eq_ignore_ascii_case(c)),
        None => Some(if along { Kind::Lines } else { Kind::Bars }).filter(|k| kinds.contains(k)),
    };
    let Some(kind) = kind else { return Chart::default() };
    let mut chart = Chart::suggested(t, kind);
    if let Some(x) = text("x").and_then(column) {
        chart.x = Some(x);
    }
    if let Some(y) = text("y") {
        let ys: Vec<usize> = y.split(',').filter_map(column).collect();
        if !ys.is_empty() {
            chart.columns = ys;
        }
    }
    if let Some(by) = text("by").and_then(column) {
        chart.by = Some(by);
    }
    chart
}

/// The output of a node to show first: a map, a stack or a series, else the first.
pub(crate) fn first_output(n: &Node, r: &Registry) -> Option<String> {
    let outs = crate::editor::output_ports(n, r);
    outs.iter()
        .find(|(_, t)| matches!(t, Type::Raster | Type::Map | Type::Stack | Type::Series))
        .or(outs.first())
        .map(|(p, _)| p.clone())
}

/// Where the viewer finds an output of a node of the scope `path`: the node itself at the top level; inside groups
/// the top-level group, which gives what its body's nodes gave as `node.port` (`inner.node.port` for a group in
/// it); none in a loop's or a switch's body (a value per step, a branch).
pub fn shown_at(s: &Session, path: &[String], id: &str, port: &str) -> Option<(String, String)> {
    crate::trace::key(&s.source, path, id, port)
}

/// The node of a group's body an output shown is of (`inner.node.port` of the group `node`), and the place of its
/// outputs there (`inner.node`); none for a node's own output.
pub fn inner_of(s: &Session, node: &str, port: &str) -> Option<(Node, String)> {
    let (place, _) = port.rsplit_once('.')?;
    let mut nodes = &s.source.nodes;
    let mut n = nodes.iter().find(|n| n.id == node)?;
    for id in place.split('.') {
        nodes = &n.body.as_ref()?.nodes;
        n = nodes.iter().find(|n| n.id == id)?;
    }
    Some((n.clone(), place.to_string()))
}

/// Select a node and show its first output.
pub fn select(s: &mut Session, r: &Registry, id: &str) {
    if s.selected.as_deref() != Some(id) {
        s.fields.clear();
    }
    s.selected = Some(id.to_string());
    let path = s.editor.path.clone();
    if let Some(n) = s.source.scope(&path).and_then(|ns| ns.iter().find(|n| n.id == id))
        && let Some(port) = first_output(n, r)
        && let Some((node, port)) = shown_at(s, &path, id, &port)
    {
        // (another node: its first band, not the one chosen for the last)
        let place = |q: &str| q.rsplit_once('.').map(|(a, _)| a.to_string());
        if s.preview.as_ref().is_none_or(|(p, q)| *p != node || place(q) != place(&port)) {
            s.preview_band = 0;
            s.preview_chosen = false;
        }
        s.preview = Some((node, port));
    }
}

/// The bands of the file a `raster.read` of the top level reads: from its last run, else from the file's header
/// (relative to the model's folder); none when neither is known.
fn bands_of(s: &Session, id: &str) -> Option<usize> {
    if let Some(Value::Raster(r)) = s.outputs.get(id).and_then(|o| o.get("raster")) {
        return Some(r.layers.len());
    }
    let n = s.source.nodes.iter().find(|n| n.id == id)?;
    let path = std::path::PathBuf::from(n.params.get("path")?.as_str()?);
    let path = match (&s.path, path.is_relative()) {
        (Some(model), true) => model.parent()?.join(path),
        _ => path,
    };
    terraflux_ops::files::Rasters::open(&path.to_string_lossy(), None, None).ok().map(|r| r.layers.len())
}

/// Where a new node goes: to the right of the node `from` (below what is there already), else below every node.
pub fn beside(nodes: &[Node], from: Option<&str>) -> [f32; 2] {
    let auto = crate::editor_view::auto_positions(nodes);
    let at = |n: &Node| n.pos.unwrap_or(auto[&n.id]);
    match from.and_then(|id| nodes.iter().find(|n| n.id == id)) {
        Some(n) => {
            let p = at(n);
            let right = [p[0] + crate::editor_view::node_width(n, crate::editor_view::standard()) + 70.0, p[1]];
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
    }
}

/// Add a node of an operator next to the one selected, its first input fed by the selected node's first output
/// that fits; it becomes the node selected.
pub fn add(s: &mut Session, r: &Registry, op: &str, params: &[(&str, serde_json::Value)]) -> Result<String, String> {
    add_at(s, r, op, params, None)
}

/// Add a node of an operator: at a place of the graph, if given (nothing connected to it), else as [`add`] does.
pub fn add_at(
    s: &mut Session,
    r: &Registry,
    op: &str,
    params: &[(&str, serde_json::Value)],
    at: Option<[f32; 2]>,
) -> Result<String, String> {
    let path = s.editor.path.clone();
    let nodes = s.source.scope(&path).cloned().unwrap_or_default();
    let from = match at {
        Some(_) => None,
        None => s.selected.as_ref().and_then(|id| nodes.iter().find(|n| &n.id == id)).cloned(),
    };
    let pos = at.unwrap_or_else(|| beside(&nodes, from.as_ref().map(|n| n.id.as_str())));
    let id = crate::editor::add_node(&mut s.source, r, &path, op, pos)?;
    if s.source.scope(&path).is_some_and(|v| v.iter().any(|n| n.id == id)) {
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
            feed(s, r, &path, &id, f);
        }
    }
    s.edited();
    select(s, r, &id);
    Ok(id)
}

/// Connect a node just added to the node `from`: the first input it must have, else the first a value of `from`
/// fits, fed by the first output of `from` that fits it.
pub fn feed(s: &mut Session, r: &Registry, path: &[String], id: &str, from: &Node) {
    let Some(n) = s.source.scope(path).and_then(|v| v.iter().find(|n| n.id == id)).cloned() else { return };
    let outs = crate::editor::output_ports(from, r);
    let ins = crate::editor::input_ports(&n, r);
    let target = ins
        .iter()
        .find(|(_, _, req)| *req)
        .or_else(|| ins.iter().find(|(_, ty, _)| outs.iter().any(|(_, t)| t.fits(*ty))))
        .cloned();
    if let Some((port, ty, _)) = target
        && let Some((out, _)) = outs.iter().find(|(_, t)| t.fits(ty))
    {
        let _ = crate::editor::connect(&mut s.source, r, path, &format!("{}.{out}", from.id), id, &port);
    }
}

/// The files of vector features the dialogs offer (GDAL reads others too).
pub const VECTOR_EXTENSIONS: [&str; 8] = ["gpkg", "shp", "geojson", "json", "fgb", "kml", "gml", "parquet"];

/// An operator's whole name as the lists write it: its subject (`stack.`) in a weaker colour than the rest (`of`);
/// on the colour of a row `selected`, both in the selection's.
pub fn op_name(ui: &Ui, name: &str, selected: bool) -> egui::text::LayoutJob {
    let at = name.find('.').map_or(0, |k| k + 1);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let v = ui.visuals();
    let (weak, strong) = if selected {
        (v.selection.stroke.color.gamma_multiply(0.75), v.selection.stroke.color)
    } else {
        (v.weak_text_color(), v.strong_text_color())
    };
    let mut job = egui::text::LayoutJob::default();
    job.append(&name[..at], 0.0, egui::TextFormat::simple(font.clone(), weak));
    job.append(&name[at..], 0.0, egui::TextFormat::simple(font, strong));
    job
}

/// The operators as a classic menu (as Photoshop's): the favourites by colour, then a submenu per subject, its
/// groups a level below; an operator chosen is added after the node selected. Whether one was.
pub fn operators_menu(ui: &mut Ui, s: &mut Session, r: &Registry, lib: &Library) -> bool {
    let mut chosen: Option<String> = None;
    let item = |ui: &mut Ui, e: &Entry, chosen: &mut Option<String>| {
        let colls = crate::settings::collections_of(&e.name);
        let resp = ui
            .add(egui::Button::new(op_name(ui, &e.name, false)).right_text(RichText::new(" ".repeat(3 * colls.len()))));
        for (i, c) in colls.iter().rev().enumerate() {
            let x = resp.rect.right() - 6.0 - i as f32 * 10.0;
            let sq = egui::Rect::from_center_size(egui::pos2(x - 3.5, resp.rect.center().y), Vec2::splat(7.0));
            ui.painter().rect_filled(sq, 1.0, crate::settings::COLLECTIONS[*c as usize].1);
        }
        if resp.on_hover_text(&e.doc).clicked() {
            *chosen = Some(e.name.clone());
            ui.close();
        }
    };
    ui.set_min_width(200.0);
    if ui.button("Open a raster or vectors...    ctrl+O").clicked() {
        ui.close();
        open_data(s, r);
    }
    ui.separator();
    ui.menu_button("Favourites", |ui| {
        let mut any = false;
        for (k, (_, colour)) in crate::settings::COLLECTIONS.iter().enumerate() {
            let ops = crate::settings::in_collection(k as u8);
            if ops.is_empty() {
                continue;
            }
            any = true;
            let label = RichText::new(format!("■  {}", crate::settings::collection_name(k as u8))).color(*colour);
            ui.menu_button(label, |ui| {
                for e in lib.entries.iter().filter(|e| ops.contains(&e.name)) {
                    item(ui, e, &mut chosen);
                }
            });
        }
        if !any {
            ui.label(RichText::new("none yet: right-click an operator to put it in one").weak().size(11.0));
        }
    });
    ui.separator();
    for (subject, _) in SUBJECTS.iter().chain([("Other", &[][..])].iter()) {
        let in_it: Vec<&Entry> = lib.entries.iter().filter(|e| e.subject == *subject).collect();
        if in_it.is_empty() {
            continue;
        }
        ui.menu_button(*subject, |ui| {
            for e in in_it.iter().filter(|e| e.group.is_none()) {
                item(ui, e, &mut chosen);
            }
            let mut groups: Vec<&str> = in_it.iter().filter_map(|e| e.group).collect();
            groups.dedup();
            if !groups.is_empty() && in_it.iter().any(|e| e.group.is_none()) {
                ui.separator();
            }
            for g in groups {
                ui.menu_button(g, |ui| {
                    for e in in_it.iter().filter(|e| e.group == Some(g)) {
                        item(ui, e, &mut chosen);
                    }
                });
            }
        });
    }
    match chosen {
        Some(op) => {
            if let Err(e) = add(s, r, &op, &[]) {
                s.editor.message = Some(e);
            }
            true
        }
        None => false,
    }
}

/// Open a raster or vector features (ctrl+O): a node that reads the file chosen, after the node selected.
pub fn open_data(s: &mut Session, r: &Registry) {
    let mut both: Vec<&str> = terraflux_ops::files::RASTER_EXTENSIONS.to_vec();
    both.extend(VECTOR_EXTENSIONS);
    let Some(p) = rfd::FileDialog::new()
        .set_title("A raster or vector features")
        .add_filter("Rasters and vectors", &both)
        .add_filter("Rasters", &terraflux_ops::files::RASTER_EXTENSIONS)
        .add_filter("Vectors", &VECTOR_EXTENSIONS)
        .add_filter("Every file", &["*"])
        .pick_file()
    else {
        return;
    };
    let op = if is_vector(&p) { "vector.read" } else { "raster.read" };
    if let Err(e) = add(s, r, op, &[("path", p.to_string_lossy().into_owned().into())]) {
        s.editor.message = Some(e);
    }
}

/// Whether a file is of vector features, by its extension.
/// Whether a file is a table of values (`.csv`, `.tsv`).
pub fn is_table(p: &std::path::Path) -> bool {
    p.extension().is_some_and(|e| ["csv", "tsv"].contains(&e.to_string_lossy().to_lowercase().as_str()))
}

pub fn is_vector(p: &std::path::Path) -> bool {
    p.extension().is_some_and(|e| VECTOR_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
}

/// Whether a file is a raster, by its extension.
pub fn is_raster(p: &std::path::Path) -> bool {
    p.extension()
        .is_some_and(|e| terraflux_ops::files::RASTER_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
}

/// A node that reads a file (a raster's `raster.read`, vector features' `vector.read`), at a place of the graph or
/// after the node selected (a file of neither kind: read as a raster, GDAL knows many more).
pub fn add_file(s: &mut Session, r: &Registry, p: &Path, at: Option<[f32; 2]>) -> Result<String, String> {
    let op = if is_table(p) {
        "table.read"
    } else if is_vector(p) {
        "vector.read"
    } else {
        "raster.read"
    };
    add_at(s, r, op, &[("path", p.to_string_lossy().into_owned().into())], at)
}

/// Files dropped on a window (from the system's file manager): a node each that reads it, one under the other
/// (the folders left out); a model (`.toml`) is not data, it is given back to be opened.
pub fn add_files(s: &mut Session, r: &Registry, files: &[PathBuf]) -> Option<PathBuf> {
    let mut model = None;
    let selected = s.selected.clone();
    for p in files.iter().filter(|p| p.is_file()) {
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")) {
            model = Some(p.clone());
            continue;
        }
        // each beside the node that was selected, under the one before
        s.selected = selected.clone();
        if let Err(e) = add_file(s, r, p, None) {
            s.editor.message = Some(e);
        }
    }
    model
}

/// The files dragged from the Files pane (`egui`'s drag and drop).
#[derive(Debug, Clone)]
pub struct DraggedFile(pub Vec<PathBuf>);

fn library(ui: &mut Ui, s: &mut Session, r: &Registry, lib: &Library) {
    // the collections of favourites that have operators, by name and count: one chosen shows its operators; a
    // right click names it
    theme::section(ui, "Favourites");
    let shown_id = egui::Id::new("collection shown");
    let mut shown: Option<u8> = ui.data(|d| d.get_temp(shown_id)).flatten();
    let filled: Vec<(u8, usize)> = (0..crate::settings::COLLECTIONS.len() as u8)
        .map(|k| (k, crate::settings::in_collection(k).len()))
        .filter(|(k, count)| *count > 0 || shown == Some(*k))
        .collect();
    if filled.is_empty() {
        ui.label(
            RichText::new(
                "Right-click an operator to put it in a collection of favourites, and a collection to name it.",
            )
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
        );
    }
    ui.horizontal_wrapped(|ui| {
        for (k, count) in filled {
            let colour = crate::settings::COLLECTIONS[k as usize].1;
            let name = crate::settings::collection_name(k);
            let on = shown == Some(k);
            let mut job = egui::text::LayoutJob::default();
            let font = egui::TextStyle::Button.resolve(ui.style());
            job.append("■ ", 0.0, egui::TextFormat::simple(font.clone(), colour));
            let text = if on { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
            job.append(&format!("{name} {count}"), 0.0, egui::TextFormat::simple(font, text));
            let resp = ui.add(egui::Button::selectable(on, job)).on_hover_text(format!(
                "{name}: {count} operators (right-click an operator to put it here; right-click here to name it)"
            ));
            if resp.clicked() {
                shown = if on { None } else { Some(k) };
            }
            resp.context_menu(|ui| {
                ui.label(RichText::new("The collection's name").weak().size(11.0));
                let key = format!("collection name {k}");
                let mut text = s.fields.entry(key.clone()).or_insert_with(|| name.clone()).clone();
                let edit = ui.add(egui::TextEdit::singleline(&mut text).desired_width(160.0));
                s.fields.insert(key.clone(), text.clone());
                if edit.lost_focus() || ui.button("Rename").clicked() {
                    crate::settings::rename_collection(k, &text);
                    s.fields.remove(&key);
                    ui.close();
                }
                if ui.button("Its colour's name").clicked() {
                    crate::settings::rename_collection(k, "");
                    s.fields.remove(&key);
                    ui.close();
                }
            });
        }
    });
    ui.data_mut(|d| d.insert_temp(shown_id, shown));
    ui.add_space(4.0);
    let query = s.editor.search.clone();
    crate::my_nodes::list(ui, s, r, &query);
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
            // the name in one line (cut short where the pane is narrow), the squares of its collections at the right
            let colls = crate::settings::collections_of(&e.name);
            let room = " ".repeat(3 * colls.len());
            let resp = ui.add(
                egui::Button::selectable(false, op_name(ui, &e.name, false))
                    .wrap_mode(egui::TextWrapMode::Truncate)
                    .right_text(RichText::new(room).size(10.5))
                    .min_size(Vec2::new(ui.available_width(), 20.0)),
            );
            for (i, c) in colls.iter().rev().enumerate() {
                let x = resp.rect.right() - 6.0 - i as f32 * 10.0;
                let sq = egui::Rect::from_center_size(egui::pos2(x - 3.5, resp.rect.center().y), Vec2::splat(7.0));
                ui.painter().rect_filled(sq, 1.0, crate::settings::COLLECTIONS[*c as usize].1);
            }
            resp.context_menu(|ui| {
                ui.label(RichText::new("Favourites").weak().size(11.0));
                for (k, (_, colour)) in crate::settings::COLLECTIONS.iter().enumerate() {
                    let mut on = colls.contains(&(k as u8));
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 1.0, *colour);
                        if ui.checkbox(&mut on, crate::settings::collection_name(k as u8)).changed() {
                            crate::settings::toggle(&e.name, k as u8);
                        }
                    });
                }
            });
            if resp.on_hover_text(&e.doc).clicked() {
                *chosen = Some(e.name.clone());
            }
        };
        if let (true, Some(k)) = (query.is_empty(), shown) {
            let ops = crate::settings::in_collection(k);
            if ops.is_empty() {
                ui.label(
                    RichText::new("none yet: right-click an operator to put it in this collection").weak().size(11.0),
                );
            }
            for e in lib.entries.iter().filter(|e| ops.contains(&e.name)) {
                entry(ui, e, &mut chosen);
            }
        } else if query.is_empty() {
            for (subject, _) in SUBJECTS.iter().chain([("Other", &[][..])].iter()) {
                let in_it: Vec<&Entry> = lib.entries.iter().filter(|e| e.subject == *subject).collect();
                if in_it.is_empty() {
                    // a subject of landschaft not here yet: its name, to see where things will be
                    if *subject != "Other" {
                        ui.add_enabled(
                            false,
                            egui::Button::new(RichText::new(format!("    {subject}")).size(12.5))
                                .frame(false)
                                .wrap_mode(egui::TextWrapMode::Truncate),
                        );
                    }
                    continue;
                }
                let id = ui.make_persistent_id(*subject);
                theme::fold(ui, id, RichText::new(*subject).strong(), OPEN.contains(subject), |ui| {
                    // the operators of no group, then each group a level below, closed until opened
                    for e in in_it.iter().filter(|e| e.group.is_none()) {
                        entry(ui, e, &mut chosen);
                    }
                    let mut groups: Vec<&str> = in_it.iter().filter_map(|e| e.group).collect();
                    groups.dedup();
                    for g in groups {
                        let id = ui.make_persistent_id((*subject, g));
                        theme::fold(ui, id, g, false, |ui| {
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

/// Put the nodes selected (those chosen, else the one in the inspector) into a new group of the scope shown
/// (ctrl+G), and select it.
/// The nodes selected (or the one) turned off, or on again when all of them were off.
pub fn bypass_selected(s: &mut Session) {
    let mut ids: Vec<String> = s.editor.chosen.iter().cloned().collect();
    if ids.is_empty() {
        ids.extend(s.selected.clone());
    }
    ids.retain(|id| !crate::editor::is_source(id));
    if ids.is_empty() {
        return;
    }
    let path = s.editor.path.clone();
    if crate::editor::toggle_bypass(&mut s.source, &path, &ids).is_ok() {
        s.edited();
    }
}

pub fn group_selected(s: &mut Session, r: &Registry) {
    let mut ids: Vec<String> = s.editor.chosen.iter().cloned().collect();
    if ids.is_empty() {
        ids.extend(s.selected.clone());
    }
    ids.retain(|id| !crate::editor::is_source(id));
    let path = s.editor.path.clone();
    let Some(scope) = s.source.scope(&path) else { return };
    // in the order of the scope
    ids.sort_by_key(|id| scope.iter().position(|n| &n.id == id));
    let id = (1..)
        .map(|k| if k == 1 { "group".to_string() } else { format!("group_{k}") })
        .find(|id| !scope.iter().any(|n| &n.id == id))
        .unwrap();
    match crate::editor::group_nodes(&mut s.source, r, &path, &ids, &id, None) {
        Ok(()) => {
            s.editor.chosen.clear();
            s.edited();
            s.selected = Some(id);
        }
        Err(e) => s.editor.message = Some(e),
    }
}

/// Take the nodes of the group selected out into the scope shown (ctrl+shift+G).
pub fn ungroup_selected(s: &mut Session, r: &Registry) {
    let Some(id) = s.selected.clone() else { return };
    let path = s.editor.path.clone();
    match crate::editor::ungroup(&mut s.source, r, &path, &id) {
        Ok(()) => {
            s.selected = None;
            s.editor.chosen.clear();
            s.edited();
        }
        Err(e) => s.editor.message = Some(e),
    }
}

/// Put the model's top level in a group per stage (data, preparation, model, validation, results), laid out again.
pub fn group_by_stage(s: &mut Session, r: &Registry) {
    match crate::document::group_by_stage(&mut s.source, r) {
        Ok(made) => {
            s.editor.path.clear();
            s.selected = None;
            s.editor.chosen.clear();
            s.edited();
            s.editor.fit();
            s.notice = Some((format!("grouped by stage: {}", made.join(", ")), std::time::Instant::now()));
        }
        Err(e) => s.editor.message = Some(e),
    }
}

/// Inside a container, the canvas framed in the accent's colour and told so at its top left: the nodes shown are
/// the container's, not the whole model's.
fn inside(ui: &Ui, rect: egui::Rect, name: &str, kind: &str) {
    let painter = ui.painter_at(rect);
    painter.rect_stroke(rect.shrink(1.0), 0.0, (2.0, theme::accent()), egui::StrokeKind::Inside);
    let text = format!("Inside {name}, {kind}  ·  Esc or ‹ Back: the level above");
    let galley = painter.layout_no_wrap(text, egui::FontId::proportional(13.0), theme::on_accent());
    let tag = egui::Rect::from_min_size(rect.min, galley.size() + Vec2::new(16.0, 8.0));
    painter.rect_filled(tag, egui::CornerRadius { se: 6, ..Default::default() }, theme::accent());
    painter.galley(tag.min + Vec2::new(8.0, 4.0), galley, theme::on_accent());
}

/// Back up to the level `depth` of the containers entered (0: the whole model), the container left selected.
pub fn go_up(s: &mut Session, depth: usize) {
    if depth >= s.editor.path.len() {
        return;
    }
    let left = s.editor.path.get(depth).cloned();
    s.editor.path.truncate(depth);
    s.editor.chosen.clear();
    s.selected = left;
    s.fields.clear();
}

/// What the container shown is: a group, a loop, a node of the user's... (none at the model's top level).
pub fn container_kind(s: &Session) -> Option<String> {
    let (last, above) = s.editor.path.split_last()?;
    let n = s.source.scope(above)?.iter().find(|n| &n.id == last)?;
    let kind = match n.op.as_str() {
        _ if n.uses.is_some() => "a node of your own",
        "control.group" => "a group",
        "control.repeat" => "a loop, each step",
        "control.each" => "a loop over a collection, each element",
        "control.switch" => "a switch, its branches",
        "control.submodel" => "a submodel",
        _ => "a container",
    };
    Some(kind.to_string())
}

/// The bar above the canvas: where in the model, run up to the node, the preview kept up to date.
fn bar(ui: &mut Ui, s: &mut Session, r: &Registry) {
    // the model's file written by another program while this one has edits not saved
    if s.on_disk.is_some() {
        ui.horizontal(|ui| {
            ui.colored_label(ui.visuals().warn_fg_color, "The model's file was changed by another program.");
            if ui.button("Read it").on_hover_text("the model as the file has it now (ctrl+Z goes back)").clicked()
                && let Some(m) = s.on_disk.take()
            {
                crate::watch::take(s, m);
            }
            if ui
                .button("Keep mine")
                .on_hover_text("keep the model as it is here (saving it replaces the file)")
                .clicked()
            {
                s.on_disk = None;
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        // where in the model: inside a container, a button back up and its kind; the levels above, each a button
        // back to it; the level shown, in bold (on a second line what a narrow pane has no room for)
        let depth = s.editor.path.len();
        if depth > 0 {
            let up =
                egui::Button::new(RichText::new("‹ Back").color(theme::on_accent()).strong()).fill(theme::accent());
            if ui.add(up).on_hover_text("back to the level above (Esc on the canvas)").clicked() {
                go_up(s, depth - 1);
            }
        }
        let crumb = |ui: &mut Ui, text: &str, here: bool| {
            if here {
                ui.label(RichText::new(text).strong());
                false
            } else {
                ui.button(text).on_hover_text("back up to this level").clicked()
            }
        };
        if crumb(ui, "Whole model", depth == 0) {
            go_up(s, 0);
        }
        for k in 0..depth {
            // (the path cut short by a click this frame)
            let Some(name) = s.editor.path.get(k).cloned() else { break };
            ui.label(RichText::new("›").color(ui.visuals().weak_text_color()));
            if crumb(ui, &name, k + 1 == depth) {
                go_up(s, k + 1);
            }
        }
        if let Some(kind) = container_kind(s) {
            ui.label(RichText::new(format!("({kind})")).weak());
        }
        ui.separator();
        let target = runnable(s);
        let can = target.is_some() && s.can_run_to();
        if ui
            .add_enabled(can, egui::Button::new("Run to here"))
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
        if ui
            .add_enabled(s.history.can_undo(), egui::Button::new("Undo"))
            .on_hover_text("undo the last edit (ctrl+Z)")
            .clicked()
        {
            undo_redo(s, false);
        }
        if ui
            .add_enabled(s.history.can_redo(), egui::Button::new("Redo"))
            .on_hover_text("do the edit undone again (ctrl+shift+Z, ctrl+Y)")
            .clicked()
        {
            undo_redo(s, true);
        }
        ui.checkbox(&mut s.auto, "update after every edit")
            .on_hover_text("run up to the node shown after every change of the model");
        let label = if s.editor.text { "Diagram" } else { "Text" };
        if ui
            .add(egui::Button::new(label).selected(s.editor.text))
            .on_hover_text("the model as text (TOML), with the problems found in it; again: the diagram")
            .clicked()
        {
            s.editor.text = !s.editor.text;
        }
        if let Some(m) = &s.editor.message {
            ui.colored_label(ui.visuals().warn_fg_color, m);
        }
        // how far the node running is, as its operator tells it: a bar when it can say, else its words moving
        if let (Some((at, p)), None) = (&s.task, s.stopping) {
            crate::theme::progress(ui, p.fraction.map(|f| f as f32), &format!("{at}: {}", p.text), 160.0);
        }
    });
}

fn canvas(ui: &mut Ui, s: &mut Session, r: &Registry) {
    let shown = s.preview.clone();
    // the node running's share done, drawn along its bottom
    s.editor.progress = s.task.as_ref().and_then(|(at, p)| {
        let top = at.split('/').next().unwrap_or(at).to_string();
        p.fraction.map(|f| (top, f as f32))
    });
    // a file of the Files pane let go over the canvas: a node that reads it, there
    let over = ui.max_rect();
    if egui::DragAndDrop::has_payload_of_type::<DraggedFile>(ui.ctx()) {
        let pointer = ui.input(|i| i.pointer.latest_pos()).filter(|p| over.contains(*p));
        if let Some(p) = pointer {
            ui.painter().rect_stroke(over.shrink(2.0), 4.0, (2.0, theme::accent()), egui::StrokeKind::Inside);
            if ui.input(|i| i.pointer.any_released())
                && let Some(f) = egui::DragAndDrop::take_payload::<DraggedFile>(ui.ctx())
            {
                // a node each, one under the other from where they are let go
                let at = s.editor.graph_at(p).map(|q| [q.x - crate::editor_view::WIDTH / 2.0, q.y - 19.0]);
                for (k, file) in f.0.iter().enumerate() {
                    let at = at.map(|a| [a[0], a[1] + 150.0 * k as f32]);
                    if let Err(e) = add_file(s, r, file, at) {
                        s.editor.message = Some(e);
                    }
                }
            }
        }
    }
    let problems = problems(s);
    let layers = per_layer(s, r);
    let checks = crate::checks::badges(&crate::checks::of(s), &s.editor.path);
    // escape on the canvas, nothing selected (escape lets the nodes selected go first): back up a level
    let nothing_chosen = s.editor.chosen.is_empty();
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
        &problems,
        &layers,
        &checks,
    );
    if let (Some((last, _)), Some(rect)) = (s.editor.path.split_last(), s.editor.canvas()) {
        inside(ui, rect, last, container_kind(s).as_deref().unwrap_or("a container"));
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        let over = ui.input(|i| i.pointer.latest_pos()).is_some_and(|p| rect.contains(p));
        if nothing_chosen
            && over
            && !typing
            && s.editor.quick.is_none()
            && s.editor.renaming.is_none()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            go_up(s, s.editor.path.len() - 1);
        }
    }
    if s.source.nodes.is_empty() && s.editor.path.is_empty() {
        let c = ui.max_rect().center();
        ui.painter().text(
            c,
            egui::Align2::CENTER_CENTER,
            "Open a raster, click an operator of the library or press / to start; or begin from a template (Templates)",
            egui::FontId::proportional(15.0),
            ui.visuals().weak_text_color(),
        );
        let b = egui::Rect::from_center_size(c + Vec2::new(0.0, 36.0), Vec2::new(260.0, 26.0));
        if ui.put(b, egui::Button::new("Process a folder of rasters...")).on_hover_text(PROCESS_HELP).clicked() {
            process_folder_dialog(s);
        }
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
                if let Some((id, port)) = shown_at(s, &path, &id, &port) {
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
            // the canvas moved the node (or the box of something the scope reads) in its scene already: no new
            // layout
            Action::Move(id, p) if crate::editor::is_source(&id) => {
                crate::editor::place_source(&mut s.source, &path, &id, p);
                s.history.changed(&s.source);
                s.dirty = true;
                Ok(false)
            }
            Action::Move(id, p) => crate::editor::move_node(&mut s.source, &path, &id, p).map(|_| {
                s.history.changed(&s.source);
                s.dirty = true;
                false
            }),
            // resized by its edge (in the scene already), or back to the width its texts need (laid out again)
            Action::Resize(id, w) => crate::editor::resize_node(&mut s.source, &path, &id, w).map(|_| {
                s.history.changed(&s.source);
                s.dirty = true;
                w.is_none()
            }),
            Action::Connect { reference, node, port } => {
                crate::editor::connect(&mut s.source, r, &path, &reference, &node, &port).map(|_| {
                    // a node of the user's given what lacks a layer it reads: told at once
                    if let Some(n) = s.source.scope(&path).and_then(|v| v.iter().find(|n| n.id == node)).cloned()
                        && let Some((_, why)) =
                            crate::my_nodes::lacking(s, &path, &n).into_iter().find(|(k, _)| *k == port)
                    {
                        s.notice = Some((format!("{node}.{port}: {why}"), std::time::Instant::now()));
                    }
                    true
                })
            }
            Action::Disconnect { node, port } => {
                crate::editor::disconnect(&mut s.source, &path, &node, &port).map(|_| true)
            }
            Action::RunOnFolder(node) => match rfd::FileDialog::new()
                .set_title("The folder of the rasters")
                .pick_folder()
            {
                Some(folder) => crate::editor::run_on_folder(&mut s.source, r, &path, &node, &folder.to_string_lossy())
                    .map(|_| true),
                None => Ok(false),
            },
            Action::InputToNode(name) => crate::editor::input_to_node(&mut s.source, r, &name).map(|id| {
                s.selected = Some(id.clone());
                s.editor.chosen = [id].into();
                true
            }),
            Action::NodeToInput(id) => {
                let name = crate::editor::input_name(&s.source, &id);
                let bands = bands_of(s, &id);
                crate::editor::node_to_input(&mut s.source, r, &id, &name, bands).map(|_| {
                    let key = format!("${name}");
                    s.selected = Some(key.clone());
                    s.editor.chosen = [key].into();
                    true
                })
            }
            Action::RemoveSource(key) => crate::editor::remove_box(&mut s.source, &path, &key).map(|_| {
                s.selected = None;
                s.editor.chosen.clear();
                true
            }),
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

/// The problems of the boxes of the scope shown, by box: the checks' errors and warnings of each node (of its
/// body's too, for a container), those of the data of the model's inputs, and the run's error at the node that
/// failed. Whether each is an error, and what it says.
/// The top-level nodes that ran once per layer in the last run, with the number of layers: an output of one value
/// (a map, a number...) that gave a stack or a series.
fn per_layer(s: &Session, r: &Registry) -> std::collections::HashMap<String, usize> {
    if !s.editor.path.is_empty() {
        return Default::default();
    }
    s.source
        .nodes
        .iter()
        .filter_map(|n| {
            let (sig, out) = (r.sig(&n.op)?, s.outputs.get(&n.id)?);
            let many =
                sig.outputs.iter().filter(|p| !matches!(p.ty, Type::Stack | Type::Series | Type::Raster | Type::Any));
            many.into_iter()
                .find_map(|p| match out.get(&p.name)? {
                    Value::Stack(st) => Some(st.layers.len()),
                    Value::Series(v) => Some(v.len()),
                    _ => None,
                })
                .map(|k| (n.id.clone(), k))
        })
        .collect()
}

fn problems(s: &Session) -> std::collections::HashMap<String, Vec<(bool, String)>> {
    use terraflux_exec::lint::Level;
    let mut out: std::collections::HashMap<String, Vec<(bool, String)>> = Default::default();
    let prefix = if s.editor.path.is_empty() { String::new() } else { format!("{}/", s.editor.path.join("/")) };
    for i in s.shown_issues() {
        let head = i.at.split('.').next().unwrap_or("");
        if let Some(rest) = head.strip_prefix(&prefix) {
            let id = rest.split('/').next().unwrap_or(rest);
            let error = i.severity == terraflux_graph::Severity::Error;
            out.entry(id.to_string()).or_default().push((error, i.message.clone()));
        }
    }
    for f in s.findings.iter().filter(|f| f.level != Level::Info) {
        let input = f.at.split('/').next().unwrap_or(&f.at);
        out.entry(input.to_string()).or_default().push((f.level == Level::Error, f.message.clone()));
    }
    if s.editor.path.is_empty() {
        for (id, st) in &s.status {
            if *st == NodeStatus::Failed {
                let e = s.error.clone().unwrap_or_else(|| "it failed in the last run".into());
                out.entry(id.clone()).or_default().push((true, e));
            }
        }
    }
    out
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
    let mut shown = Shown {
        node,
        port,
        band: s.preview_band,
        chosen: s.preview_chosen,
        view: std::mem::take(&mut s.preview_view),
        overlays: std::mem::take(&mut s.preview_overlays),
        rasters: std::mem::take(&mut s.preview_rasters),
        compare: std::mem::take(&mut s.preview_compare),
    };
    let before = s.preview.clone();
    let hover = output(ui, s, r, &mut shown, "viewer");
    // (a node selected meanwhile, by what the viewer shows: that node's output shown now)
    if s.preview == before {
        s.preview = Some((shown.node, shown.port));
        s.preview_band = shown.band;
        s.preview_chosen = shown.chosen;
    }
    s.preview_view = shown.view;
    s.preview_overlays = shown.overlays;
    s.preview_rasters = shown.rasters;
    s.preview_compare = shown.compare;
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
    /// The vector layers drawn over the map: (node, port, style), the last on top.
    pub overlays: Vec<(String, String, crate::vector_tiles::Style)>,
    /// Other maps drawn over the map: (node, port, opacity), the last on top.
    pub rasters: Vec<(String, String, f32)>,
    /// Another map compared with this one.
    pub compare: Compare,
}

/// A comparison of the map shown with another: which, by a curtain or side by side (the other in a view of its
/// own following this one's).
#[derive(Default)]
pub struct Compare {
    pub with: Option<CompareWith>,
    pub side_by_side: bool,
    pub view: crate::map_view::MapView,
}

/// The map compared: another output of the run (node, port), or another layer of the one shown (of a stack, of
/// the steps of a loop, of rasters on disk).
#[derive(Debug, Clone, PartialEq)]
pub enum CompareWith {
    Output(String, String),
    Layer(usize),
}

/// How an output is named in the viewer: a raster read by its node's name, else `node.port`.
fn output_name(node: &str, port: &str) -> String {
    if port == "raster" { node.to_string() } else { format!("{node}.{port}") }
}

/// Two views side by side following each other: the one that moved since the last frame (`before`) moves the
/// other, through the place of the right one's grid on the left one's; the pointer over one marked in the other.
pub fn follow(
    left: &mut crate::map_view::MapView,
    right: &mut crate::map_view::MapView,
    place: crate::map_view::Place,
    before: [(f32, Vec2); 2],
) {
    let (l, r) = (left.camera(), right.camera());
    let scale = place.scale.x.abs().max(1e-12);
    if r != before[1] && l == before[0] {
        left.set_camera(r.0 / scale, place.to_view(r.1));
    } else {
        right.set_camera(l.0 * scale, place.to_source(l.1));
    }
    left.ghost = right.pointer().map(|p| place.to_view(p));
    right.ghost = left.pointer().map(|p| place.to_source(p));
}

/// A layer a plot stacks: a map (its reference and opacity) or vector features.
#[derive(Debug, Clone, PartialEq)]
pub enum PlotLayer {
    Map(String, String, f32),
    Features(String, String),
}

/// The layers of a plot, the lowest first: those of the plots down its `under` chain, then its own map and
/// features. An `under` fed by another node than a plot is a map at the bottom.
pub fn plot_stack(m: &terraflux_graph::Model, id: &str) -> Vec<PlotLayer> {
    let mut out = Vec::new();
    let mut seen = Vec::new();
    stack_of(m, id, &mut out, &mut seen);
    out
}

fn stack_of(m: &terraflux_graph::Model, id: &str, out: &mut Vec<PlotLayer>, seen: &mut Vec<String>) {
    if seen.iter().any(|s| s == id) {
        return;
    }
    seen.push(id.to_string());
    let Some(n) = m.nodes.iter().find(|n| n.id == id && n.op == "view.plot") else { return };
    let port_of = |name: &str| match n.inputs.get(name) {
        Some(terraflux_graph::Source::Ref(r)) => match terraflux_graph::Ref::parse(r) {
            Ok(terraflux_graph::Ref::Port { node, port }) => Some((node, port)),
            _ => None,
        },
        _ => None,
    };
    if let Some((node, port)) = port_of("under") {
        if m.nodes.iter().any(|x| x.id == node && x.op == "view.plot") {
            stack_of(m, &node, out, seen);
        } else {
            out.push(PlotLayer::Map(node, port, 1.0));
        }
    }
    let opacity = n.params.get("opacity").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    if let Some((node, port)) = port_of("value") {
        out.push(PlotLayer::Map(node, port, opacity));
    }
    if let Some((node, port)) = port_of("features") {
        out.push(PlotLayer::Features(node, port));
    }
}

/// The first map of a value as a source (a stack's first layer, rasters' chosen one).
fn first_source(v: &Value) -> Option<Source> {
    match v {
        Value::Map(m) => Some(Source::from(m)),
        Value::Stack(st) => st.layers.first().map(|l| Source::Map(Map { grid: st.grid.clone(), data: l.clone() })),
        Value::Raster(r) => Some(Source::Rasters(r.clone(), r.chosen)),
        Value::Series(items) => items.iter().find_map(first_source),
        _ => None,
    }
}

/// The map outputs of a session's run but one: (node, port), in the model's order.
fn map_values(s: &Session, r: &Registry, except: (&str, &str)) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for n in &s.model.nodes {
        for (p, ty) in crate::editor::output_ports(n, r) {
            if (n.id.as_str(), p.as_str()) == except || !matches!(ty, Type::Map | Type::Raster | Type::Stack) {
                continue;
            }
            if s.value_of(&n.id, &p).as_ref().and_then(first_source).is_some() {
                out.push((n.id.clone(), p));
            }
        }
    }
    out
}

/// The vector values of a session's run: (node, port, features), in the model's order.
pub fn vector_values(
    s: &Session,
    r: &Registry,
) -> Vec<(String, String, std::sync::Arc<terraflux_raster::vector::Vector>)> {
    let mut out = Vec::new();
    for n in &s.model.nodes {
        let Some(op) = r.get(&n.op) else { continue };
        for p in op.signature().outputs.iter().filter(|p| p.ty == terraflux_graph::Type::Vector) {
            if let Some(Value::Vector(v)) = s.value_of(&n.id, &p.name) {
                out.push((n.id.clone(), p.name.clone(), v));
            }
        }
    }
    out
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
    let from = s.source.nodes.iter().find(|n| n.id == node);
    let at = from.and_then(|n| n.pos).unwrap_or([0.0, 0.0]);
    let width = from.map_or(crate::editor_view::WIDTH, |n| crate::editor_view::node_width(n, r));
    let pos = [at[0] + width + 70.0, at[1] + 140.0];
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
    // a node of a group's body: named by its place in the group, its own outputs to choose from (the group's
    // `inner.node.port`)
    let inner = inner_of(s, &node, &port);
    let (title, op, ports): (String, String, Vec<(String, String)>) = match &inner {
        Some((n, place)) => (
            format!("{node}/{}", place.replace('.', "/")),
            n.op.clone(),
            crate::editor::output_ports(n, r).into_iter().map(|(p, _)| (format!("{place}.{p}"), p)).collect(),
        ),
        None => (
            node.clone(),
            s.model.nodes.iter().find(|n| n.id == node).map(|n| n.op.clone()).unwrap_or_default(),
            s.source
                .nodes
                .iter()
                .find(|n| n.id == node)
                .map(|n| crate::editor::output_ports(n, r))
                .unwrap_or_default()
                .into_iter()
                .map(|(p, _)| (p.clone(), p))
                .collect(),
        ),
    };
    let value = s.value_of(&node, &port);
    // the header: the node, its outputs, how it did
    egui::Frame::new().fill(theme::bar()).inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&title).strong());
            ui.label(RichText::new(&op).monospace().weak());
            // the node's outputs to choose from, when it has several
            if ports.len() > 1 {
                ui.separator();
                for (p, label) in &ports {
                    if ui.selectable_label(*p == port, label).clicked() {
                        sh.port = p.clone();
                        sh.band = 0;
                        sh.chosen = false;
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // what the rasters or features are (their coordinate system, cells, extent...)
                let has = value.as_ref().is_some_and(|v| !crate::metadata::facts(v).is_empty());
                let id = egui::Id::new(("metadata open", salt, &node, &port));
                let mut open: bool = ui.data(|d| d.get_temp(id)).unwrap_or(false);
                if ui
                    .add_enabled(has, egui::Button::new("Metadata").selected(open))
                    .on_hover_text("the coordinate system, the cells, the extent, the bands...")
                    .clicked()
                {
                    open = !open;
                }
                if open && let Some(v) = &value {
                    crate::metadata::window(ui.ctx(), &format!("{node}.{port}"), &crate::metadata::facts(v), &mut open);
                }
                ui.data_mut(|d| d.insert_temp(id, open));
                // what is shown, saved: a file only, or a file and a node that writes it at every run
                let maps = matches!(&value, Some(Value::Map(_) | Value::Stack(_) | Value::Raster(_)))
                    || matches!(&value, Some(Value::Series(v)) if v.iter().all(|x| matches!(x, Value::Map(_))));
                let can = value.as_ref().is_some_and(|v| !matches!(v, Value::Rng(_) | Value::NumpyRng(_)));
                ui.add_enabled_ui(can, |ui| {
                    ui.menu_button("Save output", |ui| {
                        if ui.button("As a file...").clicked() {
                            if let Some(p) = save_dialog(&node, maps, matches!(&value, Some(Value::Vector(_)))) {
                                s.save_output(&node, &port, &p);
                            }
                            ui.close();
                        }
                        // (a node of a group's body is not read from outside it: no write node)
                        if maps
                            && inner.is_none()
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
                // the cell clicked: why it has its value
                if let (Some((row, col)), Some(grid)) = (sh.view.picked, value.as_ref().and_then(crate::trace::grid_of))
                    && ui
                        .button("Trace this cell")
                        .on_hover_text(format!(
                            "why the cell at row {row}, column {col} has its value: the chain of values that made it"
                        ))
                        .clicked()
                {
                    s.trace_asked =
                        Some((node.clone(), port.clone(), grid, (row, col), crate::trace_view::Explain::Here));
                }
                // asked for from the map's menu (a right click on a cell)
                if let Some(explain) = sh.view.trace_request.take()
                    && let (Some(cell), Some(grid)) = (sh.view.picked, value.as_ref().and_then(crate::trace::grid_of))
                {
                    s.trace_asked = Some((node.clone(), port.clone(), grid, cell, explain));
                }
                ui.separator();
                let weak = ui.visuals().weak_text_color();
                match s.status.get(&node) {
                    Some(NodeStatus::Running) => {
                        ui.label(RichText::new("running").color(theme::accent()));
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
    // a plot over other plots: the lowest map shown, the others over it, the features over them all
    let stack = if op == "view.plot" { plot_stack(&s.model, &node) } else { Vec::new() };
    let mut over: Vec<(Source, f32, String, String)> = Vec::new();
    let mut plot_vectors: Vec<Arc<terraflux_raster::vector::Vector>> = Vec::new();
    let mut value = value;
    if stack.len() > 1 {
        let mut base = None;
        for l in &stack {
            match l {
                PlotLayer::Map(n, p, o) => match s.value_of(n, p) {
                    Some(v) if base.is_none() && first_source(&v).is_some() => base = Some(v),
                    Some(v) => {
                        if let Some(src) = first_source(&v) {
                            over.push((src, *o, n.clone(), p.clone()));
                        }
                    }
                    None => {}
                },
                PlotLayer::Features(n, p) => {
                    if let Some(Value::Vector(v)) = s.value_of(n, p) {
                        plot_vectors.push(v);
                    }
                }
            }
        }
        if let Some(b) = base {
            value = b;
        }
    }
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
            crate::vector_view::show(ui, v, &mut sh.view, egui::Id::new(("vector", salt, &node, &port)));
            return None;
        }
        if let Value::Woe(m) = &value {
            let m = m.clone();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                crate::woe_view::show(ui, s, &node, &m, salt);
            });
            return None;
        }
        if let Value::Table(t) = &value {
            // sorted, filtered and drawn as bars, the rows scrolled within
            let id = egui::Id::new(("table", salt, &node, &port));
            // a plot's table: drawn as its settings ask, from the start and whenever they change
            if let Some(n) = s.model.nodes.iter().find(|n| n.id == node && n.op == "view.plot") {
                let asked = serde_json::Value::Object(n.params.clone()).to_string();
                let key = id.with("plot settings");
                if ui.data(|d| d.get_temp::<String>(key)).as_ref() != Some(&asked) {
                    let st = crate::table_view::TableState { chart: plot_chart(n, t), ..Default::default() };
                    ui.data_mut(|d| {
                        d.insert_temp(id, st);
                        d.insert_temp(key, asked);
                    });
                }
            }
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
    let (mut made, mut hint) = producer(s, &node, &port);
    // rasters on disk that say what product they are (landschaft_product, as landschaft writes it)
    if made.is_none()
        && let Some(Source::Rasters(r, l)) = maps.get(sh.band.min(maps.len().saturating_sub(1)))
    {
        made = r.grid_of(*l).geo.item("landschaft_product").and_then(Product::from_tag);
    }
    hint.push_str(&port);
    if let Source::Rasters(r, l) = &maps[k] {
        hint.push(' ');
        hint.push_str(&r.layers[*l].name);
        hint.push(' ');
        hint.push_str(&r.images[r.layers[*l].image].path().to_string_lossy());
    }
    // a palette chosen by hand for this output, else by what it is
    let palette = s.palettes.get(&(node.clone(), port.clone())).copied().unwrap_or_else(|| Palette::of(made, &hint));
    let look = s.looks.get(ui.ctx(), &maps[k], palette);
    // the names and colours of the classes the model or the map gives
    let legend_classes = crate::legend_view::legend_of(s, &node, &port, &maps[k]);
    let look = match (&look, &legend_classes) {
        (Some(l), Some(c)) => Some(crate::legend_view::apply(l, c)),
        _ => look,
    };
    // the map compared with it: its source, its name and its look; the two in one colouring (the same value the
    // same colour on both sides), the legend that colouring's
    let main_name = if maps.len() > 1 { format!("{title} {}", names[k]) } else { title.clone() };
    let compared: Option<(Source, String, Option<crate::app::Look>)> = match &sh.compare.with {
        Some(CompareWith::Layer(j)) if *j < maps.len() => {
            let src = maps[*j].clone();
            let l = s.looks.get(ui.ctx(), &src, palette);
            Some((src, format!("{title} {}", names[*j]), l))
        }
        Some(CompareWith::Output(n, p)) => s.value_of(n, p).as_ref().and_then(first_source).map(|src| {
            let (made, mut hint) = producer(s, n, p);
            hint.push_str(p);
            let pal = s.palettes.get(&(n.clone(), p.clone())).copied().unwrap_or_else(|| Palette::of(made, &hint));
            let l = s.looks.get(ui.ctx(), &src, pal);
            let l = match (l, crate::legend_view::legend_of(s, n, p, &src)) {
                (Some(l), Some(c)) => Some(crate::legend_view::apply(&l, &c)),
                (l, _) => l,
            };
            (src, output_name(n, p), l)
        }),
        _ => None,
    };
    let shared = match (&look, &compared) {
        (Some(a), Some((_, _, Some(b)))) => Some(a.coloring.shared(&b.coloring)),
        _ => None,
    };
    let look = match (look, &shared) {
        (Some(l), Some(c)) => Some(crate::app::Look { coloring: c.clone(), ..l }),
        (l, _) => l,
    };
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
                    // the layers as an animation: the steps of a series in one colouring (a value the same colour
                    // at every step), the layers of a stack or of rasters each in its own, as shown
                    let series = matches!(value, Value::Series(_));
                    let file = title.replace(['/', '\\', ':', ' '], "_");
                    s.animations.button(ui, &file, if series { 200 } else { 600 }, || {
                        let (maps, legend) = (maps.clone(), legend_classes.clone());
                        let captions: Vec<String> = names.iter().map(|n| format!("{title} {n}")).collect();
                        move || {
                            let mut looks: Vec<Coloring> = maps
                                .iter()
                                .map(|m| {
                                    let l = look_of(m, palette);
                                    legend.as_ref().map_or(l.clone(), |c| crate::legend_view::apply(&l, c)).coloring
                                })
                                .collect();
                            if series && let Some(one) = looks.iter().cloned().reduce(|a, b| a.shared(&b)) {
                                looks = vec![one; maps.len()];
                            }
                            maps.into_iter()
                                .zip(looks)
                                .zip(captions)
                                .map(|((source, coloring), caption)| crate::animation::Frame {
                                    source,
                                    coloring,
                                    caption,
                                })
                                .collect()
                        }
                    });
                    // the cell clicked: its values along the layers (a click on the chart shows that layer)
                    if let Some((row, col)) = sh.view.picked {
                        let id = egui::Id::new(("series at", salt, &node, &port, row, col, maps.len()));
                        let values = match ui.data(|d| d.get_temp::<Arc<Vec<f64>>>(id)) {
                            Some(v) => v,
                            None => {
                                let v = Arc::new(series_at(&maps, row, col));
                                ui.data_mut(|d| d.insert_temp(id, v.clone()));
                                v
                            }
                        };
                        if let Some(at) = series_chart(ui, &values, &names, k, row, col, salt) {
                            b = at;
                        }
                    }
                    sh.band = b;
                }
                ui.add_space(4.0);
                match &look {
                    Some(l) => {
                        let named = legend_classes.as_deref();
                        // (the editor in the legend's place while it is open)
                        if !crate::legend_view::editing(ui, &node, &port) {
                            legend(ui, &l.coloring, &l.counts, &|v| crate::legend_view::name(l, named, v));
                        }
                        crate::legend_view::editor(ui, s, r, &node, &port, l, named);
                    }
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new("the legend...").weak());
                        });
                    }
                }
                // the cell clicked in every map of the run (at the same place, by their coordinates); a name
                // clicked shows that map
                if let Some((row, col)) = sh.view.picked {
                    ui.add_space(8.0);
                    theme::section(ui, "This cell in every map");
                    let mut all = vec![(node.clone(), port.clone())];
                    all.extend(map_values(s, r, (&node, &port)));
                    let rows = cell_everywhere(s, &all, &maps[k], row, col, salt, ui);
                    let mut show = None;
                    // (a row each: the value at the right, the name in what is left, cut short when long)
                    for (i, (name, v)) in rows.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(v).monospace().size(11.5));
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    let text = RichText::new(name).size(11.5);
                                    let text = if i == 0 { text.strong() } else { text };
                                    if i == 0 {
                                        ui.add(egui::Label::new(text).truncate());
                                    } else if ui
                                        .add(egui::Button::new(text).frame(false).truncate())
                                        .on_hover_text("show this map")
                                        .clicked()
                                    {
                                        show = Some(all[i].clone());
                                    }
                                });
                            });
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("Copy").on_hover_text("the names and values, a line each").clicked() {
                            let text: Vec<String> = rows.iter().map(|(n, v)| format!("{n}\t{v}")).collect();
                            ui.ctx().copy_text(format!("map\tvalue at row {row}, column {col}\n{}", text.join("\n")));
                        }
                        ui.label(RichText::new("click a name to show its map").weak().size(10.5));
                    });
                    if let Some((n, p)) = show {
                        sh.node = n;
                        sh.port = p;
                        sh.band = 0;
                        sh.chosen = false;
                    }
                }
                // the world under the data (web maps, by GDAL), the data over it
                ui.add_space(8.0);
                // the colours by hand (remembered for this output), or by what the values are
                ui.add_space(8.0);
                theme::section(ui, "Colours");
                let key = (node.clone(), port.clone());
                let chosen = s.palettes.get(&key).copied();
                let shown = match chosen {
                    Some(Palette::Chosen(r, log)) => {
                        let name = RAMPS.iter().find(|(_, x)| *x == r).map_or("ramp", |(n, _)| n);
                        if log { format!("{name}, logarithmic") } else { name.to_string() }
                    }
                    _ => "by what they are".to_string(),
                };
                let mut pick: Option<Option<Palette>> = None;
                egui::ComboBox::from_id_salt(("colours", salt)).selected_text(shown).width(180.0).show_ui(ui, |ui| {
                    if ui.selectable_label(chosen.is_none(), "by what they are").clicked() {
                        pick = Some(None);
                    }
                    for (name, r) in RAMPS {
                        for log in [false, true] {
                            let label = if log { format!("{name}, logarithmic") } else { name.to_string() };
                            if ui.selectable_label(chosen == Some(Palette::Chosen(r, log)), label).clicked() {
                                pick = Some(Some(Palette::Chosen(r, log)));
                            }
                        }
                    }
                });
                match pick {
                    Some(Some(p)) => {
                        s.palettes.insert(key, p);
                    }
                    Some(None) => {
                        s.palettes.remove(&key);
                    }
                    None => {}
                }
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
                // (the map hidden by space: here too, to show it again)
                if v.basemap == crate::map_view::Basemap::None && v.hide_data {
                    let mut show = false;
                    ui.checkbox(&mut show, "show the data").on_hover_text("space over the map: hide it, or show it");
                    v.hide_data = !show;
                }
                if v.basemap != crate::map_view::Basemap::None {
                    let mut show = !v.hide_data;
                    ui.checkbox(&mut show, "show the data").on_hover_text("space over the map: hide it, or show it");
                    v.hide_data = !show;
                    let mut opacity = 1.0 - v.transparency;
                    // (a slider no wider than the panel, its value and name beside it)
                    ui.spacing_mut().slider_width = (ui.available_width() - 110.0).clamp(40.0, 160.0);
                    ui.add_enabled(show, egui::Slider::new(&mut opacity, 0.0..=1.0).text("opacity"));
                    v.transparency = 1.0 - opacity;
                }
                // another map compared with this one: another output of the run, or another layer of this one
                ui.add_space(8.0);
                theme::section(ui, "Compare");
                let others = map_values(s, r, (&node, &port));
                let cmp = &mut sh.compare;
                let shown = match &cmp.with {
                    None => "nothing".to_string(),
                    Some(CompareWith::Layer(_)) => "another layer of this one".to_string(),
                    Some(CompareWith::Output(n, p)) => output_name(n, p),
                };
                egui::ComboBox::from_id_salt(("compare", salt))
                    .selected_text(shown)
                    .width((ui.available_width() - 8.0).clamp(80.0, 200.0))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut cmp.with, None, "nothing");
                        if maps.len() > 1 {
                            let first = if k == 0 { maps.len() - 1 } else { 0 };
                            let layer = matches!(cmp.with, Some(CompareWith::Layer(_)));
                            if ui.selectable_label(layer, "another layer of this one").clicked() && !layer {
                                cmp.with = Some(CompareWith::Layer(first));
                            }
                        }
                        for (mn, mp) in &others {
                            let with = Some(CompareWith::Output(mn.clone(), mp.clone()));
                            ui.selectable_value(&mut cmp.with, with, output_name(mn, mp));
                        }
                    })
                    .response
                    .on_hover_text("another map beside this one: a scenario, another date, the observed...");
                // (an output gone from the run, or a layer of a map that has fewer now: nothing compared)
                match &cmp.with {
                    Some(CompareWith::Layer(j)) if *j >= maps.len() || maps.len() < 2 => cmp.with = None,
                    Some(CompareWith::Output(n, p)) if !others.iter().any(|(a, b)| a == n && b == p) => cmp.with = None,
                    _ => {}
                }
                if let Some(CompareWith::Layer(j)) = &mut cmp.with {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("with").weak());
                        ui.spacing_mut().slider_width = (ui.available_width() - 70.0).clamp(40.0, 160.0);
                        ui.add(egui::Slider::new(j, 0..=maps.len() - 1).show_value(false));
                        ui.label(&names[*j]);
                    });
                }
                if cmp.with.is_some() {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut cmp.side_by_side, false, "Curtain")
                            .on_hover_text("the other map right of a line dragged across the map");
                        ui.selectable_value(&mut cmp.side_by_side, true, "Side by side")
                            .on_hover_text("two maps that move together, the pointer marked in both");
                    });
                    if shared.is_some() {
                        ui.label(
                            RichText::new("both in the same colours: the classes of both, or one range")
                                .weak()
                                .size(11.0),
                        );
                    }
                }
                // other maps of the run over this one (placed by their coordinates), each as opaque as asked
                ui.add_space(8.0);
                theme::section(ui, "Maps over it");
                if others.is_empty() {
                    ui.label(RichText::new("no other map of the run").weak().size(11.0));
                }
                for (mn, mp) in &others {
                    let at = sh.rasters.iter().position(|(a, b, _)| a == mn && b == mp);
                    let mut on = at.is_some();
                    // (a raster read: by its node's name)
                    let label = if mp == "raster" { mn.clone() } else { format!("{mn}.{mp}") };
                    if ui.checkbox(&mut on, label).changed() {
                        match at {
                            Some(i) if !on => {
                                sh.rasters.remove(i);
                            }
                            None if on => sh.rasters.push((mn.clone(), mp.clone(), 0.7)),
                            _ => {}
                        }
                    }
                    if let Some(i) = sh.rasters.iter().position(|(a, b, _)| a == mn && b == mp) {
                        ui.indent(("over", mn, mp), |ui| {
                            ui.spacing_mut().slider_width = (ui.available_width() - 110.0).clamp(40.0, 160.0);
                            ui.add(egui::Slider::new(&mut sh.rasters[i].2, 0.0..=1.0).text("opacity"));
                        });
                    }
                }
                // vector layers of the run over the map (reprojected to it), each in its style
                ui.add_space(8.0);
                theme::section(ui, "Vectors over the map");
                let available = vector_values(s, r);
                if available.is_empty() {
                    ui.label(RichText::new("no vector of the run").weak().size(11.0));
                }
                for (vn, vp, vec) in &available {
                    let at = sh.overlays.iter().position(|(a, b, _)| a == vn && b == vp);
                    let mut on = at.is_some();
                    let label = if vp == "vector" { vn.clone() } else { format!("{vn}.{vp}") };
                    if ui.checkbox(&mut on, label).on_hover_text(format!("{} features", vec.len())).changed() {
                        match at {
                            Some(i) if !on => {
                                sh.overlays.remove(i);
                            }
                            None if on => sh.overlays.push((vn.clone(), vp.clone(), Default::default())),
                            _ => {}
                        }
                    }
                    if let Some(i) = sh.overlays.iter().position(|(a, b, _)| a == vn && b == vp) {
                        ui.indent((vn, vp), |ui| {
                            crate::vector_view::style_controls(
                                ui,
                                vec,
                                &mut sh.overlays[i].2,
                                egui::Id::new((vn, vp, salt)),
                            );
                        });
                    }
                }
            });
        });
    let mut overlays: Vec<crate::vector_tiles::Overlay> = plot_vectors
        .into_iter()
        .map(|v| crate::vector_tiles::Overlay { vector: v, style: Default::default() })
        .collect();
    overlays.extend(sh.overlays.iter().filter_map(|(n, p, style)| match s.value_of(n, p) {
        Some(Value::Vector(v)) => Some(crate::vector_tiles::Overlay { vector: v, style: style.clone() }),
        _ => None,
    }));
    // the maps over it: the plot's under ones, then those chosen; each in the colours of what it is
    for (n, p, o) in &sh.rasters {
        if let Some(src) = s.value_of(n, p).as_ref().and_then(first_source) {
            over.push((src, *o, n.clone(), p.clone()));
        }
    }
    let rasters: Vec<crate::map_view::RasterLayer> = over
        .into_iter()
        .filter_map(|(src, opacity, n, p)| {
            let (made, mut hint) = producer(s, &n, &p);
            hint.push_str(&p);
            let palette = s.palettes.get(&(n.clone(), p.clone())).copied().unwrap_or_else(|| Palette::of(made, &hint));
            let look = s.looks.get(ui.ctx(), &src, palette)?;
            Some(crate::map_view::RasterLayer { source: src, coloring: look.coloring, opacity })
        })
        .collect();
    let mut hover = None;
    // the map compared, in the colouring of both (none while its look is being made)
    let other = match (&compared, &shared) {
        (Some((src, name, Some(_))), Some(c)) => Some((src.clone(), name.clone(), c.clone())),
        (Some((src, name, Some(l))), None) => Some((src.clone(), name.clone(), l.coloring.clone())),
        _ => None,
    };
    egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::well())).show(ui, |ui| {
        let Some(l) = &look else {
            ui.centered_and_justified(|ui| ui.spinner());
            return;
        };
        match other {
            None => hover = sh.view.show_stacked(ui, &maps[k], &l.coloring, &rasters, &overlays),
            Some((src, name, coloring)) if !sh.compare.side_by_side => {
                let layer = crate::map_view::RasterLayer { source: src, coloring, opacity: 1.0 };
                let curtain = crate::map_view::Curtain { layer, names: [main_name.clone(), name] };
                hover = sh.view.show_compared(ui, &maps[k], &l.coloring, &rasters, &overlays, Some(&curtain));
            }
            Some((src, name, coloring)) => {
                hover = side_by_side(
                    ui,
                    sh,
                    (&maps[k], &l.coloring, &main_name),
                    (&src, &coloring, &name),
                    &rasters,
                    &overlays,
                );
            }
        }
    });
    // the cell of the map compared side by side, traced from its menu: of the output it is, or of the layer
    if let Some(explain) = sh.compare.view.trace_request.take()
        && let Some(cell) = sh.compare.view.picked
    {
        let (n, p) = match &sh.compare.with {
            Some(CompareWith::Output(n, p)) => (n.clone(), p.clone()),
            _ => (node.clone(), port.clone()),
        };
        if let Some(grid) = s.value_of(&n, &p).as_ref().and_then(crate::trace::grid_of) {
            s.trace_asked = Some((n, p, grid, cell, explain));
        }
    }
    hover
}

/// Two maps side by side, each in a view of its own, the views moving together (the pointer over one marked in
/// the other) and each named at its top. What is under the pointer, over either.
fn side_by_side(
    ui: &mut Ui,
    sh: &mut Shown,
    left: (&Source, &Coloring, &str),
    right: (&Source, &Coloring, &str),
    rasters: &[crate::map_view::RasterLayer],
    overlays: &[crate::vector_tiles::Overlay],
) -> Option<Hover> {
    let Some(place) = crate::map_view::place_on(right.0.grid(), left.0.grid()) else {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new(
                    "The map compared is of another coordinate system, or rotated: put it on this map's grid \
                     (raster.match) to see them side by side.",
                )
                .color(ui.visuals().error_fg_color),
            );
        });
        return None;
    };
    let full = ui.available_rect_before_wrap();
    let gap = 4.0;
    let half = (full.width() - gap) / 2.0;
    let a = Rect::from_min_size(full.min, Vec2::new(half, full.height()));
    let b = Rect::from_min_size(egui::pos2(a.right() + gap, full.top()), Vec2::new(half, full.height()));
    // (the one beside sees the world under it as this one does)
    let (basemap, transparency, hide) = (sh.view.basemap, sh.view.transparency, sh.view.hide_data);
    let v = &mut sh.compare.view;
    (v.basemap, v.transparency, v.hide_data) = (basemap, transparency, hide);
    let before = [sh.view.camera(), sh.compare.view.camera()];
    let mut hover = None;
    for (rect, (src, coloring, name), view) in [(a, left, &mut sh.view), (b, right, &mut sh.compare.view)] {
        let h = ui
            .scope_builder(egui::UiBuilder::new().max_rect(rect).id_salt(name), |ui| {
                let h = view.show_stacked(ui, src, coloring, rasters, overlays);
                let painter = ui.painter_at(rect);
                let galley = painter.layout_no_wrap(name.to_string(), egui::FontId::proportional(12.0), Color32::WHITE);
                let at = rect.left_top() + Vec2::new(10.0, 8.0);
                painter.rect_filled(
                    Rect::from_min_size(at, galley.size()).expand2(Vec2::new(5.0, 2.0)),
                    3.0,
                    Color32::from_black_alpha(160),
                );
                painter.galley(at, galley, Color32::WHITE);
                h
            })
            .inner;
        hover = hover.or(h);
    }
    ui.painter().rect_filled(
        Rect::from_min_size(egui::pos2(a.right(), full.top()), Vec2::new(gap, full.height())),
        0.0,
        theme::bar(),
    );
    follow(&mut sh.view, &mut sh.compare.view, place, before);
    hover
}

/// A cell of the map shown (`base`, its row and column) in every map of `all` (node, port), at the same place by
/// their coordinates: each map's name and the value there ("no value", "outside", "another coordinate system").
/// Kept while the cell and the maps are the same (a raster on disk is read a cell at a time).
fn cell_everywhere(
    s: &Session,
    all: &[(String, String)],
    base: &Source,
    row: usize,
    col: usize,
    salt: &str,
    ui: &Ui,
) -> Arc<Vec<(String, String)>> {
    // each map's source (a stack's first layer, rasters' chosen one), which layer it is and of how many
    let sources: Vec<Option<(Source, usize, usize)>> = all
        .iter()
        .map(|(n, p)| {
            let v = s.value_of(n, p)?;
            let (layer, layers) = match &v {
                Value::Stack(st) => (0, st.layers.len()),
                Value::Raster(r) => (r.chosen, r.layers.len()),
                Value::Series(items) => (0, items.len()),
                _ => (0, 1),
            };
            Some((first_source(&v)?, layer, layers))
        })
        .collect();
    let id = egui::Id::new((
        "cell everywhere",
        salt,
        row,
        col,
        base.key(),
        sources.iter().map(|x| x.as_ref().map(|(src, _, _)| src.key())).collect::<Vec<_>>(),
    ));
    if let Some(rows) = ui.data(|d| d.get_temp::<Arc<Vec<(String, String)>>>(id)) {
        return rows;
    }
    let at = Vec2::new(col as f32 + 0.5, row as f32 + 0.5);
    let rows: Vec<(String, String)> = all
        .iter()
        .zip(&sources)
        .map(|((n, p), src)| {
            let mut name = output_name(n, p);
            let value = match src {
                None => "not run".to_string(),
                Some((src, layer, layers)) => {
                    if *layers > 1 {
                        name.push_str(&format!(" (layer {} of {layers})", layer + 1));
                    }
                    match crate::map_view::place_on(src.grid(), base.grid()) {
                        None => "another coordinate system".to_string(),
                        Some(place) => {
                            let c = place.to_source(at);
                            let (w, h) = src.size();
                            if c.x < 0.0 || c.y < 0.0 || c.x >= w as f32 || c.y >= h as f32 {
                                "outside".to_string()
                            } else {
                                let v = series_at(std::slice::from_ref(src), c.y as usize, c.x as usize)[0];
                                if v.is_nan() { "no value".to_string() } else { terraflux_ops::value::class_label(v) }
                            }
                        }
                    }
                }
            };
            (name, value)
        })
        .collect();
    let rows = Arc::new(rows);
    ui.data_mut(|d| d.insert_temp(id, rows.clone()));
    rows
}

/// A cell's values along maps (NaN where it has none): of maps in memory at once, of rasters on disk a cell's
/// window of each layer.
pub fn series_at(maps: &[Source], row: usize, col: usize) -> Vec<f64> {
    maps.iter()
        .map(|m| match m {
            Source::Map(map) => {
                let w = map.grid.width;
                if col < w && row < map.grid.height { map.data[row * w + col] } else { f64::NAN }
            }
            Source::Rasters(r, l) => {
                let win = terraflux_raster::tiff::Window { x0: col as i64, y0: row as i64, width: 1, height: 1 };
                r.window(*l, win).ok().and_then(|v| v.first().copied()).unwrap_or(f64::NAN)
            }
            Source::Blank(_) => f64::NAN,
        })
        .collect()
}

/// The chart of a cell's values along the layers (by their names when they are numbers, as years are; else
/// their order), the layer shown marked; the layer clicked on it.
fn series_chart(
    ui: &mut Ui,
    values: &[f64],
    names: &[String],
    shown: usize,
    row: usize,
    col: usize,
    salt: &str,
) -> Option<usize> {
    use egui_plot::{Line, Plot, Points, VLine};
    ui.add_space(6.0);
    theme::section(ui, &format!("row {row}, column {col}"));
    let numbers: Option<Vec<f64>> = names.iter().map(|n| n.trim().parse::<f64>().ok()).collect();
    let xs: Vec<f64> = match numbers {
        Some(x) if x.len() == values.len() => x,
        _ => (0..values.len()).map(|i| i as f64).collect(),
    };
    let pts: Vec<[f64; 2]> = xs.iter().zip(values).filter(|(_, v)| !v.is_nan()).map(|(x, v)| [*x, *v]).collect();
    let mut clicked = None;
    let weak = ui.visuals().weak_text_color();
    let resp = crate::theme::plot(Plot::new(("series", salt)))
        .height(150.0)
        .allow_scroll(false)
        .show_background(false)
        .show(ui, |p| {
            p.line(Line::new("values", pts.clone()).color(theme::accent()).width(1.8));
            p.points(Points::new("", pts.clone()).color(theme::accent()).radius(2.5));
            if let Some(x) = xs.get(shown) {
                p.vline(VLine::new("shown", *x).color(weak));
            }
            if p.response().clicked()
                && let Some(c) = p.pointer_coordinate()
            {
                clicked = xs
                    .iter()
                    .enumerate()
                    .min_by(|a, b| (a.1 - c.x).abs().total_cmp(&(b.1 - c.x).abs()))
                    .map(|(i, _)| i);
            }
        });
    resp.response.on_hover_text("the cell's value in every layer; click to show a layer");
    clicked
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

/// A rule of a reclassification as the table shows it: the values it takes (`from`: a value, values, `*`, or
/// where a range starts), where the range ends (`to`, empty for values), the value they get (`becomes`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuleRow {
    pub from: String,
    pub to: String,
    pub becomes: String,
}

impl RuleRow {
    pub fn new(from: &str, to: &str, becomes: &str) -> Self {
        RuleRow { from: from.into(), to: to.into(), becomes: becomes.into() }
    }

    fn is_empty(&self) -> bool {
        self.from.trim().is_empty() && self.to.trim().is_empty() && self.becomes.trim().is_empty()
    }
}

/// The rules of a reclassification as rows, from its text (a rule per line or `;`): `from -> becomes`, a range
/// `a..b` its own `from` and `to`; a line of three fields without `->` (`0 10 2`) is from, to, becomes. Lines
/// of tabs are rows as they are being written ([`rule_draft`]), their fields as typed.
pub fn rule_rows(text: &str) -> Vec<RuleRow> {
    let mut rows = Vec::new();
    for line in text.split('\n') {
        if line.contains('\t') {
            let mut f = line.splitn(3, '\t');
            let mut next = || f.next().unwrap_or("").to_string();
            rows.push(RuleRow { from: next(), to: next(), becomes: next() });
            continue;
        }
        for l in line.split(';').map(str::trim).filter(|l| !l.is_empty()) {
            let fields: Vec<&str> = l.split([' ', ',']).map(str::trim).filter(|f| !f.is_empty()).collect();
            rows.push(match l.split_once("->") {
                Some((a, b)) => {
                    let (a, b) = (a.trim(), b.trim());
                    match a.split_once("..") {
                        Some((x, y)) if !a.contains([' ', ',']) => RuleRow::new(x, y, b),
                        _ => RuleRow::new(a, "", b),
                    }
                }
                None if fields.len() == 3 => RuleRow::new(fields[0], fields[1], fields[2]),
                None => RuleRow::new(l, "", ""),
            });
        }
    }
    rows
}

/// The text of rows of rules (rows with nothing in them left out): `from -> becomes`, or `from..to -> becomes`
/// when the row has a `to` (`-inf..to` without a `from`: every value up to it).
pub fn rule_text(rows: &[RuleRow]) -> String {
    rows.iter()
        .filter(|r| !r.is_empty())
        .map(|r| {
            let (from, to, becomes) = (r.from.trim(), r.to.trim(), r.becomes.trim());
            match (from.is_empty(), to.is_empty()) {
                (_, true) => format!("{from} -> {becomes}"),
                (true, false) => format!("-inf..{to} -> {becomes}"),
                (false, false) => format!("{from}..{to} -> {becomes}"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Rows as they are being written (empty ones too, their fields as typed): a line of tabs each.
fn rule_draft(rows: &[RuleRow]) -> String {
    rows.iter().map(|r| format!("{}\t{}\t{}", r.from, r.to, r.becomes)).collect::<Vec<_>>().join("\n")
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
    egui::Frame::new()
        .fill(theme::well())
        .corner_radius(crate::theme::rounded(4.0))
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
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
                                    RichText::new(format!(
                                        "{} ({:.1}%)",
                                        number(*v),
                                        100.0 * cells as f64 / total as f64
                                    ))
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
                        let taken: Vec<String> =
                            rows.iter().filter(|r| r.to.trim().is_empty()).map(|r| r.from.trim().to_string()).collect();
                        for (v, _) in classes {
                            let t = number(*v);
                            if !taken.contains(&t) {
                                rows.push(RuleRow::new(&t, "", &t));
                            }
                        }
                        commit = true;
                    }
                    }
                    Coloring::Continuous { min, max, .. } => {
                        ui.label(
                            RichText::new(format!(
                                "The map's values go from {} to {}: take them by ranges (from, to).",
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
                            rows.retain(|r| !r.is_empty());
                            for k in 0..4 {
                                let (a, b) = (min + w * k as f64, if k == 3 { *max } else { min + w * (k + 1) as f64 });
                                rows.push(RuleRow::new(&number(a), &number(b), &(k + 1).to_string()));
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
    // the three fields share the pane's width ("->", the button and the gaps aside)
    let w = ((ui.available_width() - 70.0) / 3.4).clamp(40.0, 110.0);
    egui::Grid::new(("rules", &n.id)).num_columns(5).spacing([6.0, 4.0]).show(ui, |ui| {
        ui.label(RichText::new("from").size(11.0).color(weak)).on_hover_text(
            "the value the rule takes (1 2 3: several; *: every other value), or where its range starts",
        );
        ui.label(RichText::new("to").size(11.0).color(weak))
            .on_hover_text("where the range ends (both ends in); empty: only the values in from");
        ui.label("");
        ui.label(RichText::new("becomes").size(11.0).color(weak)).on_hover_text("the new value (nan: no data)");
        ui.end_row();
        for (k, r) in rows.iter_mut().enumerate() {
            let a = ui.add(egui::TextEdit::singleline(&mut r.from).desired_width(w * 1.4).hint_text("1 2 3 or *"));
            let b = ui.add(egui::TextEdit::singleline(&mut r.to).desired_width(w).hint_text("end"));
            ui.label("->");
            let c = ui.add(egui::TextEdit::singleline(&mut r.becomes).desired_width(w).hint_text("value"));
            if ui.small_button("x").on_hover_text("take this rule away").clicked() {
                gone = Some(k);
            }
            commit |= a.lost_focus() || b.lost_focus() || c.lost_focus();
            ui.end_row();
        }
    });
    if let Some(k) = gone {
        rows.remove(k);
        commit = true;
    }
    ui.horizontal(|ui| {
        if ui.button("+ a rule").clicked() {
            rows.push(RuleRow::default());
        }
        if ui.button("+ every other value").on_hover_text("a rule `* -> 0`: what no rule above takes").clicked() {
            rows.push(RuleRow::new("*", "", "0"));
            commit = true;
        }
    });
    egui::CollapsingHeader::new(RichText::new("How to write the values").size(11.5))
        .id_salt(("rules help", &n.id))
        .show(ui, |ui| {
            egui::Grid::new(("rules examples", &n.id)).num_columns(2).spacing([12.0, 2.0]).show(ui, |ui| {
                ui.label(RichText::new("from | to | becomes").size(11.0).color(weak));
                ui.label("");
                ui.end_row();
                for (rule, says) in [
                    ("3 |    | 1", "3 becomes 1"),
                    ("1 2 3 |    | 1", "1, 2 and 3 become 1"),
                    ("0 | 10 | 2", "from 0 to 10 (both in, decimals too) become 2"),
                    ("700 | inf | 3", "700 and above become 3"),
                    ("* |    | 0", "every value no rule above takes becomes 0"),
                    ("5 |    | nan", "5 becomes no data"),
                ] {
                    ui.label(RichText::new(rule).monospace());
                    ui.label(RichText::new(says).size(11.5));
                    ui.end_row();
                }
            });
            ui.label(
                RichText::new(
                    "For MSPA, say: the forest's classes become 2 and * becomes 1 (the background); cells without a \
                     value stay missing. In the model's file a range is written 0..10 -> 2.",
                )
                .size(11.0)
                .color(weak),
            );
        });
    // the rows as they are being written (empty ones too), until the next change of the model
    s.fields.insert(key, rule_draft(&rows));
    let new = rule_text(&rows);
    (commit && new != committed).then(|| crate::editor::set_param(&mut s.source, path, &n.id, "rules", &new))
}

/// A setting of a few values in a list: the default (when it may be left out), then its values (yes or no for a
/// setting of that type).
pub fn choice_param(
    ui: &mut Ui,
    s: &mut Session,
    n: &Node,
    path: &[String],
    spec: &terraflux_graph::PortSpec,
    current: &str,
) -> Option<Result<(), String>> {
    let options: Vec<String> =
        if spec.choices.is_empty() { vec!["true".into(), "false".into()] } else { spec.choices.clone() };
    let mut out = None;
    let shown = if current.is_empty() { "default".to_string() } else { current.to_string() };
    egui::ComboBox::from_id_salt(("param", &n.id, &spec.name))
        .selected_text(shown)
        .width(ui.available_width() - 8.0)
        .show_ui(ui, |ui| {
            if !spec.required
                && ui.selectable_label(current.is_empty(), "default").on_hover_text(&spec.doc).clicked()
                && !current.is_empty()
            {
                out = Some(crate::editor::set_param(&mut s.source, path, &n.id, &spec.name, ""));
            }
            for o in &options {
                if ui.selectable_label(current == o, o).clicked() && current != o {
                    out = Some(crate::editor::set_param(&mut s.source, path, &n.id, &spec.name, o));
                }
            }
        })
        .response
        .on_hover_text(&spec.doc);
    out
}

/// The metrics of a landscape metrics node (its setting `param`): all of its level (the default), or some, chosen
/// by type (area and edge, shape, core area...) or one by one. The signatures' lists of landscape and class
/// metrics are none by default.
pub fn metrics_param(
    ui: &mut Ui,
    s: &mut Session,
    n: &Node,
    path: &[String],
    param: &str,
) -> Option<Result<(), String>> {
    use terraflux_ops::lsm_catalog::METRICS;
    let level = match (n.op.as_str(), param) {
        (_, "landscape_metrics") => "landscape",
        (_, "class_metrics") => "class",
        ("metrics.patch", _) => "patch",
        ("metrics.class", _) => "class",
        ("metrics.sample" | "metrics.series", _) => {
            n.params.get("level").and_then(|v| v.as_str()).unwrap_or("landscape")
        }
        _ => "landscape",
    };
    // "all" when none is chosen, but for the signatures' lists
    let empty = if param == "metrics" { "all" } else { "none" };
    let named: Vec<String> = match n.params.get(param) {
        Some(serde_json::Value::String(m)) => vec![m.clone()],
        Some(serde_json::Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        _ => Vec::new(),
    };
    // the names as the catalogue has them (lsm_c_ca is ca)
    let chosen: Vec<&'static str> =
        named.iter().filter_map(|m| terraflux_ops::metrics::find(level, m).map(|x| x.metric)).collect();
    let of_level: Vec<_> = METRICS.iter().filter(|m| m.level == level).collect();
    let mut kinds: Vec<&str> = Vec::new();
    for m in &of_level {
        if !kinds.contains(&m.kind) {
            kinds.push(m.kind);
        }
    }
    let shown = match chosen.len() {
        0 => empty.to_string(),
        1..=4 => chosen.join(", "),
        k => format!("{k} metrics"),
    };
    let mut set: Vec<&'static str> = chosen.clone();
    let mut changed = false;
    egui::ComboBox::from_id_salt((param, &n.id))
        .selected_text(shown)
        .width(ui.available_width() - 8.0)
        .height(460.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            let mut all = set.is_empty();
            let hint = if param == "metrics" { "every metric of the level" } else { "no metric of the level" };
            if ui.checkbox(&mut all, RichText::new(empty).strong()).on_hover_text(hint).changed() && all {
                set.clear();
                changed = true;
            }
            ui.separator();
            for kind in &kinds {
                let of: Vec<&'static str> = of_level.iter().filter(|m| m.kind == *kind).map(|m| m.metric).collect();
                let k = of.iter().filter(|m| set.contains(m)).count();
                let id = ui.make_persistent_id(("metrics kind", &n.id, param, kind));
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                    .show_header(ui, |ui| {
                        let mut on = k == of.len();
                        let label = format!("{} ({k}/{})", kind.trim_end_matches(" metric"), of.len());
                        if ui.add(egui::Checkbox::new(&mut on, label).indeterminate(k > 0 && k < of.len())).changed() {
                            set.retain(|m| !of.contains(m));
                            if on {
                                set.extend(&of);
                            }
                            changed = true;
                        }
                    })
                    .body(|ui| {
                        for m in of_level.iter().filter(|m| m.kind == *kind) {
                            let mut on = set.contains(&m.metric);
                            let extra = if m.extra { " (FRAGSTATS)" } else { "" };
                            if ui.checkbox(&mut on, format!("{} · {}{extra}", m.metric, m.name)).changed() {
                                set.retain(|x| *x != m.metric);
                                if on {
                                    set.push(m.metric);
                                }
                                changed = true;
                            }
                        }
                    });
            }
        });
    if !changed {
        return None;
    }
    // in the catalogue's order
    let ordered: Vec<&str> = of_level.iter().map(|m| m.metric).filter(|m| set.contains(m)).collect();
    let text = if ordered.is_empty() { String::new() } else { serde_json::to_string(&ordered).unwrap_or_default() };
    Some(crate::editor::set_param(&mut s.source, path, &n.id, param, &text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_as_rows_from_to_becomes() {
        let rows = rule_rows("1 2 -> 10\n0..10 -> 2; * -> 0\n700 inf 3\n-5..-1 -> nan");
        assert_eq!(rows[0], RuleRow::new("1 2", "", "10"));
        assert_eq!(rows[1], RuleRow::new("0", "10", "2"));
        assert_eq!(rows[2], RuleRow::new("*", "", "0"));
        assert_eq!(rows[3], RuleRow::new("700", "inf", "3"));
        assert_eq!(rows[4], RuleRow::new("-5", "-1", "nan"));
        assert_eq!(rule_text(&rows), "1 2 -> 10\n0..10 -> 2\n* -> 0\n700..inf -> 3\n-5..-1 -> nan");
        // no from: every value up to the end; empty rows left out
        let rows = [RuleRow::new("", "10", "1"), RuleRow::default()];
        assert_eq!(rule_text(&rows), "-inf..10 -> 1");
        // the rows being written come back as typed, empty ones and spaces too
        let rows = vec![RuleRow::new("1 ", "", ""), RuleRow::default(), RuleRow::new("0", "5", "2")];
        assert_eq!(rule_rows(&rule_draft(&rows)), rows);
    }

    #[test]
    fn views_side_by_side_follow_each_other() {
        use crate::map_view::{MapView, Place};
        // the right map's cells twice as large, starting 10 cells right of the left one's
        let place = Place { offset: Vec2::new(10.0, 0.0), scale: Vec2::splat(2.0) };
        let (mut l, mut r) = (MapView::default(), MapView::default());
        // the left one moved: the right one follows (the same place, its cells shown twice as large)
        let before = [l.camera(), r.camera()];
        l.set_camera(4.0, Vec2::new(30.0, 20.0));
        follow(&mut l, &mut r, place, before);
        assert_eq!(r.camera(), (8.0, Vec2::new(10.0, 10.0)));
        // the right one moved: the left one follows
        let before = [l.camera(), r.camera()];
        r.set_camera(2.0, Vec2::new(0.0, 0.0));
        follow(&mut l, &mut r, place, before);
        assert_eq!(l.camera(), (1.0, Vec2::new(10.0, 0.0)));
        // nothing moved: as they were
        let before = [l.camera(), r.camera()];
        follow(&mut l, &mut r, place, before);
        assert_eq!([l.camera(), r.camera()], before);
    }
}
