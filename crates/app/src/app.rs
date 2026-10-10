//! The application: a model open (its graph, its data checked), its run with the maps drawn as they come, the
//! area of each class along the steps, and the validation against an observed map.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Ui};
use egui_plot::{Legend, Line, Plot};
use terraflux_exec::lint::{Finding, Level, lint};
use terraflux_graph::{Issue, Model, Severity, Source, check};
use terraflux_ops::validate::{Agreement, agreement, similarity_of_differences};
use terraflux_ops::{Map, Registry, Table, Value};
use terraflux_raster::Raster;

use crate::editor_view::{Action, EditorState};
use crate::map_view::{Hover, MapView, class_counts, legend};
use crate::palette::Coloring;
use crate::runner::{Message, Run};
use crate::wizard::{Outcome, Wizard};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Map,
    Charts,
    Graph,
    Model,
    Validation,
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

/// A model open.
pub struct Session {
    /// The model as written (what the editor changes and saves).
    pub source: Model,
    /// The model run: its submodels expanded.
    pub model: Model,
    /// The editor's state; whether the model changed since it was saved.
    pub editor: EditorState,
    pub dirty: bool,
    /// The texts being typed in the side panel, by field.
    pub fields: BTreeMap<String, String>,
    pub path: PathBuf,
    pub issues: Vec<Issue>,
    pub findings: Vec<Finding>,
    pub inputs: Option<BTreeMap<String, Value>>,
    pub load_error: Option<String>,
    pub run: Option<Run>,
    pub progress: Option<(String, usize, usize)>,
    pub node: String,
    /// The map the simulation starts from, then the map of every step.
    pub maps: Vec<Map>,
    pub tables: Vec<Table>,
    pub shown: usize,
    pub follow: bool,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    pub elapsed: Option<Duration>,
    pub started: Option<Instant>,
    pub view: MapView,
    pub coloring: Option<Coloring>,
    pub selected: Option<String>,
    pub validation: Validation,
}

impl Session {
    pub fn open(path: &Path) -> Result<Session, String> {
        let source = Model::load(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut model = source.clone();
        terraflux_graph::submodel::expand(&mut model, &base).map_err(|e| format!("{}: {e}", path.display()))?;
        let issues = check(&model, &Registry::standard());
        let (inputs, load_error) = match terraflux_exec::load_inputs(&model, &base, &BTreeMap::new()) {
            Ok(i) => (Some(i), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let findings = inputs.as_ref().map(|i| lint(&model, i)).unwrap_or_default();
        let mut s = Session {
            source,
            model,
            editor: EditorState::default(),
            dirty: false,
            fields: BTreeMap::new(),
            path: path.to_path_buf(),
            issues,
            findings,
            inputs,
            load_error,
            run: None,
            progress: None,
            node: String::new(),
            maps: Vec::new(),
            tables: Vec::new(),
            shown: 0,
            follow: true,
            warnings: Vec::new(),
            error: None,
            elapsed: None,
            started: None,
            view: MapView::default(),
            coloring: None,
            selected: None,
            validation: Validation { windows: "1, 3, 5, 7, 9, 11".into(), ..Default::default() },
        };
        if let Some(m) = s.start_map() {
            s.coloring = Some(Coloring::for_values(&m.data));
            s.maps = vec![m];
        }
        Ok(s)
    }

    /// The map the simulation starts from: the first value of the loop's carried `landscape`.
    pub fn start_map(&self) -> Option<Map> {
        let inputs = self.inputs.as_ref()?;
        for n in &self.model.nodes {
            if n.body.is_some()
                && let Some(terraflux_graph::Source::Ref(r)) = n.inputs.get("landscape")
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

    /// After an edit of the source: the model run and its checks follow.
    pub fn edited(&mut self) {
        self.dirty = true;
        self.fields.clear();
        let base = self.path.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut m = self.source.clone();
        match terraflux_graph::submodel::expand(&mut m, &base) {
            Ok(()) => {
                self.issues = check(&m, &Registry::standard());
                self.model = m;
            }
            Err(e) => self.editor.message = Some(e.to_string()),
        }
    }

    /// Write the model where it was read.
    pub fn save(&mut self) -> Result<(), String> {
        self.source.save(&self.path).map_err(|e| e.to_string())?;
        self.dirty = false;
        Ok(())
    }

    pub fn running(&self) -> bool {
        self.run.is_some()
    }

    pub fn can_run(&self) -> bool {
        !self.running()
            && self.load_error.is_none()
            && !self.issues.iter().any(|i| i.severity == Severity::Error)
            && !self.findings.iter().any(|f| f.level == Level::Error)
    }

    pub fn start(&mut self, repaint: Box<dyn Fn() + Send>) {
        let base = self.path.parent().map(Path::to_path_buf).unwrap_or_default();
        self.maps.truncate(1);
        self.tables.clear();
        self.warnings.clear();
        self.error = None;
        self.elapsed = None;
        self.shown = 0;
        self.validation.agreement = None;
        self.validation.similarity = None;
        self.started = Some(Instant::now());
        self.run = Some(Run::start(self.model.clone(), base, repaint));
    }

    /// Take what the run sent.
    pub fn poll(&mut self) {
        let Some(run) = &self.run else { return };
        let mut done = None;
        while let Ok(m) = run.rx.try_recv() {
            match m {
                Message::Node(at) => self.node = at,
                Message::Step { at, step, of, values } => {
                    if let Some(Value::Map(m)) = values.get("maps") {
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
        if let Some(r) = done {
            self.run = None;
            self.elapsed = self.started.map(|t| t.elapsed());
            if let Err(e) = r {
                self.error = Some(e);
            }
        }
    }

    /// The cells of each class at each step (rows: steps).
    pub fn areas(&self) -> Vec<Vec<(f64, usize)>> {
        self.maps.iter().map(|m| class_counts(&m.data)).collect()
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
    pub tab: Tab,
    pub message: Option<String>,
    hover: Option<Hover>,
}

impl TerrafluxApp {
    pub fn new(model: Option<PathBuf>) -> Self {
        let mut app = TerrafluxApp { session: None, wizard: None, tab: Tab::Map, message: None, hover: None };
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
                self.tab = Tab::Map;
            }
            Err(e) => self.message = Some(e),
        }
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.heading("Terraflux");
            ui.separator();
            if ui.button("New model...").clicked() {
                self.wizard = Some(Wizard::default());
            }
            if ui.button("Open...").clicked()
                && let Some(p) = rfd::FileDialog::new().add_filter("Terraflux model", &["toml", "json"]).pick_file()
            {
                self.open(&p);
            }
            let ctx = ui.ctx().clone();
            if let Some(s) = &mut self.session {
                ui.separator();
                if ui.add_enabled(s.can_run(), egui::Button::new("Run")).clicked() {
                    s.start(Box::new(move || ctx.request_repaint()));
                }
                if ui.add_enabled(s.running(), egui::Button::new("Stop")).clicked()
                    && let Some(r) = &s.run
                {
                    r.stop();
                }
                ui.separator();
                ui.label(RichText::new(s.model.name.clone().unwrap_or_else(|| s.path.display().to_string())).strong());
                if let Some((at, step, of)) = &s.progress {
                    let frac = *step as f32 / (*of).max(1) as f32;
                    ui.add(
                        egui::ProgressBar::new(frac).desired_width(160.0).text(format!("{at}: step {step} of {of}")),
                    );
                }
                if s.running() {
                    ui.spinner();
                    ui.label(&s.node);
                } else if let Some(e) = s.elapsed {
                    ui.label(format!("done in {:.1} s", e.as_secs_f64()));
                }
            }
        });
    }

    fn tabs(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (t, name) in [
                (Tab::Map, "Map"),
                (Tab::Charts, "Charts"),
                (Tab::Graph, "Graph"),
                (Tab::Model, "Model"),
                (Tab::Validation, "Validation"),
            ] {
                ui.selectable_value(&mut self.tab, t, name);
            }
        });
        ui.separator();
    }

    fn map_tab(&mut self, ui: &mut Ui) {
        let Some(s) = &mut self.session else { return };
        if s.maps.is_empty() {
            ui.label("No map to show: the model's inputs could not be read.");
            return;
        }
        egui::Panel::left("map_side").resizable(true).default_size(240.0).show(ui, |ui| {
            let last = s.maps.len() - 1;
            ui.label(RichText::new("Step").strong());
            if ui.add(egui::Slider::new(&mut s.shown, 0..=last)).changed() {
                s.follow = s.shown == last;
            }
            ui.checkbox(&mut s.follow, "follow the run");
            ui.label(if s.shown == 0 {
                "the map the simulation starts from".to_string()
            } else {
                format!("after step {}", s.shown)
            });
            ui.separator();
            if let Some(c) = &s.coloring {
                let counts = class_counts(&s.maps[s.shown].data);
                legend(ui, c, &counts, &|v| terraflux_ops::value::class_label(v));
            }
            ui.separator();
            match &self.hover {
                Some(h) => {
                    ui.label(format!("row {}, column {}", h.row, h.col));
                    ui.label(format!("x {:.2}, y {:.2}", h.x, h.y));
                    ui.label(if h.value.is_nan() { "no value".to_string() } else { format!("value {}", h.value) });
                }
                None => {
                    ui.label("Wheel: zoom; drag: move; double click: fit.");
                }
            }
            if !s.warnings.is_empty() {
                ui.separator();
                ui.label(RichText::new(format!("{} warnings", s.warnings.len())).color(ui.visuals().warn_fg_color));
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
        egui::CentralPanel::default().show(ui, |ui| {
            let shown = s.shown.min(s.maps.len() - 1);
            let coloring = s.coloring.clone().unwrap_or(Coloring::Continuous { min: 0.0, max: 1.0 });
            self.hover = s.view.show(ui, &s.maps[shown], &coloring, 0);
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

    fn graph_tab(&mut self, ui: &mut Ui) {
        let Some(s) = &mut self.session else { return };
        let r = Registry::standard();
        egui::Panel::top("editor_bar").show(ui, |ui| {
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
                let mut names: Vec<String> = r.signatures().into_iter().map(|g| g.name).collect();
                names.sort();
                egui::ComboBox::from_id_salt("add_op")
                    .selected_text(if s.editor.add.is_empty() { "operator..." } else { s.editor.add.as_str() })
                    .show_ui(ui, |ui| {
                        for n in names {
                            ui.selectable_value(&mut s.editor.add, n.clone(), n);
                        }
                    });
                if ui.add_enabled(!s.editor.add.is_empty(), egui::Button::new("Add node")).clicked() {
                    let op = s.editor.add.clone();
                    // below the nodes there
                    let nodes = s.source.scope(&s.editor.path).cloned().unwrap_or_default();
                    let auto = crate::editor_view::auto_positions(&nodes);
                    let y = nodes.iter().map(|n| n.pos.unwrap_or(auto[&n.id])[1] + 160.0).fold(0.0f32, f32::max);
                    match crate::editor::add_node(&mut s.source, &r, &s.editor.path, &op, [0.0, y]) {
                        Ok(id) => {
                            s.selected = Some(id);
                            s.edited();
                        }
                        Err(e) => s.editor.message = Some(e),
                    }
                }
                ui.separator();
                if ui.add_enabled(s.dirty, egui::Button::new("Save")).clicked()
                    && let Err(e) = s.save()
                {
                    s.editor.message = Some(e);
                }
                if let Some(m) = &s.editor.message {
                    ui.colored_label(ui.visuals().warn_fg_color, m);
                }
            });
        });
        egui::Panel::right("node_side").resizable(true).default_size(320.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| node_panel(ui, s, &r));
        });
        egui::CentralPanel::default().show(ui, |ui| {
            let actions = crate::editor_view::show(ui, &s.source, &r, &mut s.editor, s.selected.as_deref());
            for a in actions {
                let path = s.editor.path.clone();
                let result = match a {
                    Action::Select(id) => {
                        if s.selected.as_deref() != Some(id.as_str()) {
                            s.fields.clear();
                        }
                        s.selected = Some(id);
                        Ok(false)
                    }
                    Action::Enter(id) => {
                        s.editor.path.push(id);
                        s.selected = None;
                        s.fields.clear();
                        Ok(false)
                    }
                    Action::Move(id, p) => crate::editor::move_node(&mut s.source, &path, &id, p).map(|_| {
                        s.dirty = true;
                        false
                    }),
                    Action::Connect { reference, node, port } => {
                        crate::editor::connect(&mut s.source, &r, &path, &reference, &node, &port).map(|_| true)
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
        });
    }

    fn model_tab(&mut self, ui: &mut Ui) {
        let Some(s) = &self.session else { return };
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(RichText::new(s.path.display().to_string()).monospace());
            if let Some(e) = &s.load_error {
                ui.colored_label(ui.visuals().error_fg_color, format!("the inputs could not be read: {e}"));
            }
            let problems = s.issues.len() + s.findings.len();
            ui.label(
                RichText::new(if problems == 0 {
                    "No problem found in the model or its data.".to_string()
                } else {
                    format!("{problems} problems found")
                })
                .strong(),
            );
            for i in &s.issues {
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
            ui.separator();
            let text = s.model.to_toml().unwrap_or_else(|e| e.to_string());
            ui.label(RichText::new(text).monospace());
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

    /// One frame of the application.
    pub fn frame(&mut self, ui: &mut Ui) {
        if let Some(s) = &mut self.session {
            s.poll();
            if s.running() {
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
        }
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        if let Some(w) = &mut self.wizard {
            let mut close = false;
            let mut created = None;
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| match w.show(ui) {
                    Some(Outcome::Cancel) => close = true,
                    Some(Outcome::Created(p)) => created = Some(p),
                    Some(Outcome::Open) | None => {}
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
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(m) = &self.message {
                ui.colored_label(ui.visuals().error_fg_color, m);
            }
            if self.session.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.heading("Land use and land cover change models");
                    ui.label("Create a model from your maps with the wizard, or open a model file.");
                    ui.add_space(12.0);
                    if ui.button("New model...").clicked() {
                        self.wizard = Some(Wizard::default());
                    }
                });
                return;
            }
            self.tabs(ui);
            match self.tab {
                Tab::Map => self.map_tab(ui),
                Tab::Charts => self.charts_tab(ui),
                Tab::Graph => self.graph_tab(ui),
                Tab::Model => self.model_tab(ui),
                Tab::Validation => self.validation_tab(ui),
            }
        });
    }
}

impl eframe::App for TerrafluxApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.frame(ui);
    }
}

/// The side panel of the node selected: what it does and why, its id, its inputs and settings, a container's
/// outputs; every field editable.
fn node_panel(ui: &mut Ui, s: &mut Session, r: &Registry) {
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
    let sig = r.get(&n.op).map(|o| o.signature());
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
    let mut params: Vec<(String, String)> =
        sig.as_ref().map(|g| g.params.iter().map(|p| (p.name.clone(), p.doc.clone())).collect()).unwrap_or_default();
    for k in n.params.keys() {
        if !params.iter().any(|(p, _)| p == k) {
            params.push((k.clone(), String::new()));
        }
    }
    if !params.is_empty() {
        ui.separator();
        ui.label(RichText::new("Settings").strong());
        for (name, doc) in params {
            let init = n
                .params
                .get(&name)
                .map(|v| match v {
                    serde_json::Value::String(t) => t.clone(),
                    v => v.to_string(),
                })
                .unwrap_or_default();
            ui.label(&name).on_hover_text(doc);
            let key = format!("param.{name}");
            let mut text = field(s, &key, init.clone());
            let resp = ui.add(egui::TextEdit::multiline(&mut text).desired_rows(1).desired_width(f32::INFINITY));
            s.fields.insert(key, text.clone());
            if resp.lost_focus() && text != init {
                change = Some(crate::editor::set_param(&mut s.source, &path, &n.id, &name, &text));
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
