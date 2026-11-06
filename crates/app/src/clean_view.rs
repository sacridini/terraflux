//! *Clean outputs...*: the files and folders the model's runs wrote, from its journal (see
//! [`terraflux_exec::journal`]), listed with their sizes to delete the ones chosen. Every run's outputs (a path written
//! by several runs is one row, marked when the newest run wrote it) and the folders of the batches. By default the
//! batch folders and the outputs the newest run did not write are chosen, never the newest run's. The deletion is
//! permanent (there is no recycle bin) and asked about first; only paths the journal lists are touched, and never the
//! model's own folder, a drive's root or the user's home. The journal keeps its entries: what was deleted shows as
//! missing in the Runs pane. The copies of the outputs the journal keeps with the runs are a choice of their own,
//! off by default, with their size.

use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use egui::{RichText, Ui};
use terraflux_exec::journal::{Entry, Journal, Kind};

use crate::app::Session;
use crate::theme;

/// A file or folder a run wrote.
#[derive(Debug, Clone)]
pub struct Item {
    pub path: PathBuf,
    /// A batch's results folder, not a run's output.
    pub batch: bool,
    /// The newest run wrote it.
    pub latest: bool,
    /// A check run's file (never the newest run's: the real run is).
    pub check: bool,
    /// When the newest run that wrote it began (seconds since 1970).
    pub when: u64,
    /// Whether it is there now.
    pub exists: bool,
    /// Its bytes (a folder's summed); none while they are being counted.
    pub size: Option<u64>,
    /// Why it is not to be deleted (the model's folder, a drive, the home folder...), when it is not.
    pub kept: Option<&'static str>,
    pub checked: bool,
}

/// The files and folders of a journal's runs, the newest first: each path once, from the entries (the oldest first).
/// Not yet checked for existing, sized or chosen.
pub fn items(entries: &[Entry]) -> Vec<Item> {
    let whole = |e: &&Entry| !matches!(e.kind, Kind::Batch(_) | Kind::Check(_)) && !e.outputs.is_empty();
    let newest = entries.iter().rev().find(whole).map(|e| e.id);
    let mut rows: Vec<(Item, u64)> = Vec::new();
    for e in entries {
        let paths: Vec<(PathBuf, bool)> = match &e.kind {
            Kind::Batch(b) if !b.out.as_os_str().is_empty() => vec![(b.out.clone(), true)],
            Kind::Batch(_) => Vec::new(),
            _ => e.outputs.iter().map(|w| (PathBuf::from(&w.path), false)).collect(),
        };
        for (path, batch) in paths {
            let latest = !batch && Some(e.id) == newest;
            let check = matches!(e.kind, Kind::Check(_));
            match rows.iter_mut().find(|(i, _)| i.path == path) {
                Some((i, id)) => {
                    i.when = e.when;
                    i.latest = latest;
                    i.check = check;
                    i.batch |= batch;
                    *id = e.id;
                }
                None => rows.push((
                    Item {
                        path,
                        batch,
                        latest,
                        check,
                        when: e.when,
                        exists: false,
                        size: None,
                        kept: None,
                        checked: false,
                    },
                    e.id,
                )),
            }
        }
    }
    let mut out: Vec<Item> = rows.into_iter().map(|(i, _)| i).collect();
    out.sort_by_key(|i| std::cmp::Reverse(i.when));
    out
}

/// Why a path must not be deleted from a model's runs, when it must not: not a full path, a drive's root, the home
/// folder (or a folder above it), the model's folder (or one above it), the model's file, the journal's folder.
pub fn refuse(path: &Path, model_file: &Path) -> Option<&'static str> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return Some("not a full path");
    }
    if path.parent().is_none() || path.components().filter(|c| matches!(c, Component::Normal(_))).count() == 0 {
        return Some("a drive's root");
    }
    let abs = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let same = |a: &Path, b: &Path| a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase();
    // (a path above the folder, or the folder itself)
    let covers = |p: &Path, q: &Path| q.ancestors().any(|a| same(a, p));
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    if home.is_some_and(|h| covers(path, &abs(&h))) {
        return Some("the home folder");
    }
    let file = abs(model_file);
    let folder = file.parent().map(Path::to_path_buf).unwrap_or_default();
    if covers(path, &folder) {
        return Some("the model's folder");
    }
    if same(path, &file) {
        return Some("the model's file");
    }
    if covers(&folder.join(terraflux_exec::journal::FOLDER), path) {
        return Some("the journal's folder");
    }
    None
}

/// The bytes of a file, or of a folder with everything in it (links are not followed).
pub fn bytes_of(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if !meta.is_dir() {
        return meta.len();
    }
    let Ok(read) = std::fs::read_dir(path) else { return 0 };
    read.filter_map(|e| e.ok()).map(|e| bytes_of(&e.path())).sum()
}

/// Delete the files and folders, each once more checked against [`refuse`]. How many went, how many bytes, and what
/// failed (path and why).
pub fn delete(paths: &[PathBuf], model_file: &Path) -> (usize, u64, Vec<(PathBuf, String)>) {
    let (mut gone, mut bytes, mut failed) = (0, 0, Vec::new());
    for p in paths {
        if let Some(why) = refuse(p, model_file) {
            failed.push((p.clone(), format!("kept: {why}")));
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(p) else { continue };
        let size = bytes_of(p);
        let done = if meta.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) };
        match done {
            Ok(()) => {
                gone += 1;
                bytes += size;
            }
            Err(e) => failed.push((p.clone(), e.to_string())),
        }
    }
    (gone, bytes, failed)
}

/// The window's state.
pub struct State {
    pub model_file: PathBuf,
    pub items: Vec<Item>,
    /// The deletion is being asked about.
    pub confirm: bool,
    /// What the last deletion did.
    pub said: Option<String>,
    /// The bytes of the copies of outputs the journal keeps (counted when the window opens and after a deletion).
    pub kept_bytes: u64,
    /// The copies are to be deleted too.
    pub clear_kept: bool,
    sizes: Option<Receiver<(usize, u64)>>,
}

impl State {
    /// The window for a model's runs: the journal read, the sizes counted in a thread.
    pub fn open(model_file: &Path) -> State {
        let journal = Journal::of(model_file);
        let entries = journal.as_ref().map(|j| j.entries()).unwrap_or_default();
        let mut st = State {
            model_file: model_file.to_path_buf(),
            items: items(&entries),
            confirm: false,
            said: None,
            kept_bytes: journal.as_ref().map_or(0, |j| j.kept_bytes()),
            clear_kept: false,
            sizes: None,
        };
        st.look();
        st
    }

    /// Look at what is there now and what is chosen by default, and count the sizes in a thread.
    fn look(&mut self) {
        for i in &mut self.items {
            i.exists = i.path.exists();
            i.kept = refuse(&i.path, &self.model_file);
            i.checked = i.exists && i.kept.is_none() && (i.batch || !i.latest);
            i.size = (!i.exists).then_some(0);
        }
        let todo: Vec<(usize, PathBuf)> =
            self.items.iter().enumerate().filter(|(_, i)| i.exists).map(|(k, i)| (k, i.path.clone())).collect();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for (k, p) in todo {
                if tx.send((k, bytes_of(&p))).is_err() {
                    return;
                }
            }
        });
        self.sizes = Some(rx);
    }

    /// Take the sizes counted so far; whether some are still to come.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.sizes else { return false };
        while let Ok((k, n)) = rx.try_recv() {
            if let Some(i) = self.items.get_mut(k) {
                i.size = Some(n);
            }
        }
        let pending = self.items.iter().any(|i| i.size.is_none());
        if !pending {
            self.sizes = None;
        }
        pending
    }

    /// The items chosen.
    pub fn chosen(&self) -> Vec<&Item> {
        self.items.iter().filter(|i| i.checked && i.exists && i.kept.is_none()).collect()
    }

    /// Whether the copies kept with the runs are chosen (and there are some).
    pub fn copies_chosen(&self) -> bool {
        self.clear_kept && self.kept_bytes > 0
    }

    /// How many things are chosen: the items, and the copies kept as one more.
    pub fn count(&self) -> usize {
        self.chosen().len() + usize::from(self.copies_chosen())
    }

    /// The bytes of the items chosen (and the copies kept, when chosen), and whether all their sizes are known.
    pub fn total(&self) -> (u64, bool) {
        let c = self.chosen();
        let copies = if self.copies_chosen() { self.kept_bytes } else { 0 };
        (c.iter().filter_map(|i| i.size).sum::<u64>() + copies, c.iter().all(|i| i.size.is_some()))
    }

    /// "Delete 3 items (1.2 GB)...".
    pub fn button(&self) -> String {
        let n = self.count();
        let (bytes, known) = self.total();
        let size = if known { crate::disk::size_text(bytes) } else { "counting".into() };
        format!("Delete {n} {} ({size})...", if n == 1 { "item" } else { "items" })
    }

    /// Delete the items chosen, and look again.
    pub fn delete_chosen(&mut self) -> String {
        let paths: Vec<PathBuf> = self.chosen().iter().map(|i| i.path.clone()).collect();
        let (mut gone, mut bytes, mut failed) = delete(&paths, &self.model_file);
        if self.copies_chosen() {
            match Journal::of(&self.model_file).map(|j| j.clear_kept()) {
                Some(Ok(n)) => {
                    gone += 1;
                    bytes += n;
                }
                Some(Err(e)) => failed.push((PathBuf::from("the copies kept"), e.to_string())),
                None => {}
            }
        }
        self.kept_bytes = Journal::of(&self.model_file).map_or(0, |j| j.kept_bytes());
        self.clear_kept = self.clear_kept && self.kept_bytes > 0;
        let mut said =
            format!("Deleted {gone} {} ({})", if gone == 1 { "item" } else { "items" }, crate::disk::size_text(bytes));
        if let Some((p, why)) = failed.first() {
            said.push_str(&format!("; {} not deleted, {}: {why}", failed.len(), p.display()));
        }
        self.confirm = false;
        self.said = Some(said.clone());
        for i in &mut self.items {
            i.exists = i.path.exists();
            i.checked = i.checked && i.exists;
            if !i.exists {
                i.size = Some(0);
            }
        }
        said
    }
}

/// A path as the window tells it: from the model's folder when it is in it.
fn shown(path: &Path, model_file: &Path) -> String {
    let base = model_file.parent().map(|p| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()));
    base.and_then(|b| path.strip_prefix(b).ok().map(|r| r.display().to_string()))
        .unwrap_or_else(|| path.display().to_string())
}

/// The window of *Clean outputs...*, while the session has one open.
pub fn window(ctx: &egui::Context, s: &mut Session) {
    let Some(mut st) = s.clean.take() else { return };
    if st.poll() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
    let mut open = true;
    let mut closed = false;
    let mut deleted = None;
    egui::Window::new("Clean outputs").open(&mut open).resizable(true).collapsible(false).default_width(640.0).show(
        ctx,
        |ui| {
            let (weak, warn) = (theme::weak(), ui.visuals().warn_fg_color);
            ui.label(
                RichText::new(
                    "The files and folders the runs of this model wrote, from its journal. The batch folders and \
                     the outputs of the runs before the newest are chosen.",
                )
                .small()
                .color(weak),
            );
            if st.items.is_empty() {
                ui.add_space(4.0);
                ui.label("No run has written files yet.");
            } else {
                let room = (ui.available_height() - 90.0).clamp(120.0, 420.0);
                egui::ScrollArea::both().id_salt("clean grid").max_height(room).auto_shrink([true, true]).show(
                    ui,
                    |ui| {
                        egui::Grid::new("clean items").num_columns(5).spacing([14.0, 5.0]).striped(true).show(
                            ui,
                            |ui| {
                                for h in ["", "Path", "Written", "Size", "State"] {
                                    ui.label(RichText::new(h).strong().color(weak));
                                }
                                ui.end_row();
                                for i in &mut st.items {
                                    let on = i.exists && i.kept.is_none();
                                    ui.add_enabled(on, egui::Checkbox::without_text(&mut i.checked));
                                    let name = shown(&i.path, &s.path.clone().unwrap_or_default());
                                    let r = ui.label(match (i.batch, i.check) {
                                        (true, _) => format!("{name} (batch)"),
                                        (_, true) => format!("{name} (check run)"),
                                        _ => name,
                                    });
                                    r.on_hover_text(i.path.display().to_string());
                                    let when = crate::eta::stamp(i.when).unwrap_or_default();
                                    if i.latest {
                                        ui.label(format!("{when}, the latest run's"));
                                    } else {
                                        ui.label(when);
                                    }
                                    match i.size {
                                        Some(n) if i.exists => ui.label(crate::disk::size_text(n)),
                                        None => ui.label(RichText::new("counting...").color(weak)),
                                        _ => ui.label(RichText::new("-").color(weak)),
                                    };
                                    match (i.exists, i.kept) {
                                        (false, _) => ui.label(RichText::new("missing").color(weak)),
                                        (_, Some(why)) => ui.label(RichText::new(format!("kept: {why}")).color(warn)),
                                        _ => ui.label("exists"),
                                    };
                                    ui.end_row();
                                }
                            },
                        );
                    },
                );
            }
            if st.kept_bytes > 0 {
                ui.add_space(4.0);
                ui.checkbox(
                    &mut st.clear_kept,
                    format!("the copies kept with the runs (in .terraflux), {}", crate::disk::size_text(st.kept_bytes)),
                )
                .on_hover_text(
                    "the small outputs of each run, copied so the Runs pane can still show and compare them after \
                     a later run writes over the files; deleting them leaves the runs in the journal",
                );
            }
            ui.add_space(4.0);
            if let Some(said) = &st.said {
                ui.label(said);
            }
            let n = st.count();
            if st.confirm {
                let (bytes, _) = st.total();
                ui.label(
                    RichText::new(format!(
                        "Delete {n} {} ({}) for good? They are not moved to the recycle bin.",
                        if n == 1 { "item" } else { "items" },
                        crate::disk::size_text(bytes)
                    ))
                    .color(warn),
                );
                ui.horizontal(|ui| {
                    if ui.add(crate::app::accent_button("Delete", true)).clicked() {
                        deleted = Some(st.delete_chosen());
                    }
                    if ui.button("Back").clicked() {
                        st.confirm = false;
                    }
                });
            } else {
                ui.horizontal(|ui| {
                    let b = ui.add_enabled(n > 0, egui::Button::new(st.button()));
                    if b.on_hover_text("asks first; the files are deleted for good").clicked() {
                        st.confirm = true;
                    }
                    if ui.button("Close").clicked() {
                        closed = true;
                    }
                });
            }
        },
    );
    if let Some(said) = deleted {
        s.notice = Some((said, std::time::Instant::now()));
        s.journal_rev += 1;
    }
    if open && !closed {
        s.clean = Some(st);
    }
}

/// The button at the top of the Runs pane.
pub fn button(ui: &mut Ui, s: &mut Session) {
    if ui.button("Clean outputs...").on_hover_text("delete the files and folders the runs wrote").clicked() {
        s.open_clean();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_exec::journal::{Batch, Written};

    fn out(path: &str) -> Written {
        Written { name: "n".into(), path: path.into(), ..Default::default() }
    }

    fn run(id: u64, kind: Kind, paths: &[&str]) -> Entry {
        Entry { id, when: id, kind, outputs: paths.iter().map(|p| out(p)).collect(), ..Default::default() }
    }

    fn batch(id: u64, folder: &str) -> Entry {
        let b = Batch { kind: "replicates".into(), out: PathBuf::from(folder), runs: 3, ..Default::default() };
        run(id, Kind::Batch(b), &[])
    }

    #[test]
    fn a_path_written_by_several_runs_is_one_row_and_the_newest_runs_are_marked() {
        let entries = [
            run(1, Kind::Run, &["/m/output/a.tif", "/m/output/b.tif"]),
            run(2, Kind::Run, &["/m/output/a.tif"]),
            batch(3, "/m/batch"),
            run(4, Kind::To("x".into()), &[]),
        ];
        let all = items(&entries);
        assert_eq!(all.len(), 3);
        let find = |p: &str| all.iter().find(|i| i.path == Path::new(p)).unwrap();
        // (the batch is newest, then a.tif of run 2, then b.tif of run 1)
        assert_eq!(all.iter().map(|i| i.when).collect::<Vec<_>>(), [3, 2, 1]);
        assert!(find("/m/output/a.tif").latest && !find("/m/output/b.tif").latest);
        assert!(find("/m/batch").batch && !find("/m/batch").latest);
    }

    #[test]
    fn a_check_runs_files_are_listed_as_such_and_never_the_newest_runs() {
        let check = Kind::Check(terraflux_ops::check::Check::new(None, 4));
        let entries = [run(1, Kind::Run, &["/m/output/a.tif"]), run(2, check, &["/m/runs/check/a.tif"])];
        let all = items(&entries);
        let find = |p: &str| all.iter().find(|i| i.path == Path::new(p)).unwrap();
        // (the check run is newer, yet the real run's file is the latest one)
        assert!(find("/m/runs/check/a.tif").check && !find("/m/runs/check/a.tif").latest);
        assert!(find("/m/output/a.tif").latest && !find("/m/output/a.tif").check);
    }

    #[test]
    fn the_batch_folders_and_the_older_outputs_are_chosen_and_the_newest_runs_are_not() {
        let dir = std::env::temp_dir().join("terraflux-clean-defaults");
        let _ = std::fs::remove_dir_all(&dir);
        let (model, output, folder) = (dir.join("m.toml"), dir.join("output"), dir.join("batch"));
        std::fs::create_dir_all(&output).unwrap();
        std::fs::create_dir_all(&folder).unwrap();
        for f in ["old.tif", "new.tif"] {
            std::fs::write(output.join(f), vec![0u8; 100]).unwrap();
        }
        std::fs::write(folder.join("x.csv"), vec![0u8; 50]).unwrap();
        let path = |p: PathBuf| p.to_string_lossy().into_owned();
        let journal = Journal::of(&model).unwrap();
        let m = terraflux_graph::Model::new("m");
        journal
            .record(run(1, Kind::Run, &[&path(output.join("old.tif")), &path(output.join("gone.tif"))]), &m)
            .unwrap();
        journal.record(batch(2, &path(folder.clone())), &m).unwrap();
        journal.record(run(3, Kind::Run, &[&path(output.join("new.tif"))]), &m).unwrap();
        let mut st = State::open(&model);
        while st.poll() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let by = |name: &str| st.items.iter().find(|i| i.path.ends_with(name)).unwrap();
        assert!(by("batch").checked && by("old.tif").checked);
        assert!(!by("new.tif").checked && by("new.tif").latest);
        // (a file that is gone is listed, missing, not chosen)
        assert!(!by("gone.tif").exists && !by("gone.tif").checked);
        assert_eq!((by("batch").size, by("old.tif").size), (Some(50), Some(100)));
        assert_eq!(st.total(), (150, true));
        assert_eq!(st.button(), "Delete 2 items (150 B)...");
        let said = st.delete_chosen();
        assert!(said.starts_with("Deleted 2 items"), "{said}");
        assert!(!folder.exists() && !output.join("old.tif").exists() && output.join("new.tif").exists());
        assert!(st.chosen().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_copies_kept_are_counted_once_and_deleted_only_when_chosen() {
        let dir = std::env::temp_dir().join("terraflux-clean-copies");
        let _ = std::fs::remove_dir_all(&dir);
        let (model, output) = (dir.join("m.toml"), dir.join("output"));
        std::fs::create_dir_all(&output).unwrap();
        let file = output.join("a.tif");
        std::fs::write(&file, vec![0u8; 100]).unwrap();
        let journal = Journal::of(&model).unwrap();
        let m = terraflux_graph::Model::new("m");
        let mut e = run(1, Kind::Run, &[&file.to_string_lossy()]);
        e.outputs[0].size = 100;
        journal.record(e, &m).unwrap();
        let mut st = State::open(&model);
        while st.poll() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(st.kept_bytes, 100);
        // (unchecked by default, the newest run's file is not chosen either: nothing to delete)
        assert!(!st.clear_kept && st.count() == 0);
        st.clear_kept = true;
        assert_eq!((st.count(), st.total()), (1, (100, true)));
        assert_eq!(st.button(), "Delete 1 item (100 B)...");
        let said = st.delete_chosen();
        assert!(said.starts_with("Deleted 1 item"), "{said}");
        assert!(file.exists() && journal.kept_bytes() == 0);
        assert_eq!((st.kept_bytes, st.clear_kept, st.count()), (0, false, 0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_is_not_the_runs_to_delete_is_refused() {
        let base = std::env::temp_dir().join("terraflux-clean-refuse");
        let model = base.join("sub").join("m.toml");
        assert_eq!(refuse(Path::new("output/a.tif"), &model), Some("not a full path"));
        assert_eq!(refuse(&base.join("..").join("x"), &model), Some("not a full path"));
        assert_eq!(refuse(&base.join("sub"), &model), Some("the model's folder"));
        assert_eq!(refuse(&base, &model), Some("the model's folder"));
        assert_eq!(refuse(&model, &model), Some("the model's file"));
        assert_eq!(refuse(&base.join("sub").join(".terraflux"), &model), Some("the journal's folder"));
        let root = std::env::temp_dir().ancestors().last().unwrap().to_path_buf();
        assert!(refuse(&root, &model).is_some());
        if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
            assert_eq!(refuse(Path::new(&home), &model), Some("the home folder"));
            assert_eq!(refuse(Path::new(&home).parent().unwrap(), &model), Some("the home folder"));
        }
        assert_eq!(refuse(&base.join("sub").join("output").join("a.tif"), &model), None);
        assert_eq!(refuse(&base.join("sub").join("batch"), &model), None);
        // and delete leaves them be
        let (gone, _, failed) = delete(&[base.join("sub")], &model);
        assert_eq!((gone, failed.len()), (0, 1));
    }
}
