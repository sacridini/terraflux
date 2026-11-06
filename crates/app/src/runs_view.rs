//! The Runs pane: the journal of the model in front (`.terraflux` beside its file, see
//! [`terraflux_exec::journal`]), newest run first. Each row says when it ran, how long, how it ended, what it was
//! (the model, up to a node, or a batch of Many runs), the scenario, what its values changed from the run before,
//! the outputs it wrote and whether the model itself changed. One or two rows can be selected (click, ctrl+click).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Margin, RichText, Ui};
use serde_json::Value as Json;
use terraflux_exec::journal::{Entry, Journal, Kind, Status};

use crate::app::Session;
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
    /// What was read: the model's file, the count of runs the session and the batches wrote, the file's size.
    read: Option<(PathBuf, u64, u64)>,
    checked: Option<Instant>,
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
        let at = self.selected.iter().position(|s| *s == id);
        match (add, at) {
            (_, Some(k)) if add || self.selected.len() == 1 => {
                self.selected.remove(k);
            }
            (true, None) => {
                if self.selected.len() == 2 {
                    self.selected.remove(0);
                }
                self.selected.push(id);
            }
            _ => self.selected = vec![id],
        }
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
pub fn pane(ui: &mut Ui, st: &mut State, s: &Session, batches: u64) {
    st.refresh(s.path.as_deref(), s.journal_rev + batches);
    egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
        theme::section(ui, "Runs");
        if s.path.is_none() {
            ui.label(RichText::new("Save the model to keep a record of its runs.").color(theme::weak()));
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
        ui.ctx().request_repaint_after(LOOK);
        table(ui, st);
    });
}

fn table(ui: &mut Ui, st: &mut State) {
    let (error, warn, done, weak) = (
        ui.visuals().error_fg_color,
        ui.visuals().warn_fg_color,
        egui::Color32::from_rgb(120, 220, 110),
        theme::weak(),
    );
    let add = ui.input(|i| i.modifiers.command);
    let mut picked = None;
    egui::ScrollArea::both().id_salt("runs pane").auto_shrink([false, false]).show(ui, |ui| {
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
