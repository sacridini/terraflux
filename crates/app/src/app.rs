//! The application: a model open (its graph, its data checked), its run with the maps drawn as they come, the
//! maps of its inputs (a stack's one after the other), the area of each class along the steps, and the
//! validation against an observed map.
//!
//! What takes long is done in threads: the data is read and its colours and areas worked out while the window
//! keeps answering, the areas of the simulated maps are counted in the run's thread, and the maps' textures are
//! kept to be shown again at once.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{Color32, Margin, RichText, Ui, Vec2};
use egui_dock::{DockState, NodeIndex};
use egui_plot::{Legend, Line, Plot};
use terraflux_exec::Cache;
use terraflux_exec::lint::{Finding, Level, lint};
use terraflux_graph::{Input, Issue, Model, Severity, Source, check};
use terraflux_ops::validate::{Agreement, agreement, similarity_of_differences};
use terraflux_ops::{Map, Registry, Table, Value};
use terraflux_raster::Raster;

use crate::builder::{Library, Looks};
use crate::editor_view::{EditorState, NodeStatus};
use crate::map_view::{Hover, MapView, Thumbs, class_counts, legend};
use crate::palette::Coloring;
use crate::runner::{Gate, Message, Record, Run};
use crate::theme;
use crate::wizard::{Outcome, Wizard};

/// A pane of the application: a tab of the dock, moved, split, closed (the Panels menu brings it back) or taken
/// to a window of its own (another screen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Pane {
    /// The model's graph.
    Model,
    /// The model read as a document: its stages and steps, in words, with the maps of the last run.
    Document,
    /// The operators' library.
    Operators,
    /// The computer's folders: data dragged to the model.
    Files,
    /// The node selected.
    Node,
    /// The data of the node selected put on the reference grid, or made drivers, in a click.
    Prepare,
    /// What the node selected gives.
    Viewer,
    /// The run's maps, step by step, and the inputs' maps.
    Simulation,
    /// The area of each class along the steps.
    Areas,
    Validation,
    /// The model run many times: replicates, scenarios, calibration.
    Batch,
    /// Other computers helping this one, this one helping another.
    Computers,
    /// A chat with a model that explains, changes and runs the model open.
    Assistant,
    /// The review of the model's changes (an assistant's, or the differences with another version).
    Review,
    /// The model's parameters as a form, and its scenarios saved, applied and compared.
    Parameters,
    /// Why a cell of a result has its value: the chain of values that made it.
    Trace,
    /// What is wrong with the model: a run's failure explained, the graph's checks, the data's.
    Problems,
    /// The script of the map algebra node selected: written with its names completed, its outputs previewed, and
    /// anything computed in its scope.
    Calculator,
    /// A node's output in a tab of its own (opened by a double click on the node; one per node, brought to the
    /// front when it is open already).
    Map(u64),
}

impl Pane {
    pub const ALL: [Pane; 18] = [
        Pane::Model,
        Pane::Document,
        Pane::Operators,
        Pane::Files,
        Pane::Node,
        Pane::Prepare,
        Pane::Viewer,
        Pane::Simulation,
        Pane::Areas,
        Pane::Validation,
        Pane::Batch,
        Pane::Computers,
        Pane::Assistant,
        Pane::Review,
        Pane::Parameters,
        Pane::Trace,
        Pane::Problems,
        Pane::Calculator,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Pane::Model => "Model",
            Pane::Document => "Document",
            Pane::Operators => "Operators",
            Pane::Files => "Files",
            Pane::Node => "Node",
            Pane::Prepare => "Prepare",
            Pane::Viewer => "Viewer",
            Pane::Simulation => "Simulation",
            Pane::Areas => "Areas",
            Pane::Validation => "Validation",
            Pane::Batch => "Many runs",
            Pane::Computers => "Computers",
            Pane::Assistant => "Assistant",
            Pane::Review => "Changes",
            Pane::Parameters => "Parameters",
            Pane::Trace => "Trace",
            Pane::Problems => "Problems",
            Pane::Calculator => "Calculator",
            Pane::Map(_) => "Map",
        }
    }
}

/// The panes as they first are: the model in most of the window (its document in a tab beside it), the node
/// selected at its left as an inspector (the preparation of its data in a tab beside it), nothing at the right
/// (the assistant goes there when it opens); the others from the View menu.
/// The viewer opens in a tab behind the model's when the first data comes (a raster or vectors read, a template);
/// the maps of nodes open beside the viewer. The operators and the files are in drawers at the window's left edge
/// (the operators also in the Operators menu, behind "/" and in the commands, ctrl+K); their panes, shown, take
/// the left edge.
pub fn default_layout() -> DockState<Pane> {
    // the inspector a fifth of the window
    const NODE: f32 = 0.2;
    let mut d = DockState::new(vec![Pane::Model, Pane::Document]);
    let s = d.main_surface_mut();
    // (split_left: the new node's share)
    let [_, _] = s.split_left(NodeIndex::root(), NODE, vec![Pane::Node, Pane::Prepare]);
    d
}

/// The panes kept that `keep` says to keep, and the floating windows left with none taken away (egui_dock's
/// own `retain_tabs` leaves such a window, a tree of one empty node, and panics drawing it). A leaf whose tab in
/// front was taken away shows its last (egui_dock leaves the index past its tabs: nothing is drawn there).
fn retain_panes(dock: &mut DockState<Pane>, keep: impl FnMut(&mut Pane) -> bool) {
    dock.retain_tabs(keep);
    for (_, leaf) in dock.iter_leaves_mut() {
        let n = leaf.tabs().len();
        if n > 0 && leaf.active.0 >= n {
            leaf.active = egui_dock::TabIndex(n - 1);
        }
    }
    let empty: Vec<_> = dock
        .iter_surfaces_indexed()
        .filter(|(i, s)| !i.is_main() && s.node_tree().is_some_and(|t| t.num_tabs() == 0))
        .map(|(i, _)| i)
        .collect();
    for i in empty.into_iter().rev() {
        dock.remove_surface(i);
    }
}

/// The version of the layout kept (a layout of another version is let go).
const LAYOUT_VERSION: u32 = 11;

/// A window of its own (another screen): its panes in a dock.
pub struct Detached {
    pub id: egui::ViewportId,
    pub dock: DockState<Pane>,
    /// Where it opens on the screen (where its tab was dropped), the first time.
    pub at: Option<egui::Pos2>,
}

/// The layout kept between sessions: the main dock, and the panes of each window of their own.
#[derive(serde::Serialize, serde::Deserialize)]
struct Layout {
    #[serde(default)]
    version: u32,
    main: DockState<Pane>,
    windows: Vec<Vec<Pane>>,
}

/// Where the layout is kept (the user's application data; on macOS `~/Library/Application Support`).
pub fn layout_file() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| {
            let home = PathBuf::from(std::env::var_os("HOME")?);
            Some(if cfg!(target_os = "macos") {
                home.join("Library").join("Application Support")
            } else {
                home.join(".config")
            })
        })
        .map(|d| d.join("Terraflux").join("layout.json"))
}

/// The application's keys pressed in a window.
#[derive(Default)]
struct Keys {
    run: bool,
    save: bool,
    save_as: bool,
    open: bool,
    open_model: bool,
    new: bool,
    commands: bool,
    shortcuts: bool,
    tab: bool,
    close: bool,
}

impl Keys {
    /// The keys of the window `ui` is in, taken.
    fn read(ui: &mut Ui) -> Keys {
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        let shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        // (with shift first: a shortcut without it takes the key with shift too)
        ui.input_mut(|i| Keys {
            run: i.consume_key(egui::Modifiers::NONE, egui::Key::F5),
            save_as: i.consume_key(shift, egui::Key::S),
            save: i.consume_key(egui::Modifiers::COMMAND, egui::Key::S),
            open_model: i.consume_key(shift, egui::Key::O),
            open: i.consume_key(egui::Modifiers::COMMAND, egui::Key::O),
            new: i.consume_key(egui::Modifiers::COMMAND, egui::Key::N),
            commands: i.consume_key(shift, egui::Key::P) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::K),
            shortcuts: i.consume_key(egui::Modifiers::NONE, egui::Key::F1),
            tab: !typing && i.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
            close: i.consume_key(egui::Modifiers::COMMAND, egui::Key::W),
        })
    }
}

/// What the panes asked of the dock in a frame.
#[derive(Default)]
struct Moves {
    /// to a window of their own
    detach: Vec<Pane>,
    /// back to the main window
    attach: Vec<Pane>,
}

/// The dock's view of the application: each pane drawn by the application.
struct Panes<'a> {
    app: &'a mut TerrafluxApp,
    moves: &'a mut Moves,
    /// whether these panes are in a window of their own
    detached: bool,
}

impl egui_dock::TabViewer for Panes<'_> {
    type Tab = Pane;

    fn id(&mut self, tab: &mut Pane) -> egui::Id {
        egui::Id::new(("pane", *tab))
    }

    fn title(&mut self, tab: &mut Pane) -> egui::WidgetText {
        match tab {
            Pane::Map(id) => self
                .app
                .session
                .as_ref()
                .and_then(|s| s.map_tabs.get(id))
                .map_or("Map".to_string(), |m| {
                    // a plot's title, else the node and the output
                    let title = self.app.session.as_ref().and_then(|s| {
                        let n = s.source.nodes.iter().find(|n| n.id == m.node)?;
                        (n.op == "view.plot").then(|| n.params.get("title")?.as_str().map(String::from)).flatten()
                    });
                    title.unwrap_or_else(|| format!("{} · {}", m.node, m.port))
                })
                .into(),
            t => t.title().into(),
        }
    }

    fn on_close(&mut self, tab: &mut Pane) -> egui_dock::widgets::tab_viewer::OnCloseResponse {
        if let Pane::Map(id) = tab
            && let Some(s) = &mut self.app.session
        {
            s.map_tabs.remove(id);
        }
        egui_dock::widgets::tab_viewer::OnCloseResponse::Close
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Pane) {
        self.app.pane(ui, *tab);
    }

    fn scroll_bars(&self, tab: &Pane) -> [bool; 2] {
        // (the panes that scroll by themselves: the Files pane's list, what is chosen described at its bottom; the
        // texts of the assistant, the document, a trace, the parameters and the changes, which wrap at the pane's
        // sides only without a scroll of the dock around them)
        let own = [
            Pane::Files,
            Pane::Assistant,
            Pane::Document,
            Pane::Trace,
            Pane::Parameters,
            Pane::Review,
            Pane::Problems,
            Pane::Calculator,
        ];
        [!own.contains(tab); 2]
    }

    fn context_menu(&mut self, ui: &mut Ui, tab: &mut Pane, _path: egui_dock::NodePath) {
        if self.detached {
            if ui.button("Back to the main window").clicked() {
                self.moves.attach.push(*tab);
                ui.close();
            }
        } else if ui.button("Open in a window of its own").on_hover_text("to take it to another screen").clicked() {
            self.moves.detach.push(*tab);
            ui.close();
        }
    }
}

/// The tabs of a dock's floating windows (tabs dragged out of it), taken out of it: a list per window.
fn floating_tabs(dock: &mut DockState<Pane>) -> Vec<Vec<Pane>> {
    let surfaces: Vec<egui_dock::SurfaceIndex> =
        dock.iter_surfaces_indexed().filter(|(i, s)| !i.is_main() && !s.is_empty()).map(|(i, _)| i).collect();
    let mut out = Vec::new();
    for i in surfaces.into_iter().rev() {
        let tabs: Vec<Pane> = dock[i].tabs().copied().collect();
        dock.remove_surface(i);
        if !tabs.is_empty() {
            out.push(tabs);
        }
    }
    out
}

/// The dock's look: the theme's greys, the tab shown in the panel's grey, the bar a shade darker.
fn dock_style(ui: &Ui) -> egui_dock::Style {
    let mut st = egui_dock::Style::from_egui(ui.style().as_ref());
    st.tab_bar.bg_fill = theme::bar();
    st.tab_bar.hline_color = theme::bar();
    st.tab_bar.height = 24.0;
    st.tab.active.bg_fill = ui.visuals().panel_fill;
    st.tab.focused.bg_fill = ui.visuals().panel_fill;
    st.tab.focused.text_color = theme::accent();
    st.tab.active.text_color = ui.visuals().strong_text_color();
    st.main_surface_border_stroke = egui::Stroke::NONE;
    if theme::modern() {
        modern_dock(&mut st);
    }
    st
}

/// The modern look of the dock: each pane a rounded card on the bar's colour, its tabs rounded at the top, the
/// others' text weak until the pointer is over them.
fn modern_dock(st: &mut egui_dock::Style) {
    let p = theme::current();
    let top = egui::CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 };
    st.tab_bar.height = 30.0;
    st.tab.spacing = 2.0;
    let t = &mut st.tab;
    for s in [
        &mut t.active,
        &mut t.inactive,
        &mut t.focused,
        &mut t.hovered,
        &mut t.active_with_kb_focus,
        &mut t.inactive_with_kb_focus,
        &mut t.focused_with_kb_focus,
    ] {
        s.corner_radius = top;
        s.outline_color = Color32::TRANSPARENT;
    }
    t.inactive.bg_fill = p.bar;
    t.inactive.text_color = p.weak;
    t.inactive_with_kb_focus.bg_fill = p.bar;
    t.hovered.bg_fill = p.panel.lerp_to_gamma(p.bar, 0.5);
    t.hovered.text_color = p.text;
    t.tab_body.corner_radius = egui::CornerRadius { nw: 0, ne: 8, sw: 8, se: 8 };
    t.tab_body.stroke = egui::Stroke::NONE;
    t.tab_body.bg_fill = p.panel;
    // the gutters between the panes, in the bar's colour
    st.separator.width = 6.0;
    st.separator.color_idle = p.bar;
    st.separator.color_hovered = p.hover;
    st.separator.color_dragged = p.accent;
    st.buttons.close_tab_color = p.weak;
    st.buttons.close_tab_active_color = p.text;
    st.buttons.close_tab_bg_fill = p.hover;
    st.dock_area_padding = Some(Margin { left: 6, right: 6, top: 0, bottom: 6 });
}

/// Comparing the last simulated map with an observed one.
#[derive(Default)]
pub struct Validation {
    pub observed: Option<PathBuf>,
    pub bands: Vec<String>,
    pub band: usize,
    pub windows: String,
    pub agreement: Option<Agreement>,
    pub similarity: Option<Table>,
    pub error: Option<String>,
}

/// What a map looks like: its colours and, for classes, the cells of each.
#[derive(Debug, Clone)]
pub struct Look {
    pub coloring: Coloring,
    pub counts: Vec<(f64, usize)>,
    /// The product of GuidosToolbox the classes are (named so in the legend).
    pub product: Option<terraflux_ops::landscape::Product>,
}

impl Look {
    /// The look of a map's cells; its classes in the colours of `base` when `base` has them all (a map of the
    /// same classes, as the land use of another date).
    pub fn of(values: &[f64], base: Option<&Coloring>) -> Look {
        let mut coloring = Coloring::for_values(values);
        if let (Some(b @ Coloring::Classes(known)), Coloring::Classes(own)) = (base, &coloring)
            && own.iter().all(|(v, _)| known.iter().any(|(k, _)| k == v))
        {
            coloring = coloring.like(b);
        }
        let counts = if matches!(coloring, Coloring::Classes(_)) { class_counts(values) } else { Vec::new() };
        Look { coloring, counts, product: None }
    }
}

/// An input the map tab shows: one map, or the maps of a stack to go through.
pub struct Layer {
    /// `$name`
    pub name: String,
    pub stack: bool,
    /// The name of each map, the map and its look.
    pub bands: Vec<(String, Map, Look)>,
    /// The map shown.
    pub shown: usize,
}

/// A model's data, read in a thread: its values, what is wrong with them, the map the simulation starts from
/// and the maps of the inputs, with their looks.
pub struct Loaded {
    /// The model's inputs the values are of.
    pub of: BTreeMap<String, Input>,
    pub inputs: Result<BTreeMap<String, Value>, String>,
    pub findings: Vec<Finding>,
    pub start: Option<(Map, Look)>,
    pub layers: Vec<Layer>,
}

impl Loaded {
    pub fn read(model: &Model, base: &Path) -> Loaded {
        let of = model.inputs.clone();
        let inputs = match terraflux_exec::load_inputs(model, base, &BTreeMap::new()) {
            Ok(i) => i,
            Err(e) => {
                return Loaded {
                    of,
                    inputs: Err(e.to_string()),
                    findings: Vec::new(),
                    start: None,
                    layers: Vec::new(),
                };
            }
        };
        let findings = lint(model, &inputs);
        let start = start_map(model, &inputs).map(|m| {
            let look = Look::of(&m.data, None);
            (m, look)
        });
        let classes = start.as_ref().map(|(_, l)| &l.coloring);
        let layers = inputs
            .iter()
            .filter_map(|(name, v)| {
                let bands = match v {
                    Value::Map(m) => {
                        let band = match model.inputs.get(name) {
                            Some(Input::Map { band: Some(b), .. }) => b.clone(),
                            _ => "band 1".into(),
                        };
                        vec![(band, m.clone(), Look::of(&m.data, classes))]
                    }
                    Value::Stack(st) => st
                        .keys
                        .iter()
                        .zip(&st.layers)
                        .map(|(k, d)| {
                            (k.to_string(), Map { grid: st.grid.clone(), data: d.clone() }, Look::of(d, classes))
                        })
                        .collect(),
                    _ => return None,
                };
                Some(Layer { name: format!("${name}"), stack: matches!(v, Value::Stack(_)), bands, shown: 0 })
            })
            .collect();
        Loaded { of, inputs: Ok(inputs), findings, start, layers }
    }
}

/// The map a simulation starts from: the first value of a loop's carried `landscape`, else `$start` or `$t0`.
pub fn start_map(model: &Model, inputs: &BTreeMap<String, Value>) -> Option<Map> {
    for n in &model.nodes {
        if n.body.is_some()
            && let Some(Source::Ref(r)) = n.inputs.get("landscape")
            && let Some(Value::Map(m)) = r.strip_prefix('$').and_then(|k| inputs.get(k))
        {
            return Some(m.clone());
        }
    }
    ["start", "t0"].iter().find_map(|k| match inputs.get(*k) {
        Some(Value::Map(m)) => Some(m.clone()),
        _ => None,
    })
}

/// A model open.
pub struct Session {
    /// The model as written (what the editor changes and saves).
    pub source: Model,
    /// The model run: its submodels expanded.
    pub model: Model,
    /// The model run as text (the model tab).
    pub toml: Option<String>,
    /// The editor's state; whether the model changed since it was saved.
    pub editor: EditorState,
    pub dirty: bool,
    /// The texts being typed in the side panel, by field.
    pub fields: BTreeMap<String, String>,
    /// The model's file (none: not saved yet).
    pub path: Option<PathBuf>,
    pub issues: Vec<Issue>,
    /// Whether the nodes not filled in yet are told among the problems: once a run is asked for, until every node is
    /// filled in.
    pub unfilled_shown: bool,
    pub findings: Vec<Finding>,
    pub inputs: Option<BTreeMap<String, Value>>,
    /// The model's inputs `inputs` are of (a run reads them again when they changed).
    inputs_of: BTreeMap<String, Input>,
    pub load_error: Option<String>,
    loading: Option<Receiver<Loaded>>,
    pub run: Option<Run>,
    pub progress: Option<(String, usize, usize)>,
    /// The top-level nodes the run goes through, and those done (run or taken from the cache).
    pub run_total: usize,
    pub run_done: usize,
    /// A cell's trace asked for (a map's cell clicked, then Trace this cell).
    pub trace_asked: Option<crate::trace_view::Ask>,
    /// The window of the model's methods and citations open.
    pub methods_open: bool,
    /// The window of the export of the run's maps to a QGIS project.
    pub qgis: crate::qgis::Export,
    /// The data catalog's window (the datasets data.download fetches).
    pub catalog: crate::catalog::Catalog,
    /// The window of the model as a Python script of landschaft.
    pub python: Option<crate::python::View>,
    /// A figure being composed of the map shown (the Viewer's Figure...).
    pub figure: Option<crate::figure::Composer>,
    /// The Problems pane asked for (why a node failed).
    pub ask_problems: bool,
    /// The calculator asked for (the script of the node selected).
    pub ask_calculator: bool,
    /// How far the node running is, as its operator tells it (a solver's gap...).
    pub task: Option<(String, terraflux_ops::Progress)>,
    /// When Stop was pressed, while the run stops.
    pub stopping: Option<Instant>,
    pub node: String,
    /// The map the simulation starts from, then the map of every step; the cells of each class of each.
    pub maps: Vec<Map>,
    pub counts: Vec<Vec<(f64, usize)>>,
    pub tables: Vec<Table>,
    pub shown: usize,
    pub follow: bool,
    pub warnings: Vec<String>,
    /// The files of a folder that failed in the last run, by the chain of nodes they ran through (its first node's
    /// path): each file's name and why.
    pub failures: BTreeMap<String, Vec<(String, String)>>,
    pub error: Option<String>,
    pub elapsed: Option<Duration>,
    /// What the last whole run wrote: its manifest (`output/manifest.json`, with the outputs), or why not.
    pub recorded: Option<Result<PathBuf, String>>,
    pub started: Option<Instant>,
    pub view: MapView,
    pub thumbs: Thumbs,
    /// The colours of the simulated maps.
    pub coloring: Option<Coloring>,
    /// The inputs' maps; the one shown (none: the simulation).
    pub layers: Vec<Layer>,
    pub layer: Option<usize>,
    /// Going through the steps or maps of the layer by itself.
    pub playing: bool,
    tick: Instant,
    pub selected: Option<String>,
    pub validation: Validation,
    /// The outputs of the nodes run, kept between runs (a run only runs what changed).
    pub cache: Arc<Mutex<Cache>>,
    /// Step by step: the runs pause after each step of a loop (Step in the status bar), until Next or Continue.
    pub gate: Arc<Gate>,
    /// How each top-level node did in the last run.
    pub status: BTreeMap<String, NodeStatus>,
    /// How long each node took when it last ran (its place: `node`, `container/inner`; a loop's body summed over its
    /// steps), and about how much memory what it gave holds.
    pub timings: BTreeMap<String, (f64, usize)>,
    /// Counters of the changes of the model and of the nodes' states (the canvas is laid out again then).
    pub revision: u64,
    pub status_rev: u64,
    /// The node a run stops at (none: the whole model).
    pub target: Option<String>,
    /// What every top-level node gave in the runs so far (the cache keeps only what may be used again).
    pub outputs: BTreeMap<String, terraflux_ops::Outputs>,
    /// The output shown under the canvas: node and port; the band or step of a stack or series.
    pub preview: Option<(String, String)>,
    pub preview_band: usize,
    /// Whether the band chosen of the rasters shown was shown first already.
    pub preview_chosen: bool,
    pub preview_view: MapView,
    /// The palettes chosen by hand for outputs shown: (node, port).
    pub palettes: std::collections::HashMap<(String, String), crate::builder::Palette>,
    /// The vector layers drawn over the maps shown (they stay as other nodes are shown).
    pub preview_overlays: Vec<(String, String, crate::vector_tiles::Style)>,
    /// The maps drawn over the map shown: (node, port, opacity).
    pub preview_rasters: Vec<(String, String, f32)>,
    /// The map compared with the map shown (by a curtain or side by side).
    pub preview_compare: crate::builder::Compare,
    /// The animations (GIF) of steps or of a stack's maps being made, and what the last one said.
    pub animations: crate::animation::Animations,
    pub looks: Looks,
    /// The outputs shown in map tabs of their own (a double click on a node), by tab; the tabs the dock is still
    /// to show.
    pub map_tabs: BTreeMap<u64, crate::builder::Shown>,
    next_tab: u64,
    pub opened: Vec<u64>,
    /// The map tabs to bring to the front (asked for again while open).
    pub raised: Vec<u64>,
    /// The map tabs opened by a run (its plots): they do not take the front from the model's tab.
    pub quiet: Vec<u64>,
    /// Outputs being saved (in threads): where, and what came of it; a word for the status bar and since when.
    saving: Vec<(PathBuf, Receiver<Result<PathBuf, String>>)>,
    pub notice: Option<(String, Instant)>,
    /// Run up to the node shown after every edit; when the last edit was.
    pub auto: bool,
    /// Run the whole model once its data is read.
    pub run_when_ready: bool,
    edited_at: Option<Instant>,
    /// The output whose grid the model's maps are put on (node, port).
    pub reference: Option<(String, String)>,
    /// What the values of nodes' rasters are, as chosen in the preparation (else guessed from their type).
    pub kinds: BTreeMap<String, crate::prepare::Kind>,
    /// The edits of the source, to undo and do again.
    pub history: crate::undo::History,
    /// The model's file followed ([`crate::watch`]); the model another program wrote to it while this one had
    /// edits not saved (to read it or not).
    pub watch: Option<crate::watch::Watch>,
    pub on_disk: Option<Model>,
    /// The model as its file has it (read or saved last).
    pub saved: Option<Model>,
    /// The review of the changes going on (an assistant's, or the differences with another version).
    pub review: Option<crate::review::Review>,
}

impl Session {
    /// Read a model and check it; its data is read in a thread (see [`Session::loading`]).
    pub fn open(path: &Path) -> Result<Session, String> {
        let (source, notes) = Model::load_noting(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut s = Session::new(source, Some(path.to_path_buf()))?;
        // map algebra of the old syntax, rewritten: told, and to be saved
        if !notes.is_empty() {
            s.dirty = true;
            s.editor.message = Some(format!(
                "{} written in map algebra's old syntax, rewritten in Python's (save the model to keep it): {}",
                if notes.len() == 1 { "A setting was".to_string() } else { format!("{} settings were", notes.len()) },
                notes.join("; ")
            ));
        }
        Ok(s)
    }

    /// A model without nodes, not saved yet.
    pub fn blank() -> Session {
        let mut m = Model::new("Untitled");
        m.name = Some("Untitled".into());
        Session::new(m, None).expect("an empty model")
    }

    /// A model to start from, not saved yet.
    pub fn template(t: &crate::templates::Template) -> Result<Session, String> {
        Session::from_model(t.model())
    }

    /// A session of a model not saved yet (a template's).
    pub fn from_model(m: Model) -> Result<Session, String> {
        let mut s = Session::new(m, None)?;
        s.dirty = true;
        Ok(s)
    }

    fn new(mut source: Model, path: Option<PathBuf>) -> Result<Session, String> {
        // (a model made in code, as a template's, brought up to date as a model read is)
        source.upgrade();
        let base = path.as_deref().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default();
        let shown = path.as_ref().map_or("the model".into(), |p| p.display().to_string());
        let mut model = source.clone();
        terraflux_graph::submodel::expand(&mut model, &base).map_err(|e| format!("{shown}: {e}"))?;
        let issues = check(&model, &Registry::standard());
        let (tx, rx) = std::sync::mpsc::channel();
        let m = model.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Loaded::read(&m, &base));
        });
        let reference = crate::prepare::reference_of(&source);
        let saved = path.as_ref().map(|_| source.clone());
        Ok(Session {
            saved,
            history: crate::undo::History::new(&source),
            toml: None,
            reference,
            kinds: BTreeMap::new(),
            source,
            model,
            editor: EditorState::default(),
            dirty: false,
            fields: BTreeMap::new(),
            path,
            issues,
            unfilled_shown: false,
            findings: Vec::new(),
            inputs: None,
            inputs_of: BTreeMap::new(),
            load_error: None,
            loading: Some(rx),
            run: None,
            progress: None,
            run_total: 0,
            run_done: 0,
            trace_asked: None,
            methods_open: false,
            qgis: Default::default(),
            catalog: Default::default(),
            python: None,
            figure: None,
            ask_problems: false,
            ask_calculator: false,
            task: None,
            stopping: None,
            node: String::new(),
            maps: Vec::new(),
            counts: Vec::new(),
            tables: Vec::new(),
            shown: 0,
            follow: true,
            warnings: Vec::new(),
            failures: BTreeMap::new(),
            error: None,
            elapsed: None,
            recorded: None,
            started: None,
            view: MapView::default(),
            thumbs: Thumbs::default(),
            coloring: None,
            layers: Vec::new(),
            layer: None,
            playing: false,
            tick: Instant::now(),
            selected: None,
            validation: Validation { windows: "1, 3, 5, 7, 9, 11".into(), ..Default::default() },
            cache: Arc::new(Mutex::new(Cache::default())),
            gate: Arc::new(Gate::default()),
            status: BTreeMap::new(),
            timings: BTreeMap::new(),
            revision: 0,
            status_rev: 0,
            target: None,
            outputs: BTreeMap::new(),
            preview: None,
            preview_band: 0,
            preview_chosen: false,
            preview_view: MapView::default(),
            preview_overlays: Vec::new(),
            preview_rasters: Vec::new(),
            preview_compare: Default::default(),
            animations: Default::default(),
            palettes: Default::default(),
            looks: Looks::default(),
            auto: true,
            edited_at: None,
            map_tabs: BTreeMap::new(),
            next_tab: 0,
            opened: Vec::new(),
            raised: Vec::new(),
            quiet: Vec::new(),
            run_when_ready: false,
            saving: Vec::new(),
            notice: None,
            watch: None,
            on_disk: None,
            review: None,
        })
    }

    /// Read the model's data again, in a thread (its inputs changed).
    pub fn read_data(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let (m, base) = (self.model.clone(), self.base());
        std::thread::spawn(move || {
            let _ = tx.send(Loaded::read(&m, &base));
        });
        self.loading = Some(rx);
    }

    /// The folder the model's paths are relative to.
    pub fn base(&self) -> PathBuf {
        self.path.as_deref().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default()
    }

    /// The model's name, else its file's.
    pub fn title(&self) -> String {
        self.model
            .name
            .clone()
            .unwrap_or_else(|| self.path.as_ref().map_or("Untitled".into(), |p| p.display().to_string()))
    }

    /// Whether the data is still being read.
    pub fn loading(&self) -> bool {
        self.loading.is_some()
    }

    /// Wait for the data.
    pub fn wait(&mut self) {
        if let Some(rx) = self.loading.take() {
            match rx.recv() {
                Ok(l) => self.loaded(l),
                Err(_) => self.load_error = Some("the data could not be read".into()),
            }
        }
    }

    fn loaded(&mut self, l: Loaded) {
        self.inputs_of = l.of;
        match l.inputs {
            Ok(i) => {
                self.inputs = Some(i);
                self.load_error = None;
            }
            Err(e) => {
                self.inputs = None;
                self.load_error = Some(e);
            }
        }
        self.findings = l.findings;
        if let Some((m, look)) = l.start {
            self.coloring = Some(look.coloring);
            self.maps = vec![m];
            self.counts = vec![look.counts];
        }
        self.layers = l.layers;
        if self.maps.is_empty() && !self.layers.is_empty() {
            self.layer = Some(0);
        }
    }

    /// After an edit of the source: the model run and its checks follow.
    pub fn edited(&mut self) {
        self.history.changed(&self.source);
        self.dirty = true;
        self.fields.clear();
        self.edited_at = Some(Instant::now());
        self.revision += 1;
        // what the nodes gave may be out of date until they run again
        if !self.running() {
            self.status.clear();
            self.status_rev += 1;
        }
        let mut m = self.source.clone();
        match terraflux_graph::submodel::expand(&mut m, &self.base()) {
            Ok(()) => {
                self.issues = check(&m, &Registry::standard());
                self.unfilled_shown &= self.issues.iter().any(|i| i.unfilled);
                self.toml = None;
                self.model = m;
            }
            Err(e) => self.editor.message = Some(e.to_string()),
        }
    }

    /// Write the model where it was read.
    pub fn save(&mut self) -> Result<(), String> {
        let path = self.path.clone().ok_or("the model has no file yet")?;
        self.save_as(&path)
    }

    /// Write the model to a file, the paths of its files relative to the file's folder; the model is that
    /// file's from then on.
    pub fn save_as(&mut self, path: &Path) -> Result<(), String> {
        let dir = path.parent().unwrap_or(Path::new(""));
        terraflux_graph::relative_paths(&mut self.source.nodes, dir);
        if self.path.is_none() && self.source.name.as_deref() == Some("Untitled") {
            self.source.name = path.file_stem().map(|s| s.to_string_lossy().into_owned());
        }
        self.source.save(path).map_err(|e| e.to_string())?;
        self.revision += 1;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        self.saved = Some(self.source.clone());
        self.on_disk = None;
        let mut m = self.source.clone();
        if terraflux_graph::submodel::expand(&mut m, dir).is_ok() {
            self.toml = None;
            self.model = m;
        }
        Ok(())
    }

    pub fn running(&self) -> bool {
        self.run.is_some()
    }

    pub fn can_run(&self) -> bool {
        !self.running()
            && !self.loading()
            && self.load_error.is_none()
            && !self.issues.iter().any(|i| i.severity == Severity::Error)
            && !self.findings.iter().any(|f| f.level == Level::Error)
    }

    /// The problems told: those of the nodes not filled in yet (a node just added) only once a run was asked for.
    pub fn shown_issues(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| !i.unfilled || self.unfilled_shown)
    }

    /// Whether a run may be asked for: it runs ([`Session::can_run`]), or it tells what the nodes not filled in
    /// need ([`Session::ask_run`]).
    pub fn can_ask_run(&self) -> bool {
        self.can_run() || (!self.running() && !self.loading() && self.load_error.is_none() && self.unfilled_only())
    }

    /// A run asked for: whether it may start; when the nodes not filled in stop it, they are told from then on.
    pub fn ask_run(&mut self) -> bool {
        if self.can_run() {
            return true;
        }
        if self.issues.iter().any(|i| i.unfilled) {
            self.unfilled_shown = true;
        }
        false
    }

    /// Whether the only errors are of nodes not filled in (some at least).
    fn unfilled_only(&self) -> bool {
        let mut errors = self.issues.iter().filter(|i| i.severity == Severity::Error).peekable();
        errors.peek().is_some() && errors.all(|i| i.unfilled) && !self.findings.iter().any(|f| f.level == Level::Error)
    }

    /// Whether a run may stop at a node: only the errors of the nodes it needs count, and the engine checks
    /// them.
    pub fn can_run_to(&self) -> bool {
        !self.running() && !self.loading() && self.load_error.is_none()
    }

    /// The folder of the model's frozen results, next to its file (none: not saved).
    pub fn frozen_folder(&self) -> Option<PathBuf> {
        self.path.as_deref().map(terraflux_exec::frozen::folder)
    }

    /// Freeze a top-level node's result (its outputs of the last run written next to the model and used instead of
    /// running it, in later sessions too; not run yet: frozen when it runs) or thaw it. The flag is an edit of the
    /// model (`edited` follows).
    pub fn freeze(&mut self, id: &str, on: bool) -> Result<(), String> {
        let folder = self.frozen_folder();
        if on {
            let folder = folder.ok_or("save the model first: a frozen result is kept next to its file")?;
            let outputs = self.cache.lock().unwrap().outputs(id).cloned();
            if let Some(outputs) = outputs {
                terraflux_exec::frozen::freeze(&folder, id, &outputs).map_err(|e| format!("{id}: {e}"))?;
                self.cache.lock().unwrap().hold(id, None, 0);
            }
        } else {
            if let Some(f) = &folder {
                terraflux_exec::frozen::thaw(f, id).map_err(|e| e.to_string())?;
            }
            self.cache.lock().unwrap().release(id);
        }
        crate::editor::set_frozen(&mut self.source, id, on).map_err(|e| e.to_string())
    }

    /// The frozen results held for the run as the model says: read from disk when not held yet (a new session),
    /// let go of when thawed (an undo).
    fn hold_frozen(&mut self) {
        let folder = self.frozen_folder();
        let mut c = self.cache.lock().unwrap();
        for n in &self.model.nodes {
            if n.frozen && !c.holds(&n.id) {
                if let Some(f) = &folder
                    && let Ok((outputs, when)) = terraflux_exec::frozen::read(f, &n.id)
                {
                    c.hold(&n.id, Some(outputs), when);
                }
            } else if !n.frozen && c.holds(&n.id) {
                c.release(&n.id);
            }
        }
    }

    /// The nodes frozen without a result kept (frozen before they ran, or their files gone): what they gave in
    /// this run written next to the model, and held.
    fn freeze_new(&mut self) {
        let Some(folder) = self.frozen_folder() else { return };
        let mut c = self.cache.lock().unwrap();
        for n in self.model.nodes.iter().filter(|n| n.frozen) {
            if c.holds(&n.id) {
                continue;
            }
            let Some(outputs) = c.outputs(&n.id).cloned() else { continue };
            match terraflux_exec::frozen::freeze(&folder, &n.id, &outputs) {
                Ok(()) => {
                    c.hold(&n.id, None, 0);
                }
                Err(e) => self.warnings.push(format!("{}: its result could not be frozen: {e}", n.id)),
            }
        }
    }

    /// Run the whole model.
    pub fn start(&mut self, repaint: Box<dyn Fn() + Send>) {
        self.start_to(None, repaint);
    }

    /// Run what a node needs (none: the whole model); what did not change since the last run comes from the
    /// cache.
    pub fn start_to(&mut self, target: Option<String>, repaint: Box<dyn Fn() + Send>) {
        // (a run asked for: the nodes not filled in told)
        self.unfilled_shown = self.issues.iter().any(|i| i.unfilled);
        self.hold_frozen();
        let base = self.base();
        self.edited_at = None;
        match &target {
            None => self.status.clear(),
            Some(t) => {
                self.status.remove(t);
            }
        }
        self.status.retain(|_, st| *st != NodeStatus::Failed);
        self.status_rev += 1;
        self.target = target.clone();
        // how far the run is: none of its nodes done
        self.progress = None;
        self.run_done = 0;
        self.node.clear();
        self.run_total = match &target {
            None => self.model.nodes.len(),
            Some(t) => {
                let among = terraflux_graph::ids(&self.model.nodes);
                let mut seen = std::collections::BTreeSet::new();
                let mut todo = vec![t.clone()];
                while let Some(id) = todo.pop() {
                    if let Some(n) = self.model.nodes.iter().find(|n| n.id == id)
                        && seen.insert(id)
                    {
                        todo.extend(terraflux_graph::dependencies_among(n, &among));
                    }
                }
                seen.len()
            }
        };
        self.maps.truncate(1);
        self.counts.truncate(1);
        self.tables.clear();
        self.warnings.clear();
        self.failures.clear();
        self.error = None;
        self.elapsed = None;
        self.shown = 0;
        self.layer = None;
        self.playing = false;
        self.validation.agreement = None;
        self.validation.similarity = None;
        self.started = Some(Instant::now());
        // the data read already, unless the model's inputs changed since
        let inputs = self.inputs.clone().filter(|_| self.inputs_of == self.model.inputs);
        // a whole run of the model as saved: its outputs and its manifest written next to it, as `terraflux run`
        let record = match &self.path {
            Some(p) if target.is_none() && !self.dirty => Some(Record {
                model_file: std::path::absolute(p).unwrap_or_else(|_| p.clone()),
                out: base.join("output"),
            }),
            _ => None,
        };
        self.recorded = None;
        let (cache, gate) = (self.cache.clone(), self.gate.clone());
        self.run = Some(Run::start(self.model.clone(), inputs, base, cache, target, record, gate, repaint));
    }

    /// After an edit, run up to the node shown (when the edits paused a moment and nothing runs).
    pub fn auto_run(&mut self, ctx: &egui::Context) {
        // a whole run asked for before the data was read (a template)
        if self.run_when_ready && !self.loading() {
            self.run_when_ready = false;
            if self.can_run() {
                let c = ctx.clone();
                self.start(Box::new(move || c.request_repaint()));
                return;
            }
        }
        let Some(at) = self.edited_at else { return };
        if !self.auto || self.preview.is_none() || !self.can_run_to() {
            return;
        }
        let wait = Duration::from_millis(250);
        if at.elapsed() < wait {
            ctx.request_repaint_after(wait - at.elapsed());
            return;
        }
        let target =
            self.preview.as_ref().map(|(n, _)| n.clone()).filter(|n| self.model.nodes.iter().any(|m| m.id == *n));
        if target.is_none() {
            self.edited_at = None;
            return;
        }
        let c = ctx.clone();
        self.start_to(target, Box::new(move || c.request_repaint()));
    }

    /// An output in a map tab of its own; the tab. A node has one: asked for again, its tab comes to the front
    /// (showing the output asked for), and a new one opens only once it is closed.
    pub fn open_map(&mut self, node: &str, port: &str) -> u64 {
        if let Some((&id, shown)) = self.map_tabs.iter_mut().find(|(_, t)| t.node == node) {
            if shown.port != port {
                shown.port = port.into();
                shown.band = 0;
                shown.chosen = false;
            }
            self.raised.push(id);
            return id;
        }
        self.next_tab += 1;
        let shown = crate::builder::Shown { node: node.into(), port: port.into(), ..Default::default() };
        self.map_tabs.insert(self.next_tab, shown);
        self.opened.push(self.next_tab);
        self.next_tab
    }

    /// Save an output to a file in a thread (a map, a stack, a series or rasters: GeoTIFF, copied in blocks
    /// whatever its size; a table: CSV; others: JSON); the status bar says when it is done.
    pub fn save_output(&mut self, node: &str, port: &str, path: &Path) {
        let Some(value) = self.value_of(node, port) else {
            self.notice = Some((format!("{node}.{port} has not run: nothing to save"), Instant::now()));
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let p = path.to_path_buf();
        std::thread::spawn(move || {
            let _ = tx.send(terraflux_exec::write_value(&value, &p, None).map_err(|e| e.to_string()));
        });
        self.saving.push((path.to_path_buf(), rx));
        self.notice = Some((format!("saving {}...", path.display()), Instant::now()));
    }

    /// Whether outputs are being saved.
    pub fn saving(&self) -> bool {
        !self.saving.is_empty()
    }

    /// The output shown in the viewer.
    pub fn preview_value(&self) -> Option<Value> {
        let (node, port) = self.preview.as_ref()?;
        self.value_of(node, port)
    }

    /// An output of a node: from the cache while a run goes on, else from the last run.
    pub fn value_of(&self, node: &str, port: &str) -> Option<Value> {
        if let Some(v) = self.outputs.get(node).and_then(|o| o.get(port)).filter(|_| !self.running()) {
            return Some(v.clone());
        }
        let kept = self.cache.lock().unwrap().outputs(node).and_then(|o| o.get(port)).cloned();
        kept.or_else(|| self.outputs.get(node)?.get(port).cloned())
    }

    /// Take what the data's reading, the savings and the run sent.
    pub fn poll(&mut self) {
        let mut left = Vec::new();
        for (path, rx) in self.saving.drain(..) {
            match rx.try_recv() {
                Ok(Ok(p)) => self.notice = Some((format!("saved {}", p.display()), Instant::now())),
                Ok(Err(e)) => self.notice = Some((format!("{} not saved: {e}", path.display()), Instant::now())),
                Err(TryRecvError::Empty) => left.push((path, rx)),
                Err(TryRecvError::Disconnected) => {}
            }
        }
        self.saving = left;
        if let Some(rx) = &self.loading {
            match rx.try_recv() {
                Ok(l) => {
                    self.loading = None;
                    self.loaded(l);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.loading = None;
                    self.load_error = Some("the data could not be read".into());
                }
            }
        }
        let Some(run) = &self.run else { return };
        let mut done = None;
        let mut plots = Vec::new();
        while let Ok(m) = run.rx.try_recv() {
            match m {
                Message::Node(at) => {
                    let top = at.split('/').next().unwrap_or(&at).to_string();
                    if self.status.insert(top, NodeStatus::Running) != Some(NodeStatus::Running) {
                        self.status_rev += 1;
                    }
                    self.task = None;
                    self.node = at;
                }
                Message::Progress { at, progress } => self.task = Some((at, progress)),
                Message::Timed { at, seconds, bytes } => {
                    self.timings.insert(at, (seconds, bytes));
                }
                Message::Finished { at, cached } => {
                    // a plot that ran: its map tab, opened once
                    let plot = self.model.nodes.iter().any(|n| n.id == at && n.op == "view.plot");
                    if plot && !self.map_tabs.values().any(|t| t.node == at) && !plots.contains(&at) {
                        plots.push(at.clone());
                    }
                    self.status.insert(at, NodeStatus::Done { cached });
                    self.status_rev += 1;
                    self.run_done += 1;
                }
                Message::Failed(at) => {
                    self.status.insert(at, NodeStatus::Failed);
                    self.status_rev += 1;
                }
                Message::Step { at, step, of, values, counts } => {
                    if let Some(Value::Map(m)) = values.get("maps") {
                        self.counts.push(counts.unwrap_or_else(|| class_counts(&m.data)));
                        self.maps.push(m.clone());
                        if self.follow {
                            self.shown = self.maps.len() - 1;
                        }
                    }
                    if let Some(Value::Table(t)) = values.get("info") {
                        self.tables.push(t.clone());
                    }
                    self.progress = Some((at, step, of));
                }
                Message::Warning(w) => self.warnings.push(w),
                Message::Failures { at, failed } => {
                    self.failures.insert(at, failed);
                }
                Message::Recorded(r) => self.recorded = Some(r),
                Message::Done(r) => done = Some(r),
            }
        }
        // (a plot open already is left where it is)
        for p in plots {
            if !self.map_tabs.values().any(|t| t.node == p) {
                let id = self.open_map(&p, "shown");
                self.opened.retain(|&o| o != id);
                self.quiet.push(id);
            }
        }
        // stopped, and still busy in a node that cannot stop in the middle a second later: let it end in the
        // background, its results not shown
        if done.is_none() && self.stopping.is_some_and(|t| t.elapsed() > Duration::from_secs(1)) {
            done = Some(Err(format!("stopped (the node {} goes on in the background until it ends)", self.node)));
            if let Some(run) = self.run.take() {
                run.detach();
            }
        }
        if let Some(r) = done {
            self.run = None;
            self.task = None;
            self.progress = None;
            self.stopping = None;
            self.elapsed = self.started.map(|t| t.elapsed());
            self.status.retain(|_, st| *st != NodeStatus::Running);
            self.status_rev += 1;
            match r {
                Ok(results) => {
                    self.outputs.extend(results.values);
                    self.freeze_new();
                }
                Err(e) => self.error = Some(e),
            }
            self.outputs.retain(|id, _| self.model.nodes.iter().any(|n| n.id == *id));
        }
    }

    /// How far the run is (0 to 1) and in words: the top-level nodes done, with the share of the one running (a
    /// loop's steps, or what its operator tells), and what runs.
    pub fn run_progress(&self) -> (f32, String) {
        let top = |at: &str| at.split('/').next().unwrap_or(at).to_string();
        let running = top(&self.node);
        let mut part = 0.0;
        let mut text = if self.node.is_empty() { "starting".to_string() } else { self.node.clone() };
        if let Some((at, step, of)) = &self.progress
            && top(at) == running
        {
            part = *step as f32 / (*of).max(1) as f32;
            text = format!("{at}: step {step} of {of}");
        }
        if let Some((at, p)) = &self.task
            && top(at) == running
        {
            if let Some(f) = p.fraction {
                part = f as f32;
            }
            if !p.text.is_empty() {
                text = format!("{at}: {}", p.text);
            }
        }
        let total = self.run_total.max(1) as f32;
        let done = (self.run_done as f32 + part.clamp(0.0, 1.0)).min(total);
        (done / total, format!("{text} ({} of {} nodes)", self.run_done.min(self.run_total), self.run_total))
    }

    /// The cells of each class at each step (rows: steps).
    pub fn areas(&self) -> &[Vec<(f64, usize)>] {
        &self.counts
    }

    /// The maps of the layer shown.
    pub fn frames(&self) -> usize {
        match self.layer {
            None => self.maps.len(),
            Some(i) => self.layers.get(i).map_or(0, |l| l.bands.len()),
        }
    }

    /// The map of the layer shown.
    pub fn frame(&self) -> usize {
        match self.layer {
            None => self.shown.min(self.maps.len().saturating_sub(1)),
            Some(i) => self.layers.get(i).map_or(0, |l| l.shown),
        }
    }

    pub fn set_frame(&mut self, k: usize) {
        let k = k.min(self.frames().saturating_sub(1));
        match self.layer {
            None => {
                self.shown = k;
                self.follow = k + 1 == self.maps.len();
            }
            Some(i) => {
                if let Some(l) = self.layers.get_mut(i) {
                    l.shown = k;
                }
            }
        }
    }

    /// The map shown, its look and its name.
    pub fn current(&self) -> Option<(Map, Look, String)> {
        match self.layer {
            None => {
                let k = self.frame();
                let m = self.maps.get(k)?;
                let name = if k == 0 { "the start".to_string() } else { format!("step {k}") };
                let coloring = self.coloring.clone().unwrap_or(Coloring::Continuous {
                    min: 0.0,
                    max: 1.0,
                    ramp: Default::default(),
                    log: false,
                });
                Some((
                    m.clone(),
                    Look { coloring, counts: self.counts.get(k).cloned().unwrap_or_default(), product: None },
                    name,
                ))
            }
            Some(i) => {
                let l = self.layers.get(i)?;
                let (name, m, look) = l.bands.get(l.shown)?;
                let name = if l.stack { format!("{} / {name}", l.name) } else { format!("{} ({name})", l.name) };
                Some((m.clone(), look.clone(), name))
            }
        }
    }

    pub fn validate(&mut self) {
        let v = &mut self.validation;
        v.error = None;
        let result = (|| -> Result<(Agreement, Table), String> {
            let path = v.observed.as_ref().ok_or("choose the observed map")?;
            let r = Raster::read(path).map_err(|e| e.to_string())?;
            let start = self.maps.first().ok_or("no start map")?;
            let last = self.maps.last().filter(|_| self.maps.len() > 1).ok_or("run the model first")?;
            if r.grid.width != start.grid.width || r.grid.height != start.grid.height {
                return Err("the observed map is on another grid".into());
            }
            let obs = Map::new(start.grid.clone(), r.bands[v.band.min(r.bands.len() - 1)].masked(r.nodata));
            let windows: Vec<i32> = v
                .windows
                .split(',')
                .map(|s| s.trim().parse::<i32>().map_err(|_| format!("{:?} is not a window", s.trim())))
                .collect::<Result<_, _>>()?;
            if windows.iter().any(|w| *w < 1 || w % 2 == 0) {
                return Err("windows are odd sides: 1, 3, 5...".into());
            }
            let a = agreement(&obs, last, start).map_err(|e| e.to_string())?;
            let s = similarity_of_differences(start, &obs, last, &windows, true).map_err(|e| e.to_string())?;
            Ok((a, s))
        })();
        match result {
            Ok((a, s)) => {
                v.agreement = Some(a);
                v.similarity = Some(s);
            }
            Err(e) => v.error = Some(e),
        }
    }
}

/// The application.
pub struct TerrafluxApp {
    pub session: Option<Session>,
    pub wizard: Option<Wizard>,
    /// The panes of the main window, and the windows of their own.
    pub dock: Option<DockState<Pane>>,
    pub windows: Vec<Detached>,
    window_count: u64,
    /// Where each pane in a window of its own was in the main window (to go back there).
    homes: HashMap<Pane, egui_dock::TabPath>,
    /// Where the layout is kept, its text kept last and when.
    layout_file: Option<PathBuf>,
    layout_text: String,
    layout_saved: Instant,
    pub message: Option<String>,
    /// The name typed for a template of the user's.
    pub template_name: String,
    /// The operators, made once.
    pub registry: Registry,
    pub library: Library,
    pub(crate) hover: Option<Hover>,
    /// The style applied: the theme, the text's size and the zoom it was made for.
    styled: Option<(String, u32, u32)>,
    /// The settings' window is open.
    pub settings_open: bool,
    /// The updates of Terraflux: the latest release asked, its installer downloaded ([`crate::update`]).
    pub updater: crate::update::Updater,
    /// The window asking to update (the release), and the user's word given: installed once downloaded.
    update_asked: Option<crate::update::Release>,
    update_go: bool,
    /// The updates' failures told in the toolbar (the user asked), not those of the check at the start.
    pub update_told: bool,
    /// Terraflux was updated since it ran last: from which version, told in the toolbar for a while.
    pub updated_from: Option<(String, Instant)>,
    /// The bytes in the caches (rasters made, data downloaded), measured when (counted again every few seconds
    /// while the settings show it).
    cache_used: Option<(Instant, u64)>,
    /// The map seen last (a map's tab or the viewer): Tab goes from the model to it and back.
    tab_back: Option<Pane>,
    /// Tab was taken this frame: the keyboard's focus it may have given a field is let go at the frame's end.
    tabbed: bool,
    /// The drawer out (the operators' or the files', its pane closed: the pointer went to its tab at the window's
    /// left edge).
    pub drawer: Option<Pane>,
    /// The viewer was opened when the first data came (not again, if it is closed after).
    viewer_opened: bool,
    /// The window to be maximized, at its third frame (the frames drawn so far): on Windows the window shown
    /// after its first frame loses the maximized state it was made with.
    pub maximize: Option<u32>,
    /// The Files pane's folder.
    pub files: crate::files_view::Files,
    /// The model run many times (replicates, scenarios, calibration).
    pub batch: crate::batch_view::BatchState,
    /// The Parameters pane's state.
    pub params: crate::params_view::State,
    /// The Trace pane's state.
    pub trace: crate::trace_view::State,
    /// The dock is being drawn (taken out of the application meanwhile): the panes asked for then, brought to the
    /// front once it is back.
    in_dock: bool,
    deferred: Vec<Pane>,
    /// The application's pool of workers, shared or not, and this computer lent to another.
    pub computers: crate::computers::Computers,
    /// The assistant: its conversation and its server.
    pub chat: crate::chat::Chat,
    /// The calculator's script, its preview and its console.
    pub calculator: crate::calc_view::Calculator,
    /// The palette of commands, while it is open (ctrl+K).
    pub commands: Option<crate::commands::Commands>,
    /// The window of the keyboard's shortcuts is open (F1).
    pub shortcuts_open: bool,
    /// The main window's title, as last given to it.
    window_title: String,
    /// The models handed by `terraflux mcp` (open_in_app) come here.
    pub handoff: Option<crate::handoff::Listener>,
}

/// A row of a list: a name, selected or not, and a note on the right.
fn row(ui: &mut Ui, selected: bool, name: &str, note: &str) -> egui::Response {
    let note = RichText::new(note).size(11.0).color(if selected {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().weak_text_color()
    });
    ui.add(egui::Button::selectable(selected, name).right_text(note).min_size(Vec2::new(ui.available_width(), 22.0)))
}

/// A button in the accent when it may be pressed.
pub(crate) fn accent_button(text: &str, on: bool) -> egui::Button<'_> {
    if on {
        egui::Button::new(RichText::new(text).color(theme::on_accent()).strong()).fill(theme::accent())
    } else {
        egui::Button::new(text)
    }
}

impl TerrafluxApp {
    /// The application with a model, else with a new model in the builder.
    pub fn new(model: Option<PathBuf>) -> Self {
        let registry = Registry::standard();
        let library = Library::new(&registry);
        terraflux_ops::cache::set_limit(crate::settings::get().cache_gb);
        let mut app = TerrafluxApp {
            template_name: String::new(),
            session: Some(Session::blank()),
            wizard: None,
            dock: Some(default_layout()),
            windows: Vec::new(),
            window_count: 0,
            homes: HashMap::new(),
            layout_file: None,
            layout_text: String::new(),
            layout_saved: Instant::now(),
            message: None,
            registry,
            library,
            hover: None,
            styled: None,
            settings_open: false,
            updater: crate::update::Updater::default(),
            update_asked: None,
            update_go: false,
            update_told: false,
            updated_from: None,
            cache_used: None,
            tab_back: None,
            tabbed: false,
            drawer: None,
            viewer_opened: false,
            maximize: None,
            files: crate::files_view::Files::default(),
            batch: crate::batch_view::BatchState::new(),
            params: crate::params_view::State::default(),
            trace: crate::trace_view::State::default(),
            in_dock: false,
            deferred: Vec::new(),
            computers: crate::computers::Computers::new(),
            chat: crate::chat::Chat::default(),
            calculator: crate::calc_view::Calculator::default(),
            commands: None,
            shortcuts_open: false,
            window_title: String::new(),
            handoff: None,
        };
        if let Some(p) = model {
            app.open(&p);
        }
        app
    }

    pub fn open(&mut self, path: &Path) {
        match Session::open(path) {
            Ok(s) => {
                crate::settings::opened_model(path);
                self.session = Some(s);
                self.message = None;
                self.focus(Pane::Model);
                // a model of data: its viewer beside it, the graph in front
                self.viewer_opened = false;
                if self.has_data() {
                    self.add_viewer();
                }
            }
            Err(e) => self.message = Some(e),
        }
    }

    /// A template in the builder, its graph arranged, its last node shown, and run (its plots open by themselves).
    pub fn start_template(&mut self, ctx: &egui::Context, t: &crate::templates::Template) {
        self.start_model(ctx, t.model());
    }

    /// A model to start from in the builder (a template's), its graph arranged, its last node shown, and run.
    pub fn start_model(&mut self, ctx: &egui::Context, model: Model) {
        let mut s = match Session::from_model(model) {
            Ok(s) => s,
            Err(e) => {
                self.message = Some(e);
                return;
            }
        };
        crate::builder::arrange(&mut s, &self.registry);
        if let Some(last) = s.source.nodes.iter().rev().find(|n| n.op != "view.plot").map(|n| n.id.clone()) {
            crate::builder::select(&mut s, &self.registry, &last);
        }
        s.run_when_ready = true;
        ctx.request_repaint();
        self.wizard = None;
        self.session = Some(s);
        self.message = None;
        self.focus(Pane::Model);
        // what it makes, in the viewer (behind the graph)
        self.add_viewer();
    }

    /// Save the model (asking where when it has no file yet).
    fn save(&mut self, choose: bool) {
        let Some(s) = &mut self.session else { return };
        let path = match (&s.path, choose) {
            (Some(p), false) => Some(p.clone()),
            _ => rfd::FileDialog::new()
                .set_title("Save the model")
                .add_filter("Terraflux model", &["toml", "json"])
                .set_file_name("model.toml")
                .save_file(),
        };
        if let Some(p) = path {
            match s.save_as(&p) {
                Ok(()) => crate::settings::opened_model(&p),
                Err(e) => s.editor.message = Some(e),
            }
        }
    }

    /// A new model in the builder.
    fn new_model(&mut self) {
        self.wizard = None;
        self.session = Some(Session::blank());
        self.focus(Pane::Model);
    }

    /// Process a folder...: in the model open (a new one if none), the folder's rasters through an operator.
    fn process_folder(&mut self) {
        if self.session.is_none() || self.wizard.is_some() {
            self.new_model();
        }
        self.focus(Pane::Model);
        if let Some(s) = &mut self.session {
            crate::builder::process_folder_dialog(s);
        }
    }

    /// The models handed by `terraflux mcp` (open_in_app): opened here and the window brought to the front, unless
    /// the model open has edits not saved or a wizard is open (the answer says why, and it opens a window of its own).
    fn take_handoffs(&mut self, ctx: &egui::Context) {
        let Some(h) = &self.handoff else { return };
        let asked: Vec<_> = h.requests.try_iter().collect();
        for r in asked {
            let wanted = std::fs::canonicalize(&r.path).ok();
            let open_already = self
                .session
                .as_ref()
                .and_then(|s| s.path.as_ref())
                .is_some_and(|p| wanted.is_some() && std::fs::canonicalize(p).ok() == wanted);
            let answer = match &self.session {
                _ if open_already => Ok(()),
                _ if self.wizard.is_some() => Err("Terraflux has a wizard open".to_string()),
                Some(s) if s.dirty => Err(format!("the model open in Terraflux ({}) has edits not saved", s.title())),
                _ => {
                    self.message = None;
                    self.open(&r.path);
                    self.message.clone().map_or(Ok(()), Err)
                }
            };
            if answer.is_ok() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Informational,
                ));
            }
            let _ = r.answer.send(answer);
        }
    }

    /// Open a model chosen in a dialog.
    fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("Terraflux model", &["toml", "json"]).pick_file() {
            self.wizard = None;
            self.open(&p);
        }
    }

    /// Run the whole model.
    fn run_all(&mut self, ctx: &egui::Context) {
        let Some(s) = &mut self.session else { return };
        if s.ask_run() {
            let ctx = ctx.clone();
            s.start(Box::new(move || ctx.request_repaint()));
        } else if s.unfilled_shown {
            // what the nodes not filled in need
            self.focus(Pane::Problems);
        }
    }

    /// Stop the run.
    fn stop_run(&mut self) {
        if let Some(s) = &mut self.session
            && s.stopping.is_none()
            && let Some(r) = &s.run
        {
            r.stop();
            s.stopping = Some(Instant::now());
        }
    }

    /// The model's file: new, open (the models opened last too), save, save as.
    fn file_menu(&mut self, ui: &mut Ui, save: &mut Option<bool>) {
        let c = crate::commands::cmd();
        ui.set_min_width(220.0);
        if ui.add(egui::Button::new("New model").shortcut_text(format!("{c}N"))).clicked() {
            self.new_model();
            ui.close();
        }
        if ui.add(egui::Button::new("Open...").shortcut_text(format!("{c}shift+O"))).clicked() {
            ui.close();
            self.open_dialog();
        }
        let recent = crate::settings::recent_models();
        ui.add_enabled_ui(!recent.is_empty(), |ui| {
            ui.menu_button("Open recent", |ui| {
                for p in recent {
                    let name = p.file_name().map_or(p.display().to_string(), |n| n.to_string_lossy().into_owned());
                    if ui.button(name).on_hover_text(p.display().to_string()).clicked() {
                        ui.close();
                        self.wizard = None;
                        self.open(&p);
                    }
                }
            });
        });
        ui.separator();
        let (can, editing) =
            self.session.as_ref().map_or((false, false), |s| (s.dirty || s.path.is_none(), self.wizard.is_none()));
        if ui.add_enabled(can && editing, egui::Button::new("Save").shortcut_text(format!("{c}S"))).clicked() {
            *save = Some(false);
            ui.close();
        }
        if ui.add_enabled(editing, egui::Button::new("Save as...").shortcut_text(format!("{c}shift+S"))).clicked() {
            *save = Some(true);
            ui.close();
        }
        ui.separator();
        if let Some(s) = &mut self.session
            && ui.add(egui::Button::new("Open a raster or vectors...").shortcut_text(format!("{c}O"))).clicked()
        {
            ui.close();
            crate::builder::open_data(s, &self.registry);
        }
        if let Some(s) = &mut self.session
            && ui
                .button("Data catalog...")
                .on_hover_text("climate, terrain, soil, land cover and rivers, downloaded onto the model's grid")
                .clicked()
        {
            ui.close();
            s.catalog.open = true;
        }
        if ui.button("Process a folder...").on_hover_text(crate::builder::PROCESS_HELP).clicked() {
            ui.close();
            self.process_folder();
        }
        ui.separator();
        if let Some(s) = &mut self.session {
            ui.menu_button("Compare", |ui| {
                crate::review::compare_menu(ui, s);
            });
            if ui.button("Export report...").on_hover_text("the model as a document: HTML or PDF").clicked() {
                ui.close();
                crate::document_view::export(s, &self.registry);
            }
            if ui
                .button("Methods and citations...")
                .on_hover_text("the model's methods as a paper tells them, the papers to cite (BibTeX, RIS)")
                .clicked()
            {
                ui.close();
                s.methods_open = true;
            }
            if ui
                .button("Python script (landschaft)...")
                .on_hover_text("the model as a Python script of landschaft: for a paper, a supervisor, a reviewer")
                .clicked()
            {
                ui.close();
                crate::python::open(s, &self.registry);
            }
            if ui
                .button("Export to QGIS...")
                .on_hover_text("the run's maps and vector layers, in their colours, as a QGIS project")
                .clicked()
            {
                ui.close();
                s.qgis.open = true;
            }
        }
    }

    /// The templates by group, and the user's own (a model saved as a template).
    fn templates_menu(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Models to start from, run at once").weak().size(11.0));
        for g in crate::templates::GROUPS {
            ui.menu_button(g, |ui| {
                for t in crate::templates::TEMPLATES.iter().filter(|t| t.group == g) {
                    if ui.button(t.name).on_hover_text(t.about).clicked() {
                        self.start_template(ui.ctx(), t);
                        ui.close();
                    }
                }
            });
        }
        // the user's own: a model saved as a template, opened as a new model
        ui.separator();
        ui.label(RichText::new("Yours").weak().size(11.0));
        let mut remove = None;
        for (name, path) in crate::templates::user_templates() {
            ui.horizontal(|ui| {
                if ui.button(&name).on_hover_text(path.display().to_string()).clicked() {
                    match terraflux_graph::Model::load(&path) {
                        Ok(m) => self.start_model(ui.ctx(), m),
                        Err(e) => self.message = Some(format!("{}: {e}", path.display())),
                    }
                    ui.close();
                }
                if ui.small_button("x").on_hover_text("remove this template").clicked() {
                    remove = Some(path.clone());
                }
            });
        }
        if let Some(p) = remove {
            let _ = std::fs::remove_file(p);
        }
        if let Some(s) = &self.session {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.template_name).hint_text("name").desired_width(140.0));
                let base = s.path.as_deref().and_then(Path::parent);
                if ui.button("Save this model as a template").clicked() {
                    let name = if self.template_name.trim().is_empty() {
                        s.source.name.clone().unwrap_or_else(|| "model".into())
                    } else {
                        self.template_name.clone()
                    };
                    match crate::templates::save_user_template(&s.source, base, &name) {
                        Ok(p) => self.message = Some(format!("saved the template {}", p.display())),
                        Err(e) => self.message = Some(e),
                    }
                    self.template_name.clear();
                }
            });
        }
    }

    /// The panes on or off, the layout as it first was, the commands and the shortcuts.
    fn view_menu(&mut self, ui: &mut Ui) {
        let c = crate::commands::cmd();
        for p in Pane::ALL {
            let mut open = self.is_open(p);
            if ui.checkbox(&mut open, p.title()).changed() {
                if open {
                    self.focus(p);
                } else {
                    self.close(p);
                }
            }
        }
        ui.separator();
        if ui.button("Reset the layout").clicked() {
            self.windows.clear();
            self.dock = Some(default_layout());
            ui.close();
        }
        ui.separator();
        // the maps as a reader with a colour vision deficiency sees them
        ui.menu_button("Colour vision", |ui| {
            ui.label(
                RichText::new("every map and legend drawn as this reader sees them, to check a figure")
                    .weak()
                    .size(11.0),
            );
            let now = crate::palette::vision();
            for v in crate::palette::Vision::ALL {
                if ui.radio(now == v, v.name()).clicked() {
                    crate::palette::set_vision(v);
                    ui.close();
                }
            }
        });
        ui.separator();
        if ui.add(egui::Button::new("Commands...").shortcut_text(format!("{c}K"))).clicked() {
            self.commands = Some(Default::default());
            ui.close();
        }
        if ui.add(egui::Button::new("Keyboard shortcuts").shortcut_text("F1")).clicked() {
            self.shortcuts_open = true;
            ui.close();
        }
    }

    /// The bar at the top: the menus at the left; running the model, its name and how the run goes in the middle;
    /// the tools and the settings at the right.
    fn toolbar(&mut self, ui: &mut Ui) {
        let mut save = None;
        let (mut run, mut stop) = (false, false);
        ui.horizontal(|ui| {
            // (the modern look: the bar's buttons without a frame until the pointer is over them)
            theme::quiet(ui);
            let (mark, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
            crate::logo::paint_mark(ui.painter(), mark, theme::accent(), theme::current().text);
            ui.label(crate::logo::wordmark(14.0, theme::current().text, theme::accent()))
                .on_hover_text(format!("Terraflux {}", env!("CARGO_PKG_VERSION")));
            ui.label(RichText::new(env!("CARGO_PKG_VERSION")).weak().size(11.0))
                .on_hover_text("Terraflux's version (its updates: ⚙, the settings)");
            self.update_strip(ui);
            // (maps seen as a colour-blind reader sees them: said, and undone in a click)
            let vision = crate::palette::vision();
            if vision != crate::palette::Vision::Normal
                && ui
                    .button(RichText::new(format!("seen with {} · back to normal", vision.name())).color(theme::accent()).size(11.5))
                    .on_hover_text("back to normal colour vision (View > Colour vision)")
                    .clicked()
            {
                crate::palette::set_vision(crate::palette::Vision::Normal);
            }
            ui.add_space(6.0);
            ui.menu_button("File", |ui| self.file_menu(ui, &mut save));
            ui.menu_button("Templates", |ui| self.templates_menu(ui));
            if let Some(s) = &mut self.session {
                ui.menu_button("Operators", |ui| {
                    crate::builder::operators_menu(ui, s, &self.registry, &self.library);
                })
                .response
                .on_hover_text("add an operator after the node selected (also at the window's left edge, and \"/\")");
            }
            ui.menu_button("View", |ui| self.view_menu(ui));
            ui.separator();
            if let Some(s) = &self.session
                && self.wizard.is_none()
            {
                let label = if theme::modern() { "▶  Run" } else { "Run" };
                run = ui
                    .add_enabled(s.can_ask_run(), accent_button(label, s.can_run()))
                    .on_hover_text(if s.can_run() {
                        "run the whole model (F5)"
                    } else {
                        "run the whole model (F5): nodes not filled in yet, told among the problems"
                    })
                    .clicked();
                stop = ui.add_enabled(s.running() && s.stopping.is_none(), egui::Button::new("Stop")).clicked();
                // step by step: paused after each step of a loop
                let mut stepping = s.gate.stepping.load(std::sync::atomic::Ordering::Relaxed);
                if ui
                    .toggle_value(&mut stepping, "Step")
                    .on_hover_text("step by step: the run pauses after each step of a loop (its maps shown), until Next or Continue")
                    .changed()
                {
                    if stepping {
                        s.gate.stepping.store(true, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        s.gate.resume();
                    }
                }
                ui.separator();
            }
            // the tools and the settings at the right first, the model's name and how its run goes (the modern
            // look: in a pill) in the room left; too little room: the tools in a menu (the modern look: their names
            // without dots)
            let tool = |t: &str| if theme::modern() { t.to_string() } else { format!("{t}...") };
            let (menu_id, wide_id) = (ui.id().with("tools in a menu"), ui.id().with("tools' width"));
            let in_menu = ui.data(|d| d.get_temp::<bool>(menu_id)).unwrap_or(false);
            let (mut asked, mut wizard, mut settings) = (None, false, false);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(RichText::new("⚙").size(15.0)).frame(false))
                    .on_hover_text("Settings: the theme, the text's size, the threads...")
                    .clicked()
                {
                    settings = true;
                }
                ui.separator();
                let tools = |ui: &mut Ui, asked: &mut Option<Pane>, wizard: &mut bool| {
                    if ui
                        .button(tool("Assistant"))
                        .on_hover_text("a chat with a model (of a university, this computer or a provider) that explains, changes and runs the model")
                        .clicked()
                    {
                        *asked = Some(Pane::Assistant);
                    }
                    if ui
                        .button(tool("Computers"))
                        .on_hover_text("other computers helping with the runs, this one helping another")
                        .clicked()
                    {
                        *asked = Some(Pane::Computers);
                    }
                    if ui
                        .button(tool("Many runs"))
                        .on_hover_text("the model run many times: replicates, scenarios, calibration")
                        .clicked()
                    {
                        *asked = Some(Pane::Batch);
                    }
                    if ui.button(tool("Wizard")).on_hover_text("a land change model from your maps, step by step").clicked() {
                        *wizard = true;
                    }
                };
                if in_menu {
                    ui.menu_button("Tools", |ui| {
                        // (in the menu's order, top to bottom: as at the bar's left to right)
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| tools(ui, &mut asked, &mut wizard));
                        if asked.is_some() || wizard {
                            ui.close();
                        }
                    });
                } else {
                    tools(ui, &mut asked, &mut wizard);
                }
                // the room left: the tools in a menu below a fifth of the bar, back on it when they fit again
                let width = ui.min_rect().width();
                let room = ui.available_width();
                let least = 220.0;
                let to_menu = if in_menu {
                    let wide = ui.data(|d| d.get_temp::<f32>(wide_id)).unwrap_or(f32::INFINITY);
                    room + width - wide >= least + 40.0
                } else {
                    ui.data_mut(|d| d.insert_temp(wide_id, width));
                    room < least
                };
                if to_menu {
                    ui.data_mut(|d| d.insert_temp(menu_id, !in_menu));
                    ui.ctx().request_discard("the bar's tools in a menu, or out of it");
                }
                if let Some(s) = &self.session
                    && self.wizard.is_none()
                {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        theme::pill(ui, |ui| {
                        // the model's name, a dot when it has edits not saved
                        // (cut short when the bar has no room for it)
                        let title = ui
                            .scope(|ui| {
                                ui.set_max_width((ui.available_width() * 0.6).max(60.0));
                                ui.add(egui::Label::new(RichText::new(s.title()).strong()).truncate())
                            })
                            .inner;
                        if let Some(p) = &s.path {
                            title.on_hover_text(p.display().to_string());
                        }
                        if s.dirty {
                            let (r, resp) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                            ui.painter().circle_filled(r.center(), 3.5, ui.visuals().warn_fg_color);
                            resp.on_hover_text(format!("not saved ({}S saves it)", crate::commands::cmd()));
                        }
                        if s.loading() {
                            ui.spinner();
                            ui.label(RichText::new("reading the data...").weak());
                        }
                        if s.running() && s.stopping.is_none() && s.gate.paused() {
                            // paused after a step (in place of the progress)
                            let at = s.progress.as_ref().map(|(at, step, of)| format!("{at}: step {step} of {of}"));
                            ui.label(RichText::new(format!("paused at {}", at.unwrap_or_default())).color(theme::accent()));
                            if ui.button("Next").on_hover_text("run the next step and pause again").clicked() {
                                s.gate.next();
                            }
                            if ui.button("Continue").on_hover_text("run on to the end without pausing").clicked() {
                                s.gate.resume();
                            }
                        } else if s.running() && s.stopping.is_none() {
                            let (frac, text) = s.run_progress();
                            theme::progress(ui, Some(frac), &text, 140.0);
                        }
                        if s.stopping.is_some() {
                            ui.spinner();
                            ui.label(RichText::new("stopping...").weak());
                        } else if let Some(e) = s.elapsed.filter(|_| !s.running()) {
                            let done = RichText::new(format!("done in {:.2} s", e.as_secs_f64())).weak();
                            ui.add(egui::Label::new(done).truncate());
                            match &s.recorded {
                                Some(Ok(p)) => {
                                    let at = p.parent().unwrap_or(p);
                                    if ui
                                        .link(RichText::new("manifest").weak())
                                        .on_hover_text(format!(
                                            "the outputs and the manifest of the run written in {} (terraflux reproduce runs it again); click to open the folder",
                                            at.display()
                                        ))
                                        .clicked()
                                    {
                                        crate::batch_view::open_folder(at);
                                    }
                                }
                                Some(Err(e)) => {
                                    ui.label(RichText::new("manifest ✖").color(ui.visuals().warn_fg_color))
                                        .on_hover_text(format!("the outputs and the manifest of the run were not written: {e}"));
                                }
                                None => {}
                            }
                        }
                        });
                    });
                }
            });
            if settings {
                self.settings_open = !self.settings_open;
            }
            if let Some(p) = asked {
                self.focus(p);
            }
            if wizard {
                self.wizard = Some(Wizard::default());
            }
        });
        if run {
            self.run_all(ui.ctx());
        }
        if stop {
            self.stop_run();
        }
        if let Some(choose) = save {
            self.save(choose);
        }
    }

    /// Everything the palette of commands offers: the actions, the models opened last, the templates, the nodes of
    /// the scope shown and the operators.
    fn command_list(&self) -> Vec<crate::commands::Command> {
        use crate::commands::{Command, Do, cmd};
        let c = cmd();
        let action = |label: &str, key: String, act: Do| Command::new(label, "action", key, act);
        let mut all = vec![
            action("Run the whole model", "F5".into(), Do::Run),
            action("Run up to the node selected", format!("{c}enter"), Do::RunToHere),
            action("Stop the run", String::new(), Do::Stop),
            action("Save the model", format!("{c}S"), Do::Save),
            action("Save the model as...", format!("{c}shift+S"), Do::SaveAs),
            action("New model", format!("{c}N"), Do::New),
            action("Open a model...", format!("{c}shift+O"), Do::Open),
            action("Open a raster or vectors...", format!("{c}O"), Do::OpenData),
            action("Process a folder of rasters...", String::new(), Do::ProcessFolder),
            action("Add an operator", "/".into(), Do::Add(String::new())),
            action("Arrange the nodes", format!("{c}L"), Do::Arrange),
            action("Group the nodes selected", format!("{c}G"), Do::Group),
            action("Ungroup the group selected", format!("{c}shift+G"), Do::Ungroup),
            action("Group the model by stage (data, preparation, model...)", String::new(), Do::GroupByStage),
            action("Save the node selected as my node (a group or a calculator)", String::new(), Do::SaveNode),
            action("Undo", format!("{c}Z"), Do::Undo),
            action("Redo", format!("{c}shift+Z"), Do::Redo),
            action("The model as text, or as a diagram", String::new(), Do::Text),
            action("Wizard: a land change model from your maps", String::new(), Do::Wizard),
            action("Settings", String::new(), Do::Settings),
            action("Keyboard shortcuts", "F1".into(), Do::Shortcuts),
            action("Methods and citations: the model as a paper's methods", String::new(), Do::Methods),
            action("Export to QGIS: the run's maps as a QGIS project", String::new(), Do::Qgis),
            action("Data catalog: climate, terrain, soil, land cover, rivers to download", String::new(), Do::Catalog),
            action("The model as a Python script (landschaft)", String::new(), Do::Python),
            action("Reset the layout", String::new(), Do::ResetLayout),
        ];
        for p in Pane::ALL {
            all.push(Command::new(format!("Show {}", p.title()), "pane", String::new(), Do::Pane(p)));
        }
        for p in crate::settings::recent_models() {
            let name = p.file_name().map_or(p.display().to_string(), |n| n.to_string_lossy().into_owned());
            all.push(Command::new(format!("Open {name}"), "recent", p.display().to_string(), Do::OpenRecent(p)));
        }
        for (k, t) in crate::templates::TEMPLATES.iter().enumerate() {
            all.push(Command::new(t.name, "template", t.group, Do::Template(k)));
        }
        if let Some(s) = &self.session
            && let Some(nodes) = s.source.scope(&s.editor.path)
        {
            for n in nodes {
                all.push(Command::new(format!("Go to {}", n.id), "node", n.op.clone(), Do::GoTo(n.id.clone())));
            }
        }
        for e in &self.library.entries {
            all.push(Command::new(format!("Add {}", e.name), "operator", e.doc.clone(), Do::Add(e.name.clone())));
        }
        for u in crate::my_nodes::nodes() {
            let doc = u.description.clone().unwrap_or_default();
            all.push(Command::new(format!("Add {}", u.label()), "my node", doc, Do::AddMine(u.name.clone())));
        }
        all
    }

    /// Do what a command of the palette asks.
    fn command(&mut self, ctx: &egui::Context, d: crate::commands::Do) {
        use crate::commands::Do;
        match d {
            Do::New => self.new_model(),
            Do::Open => self.open_dialog(),
            Do::ProcessFolder => self.process_folder(),
            Do::OpenRecent(p) => {
                self.wizard = None;
                self.open(&p);
            }
            Do::Save => self.save(false),
            Do::SaveAs => self.save(true),
            Do::Run => self.run_all(ctx),
            Do::Stop => self.stop_run(),
            Do::Settings => self.settings_open = true,
            Do::Shortcuts => self.shortcuts_open = true,
            Do::Methods => {
                if let Some(s) = &mut self.session {
                    s.methods_open = true;
                }
            }
            Do::Qgis => {
                if let Some(s) = &mut self.session {
                    s.qgis.open = true;
                }
            }
            Do::Catalog => {
                if let Some(s) = &mut self.session {
                    s.catalog.open = true;
                }
            }
            Do::Python => {
                if let Some(s) = &mut self.session {
                    crate::python::open(s, &self.registry);
                }
            }
            Do::Wizard => self.wizard = Some(Wizard::default()),
            Do::Pane(p) => self.focus(p),
            Do::ResetLayout => {
                self.windows.clear();
                self.dock = Some(default_layout());
            }
            Do::Template(k) => self.start_template(ctx, &crate::templates::TEMPLATES[k]),
            d => {
                let Some(s) = &mut self.session else { return };
                let r = &self.registry;
                let model = matches!(
                    d,
                    Do::GoTo(_)
                        | Do::Add(_)
                        | Do::AddMine(_)
                        | Do::Arrange
                        | Do::Group
                        | Do::Ungroup
                        | Do::GroupByStage
                );
                match d {
                    Do::OpenData => crate::builder::open_data(s, r),
                    Do::RunToHere => crate::builder::run_selected(ctx, s),
                    Do::Arrange => crate::builder::arrange(s, r),
                    Do::Group => crate::builder::group_selected(s, r),
                    Do::Ungroup => crate::builder::ungroup_selected(s, r),
                    Do::GroupByStage => crate::builder::group_by_stage(s, r),
                    Do::Undo => crate::builder::undo_redo(s, false),
                    Do::Redo => crate::builder::undo_redo(s, true),
                    Do::Text => s.editor.text = !s.editor.text,
                    // (none: the search of the operators)
                    Do::Add(op) if op.is_empty() => s.editor.quick = Some(Default::default()),
                    Do::Add(op) => {
                        if let Err(e) = crate::builder::add(s, r, &op, &[]) {
                            s.editor.message = Some(e);
                        }
                    }
                    Do::SaveNode => match s.selected.clone() {
                        Some(id) => crate::my_nodes::begin(s, r, &id),
                        None => s.editor.message = Some("select a group or a calculator first".into()),
                    },
                    Do::AddMine(name) => {
                        let added = crate::my_nodes::get(&name, None).and_then(|u| crate::my_nodes::add(s, r, &u));
                        if let Err(e) = added {
                            s.editor.message = Some(e);
                        }
                    }
                    Do::GoTo(id) => {
                        s.editor.text = false;
                        crate::builder::select(s, r, &id);
                        s.editor.chosen = [id.clone()].into();
                        s.editor.reveal(&id);
                    }
                    _ => {}
                }
                if model {
                    self.focus(Pane::Model);
                }
            }
        }
    }

    /// In the toolbar: Terraflux updated (a minute after it opens), a newer one (the button to update), its
    /// download, the update's failure when the user asked for it.
    fn update_strip(&mut self, ui: &mut Ui) {
        use crate::update::{State, show};
        if let Some((from, at)) = &self.updated_from
            && at.elapsed() < Duration::from_secs(60)
        {
            ui.label(RichText::new(format!("updated to {}", env!("CARGO_PKG_VERSION"))).color(theme::accent()))
                .on_hover_text(format!("Terraflux was {from} when it ran last"));
        }
        match self.updater.state() {
            State::Available(r) => {
                let b = egui::Button::new(
                    RichText::new(format!("Update to {}", show(r.version))).color(theme::on_orange()).strong(),
                )
                .fill(theme::orange());
                if ui.add(b).on_hover_text(format!("Terraflux {} is out", show(r.version))).clicked() {
                    self.update_asked = Some(r);
                }
            }
            State::Downloading(done, total) => {
                let fraction = total.map(|t| done as f32 / t.max(1) as f32);
                theme::progress(ui, fraction, &format!("downloading Terraflux ({:.0} MB)", done as f64 / 1e6), 120.0);
            }
            State::Ready(..) => {
                ui.spinner();
                ui.label(RichText::new("installing...").weak());
            }
            State::Failed(e) if self.update_told => {
                ui.label(RichText::new("the update failed").color(ui.visuals().warn_fg_color)).on_hover_text(e);
            }
            _ => {}
        }
    }

    /// The window asking to update: what happens then, the model's edits not saved saved first. A Terraflux not
    /// of the Windows installer (the zip, macOS, Linux) opens the release's page instead.
    fn update_window(&mut self, ctx: &egui::Context) {
        use crate::update::show;
        let Some(r) = self.update_asked.clone() else { return };
        let mut open = true;
        let (mut go, mut later) = (None, false);
        let dirty = self.session.as_ref().is_some_and(|s| s.dirty);
        let how = crate::update::install().filter(|how| r.package(how).is_some());
        let installer = how.is_some();
        egui::Window::new(format!("Terraflux {}", show(r.version)))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(format!("Terraflux {} is out (this one is {}).", show(r.version), env!("CARGO_PKG_VERSION")));
                ui.add_space(4.0);
                if let Some(how) = &how {
                    ui.label(how.tell());
                    if dirty {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("The model has edits not saved: saved first.")
                                .color(ui.visuals().warn_fg_color),
                        );
                    }
                } else {
                    ui.label(
                        "This Terraflux cannot replace itself (Windows' zip, an application or a folder in a place \
                         you may not write, a build): download the new one from its page.",
                    );
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if installer {
                        let text = if dirty { "Save and update" } else { "Update" };
                        let b = egui::Button::new(RichText::new(text).color(theme::on_orange()).strong())
                            .fill(theme::orange());
                        if ui.add(b).clicked() {
                            go = Some(dirty);
                        }
                    } else if ui.button("Open its page").clicked() {
                        let page = if r.page.is_empty() {
                            format!("https://github.com/{}/releases/latest", crate::update::REPO)
                        } else {
                            r.page.clone()
                        };
                        crate::update::open_page(&page);
                        later = true;
                    }
                    if ui.button("Not now").clicked() {
                        later = true;
                    }
                    if installer && !r.page.is_empty() && ui.link("what is new").clicked() {
                        crate::update::open_page(&r.page);
                    }
                });
            });
        if let Some(save) = go {
            if save {
                self.save(false);
                if self.session.as_ref().is_some_and(|s| s.dirty) {
                    // not saved (no file chosen, or it could not be written): asked again
                    return;
                }
            }
            let c = ctx.clone();
            self.updater.download(r, move || c.request_repaint());
            self.update_go = true;
            self.update_told = true;
            later = true;
        }
        if !open || later {
            self.update_asked = None;
        }
    }

    /// The installer downloaded and checked, the user's word given: installed once this application closes, then
    /// opened again with the model.
    fn install_update(&mut self, ctx: &egui::Context) {
        let crate::update::State::Ready(version, ready) = self.updater.state() else { return };
        if !std::mem::take(&mut self.update_go) {
            return;
        }
        let Some(how) = crate::update::install() else { return };
        let model = self.session.as_ref().filter(|s| !s.dirty).and_then(|s| s.path.clone());
        match crate::update::install_after_exit(&how, version, &ready, model.as_deref()) {
            Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(e) => self.updater.fail(e),
        }
    }

    /// The settings, in a window of the application: the theme, the text's size, the interface's zoom, the
    /// threads the functions use; kept at once, the look changed at once.
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        let mut st = crate::settings::get();
        egui::Window::new("Settings").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            egui::Grid::new("settings").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                ui.label("Theme");
                egui::ComboBox::from_id_salt("theme").selected_text(st.theme.clone()).width(180.0).show_ui(ui, |ui| {
                    for t in &theme::THEMES {
                        ui.horizontal(|ui| {
                            for c in [t.panel, t.canvas, t.accent] {
                                let (r, _) = ui.allocate_exact_size(Vec2::new(10.0, 14.0), egui::Sense::hover());
                                ui.painter().rect_filled(r, 1.0, c);
                            }
                            ui.selectable_value(&mut st.theme, t.name.to_string(), t.name);
                        });
                    }
                });
                ui.end_row();
                ui.label("Text size");
                ui.add(
                    egui::Slider::new(&mut st.text, 0.8..=1.6)
                        .step_by(0.05)
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
                );
                ui.end_row();
                ui.label("Interface zoom");
                ui.add(
                    egui::Slider::new(&mut st.zoom, 0.75..=2.0)
                        .step_by(0.05)
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
                );
                ui.end_row();
                ui.label("Model");
                ui.checkbox(&mut st.node_colours, "the nodes of data in the colour of their type").on_hover_text(
                    "the model's inputs and the nodes that read, write, stack or plot, with a header in the colour \
                     of what they give (a map blue, vectors pink...), as their connections",
                );
                ui.end_row();
                ui.label("Threads");
                ui.vertical(|ui| {
                    let most = terraflux_ops::threads::most();
                    let mut n = st.threads.unwrap_or(most).clamp(1, most);
                    let mut auto = st.threads.is_none();
                    ui.checkbox(&mut auto, format!("all the cores but one ({most})"));
                    ui.add_enabled_ui(!auto, |ui| {
                        ui.add(egui::Slider::new(&mut n, 1..=most));
                    });
                    st.threads = if auto { None } else { Some(n) };
                    ui.label(
                        RichText::new(format!(
                            "the threads the functions use; from the next start (now {})",
                            terraflux_ops::threads::ensure()
                        ))
                        .weak()
                        .size(11.0),
                    );
                });
                ui.end_row();
                ui.label("Cache");
                ui.vertical(|ui| {
                    let mut gb = st.cache_gb.unwrap_or(terraflux_ops::cache::DEFAULT_GB);
                    ui.add(egui::Slider::new(&mut gb, 1.0..=200.0).logarithmic(true).suffix(" GB").max_decimals(0));
                    st.cache_gb = (gb != terraflux_ops::cache::DEFAULT_GB).then_some(gb);
                    let dirs = terraflux_ops::cache::emptied();
                    if self.cache_used.is_none_or(|(t, _)| t.elapsed() > Duration::from_secs(3)) {
                        let used = dirs.iter().map(|d| terraflux_ops::cache::size(d)).sum();
                        self.cache_used = Some((Instant::now(), used));
                    }
                    let used = self.cache_used.map_or(0, |(_, b)| b);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{:.2} GB used", used as f64 / 1e9)).size(11.0));
                        if ui.add_enabled(used > 0, egui::Button::new("Empty")).clicked() {
                            for d in &dirs {
                                terraflux_ops::cache::clear(d);
                            }
                            self.cache_used = None;
                        }
                    })
                    .response
                    .on_hover_text(terraflux_ops::cache::root().display().to_string());
                    ui.label(
                        RichText::new(
                            "the rasters made without a file (reprojected, clipped...) and the data downloaded: kept \
                             to be used again, the least recently used removed beyond this size",
                        )
                        .weak()
                        .size(11.0),
                    );
                });
                ui.end_row();
                ui.label("Node libraries");
                ui.vertical(|ui| {
                    ui.label(RichText::new(crate::my_nodes::dir().display().to_string()).size(11.0))
                        .on_hover_text("your own library: the nodes you save");
                    let mut gone = None;
                    for (k, d) in st.node_libraries.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.small_button("x").on_hover_text("no longer used here (its folder stays)").clicked() {
                                gone = Some(k);
                            }
                            let missing = if d.is_dir() { "" } else { " (not found)" };
                            ui.label(RichText::new(format!("{}{missing}", d.display())).size(11.0));
                        });
                    }
                    if let Some(k) = gone {
                        st.node_libraries.remove(k);
                        crate::my_nodes::changed();
                    }
                    if ui.button("Add a shared library...").clicked()
                        && let Some(d) = rfd::FileDialog::new().pick_folder()
                        && !st.node_libraries.contains(&d)
                    {
                        st.node_libraries.push(d);
                        crate::my_nodes::changed();
                    }
                    ui.label(
                        RichText::new(
                            "folders of nodes shared with others (a group's folder on a shared drive, a repository \
                             cloned): their nodes offered beside yours, and saved in them when chosen",
                        )
                        .weak()
                        .size(11.0),
                    );
                });
                ui.end_row();
                ui.label("Updates");
                ui.vertical(|ui| {
                    use crate::update::{State, show};
                    ui.label(RichText::new(format!("Terraflux {}", env!("CARGO_PKG_VERSION"))).strong());
                    ui.checkbox(&mut st.check_updates, "look for a new version when Terraflux starts (once a day)")
                        .on_hover_text("asks GitHub which Terraflux is the latest; nothing else is sent");
                    ui.horizontal(|ui| {
                        let state = self.updater.state();
                        let busy = matches!(state, State::Checking | State::Downloading(..) | State::Ready(..));
                        if ui.add_enabled(!busy, egui::Button::new("Check now")).clicked() {
                            self.update_told = true;
                            let c = ctx.clone();
                            self.updater.check(move || c.request_repaint());
                        }
                        match state {
                            State::Idle => {}
                            State::Checking => {
                                ui.spinner();
                            }
                            State::UpToDate => {
                                ui.label(RichText::new("this is the latest").weak());
                            }
                            State::Available(r) => {
                                if ui.button(format!("Update to {}...", show(r.version))).clicked() {
                                    self.update_asked = Some(r);
                                }
                            }
                            State::Downloading(..) => {
                                ui.label(RichText::new("downloading...").weak());
                            }
                            State::Ready(..) => {
                                ui.label(RichText::new("installing...").weak());
                            }
                            State::Failed(e) => {
                                ui.label(RichText::new(e).color(ui.visuals().warn_fg_color).size(11.0));
                            }
                        }
                    });
                });
                ui.end_row();
            });
            // which GDAL reads the other formats and does the warping, if any
            ui.add_space(6.0);
            match terraflux_gdal::gdal() {
                Ok(g) => ui
                    .label(RichText::new(format!("GDAL {} · {} formats", g.version, g.drivers)).weak().size(11.0))
                    .on_hover_text(g.library.display().to_string()),
                Err(e) => ui
                    .label(RichText::new("no GDAL: GeoTIFF only").color(ui.visuals().warn_fg_color).size(11.0))
                    .on_hover_text(e.to_string()),
            };
            ui.add_space(6.0);
            if ui.button("Back to the defaults").clicked() {
                let favourites = std::mem::take(&mut st.favourites);
                let collection_names = std::mem::take(&mut st.collection_names);
                // the computers' choices are not looks: they stay
                st = crate::settings::Settings {
                    favourites,
                    collection_names,
                    share: st.share,
                    share_code: st.share_code.clone(),
                    lend: st.lend,
                    lend_code: st.lend_code.clone(),
                    lend_threads: st.lend_threads,
                    lend_idle: st.lend_idle,
                    node_libraries: st.node_libraries.clone(),
                    ..Default::default()
                };
            }
        });
        terraflux_ops::cache::set_limit(st.cache_gb);
        crate::settings::update(|s| *s = st);
        self.settings_open = open;
    }

    /// The strip at the window's left edge while the operators' or the files' pane is closed: a tab each, its name
    /// written along it; the pointer over a tab brings its drawer out, a click opens its pane. Where it is, and
    /// where its tabs are.
    fn drawers_strip(&mut self, ui: &mut Ui) -> (egui::Rect, Vec<(Pane, egui::Rect)>) {
        let open = self.drawer;
        let closed: Vec<Pane> = [Pane::Operators, Pane::Files].into_iter().filter(|p| !self.is_open(*p)).collect();
        let c = crate::commands::cmd();
        let (rect, tabs, clicked) = egui::Panel::left("drawers strip")
            .exact_size(20.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(theme::bar()))
            .show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                let mut tabs = Vec::new();
                let mut clicked = None;
                let mut top = rect.min.y + 6.0;
                for pane in closed {
                    let (word, long, hint) = match pane {
                        Pane::Operators => (
                            "OPERATORS  ›",
                            112.0,
                            format!(
                                "The operators: here to see them, a click keeps their pane open (also \"/\", {c}K and \
                                 the Operators menu)"
                            ),
                        ),
                        _ => (
                            "FILES  ›",
                            72.0,
                            "The computer's folders: here to see them and drag data to the model, a click keeps their \
                             pane open"
                                .to_string(),
                        ),
                    };
                    let tab = egui::Rect::from_min_size(egui::pos2(rect.min.x + 2.0, top), Vec2::new(16.0, long));
                    top += long + 8.0;
                    let resp = ui.interact(tab, ui.id().with(("drawer tab", pane)), egui::Sense::click());
                    let hot = open == Some(pane) || resp.hovered();
                    let v = ui.visuals();
                    let (fill, text) = if hot {
                        (v.widgets.hovered.bg_fill, v.strong_text_color())
                    } else {
                        (v.widgets.inactive.weak_bg_fill, v.text_color())
                    };
                    ui.painter().rect_filled(tab, crate::theme::rounded(3.0), fill);
                    // the word along it, bottom to top
                    let galley = ui.painter().layout_no_wrap(word.to_string(), egui::FontId::proportional(10.5), text);
                    let at = egui::pos2(tab.center().x - galley.size().y / 2.0, tab.center().y + galley.size().x / 2.0);
                    ui.painter()
                        .add(egui::epaint::TextShape::new(at, galley, text).with_angle(-std::f32::consts::FRAC_PI_2));
                    if resp.on_hover_text(hint).clicked() {
                        clicked = Some(pane);
                    }
                    tabs.push((pane, tab));
                }
                (rect, tabs, clicked)
            })
            .inner;
        if let Some(p) = clicked {
            self.drawer = None;
            self.focus(p);
        }
        (rect, tabs)
    }

    /// The drawer of the operators or of the files, while their pane is closed: out over the panes while the
    /// pointer is over its tab of the strip at the window's left edge (the operators' along the rest of the edge)
    /// or over the drawer, in again when the pointer leaves it (not while something is dragged out of it).
    fn drawer(&mut self, ctx: &egui::Context, area: egui::Rect, strip: Option<(egui::Rect, Vec<(Pane, egui::Rect)>)>) {
        const WIDTH: f32 = 300.0;
        if self.drawer.is_some_and(|p| self.is_open(p)) {
            self.drawer = None;
        }
        let Some(s) = &mut self.session else { return };
        let pointer = ctx.input(|i| i.pointer.latest_pos());
        let dragging = ctx.dragged_id().is_some();
        if let Some(p) = pointer {
            // (up to the strip's right side: the window's edge too)
            let edge = strip.as_ref().map_or(area.left() + 3.0, |(r, _)| r.right());
            let at_edge = p.x <= edge && area.y_range().contains(p.y);
            let tabs = strip.as_ref().map(|(_, t)| t.as_slice()).unwrap_or(&[]);
            // over a tab: its drawer; elsewhere along the edge: the operators', when theirs is closed
            let under = tabs
                .iter()
                .find(|(_, r)| r.y_range().contains(p.y))
                .map(|(pane, _)| *pane)
                .or_else(|| tabs.iter().any(|(pane, _)| *pane == Pane::Operators).then_some(Pane::Operators));
            if !dragging && at_edge && under.is_some() && under != self.drawer {
                self.drawer = under;
            } else if self.drawer.is_some()
                && !dragging
                && (p.x > area.left() + WIDTH + 24.0 || !area.y_range().contains(p.y))
                && !ctx.any_popup_open()
            {
                self.drawer = None;
            }
        }
        let Some(pane) = self.drawer else { return };
        let before = s.revision;
        let mut open = None;
        egui::Area::new(egui::Id::new(("drawer", pane))).fixed_pos(area.min).order(egui::Order::Foreground).show(
            ctx,
            |ui| {
                egui::Frame::new()
                    .fill(ui.visuals().panel_fill)
                    .shadow(ui.visuals().popup_shadow)
                    .stroke(ui.visuals().window_stroke)
                    .show(ui, |ui| {
                        ui.set_width(WIDTH);
                        ui.set_height(area.height());
                        if pane == Pane::Operators {
                            crate::builder::operators_pane(ui, s, &self.registry, &self.library);
                        } else if let Some(crate::files_view::Asked::Open(p)) =
                            crate::files_view::pane(ui, &mut self.files, s, &self.registry)
                        {
                            open = Some(p);
                        }
                    });
            },
        );
        // an operator added: the drawer goes in
        if pane == Pane::Operators && s.revision != before {
            self.drawer = None;
        }
        if let Some(p) = open {
            self.drawer = None;
            self.open(&p);
        }
    }

    /// The bar at the bottom: the cell under the pointer, else what the keys and the mouse do in the pane in
    /// focus; at the right, the warnings and what was just done.
    /// (Whether the problems are asked for.)
    fn status(&self, ui: &mut Ui, focused: Option<Pane>) -> bool {
        let mut problems = false;
        ui.horizontal(|ui| {
            let weak = ui.visuals().weak_text_color();
            let batch = match (&self.batch.running, &self.batch.interrupted) {
                (Some(_), _) => Some("a batch runs (Many runs)"),
                (None, Some(r)) if !r.elsewhere() => Some("a batch stopped before its end (Many runs)"),
                _ => None,
            };
            // the right side first, what is at the left in the room left (cut short when it does not fit)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(s) = &self.session {
                    if let Some(b) = batch {
                        ui.label(RichText::new(b).strong());
                        ui.separator();
                    }
                    // a word of what was done (a file saved), for a while
                    if let Some((text, at)) = &s.notice
                        && at.elapsed() < Duration::from_secs(8)
                    {
                        ui.label(RichText::new(text).strong());
                        ui.separator();
                    }
                    // the problems, a click away
                    let (errors, warnings) = crate::problems_view::counts(s);
                    if errors + warnings > 0 {
                        let (text, colour) = if s.error.is_some() {
                            ("the run failed: why?".to_string(), ui.visuals().error_fg_color)
                        } else if errors > 0 {
                            (format!("{errors} problems, {warnings} warnings"), ui.visuals().error_fg_color)
                        } else {
                            (format!("{warnings} warnings"), ui.visuals().warn_fg_color)
                        };
                        if ui
                            .add(egui::Button::new(RichText::new(text).color(colour)).frame(false))
                            .on_hover_text("the Problems pane: each explained, a click away from its node")
                            .clicked()
                        {
                            problems = true;
                        }
                    }
                    // the model's health: how its checks fared in the last run
                    let checks = crate::checks::of(s);
                    if let (Some(h), Some(w)) = (crate::checks::health(&checks), crate::checks::worst(&checks)) {
                        ui.separator();
                        ui.label(RichText::new(format!("{} {h}", w.symbol())).color(w.colour(ui.visuals())))
                            .on_hover_text(crate::document_view::check_list(&checks));
                    }
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| match &self.hover {
                    Some(h) => {
                        let value = if h.value.is_nan() {
                            "no value".to_string()
                        } else {
                            format!("value {}", terraflux_ops::value::class_label(h.value))
                        };
                        ui.label(RichText::new(value).strong());
                        let at = format!("row {}  column {}   x {:.2}  y {:.2}", h.row, h.col, h.x, h.y);
                        ui.add(egui::Label::new(RichText::new(at).monospace().color(weak)).truncate());
                    }
                    None if self.session.is_some() => {
                        ui.add(egui::Label::new(RichText::new(hints(focused)).color(weak)).truncate());
                    }
                    None => {}
                });
            });
        });
        problems
    }

    fn map_tab(&mut self, ui: &mut Ui) {
        let Some(s) = &mut self.session else { return };
        if s.loading() {
            ui.centered_and_justified(|ui| {
                ui.horizontal_centered(|ui| {
                    ui.spinner();
                    ui.label("Reading the data...");
                });
            });
            return;
        }
        if s.maps.is_empty() && s.layers.is_empty() {
            let text = match &s.load_error {
                Some(e) => format!("The model's inputs could not be read: {e}"),
                None => "A model's maps show here: those of its inputs, and those of a loop (control.repeat) step by step as it runs."
                    .to_string(),
            };
            ui.label(RichText::new(text).color(ui.visuals().weak_text_color()));
            return;
        }
        if s.layer.is_none() && s.maps.is_empty() {
            s.layer = Some(0);
        }
        // the keyboard and the player go through the maps of the layer
        let n = s.frames();
        if n > 1 {
            let typing = ui.ctx().memory(|m| m.focused().is_some());
            let (back, next) =
                ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight)));
            if !typing && back {
                s.set_frame(s.frame().saturating_sub(1));
            }
            if !typing && next {
                s.set_frame((s.frame() + 1).min(n - 1));
            }
            if s.playing {
                let period = Duration::from_millis(if s.layer.is_none() { 200 } else { 600 });
                if s.tick.elapsed() >= period {
                    s.tick = Instant::now();
                    s.set_frame((s.frame() + 1) % n);
                }
                ui.ctx().request_repaint_after(period);
            }
        } else {
            s.playing = false;
        }
        let current = s.current();
        egui::Panel::left("map_side").resizable(true).default_size(270.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                theme::section(ui, "Layers");
                if !s.maps.is_empty() {
                    let note =
                        if s.maps.len() == 1 { "the start".to_string() } else { format!("{} steps", s.maps.len() - 1) };
                    if row(ui, s.layer.is_none(), "Simulation", &note).clicked() {
                        s.layer = None;
                        s.playing = false;
                    }
                }
                for i in 0..s.layers.len() {
                    let l = &s.layers[i];
                    let note = if l.stack { format!("stack of {}", l.bands.len()) } else { "map".to_string() };
                    if row(ui, s.layer == Some(i), &l.name, &note).clicked() {
                        s.layer = Some(i);
                        s.playing = false;
                    }
                }
                let n = s.frames();
                if n > 1 {
                    ui.add_space(6.0);
                    theme::section(ui, if s.layer.is_none() { "Step" } else { "Map of the stack" });
                    let mut k = s.frame();
                    ui.horizontal(|ui| {
                        if ui.button("<").on_hover_text("previous (left)").clicked() {
                            k = k.saturating_sub(1);
                        }
                        let play = if s.playing { "Pause" } else { "Play" };
                        if ui.add(egui::Button::selectable(s.playing, play)).clicked() {
                            s.playing = !s.playing;
                            s.tick = Instant::now();
                        }
                        if ui.button(">").on_hover_text("next (right)").clicked() {
                            k = (k + 1).min(n - 1);
                        }
                    });
                    ui.spacing_mut().slider_width = ui.available_width() - 8.0;
                    ui.add(egui::Slider::new(&mut k, 0..=n - 1).show_value(false));
                    if k != s.frame() {
                        s.set_frame(k);
                    }
                    match s.layer {
                        None => {
                            ui.label(if s.frame() == 0 {
                                "the map the simulation starts from".to_string()
                            } else {
                                format!("after step {} of {}", s.frame(), s.maps.len() - 1)
                            });
                            ui.checkbox(&mut s.follow, "follow the run");
                        }
                        Some(i) => {
                            let l = &s.layers[i];
                            ui.label(format!("{}  ({} of {})", l.bands[l.shown].0, l.shown + 1, l.bands.len()));
                        }
                    }
                    // the steps, or the maps of the stack, as an animation
                    let name = match s.layer {
                        None => "simulation".to_string(),
                        Some(i) => s.layers[i].name.trim_start_matches('$').to_string(),
                    };
                    let coloring = current.as_ref().map(|c| c.1.coloring.clone());
                    let delay = if s.layer.is_none() { 200 } else { 600 };
                    let (maps, layers, layer) = (&s.maps, &s.layers, s.layer);
                    s.animations.button(ui, &name, delay, || {
                        let frames: Vec<(String, Map, Coloring)> = match layer {
                            // every step in the simulation's colours
                            None => {
                                let (last, coloring) =
                                    (maps.len() - 1, coloring.unwrap_or(Coloring::Classes(Vec::new())));
                                let caption =
                                    |k: usize| if k == 0 { "the start".into() } else { format!("step {k} of {last}") };
                                maps.iter()
                                    .enumerate()
                                    .map(|(k, m)| (caption(k), m.clone(), coloring.clone()))
                                    .collect()
                            }
                            // each map of the stack in its own colours, as shown
                            Some(i) => layers[i]
                                .bands
                                .iter()
                                .map(|(b, m, look)| (b.clone(), m.clone(), look.coloring.clone()))
                                .collect(),
                        };
                        move || {
                            frames
                                .into_iter()
                                .map(|(caption, m, coloring)| crate::animation::Frame {
                                    source: (&m).into(),
                                    coloring,
                                    caption,
                                })
                                .collect()
                        }
                    });
                }
                // every map of a stack at once, a click shows one
                if let Some(i) = s.layer
                    && s.layers[i].stack
                {
                    ui.add_space(6.0);
                    let cols = 2;
                    let w = (ui.available_width() - 8.0 - (cols - 1) as f32 * 6.0) / cols as f32;
                    let mut pick = None;
                    egui::Grid::new("thumbs").spacing([6.0, 6.0]).show(ui, |ui| {
                        for (b, (name, m, look)) in s.layers[i].bands.iter().enumerate() {
                            let (id, size) = s.thumbs.get(ui.ctx(), m, &look.coloring);
                            let size = size * (w / size.x);
                            let shown = b == s.layers[i].shown;
                            ui.vertical(|ui| {
                                let r = ui.add(egui::Image::new((id, size)).sense(egui::Sense::click()));
                                let stroke = if shown {
                                    egui::Stroke::new(2.0, theme::accent())
                                } else {
                                    egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
                                };
                                ui.painter().rect_stroke(r.rect, 0.0, stroke, egui::StrokeKind::Outside);
                                let text = RichText::new(name).size(11.0);
                                ui.label(if shown { text.color(theme::accent()) } else { text });
                                if r.clicked() {
                                    pick = Some(b);
                                }
                                r.on_hover_text(name);
                            });
                            if (b + 1) % cols == 0 {
                                ui.end_row();
                            }
                        }
                    });
                    if let Some(b) = pick {
                        s.layers[i].shown = b;
                        s.playing = false;
                    }
                }
                if let Some((_, look, _)) = &current {
                    ui.add_space(6.0);
                    theme::section(ui, "Legend");
                    legend(ui, &look.coloring, &look.counts, &|v| terraflux_ops::value::class_label(v));
                }
                if !s.warnings.is_empty() {
                    ui.add_space(6.0);
                    theme::section(ui, &format!("{} warnings", s.warnings.len()));
                    egui::ScrollArea::vertical().id_salt("warnings").max_height(160.0).show(ui, |ui| {
                        for w in &s.warnings {
                            ui.small(w);
                        }
                    });
                }
                if let Some(e) = &s.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::well())).show(ui, |ui| {
            if let Some((map, look, name)) = &current {
                self.hover = s.view.show(ui, &crate::map_view::Source::from(map), &look.coloring);
                // what is shown, in the corner
                let at = ui.max_rect().left_top() + Vec2::new(10.0, 8.0);
                let galley =
                    ui.painter().layout_no_wrap(name.clone(), egui::FontId::proportional(13.0), Color32::WHITE);
                let back = egui::Rect::from_min_size(at, galley.size()).expand2(Vec2::new(6.0, 3.0));
                ui.painter().rect_filled(back, 2.0, Color32::from_black_alpha(150));
                ui.painter().galley(at, galley, Color32::WHITE);
            }
        });
    }

    fn charts_tab(&mut self, ui: &mut Ui) {
        let Some(s) = &self.session else { return };
        let areas = s.areas();
        if areas.len() < 2 {
            ui.label("Run the model to see the area of each class along the steps.");
            return;
        }
        let mut classes: Vec<f64> = areas.iter().flat_map(|a| a.iter().map(|(c, _)| *c)).collect();
        classes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        classes.dedup();
        let colour = |c: f64| match &s.coloring {
            Some(Coloring::Classes(v)) => v.iter().find(|(k, _)| *k == c).map_or(Color32::GRAY, |(_, x)| *x),
            _ => Color32::GRAY,
        };
        crate::theme::plot(Plot::new("areas"))
            .legend(Legend::default())
            .x_axis_label("step")
            .y_axis_label("cells")
            .show(ui, |p| {
                for &c in &classes {
                    let pts: Vec<[f64; 2]> = areas
                        .iter()
                        .enumerate()
                        .map(|(k, a)| [k as f64, a.iter().find(|(x, _)| *x == c).map_or(0, |(_, n)| *n) as f64])
                        .collect();
                    p.line(
                        Line::new(format!("class {}", terraflux_ops::value::class_label(c)), pts)
                            .color(colour(c))
                            .width(2.0),
                    );
                }
            });
    }

    fn validation_tab(&mut self, ui: &mut Ui) {
        let Some(s) = &mut self.session else { return };
        ui.label("Compare the last simulated map with an observed one, both from the same start: cell by cell (the figure of merit) and in neighbourhoods of growing size (Dinamica EGO's fuzzy similarity of the changes).");
        ui.horizontal(|ui| {
            if ui.button("Observed map...").clicked()
                && let Some(p) = rfd::FileDialog::new().add_filter("GeoTIFF", &["tif", "tiff"]).pick_file()
            {
                match Raster::read(&p) {
                    Ok(r) => {
                        s.validation.bands = r.band_names();
                        s.validation.band = s.validation.bands.len() - 1;
                        s.validation.observed = Some(p);
                    }
                    Err(e) => s.validation.error = Some(e.to_string()),
                }
            }
            ui.label(s.validation.observed.as_ref().map_or("none".into(), |p| p.display().to_string()));
        });
        if !s.validation.bands.is_empty() {
            let bands = s.validation.bands.clone();
            egui::ComboBox::from_label("band").selected_text(&bands[s.validation.band]).show_ui(ui, |ui| {
                for (i, b) in bands.iter().enumerate() {
                    ui.selectable_value(&mut s.validation.band, i, b);
                }
            });
        }
        ui.horizontal(|ui| {
            ui.label("windows");
            ui.text_edit_singleline(&mut s.validation.windows);
        });
        if ui.add_enabled(s.maps.len() > 1 && s.validation.observed.is_some(), egui::Button::new("Compare")).clicked() {
            s.validate();
        }
        if let Some(e) = &s.validation.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        if let Some(a) = &s.validation.agreement {
            ui.separator();
            egui::Grid::new("agreement").num_columns(2).show(ui, |ui| {
                for (k, v) in [
                    ("same class", format!("{:.2}%", 100.0 * a.overall)),
                    ("figure of merit", format!("{:.4}", a.figure_of_merit)),
                    ("hits (change where it happened, right class)", a.hits.to_string()),
                    ("wrong hits (where it happened, another class)", a.wrong_hits.to_string()),
                    ("misses (change that did not happen)", a.misses.to_string()),
                    ("false alarms (change where none happened)", a.false_alarms.to_string()),
                ] {
                    ui.label(k);
                    ui.label(v);
                    ui.end_row();
                }
            });
        }
        if let Some(t) = &s.validation.similarity {
            ui.separator();
            ui.label(RichText::new("Fuzzy similarity of the changes").strong());
            egui::Grid::new("similarity").num_columns(4).striped(true).show(ui, |ui| {
                for h in ["window", "simulated", "observed", "minimum"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for r in &t.rows {
                    ui.label(format!("{}", r[0]));
                    ui.label(format!("{:.4}", r[1]));
                    ui.label(format!("{:.4}", r[2]));
                    ui.label(format!("{:.4}", r[3]));
                    ui.end_row();
                }
            });
        }
    }

    /// The keys of the whole application, in the main window when it has the focus (a window of its own takes
    /// its keys itself): F5 runs the model, ctrl+S saves it, ctrl+O opens a raster or vectors, Tab goes to the
    /// model and to the last map seen, ctrl+W closes the tab in focus.
    fn keys(&mut self, ui: &mut Ui) {
        if !ui.input(|i| i.focused) {
            return;
        }
        // the map seen last: a map's tab, or the viewer
        if let Some(p @ (Pane::Map(_) | Pane::Viewer)) = self.focused() {
            self.tab_back = Some(p);
        }
        let keys = Keys::read(ui);
        if keys.tab {
            self.tabbed = true;
            match self.focused() {
                Some(Pane::Model) => {
                    if let Some(back) = self.tab_back
                        && self.is_open(back)
                    {
                        self.focus(back);
                    }
                }
                _ => self.focus(Pane::Model),
            }
        }
        if keys.close
            && let Some(p) = self.focused()
        {
            if let (Pane::Map(id), Some(s)) = (p, &mut self.session) {
                s.map_tabs.remove(&id);
            }
            self.close(p);
        }
        self.app_keys(ui.ctx(), &keys);
    }

    /// What the keys of any window ask of the application: run, save, open data or a model, a new model, the
    /// commands, the shortcuts.
    fn app_keys(&mut self, ctx: &egui::Context, keys: &Keys) {
        if keys.save || keys.save_as {
            self.save(keys.save_as);
        }
        if keys.new {
            self.new_model();
        }
        if keys.open_model {
            self.open_dialog();
        }
        if keys.commands {
            self.commands = if self.commands.is_some() { None } else { Some(Default::default()) };
        }
        if keys.shortcuts {
            self.shortcuts_open = !self.shortcuts_open;
        }
        if let Some(s) = &mut self.session
            && keys.open
        {
            crate::builder::open_data(s, &self.registry);
        }
        if keys.run {
            self.run_all(ctx);
        }
    }

    /// The main window's title: the model's name, a dot when it has edits not saved.
    fn window_title(&mut self, ctx: &egui::Context) {
        let title = match &self.session {
            Some(s) => {
                format!("{}{} - Terraflux {}", s.title(), if s.dirty { " ●" } else { "" }, env!("CARGO_PKG_VERSION"))
            }
            None => format!("Terraflux {}", env!("CARGO_PKG_VERSION")),
        };
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    /// Files dragged from the system's file manager to a window: shown while they are over it, a node each that
    /// reads it when they are let go (a model dropped is opened).
    fn take_dropped(&mut self, ui: &Ui) {
        let (dropped, over): (Vec<PathBuf>, usize) = ui.input(|i| {
            (i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect(), i.raw.hovered_files.len())
        });
        if over > 0 {
            let screen = ui.ctx().content_rect();
            let p = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("files over")));
            p.rect_filled(screen, 0.0, Color32::from_black_alpha(110));
            p.rect_stroke(screen.shrink(6.0), 8.0, (2.0, theme::accent()), egui::StrokeKind::Inside);
            let text =
                if over == 1 { "a node that reads it".to_string() } else { format!("{over} nodes that read them") };
            p.text(
                screen.center(),
                egui::Align2::CENTER_CENTER,
                format!("Drop: {text}"),
                egui::FontId::proportional(20.0),
                Color32::WHITE,
            );
        }
        if dropped.is_empty() {
            return;
        }
        let Some(s) = &mut self.session else { return };
        match crate::builder::add_files(s, &self.registry, &dropped) {
            Some(model) => self.open(&model),
            None => ui.ctx().request_repaint(),
        }
    }

    /// The viewer opened (beside the model, in front), once, when the first data comes: a raster or vectors read,
    /// a model with inputs, a template started.
    fn open_viewer(&mut self) {
        if !self.viewer_opened && self.has_data() {
            self.add_viewer();
        }
    }

    /// Whether the model reads data: inputs, or nodes that read rasters or vectors.
    fn has_data(&self) -> bool {
        self.session.as_ref().is_some_and(|s| {
            !s.source.inputs.is_empty() || s.source.nodes.iter().any(|n| n.op == "raster.read" || n.op == "vector.read")
        })
    }

    /// The viewer's tab opened beside the model's (if it is not open), behind it: the graph stays in front, and the
    /// focus where it was.
    fn add_viewer(&mut self) {
        self.viewer_opened = true;
        if self.is_open(Pane::Viewer) {
            return;
        }
        let before = self.focused();
        self.focus(Pane::Viewer);
        self.focus(Pane::Model);
        if let Some(b) = before.filter(|p| !matches!(p, Pane::Viewer | Pane::Model)) {
            self.focus(b);
        }
    }

    /// One frame of the application.
    pub fn frame(&mut self, ui: &mut Ui) {
        if let Some(k) = &mut self.maximize {
            *k += 1;
            if *k >= 3 {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                self.maximize = None;
            }
        }
        // the style of the settings (again when they change)
        let st = crate::settings::get();
        let key = (st.theme.clone(), st.text.to_bits(), st.zoom.to_bits());
        if self.styled.as_ref() != Some(&key) {
            theme::choose(&st.theme);
            theme::apply_with(ui.ctx(), st.text);
            ui.ctx().set_zoom_factor(st.zoom.clamp(0.5, 3.0));
            self.styled = Some(key);
        }
        if let Some(s) = &mut self.session {
            s.poll();
            crate::watch::follow(s, ui.ctx());
            s.auto_run(ui.ctx());
            self.chat.poll(s, &self.registry, ui.ctx());
            crate::review::follow(s);
            if s.running() || s.loading() || s.saving() {
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
        }
        // a cell's trace asked for: in its pane; the problems asked for: theirs
        if let Some(a) = self.session.as_mut().and_then(|s| s.trace_asked.take()) {
            self.trace.ask(a);
            self.focus(Pane::Trace);
        }
        if self.session.as_mut().is_some_and(|s| std::mem::take(&mut s.ask_problems)) {
            self.focus(Pane::Problems);
        }
        if self.session.as_mut().is_some_and(|s| std::mem::take(&mut s.ask_calculator)) {
            self.focus(Pane::Calculator);
        }
        self.open_viewer();
        // a review begun by another program (or asked for): its pane in front
        if self.session.as_mut().and_then(|s| s.review.as_mut()).is_some_and(|r| std::mem::take(&mut r.show)) {
            self.focus(Pane::Review);
        }
        // a batch goes on whether its pane is shown or not
        self.batch.poll();
        if self.batch.running() {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
        // a comparison of the Parameters pane run in Many runs: its runs back there
        self.params.in_batch = self.batch.compare.is_some() && self.batch.running();
        if let Some(c) = self.batch.compared.take() {
            self.params.from_batch = Some(c);
            ui.ctx().request_repaint();
        }
        // the cell under the pointer, from the map that has it this frame
        self.hover = None;
        if self.wizard.is_none() {
            self.keys(ui);
            let canvas = self.focused() == Some(Pane::Model);
            let mut show = false;
            if let Some(s) = &mut self.session {
                show = crate::builder::keys(ui, s, &self.registry, canvas);
                // the operators by name, from the keyboard ("/"), over the panes (its keys taken before theirs)
                crate::builder::quick_add(ui.ctx(), s, &self.registry, &self.library);
            }
            if show {
                self.focus(Pane::Viewer);
            }
        }
        let bar = egui::Frame::new().fill(theme::bar()).inner_margin(Margin::symmetric(8, 5));
        let top = if theme::modern() { bar.inner_margin(Margin::symmetric(10, 7)) } else { bar };
        egui::Panel::top("toolbar").frame(top).show(ui, |ui| self.toolbar(ui));
        let focused = self.focused();
        let problems = egui::Panel::bottom("status")
            .frame(bar.inner_margin(Margin::symmetric(8, 3)))
            .show(ui, |ui| self.status(ui, focused));
        if problems.inner {
            self.focus(Pane::Problems);
        }
        self.window_title(ui.ctx());
        if let Some(w) = &mut self.wizard {
            let mut close = false;
            let mut created = None;
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // a column of reading width in the middle
                    let width = ui.available_width().min(820.0);
                    ui.horizontal(|ui| {
                        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                        ui.vertical(|ui| {
                            ui.set_width(width);
                            match w.show(ui) {
                                Some(Outcome::Cancel) => close = true,
                                Some(Outcome::Created(p)) => created = Some(p),
                                Some(Outcome::Open) | None => {}
                            }
                        });
                    });
                });
            });
            if close {
                self.wizard = None;
            }
            if let Some(p) = created {
                self.wizard = None;
                self.open(&p);
            }
            return;
        }
        if let Some(m) = self.message.clone() {
            egui::Panel::top("message")
                .frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::symmetric(8, 4)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(ui.visuals().error_fg_color, m);
                        if ui.small_button("x").clicked() {
                            self.message = None;
                        }
                    });
                });
        }
        self.settings_window(ui.ctx());
        self.update_window(ui.ctx());
        self.install_update(ui.ctx());
        let mut shortcuts = self.shortcuts_open;
        crate::commands::shortcuts_window(ui.ctx(), &mut shortcuts);
        self.shortcuts_open = shortcuts;
        if let Some(s) = &mut self.session {
            crate::methods::window(ui.ctx(), s, &self.registry);
            crate::qgis::window(ui.ctx(), s);
            crate::catalog::window(ui.ctx(), s, &self.registry);
            crate::python::window(ui.ctx(), s);
            crate::figure::window(ui.ctx(), s);
        }
        if self.commands.is_some() {
            let all = self.command_list();
            if let Some(d) = crate::commands::show(ui.ctx(), &mut self.commands, &all) {
                self.command(ui.ctx(), d);
            }
        }
        self.take_dropped(ui);
        // the operators' or the files' pane closed: a strip at the window's left edge says they are there
        let strip = (self.session.is_some() && !(self.is_open(Pane::Operators) && self.is_open(Pane::Files)))
            .then(|| self.drawers_strip(ui));
        let area = ui.available_rect_before_wrap();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::bar())).show(ui, |ui| self.docks(ui));
        self.drawer(ui.ctx(), area, strip);
        // Tab is ours: no field keeps the focus egui gave it
        if std::mem::take(&mut self.tabbed) {
            ui.ctx().memory_mut(|m| {
                if let Some(f) = m.focused() {
                    m.surrender_focus(f);
                }
            });
        }
        self.keep_layout();
    }

    /// A pane's content.
    fn pane(&mut self, ui: &mut Ui, pane: Pane) {
        let Some(s) = &mut self.session else { return };
        let (r, lib) = (&self.registry, &self.library);
        match pane {
            Pane::Model => crate::builder::model_pane(ui, s, r),
            Pane::Document => {
                if crate::document_view::pane(ui, s, r) {
                    self.focus(Pane::Parameters);
                }
            }
            Pane::Operators => crate::builder::operators_pane(ui, s, r, lib),
            Pane::Files => {
                if let Some(crate::files_view::Asked::Open(p)) = crate::files_view::pane(ui, &mut self.files, s, r) {
                    self.open(&p);
                }
            }
            Pane::Node => crate::builder::node_pane(ui, s, r),
            Pane::Prepare => crate::prepare_view::pane(ui, s, r),
            Pane::Viewer => {
                if let Some(h) = crate::builder::viewer_pane(ui, s, r) {
                    self.hover = Some(h);
                }
            }
            Pane::Simulation => {
                egui::Frame::new().inner_margin(Margin::same(6)).show(ui, |ui| self.map_tab(ui));
            }
            Pane::Areas => {
                egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| self.charts_tab(ui));
            }
            Pane::Validation => {
                egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| self.validation_tab(ui));
            }
            Pane::Batch => crate::batch_view::pane(ui, &mut self.batch, s, r, &mut self.computers),
            Pane::Computers => crate::computers::pane(ui, &mut self.computers),
            Pane::Assistant => {
                if crate::chat::pane(ui, &mut self.chat, s) {
                    self.focus(Pane::Review);
                }
            }
            Pane::Review => crate::review::pane(ui, s, r),
            Pane::Calculator => match crate::calc_view::pane(ui, &mut self.calculator, s) {
                Some(crate::calc_view::Asked::RunTo(id)) => crate::builder::run_to(ui, s, Some(id)),
                Some(crate::calc_view::Asked::Assistant(text)) => {
                    self.chat.send(text, s, ui.ctx());
                    self.focus(Pane::Assistant);
                }
                None => {}
            },
            Pane::Problems => {
                if let Some(crate::problems_view::Asked::Assistant(text)) = crate::problems_view::pane(ui, s, r) {
                    self.chat.send(text, s, ui.ctx());
                    self.focus(Pane::Assistant);
                }
            }
            Pane::Trace => {
                let asked = crate::trace_view::pane(ui, &mut self.trace, s, r);
                if let Some((_, _, k)) = &self.trace.kept {
                    self.chat.kept = k.clone();
                }
                match asked {
                    Some(crate::trace_view::Asked::Assistant(text)) => {
                        self.chat.send(text, s, ui.ctx());
                        self.focus(Pane::Assistant);
                    }
                    Some(crate::trace_view::Asked::Viewer) => self.focus(Pane::Viewer),
                    None => {}
                }
            }
            Pane::Parameters => match crate::params_view::pane(ui, &mut self.params, s, r) {
                Some(crate::params_view::Asked::ManyRuns) => {
                    crate::params_view::to_batch(&mut self.batch, &s.source);
                    self.focus(Pane::Batch);
                }
                Some(crate::params_view::Asked::Calibrate) => {
                    crate::params_view::to_calibration(&mut self.batch, &s.source);
                    self.focus(Pane::Batch);
                }
                Some(crate::params_view::Asked::CompareInManyRuns(output)) => {
                    let chosen = self.params.chosen.clone();
                    crate::params_view::to_batch_compare(&mut self.batch, &s.source, &chosen, &output);
                    match crate::batch_view::start(&mut self.batch, s, r, &mut self.computers, ui.ctx()) {
                        Ok(()) => {
                            self.params.in_batch = true;
                            self.params.error = None;
                        }
                        Err(e) => {
                            self.batch.compare = None;
                            self.params.error = Some(e);
                        }
                    }
                }
                None => {}
            },
            Pane::Map(id) => {
                let Some(mut shown) = s.map_tabs.remove(&id) else {
                    ui.label(RichText::new("This map's node is gone.").weak());
                    return;
                };
                let hover = crate::builder::output(ui, s, r, &mut shown, &format!("map {id}"));
                s.map_tabs.insert(id, shown);
                if hover.is_some() {
                    self.hover = hover;
                }
            }
        }
    }

    /// Close a pane wherever it is.
    pub fn close(&mut self, pane: Pane) {
        if let Some(d) = &mut self.dock {
            retain_panes(d, |t| *t != pane);
        }
        for w in &mut self.windows {
            retain_panes(&mut w.dock, |t| *t != pane);
        }
        self.windows.retain(|w| w.dock.iter_all_tabs().next().is_some());
    }

    /// The map tabs asked for put in the dock (beside the viewer, else where the focus is); those of nodes no
    /// longer open taken away.
    fn place_map_tabs(&mut self) {
        let (opened, raised, quiet, known): (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>) = match &mut self.session {
            Some(s) => (
                std::mem::take(&mut s.opened),
                std::mem::take(&mut s.raised),
                std::mem::take(&mut s.quiet),
                s.map_tabs.keys().copied().collect(),
            ),
            None => (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        };
        let stale = |t: &mut Pane| !matches!(t, Pane::Map(id) if !known.contains(id));
        let dock = self.dock.get_or_insert_with(default_layout);
        retain_panes(dock, stale);
        for w in &mut self.windows {
            retain_panes(&mut w.dock, stale);
        }
        self.windows.retain(|w| w.dock.iter_all_tabs().next().is_some());
        for id in opened {
            self.beside_viewer(Pane::Map(id), true);
        }
        for id in quiet {
            self.beside_viewer(Pane::Map(id), false);
        }
        // (brought to the front where it is, or put back if its window went away)
        for id in raised {
            self.focus(Pane::Map(id));
        }
    }

    /// A tab put beside the viewer's wherever it is (in the main window or one of its own), else beside the maps
    /// open, else beside the model's, else where the focus is; brought to the front (not `front`: unless the model's tab is in front there).
    fn beside_viewer(&mut self, pane: Pane, front: bool) {
        let is_map = |t: &Pane| matches!(t, Pane::Map(_));
        let dock = self.dock.get_or_insert_with(default_layout);
        for d in std::iter::once(dock).chain(self.windows.iter_mut().map(|w| &mut w.dock)) {
            let at = d
                .find_tab(&Pane::Viewer)
                .or_else(|| d.iter_all_tabs().find(|(_, t)| is_map(t)).map(|(p, _)| p))
                .or_else(|| d.find_tab(&Pane::Model));
            if let Some(path) = at
                && let Ok(leaf) = d.leaf_mut(path.node_path())
            {
                let before = leaf.active;
                let model = leaf.tabs().get(before.0) == Some(&Pane::Model);
                leaf.append_tab(pane);
                if !front && model {
                    leaf.active = before;
                } else if let Some(p) = d.find_tab(&pane) {
                    let _ = d.set_active_tab(p);
                }
                return;
            }
        }
        self.dock.get_or_insert_with(default_layout).push_to_focused_leaf(pane);
    }

    /// The pane whose tab has the focus (of the main window).
    pub fn focused(&mut self) -> Option<Pane> {
        self.dock.as_mut()?.find_active_focused().map(|(_, t)| *t)
    }

    /// Show a pane: its tab brought to the front where it is (and given the focus), or opened in the main window.
    pub fn focus(&mut self, pane: Pane) {
        // (asked for by a pane while the dock is drawn: once it is back)
        if self.in_dock {
            self.deferred.push(pane);
            return;
        }
        let dock = self.dock.get_or_insert_with(default_layout);
        if let Some(path) = dock.find_tab(&pane) {
            let _ = dock.set_active_tab(path);
            dock.set_focused_node_and_surface(path.node_path());
            return;
        }
        for w in &mut self.windows {
            if let Some(path) = w.dock.find_tab(&pane) {
                let _ = w.dock.set_active_tab(path);
                return;
            }
        }
        // beside the pane it goes with: the maps and results by the viewer (else the model), the viewer by the model
        let dock = self.dock.get_or_insert_with(default_layout);
        let near: &[Pane] = match pane {
            Pane::Simulation | Pane::Areas | Pane::Validation | Pane::Batch | Pane::Computers | Pane::Map(_) => {
                &[Pane::Viewer, Pane::Model]
            }
            Pane::Viewer | Pane::Document => &[Pane::Model],
            Pane::Files => &[Pane::Operators],
            Pane::Prepare => &[Pane::Node],
            Pane::Node => &[Pane::Prepare, Pane::Parameters],
            Pane::Parameters => &[Pane::Node, Pane::Prepare],
            Pane::Review => &[Pane::Assistant],
            Pane::Trace => &[Pane::Assistant, Pane::Review],
            Pane::Problems => &[Pane::Trace, Pane::Assistant, Pane::Review],
            _ => &[],
        };
        let found = near.iter().find_map(|n| dock.find_tab(n));
        // the calculator under the model, as a console
        if pane == Pane::Calculator
            && let Some(path) = dock.find_tab(&Pane::Model)
            && path.surface.is_main()
        {
            // (split_below: the old node's share)
            dock.main_surface_mut().split_below(path.node, 0.62, vec![pane]);
            if let Some(path) = dock.find_tab(&pane) {
                let _ = dock.set_active_tab(path);
                dock.set_focused_node_and_surface(path.node_path());
            }
            return;
        }
        // nowhere to go with: the operators and the files at the left edge of the window, the assistant and the
        // review of its changes at the right, the node at the left of the model
        let edge = match pane {
            Pane::Operators | Pane::Files => Some((true, 0.15)),
            Pane::Assistant | Pane::Review | Pane::Trace | Pane::Problems => Some((false, 0.74)),
            Pane::Node | Pane::Prepare | Pane::Parameters => Some((true, 0.2)),
            _ => None,
        };
        if let (None, Some((left, fraction))) = (found, edge)
            && dock.main_surface().num_tabs() > 0
        {
            let s = dock.main_surface_mut();
            if left {
                // (split_left: the new node's share; split_right: the old one's)
                s.split_left(NodeIndex::root(), fraction, vec![pane]);
            } else {
                s.split_right(NodeIndex::root(), fraction, vec![pane]);
            }
            if let Some(path) = dock.find_tab(&pane) {
                let _ = dock.set_active_tab(path);
                dock.set_focused_node_and_surface(path.node_path());
            }
            return;
        }
        match near.iter().find_map(|n| dock.find_tab(n)).map(|p| dock.leaf_mut(p.node_path())) {
            Some(Ok(leaf)) => leaf.append_tab(pane),
            _ => dock.push_to_focused_leaf(pane),
        }
        // in front, with the focus
        if let Some(path) = dock.find_tab(&pane) {
            let _ = dock.set_active_tab(path);
            dock.set_focused_node_and_surface(path.node_path());
        }
    }

    /// Whether a pane is open somewhere.
    pub fn is_open(&self, pane: Pane) -> bool {
        self.dock.as_ref().is_some_and(|d| d.find_tab(&pane).is_some())
            || self.windows.iter().any(|w| w.dock.find_tab(&pane).is_some())
    }

    /// The panes in windows of their own.
    pub fn detached(&self) -> Vec<Pane> {
        self.windows.iter().flat_map(|w| w.dock.iter_all_tabs().map(|(_, t)| *t).collect::<Vec<_>>()).collect()
    }

    /// Take a pane to a window of its own.
    pub fn detach(&mut self, pane: Pane) {
        let dock = self.dock.get_or_insert_with(default_layout);
        if let Some(path) = dock.find_tab(&pane) {
            self.homes.insert(pane, path);
            dock.remove_tab(path);
        }
        self.windows.retain_mut(|w| {
            if let Some(path) = w.dock.find_tab(&pane) {
                w.dock.remove_tab(path);
            }
            w.dock.iter_all_tabs().next().is_some()
        });
        self.window_count += 1;
        let id = egui::ViewportId::from_hash_of(("terraflux window", self.window_count));
        self.windows.push(Detached { id, dock: DockState::new(vec![pane]), at: None });
    }

    /// Tabs dragged out of a dock: a window of their own each, where the pointer let them go (on the screen).
    fn float_out(&mut self, ctx: &egui::Context, tabs: Vec<Vec<Pane>>) {
        if tabs.is_empty() {
            return;
        }
        let origin = ctx.input(|i| i.viewport().outer_rect.map(|r| r.min)).unwrap_or_default();
        let pointer = ctx.input(|i| i.pointer.latest_pos()).unwrap_or_default();
        for group in tabs {
            let Some((&first, rest)) = group.split_first() else { continue };
            self.window_count += 1;
            let id = egui::ViewportId::from_hash_of(("terraflux window", self.window_count));
            let mut dock = DockState::new(vec![first]);
            for &t in rest {
                dock.push_to_first_leaf(t);
            }
            let at = origin + (pointer - egui::Pos2::ZERO) - egui::vec2(40.0, 12.0);
            self.windows.push(Detached { id, dock, at: Some(at) });
        }
    }

    /// Bring a pane back to the main window.
    pub fn attach(&mut self, pane: Pane) {
        self.windows.retain_mut(|w| {
            if let Some(path) = w.dock.find_tab(&pane) {
                w.dock.remove_tab(path);
            }
            w.dock.iter_all_tabs().next().is_some()
        });
        self.put_back(pane);
    }

    /// A pane in the main window again: where it was, if that place is still there.
    fn put_back(&mut self, pane: Pane) {
        let dock = self.dock.get_or_insert_with(default_layout);
        if dock.find_tab(&pane).is_some() {
            return;
        }
        let home = self.homes.remove(&pane);
        match home.map(|h| (h.tab.0, dock.leaf_mut(h.node_path()))) {
            Some((at, Ok(leaf))) => {
                // at its place among the leaf's tabs
                let at = at.min(leaf.tabs().len());
                leaf.insert_tab(at, pane);
                if let Some(path) = dock.find_tab(&pane) {
                    let _ = dock.set_active_tab(path);
                }
            }
            _ => dock.push_to_first_leaf(pane),
        }
    }

    /// Read the layout kept, if any (the panes in windows of their own open again).
    pub fn restore_layout(&mut self, path: PathBuf) {
        if let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(mut l) = serde_json::from_str::<Layout>(&text)
            && l.version == LAYOUT_VERSION
        {
            // the maps of nodes were of the last session; the viewer opens when there is data
            let kept = |t: &Pane| !matches!(t, Pane::Map(_) | Pane::Viewer);
            retain_panes(&mut l.main, |t| kept(t));
            self.dock = Some(l.main);
            for panes in l.windows {
                for p in panes.into_iter().filter(kept) {
                    self.detach(p);
                }
            }
            self.viewer_opened = false;
            if self.has_data() {
                self.add_viewer();
            }
        }
        self.layout_file = Some(path);
    }

    /// Keep the layout when it changed (once a second at most).
    fn keep_layout(&mut self) {
        let Some(path) = &self.layout_file else { return };
        if self.layout_saved.elapsed() < Duration::from_secs(1) {
            return;
        }
        let Some(main) = &self.dock else { return };
        let windows: Vec<Vec<Pane>> =
            self.windows.iter().map(|w| w.dock.iter_all_tabs().map(|(_, t)| *t).collect()).collect();
        #[derive(serde::Serialize)]
        struct Out<'a> {
            version: u32,
            main: &'a DockState<Pane>,
            windows: Vec<Vec<Pane>>,
        }
        let Ok(text) = serde_json::to_string(&Out { version: LAYOUT_VERSION, main, windows }) else { return };
        if text != self.layout_text {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, &text);
            self.layout_text = text;
        }
        self.layout_saved = Instant::now();
    }

    /// The dock of the main window, and the windows of their own.
    fn docks(&mut self, ui: &mut Ui) {
        self.place_map_tabs();
        let mut moves = Moves::default();
        let mut dock = self.dock.take().unwrap_or_else(default_layout);
        let style = dock_style(ui);
        self.in_dock = true;
        egui_dock::DockArea::new(&mut dock)
            .id(egui::Id::new("main dock"))
            .style(style.clone())
            .show_leaf_collapse_buttons(false)
            .show_inside(ui, &mut Panes { app: self, moves: &mut moves, detached: false });
        self.in_dock = false;
        // tabs dragged out of the dock (egui_dock's floating windows): windows of their own, to take anywhere
        let floating = floating_tabs(&mut dock);
        self.dock = Some(dock);
        // the panes a pane asked for while the dock was drawn
        for p in std::mem::take(&mut self.deferred) {
            self.focus(p);
        }
        // (their windows shown from the next frame: their panes were drawn in the dock's floating window in this
        // one)
        let mut windows = std::mem::take(&mut self.windows);
        self.float_out(ui.ctx(), floating);
        let mut closed = Vec::new();
        let mut window_keys: Vec<Keys> = Vec::new();
        for (k, w) in windows.iter_mut().enumerate() {
            let title = w.dock.iter_all_tabs().map(|(_, t)| t.title()).collect::<Vec<_>>().join(", ");
            let mut builder = egui::ViewportBuilder::default()
                .with_title(format!("Terraflux - {title}"))
                .with_inner_size([960.0, 720.0])
                .with_icon(crate::logo::window_icon());
            if let Some(at) = w.at.take() {
                builder = builder.with_position(at);
            }
            let app = &mut *self;
            let moves = &mut moves;
            let style = style.clone();
            let close = ui.ctx().show_viewport_immediate(w.id, builder, |ui, _class| {
                // its keys: ctrl+W closes its tab in front, the others are the application's
                let keys = if ui.input(|i| i.focused) { Keys::read(ui) } else { Keys::default() };
                if keys.close {
                    let front = w
                        .dock
                        .find_active_focused()
                        .map(|(_, t)| *t)
                        .or_else(|| w.dock.iter_leaves().find_map(|l| l.1.tabs().get(l.1.active.0).copied()));
                    if let Some(p) = front
                        && let Some(path) = w.dock.find_tab(&p)
                    {
                        if let (Pane::Map(id), Some(s)) = (p, &mut app.session) {
                            s.map_tabs.remove(&id);
                        }
                        w.dock.remove_tab(path);
                    }
                }
                window_keys.push(keys);
                egui::CentralPanel::default().frame(egui::Frame::new().fill(ui.visuals().panel_fill)).show(ui, |ui| {
                    egui_dock::DockArea::new(&mut w.dock)
                        .id(egui::Id::new(("window dock", w.id)))
                        .style(style.clone())
                        .show_leaf_collapse_buttons(false)
                        .show_inside(ui, &mut Panes { app, moves, detached: true });
                });
                app.take_dropped(ui);
                ui.input(|i| i.viewport().close_requested())
            });
            if close || w.dock.iter_all_tabs().next().is_none() {
                closed.push(k);
            }
        }
        // a window closed: its panes back in the main window
        for k in closed.into_iter().rev() {
            let w = windows.remove(k);
            for (_, t) in w.dock.iter_all_tabs() {
                self.put_back(*t);
            }
        }
        let mut out = Vec::new();
        for w in &mut windows {
            out.extend(floating_tabs(&mut w.dock));
        }
        windows.append(&mut self.windows);
        self.windows = windows;
        self.float_out(ui.ctx(), out);
        for k in &window_keys {
            self.app_keys(ui.ctx(), k);
        }
        for p in moves.detach {
            self.detach(p);
        }
        for p in moves.attach {
            self.attach(p);
        }
    }
}

impl eframe::App for TerrafluxApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.take_handoffs(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.frame(ui);
    }
}

/// What the keys and the mouse do in a pane, for the status bar: the canvas's, the maps', else the application's.
fn hints(focused: Option<Pane>) -> String {
    let c = crate::commands::cmd();
    match focused {
        Some(Pane::Model) => format!(
            "click: select    drag: a box of nodes    right or middle drag: move    double click: its map    /: add an \
             operator    {c}enter: run to it    {c}K: commands    F1: shortcuts"
        ),
        Some(Pane::Viewer | Pane::Map(_) | Pane::Simulation) => {
            "wheel: zoom    drag: move    double click: the whole map    B: satellite under the map    space: hide the \
             map    F1: shortcuts"
                .to_string()
        }
        _ => format!(
            "/: add an operator    F5: run all    {c}K: commands    Tab: the model and the last map    F1: shortcuts"
        ),
    }
}
