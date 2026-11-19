//! "Use my data", the window: the user's files in place of the data a model draws ([`crate::my_data`] makes the
//! change). Opened from the land change entry points (the toolbar's button, the start page, the palette) it has a first
//! page, *Which model* (Dinamica EGO's classic model, CLUE-S, ordered allocation or PLUS: the template of it is opened
//! and the window goes on to its data). Then *Your data* (a file for each thing the model draws, its band, the grid of reference, each
//! choice checked as soon as it is made, in a thread; on a land change model the second date can be another band of
//! the first date's file), *Classes* (the user's classes of a map against the model's, one table each), *Parameters*
//! (the model's exposed parameters and, for a land change model, the settings of the model: steps, increment,
//! elasticity, order of the classes, seed; the dates of the bands give their first values) and *Review* (what will be
//! done, what was refused, the findings; errors stop it). *Apply* is one step of undo, seen in the Changes pane, and
//! the model runs once its data is read (unless unticked).
//!
//! A strip on the Model pane's canvas offers the window when the model draws data ([`banner`]); what a model draws
//! is worked out once for each revision of it ([`sources_of`]), never on every frame.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{Align, Layout, Margin, Rect, RichText, Ui, UiBuilder, Vec2, pos2};
use serde_json::Value as Json;
use terraflux_exec::lint::{Finding, Level};
use terraflux_graph::{DataKind, Model, Parameter, Type};
use terraflux_ops::Registry;

use crate::app::Session;
use crate::my_data::{self, Choice, Plan, Source, UserClass};
use crate::rgb::{ROLES, Scale};

/// What a model draws, kept for the revision it was worked out for, and whether the banner was hidden.
#[derive(Default)]
pub struct Cache {
    rev: Option<u64>,
    sources: Vec<Source>,
    drawn: bool,
    /// The banner of the Model pane hidden for this model, until it is closed.
    pub hidden: bool,
}

/// Whether what a model offers to replace is drawn data (a neutral landscape model, a read with no file, an input
/// with no path), not only data a node marked says it stands for or an input that already holds the user's file.
pub fn drawn(sources: &[Source]) -> bool {
    sources
        .iter()
        .any(|s| (s.op == my_data::INPUT && s.file.is_none()) || s.op == "raster.read" || s.op.starts_with("nlm."))
}

/// The data the model open draws that a file can replace, worked out again only after an edit.
pub fn sources_of<'a>(s: &'a mut Session, r: &Registry) -> &'a [Source] {
    if s.my_data.rev != Some(s.revision) {
        let list = my_data::sources(&s.source, r);
        s.my_data.drawn = drawn(&list);
        s.my_data.sources = list;
        s.my_data.rev = Some(s.revision);
    }
    &s.my_data.sources
}

/// The strip over the top of the Model pane's canvas when the model draws data: whether *Use my data...* was
/// clicked. Nothing is done for a model that is empty, hidden or inside a container, and the check is a revision's.
pub fn banner(ui: &mut Ui, s: &mut Session, r: &Registry, canvas: Rect) -> bool {
    if s.my_data.hidden || s.source.nodes.is_empty() || !s.editor.path.is_empty() {
        return false;
    }
    sources_of(s, r);
    let width = (canvas.width() - 24.0).min(520.0);
    if !s.my_data.drawn || width < 260.0 {
        return false;
    }
    let rect = Rect::from_min_size(pos2(canvas.center().x - width / 2.0, canvas.top() + 6.0), Vec2::new(width, 30.0));
    let mut strip = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)));
    let mut asked = false;
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .corner_radius(6)
        .inner_margin(Margin::symmetric(10, 4))
        .show(&mut strip, |ui| {
            ui.set_width(width - 20.0);
            ui.horizontal(|ui| {
                ui.label("This model runs on drawn data.");
                if ui
                    .small_button("Use my data...")
                    .on_hover_text(
                        "your files in place of the data it draws: the grids put right, your classes into its",
                    )
                    .clicked()
                {
                    asked = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("Hide").on_hover_text("until this model is closed").clicked() {
                        s.my_data.hidden = true;
                    }
                });
            });
        });
    asked
}

/// The land change models the first page offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Dinamica,
    Clue,
    /// CLUE-S's template with its method chosen as ordered.
    Ordered,
    Plus,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Dinamica, Kind::Clue, Kind::Ordered, Kind::Plus];

    pub fn title(self) -> &'static str {
        match self {
            Kind::Dinamica => "Dinamica EGO's classic model",
            Kind::Clue => "CLUE-S",
            Kind::Ordered => "Ordered allocation",
            Kind::Plus => "PLUS",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Kind::Dinamica => {
                "Fitted on two maps: how much changes comes from the transition rates between them (a Markov chain), \
                 where from weights of evidence of the drivers, and the shape of the new patches from the patches \
                 observed (Expander and Patcher). A good start when you have two dates of land use."
            }
            Kind::Clue => {
                "lulcc's CLUE-S: the suitability of each class from logistic regressions on the drivers, the cells \
                 of each class at each step interpolated from observed maps, and an allocation where classes compete \
                 for cells until the demand is met. Two dates, or a file with a band for each of three or more."
            }
            Kind::Ordered => {
                "lulcc's ordered allocation: the same suitability and demand as CLUE-S, the classes served one after \
                 the other in a given order, each taking the cells it suits best."
            }
            Kind::Plus => {
                "PLUS (Liang et al. 2021): where each class grew between two dates is learned by a random forest on \
                 the drivers (the land expansion analysis), and CARS allocates the demand that the Markov chain of the \
                 period projects."
            }
        }
    }

    /// The template it starts from (its name in [`crate::templates::TEMPLATES`]).
    pub fn template(self) -> &'static str {
        match self {
            Kind::Dinamica => "Dinamica EGO",
            Kind::Clue | Kind::Ordered => "CLUE-S",
            Kind::Plus => "PLUS (LEAS and CARS)",
        }
    }
}

/// What the window asks of the application.
pub enum Act {
    /// A template (by its index in [`crate::templates::TEMPLATES`]) started as a model.
    Template(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// Which land change model (only when opened from a land change entry point).
    Model,
    Data,
    /// The roles of an image's bands and its reflectance's scale (only when a file is chosen for an image).
    Bands,
    Classes,
    Parameters,
    Review,
}

impl Page {
    const ALL: [Page; 6] = [Page::Model, Page::Data, Page::Bands, Page::Classes, Page::Parameters, Page::Review];

    fn title(self) -> &'static str {
        match self {
            Page::Model => "Which model",
            Page::Data => "Your data",
            Page::Bands => "Bands",
            Page::Classes => "Classes",
            Page::Parameters => "Parameters",
            Page::Review => "Review",
        }
    }
}

/// The classes of a file's band: the user's code, its name from the file's legend, and its cells.
type Found = Vec<(i64, Option<String>, usize)>;

enum Cls {
    /// Not a map whose classes are translated.
    None,
    /// Being read, for this file and band.
    Loading(String),
    Ready(Found),
    Failed(String),
}

/// What the user sets for an image: the role of each band and the scale of its reflectance.
struct ImagePick {
    /// The role of each band (`blue`, `green`, `red`, `nir`, `swir1`, `swir2` or `other`, dropped), guessed from the
    /// bands' names until the user sets them.
    roles: Vec<String>,
    /// The scale the user chose (none: the one detected).
    scale: Option<Scale>,
    /// The scale the file's values and names tell, once read in a thread.
    detected: Option<Scale>,
}

/// A file chosen for a source.
struct Pick {
    file: PathBuf,
    bands: Vec<String>,
    band: Option<String>,
    /// The classes the model reads, for a table (none: it keeps the user's classes as they are).
    template: Option<BTreeMap<i64, String>>,
    classes: Cls,
    crosswalk: BTreeMap<i64, Option<i64>>,
    /// The source whose file this one was offered as ("the same file, another band"): its table of classes is the
    /// first one of this.
    like: Option<String>,
    /// An image: the roles of its bands and its scale.
    image: Option<ImagePick>,
    /// An image of single-band files: the files in band order (`file` is the first, `bands` their names).
    files: Vec<PathBuf>,
    /// An image with no file of the user's: a Sentinel-2 scene of an area.
    scene: Option<my_data::Scene>,
}

/// What a thread did.
enum Done {
    Classes { node: String, key: String, result: Result<Found, String> },
    Scale { node: String, file: PathBuf, result: Result<Scale, String> },
    Checked { key: String, result: Box<Result<(Plan, Vec<Finding>), String>> },
}

struct Checked {
    key: String,
    result: Result<(Plan, Vec<Finding>), String>,
}

/// The window's state.
pub struct State {
    pub page: Page,
    /// Run the model once its data is read, after *Apply*.
    pub run_now: bool,
    /// The source whose grid the others are put on (none: the first map chosen).
    pub reference: Option<String>,
    /// Opened from a land change entry point: the page *Which model* comes first.
    pub land: bool,
    /// The method of CLUE-S's allocation to set once its template is the model open (ordered allocation).
    start_ordered: Option<bool>,
    /// Every band of the file both dates share is a date (CLUE-S's model with three dates or more).
    all_dates: bool,
    /// The sources of the two dates of the model open, and whether it is CLUE-S's.
    dates: Vec<String>,
    lulcc: bool,
    picks: BTreeMap<String, Pick>,
    /// What the user set on the *Parameters* page, by setting id (none: automatic); and what the dates of the files
    /// give the settings that the user did not set (see [`my_data::derive`]).
    settings: BTreeMap<String, Option<Json>>,
    derived: BTreeMap<String, Option<Json>>,
    /// Why a file could not be chosen, by source.
    problems: BTreeMap<String, String>,
    /// The model the choices are of: its tab and revision.
    seen: Option<(usize, u64)>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
    ctx: Option<egui::Context>,
    working: bool,
    done: Option<Checked>,
    /// Whether `done` is of the choices as they are now.
    current: bool,
    error: Option<String>,
    /// The classes of the template's rules (`objects.rules`, by node) and the user's for them (none for a node: the
    /// template's own), for an image.
    rules: Vec<(String, Vec<UserClass>)>,
    classes: BTreeMap<String, Vec<UserClass>>,
    /// A file chosen at a click, taken on the next frame (which has the session).
    asked: Option<(String, PathBuf)>,
    /// The band files (a folder, or a selection) of an image chosen at a click, taken on the next frame.
    asked_files: Option<(String, Vec<PathBuf>)>,
    /// The area form of an image's Sentinel-2 scene: its source and the scene being set.
    scene_form: Option<(String, my_data::Scene)>,
    /// A scene chosen at a click, taken on the next frame.
    asked_scene: Option<(String, my_data::Scene)>,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

/// The classes a table maps to, for a source: those the model reads, a mask's 0 and 1; none for any classes.
fn template_of(s: &Source) -> Option<BTreeMap<i64, String>> {
    let d = s.data.as_ref()?;
    match d.kind {
        DataKind::Classes if !d.classes.is_empty() => Some(d.classes.clone()),
        DataKind::Mask if d.classes.is_empty() => Some(BTreeMap::from([(0, "no".to_string()), (1, "yes".to_string())])),
        DataKind::Mask => Some(d.classes.clone()),
        _ => None,
    }
}

/// Whether what a source stands for is classes or a mask (its page of classes has a line for it).
fn is_classes(s: &Source) -> bool {
    s.data.as_ref().is_some_and(|d| matches!(d.kind, DataKind::Classes | DataKind::Mask))
}

/// A colour written `#rrggbb`.
fn hex_rgb(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().strip_prefix('#')?;
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let b = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([b(0)?, b(2)?, b(4)?])
}

/// What the scale of an image's reflectance does, in a line.
fn reflectance_line(s: Scale) -> String {
    match s {
        Scale::None => "Reflectance 0\u{2013}1: kept as it is".to_string(),
        Scale::Div10000 => "Reflectance 0\u{2013}10 000 \u{2192} divided by 10 000".to_string(),
        Scale::LandsatC2 => "Landsat collection 2 \u{2192} x 0.0000275 - 0.2, reflectance 0\u{2013}1".to_string(),
        Scale::Sentinel2Offset => {
            "Sentinel-2 from 2022 \u{2192} divided by 10 000, - 0.1 (the offset of processing baseline 04.00)"
                .to_string()
        }
    }
}

/// The cells that go to no data under a table, and all the cells: a class mapped to nothing, or not in the table.
pub fn no_data_cells(found: &Found, crosswalk: &BTreeMap<i64, Option<i64>>) -> (usize, usize) {
    let total = found.iter().map(|c| c.2).sum();
    let none = found.iter().filter(|c| crosswalk.get(&c.0).copied().flatten().is_none()).map(|c| c.2).sum();
    (none, total)
}

/// What a plan did, in counts: the data replaced, the nodes added and the nodes taken away.
pub fn summary(before: &Model, plan: &Plan) -> (usize, usize, usize) {
    let had: BTreeSet<&str> = before.nodes.iter().map(|n| n.id.as_str()).collect();
    let has: BTreeSet<&str> = plan.model.nodes.iter().map(|n| n.id.as_str()).collect();
    (plan.inputs.len(), has.difference(&had).count(), had.difference(&has).count())
}

fn bands_of(file: &Path) -> Result<Vec<String>, String> {
    let f = terraflux_ops::files::File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(f.meta().band_names())
}

/// The node a finding is about (`$name/band` is the input `name`).
fn finding_node(f: &Finding) -> &str {
    f.at.trim_start_matches('$').split('/').next().unwrap_or("")
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// What the settings made are, a line each, for the review: the label and the value.
fn setting_notes(model: &Model, made: &BTreeMap<String, Option<Json>>) -> Vec<String> {
    my_data::settings(model)
        .into_iter()
        .filter(|s| made.contains_key(&s.id))
        .map(|s| {
            let value = s.now.map_or_else(|| "automatic".to_string(), |v| crate::document::words(&v));
            format!("{}: {value}", s.param.label.unwrap_or(s.id))
        })
        .collect()
}

/// The sources of the two dates of a land change model: its maps of classes, in the model's order (none for any
/// other model).
fn date_sources(sources: &[Source], model: &Model) -> Vec<String> {
    if my_data::land(model).is_none() {
        return Vec::new();
    }
    sources.iter().filter(|x| is_classes(x) && x.needs == Type::Map).take(2).map(|x| x.node.clone()).collect()
}

/// The band a pick reads (by name; the first when none is chosen).
fn band_name(p: &Pick) -> Option<String> {
    p.band.clone().or_else(|| p.bands.first().cloned())
}

/// The classes the model reads in a pick, once its table is made: those the table maps to, else the file's own.
fn classes_after(p: &Pick) -> Vec<i64> {
    if p.crosswalk.is_empty() {
        match &p.classes {
            Cls::Ready(f) => f.iter().map(|c| c.0).collect(),
            _ => Vec::new(),
        }
    } else {
        p.crosswalk.values().flatten().copied().collect::<BTreeSet<i64>>().into_iter().collect()
    }
}

/// The classes of the file's band of a pick, as the file has them.
fn found_classes(p: &Pick) -> Vec<i64> {
    match &p.classes {
        Cls::Ready(f) => f.iter().map(|c| c.0).collect(),
        _ => Vec::new(),
    }
}

impl State {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        State {
            page: Page::Data,
            run_now: true,
            reference: None,
            land: false,
            start_ordered: None,
            all_dates: false,
            dates: Vec::new(),
            lulcc: false,
            picks: BTreeMap::new(),
            settings: BTreeMap::new(),
            derived: BTreeMap::new(),
            problems: BTreeMap::new(),
            seen: None,
            tx,
            rx,
            ctx: None,
            working: false,
            done: None,
            current: false,
            error: None,
            rules: Vec::new(),
            classes: BTreeMap::new(),
            asked: None,
            asked_files: None,
            scene_form: None,
            asked_scene: None,
        }
    }

    /// The window as the land change entry points open it: its first page asks which model.
    pub fn land_change() -> Self {
        State { land: true, page: Page::Model, ..State::new() }
    }

    /// A land change model chosen on the first page: the index of its template (to be opened), the window going on to
    /// the data. Ordered allocation is CLUE-S's template with its method set once it is open.
    pub fn choose_model(&mut self, kind: Kind) -> Option<usize> {
        let k = crate::templates::TEMPLATES.iter().position(|t| t.name == kind.template())?;
        self.start_ordered = Some(kind == Kind::Ordered);
        self.picks.clear();
        self.settings.clear();
        self.problems.clear();
        self.reference = None;
        self.all_dates = false;
        self.page = Page::Data;
        Some(k)
    }

    /// Choose a file for a source of the model open (what the file picker does): its bands read, its classes read in
    /// a thread when the model reads classes of it. The reason when it cannot be.
    pub fn choose(&mut self, s: &mut Session, r: &Registry, node: &str, file: &Path) -> Result<(), String> {
        let src = sources_of(s, r).iter().find(|x| x.node == node).cloned();
        let Some(src) = src else { return Err(format!("{node}: the model draws no such data")) };
        match bands_of(file) {
            Ok(bands) => {
                self.problems.remove(node);
                let template = template_of(&src);
                let image = my_data::is_image(&src).then(|| ImagePick {
                    roles: crate::rgb::band_roles(&bands).into_iter().map(String::from).collect(),
                    scale: None,
                    detected: None,
                });
                if image.is_some() {
                    let (tx, ctx, names) = (self.tx.clone(), self.ctx.clone(), bands.clone());
                    let (node, file) = (node.to_string(), file.to_path_buf());
                    std::thread::spawn(move || {
                        let result = my_data::image_scale(&file, &names);
                        let _ = tx.send(Done::Scale { node, file, result });
                        if let Some(c) = ctx {
                            c.request_repaint();
                        }
                    });
                }
                let pick = Pick {
                    file: file.to_path_buf(),
                    bands,
                    band: None,
                    template,
                    classes: Cls::None,
                    crosswalk: BTreeMap::new(),
                    like: None,
                    image,
                    files: Vec::new(),
                    scene: None,
                };
                self.picks.insert(node.to_string(), pick);
                if self.dates.iter().any(|d| d == node) {
                    self.all_dates = false;
                }
                self.load_classes(node);
                Ok(())
            }
            Err(e) => {
                self.problems.insert(node.to_string(), e.clone());
                Err(e)
            }
        }
    }

    /// Choose the single-band files of an image (a folder, or the files selected): sorted by band, the bands named by
    /// the files' names, their roles guessed from them. The reason when it cannot be.
    pub fn choose_files(&mut self, s: &mut Session, r: &Registry, node: &str, paths: &[PathBuf]) -> Result<(), String> {
        let src = sources_of(s, r).iter().find(|x| x.node == node).cloned();
        let Some(src) = src else { return Err(format!("{node}: the model draws no such data")) };
        let made = (|| {
            if !my_data::is_image(&src) {
                return Err("bands of several files are for an image".to_string());
            }
            let files = my_data::band_files(paths)?;
            for f in &files {
                if bands_of(f)?.len() != 1 {
                    return Err(format!(
                        "{} has more than one band: a file for each band takes single-band files",
                        f.display()
                    ));
                }
            }
            Ok(files)
        })();
        let files = match made {
            Ok(f) => f,
            Err(e) => {
                self.problems.insert(node.to_string(), e.clone());
                return Err(e);
            }
        };
        self.problems.remove(node);
        let bands: Vec<String> = files.iter().map(|f| my_data::stem(f)).collect();
        let image = ImagePick {
            roles: crate::rgb::band_roles(&bands).into_iter().map(String::from).collect(),
            scale: None,
            detected: None,
        };
        let (tx, ctx, list, first) = (self.tx.clone(), self.ctx.clone(), files.clone(), files[0].clone());
        let name = node.to_string();
        std::thread::spawn(move || {
            let result = my_data::files_scale(&list);
            let _ = tx.send(Done::Scale { node: name, file: list[0].clone(), result });
            if let Some(c) = ctx {
                c.request_repaint();
            }
        });
        self.picks.insert(
            node.to_string(),
            Pick {
                file: first,
                bands,
                band: None,
                template: None,
                classes: Cls::None,
                crosswalk: BTreeMap::new(),
                like: None,
                image: Some(image),
                files,
                scene: None,
            },
        );
        self.scene_form = None;
        Ok(())
    }

    /// Choose a cloud-free Sentinel-2 scene of an area for an image that has no file of the user's: nothing is
    /// downloaded until the model runs. The reason when the area or the dates cannot be asked for.
    pub fn choose_scene(
        &mut self,
        s: &mut Session,
        r: &Registry,
        node: &str,
        scene: my_data::Scene,
    ) -> Result<(), String> {
        let src = sources_of(s, r).iter().find(|x| x.node == node).cloned();
        let Some(src) = src else { return Err(format!("{node}: the model draws no such data")) };
        let problem =
            if !my_data::is_image(&src) { Some("a scene is for an image".to_string()) } else { scene.problem() };
        if let Some(e) = problem {
            self.problems.insert(node.to_string(), e.clone());
            return Err(e);
        }
        self.problems.remove(node);
        let bands: Vec<String> = my_data::SCENE_ASSETS.iter().map(|a| a.to_string()).collect();
        let image = ImagePick {
            roles: crate::rgb::band_roles(&bands).into_iter().map(String::from).collect(),
            scale: None,
            detected: Some(Scale::Div10000),
        };
        self.picks.insert(
            node.to_string(),
            Pick {
                file: PathBuf::new(),
                bands,
                band: None,
                template: None,
                classes: Cls::None,
                crosswalk: BTreeMap::new(),
                like: None,
                image: Some(image),
                files: Vec::new(),
                scene: Some(scene),
            },
        );
        self.scene_form = None;
        Ok(())
    }

    /// Keep the drawn data of a source: no file for it.
    pub fn keep_drawn(&mut self, node: &str) {
        if self.dates.iter().any(|d| d == node) {
            self.all_dates = false;
        }
        self.picks.remove(node);
        self.problems.remove(node);
    }

    /// The file of a source chosen for another as well, in another band (two dates of one file of bands): its table
    /// of classes starts as the other's.
    pub fn same_file(&mut self, sources: &[Source], node: &str, from: &str, band: &str) -> Result<(), String> {
        let src =
            sources.iter().find(|x| x.node == node).ok_or_else(|| format!("{node}: the model draws no such data"))?;
        let other = self.picks.get(from).ok_or_else(|| format!("{from}: no file chosen"))?;
        if !other.bands.iter().any(|b| b == band) {
            return Err(format!("{} has no band {band:?}", other.file.display()));
        }
        let pick = Pick {
            file: other.file.clone(),
            bands: other.bands.clone(),
            band: Some(band.to_string()),
            template: template_of(src),
            classes: Cls::None,
            crosswalk: BTreeMap::new(),
            like: Some(from.to_string()),
            image: None,
            files: Vec::new(),
            scene: None,
        };
        self.problems.remove(node);
        self.picks.insert(node.to_string(), pick);
        self.all_dates = false;
        self.load_classes(node);
        Ok(())
    }

    /// The file both dates of CLUE-S's model have chosen, with its bands, when it has three or more: every band can be
    /// a date.
    pub fn dates_file(&self) -> Option<(PathBuf, Vec<String>)> {
        if !self.lulcc {
            return None;
        }
        let (a, b) = (self.picks.get(self.dates.first()?)?, self.picks.get(self.dates.get(1)?)?);
        (a.file == b.file && a.bands.len() >= 3).then(|| (a.file.clone(), a.bands.clone()))
    }

    /// Whether every band of the file is a date now.
    pub fn all_dates_now(&self) -> bool {
        self.all_dates && self.dates_file().is_some()
    }

    /// Every band of a file as the dates: the file chosen for the other date taken for this one too, the first date in
    /// its first band and the last in its last, and every band a date.
    pub fn all_dates_from(&mut self, sources: &[Source], node: &str, from: &str) -> Result<(), String> {
        let at = self.dates.iter().position(|d| d == node).ok_or_else(|| format!("{node}: not a date"))?;
        let bands = self.picks.get(from).map(|p| p.bands.clone()).ok_or_else(|| format!("{from}: no file chosen"))?;
        if !self.lulcc || bands.len() < 3 {
            return Err("every band as a date needs CLUE-S's model and a file of three bands or more".into());
        }
        let (first, last) = (bands[0].clone(), bands[bands.len() - 1].clone());
        let (mine, theirs) = if at == 0 { (first, last) } else { (last, first) };
        self.same_file(sources, node, from, &mine)?;
        if self.picks.get(from).and_then(band_name).as_deref() != Some(theirs.as_str()) {
            self.set_band(from, Some(theirs));
        }
        self.all_dates = true;
        Ok(())
    }

    /// The offer of every band as the dates for a date with no file: the source of the other date with a file of three
    /// bands or more (CLUE-S's model).
    fn all_dates_offer(&self, node: &str) -> Option<(String, usize)> {
        if !self.lulcc || !self.dates.iter().any(|d| d == node) {
            return None;
        }
        let from = self.dates.iter().find(|d| *d != node)?;
        let p = self.picks.get(from).filter(|p| p.bands.len() >= 3)?;
        Some((from.clone(), p.bands.len()))
    }

    /// The offer of the same file for a date: the source whose file has more bands, and the band of the other date
    /// (the next for the later date, the previous for the earlier).
    fn same_file_offer(&self, dates: &[String], node: &str) -> Option<(String, String)> {
        let at = dates.iter().position(|d| d == node)?;
        dates.iter().enumerate().filter(|(_, d)| *d != node).find_map(|(k, d)| {
            let p = self.picks.get(d).filter(|p| p.bands.len() > 1)?;
            let i = band_name(p).and_then(|b| p.bands.iter().position(|x| *x == b)).unwrap_or(0);
            let j = if at > k { i + 1 } else { i.checked_sub(1)? };
            Some((d.clone(), p.bands.get(j)?.clone()))
        })
    }

    /// Set a setting of the *Parameters* page (none: automatic): the model is planned again.
    pub fn set_setting(&mut self, id: &str, value: Option<Json>) {
        self.settings.insert(id.to_string(), value);
    }

    /// What a setting is now: what the user set, else what the dates of the files gave, else what the model has.
    pub fn setting_now(&self, base: &my_data::Setting) -> Option<Json> {
        match self.settings.get(&base.id).or_else(|| self.derived.get(&base.id)) {
            Some(v) => v.clone(),
            None => base.now.clone(),
        }
    }

    /// The settings of the model open and their values, as the *Parameters* page shows them.
    pub fn settings_of(&self, model: &Model) -> Vec<(my_data::Setting, Option<Json>)> {
        my_data::settings(model)
            .into_iter()
            .map(|s| {
                let v = self.setting_now(&s);
                (s, v)
            })
            .collect()
    }

    /// The settings the plan is made with: what the dates gave, over it what the user set.
    fn effective(&self) -> BTreeMap<String, Option<Json>> {
        let mut all = self.derived.clone();
        all.extend(self.settings.clone());
        all
    }

    /// The band of a source's file (a name; none: the first), its classes read again.
    pub fn set_band(&mut self, node: &str, band: Option<String>) {
        if let Some(p) = self.picks.get_mut(node) {
            p.band = band;
            self.load_classes(node);
        }
    }

    /// Map one of the user's classes of a source to the model's (none: no data).
    pub fn set_class(&mut self, node: &str, user: i64, to: Option<i64>) {
        if let Some(p) = self.picks.get_mut(node) {
            p.crosswalk.insert(user, to);
        }
    }

    /// The user's classes of a source's file, once read.
    pub fn classes(&self, node: &str) -> Option<&[(i64, Option<String>, usize)]> {
        match &self.picks.get(node)?.classes {
            Cls::Ready(f) => Some(f),
            _ => None,
        }
    }

    /// The table of classes of a source now.
    pub fn crosswalk(&self, node: &str) -> Option<&BTreeMap<i64, Option<i64>>> {
        self.picks.get(node).map(|p| &p.crosswalk)
    }

    /// Whether a file is chosen for a source.
    pub fn picked(&self, node: &str) -> bool {
        self.picks.contains_key(node)
    }

    /// The plan of the choices as they are now, once checked.
    pub fn plan(&self) -> Option<&Plan> {
        match &self.done {
            Some(Checked { result: Ok((p, _)), .. }) if self.current => Some(p),
            _ => None,
        }
    }

    /// The findings of the choices as they are now (none while they are being checked).
    pub fn findings(&self) -> &[Finding] {
        match &self.done {
            Some(Checked { result: Ok((_, f)), .. }) if self.current => f,
            _ => &[],
        }
    }

    /// Whether what was checked is of the choices as they are now (and no thread is at work).
    pub fn current(&self) -> bool {
        self.current && !self.working
    }

    fn load_classes(&mut self, node: &str) {
        let Some(p) = self.picks.get_mut(node) else { return };
        p.crosswalk.clear();
        if p.template.is_none() {
            p.classes = Cls::None;
            return;
        }
        let key = format!("{}|{:?}", p.file.display(), p.band);
        p.classes = Cls::Loading(key.clone());
        let (file, band, node) = (p.file.clone(), p.band.clone(), node.to_string());
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let result = my_data::classes_of(&file, band.as_deref());
            let _ = tx.send(Done::Classes { node, key, result });
            if let Some(c) = ctx {
                c.request_repaint();
            }
        });
    }

    /// What the threads did: the classes read (the table first suggested), the check done.
    fn poll(&mut self) {
        while let Ok(d) = self.rx.try_recv() {
            match d {
                Done::Classes { node, key, result } => {
                    let inherited = self
                        .picks
                        .get(&node)
                        .and_then(|p| p.like.as_ref())
                        .and_then(|l| self.picks.get(l))
                        .map(|l| l.crosswalk.clone());
                    let Some(p) = self.picks.get_mut(&node) else { continue };
                    // (the answer of an earlier file or band is let go)
                    if !matches!(&p.classes, Cls::Loading(k) if *k == key) {
                        continue;
                    }
                    p.classes = match result {
                        Ok(found) => {
                            if let Some(t) = &p.template {
                                p.crosswalk = my_data::suggest(t, &found);
                                // (the same file as another date: its table, for the classes both have)
                                for (user, to) in inherited.iter().flatten() {
                                    if found.iter().any(|c| c.0 == *user) {
                                        p.crosswalk.insert(*user, *to);
                                    }
                                }
                            }
                            Cls::Ready(found)
                        }
                        Err(e) => Cls::Failed(e),
                    };
                }
                Done::Scale { node, file, result } => {
                    // (the answer of an earlier file is let go)
                    if let Some(i) = self.picks.get_mut(&node).filter(|p| p.file == file).and_then(|p| p.image.as_mut())
                    {
                        i.detected = Some(result.unwrap_or_default());
                    }
                }
                Done::Checked { key, result } => {
                    self.working = false;
                    self.done = Some(Checked { key, result: *result });
                }
            }
        }
    }

    /// The choices that go to the plan: the files chosen, and for the maps of classes, their table.
    fn choices(&self) -> BTreeMap<String, Choice> {
        let all = self.all_dates_now();
        self.picks
            .iter()
            .filter(|(_, p)| !matches!(p.classes, Cls::Failed(_)))
            .map(|(k, p)| {
                // (every band a date: the classes are read as they are, no table, and the first date says so)
                let dated = all && self.dates.contains(k);
                let c = Choice {
                    file: p.file.clone(),
                    band: p.band.clone(),
                    crosswalk: if dated { BTreeMap::new() } else { p.crosswalk.clone() },
                    dates: dated && self.dates.first() == Some(k),
                    roles: p.image.as_ref().map(|i| i.roles.clone()),
                    scale: p.image.as_ref().and_then(|i| i.scale.or(i.detected)),
                    files: p.files.clone(),
                    scene: p.scene.clone(),
                    ..Default::default()
                };
                (k.clone(), c)
            })
            .collect()
    }

    fn reference_now(&self) -> Option<String> {
        self.reference.clone().filter(|k| self.picks.contains_key(k))
    }

    fn key(&self, rev: u64) -> String {
        format!("{rev}|{:?}|{:?}|{:?}|{:?}", self.reference_now(), self.choices(), self.effective(), self.classes)
    }

    /// What the dates of the two files give the settings (the steps between them, the dates of CLUE-S's), when both are
    /// chosen; with every band of one file a date, all of them.
    fn derive(&mut self, s: &Session) {
        self.dates = date_sources(&s.my_data.sources, &s.source);
        self.lulcc = my_data::land(&s.source) == Some(my_data::Land::Lulcc);
        let picks: Vec<&Pick> = self.dates.iter().filter_map(|d| self.picks.get(d)).collect();
        self.derived = if picks.len() == 2 {
            let every = self.all_dates_now();
            // (every band a date: the dates are all the bands' names, the classes the file's own)
            let names: Vec<String> =
                if every { picks[0].bands.clone() } else { picks.iter().filter_map(|p| band_name(p)).collect() };
            let mut t: Vec<Option<f64>> = names.iter().map(|b| b.trim().parse().ok()).collect();
            // (bands that are no dates: 0, 1, 2...)
            if t.iter().any(Option::is_none) {
                t = (0..names.len()).map(|i| Some(i as f64)).collect();
            }
            let classes = if every { found_classes(picks[0]) } else { classes_after(picks[0]) };
            my_data::derive(&s.source, &t, &classes, &self.settings)
        } else {
            BTreeMap::new()
        };
    }

    /// The model changed or another is in front: the choices of sources it no longer has go (all of them for another
    /// model); then the check of the choices as they are, in a thread, when it was not done.
    fn pump(&mut self, s: &Session, tab: usize) {
        let now = (tab, s.revision);
        if self.seen != Some(now) {
            if self.seen.is_some_and(|(t, _)| t != tab) {
                self.picks.clear();
                self.settings.clear();
                self.problems.clear();
                self.reference = None;
                self.page = Page::Data;
            } else {
                let nodes: Vec<&str> = s.my_data.sources.iter().map(|x| x.node.as_str()).collect();
                self.picks.retain(|k, _| nodes.contains(&k.as_str()));
            }
            self.seen = Some(now);
        }
        // (the classes of the rules of the model open, and the edits of those it no longer has gone)
        self.rules = my_data::rule_classes(&s.source);
        let rules = self.rules.clone();
        self.classes.retain(|k, _| rules.iter().any(|(n, _)| n == k));
        // (ordered allocation: CLUE-S's template once it is the model open)
        if let Some(o) = self.start_ordered
            && my_data::land(&s.source) == Some(my_data::Land::Lulcc)
        {
            self.start_ordered = None;
            if o {
                self.settings.insert("method".into(), Some(Json::from("ordered")));
            }
        }
        self.poll();
        self.derive(s);
        let choices = self.choices();
        let loading = self.picks.values().any(|p| {
            matches!(p.classes, Cls::Loading(_))
                || p.image.as_ref().is_some_and(|i| i.scale.is_none() && i.detected.is_none())
        });
        if choices.is_empty() {
            self.done = None;
            self.current = false;
            return;
        }
        let key = self.key(s.revision);
        self.current = !loading && self.done.as_ref().is_some_and(|d| d.key == key);
        if self.working || self.current || loading {
            return;
        }
        let (model, base, reference) = (s.source.clone(), s.base(), self.reference_now());
        let settings = self.effective();
        let classes =
            if self.picks.values().any(|p| p.image.is_some()) { self.classes.clone() } else { BTreeMap::new() };
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        self.working = true;
        std::thread::spawn(move || {
            let r = Registry::standard();
            let result = my_data::plan(&model, &r, &choices, reference.as_deref(), &base)
                .and_then(|mut p| {
                    my_data::apply_settings(&mut p.model, &settings)?;
                    p.notes.extend(setting_notes(&p.model, &settings));
                    for (node, list) in &classes {
                        match my_data::apply_classes(&mut p.model, node, list) {
                            Ok(note) => p.notes.extend(note),
                            Err(e) => p.refused.push(e),
                        }
                    }
                    Ok(p)
                })
                .map(|p| {
                    let f = my_data::check(&p, &base);
                    (p, f)
                });
            let _ = tx.send(Done::Checked { key, result: Box::new(result) });
            if let Some(c) = ctx {
                c.request_repaint();
            }
        });
    }

    /// Whether *Apply* may be done: the choices checked as they are now, no error.
    pub fn can_apply(&self) -> bool {
        self.current() && self.plan().is_some() && !self.findings().iter().any(|f| f.level == Level::Error)
    }

    /// The pages the *Back* and *Next* buttons go through, in order: the classes when a file has a table of them, the
    /// parameters when the model has some.
    fn pages(&self, has_settings: bool) -> Vec<Page> {
        let mut p = if self.land { vec![Page::Model, Page::Data] } else { vec![Page::Data] };
        if self.has_image() {
            p.push(Page::Bands);
        }
        if self.needs_classes() || (self.has_image() && !self.rules.is_empty()) {
            p.push(Page::Classes);
        }
        if has_settings {
            p.push(Page::Parameters);
        }
        p.push(Page::Review);
        p
    }

    /// Whether a file is chosen for an image (the *Bands* page has a table for it).
    fn has_image(&self) -> bool {
        self.picks.values().any(|p| p.image.is_some())
    }

    /// Whether any chosen file has a table of classes to look at.
    fn needs_classes(&self) -> bool {
        self.picks.values().any(|p| p.template.is_some())
    }

    /// Put the plan in the model as one step of undo, seen in the Changes pane; the model runs once its data is read
    /// when asked.
    pub fn apply(&mut self, s: &mut Session) -> Result<(), String> {
        let model = match (&self.done, self.can_apply()) {
            (Some(Checked { result: Ok((p, _)), .. }), true) => p.model.clone(),
            _ => return Err("the choices are not checked, or have errors".into()),
        };
        crate::review::begin(s, "Use my data", true);
        s.source = model;
        crate::builder::forget_missing(s);
        s.history.name("Use my data");
        s.edited();
        s.read_data();
        if self.run_now {
            s.run_when_ready = true;
        }
        Ok(())
    }

    /// The pages to go to, the one at hand lit.
    fn strip(&mut self, ui: &mut Ui, has_settings: bool) {
        let (land, image) = (self.land, self.has_image());
        ui.horizontal(|ui| {
            for p in Page::ALL
                .into_iter()
                .filter(|p| (*p != Page::Parameters || has_settings) && (*p != Page::Model || land))
                .filter(|p| *p != Page::Bands || image)
            {
                if ui.selectable_label(self.page == p, p.title()).clicked() {
                    self.page = p;
                }
            }
        });
    }

    /// The page of *Your data*: a row for each thing the model draws.
    fn data_page(&mut self, ui: &mut Ui, sources: &[Source], dates: &[String]) {
        ui.label(
            RichText::new(
                "A file for each thing the model draws; what you leave out stays drawn. Each choice is checked as soon \
                 as it is made.",
            )
            .weak(),
        );
        let first = sources
            .iter()
            .find(|x| self.picks.contains_key(&x.node) && x.needs == Type::Map)
            .or_else(|| sources.iter().find(|x| self.picks.contains_key(&x.node)))
            .map(|x| x.node.clone());
        let reference = self.reference_now().or(first);
        let many = self.picks.len() > 1;
        let (mut choose, mut keep, mut band, mut refer, mut same) = (None, None, None, None, None);
        let mut all: Option<(String, String)> = None;
        let mut files: Option<(String, Vec<PathBuf>)> = None;
        let mut open_form: Option<String> = None;
        let mut use_scene: Option<(String, my_data::Scene)> = None;
        let mut close_form = false;
        for src in sources {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&src.what).strong());
                let kind = match (&src.data, src.op.as_str()) {
                    (Some(d), _) => d.kind.name().to_string(),
                    (None, my_data::INPUT) => "input".to_string(),
                    (None, op) => op.to_string(),
                };
                ui.label(RichText::new(format!("{} \u{b7} {kind}", src.node)).weak().small());
            });
            let pick = self.picks.get(&src.node);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Choose a file...").clicked()
                    && let Some(p) = crate::fixes::pick_file("raster.read")
                {
                    choose = Some((src.node.clone(), PathBuf::from(p)));
                }
                // an image: a band to a file, a folder of them, or a scene of an area for the one who has no image
                if my_data::is_image(src) {
                    if ui
                        .button("Choose band files...")
                        .on_hover_text(
                            "single-band files (Sentinel-2 B02, B03...; Landsat SR_B2...): stacked in band order",
                        )
                        .clicked()
                        && let Some(p) = rfd::FileDialog::new()
                            .set_title("The band files of the image")
                            .add_filter("Rasters", &terraflux_ops::files::RASTER_EXTENSIONS)
                            .pick_files()
                    {
                        files = Some((src.node.clone(), p));
                    }
                    if ui
                        .button("Choose a folder of bands...")
                        .on_hover_text("the single-band files of a folder (a Sentinel-2 or Landsat product), by band")
                        .clicked()
                        && let Some(p) = rfd::FileDialog::new().set_title("The folder of the band files").pick_folder()
                    {
                        files = Some((src.node.clone(), vec![p]));
                    }
                    if ui
                        .button("No image of your own?")
                        .on_hover_text(
                            "choose an area and a cloud-free Sentinel-2 scene of it is read from a STAC catalogue",
                        )
                        .clicked()
                    {
                        open_form = Some(src.node.clone());
                    }
                }
                let Some(p) = pick else {
                    match &src.file {
                        Some(f) => ui.label(RichText::new(format!("keeps your file {f}")).weak()),
                        None => ui.label(RichText::new("kept drawn").weak()),
                    };
                    // two dates of one file of bands: the other band of the file chosen for the other date
                    if let Some((from, b)) = self.same_file_offer(dates, &src.node)
                        && ui
                            .button(format!("The same file, band {b}"))
                            .on_hover_text("the file chosen for the other date, in the band of this one")
                            .clicked()
                    {
                        same = Some((src.node.clone(), from, b));
                    }
                    // CLUE-S's model: every band of that file as the dates
                    if let Some((from, n)) = self.all_dates_offer(&src.node)
                        && ui
                            .button(format!("All {n} bands as the dates"))
                            .on_hover_text(
                                "the file chosen for the other date, every band one date: the first band is the first \
                                 date, the last band the last",
                            )
                            .clicked()
                    {
                        all = Some((src.node.clone(), from));
                    }
                    return;
                };
                let name = if p.scene.is_some() {
                    "a Sentinel-2 scene of your area".to_string()
                } else if !p.files.is_empty() {
                    format!("{} band files", p.files.len())
                } else {
                    p.file
                        .file_name()
                        .map_or_else(|| p.file.display().to_string(), |n| n.to_string_lossy().into_owned())
                };
                let tip = match &p.scene {
                    Some(sc) => format!(
                        "{:?}, {} to {}, read from Earth Search when the model runs",
                        sc.extent, sc.dates, sc.cellsize
                    ),
                    None if !p.files.is_empty() => p.bands.join(", "),
                    None => p.file.display().to_string(),
                };
                ui.label(RichText::new(name).strong()).on_hover_text(tip);
                if src.needs == Type::Stack {
                    ui.label(RichText::new(format!("all {} bands", p.bands.len())).weak());
                } else if p.bands.len() > 1 {
                    let shown = p.band.clone().unwrap_or_else(|| p.bands[0].clone());
                    egui::ComboBox::from_id_salt(("my data band", &src.node))
                        .selected_text(format!("band {shown}"))
                        .show_ui(ui, |ui| {
                            for b in &p.bands {
                                if ui.selectable_label(*b == shown, b).clicked() {
                                    band = Some((src.node.clone(), Some(b.clone())));
                                }
                            }
                        });
                }
                if many
                    && ui
                        .radio(reference.as_deref() == Some(src.node.as_str()), "grid of reference")
                        .on_hover_text("the grid the other files are put on (cells, extent, CRS)")
                        .clicked()
                {
                    refer = Some(src.node.clone());
                }
                if ui.small_button("Keep drawn").on_hover_text("no file for it: the model draws it as before").clicked()
                {
                    keep = Some(src.node.clone());
                }
            });
            if let Some(e) = self.problems.get(&src.node) {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            if let Some((_, sc)) = self.scene_form.as_mut().filter(|f| f.0 == src.node) {
                scene_form(ui, sc, &mut use_scene, &mut close_form, &src.node);
            }
            if let Some(Pick { classes: Cls::Failed(e), .. }) = pick {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            if pick.is_some() && self.current {
                self.row_findings(ui, &src.node);
            }
        }
        ui.separator();
        if let Some((_, bands)) = self.dates_file() {
            ui.checkbox(&mut self.all_dates, format!("All {} bands are the dates ({})", bands.len(), bands.join(", ")))
                .on_hover_text(
                    "every band of the file one date for the demand; your classes are read as they are, with no table",
                );
        }
        if let Some(c) = &self.done
            && self.current
        {
            match &c.result {
                Err(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                Ok((_, f)) => {
                    let named: BTreeSet<&str> = sources.iter().map(|x| x.node.as_str()).collect();
                    for f in f.iter().filter(|f| !named.contains(finding_node(f))) {
                        finding_line(ui, f);
                    }
                }
            }
        } else if !self.picks.is_empty() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Checking the data...");
            });
        }
        if let Some((n, p)) = choose {
            self.pending(n, p);
        }
        if let Some((n, from, b)) = same
            && let Err(e) = self.same_file(sources, &n, &from, &b)
        {
            self.error = Some(e);
        }
        if let Some((n, from)) = all
            && let Err(e) = self.all_dates_from(sources, &n, &from)
        {
            self.error = Some(e);
        }
        if let Some(f) = files {
            self.asked_files = Some(f);
        }
        if let Some(n) = open_form {
            let kept = self.picks.get(&n).and_then(|p| p.scene.clone());
            self.scene_form = Some((n, kept.unwrap_or_else(|| my_data::Scene::new([-47.90, -15.90, -47.85, -15.85]))));
        }
        if let Some(u) = use_scene {
            self.asked_scene = Some(u);
        }
        if close_form {
            self.scene_form = None;
        }
        if let Some(n) = keep {
            self.keep_drawn(&n);
        }
        if let Some((n, b)) = band {
            self.set_band(&n, b);
        }
        if let Some(n) = refer {
            self.reference = Some(n);
        }
    }

    /// The findings and refusals of the check that are about a source, under its row.
    fn row_findings(&self, ui: &mut Ui, node: &str) {
        let Some(Checked { result: Ok((plan, findings)), .. }) = &self.done else { return };
        let prefix = format!("{node}: ");
        for line in plan.refused.iter().filter(|l| l.starts_with(&prefix)) {
            ui.colored_label(ui.visuals().warn_fg_color, format!("Refused: {}", &line[prefix.len()..]));
        }
        for f in findings.iter().filter(|f| finding_node(f) == node) {
            finding_line(ui, f);
        }
    }

    /// A file chosen at a click: the reason shown in the row when it cannot be used. (The click has no session at
    /// hand: the choice is made on the next frame, which has.)
    fn pending(&mut self, node: String, file: PathBuf) {
        self.asked = Some((node, file));
    }

    /// The page of *Bands*: for each image chosen, the role of each band (what the model's attributes and rules
    /// call it: blue, green, red, nir, swir1, swir2) and the scale of its reflectance.
    fn bands_page(&mut self, ui: &mut Ui, sources: &[Source]) {
        let mut any = false;
        let chosen: Vec<&Source> =
            sources.iter().filter(|x| self.picks.get(&x.node).is_some_and(|p| p.image.is_some())).collect();
        for src in chosen {
            any = true;
            ui.separator();
            ui.label(RichText::new(&src.what).strong());
            let Some(p) = self.picks.get_mut(&src.node) else { continue };
            let bands = p.bands.clone();
            let Some(img) = p.image.as_mut() else { continue };
            ui.label(
                RichText::new(
                    "The role of each band, as the model reads it: its attributes are named for them (mean_nir, ndvi...). \
                     A band with the role other is left out. Guessed from the names of the bands (Sentinel-2 B02, B03, \
                     B04, B08, B11, B12; Landsat 8/9 SR_B2 to SR_B7).",
                )
                .weak(),
            );
            egui::Grid::new(("my data bands", &src.node)).striped(true).show(ui, |ui| {
                ui.label(RichText::new("Band").strong());
                ui.label(RichText::new("Role").strong());
                ui.end_row();
                for (k, name) in bands.iter().enumerate() {
                    ui.label(name);
                    let now = img.roles.get(k).cloned().unwrap_or_else(|| "other".into());
                    egui::ComboBox::from_id_salt(("my data role", &src.node, k)).selected_text(&now).show_ui(
                        ui,
                        |ui| {
                            for role in ROLES.iter().copied().chain(["other"]) {
                                if ui.selectable_label(now == role, role).clicked() {
                                    img.roles[k] = role.to_string();
                                }
                            }
                        },
                    );
                    ui.end_row();
                }
            });
            let mut seen = BTreeSet::new();
            if let Some(r) = img.roles.iter().filter(|r| *r != "other").find(|r| !seen.insert(r.as_str())) {
                ui.colored_label(ui.visuals().error_fg_color, format!("Two bands are {r}: a role is one band's."));
            } else if img.roles.iter().all(|r| r == "other") {
                ui.colored_label(ui.visuals().warn_fg_color, "No band has a role: choose at least red and nir.");
            }
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Reflectance").strong());
                match img.detected {
                    None => {
                        ui.spinner();
                        ui.label("Reading its values...");
                    }
                    Some(d) => {
                        let now = img.scale.unwrap_or(d);
                        let auto = if img.scale.is_none() { " (detected)" } else { "" };
                        ui.label(format!("{}{auto}", reflectance_line(now)));
                        egui::ComboBox::from_id_salt(("my data scale", &src.node)).selected_text(now.label()).show_ui(
                            ui,
                            |ui| {
                                for sc in Scale::ALL {
                                    if ui.selectable_label(now == sc, sc.label()).clicked() {
                                        img.scale = Some(sc);
                                    }
                                }
                            },
                        );
                    }
                }
            });
        }
        if !any {
            ui.label(RichText::new("No file chosen is an image.").weak());
        }
    }

    /// The page of *Classes*: for each chosen map of classes, the user's classes against the model's.
    fn classes_page(&mut self, ui: &mut Ui, sources: &[Source]) {
        let mut any = false;
        let chosen: Vec<&Source> =
            sources.iter().filter(|x| is_classes(x) && self.picks.contains_key(&x.node)).collect();
        for src in chosen {
            any = true;
            ui.separator();
            ui.label(RichText::new(&src.what).strong());
            let Some(p) = self.picks.get(&src.node) else { continue };
            if self.all_dates_now() && self.dates.contains(&src.node) {
                ui.label(
                    RichText::new("every band is a date: your classes are read as they are, with no table").weak(),
                );
                continue;
            }
            let Some(template) = p.template.clone() else {
                ui.label(RichText::new("any classes: your classes are kept as they are").weak());
                continue;
            };
            let found = match &p.classes {
                Cls::Ready(f) => f.clone(),
                Cls::Failed(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                    continue;
                }
                _ => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Reading its classes...");
                    });
                    continue;
                }
            };
            let mut cross = p.crosswalk.clone();
            egui::ScrollArea::vertical().id_salt(("my data classes", &src.node)).max_height(240.0).show(ui, |ui| {
                egui::Grid::new(("my data table", &src.node)).striped(true).show(ui, |ui| {
                    for h in ["Your class", "Name", "Cells", "Becomes"] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (code, name, cells) in &found {
                        ui.label(code.to_string());
                        ui.label(name.clone().unwrap_or_default());
                        ui.label(cells.to_string());
                        let to = cross.get(code).copied().flatten();
                        let shown = match to {
                            Some(t) => format!("{t} {}", template.get(&t).map_or("", String::as_str)),
                            None => "no data".to_string(),
                        };
                        egui::ComboBox::from_id_salt(("my data class", &src.node, code)).selected_text(shown).show_ui(
                            ui,
                            |ui| {
                                if ui.selectable_label(to.is_none(), "no data").clicked() {
                                    cross.insert(*code, None);
                                }
                                for (t, n) in &template {
                                    if ui.selectable_label(to == Some(*t), format!("{t} {n}")).clicked() {
                                        cross.insert(*code, Some(*t));
                                    }
                                }
                            },
                        );
                        ui.end_row();
                    }
                });
            });
            let (none, total) = no_data_cells(&found, &cross);
            let pct = if total == 0 { 0.0 } else { 100.0 * none as f64 / total as f64 };
            ui.label(format!("{none} of {total} cells go to no data ({pct:.1}%)."));
            if let Some(p) = self.picks.get_mut(&src.node) {
                p.crosswalk = cross;
            }
        }
        if self.has_image() && !self.rules.is_empty() {
            any = true;
            self.image_classes(ui);
        }
        if !any {
            ui.label(RichText::new("No file chosen has classes to translate.").weak());
        }
    }

    /// The classes of the template's rules (`objects.rules`), to be the user's: each a name and a colour, the ones
    /// not wanted removed, new ones added (with the rule 0: no object until a rule is written for them). The rules of
    /// the classes kept stay, with the references to a renamed class renamed. Classes by samples have none here:
    /// their names are the ones of the samples.
    fn image_classes(&mut self, ui: &mut Ui) {
        ui.separator();
        ui.label(RichText::new("The classes of the image").strong());
        ui.label(
            RichText::new(
                "Your classes in place of the template's: rename, recolour, remove, add. A class keeps the rule it \
                 had (edit the rules in the model afterwards); a new class has the rule 0 until you write one. Where \
                 you classify by samples, the classes are the names of your samples.",
            )
            .weak(),
        );
        let mut changes: Vec<(String, Option<Vec<UserClass>>)> = Vec::new();
        for (node, start) in &self.rules {
            let mut list = self.classes.get(node).cloned().unwrap_or_else(|| start.clone());
            let before = list.clone();
            ui.label(RichText::new(format!("Rules of {node}")).strong());
            let mut gone = None;
            egui::Grid::new(("my data image classes", node)).striped(true).show(ui, |ui| {
                for h in ["Name", "Colour", "Was", ""] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (k, c) in list.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(140.0).id_salt((node, "n", k)));
                    let mut rgb = c.colour.as_deref().and_then(hex_rgb).unwrap_or([128, 128, 128]);
                    if ui.color_edit_button_srgb(&mut rgb).changed() {
                        c.colour = Some(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]));
                    }
                    ui.label(RichText::new(c.from.clone().unwrap_or_else(|| "new".into())).weak());
                    if ui.small_button("Remove").clicked() {
                        gone = Some(k);
                    }
                    ui.end_row();
                }
            });
            if let Some(k) = gone {
                list.remove(k);
            }
            ui.horizontal(|ui| {
                if ui.small_button("Add a class").clicked() {
                    let mut name = "class".to_string();
                    for k in 2.. {
                        if !list.iter().any(|c| c.name == name) {
                            break;
                        }
                        name = format!("class {k}");
                    }
                    list.push(UserClass { from: None, name, colour: None });
                }
                if ui.small_button("Back to the template's").clicked() {
                    list = start.clone();
                }
            });
            if list != before {
                changes.push((node.clone(), (list != *start).then_some(list)));
            }
        }
        for (node, list) in changes {
            match list {
                Some(l) => self.classes.insert(node, l),
                None => self.classes.remove(&node),
            };
        }
    }

    /// The page of *Parameters*: the settings of the model (the planned one is made with these values): the
    /// model's exposed parameters and, for a land change model, its own (steps, increment, order of the classes...).
    fn parameters_page(&mut self, ui: &mut Ui, s: &mut Session) {
        let all = self.settings_of(&s.source);
        if all.is_empty() {
            ui.label(RichText::new("This model has no parameters to set.").weak());
            return;
        }
        ui.label(
            RichText::new(
                "What the model does, apart from the data. Where your files have dates (their bands' names) the steps \
                 and dates start from them; a value you set stays.",
            )
            .weak(),
        );
        let method = all
            .iter()
            .find(|(st, _)| st.id == "method")
            .and_then(|(_, v)| v.as_ref().and_then(Json::as_str).map(String::from))
            .unwrap_or_default();
        let mut change: Option<(String, Option<Json>)> = None;
        for (st, now) in all.iter().filter(|(st, _)| my_data::shown(&st.id, &method)) {
            ui.separator();
            ui.label(RichText::new(st.param.label.clone().unwrap_or_else(|| st.id.clone())).strong());
            if let Some(h) = &st.param.help {
                ui.label(RichText::new(h).small().weak());
            }
            ui.horizontal_wrapped(|ui| {
                if let Some((text, on)) = &st.auto {
                    let mut auto = now.is_none();
                    if ui.checkbox(&mut auto, text).changed() {
                        change = Some((st.id.clone(), if auto { None } else { Some(on.clone()) }));
                    }
                }
                if now.is_some() || st.auto.is_none() {
                    // (its own text fields: the Parameters pane's are of the model open)
                    let p = Parameter { key: format!("my data.{}", st.id), ..st.param.clone() };
                    if let Some(v) = crate::params_view::control(ui, s, &p, now.as_ref(), "default") {
                        change = Some((st.id.clone(), Some(v)));
                    }
                }
            });
        }
        if let Some((id, v)) = change {
            self.set_setting(&id, v);
        }
    }

    /// The page of *Review*: what will be done, what was refused, the findings.
    fn review_page(&mut self, ui: &mut Ui, before: &Model) {
        let Some(c) = &self.done else {
            ui.label(RichText::new("Choose the file of at least one data first.").weak());
            return;
        };
        if !self.current {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Checking the data...");
            });
            return;
        }
        let (plan, findings) = match &c.result {
            Err(e) => {
                ui.colored_label(ui.visuals().error_fg_color, e);
                return;
            }
            Ok((p, f)) => (p, f),
        };
        let (data, added, taken) = summary(before, plan);
        ui.label(
            RichText::new(format!(
                "{} replaced, {} added, {} taken away.",
                plural(data, "data", "data"),
                plural(added, "node", "nodes"),
                taken
            ))
            .strong(),
        );
        ui.add_space(4.0);
        ui.label(RichText::new("What will be done").strong());
        for n in &plan.notes {
            ui.label(RichText::new(format!("\u{b7} {n}")).small());
        }
        if !plan.refused.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Not done").strong());
            for r in &plan.refused {
                ui.colored_label(ui.visuals().warn_fg_color, r);
            }
        }
        ui.add_space(4.0);
        if findings.is_empty() {
            ui.label("The data was checked and nothing looks wrong.");
        } else {
            ui.label(RichText::new("What the check found").strong());
            for f in findings {
                finding_line(ui, f);
            }
        }
        if findings.iter().any(|f| f.level == Level::Error) {
            ui.colored_label(ui.visuals().error_fg_color, "An error stops it: put it right (or keep that data drawn).");
        }
    }
}

/// The form of an image's Sentinel-2 scene: the area (west, south, east, north), the cells and the dates, and what is
/// done with them; *Use this area* sets `use_scene`, *Cancel* `close`.
fn scene_form(
    ui: &mut Ui,
    sc: &mut my_data::Scene,
    use_scene: &mut Option<(String, my_data::Scene)>,
    close: &mut bool,
    node: &str,
) {
    ui.group(|ui| {
        ui.label(RichText::new("A cloud-free Sentinel-2 scene of your area").strong());
        ui.label(
            RichText::new(
                "Nothing is downloaded now. When the model runs, Terraflux searches Earth Search (Sentinel-2 L2A) \
                 for the scenes of those dates with less than 20% of cloud, takes the clouds, shadows and cirrus of \
                 each out pixel by pixel, and combines them (the median) into the bands blue, green, red, nir, swir1 \
                 and swir2, read only for your area. The area is a small one: at 10 m, a side of 5 km is 500 cells.",
            )
            .weak(),
        );
        let [w, s, e, n] = &mut sc.extent;
        fn drag(v: &mut f64, lo: f64, hi: f64) -> egui::DragValue<'_> {
            egui::DragValue::new(v).range(lo..=hi).speed(0.001).max_decimals(5).suffix("\u{b0}")
        }
        ui.horizontal(|ui| {
            ui.label("west");
            ui.add(drag(w, -180.0, 180.0));
            ui.label("east");
            ui.add(drag(e, -180.0, 180.0));
            ui.label("south");
            ui.add(drag(s, -90.0, 90.0));
            ui.label("north");
            ui.add(drag(n, -90.0, 90.0));
        });
        ui.horizontal(|ui| {
            ui.label("cells");
            let shown =
                my_data::SCENE_CELLS.iter().find(|(c, _)| (c - sc.cellsize).abs() < 1e-9).map_or("", |(_, n)| *n);
            egui::ComboBox::from_id_salt(("my data scene cells", node)).selected_text(shown).show_ui(ui, |ui| {
                for (c, name) in my_data::SCENE_CELLS {
                    ui.selectable_value(&mut sc.cellsize, c, name);
                }
            });
            ui.label("dates");
            ui.add(egui::TextEdit::singleline(&mut sc.dates).desired_width(190.0)).on_hover_text(
                "a day (2025-06-01) or an interval (2025-06-01/2025-08-31); the last 12 months by default",
            );
        });
        let problem = sc.problem();
        match &problem {
            Some(p) => {
                ui.colored_label(ui.visuals().error_fg_color, p);
            }
            None => {
                ui.label(RichText::new(format!("{} \u{d7} {} cells", sc.cols(), sc.rows())).weak());
            }
        }
        ui.horizontal(|ui| {
            if ui.add_enabled(problem.is_none(), egui::Button::new("Use this area")).clicked() {
                *use_scene = Some((node.to_string(), sc.clone()));
            }
            if ui.button("Cancel").clicked() {
                *close = true;
            }
        });
    });
}

/// A finding: its level, where, what, and what to do about it.
fn finding_line(ui: &mut Ui, f: &Finding) {
    let (word, colour) = match f.level {
        Level::Error => ("Error", ui.visuals().error_fg_color),
        Level::Warning => ("Warning", ui.visuals().warn_fg_color),
        Level::Info => ("Note", ui.visuals().text_color()),
    };
    let at = if f.at.is_empty() { String::new() } else { format!("{}: ", f.at) };
    ui.colored_label(colour, format!("{word}: {at}{}", f.message));
    if let Some(fix) = &f.fix {
        ui.label(RichText::new(format!("what to do: {fix}")).weak().small());
    }
}

/// The first page of the land change entry points: which model; the one clicked, if any.
fn model_page(ui: &mut Ui) -> Option<Kind> {
    let mut chosen = None;
    ui.label(RichText::new("Which land change model").strong().size(15.0));
    ui.label(
        RichText::new(
            "Choose the model: its template opens, and the next page asks for your maps and drivers in place of the \
             data it draws.",
        )
        .weak(),
    );
    for k in Kind::ALL {
        ui.add_space(6.0);
        if ui.button(RichText::new(k.title()).strong()).clicked() {
            chosen = Some(k);
        }
        ui.indent(k.title(), |ui| ui.label(k.about()));
    }
    chosen
}

/// The page when the model draws nothing to replace: the templates, to start one (its drawn data is then replaced).
fn templates_page(ui: &mut Ui, act: &mut Option<Act>, empty: bool) {
    ui.label(RichText::new("Open a template first").strong().size(15.0));
    ui.label(
        RichText::new(if empty {
            "Use my data puts your files in place of the data a template draws. Start one here, then choose your files."
        } else {
            "This model draws no data your files could replace. Start a template here, or open a model that does."
        })
        .weak(),
    );
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for g in crate::templates::GROUPS {
            ui.add_space(6.0);
            ui.label(RichText::new(g).strong());
            ui.horizontal_wrapped(|ui| {
                for (k, t) in crate::templates::TEMPLATES.iter().enumerate().filter(|(_, t)| t.group == g) {
                    if ui.add(egui::Button::new(t.name).small()).on_hover_text(t.about).clicked() {
                        *act = Some(Act::Template(k));
                    }
                }
            });
        }
    });
}

/// Draw the window (when there is one) over the model open of the tab `tab`; a template asked for, if any. *Apply*
/// closes the window.
pub fn window(
    ctx: &egui::Context,
    state: &mut Option<State>,
    tab: usize,
    session: Option<&mut Session>,
    r: &Registry,
) -> Option<Act> {
    let mut st = state.take()?;
    st.ctx = Some(ctx.clone());
    let mut act = None;
    let mut open = true;
    let mut applied = false;
    let screen = ctx.content_rect();
    egui::Window::new("Use my data")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(720.0f32.min(screen.width() - 40.0).max(320.0), 560.0f32.min(screen.height() - 60.0)))
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(screen.center())
        .show(ctx, |ui| {
            if st.land && st.page == Page::Model {
                let has_settings = session.as_deref().is_some_and(|s| !my_data::settings(&s.source).is_empty());
                st.strip(ui, has_settings);
                ui.separator();
                if let Some(k) = model_page(ui)
                    && let Some(t) = st.choose_model(k)
                {
                    act = Some(Act::Template(t));
                }
                return;
            }
            let Some(s) = session else {
                if st.land {
                    st.strip(ui, false);
                    ui.separator();
                }
                templates_page(ui, &mut act, true);
                return;
            };
            let sources = sources_of(s, r).to_vec();
            let dates = date_sources(&sources, &s.source);
            if sources.is_empty() {
                if st.land {
                    st.strip(ui, false);
                    ui.separator();
                }
                templates_page(ui, &mut act, s.source.nodes.is_empty());
                return;
            }
            // (a file chosen at a click in the last frame)
            if let Some((node, file)) = st.asked.take()
                && let Err(e) = st.choose(s, r, &node, &file)
            {
                st.error = Some(e);
            }
            if let Some((node, paths)) = st.asked_files.take()
                && let Err(e) = st.choose_files(s, r, &node, &paths)
            {
                st.error = Some(e);
            }
            if let Some((node, scene)) = st.asked_scene.take()
                && let Err(e) = st.choose_scene(s, r, &node, scene)
            {
                st.error = Some(e);
            }
            st.pump(s, tab);
            let has_settings = !my_data::settings(&s.source).is_empty();
            st.strip(ui, has_settings);
            let height = (ui.available_height() - 46.0).max(80.0);
            egui::ScrollArea::vertical().max_height(height).auto_shrink([false, false]).show(ui, |ui| match st.page {
                Page::Model | Page::Data => st.data_page(ui, &sources, &dates),
                Page::Bands => st.bands_page(ui, &sources),
                Page::Classes => st.classes_page(ui, &sources),
                Page::Parameters => st.parameters_page(ui, s),
                Page::Review => st.review_page(ui, &s.source),
            });
            ui.separator();
            ui.horizontal(|ui| {
                let any = !st.picks.is_empty();
                // the pages the buttons go through: classes only when a file has some, the parameters when there are
                let pages = st.pages(has_settings);
                let at = pages.iter().position(|p| *p == st.page).unwrap_or(0);
                if st.page != Page::Review {
                    if at > 0 && ui.button("Back").clicked() {
                        st.page = pages[at - 1];
                    }
                    let enabled = any || st.page != Page::Data;
                    if ui
                        .add_enabled(enabled, egui::Button::new("Next"))
                        .on_disabled_hover_text("choose the file of at least one data")
                        .clicked()
                    {
                        st.page = pages[(at + 1).min(pages.len() - 1)];
                    }
                } else {
                    if ui.button("Back").clicked() {
                        st.page = pages[at.saturating_sub(1)];
                    }
                    let can = st.can_apply();
                    if ui
                        .add_enabled(can, crate::app::accent_button("Apply", can))
                        .on_hover_text("the model with your data, as one step of undo, seen in the Changes pane")
                        .on_disabled_hover_text("the choices are being checked, or have an error")
                        .clicked()
                    {
                        match st.apply(s) {
                            Ok(()) => applied = true,
                            Err(e) => st.error = Some(e),
                        }
                    }
                    ui.checkbox(&mut st.run_now, "Run it now").on_hover_text("run the model once its data is read");
                }
                if let Some(e) = &st.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
            });
        });
    if open && !applied {
        *state = Some(st);
    }
    act
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::TEMPLATES;
    use terraflux_graph::{Data, Node};

    fn template(name: &str) -> Model {
        TEMPLATES.iter().find(|t| t.name == name).unwrap().model()
    }

    #[test]
    fn a_model_draws_data_when_a_source_is_an_nlm_a_read_or_an_input() {
        let r = Registry::standard();
        assert!(drawn(&my_data::sources(&template("MSPA of a forest"), &r)));
        let mut m = Model::new("t");
        let n = Node::new("a", "raster.calc").data(Data::new("a map", DataKind::Values));
        m.nodes.push(n);
        // a node that stands for data but is no drawing: not drawn
        assert!(!drawn(&my_data::sources(&m, &r)));
        m.nodes.push(Node::new("b", "raster.read"));
        assert!(drawn(&my_data::sources(&m, &r)));
        assert!(!drawn(&[]));
    }

    #[test]
    fn a_model_with_only_the_users_files_draws_nothing_but_offers_them_again() {
        let r = Registry::standard();
        let mut m = Model::new("t");
        let input = |path: &str, data: Option<Data>| terraflux_graph::Input::Map {
            path: path.into(),
            band: None,
            nodata: None,
            note: None,
            data,
        };
        m.inputs.insert("mine".into(), input("a.tif", Some(Data::new("a map", DataKind::Values))));
        let s = my_data::sources(&m, &r);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].file.as_deref(), Some("a.tif"));
        assert!(!drawn(&s), "a file of the user's is not drawn data: no banner");
        // an input with no path is drawn
        m.inputs.insert("empty".into(), input("", None));
        assert!(drawn(&my_data::sources(&m, &r)));
    }

    #[test]
    fn the_second_date_is_offered_the_same_file_in_another_band() {
        let r = Registry::standard();
        let file =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
        let mut s = Session::from_model(template("Dinamica EGO")).unwrap();
        let sources = sources_of(&mut s, &r).to_vec();
        let dates = date_sources(&sources, &s.source);
        assert_eq!(dates, ["land_2000", "land_2010"]);
        assert!(date_sources(&sources, &template("MSPA of a forest")).is_empty());
        let mut w = State::new();
        assert!(w.same_file_offer(&dates, "land_2010").is_none(), "no file chosen");
        w.choose(&mut s, &r, "land_2000", &file).unwrap();
        // the file has two bands: the later date takes the next, the earlier the previous one
        assert_eq!(w.same_file_offer(&dates, "land_2010"), Some(("land_2000".to_string(), "2011".to_string())));
        assert!(w.same_file_offer(&dates, "land_2000").is_none(), "no band before the first");
        assert!(w.same_file_offer(&dates, "drivers").is_none(), "not a date");
        w.same_file(&sources, "land_2010", "land_2000", "2011").unwrap();
        assert!(w.picked("land_2010"));
        assert_eq!(band_name(&w.picks["land_2010"]).as_deref(), Some("2011"));
        assert_eq!(band_name(&w.picks["land_2000"]).as_deref(), Some("1997"));
        assert!(w.same_file(&sources, "land_2010", "land_2000", "1900").is_err());
        assert!(w.same_file(&sources, "land_2010", "drivers", "1997").is_err());
        // the first date with the second's band chosen: the other is offered its previous one
        w.keep_drawn("land_2000");
        w.set_band("land_2010", Some("2011".into()));
        assert_eq!(w.same_file_offer(&dates, "land_2000"), Some(("land_2010".to_string(), "1997".to_string())));
    }

    #[test]
    fn the_sources_are_worked_out_once_for_a_revision() {
        let r = Registry::standard();
        let mut s = Session::from_model(template("MSPA of a forest")).unwrap();
        assert_eq!(sources_of(&mut s, &r).len(), 1);
        // the model changed under the cache without a new revision: the cache stands
        s.source.nodes.clear();
        assert_eq!(sources_of(&mut s, &r).len(), 1);
        s.edited();
        assert!(sources_of(&mut s, &r).is_empty());
        assert!(!s.my_data.drawn);
    }

    #[test]
    fn the_summary_counts_what_the_plan_replaced_added_and_took_away() {
        let r = Registry::standard();
        let m = template("MSPA of a forest");
        let file =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
        let c = Choice {
            file,
            band: Some("1997".into()),
            crosswalk: BTreeMap::from([(1, Some(1)), (2, Some(1)), (3, Some(3)), (4, Some(3)), (5, None)]),
            ..Default::default()
        };
        let p = my_data::plan(&m, &r, &BTreeMap::from([("landscape".to_string(), c)]), None, Path::new("")).unwrap();
        // the drawn node goes, its reclassification and its legend come
        assert_eq!(summary(&m, &p), (1, 2, 1));
    }

    #[test]
    fn the_cells_that_go_to_no_data() {
        let found: Found = vec![(1, None, 6), (2, None, 3), (5, None, 1)];
        let cross = BTreeMap::from([(1, Some(1)), (2, None)]);
        // 2 mapped to nothing, 5 not in the table
        assert_eq!(no_data_cells(&found, &cross), (4, 10));
        assert_eq!(no_data_cells(&Vec::new(), &BTreeMap::new()), (0, 0));
    }

    #[test]
    fn a_mask_maps_to_zero_and_one_and_any_classes_to_none() {
        let mask = Node::new("m", "raster.calc").data(Data::new("a mask", DataKind::Mask));
        let r = Registry::standard();
        let mut m = Model::new("t");
        m.nodes.push(mask);
        let s = &my_data::sources(&m, &r)[0];
        assert_eq!(template_of(s).unwrap().keys().copied().collect::<Vec<_>>(), [0, 1]);
        let any = Node::new("c", "raster.calc").data(Data::new("any classes", DataKind::Classes));
        let mut m = Model::new("t");
        m.nodes.push(any);
        let s = &my_data::sources(&m, &r)[0];
        assert!(template_of(s).is_none() && is_classes(s));
    }
}
