//! The named versions of a model, without git: *Save a version...* keeps the model as it is on screen under a label
//! (and a note), beside the runs' journal (see [`terraflux_exec::journal`]). The Runs pane's Versions tab lists them,
//! the newest first, each with what changed from the version before; one is reviewed against the model open (to
//! accept or reject each change), restored (one step of undo) or deleted, and two are compared, to read.

use std::path::{Path, PathBuf};

use egui::{RichText, Ui};
use terraflux_exec::journal::{Journal, Version};
use terraflux_graph::Model;

use crate::app::Session;
use crate::review::{self, Kind, Target};
use crate::theme;

/// What changed from one model to another, in counts: nodes added, removed and changed, and the rest (inputs, outputs,
/// the model's record).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub other: usize,
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

impl Counts {
    /// The changes from `before` to `after`, counted from the review's diff.
    pub fn between(before: &Model, after: &Model) -> Counts {
        let mut c = Counts::default();
        for ch in review::diff(before, after) {
            match (&ch.target, ch.kind) {
                (Target::Node(_), Kind::Added) => c.added += 1,
                (Target::Node(_), Kind::Removed) => c.removed += 1,
                (Target::Node(_), Kind::Changed) => c.changed += 1,
                _ => c.other += 1,
            }
        }
        c
    }

    /// In words: "1 node changed, 2 nodes added"; "no change" when there is none.
    pub fn text(&self) -> String {
        let mut parts = Vec::new();
        if self.added > 0 {
            parts.push(format!("{} added", plural(self.added, "node", "nodes")));
        }
        if self.removed > 0 {
            parts.push(format!("{} removed", plural(self.removed, "node", "nodes")));
        }
        if self.changed > 0 {
            parts.push(format!("{} changed", plural(self.changed, "node", "nodes")));
        }
        if self.other > 0 {
            parts.push(plural(self.other, "other change", "other changes"));
        }
        if parts.is_empty() { "no change".into() } else { parts.join(", ") }
    }
}

/// A version in the list: what changed from the version before (none for the first, or when a snapshot is gone), and
/// whether its own snapshot is kept.
#[derive(Debug, Clone)]
pub struct Row {
    pub version: Version,
    pub changes: Option<Counts>,
    pub kept: bool,
}

/// The versions of a model's journal, the oldest first, with their changes counted.
pub fn rows(journal: &Journal) -> Vec<Row> {
    let versions = journal.versions();
    let models: Vec<Option<Model>> = versions.iter().map(|v| journal.model(&v.model)).collect();
    versions
        .into_iter()
        .enumerate()
        .map(|(i, version)| {
            let changes = match (i.checked_sub(1).and_then(|p| models[p].as_ref()), models[i].as_ref()) {
                (Some(b), Some(a)) => Some(Counts::between(b, a)),
                _ => None,
            };
            Row { version, changes, kept: models[i].is_some() }
        })
        .collect()
}

/// The pane's state: the versions read, the ones selected, a deletion being asked about.
#[derive(Default)]
pub struct State {
    /// The versions, the oldest first.
    pub list: Vec<Row>,
    /// The versions selected (one or two, by id).
    pub selected: Vec<u64>,
    /// The version a deletion is asked about.
    pub confirm: Option<u64>,
    /// What was read: the model's file, the count of versions the session wrote, the file's size.
    read: Option<(PathBuf, u64, u64)>,
}

impl State {
    /// Read the versions again when they changed (another model, a version saved here or by another program); not at
    /// every frame, and the changes counted only then.
    pub fn refresh(&mut self, model_file: Option<&Path>, rev: u64) {
        let Some(journal) = model_file.and_then(Journal::of) else {
            *self = State::default();
            return;
        };
        let file = model_file.unwrap_or(Path::new(""));
        let size = journal.versions_size();
        if self.read.as_ref().is_some_and(|(f, r, s)| f == file && *r == rev && *s == size) {
            return;
        }
        if self.read.as_ref().is_none_or(|(f, _, _)| f != file) {
            self.selected.clear();
            self.confirm = None;
        }
        self.list = rows(&journal);
        self.selected.retain(|id| self.list.iter().any(|r| r.version.id == *id));
        self.confirm = self.confirm.filter(|id| self.list.iter().any(|r| r.version.id == *id));
        self.read = Some((file.to_path_buf(), rev, size));
    }

    pub fn select(&mut self, id: u64, add: bool) {
        self.confirm = None;
        crate::runs_view::pick(&mut self.selected, id, add);
    }
}

/// The window that asks for a version's label and note.
#[derive(Debug, Default)]
pub struct Dialog {
    pub label: String,
    pub note: String,
}

/// Save the model as it is on screen as a version, from the application. The label saved.
pub fn save(s: &mut Session, label: &str, note: &str) -> Result<String, String> {
    let file = s.path.clone().ok_or("save the model first (File, Save): versions are kept beside its file")?;
    let journal = Journal::of(&file).ok_or("the model has no file name")?;
    let v = journal.save_version(label, Some(note), &s.source, "app").map_err(|e| e.to_string())?;
    s.journal_rev += 1;
    Ok(v.label)
}

/// The window of *Save a version...*, while the session has one open.
pub fn window(ctx: &egui::Context, s: &mut Session) {
    let Some(mut d) = s.version_dialog.take() else { return };
    let mut open = true;
    let mut done = false;
    egui::Window::new("Save a version").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        let weak = theme::weak();
        ui.set_min_width(380.0);
        if s.path.is_none() {
            ui.label("Save the model first (File, Save): its versions are kept beside its file.");
            return;
        }
        ui.label(
            RichText::new("The model as it is now, kept under a name: to review against, or to go back to, later.")
                .small()
                .color(weak),
        );
        if s.dirty {
            ui.label(
                RichText::new("The model has edits not saved: the version is of what is on screen.")
                    .small()
                    .color(ui.visuals().warn_fg_color),
            );
        }
        ui.add_space(4.0);
        ui.label("Label");
        let label = ui.add(
            egui::TextEdit::singleline(&mut d.label).hint_text("before changing the scenario").desired_width(360.0),
        );
        label.request_focus();
        ui.label("Note (optional)");
        ui.add(egui::TextEdit::multiline(&mut d.note).desired_rows(3).desired_width(360.0));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let ok = !d.label.trim().is_empty();
            let b = ui.add_enabled(ok, crate::app::accent_button("Save version", true));
            let b = b.on_disabled_hover_text("a version needs a label");
            if b.clicked() {
                match save(s, &d.label, &d.note) {
                    Ok(label) => s.notice = Some((format!("Version \"{label}\" saved"), std::time::Instant::now())),
                    Err(e) => s.notice = Some((e, std::time::Instant::now())),
                }
                done = true;
            }
            if ui.button("Cancel").clicked() {
                done = true;
            }
        });
    });
    if open && !done {
        s.version_dialog = Some(d);
    }
}

/// The Versions tab.
pub fn pane(ui: &mut Ui, st: &mut State, s: &mut Session) {
    let weak = theme::weak();
    if ui.button("Save a version...").on_hover_text("the model as it is now, kept under a name (ctrl+alt+S)").clicked()
    {
        s.version_dialog = Some(Dialog::default());
    }
    if st.list.is_empty() {
        ui.label(
            RichText::new(
                "No version yet. A version is the model kept under a name, to go back to it or see what changed \
                 since, in the .terraflux folder beside the model's file.",
            )
            .color(weak),
        );
        return;
    }
    ui.request_repaint_after(std::time::Duration::from_secs(2));
    let room = ui.available_height();
    let picked = !st.selected.is_empty();
    let height = if picked { (room * 0.45).max(120.0) } else { room };
    let add = ui.input(|i| i.modifiers.command);
    let mut clicked = None;
    egui::ScrollArea::both().id_salt("versions pane").max_height(height).auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("versions grid").num_columns(4).spacing([14.0, 5.0]).striped(true).show(ui, |ui| {
            for h in ["Version", "Saved", "Changed from the version before", "Note"] {
                ui.label(RichText::new(h).strong().color(weak));
            }
            ui.end_row();
            for row in st.list.iter().rev() {
                let v = &row.version;
                let on = st.selected.contains(&v.id);
                let r = ui.selectable_label(on, &v.label);
                if r.clicked() {
                    clicked = Some(v.id);
                }
                r.on_hover_text(format!("from the {}; click to select, ctrl+click to select two", v.from));
                ui.label(crate::eta::stamp(v.when).unwrap_or_else(|| v.when.to_string()));
                match row.changes {
                    Some(c) => ui.label(c.text()),
                    None if !row.kept => ui.label(RichText::new("snapshot gone").color(ui.visuals().warn_fg_color)),
                    None => ui.label(RichText::new("-").color(weak)),
                };
                match &v.note {
                    Some(n) => ui.label(n.lines().next().unwrap_or_default()).on_hover_text(n.as_str()),
                    None => ui.label(RichText::new("-").color(weak)),
                };
                ui.end_row();
            }
        });
    });
    if let Some(id) = clicked {
        st.select(id, add);
    }
    if picked {
        ui.separator();
        detail(ui, st, s);
    }
}

/// When a version was saved, as the table tells it.
fn when_of(v: &Version) -> String {
    crate::eta::stamp(v.when).unwrap_or_else(|| v.when.to_string())
}

/// Under the table: the version selected with its actions, or the two compared.
fn detail(ui: &mut Ui, st: &mut State, s: &mut Session) {
    let mut picked: Vec<Version> = st
        .selected
        .iter()
        .filter_map(|id| st.list.iter().find(|r| r.version.id == *id).map(|r| r.version.clone()))
        .collect();
    picked.sort_by_key(|v| v.id);
    let journal = s.path.as_deref().and_then(Journal::of);
    let Some(journal) = journal else { return };
    egui::ScrollArea::vertical().id_salt("versions detail").auto_shrink([false, false]).show(ui, |ui| {
        match picked.as_slice() {
            [v] => one(ui, st, s, &journal, v),
            [a, b] => two(ui, s, &journal, a, b),
            _ => {}
        }
    });
}

fn one(ui: &mut Ui, st: &mut State, s: &mut Session, journal: &Journal, v: &Version) {
    let weak = theme::weak();
    ui.label(RichText::new(format!("Version \"{}\"", v.label)).strong());
    let from = match v.from.as_str() {
        "app" => "application",
        "cli" => "command line",
        "mcp" => "MCP (an assistant)",
        other => other,
    };
    ui.label(format!("Saved {}, from the {from}.", when_of(v)));
    if let Some(n) = &v.note {
        ui.label(n);
    }
    ui.add_space(4.0);
    let model = journal.model(&v.model);
    let open = match &model {
        None => Err("the model of this version was not kept (its snapshot is gone)".to_string()),
        Some(m) if *m == s.source => Err("the model open is already this version".to_string()),
        Some(m) => Ok(m.clone()),
    };
    ui.horizontal_wrapped(|ui| {
        let b = ui.add_enabled(open.is_ok(), egui::Button::new("Review against the model open")).on_hover_text(
            "what changed from this version to the model open, in the Changes pane: accept or reject each",
        );
        let b = match &open {
            Err(why) => b.on_disabled_hover_text(why.clone()),
            Ok(_) => b,
        };
        if b.clicked()
            && let Ok(m) = open.clone()
        {
            review::compare(s, m, &format!("the changes since the version \"{}\"", v.label));
        }
        let b = ui
            .add_enabled(open.is_ok(), egui::Button::new("Restore this version"))
            .on_hover_text("this version in place of the model open (ctrl+Z goes back)");
        let b = match &open {
            Err(why) => b.on_disabled_hover_text(why.clone()),
            Ok(_) => b,
        };
        if b.clicked()
            && let Ok(m) = open.clone()
        {
            crate::runs_view::restore_named(s, m, &format!("Restore the version {}", v.label));
        }
        if ui
            .button("Delete...")
            .on_hover_text("forget this version (the model open and the runs are not touched)")
            .clicked()
        {
            st.confirm = Some(v.id);
        }
    });
    if let Err(why) = &open {
        ui.label(RichText::new(why).small().color(weak));
    }
    if st.confirm == Some(v.id) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Delete the version \"{}\"?", v.label));
            if ui.button("Delete the version").clicked() {
                match journal.delete_version(v.id) {
                    Ok(()) => {
                        s.journal_rev += 1;
                        st.selected.retain(|id| *id != v.id);
                    }
                    Err(e) => s.notice = Some((e.to_string(), std::time::Instant::now())),
                }
                st.confirm = None;
            }
            if ui.button("Keep it").clicked() {
                st.confirm = None;
            }
        });
    }
}

fn two(ui: &mut Ui, s: &mut Session, journal: &Journal, older: &Version, newer: &Version) {
    let weak = theme::weak();
    ui.label(RichText::new(format!("Compare \"{}\" and \"{}\"", older.label, newer.label)).strong());
    let both = match (journal.model(&older.model), journal.model(&newer.model)) {
        (Some(a), Some(b)) if a == b => Err("the two versions are the same model".to_string()),
        (Some(a), Some(b)) => Ok((a, b)),
        _ => Err("a model of the two versions was not kept".to_string()),
    };
    let b = ui
        .add_enabled(both.is_ok(), egui::Button::new("Review the differences"))
        .on_hover_text("what changed from the older version to the newer, in the Changes pane (to read)");
    let b = match &both {
        Err(why) => b.on_disabled_hover_text(why.clone()),
        Ok(_) => b,
    };
    if b.clicked()
        && let Ok((a, n)) = both.clone()
    {
        review::compare_two(
            s,
            a,
            n,
            &format!("the version \"{}\" against the version \"{}\"", newer.label, older.label),
        );
    }
    if let Err(why) = &both {
        ui.label(RichText::new(why).small().color(weak));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(rows: i64) -> Model {
        let text = format!(
            "format = \"terraflux/1\"\n[[nodes]]\nid = \"land\"\nop = \"nlm.random\"\n\
             params = {{ rows = {rows}, columns = 8, seed = 4 }}\n"
        );
        Model::from_toml(&text).unwrap()
    }

    #[test]
    fn the_changes_between_versions_are_counted_and_told() {
        let mut more = model(12);
        more.nodes.push(terraflux_graph::Node::new("high", "raster.calc"));
        assert_eq!(Counts::between(&model(10), &model(10)).text(), "no change");
        assert_eq!(Counts::between(&model(10), &model(12)).text(), "1 node changed");
        assert_eq!(Counts::between(&model(10), &more).text(), "1 node added, 1 node changed");
        assert_eq!(Counts::between(&more, &model(10)).text(), "1 node removed, 1 node changed");
        let mut renamed = model(10);
        renamed.name = Some("another".into());
        assert_eq!(Counts::between(&model(10), &renamed).text(), "1 other change");
    }

    #[test]
    fn the_versions_are_read_with_their_changes_and_read_again_when_the_file_grows() {
        let d = std::env::temp_dir().join(format!("terraflux-versions-view-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let file = d.join("m.toml");
        let j = Journal::of(&file).unwrap();
        j.save_version("one", None, &model(10), "app").unwrap();
        let mut st = State::default();
        st.refresh(Some(&file), 0);
        assert_eq!(st.list.len(), 1);
        assert_eq!(st.list[0].changes, None);
        j.save_version("two", None, &model(12), "cli").unwrap();
        st.select(st.list[0].version.id, false);
        st.refresh(Some(&file), 0);
        assert_eq!(st.list.len(), 2);
        assert_eq!(st.list[1].changes.unwrap().text(), "1 node changed");
        assert_eq!(st.selected.len(), 1);
        st.refresh(None, 0);
        assert!(st.list.is_empty() && st.selected.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
