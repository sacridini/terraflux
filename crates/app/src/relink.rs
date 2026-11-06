//! The files a model reads that are not where it says (a model from another computer, a folder moved or renamed),
//! found again: in a folder chosen (the longest end of the old path that is there, else a file of the same name in
//! it), the change of folder of one tried on all the others (as a video editor relinks its media), or one by one.
//! The old paths are cut at `/` and at `\` alike: a model written on Windows (`C:\data\land.tif`) relinks on Linux
//! and macOS, and the other way round. All changed in one step of undo.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{RichText, Ui};
use terraflux_graph::{Input, Model, Node};

use crate::app::Session;

/// The search by name goes this deep into the folder chosen, through at most this many entries.
const DEPTH: usize = 4;
const ENTRIES: usize = 20_000;

/// Operators that write a file: theirs need not exist.
fn writes(op: &str) -> bool {
    op.ends_with(".write")
}

/// Where a path is in the model: an input's (`$name`), else a node's setting (the node's place: `node`,
/// `container/node`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Place {
    Input(String),
    Setting { node: String, setting: String },
}

impl Place {
    pub fn name(&self) -> String {
        match self {
            Place::Input(n) => format!("${n}"),
            Place::Setting { node, setting } if setting == "path" => node.clone(),
            Place::Setting { node, setting } => format!("{node} ({setting})"),
        }
    }
}

/// Every path the model reads (its inputs' files, its nodes' files, in containers too; not those written, nor those
/// on the web or sent to another computer), where each is.
pub fn paths(m: &Model) -> Vec<(Place, String)> {
    let mut out = Vec::new();
    for (name, i) in &m.inputs {
        if let Input::Map { path, .. } | Input::Stack { path, .. } = i {
            out.push((Place::Input(name.clone()), path.clone()));
        }
    }
    fn walk(nodes: &[Node], at: &str, out: &mut Vec<(Place, String)>) {
        for n in nodes {
            let place = if at.is_empty() { n.id.clone() } else { format!("{at}/{}", n.id) };
            if !writes(&n.op) {
                let files: Vec<String> = terraflux_graph::file_settings(&mut n.clone()).map(|p| p.clone()).collect();
                for (k, v) in &n.params {
                    if let Some(v) = v.as_str()
                        && files.iter().any(|f| f == v)
                    {
                        out.push((Place::Setting { node: place.clone(), setting: k.clone() }, v.to_string()));
                    }
                }
            }
            if let Some(b) = &n.body {
                walk(&b.nodes, &place, out);
            }
        }
    }
    walk(&m.nodes, "", &mut out);
    out.retain(|(_, p)| !p.is_empty() && !terraflux_graph::is_sent(p) && !terraflux_graph::is_remote(p));
    out
}

/// Where a path of the model is on this computer: as it is when absolute, else from the model's folder.
pub fn resolve(p: &str, base: &Path) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() { path.to_path_buf() } else { base.join(path) }
}

/// Whether a path names something that is there: a file, a folder, or a pattern (`*`, `?`) whose folder is.
pub fn there(p: &Path) -> bool {
    let text = p.to_string_lossy();
    if text.contains(['*', '?']) {
        let before: PathBuf =
            p.components().take_while(|c| !c.as_os_str().to_string_lossy().contains(['*', '?'])).collect();
        return before.is_dir();
    }
    p.exists()
}

/// The paths of the model not there, each once (in the order met), with where each is used.
pub fn missing(m: &Model, base: &Path) -> Vec<(String, Vec<Place>)> {
    let mut out: Vec<(String, Vec<Place>)> = Vec::new();
    for (place, p) in paths(m) {
        if there(&resolve(&p, base)) {
            continue;
        }
        match out.iter_mut().find(|(q, _)| *q == p) {
            Some((_, places)) => places.push(place),
            None => out.push((p, vec![place])),
        }
    }
    out
}

/// The parts of a path, cut at `/` and `\` (whatever system wrote it), without a drive (`C:`) or empty parts.
pub fn parts(p: &str) -> Vec<String> {
    p.split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != "." && !(s.len() == 2 && s.ends_with(':')))
        .map(String::from)
        .collect()
}

/// The longest end of a path's parts that is there under `folder`, and how many parts were left before it.
pub fn by_end(folder: &Path, parts: &[String]) -> Option<(PathBuf, usize)> {
    (0..parts.len()).find_map(|k| {
        let p = parts[k..].iter().fold(folder.to_path_buf(), |acc, s| acc.join(s));
        there(&p).then_some((p, k))
    })
}

/// The files and folders under `folder`, by name (in lower case where names are, on Windows and macOS), at most
/// [`DEPTH`] levels down and [`ENTRIES`] entries.
pub fn by_name(folder: &Path) -> HashMap<String, Vec<PathBuf>> {
    let mut found: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut todo = vec![(folder.to_path_buf(), 0)];
    let mut seen = 0;
    while let Some((dir, depth)) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            seen += 1;
            if seen > ENTRIES {
                return found;
            }
            let p = e.path();
            found.entry(key(&e.file_name().to_string_lossy())).or_default().push(p.clone());
            if depth + 1 < DEPTH && e.file_type().is_ok_and(|t| t.is_dir()) {
                todo.push((p, depth + 1));
            }
        }
    }
    found
}

fn key(name: &str) -> String {
    if cfg!(any(windows, target_os = "macos")) { name.to_lowercase() } else { name.to_string() }
}

/// The old paths found under `folder`: by their longest end there, else by their name (one file of that name), else
/// by the change of folder of those found (the old folder's beginning replaced by the new one's).
pub fn locate(old: &[String], folder: &Path) -> BTreeMap<String, PathBuf> {
    let mut found = BTreeMap::new();
    // (the old beginning and the new one, of each found)
    let mut moves: Vec<(Vec<String>, PathBuf)> = Vec::new();
    let mut names = None;
    for p in old {
        let ps = parts(p);
        if ps.is_empty() {
            continue;
        }
        let hit = by_end(folder, &ps).or_else(|| {
            let names = names.get_or_insert_with(|| by_name(folder));
            match names.get(&key(ps.last()?)).map(Vec::as_slice) {
                Some([one]) => Some((one.clone(), ps.len() - 1)),
                _ => None,
            }
        });
        if let Some((new, k)) = hit {
            // the new place of the old beginning: the new path less the end kept
            let kept = ps.len() - k;
            let mut root = new.clone();
            for _ in 0..kept {
                root.pop();
            }
            moves.push((ps[..k].to_vec(), root));
            found.insert(p.clone(), new);
        }
    }
    for p in old {
        if found.contains_key(p) {
            continue;
        }
        let ps = parts(p);
        for (from, to) in &moves {
            if ps.len() > from.len() && ps[..from.len()].iter().zip(from).all(|(a, b)| key(a) == key(b)) {
                let q = ps[from.len()..].iter().fold(to.clone(), |acc, s| acc.join(s));
                if there(&q) {
                    found.insert(p.clone(), q);
                    break;
                }
            }
        }
    }
    found
}

/// The model's paths replaced by the new ones (relative to the model's folder `base` when it has one, as saving
/// writes them); how many places changed.
pub fn replace(m: &mut Model, new: &BTreeMap<String, PathBuf>, base: Option<&Path>) -> usize {
    let text = |p: &PathBuf| match base {
        Some(b) => terraflux_graph::relative_path(p, b),
        None => p.display().to_string(),
    };
    let mut n = 0;
    for i in m.inputs.values_mut() {
        if let Input::Map { path, .. } | Input::Stack { path, .. } = i
            && let Some(q) = new.get(path)
        {
            *path = text(q);
            n += 1;
        }
    }
    fn walk(nodes: &mut [Node], new: &BTreeMap<String, PathBuf>, text: &dyn Fn(&PathBuf) -> String, n: &mut usize) {
        for node in nodes {
            if !writes(&node.op) {
                for p in terraflux_graph::file_settings(node) {
                    if let Some(q) = new.get(p.as_str()) {
                        *p = text(q);
                        *n += 1;
                    }
                }
            }
            if let Some(b) = &mut node.body {
                walk(&mut b.nodes, new, text, n);
            }
        }
    }
    walk(&mut m.nodes, new, &text, &mut n);
    n
}

/// The window of the files not found: each with where it is used and where it was found; a folder to look in, a
/// file to choose for one, then all changed at once.
#[derive(Default)]
pub struct Dialog {
    pub open: bool,
    /// The new places found, by old path.
    pub found: BTreeMap<String, PathBuf>,
    pub note: Option<String>,
}

/// The files of the model not found, worked out again when the model or its data changed (the session's revision
/// and readings), and the window's state.
#[derive(Default)]
pub struct Relink {
    of: Option<((u64, u64), Instant)>,
    pub missing: Vec<(String, Vec<Place>)>,
    pub dialog: Dialog,
}

impl Relink {
    /// The files not found, worked out again after an edit or a reading of the data, and every few seconds (files
    /// copied in meanwhile).
    pub fn follow(&mut self, m: &Model, base: &Path, at: (u64, u64)) {
        if self.of.is_none_or(|(was, when)| was != at || when.elapsed() > Duration::from_secs(5)) {
            self.of = Some((at, Instant::now()));
            self.missing = missing(m, base);
            if self.missing.is_empty() {
                self.dialog.open = false;
            }
        }
    }
}

/// The strip over the canvas when files are not found.
pub fn strip(ui: &mut Ui, s: &mut Session) {
    let n = s.relink.missing.len();
    if n == 0 {
        return;
    }
    ui.horizontal(|ui| {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            if n == 1 {
                "1 file of the model is not found.".to_string()
            } else {
                format!("{n} files of the model are not found.")
            },
        );
        if ui.button("Locate them...").on_hover_text("look for them in a folder, or choose each").clicked() {
            s.relink.dialog.open = true;
        }
    });
}

/// The window that finds the files again.
pub fn window(ctx: &egui::Context, s: &mut Session) {
    if !s.relink.dialog.open || s.relink.missing.is_empty() {
        return;
    }
    let mut open = true;
    let mut look = false;
    let mut one: Option<String> = None;
    let mut apply = false;
    let missing = s.relink.missing.clone();
    egui::Window::new("Files not found").open(&mut open).default_width(640.0).show(ctx, |ui| {
        ui.label(
            RichText::new(
                "The model reads these files, but they are not where it says. Look in a folder (the files found by \
                 the end of their path, or by their name), or choose one by one.",
            )
            .weak(),
        );
        ui.add_space(4.0);
        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            for (k, (old, places)) in missing.iter().enumerate() {
                if k > 0 {
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    if ui.small_button("Choose...").on_hover_text("choose where this one is").clicked() {
                        one = Some(old.clone());
                    }
                    ui.add(egui::Label::new(RichText::new(old).monospace()).truncate()).on_hover_text(old);
                });
                let used: Vec<String> = places.iter().map(Place::name).collect();
                let (text, colour) = match s.relink.dialog.found.get(old) {
                    Some(p) => (format!("found: {}", p.display()), crate::review::ADDED),
                    None => ("not found yet".to_string(), ui.visuals().weak_text_color()),
                };
                ui.add(egui::Label::new(RichText::new(text).color(colour)).truncate());
                ui.label(RichText::new(format!("read by {}", used.join(", "))).weak().size(11.0));
            }
        });
        ui.add_space(6.0);
        if let Some(n) = &s.relink.dialog.note {
            ui.label(RichText::new(n).weak());
        }
        ui.horizontal(|ui| {
            look = ui.button("Look in a folder...").clicked();
            let found = s.relink.dialog.found.len();
            apply = ui
                .add_enabled(found > 0, crate::app::accent_button(&format!("Apply ({found} found)"), found > 0))
                .on_hover_text("the model reads them from where they were found (one step of undo)")
                .clicked();
        });
    });
    s.relink.dialog.open = open;
    let olds: Vec<String> = missing.iter().map(|(p, _)| p.clone()).collect();
    if look && let Some(folder) = rfd::FileDialog::new().set_title("The folder the files are in").pick_folder() {
        let found = locate(&olds, &folder);
        let n = found.len();
        s.relink.dialog.found.extend(found);
        s.relink.dialog.note = Some(format!("{n} of {} found in {}", olds.len(), folder.display()));
    }
    if let Some(old) = one {
        let name = parts(&old).last().cloned().unwrap_or_default();
        let folder = old.contains(['*', '?']) || Path::new(&old).extension().is_none();
        let picked = if folder {
            rfd::FileDialog::new().set_title(format!("Where is {name}?")).pick_folder()
        } else {
            rfd::FileDialog::new().set_title(format!("Where is {name}?")).set_file_name(&name).pick_file()
        };
        if let Some(p) = picked {
            // (a pattern keeps its last part: the folder chosen is where it looks)
            let p = if old.contains(['*', '?']) { p.join(&name) } else { p };
            s.relink.dialog.found.insert(old.clone(), p.clone());
            // the same change of folder tried on the others
            let mut root = p.clone();
            root.pop();
            let others: Vec<String> =
                olds.iter().filter(|o| !s.relink.dialog.found.contains_key(*o)).cloned().collect();
            let more = locate(&others, &root);
            s.relink.dialog.found.extend(more);
        }
    }
    if apply {
        let found = std::mem::take(&mut s.relink.dialog.found);
        let base = s.path.as_ref().map(|_| s.base());
        let n = replace(&mut s.source, &found, base.as_deref());
        s.history.name(&format!("{n} file{} located again", if n == 1 { "" } else { "s" }));
        s.edited();
        s.read_data();
        s.relink.dialog.note = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join("terraflux-relink-tests").join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn paths_are_cut_at_both_separators() {
        assert_eq!(parts(r"C:\data\land use\2020.tif"), ["data", "land use", "2020.tif"]);
        assert_eq!(parts("/home/ana/data/2020.tif"), ["home", "ana", "data", "2020.tif"]);
        assert_eq!(parts(r"..\inputs/./x.tif"), ["..", "inputs", "x.tif"]);
    }

    #[test]
    fn a_windows_path_is_found_by_its_end_anywhere() {
        let d = tmp("end");
        touch(&d.join("project").join("inputs").join("land.tif"));
        let found = locate(&[r"D:\old\project\inputs\land.tif".to_string()], &d);
        assert_eq!(found.values().next(), Some(&d.join("project").join("inputs").join("land.tif")));
    }

    #[test]
    fn a_file_is_found_by_its_name_and_the_others_by_the_same_move() {
        let d = tmp("name");
        touch(&d.join("deep").join("a").join("roads.gpkg"));
        touch(&d.join("deep").join("a").join("sub").join("rivers.gpkg"));
        let old = [
            "/mnt/x/gis/roads.gpkg".to_string(),
            "/mnt/x/gis/sub/rivers.gpkg".to_string(),
            "/mnt/x/nothing.tif".to_string(),
        ];
        let found = locate(&old, &d);
        assert_eq!(found.get(&old[0]), Some(&d.join("deep").join("a").join("roads.gpkg")), "by its name");
        assert_eq!(found.get(&old[1]), Some(&d.join("deep").join("a").join("sub").join("rivers.gpkg")), "by the move");
        assert!(!found.contains_key(&old[2]));
    }

    #[test]
    fn the_model_reads_from_where_they_were_found() {
        let d = tmp("model");
        touch(&d.join("data").join("land.tif"));
        let mut m = Model::new("m");
        m.nodes.push(Node::new("land", "raster.read").param("path", r"C:\gone\land.tif"));
        m.nodes.push(Node::new("out", "raster.write").param("path", r"C:\gone\out.tif"));
        m.inputs.insert(
            "drivers".into(),
            Input::Stack { path: r"C:\gone\land.tif".into(), bands: None, nodata: None, note: None },
        );
        let miss = missing(&m, &d);
        assert_eq!(miss.len(), 1, "the file written is not asked for: {miss:?}");
        assert_eq!(miss[0].1.len(), 2, "read by the input and by the node");
        let found = locate(&[miss[0].0.clone()], &d);
        assert_eq!(replace(&mut m, &found, Some(&d)), 2);
        assert_eq!(m.nodes[0].params["path"], serde_json::json!("data/land.tif"));
        assert!(missing(&m, &d).is_empty());
    }

    #[test]
    fn a_pattern_is_there_when_its_folder_is() {
        let d = tmp("pattern");
        assert!(there(&d.join("*.tif")));
        assert!(!there(&d.join("gone").join("*.tif")));
    }
}
