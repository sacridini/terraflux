//! The application: a model open (its graph, its data checked), its run with the maps drawn as they come, the
//! area of each class along the steps, and the validation against an observed map.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Ui};
use egui_plot::{Legend, Line, Plot};
use terraflux_exec::lint::{Finding, Level, lint};
use terraflux_graph::{Issue, Model, Severity, check};
use terraflux_ops::validate::{Agreement, agreement, similarity_of_differences};
use terraflux_ops::{Map, Registry, Table, Value};
use terraflux_raster::Raster;

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
    pub model: Model,
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
        let model = Model::load(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let issues = check(&model, &Registry::standard());
        let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let (inputs, load_error) = match terraflux_exec::load_inputs(&model, &base, &BTreeMap::new()) {
            Ok(i) => (Some(i), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let findings = inputs.as_ref().map(|i| lint(&model, i)).unwrap_or_default();
        let mut s = Session {
            model,
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
        egui::Panel::right("node_side").resizable(true).default_size(280.0).show(ui, |ui| {
            match s.selected.as_ref().and_then(|k| s.model.node(k)) {
                Some(n) => {
                    ui.heading(&n.id);
                    ui.label(RichText::new(&n.op).monospace());
                    if let Some(sig) = Registry::standard().get(&n.op).map(|o| o.signature()) {
                        ui.label(sig.doc);
                    }
                    if let Some(note) = &n.note {
                        ui.separator();
                        ui.label(RichText::new("Why").strong());
                        ui.label(note);
                    }
                    ui.separator();
                    ui.label(RichText::new("Inputs and settings").strong());
                    ui.label(RichText::new(crate::graph_view::settings(n)).monospace());
                }
                None => {
                    ui.label("Click a node to see what it does, why it is there and its settings.");
                }
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(k) = crate::graph_view::show(ui, &s.model) {
                s.selected = Some(k);
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
