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
use crate::runner::{Message, Run};
use crate::theme;
use crate::wizard::{Outcome, Wizard};

/// A pane of the application: a tab of the dock, moved, split, closed (the Panels menu brings it back) or taken
/// to a window of its own (another screen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Pane {
    /// The model's graph.
    Model,
    /// The operators' library.
    Operators,
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
    /// A node's output in a tab of its own (opened by a double click on the node; one per node, brought to the
    /// front when it is open already).
    Map(u64),
}

impl Pane {
    pub const ALL: [Pane; 9] = [
        Pane::Model,
        Pane::Operators,
        Pane::Node,
        Pane::Prepare,
        Pane::Viewer,
        Pane::Simulation,
        Pane::Areas,
        Pane::Validation,
        Pane::Batch,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Pane::Model => "Model",
            Pane::Operators => "Operators",
            Pane::Node => "Node",
            Pane::Prepare => "Prepare",
            Pane::Viewer => "Viewer",
            Pane::Simulation => "Simulation",
            Pane::Areas => "Areas",
            Pane::Validation => "Validation",
            Pane::Batch => "Many runs",
            Pane::Map(_) => "Map",
        }
    }
}

/// The panes as they first are: the node and the preparation of its data on the left, the model over the viewer
/// beside them (the operators in a drawer at the left edge and in the toolbar, the others from the View menu; the
/// maps of nodes open beside the viewer).
pub fn default_layout() -> DockState<Pane> {
    let mut d = DockState::new(vec![Pane::Model]);
    let s = d.main_surface_mut();
    let [model, _] = s.split_left(NodeIndex::root(), 0.24, vec![Pane::Node, Pane::Prepare]);
    let [_, _] = s.split_below(model, 0.5, vec![Pane::Viewer]);
    d
}

/// The panes kept that `keep` says to keep, and the floating windows left with none taken away (egui_dock's
/// own `retain_tabs` leaves such a window, a tree of one empty node, and panics drawing it).
fn retain_panes(dock: &mut DockState<Pane>, keep: impl FnMut(&mut Pane) -> bool) {
    dock.retain_tabs(keep);
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
const LAYOUT_VERSION: u32 = 5;

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
    open: bool,
    tab: bool,
    close: bool,
}

impl Keys {
    /// The keys of the window `ui` is in, taken.
    fn read(ui: &mut Ui) -> Keys {
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        ui.input_mut(|i| Keys {
            run: i.consume_key(egui::Modifiers::NONE, egui::Key::F5),
            save: i.consume_key(egui::Modifiers::COMMAND, egui::Key::S),
            open: i.consume_key(egui::Modifiers::COMMAND, egui::Key::O),
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
    st
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
    pub findings: Vec<Finding>,
    pub inputs: Option<BTreeMap<String, Value>>,
    /// The model's inputs `inputs` are of (a run reads them again when they changed).
    inputs_of: BTreeMap<String, Input>,
    pub load_error: Option<String>,
    loading: Option<Receiver<Loaded>>,
    pub run: Option<Run>,
    pub progress: Option<(String, usize, usize)>,
    pub node: String,
    /// The map the simulation starts from, then the map of every step; the cells of each class of each.
    pub maps: Vec<Map>,
    pub counts: Vec<Vec<(f64, usize)>>,
    pub tables: Vec<Table>,
    pub shown: usize,
    pub follow: bool,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    pub elapsed: Option<Duration>,
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
    /// How each top-level node did in the last run.
    pub status: BTreeMap<String, NodeStatus>,
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
    pub looks: Looks,
    /// The outputs shown in map tabs of their own (a double click on a node), by tab; the tabs the dock is still
    /// to show.
    pub map_tabs: BTreeMap<u64, crate::builder::Shown>,
    next_tab: u64,
    pub opened: Vec<u64>,
    /// The map tabs to bring to the front (asked for again while open).
    pub raised: Vec<u64>,
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
}

impl Session {
    /// Read a model and check it; its data is read in a thread (see [`Session::loading`]).
    pub fn open(path: &Path) -> Result<Session, String> {
        let source = Model::load(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Session::new(source, Some(path.to_path_buf()))
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

    fn new(source: Model, path: Option<PathBuf>) -> Result<Session, String> {
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
        Ok(Session {
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
            findings: Vec::new(),
            inputs: None,
            inputs_of: BTreeMap::new(),
            load_error: None,
            loading: Some(rx),
            run: None,
            progress: None,
            node: String::new(),
            maps: Vec::new(),
            counts: Vec::new(),
            tables: Vec::new(),
            shown: 0,
            follow: true,
            warnings: Vec::new(),
            error: None,
            elapsed: None,
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
            status: BTreeMap::new(),
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
            palettes: Default::default(),
            looks: Looks::default(),
            auto: true,
            edited_at: None,
            map_tabs: BTreeMap::new(),
            next_tab: 0,
            opened: Vec::new(),
            raised: Vec::new(),
            run_when_ready: false,
            saving: Vec::new(),
            notice: None,
        })
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

    /// Whether a run may stop at a node: only the errors of the nodes it needs count, and the engine checks
    /// them.
    pub fn can_run_to(&self) -> bool {
        !self.running() && !self.loading() && self.load_error.is_none()
    }

    /// Run the whole model.
    pub fn start(&mut self, repaint: Box<dyn Fn() + Send>) {
        self.start_to(None, repaint);
    }

    /// Run what a node needs (none: the whole model); what did not change since the last run comes from the
    /// cache.
    pub fn start_to(&mut self, target: Option<String>, repaint: Box<dyn Fn() + Send>) {
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
        self.maps.truncate(1);
        self.counts.truncate(1);
        self.tables.clear();
        self.warnings.clear();
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
        self.run = Some(Run::start(self.model.clone(), inputs, base, self.cache.clone(), target, repaint));
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
                    self.node = at;
                }
                Message::Finished { at, cached } => {
                    // a plot that ran: its map tab, opened once
                    let plot = self.model.nodes.iter().any(|n| n.id == at && n.op == "view.plot");
                    if plot && !self.map_tabs.values().any(|t| t.node == at) && !plots.contains(&at) {
                        plots.push(at.clone());
                    }
                    self.status.insert(at, NodeStatus::Done { cached });
                    self.status_rev += 1;
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
                Message::Done(r) => done = Some(r),
            }
        }
        for p in plots {
            self.open_map(&p, "shown");
        }
        if let Some(r) = done {
            self.run = None;
            self.elapsed = self.started.map(|t| t.elapsed());
            self.status.retain(|_, st| *st != NodeStatus::Running);
            self.status_rev += 1;
            match r {
                Ok(results) => self.outputs.extend(results.values),
                Err(e) => self.error = Some(e),
            }
            self.outputs.retain(|id, _| self.model.nodes.iter().any(|n| n.id == *id));
        }
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
    /// The pane Tab went to the model from (Tab again goes back to it).
    tab_back: Option<Pane>,
    /// Tab was taken this frame: the keyboard's focus it may have given a field is let go at the frame's end.
    tabbed: bool,
    /// The operators' drawer is out (the pointer went to the window's left edge).
    pub drawer: bool,
    /// The model run many times (replicates, scenarios, calibration).
    pub batch: crate::batch_view::BatchState,
}

/// A row of a list: a name, selected or not, and a note on the right.
fn row(ui: &mut Ui, selected: bool, name: &str, note: &str) -> egui::Response {
    let note = RichText::new(note).size(11.0).color(if selected {
        theme::on_accent()
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
            tab_back: None,
            tabbed: false,
            drawer: false,
            batch: Default::default(),
        };
        if let Some(p) = model {
            app.open(&p);
        }
        app
    }

    pub fn open(&mut self, path: &Path) {
        match Session::open(path) {
            Ok(s) => {
                self.session = Some(s);
                self.message = None;
                self.focus(Pane::Model);
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
        if let Some(p) = path
            && let Err(e) = s.save_as(&p)
        {
            s.editor.message = Some(e);
        }
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        let mut save = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new("TERRAFLUX").strong().size(13.0).color(theme::accent()));
            ui.separator();
            if ui.button("New").on_hover_text("a new model in the builder").clicked() {
                self.wizard = None;
                self.session = Some(Session::blank());
                self.focus(Pane::Model);
            }
            if ui.button("Open...").clicked()
                && let Some(p) = rfd::FileDialog::new().add_filter("Terraflux model", &["toml", "json"]).pick_file()
            {
                self.wizard = None;
                self.open(&p);
            }
            ui.menu_button("Templates", |ui| {
                ui.label(RichText::new("Models to start from, run at once").weak().size(11.0));
                for t in &crate::templates::TEMPLATES {
                    if ui.button(t.name).on_hover_text(t.about).clicked() {
                        self.start_template(ui.ctx(), t);
                        ui.close();
                    }
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
                        ui.add(
                            egui::TextEdit::singleline(&mut self.template_name).hint_text("name").desired_width(140.0),
                        );
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
            });
            if let Some(s) = &mut self.session {
                ui.menu_button("Operators", |ui| {
                    crate::builder::operators_menu(ui, s, &self.registry, &self.library);
                })
                .response
                .on_hover_text("add an operator after the node selected (also at the window's left edge, and \"/\")");
            }
            if ui.button("Settings").on_hover_text("the theme, the text's size, the threads...").clicked() {
                self.settings_open = !self.settings_open;
            }
            if ui
                .button("Many runs...")
                .on_hover_text("the model run many times: replicates, scenarios, calibration")
                .clicked()
            {
                self.focus(Pane::Batch);
            }
            if ui.button("Wizard...").on_hover_text("a land change model from your maps, step by step").clicked() {
                self.wizard = Some(Wizard::default());
            }
            ui.menu_button("View", |ui| {
                // a pane on or off
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
            });
            let ctx = ui.ctx().clone();
            if let Some(s) = &mut self.session
                && self.wizard.is_none()
            {
                if ui.add_enabled(s.dirty || s.path.is_none(), egui::Button::new("Save")).clicked() {
                    save = Some(false);
                }
                if ui.button("Save as...").clicked() {
                    save = Some(true);
                }
                ui.separator();
                if ui
                    .add_enabled(s.can_run(), accent_button("Run", s.can_run()))
                    .on_hover_text("run the whole model (F5)")
                    .clicked()
                {
                    s.start(Box::new(move || ctx.request_repaint()));
                }
                if ui.add_enabled(s.running(), egui::Button::new("Stop")).clicked()
                    && let Some(r) = &s.run
                {
                    r.stop();
                }
                ui.separator();
                ui.label(RichText::new(s.title()).strong());
                if s.loading() {
                    ui.spinner();
                    ui.label(RichText::new("reading the data...").weak());
                }
                if let Some((at, step, of)) = &s.progress {
                    let frac = *step as f32 / (*of).max(1) as f32;
                    ui.add(
                        egui::ProgressBar::new(frac)
                            .desired_width(180.0)
                            .desired_height(14.0)
                            .fill(theme::accent())
                            .text(RichText::new(format!("{at}: step {step} of {of}")).color(theme::on_accent())),
                    );
                }
                if s.running() {
                    ui.spinner();
                    ui.label(RichText::new(&s.node).weak());
                } else if let Some(e) = s.elapsed {
                    ui.label(RichText::new(format!("done in {:.2} s", e.as_secs_f64())).weak());
                }
            }
        });
        if let Some(choose) = save {
            self.save(choose);
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
            });
            ui.add_space(6.0);
            if ui.button("Back to the defaults").clicked() {
                let favourites = std::mem::take(&mut st.favourites);
                st = crate::settings::Settings { favourites, ..Default::default() };
            }
        });
        crate::settings::update(|s| *s = st);
        self.settings_open = open;
    }

    /// The operators' drawer: out over the panes while the pointer is at the window's left edge or over it, in
    /// again when the pointer leaves it.
    fn drawer(&mut self, ctx: &egui::Context, area: egui::Rect) {
        const WIDTH: f32 = 280.0;
        let Some(s) = &mut self.session else { return };
        let pointer = ctx.input(|i| i.pointer.latest_pos());
        let dragging = ctx.dragged_id().is_some();
        if let Some(p) = pointer {
            if !self.drawer && !dragging && p.x <= area.left() + 3.0 && area.y_range().contains(p.y) {
                self.drawer = true;
            } else if self.drawer
                && (p.x > area.left() + WIDTH + 24.0 || !area.y_range().contains(p.y))
                && !ctx.any_popup_open()
            {
                self.drawer = false;
            }
        }
        if !self.drawer {
            return;
        }
        let before = s.revision;
        egui::Area::new(egui::Id::new("operators drawer")).fixed_pos(area.min).order(egui::Order::Foreground).show(
            ctx,
            |ui| {
                egui::Frame::new()
                    .fill(ui.visuals().panel_fill)
                    .shadow(ui.visuals().popup_shadow)
                    .stroke(ui.visuals().window_stroke)
                    .show(ui, |ui| {
                        ui.set_width(WIDTH);
                        ui.set_height(area.height());
                        crate::builder::operators_pane(ui, s, &self.registry, &self.library);
                    });
            },
        );
        // an operator added: the drawer goes in
        if s.revision != before {
            self.drawer = false;
        }
    }

    /// The bar at the bottom: the cell under the pointer, else how to move around.
    fn status(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let weak = ui.visuals().weak_text_color();
            match &self.hover {
                Some(h) => {
                    let value = if h.value.is_nan() {
                        "no value".to_string()
                    } else {
                        format!("value {}", terraflux_ops::value::class_label(h.value))
                    };
                    ui.label(RichText::new(value).strong());
                    ui.label(
                        RichText::new(format!("row {}  column {}   x {:.2}  y {:.2}", h.row, h.col, h.x, h.y))
                            .monospace()
                            .color(weak),
                    );
                }
                None if self.session.is_some() => {
                    let delete = if cfg!(target_os = "macos") { "⌫" } else { "del" };
                    ui.label(
                        RichText::new(format!(
                            "click an operator (or /) to add it    drag: select    right or middle drag: move    double \
                             click: its map    {delete}: delete    ctrl+R: rename    ctrl+L: arrange    Tab: the model \
                             and back    ctrl+W: close the tab    B: satellite under the map    F5: run all"
                        ))
                        .color(weak),
                    );
                }
                None => {}
            }
            if let Some(s) = &self.session {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // a word of what was done (a file saved), for a while
                    if let Some((text, at)) = &s.notice
                        && at.elapsed() < Duration::from_secs(8)
                    {
                        ui.label(RichText::new(text).color(theme::accent()));
                        ui.separator();
                    }
                    if s.dirty {
                        ui.label(RichText::new("not saved").color(ui.visuals().warn_fg_color));
                    }
                    if !s.warnings.is_empty() {
                        ui.label(
                            RichText::new(format!("{} warnings", s.warnings.len())).color(ui.visuals().warn_fg_color),
                        );
                    }
                });
            }
        });
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
        Plot::new("areas").legend(Legend::default()).x_axis_label("step").y_axis_label("cells").show(ui, |p| {
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
    /// model and back to the tab it came from, ctrl+W closes the tab in focus.
    fn keys(&mut self, ui: &mut Ui) {
        if !ui.input(|i| i.focused) {
            return;
        }
        let keys = Keys::read(ui);
        if keys.tab {
            self.tabbed = true;
            match self.focused() {
                Some(Pane::Model) => {
                    if let Some(back) = self.tab_back.take()
                        && self.is_open(back)
                    {
                        self.focus(back);
                    }
                }
                other => {
                    self.tab_back = other;
                    self.focus(Pane::Model);
                }
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

    /// What the keys of any window ask of the application: run, save, open data.
    fn app_keys(&mut self, ctx: &egui::Context, keys: &Keys) {
        if keys.save {
            self.save(false);
        }
        if let Some(s) = &mut self.session {
            if keys.open {
                crate::builder::open_data(s, &self.registry);
            }
            if keys.run && s.can_run() {
                let ctx = ctx.clone();
                s.start(Box::new(move || ctx.request_repaint()));
            }
        }
    }

    /// One frame of the application.
    pub fn frame(&mut self, ui: &mut Ui) {
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
            s.auto_run(ui.ctx());
            if s.running() || s.loading() || s.saving() {
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
        }
        // a batch goes on whether its pane is shown or not
        self.batch.poll();
        if self.batch.running() {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
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
        egui::Panel::top("toolbar").frame(bar).show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").frame(bar.inner_margin(Margin::symmetric(8, 3))).show(ui, |ui| self.status(ui));
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
        let area = ui.available_rect_before_wrap();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::bar())).show(ui, |ui| self.docks(ui));
        self.drawer(ui.ctx(), area);
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
            Pane::Operators => crate::builder::operators_pane(ui, s, r, lib),
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
            Pane::Batch => crate::batch_view::pane(ui, &mut self.batch, s, r),
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
        let (opened, raised, known): (Vec<u64>, Vec<u64>, Vec<u64>) = match &mut self.session {
            Some(s) => {
                (std::mem::take(&mut s.opened), std::mem::take(&mut s.raised), s.map_tabs.keys().copied().collect())
            }
            None => (Vec::new(), Vec::new(), Vec::new()),
        };
        let stale = |t: &mut Pane| !matches!(t, Pane::Map(id) if !known.contains(id));
        let dock = self.dock.get_or_insert_with(default_layout);
        retain_panes(dock, stale);
        for w in &mut self.windows {
            retain_panes(&mut w.dock, stale);
        }
        self.windows.retain(|w| w.dock.iter_all_tabs().next().is_some());
        for id in opened {
            self.beside_viewer(Pane::Map(id));
        }
        // (brought to the front where it is, or put back if its window went away)
        for id in raised {
            self.focus(Pane::Map(id));
        }
    }

    /// A tab put beside the viewer's wherever it is (in the main window or one of its own), else beside the maps
    /// open, else where the focus is; brought to the front.
    fn beside_viewer(&mut self, pane: Pane) {
        let is_map = |t: &Pane| matches!(t, Pane::Map(_));
        let dock = self.dock.get_or_insert_with(default_layout);
        for d in std::iter::once(dock).chain(self.windows.iter_mut().map(|w| &mut w.dock)) {
            let at = d.find_tab(&Pane::Viewer).or_else(|| d.iter_all_tabs().find(|(_, t)| is_map(t)).map(|(p, _)| p));
            if let Some(path) = at
                && let Ok(leaf) = d.leaf_mut(path.node_path())
            {
                leaf.append_tab(pane);
                if let Some(p) = d.find_tab(&pane) {
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
        // beside the pane it goes with: the maps and results by the viewer, the text by the model
        let dock = self.dock.get_or_insert_with(default_layout);
        let near = match pane {
            Pane::Simulation | Pane::Areas | Pane::Validation | Pane::Batch | Pane::Map(_) => Some(Pane::Viewer),
            Pane::Viewer => Some(Pane::Model),
            _ => None,
        };
        match near.and_then(|n| dock.find_tab(&n)).map(|p| dock.leaf_mut(p.node_path())) {
            Some(Ok(leaf)) => leaf.append_tab(pane),
            _ => dock.push_to_focused_leaf(pane),
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
            // the maps of nodes were of the last session
            retain_panes(&mut l.main, |t| !matches!(t, Pane::Map(_)));
            self.dock = Some(l.main);
            for panes in l.windows {
                for p in panes.into_iter().filter(|p| !matches!(p, Pane::Map(_))) {
                    self.detach(p);
                }
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
        egui_dock::DockArea::new(&mut dock)
            .id(egui::Id::new("main dock"))
            .style(style.clone())
            .show_leaf_collapse_buttons(false)
            .show_inside(ui, &mut Panes { app: self, moves: &mut moves, detached: false });
        // tabs dragged out of the dock (egui_dock's floating windows): windows of their own, to take anywhere
        let floating = floating_tabs(&mut dock);
        self.dock = Some(dock);
        self.float_out(ui.ctx(), floating);
        let mut windows = std::mem::take(&mut self.windows);
        let mut closed = Vec::new();
        let mut window_keys: Vec<Keys> = Vec::new();
        for (k, w) in windows.iter_mut().enumerate() {
            let title = w.dock.iter_all_tabs().map(|(_, t)| t.title()).collect::<Vec<_>>().join(", ");
            let mut builder = egui::ViewportBuilder::default()
                .with_title(format!("Terraflux - {title}"))
                .with_inner_size([960.0, 720.0]);
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
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.frame(ui);
    }
}

/// The side panel of the node selected: what it does and why, its id, its inputs and settings, a container's
/// outputs; every field editable.
pub(crate) fn node_panel(ui: &mut Ui, s: &mut Session, r: &Registry) {
    let path = s.editor.path.clone();
    let Some(id) = s.selected.clone() else {
        ui.label("Click a node to edit it; drag from an output (right) to an input (left) to connect them; double-click a container to edit its body.");
        return;
    };
    let Some(n) = s.source.scope(&path).and_then(|v| v.iter().find(|n| n.id == id)).cloned() else {
        s.selected = None;
        return;
    };
    let mut change: Option<Result<(), String>> = None;
    let field = |s: &mut Session, key: &str, init: String| -> String {
        s.fields.entry(key.to_string()).or_insert(init).clone()
    };
    ui.horizontal(|ui| {
        let mut name = field(s, "id", n.id.clone());
        ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.0));
        s.fields.insert("id".into(), name.clone());
        if ui.button("Rename").clicked() {
            change = Some(crate::editor::rename_node(&mut s.source, &path, &n.id, &name).map(|_| {
                s.selected = Some(name.clone());
            }));
        }
    });
    ui.label(RichText::new(&n.op).monospace());
    let sig = r.sig(&n.op).cloned();
    if let Some(sig) = &sig {
        ui.label(&sig.doc);
    }
    if let Some(note) = &n.note {
        ui.separator();
        ui.label(RichText::new("Why").strong());
        ui.label(note);
    }
    ui.separator();
    ui.label(RichText::new("Inputs").strong());
    egui::Grid::new("inputs").num_columns(3).show(ui, |ui| {
        for (port, ty, required) in crate::editor::input_ports(&n, r) {
            ui.label(if required { format!("{port} *") } else { port.clone() }).on_hover_text(format!("{ty:?}"));
            match n.inputs.get(&port) {
                Some(Source::Ref(reference)) => {
                    ui.label(RichText::new(format!("<- {reference}")).monospace());
                    if ui.small_button("x").on_hover_text("disconnect").clicked() {
                        change = Some(crate::editor::disconnect(&mut s.source, &path, &n.id, &port));
                    }
                }
                other => {
                    let init = match other {
                        Some(Source::Literal(v)) => v.to_string(),
                        _ => String::new(),
                    };
                    let key = format!("in.{port}");
                    let mut text = field(s, &key, init.clone());
                    let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(120.0).hint_text("value"));
                    s.fields.insert(key, text.clone());
                    if resp.lost_focus() && text != init {
                        change = Some(crate::editor::set_literal(&mut s.source, r, &path, &n.id, &port, &text));
                    }
                }
            }
            ui.end_row();
        }
    });
    if sig.as_ref().is_some_and(|g| g.extra.is_some() || g.container) {
        ui.horizontal(|ui| {
            let mut name = field(s, "new_input", String::new());
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(100.0).hint_text("name"));
            let mut value = field(s, "new_value", String::new());
            ui.add(egui::TextEdit::singleline(&mut value).desired_width(100.0).hint_text("$input, node.port or value"));
            s.fields.insert("new_input".into(), name.clone());
            s.fields.insert("new_value".into(), value.clone());
            if ui.button("Add input").clicked() && !name.is_empty() {
                let v = value.trim();
                change = Some(if v.starts_with('$') || v.starts_with('@') || terraflux_graph::Ref::parse(v).is_ok() {
                    crate::editor::connect(&mut s.source, r, &path, v, &n.id, &name)
                } else {
                    crate::editor::set_literal(&mut s.source, r, &path, &n.id, &name, v)
                });
            }
        });
    }
    let mut params: Vec<(String, String, Option<terraflux_graph::PortSpec>)> = sig
        .as_ref()
        .map(|g| g.params.iter().map(|p| (p.name.clone(), p.doc.clone(), Some(p.clone()))).collect())
        .unwrap_or_default();
    for k in n.params.keys() {
        if !params.iter().any(|(p, _, _)| p == k) {
            params.push((k.clone(), String::new(), None));
        }
    }
    if !params.is_empty() {
        ui.separator();
        ui.label(RichText::new("Settings").strong());
        for (name, doc, spec) in params {
            // the metrics of the landscape metrics: all, by type, or one by one
            if ["metrics", "landscape_metrics", "class_metrics"].contains(&name.as_str())
                && n.op.starts_with("metrics.")
            {
                if let Some(c) = crate::builder::metrics_param(ui, s, &n, &path, &name) {
                    change = Some(c);
                }
                continue;
            }
            // a reclassification's rules: a table of them, from the values of the map it reads
            if n.op == "raster.reclassify" && name == "rules" {
                if let Some(c) = crate::builder::reclassify_rules(ui, s, &n, &path) {
                    change = Some(c);
                }
                continue;
            }
            let init = n
                .params
                .get(&name)
                .map(|v| match v {
                    serde_json::Value::String(t) => t.clone(),
                    v => v.to_string(),
                })
                .unwrap_or_default();
            // a few values to choose from (a yes or no among them)
            if let Some(spec) = spec.as_ref().filter(|p| p.ty == terraflux_graph::Type::Bool || !p.choices.is_empty()) {
                if let Some(c) = crate::builder::choice_param(ui, s, &n, &path, spec, &init) {
                    change = Some(c);
                }
                continue;
            }
            ui.label(&name).on_hover_text(&doc);
            let key = format!("param.{name}");
            let mut text = field(s, &key, init.clone());
            let resp = ui
                .add(egui::TextEdit::multiline(&mut text).desired_rows(1).desired_width(f32::INFINITY).hint_text(&doc));
            s.fields.insert(key, text.clone());
            if resp.lost_focus() && text != init {
                change = Some(crate::editor::set_param(&mut s.source, &path, &n.id, &name, &text));
            }
            // a file: chosen in a dialog
            if name == "path" && terraflux_graph::FILE_OPS.contains(&n.op.as_str()) && ui.button("Choose...").clicked()
            {
                let dialog = match n.op.as_str() {
                    terraflux_graph::PLUGIN => rfd::FileDialog::new().add_filter("WebAssembly", &["wasm", "wat"]),
                    "vector.read" | "vector.write" => rfd::FileDialog::new()
                        .add_filter("Vectors", &crate::builder::VECTOR_EXTENSIONS)
                        .add_filter("Every file", &["*"]),
                    _ => rfd::FileDialog::new()
                        .add_filter("Rasters", &terraflux_ops::files::RASTER_EXTENSIONS)
                        .add_filter("Every file", &["*"]),
                };
                let writes = ["raster.write", "landscape.mspa_tiled", "vector.write"].contains(&n.op.as_str());
                let picked = if writes { dialog.save_file() } else { dialog.pick_file() };
                if let Some(p) = picked {
                    let p = p.to_string_lossy().into_owned();
                    change = Some(crate::editor::set_param(&mut s.source, &path, &n.id, &name, &p));
                }
            }
            // a folder: every raster in it, as one stack
            if name == "path"
                && n.op == "raster.read"
                && ui.button("A folder...").on_hover_text("every raster of a folder, as one stack").clicked()
                && let Some(p) = rfd::FileDialog::new().pick_folder()
            {
                let p = p.to_string_lossy().into_owned();
                change = Some(crate::editor::set_param(&mut s.source, &path, &n.id, &name, &p));
            }
        }
    }
    if let Some(b) = &n.body {
        ui.separator();
        ui.label(RichText::new("Body").strong());
        let maps: Vec<(&str, &BTreeMap<String, String>)> = if n.op == terraflux_graph::GROUP {
            vec![("outputs", &b.outputs)]
        } else {
            vec![("carry", &b.carry), ("collect", &b.collect)]
        };
        for (which, map) in &maps {
            for (k, v) in map.iter() {
                ui.label(RichText::new(format!("{which}: {k} <- {v}")).monospace());
            }
        }
        ui.horizontal(|ui| {
            let mut which = field(s, "body_which", maps[0].0.to_string());
            egui::ComboBox::from_id_salt("which").selected_text(&which).show_ui(ui, |ui| {
                for (w, _) in &maps {
                    ui.selectable_value(&mut which, w.to_string(), *w);
                }
            });
            s.fields.insert("body_which".into(), which.clone());
            let mut name = field(s, "body_name", String::new());
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(80.0).hint_text("name"));
            let mut reference = field(s, "body_ref", String::new());
            ui.add(egui::TextEdit::singleline(&mut reference).desired_width(110.0).hint_text("node.port inside"));
            s.fields.insert("body_name".into(), name.clone());
            s.fields.insert("body_ref".into(), reference.clone());
            if ui.button("Set").clicked() {
                let mut inner = path.clone();
                inner.push(n.id.clone());
                change = Some(crate::editor::set_body_map(&mut s.source, r, &inner, &which, &name, reference.trim()));
            }
        });
        if ui.button("Edit the body").clicked() {
            s.editor.path.push(n.id.clone());
            s.selected = None;
            s.fields.clear();
        }
    }
    ui.separator();
    if ui.button("Delete node").clicked() {
        change = Some(crate::editor::remove_node(&mut s.source, &path, &n.id).map(|_| s.selected = None));
    }
    let issues: Vec<&Issue> = s
        .issues
        .iter()
        .filter(|i| {
            i.at.split('.').next()
                == Some(&if path.is_empty() { id.clone() } else { format!("{}/{id}", path.join("/")) })
        })
        .collect();
    for i in issues {
        ui.colored_label(
            if i.severity == Severity::Error { ui.visuals().error_fg_color } else { ui.visuals().warn_fg_color },
            &i.message,
        );
    }
    match change {
        Some(Ok(())) => {
            s.fields.clear();
            s.editor.message = None;
            s.edited();
        }
        Some(Err(e)) => s.editor.message = Some(e),
        None => {}
    }
}
