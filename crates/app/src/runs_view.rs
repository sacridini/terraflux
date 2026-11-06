//! The Runs pane: the journal of the model in front (`.terraflux` beside its file, see
//! [`terraflux_exec::journal`]), newest run first. Each row says when it ran, how long, how it ended, what it was
//! (the model, up to a node, or a batch of Many runs), the scenario, what its values changed from the run before,
//! the outputs it wrote and whether the model itself changed. One or two rows can be selected (click, ctrl+click):
//! under the table, one run is told in full (its values, outputs, warnings) with the actions *Show its outputs* (as
//! layers of the Viewer) and *Restore this model* (one step of undo); two runs are compared (their values side by
//! side, their models through the Review pane, their maps in the Viewer's comparison).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Margin, RichText, Ui};
use serde_json::Value as Json;
use terraflux_exec::journal::{Entry, Journal, Kind, Status, Written};
use terraflux_graph::Model;
use terraflux_ops::Registry;

use crate::app::Session;
use crate::builder::How;
use crate::theme;

/// How often the journal's file is looked at again for runs another program wrote (the CLI, the MCP).
const LOOK: Duration = Duration::from_secs(2);

/// The values changes told in a row, the rest in its hover.
const SHOWN: usize = 3;

/// The pane's state: the journal read, and the runs selected.
#[derive(Default)]
pub struct State {
    /// The runs, the oldest first.
    pub entries: Vec<Entry>,
    /// The outputs written over since, by the run's id (their names).
    pub over: BTreeMap<u64, Vec<String>>,
    /// The runs selected (one or two, by id); the actions that come next take them.
    pub selected: Vec<u64>,
    /// Two runs compared: every value, not only those that differ.
    pub all_values: bool,
    /// What was read: the model's file, the count of runs the session and the batches wrote, the file's size.
    read: Option<(PathBuf, u64, u64)>,
    checked: Option<Instant>,
    /// The tab shown: the runs, or the named versions of the model.
    pub tab: Tab,
    pub versions: crate::versions_view::State,
}

/// The pane's two tabs.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Runs,
    Versions,
}

/// Select an item (alone), or with ctrl add it to the selection (two at most: the one picked first goes) or take it
/// out.
pub fn pick(selected: &mut Vec<u64>, id: u64, add: bool) {
    let at = selected.iter().position(|s| *s == id);
    match (add, at) {
        (_, Some(k)) if add || selected.len() == 1 => {
            selected.remove(k);
        }
        (true, None) => {
            if selected.len() == 2 {
                selected.remove(0);
            }
            selected.push(id);
        }
        _ => *selected = vec![id],
    }
}

impl State {
    /// Read the journal of a model again when it changed (another model, a run that ended, a program that wrote to
    /// it), and look at the outputs again every few seconds; not at every frame.
    pub fn refresh(&mut self, model_file: Option<&Path>, rev: u64) {
        let Some(file) = model_file else {
            *self = State::default();
            return;
        };
        let Some(journal) = Journal::of(file) else {
            *self = State::default();
            return;
        };
        let due = self.checked.is_none_or(|t| t.elapsed() > LOOK);
        let size = journal.size();
        let same = self.read.as_ref().is_some_and(|(f, r, _)| f == file && *r == rev);
        let unchanged = self.read.as_ref().is_some_and(|(_, _, s)| *s == size);
        if same && unchanged && !due {
            return;
        }
        if !(same && unchanged) {
            if !same {
                self.selected.clear();
            }
            self.entries = journal.entries();
            self.selected.retain(|id| self.entries.iter().any(|e| e.id == *id));
        }
        self.over = self
            .entries
            .iter()
            .map(|e| (e.id, e.outputs.iter().filter(|w| !w.unchanged()).map(|w| w.name.clone()).collect::<Vec<_>>()))
            .filter(|(_, o)| !o.is_empty())
            .collect();
        self.read = Some((file.to_path_buf(), rev, size));
        self.checked = Some(Instant::now());
    }

    /// Select a run (alone), or with ctrl add it to the selection (two at most: the one picked first goes) or
    /// take it out.
    pub fn select(&mut self, id: u64, add: bool) {
        pick(&mut self.selected, id, add);
    }
}

/// A value in a row: a text as it is, others as JSON, cut when long.
fn show(v: &Json) -> String {
    let t = match v {
        Json::String(s) => s.clone(),
        v => v.to_string(),
    };
    if t.chars().count() > 24 { format!("{}...", t.chars().take(22).collect::<String>()) } else { t }
}

/// What the values of a run changed from the one before: "patches.isometry 1.5 -> 2" (a value added: "= 2"; one left
/// out: "removed").
pub fn changes(before: &BTreeMap<String, Json>, now: &BTreeMap<String, Json>) -> Vec<String> {
    let mut out = Vec::new();
    for (k, v) in now {
        match before.get(k) {
            Some(b) if b == v => {}
            Some(b) => out.push(format!("{k} {} -> {}", show(b), show(v))),
            None => out.push(format!("{k} = {}", show(v))),
        }
    }
    for k in before.keys().filter(|k| !now.contains_key(*k)) {
        out.push(format!("{k} removed"));
    }
    out
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

/// What a run was, in a few words.
pub fn kind_text(k: &Kind) -> String {
    match k {
        Kind::Run => "Run".into(),
        Kind::To(node) => format!("Run to {node}"),
        Kind::Batch(b) => {
            let mut t = capital(&b.kind);
            if let Some(m) = &b.method {
                t.push_str(&format!(" ({m})"));
            }
            t.push_str(&format!(", {} runs", b.runs));
            if b.failed > 0 {
                t.push_str(&format!(", {} failed", b.failed));
            }
            t
        }
    }
}

/// The folder a run's files are in: a batch's results, else where its outputs were written.
fn folder_of(e: &Entry) -> Option<PathBuf> {
    match &e.kind {
        Kind::Batch(b) => Some(b.out.clone()),
        _ => e
            .outputs
            .first()
            .and_then(|w| Path::new(&w.path).parent().map(Path::to_path_buf))
            .or_else(|| e.manifest.as_deref().and_then(Path::parent).map(Path::to_path_buf)),
    }
}

/// What a batch tells in its hover: its folder, and for a sensitivity analysis its factors ranked.
fn kind_hover(k: &Kind) -> String {
    match k {
        Kind::Batch(b) => {
            let mut t = format!("results in {}", b.out.display());
            for f in b.factors.iter().take(5) {
                match f.index {
                    Some(i) => t.push_str(&format!("\n{}: {} {i:.3}", f.key, f.by)),
                    None => t.push_str(&format!("\n{}", f.key)),
                }
            }
            t
        }
        Kind::To(node) => format!("the nodes {node} needs, not the whole model"),
        Kind::Run => "the whole model".into(),
    }
}

/// The Runs pane.
pub fn pane(ui: &mut Ui, st: &mut State, s: &mut Session, r: &Registry, batches: u64) {
    st.refresh(s.path.as_deref(), s.journal_rev + batches);
    st.versions.refresh(s.path.as_deref(), s.journal_rev);
    egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
        theme::section(ui, "Runs");
        if s.path.is_none() {
            ui.label(
                RichText::new("Save the model to keep a record of its runs and its versions.").color(theme::weak()),
            );
            return;
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut st.tab, Tab::Runs, "Runs");
            ui.selectable_value(&mut st.tab, Tab::Versions, "Versions")
                .on_hover_text("the named versions of the model (File, Save a version...)");
        });
        if st.tab == Tab::Versions {
            crate::versions_view::pane(ui, &mut st.versions, s);
            return;
        }
        if st.entries.is_empty() {
            ui.label(
                RichText::new(
                    "No run yet. Each run you ask for is kept here, with its values, its outputs and the model as it \
                     ran, in the .terraflux folder beside the model's file.",
                )
                .color(theme::weak()),
            );
            return;
        }
        ui.request_repaint_after(LOOK);
        let room = ui.available_height();
        let picked = !st.selected.is_empty();
        table(ui, st, if picked { (room * 0.45).max(120.0) } else { room });
        if picked {
            ui.separator();
            detail(ui, st, s, r);
        }
    });
}

fn table(ui: &mut Ui, st: &mut State, height: f32) {
    let (error, warn, done, weak) = (
        ui.visuals().error_fg_color,
        ui.visuals().warn_fg_color,
        egui::Color32::from_rgb(120, 220, 110),
        theme::weak(),
    );
    let add = ui.input(|i| i.modifiers.command);
    let mut picked = None;
    egui::ScrollArea::both().id_salt("runs pane").max_height(height).auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("runs grid").num_columns(9).spacing([14.0, 5.0]).striped(true).show(ui, |ui| {
            for h in ["When", "Took", "Status", "What", "Scenario", "Values changed", "Outputs", "Model", ""] {
                ui.label(RichText::new(h).strong().color(weak));
            }
            ui.end_row();
            for (i, e) in st.entries.iter().enumerate().rev() {
                let before = i.checked_sub(1).map(|p| &st.entries[p]);
                let when = crate::eta::stamp(e.when).unwrap_or_else(|| e.when.to_string());
                let on = st.selected.contains(&e.id);
                let r = ui.selectable_label(on, when);
                if r.clicked() {
                    picked = Some(e.id);
                }
                r.on_hover_text(format!(
                    "terraflux {} from the {}; click to select, ctrl+click to select two",
                    e.terraflux, e.from
                ));
                ui.label(crate::eta::took(e.seconds));
                let (word, colour, why) = match &e.status {
                    Status::Done => ("done", done, String::new()),
                    Status::Stopped => ("stopped", warn, "stopped before its end".to_string()),
                    Status::Failed(m) => ("failed", error, m.clone()),
                };
                let r = ui.label(RichText::new(word).color(colour));
                if !why.is_empty() {
                    r.on_hover_text(why);
                } else if !e.warnings.is_empty() {
                    r.on_hover_text(format!("{} warnings:\n{}", e.warnings.len(), e.warnings.join("\n")));
                }
                ui.label(kind_text(&e.kind)).on_hover_text(kind_hover(&e.kind));
                match &e.scenario {
                    Some(sc) => ui.label(sc),
                    None => ui.label(RichText::new("-").color(weak)),
                };
                match before {
                    None => {
                        ui.label(RichText::new("-").color(weak));
                    }
                    Some(b) => {
                        let all = changes(&b.values, &e.values);
                        let mut text = all.iter().take(SHOWN).cloned().collect::<Vec<_>>().join(", ");
                        if all.len() > SHOWN {
                            text.push_str(&format!(", +{} more", all.len() - SHOWN));
                        }
                        if all.is_empty() {
                            ui.label(RichText::new("none").color(weak));
                        } else {
                            ui.label(text).on_hover_text(all.join("\n"));
                        }
                    }
                }
                let over = st.over.get(&e.id);
                if e.outputs.is_empty() {
                    ui.label(RichText::new("-").color(weak));
                } else {
                    let names: Vec<String> = e.outputs.iter().map(|w| w.name.clone()).collect();
                    let mut hover = names.join("\n");
                    let mut text = format!("{}", e.outputs.len());
                    if let Some(o) = over {
                        hover.push_str(&format!("\n\noverwritten since: {}", o.join(", ")));
                        text.push_str(" (overwritten since)");
                    }
                    let l = RichText::new(text).color(if over.is_some() { warn } else { ui.visuals().text_color() });
                    ui.label(l).on_hover_text(hover);
                }
                match before {
                    Some(b) if b.model != e.model => {
                        ui.label(RichText::new("model changed").color(warn))
                            .on_hover_text("the model was not the one the run before ran");
                    }
                    _ => {
                        ui.label("");
                    }
                }
                match folder_of(e) {
                    Some(f) => {
                        if ui.small_button("Open folder").on_hover_text(f.display().to_string()).clicked() {
                            crate::batch_view::open_folder(&f);
                        }
                    }
                    None => {
                        ui.label("");
                    }
                }
                ui.end_row();
            }
        });
    });
    if let Some(id) = picked {
        st.select(id, add);
    }
}

/// A value in full (a text as it is, others as JSON).
fn full(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        v => v.to_string(),
    }
}

/// The values of two runs side by side: key, the older's value, the newer's (none: the run had no such value). Only
/// the keys whose values differ, unless `all`.
pub fn value_rows(
    older: &BTreeMap<String, Json>,
    newer: &BTreeMap<String, Json>,
    all: bool,
) -> Vec<(String, Option<String>, Option<String>)> {
    let mut keys: Vec<&String> = older.keys().chain(newer.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|k| all || older.get(*k) != newer.get(*k))
        .map(|k| (k.clone(), older.get(k).map(full), newer.get(k).map(full)))
        .collect()
}

/// When a run began, as the table tells it.
fn when_of(e: &Entry) -> String {
    crate::eta::stamp(e.when).unwrap_or_else(|| e.when.to_string())
}

fn size_text(n: u64) -> String {
    match n {
        n if n >= 1 << 30 => format!("{:.1} GB", n as f64 / (1u64 << 30) as f64),
        n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
        n if n >= 1 << 10 => format!("{:.0} KB", n as f64 / 1024.0),
        n => format!("{n} B"),
    }
}

/// What became of an output file: as written, written over since, or gone.
pub fn state_of(w: &Written) -> &'static str {
    if w.unchanged() {
        "as written"
    } else if Path::new(&w.path).exists() {
        "overwritten since"
    } else {
        "missing"
    }
}

/// Whether a file is one the Viewer reads as a layer (a map, features, a table), or a folder of them.
fn viewable(p: &Path) -> bool {
    p.is_dir() || crate::builder::is_raster(p) || crate::builder::is_vector(p) || crate::builder::is_table(p)
}

/// The outputs of a run that can be opened in the Viewer (as written, of a kind it reads), and how many were left
/// out: written over since, missing, not of a kind it shows.
pub struct Openable {
    pub paths: Vec<PathBuf>,
    pub over: usize,
    pub missing: usize,
    pub other: usize,
}

pub fn openable(e: &Entry) -> Openable {
    let mut o = Openable { paths: Vec::new(), over: 0, missing: 0, other: 0 };
    for w in &e.outputs {
        let p = PathBuf::from(&w.path);
        match state_of(w) {
            "overwritten since" => o.over += 1,
            "missing" => o.missing += 1,
            _ if viewable(&p) => o.paths.push(p),
            _ => o.other += 1,
        }
    }
    o
}

impl Openable {
    /// What was left out and why ("1 overwritten since, 1 missing"); empty when nothing was.
    pub fn left_out(&self) -> String {
        let mut parts = Vec::new();
        if self.over > 0 {
            parts.push(format!("{} overwritten since", self.over));
        }
        if self.missing > 0 {
            parts.push(format!("{} missing", self.missing));
        }
        if self.other > 0 {
            parts.push(format!("{} not a map, features or table", self.other));
        }
        parts.join(", ")
    }
}

/// The maps two runs wrote under the same name, both files as written and rasters: (name, older's, newer's).
pub fn common_maps(older: &Entry, newer: &Entry) -> Vec<(String, PathBuf, PathBuf)> {
    older
        .outputs
        .iter()
        .filter_map(|a| {
            let b = newer.outputs.iter().find(|b| b.name == a.name)?;
            let (pa, pb) = (PathBuf::from(&a.path), PathBuf::from(&b.path));
            let maps = crate::builder::is_raster(&pa) && crate::builder::is_raster(&pb);
            (maps && pa != pb && a.unchanged() && b.unchanged()).then(|| (a.name.clone(), pa, pb))
        })
        .collect()
}

/// The model of a run to put in place of the open one, or why it cannot be.
fn restorable(s: &Session, journal: Option<&Journal>, e: &Entry) -> Result<Model, String> {
    let m = journal
        .and_then(|j| j.model(&e.model))
        .ok_or_else(|| "the model of this run was not kept (its snapshot is gone)".to_string())?;
    if m == s.source {
        return Err("the model open is already the one of this run".into());
    }
    Ok(m)
}

/// The model of a run put in place of the open one: one edit, one step of undo.
pub fn restore(s: &mut Session, m: Model, e: &Entry) {
    restore_named(s, m, &format!("Restore the model of the run of {}", when_of(e)));
}

/// A model put in place of the open one: one edit, one step of undo, with this name.
pub fn restore_named(s: &mut Session, m: Model, name: &str) {
    let inputs = m.inputs != s.source.inputs;
    s.source = m;
    crate::builder::forget_missing(s);
    s.history.name(name);
    s.edited();
    if inputs {
        s.read_data();
    }
}

/// The outputs of a run opened as layers of the Viewer; what was left out is told.
pub fn show_outputs(s: &mut Session, r: &Registry, e: &Entry) {
    let o = openable(e);
    let left = o.left_out();
    let text = match (o.paths.len(), left.is_empty()) {
        (0, _) => format!("no output of this run can be opened ({left})"),
        (n, true) => format!("{n} output(s) opened in the Viewer"),
        (n, false) => format!("{n} output(s) opened in the Viewer; left out: {left}"),
    };
    if !o.paths.is_empty() {
        crate::builder::drop_on_viewer(s, r, &o.paths);
    }
    s.notice = Some((text, std::time::Instant::now()));
}

/// Two maps opened in the Viewer's comparison once both have run.
pub fn compare_maps(s: &mut Session, r: &Registry, older: &Path, newer: &Path, how: How) {
    let ids = crate::builder::read_to_show(s, r, &[older.to_path_buf(), newer.to_path_buf()]);
    if let [a, b] = ids.as_slice() {
        s.compare_asked = Some((a.clone(), b.clone(), how));
        s.ask_viewer = true;
    }
}

/// Under the table: the run selected in full, or the two compared.
fn detail(ui: &mut Ui, st: &mut State, s: &mut Session, r: &Registry) {
    let mut picked: Vec<Entry> =
        st.selected.iter().filter_map(|id| st.entries.iter().find(|e| e.id == *id).cloned()).collect();
    picked.sort_by_key(|e| e.id);
    let journal = s.path.as_deref().and_then(Journal::of);
    // (the newest version that saved the model the run ran)
    let version = picked.first().filter(|_| picked.len() == 1).and_then(|e| {
        st.versions.list.iter().rev().find(|row| row.version.model == e.model).map(|row| row.version.label.clone())
    });
    egui::ScrollArea::vertical().id_salt("runs detail").auto_shrink([false, false]).show(ui, |ui| {
        match picked.as_slice() {
            [e] => one(ui, s, r, journal.as_ref(), e, version.as_deref()),
            [a, b] => two(ui, s, r, journal.as_ref(), &mut st.all_values, a, b),
            _ => {}
        }
    });
}

fn one(ui: &mut Ui, s: &mut Session, r: &Registry, journal: Option<&Journal>, e: &Entry, version: Option<&str>) {
    let (error, warn, weak) = (ui.visuals().error_fg_color, ui.visuals().warn_fg_color, theme::weak());
    ui.label(RichText::new(format!("Run of {}", when_of(e))).strong());
    let how = match &e.status {
        Status::Done => "done",
        Status::Stopped => "stopped before its end",
        Status::Failed(_) => "failed",
    };
    let from = match e.from.as_str() {
        "app" => "application",
        "cli" => "command line (terraflux run)",
        "mcp" => "MCP (an assistant)",
        other => other,
    };
    ui.label(format!(
        "{}, took {}, {how}. Terraflux {}, from the {from}.",
        kind_text(&e.kind),
        crate::eta::took(e.seconds),
        e.terraflux
    ));
    if let Some(sc) = &e.scenario {
        ui.label(format!("Scenario: {sc}"));
    }
    if let Some(label) = version {
        ui.label(format!("The model of version {label}"));
    }
    if let Status::Failed(m) = &e.status {
        ui.label(RichText::new(m).color(error));
    }
    if let Some(m) = &e.manifest {
        ui.label(RichText::new(format!("Manifest: {}", m.display())).small().color(weak));
    }
    ui.add_space(4.0);
    // the actions
    let open = openable(e);
    let restore_to = restorable(s, journal, e);
    ui.horizontal_wrapped(|ui| {
        let text = if open.paths.is_empty() {
            "no output can be opened".to_string()
        } else {
            format!("opens {} of {}", open.paths.len(), e.outputs.len())
        };
        let shown = ui
            .add_enabled(!open.paths.is_empty(), egui::Button::new("Show its outputs"))
            .on_hover_text("the outputs still as written, as layers of the Viewer")
            .on_disabled_hover_text(format!("{text} ({})", open.left_out()))
            .clicked();
        if shown {
            show_outputs(s, r, e);
        }
        ui.label(RichText::new(text).small().color(weak));
        if !open.left_out().is_empty() {
            ui.label(RichText::new(format!("left out: {}", open.left_out())).small().color(warn));
        }
    });
    ui.horizontal_wrapped(|ui| {
        let b = ui
            .add_enabled(restore_to.is_ok(), egui::Button::new("Restore this model"))
            .on_hover_text("the model as it ran, in place of the one open (ctrl+Z goes back)");
        let b = match &restore_to {
            Err(why) => b.on_disabled_hover_text(why.clone()),
            Ok(_) => b,
        };
        if b.clicked()
            && let Ok(m) = restore_to.clone()
        {
            restore(s, m, e);
        }
        if let Err(why) = &restore_to {
            ui.label(RichText::new(why).small().color(weak));
        }
    });
    if let Kind::Batch(b) = &e.kind {
        ui.add_space(4.0);
        ui.label(RichText::new("Batch").strong());
        ui.label(format!("{}; results in {}", kind_text(&e.kind), b.out.display()));
        for (k, f) in b.factors.iter().enumerate() {
            let index = f.index.map_or(String::new(), |i| format!("{} {i:.3}", f.by));
            ui.label(format!("{}. {}  {index}", k + 1, f.key));
        }
        if ui.small_button("Open the folder").on_hover_text(b.out.display().to_string()).clicked() {
            crate::batch_view::open_folder(&b.out);
        }
    }
    ui.add_space(4.0);
    egui::CollapsingHeader::new(format!("Values ({})", e.values.len()))
        .id_salt(("run values", e.id))
        .default_open(true)
        .show(ui, |ui| {
            if e.values.is_empty() {
                ui.label(RichText::new("no value").color(weak));
                return;
            }
            egui::Grid::new("run values grid").num_columns(2).spacing([14.0, 3.0]).striped(true).show(ui, |ui| {
                for (k, v) in &e.values {
                    ui.label(k);
                    ui.label(full(v));
                    ui.end_row();
                }
            });
        });
    egui::CollapsingHeader::new(format!("Outputs ({})", e.outputs.len()))
        .id_salt(("run outputs", e.id))
        .default_open(true)
        .show(ui, |ui| {
            if e.outputs.is_empty() {
                ui.label(RichText::new("none written").color(weak));
                return;
            }
            egui::Grid::new("run outputs grid").num_columns(4).spacing([14.0, 3.0]).striped(true).show(ui, |ui| {
                for w in &e.outputs {
                    let state = state_of(w);
                    ui.label(&w.name);
                    ui.label(RichText::new(&w.path).small());
                    ui.label(size_text(w.size));
                    ui.label(RichText::new(state).color(if state == "as written" { weak } else { warn }));
                    ui.end_row();
                }
            });
        });
    if !e.warnings.is_empty() {
        egui::CollapsingHeader::new(format!("Warnings ({})", e.warnings.len())).id_salt(("run warnings", e.id)).show(
            ui,
            |ui| {
                for w in &e.warnings {
                    ui.label(RichText::new(w).small().color(warn));
                }
            },
        );
    }
}

fn two(
    ui: &mut Ui,
    s: &mut Session,
    r: &Registry,
    journal: Option<&Journal>,
    all: &mut bool,
    older: &Entry,
    newer: &Entry,
) {
    let weak = theme::weak();
    ui.label(RichText::new(format!("Compare the runs of {} and {}", when_of(older), when_of(newer))).strong());
    // the values side by side
    ui.checkbox(all, "All values").on_hover_text("every value, not only those that differ");
    let rows = value_rows(&older.values, &newer.values, *all);
    if rows.is_empty() {
        ui.label(RichText::new("The two runs ran with the same values.").color(weak));
    } else {
        egui::Grid::new("runs compared values").num_columns(3).spacing([14.0, 3.0]).striped(true).show(ui, |ui| {
            ui.label(RichText::new("Value").strong().color(weak));
            ui.label(RichText::new(when_of(older)).strong().color(weak));
            ui.label(RichText::new(when_of(newer)).strong().color(weak));
            ui.end_row();
            for (k, a, b) in &rows {
                let differ = a != b;
                ui.label(k);
                let say = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".into());
                let paint = |t: String| {
                    if differ { RichText::new(t).color(crate::review::CHANGED) } else { RichText::new(t) }
                };
                ui.label(paint(say(a)));
                ui.label(paint(say(b)));
                ui.end_row();
            }
        });
    }
    ui.add_space(6.0);
    // the models, through the Review pane
    let (before, after) = (journal.and_then(|j| j.model(&older.model)), journal.and_then(|j| j.model(&newer.model)));
    ui.horizontal_wrapped(|ui| {
        let both = match (&before, &after) {
            (Some(a), Some(b)) if a == b => Err("the two runs ran the same model".to_string()),
            (Some(a), Some(b)) => Ok((a.clone(), b.clone())),
            _ => Err("a model of the two runs was not kept".to_string()),
        };
        let b = ui
            .add_enabled(both.is_ok(), egui::Button::new("Review the model's differences"))
            .on_hover_text("what changed in the model from the older run to the newer, in the Changes pane");
        let b = match &both {
            Err(why) => b.on_disabled_hover_text(why.clone()),
            Ok(_) => b,
        };
        if b.clicked()
            && let Ok((a, n)) = both
        {
            let what = format!("the model of the run of {} against the run of {}", when_of(newer), when_of(older));
            crate::review::compare_two(s, a, n, &what);
        }
        let now = match &after {
            Some(n) if *n == s.source => Err("the model open is the one of the newer run".to_string()),
            Some(n) => Ok(n.clone()),
            None => Err("the model of the newer run was not kept".to_string()),
        };
        let b = ui
            .add_enabled(now.is_ok(), egui::Button::new("Newer against the model open"))
            .on_hover_text("the changes from the newer run's model to the one open, to accept or reject");
        let b = match &now {
            Err(why) => b.on_disabled_hover_text(why.clone()),
            Ok(_) => b,
        };
        if b.clicked()
            && let Ok(n) = now
        {
            crate::review::compare(s, n, &format!("the changes since the run of {}", when_of(newer)));
        }
    });
    // the maps both runs wrote
    let maps = common_maps(older, newer);
    ui.horizontal_wrapped(|ui| match maps.first() {
        None => {
            ui.add_enabled(false, egui::Button::new("Compare the maps"))
                .on_disabled_hover_text("no map both runs wrote, still as written, in different files");
        }
        Some((name, a, b)) => {
            if ui.button(format!("Compare the maps ({name})")).on_hover_text("side by side in the Viewer").clicked() {
                compare_maps(s, r, a, b, How::SideBySide);
            }
            if ui.button("Their difference").on_hover_text("the newer minus the older, in the Viewer").clicked() {
                compare_maps(s, r, a, b, How::Difference);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use terraflux_exec::journal::{Batch, Written};

    fn values(v: &[(&str, Json)]) -> BTreeMap<String, Json> {
        v.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn what_changed_between_two_runs_is_told() {
        let a = values(&[("patches.isometry", json!(1.5)), ("$rate", json!(2)), ("old", json!("x"))]);
        let b = values(&[("patches.isometry", json!(2)), ("$rate", json!(2)), ("new", json!("y"))]);
        assert_eq!(changes(&a, &b), vec!["new = y", "patches.isometry 1.5 -> 2", "old removed"]);
        assert!(changes(&a, &a).is_empty());
        assert_eq!(show(&json!("a long text that goes on and on")), "a long text that goes ...");
    }

    #[test]
    fn a_run_is_told_in_a_few_words() {
        assert_eq!(kind_text(&Kind::Run), "Run");
        assert_eq!(kind_text(&Kind::To("allocate".into())), "Run to allocate");
        let b =
            Batch { kind: "sensitivity".into(), runs: 5, failed: 1, method: Some("oat".into()), ..Default::default() };
        assert_eq!(kind_text(&Kind::Batch(b)), "Sensitivity (oat), 5 runs, 1 failed");
    }

    #[test]
    fn one_or_two_runs_are_selected() {
        let mut st = State::default();
        st.select(1, false);
        st.select(2, true);
        assert_eq!(st.selected, vec![1, 2]);
        st.select(3, true);
        assert_eq!(st.selected, vec![2, 3]);
        st.select(2, true);
        assert_eq!(st.selected, vec![3]);
        // (a click alone: that run only; again on it: none)
        st.select(5, false);
        assert_eq!(st.selected, vec![5]);
        st.select(5, false);
        assert!(st.selected.is_empty());
    }

    #[test]
    fn the_values_of_two_runs_are_put_side_by_side() {
        let a = values(&[("patches.isometry", json!(1.5)), ("$rate", json!(2)), ("old", json!("x"))]);
        let b = values(&[("patches.isometry", json!(2)), ("$rate", json!(2)), ("new", json!("y"))]);
        let some = |t: &str| Some(t.to_string());
        assert_eq!(
            value_rows(&a, &b, false),
            vec![
                ("new".to_string(), None, some("y")),
                ("old".to_string(), some("x"), None),
                ("patches.isometry".to_string(), some("1.5"), some("2")),
            ]
        );
        let all = value_rows(&a, &b, true);
        assert_eq!(all.len(), 4);
        assert_eq!(all[0], ("$rate".to_string(), some("2"), some("2")));
        assert!(value_rows(&a, &a, false).is_empty());
    }

    #[test]
    fn the_outputs_to_open_and_the_maps_to_compare_are_those_still_as_written() {
        let d = std::env::temp_dir().join(format!("terraflux-runs-open-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let file = |n: &str| {
            let p = d.join(n);
            std::fs::write(&p, "map").unwrap();
            p
        };
        let (a1, a2, b1, notes) = (file("a1.tif"), file("a2.tif"), file("b1.tif"), file("notes.txt"));
        let wrote = |name: &str, p: &Path| Written::of(name, p, None, None).unwrap();
        let mut older = Entry::new(Kind::Run, "app", 1.0);
        older.outputs = vec![wrote("land", &a1), wrote("risk", &a2), wrote("notes", &notes)];
        let mut newer = Entry::new(Kind::Run, "app", 1.0);
        newer.outputs = vec![wrote("land", &b1), wrote("risk", &a2)];
        let o = openable(&older);
        assert_eq!(o.paths, vec![a1.clone(), a2.clone()]);
        assert_eq!((o.over, o.missing, o.other), (0, 0, 1));
        // (the same file for both runs is not two maps)
        assert_eq!(common_maps(&older, &newer), vec![("land".to_string(), a1.clone(), b1.clone())]);
        // an output written over, one gone
        std::fs::write(&a1, "another map").unwrap();
        std::fs::remove_file(&a2).unwrap();
        let o = openable(&older);
        assert!(o.paths.is_empty());
        assert_eq!(o.left_out(), "1 overwritten since, 1 missing, 1 not a map, features or table");
        assert!(common_maps(&older, &newer).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_journal_is_read_again_when_it_grows() {
        let d = std::env::temp_dir().join(format!("terraflux-runs-view-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let file = d.join("m.toml");
        let model = terraflux_graph::Model::from_toml("format = \"terraflux/1\"").unwrap();
        let j = Journal::of(&file).unwrap();
        let out = d.join("land.tif");
        std::fs::write(&out, "one").unwrap();
        let mut e = Entry::new(Kind::Run, "app", 1.0);
        e.outputs = vec![Written::of("land", &out, None, None).unwrap()];
        j.record(e, &model).unwrap();
        let mut st = State::default();
        st.refresh(Some(&file), 0);
        assert_eq!(st.entries.len(), 1);
        assert!(st.over.is_empty());
        st.select(st.entries[0].id, false);
        // another program wrote to the journal, and an output was written over
        j.record(Entry::new(Kind::Run, "cli", 1.0), &model).unwrap();
        std::fs::write(&out, "another one").unwrap();
        st.refresh(Some(&file), 0);
        assert_eq!(st.entries.len(), 2);
        assert_eq!(st.over.values().next().unwrap(), &vec!["land".to_string()]);
        assert_eq!(st.selected.len(), 1);
        // a model without a file: nothing
        st.refresh(None, 0);
        assert!(st.entries.is_empty() && st.selected.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
