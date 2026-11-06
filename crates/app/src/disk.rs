//! The disk, before a run: the free space of the folder the outputs go to, and an estimate of what a run (or a batch
//! that keeps its files) writes, so that a run that will not fit is told before it starts and not at its end. The
//! estimate comes from the journal (the outputs of the latest whole run of the model, the one of the same model
//! first), else from the model: the cells of the largest raster it reads, 4 bytes a cell (the output's `dtype` when
//! it has one), the layers of each map, series or stack it writes, halved for the compression. It says "about" and
//! stays simple: tables count for nothing.

use std::path::{Path, PathBuf};

use egui::RichText;
use terraflux_exec::journal::{Entry, Kind, Status};
use terraflux_graph::{Input, Model, Node, Source, Type};
use terraflux_ops::Registry;

/// A gigabyte.
const GB: u64 = 1 << 30;

/// The bytes of a cell when the output does not say (`f32`).
const CELL: u64 = 4;

/// The share of the bytes the compression is expected to save.
const COMPRESSED: f64 = 0.5;

/// A disk's space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Space {
    pub free: u64,
    pub total: u64,
}

/// The nearest folder of a path that exists (the folder itself, else the first of its parents that is there).
fn existing(dir: &Path) -> Option<PathBuf> {
    let abs = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    abs.ancestors().find(|p| p.exists()).map(Path::to_path_buf)
}

#[cfg(windows)]
fn query(p: &Path) -> Option<Space> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let (mut free, mut total, mut all_free) = (0u64, 0u64, 0u64);
    // SAFETY: `wide` ends in a nul and the three numbers are there for the call to write
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, &mut all_free) };
    (ok != 0).then_some(Space { free, total })
}

#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
fn query(p: &Path) -> Option<Space> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
    // SAFETY: `s` is zeroed memory of the right type, and `c` a nul-ended path
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let unit = s.f_frsize as u64;
    Some(Space { free: s.f_bavail as u64 * unit, total: s.f_blocks as u64 * unit })
}

#[cfg(not(any(windows, unix)))]
fn query(_: &Path) -> Option<Space> {
    None
}

/// The space of the disk a folder is on (the nearest of its parents that exists, when the folder is not made yet).
pub fn space(dir: &Path) -> Option<Space> {
    query(&existing(dir)?)
}

/// The bytes free on the disk a folder is on.
pub fn free_space(dir: &Path) -> Option<u64> {
    space(dir).map(|s| s.free)
}

/// The disk a folder is on, by name: its letter on Windows (`D:`), else the folder.
pub fn disk_name(dir: &Path) -> String {
    let abs = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    match abs.components().next() {
        Some(std::path::Component::Prefix(p)) => {
            p.as_os_str().to_string_lossy().trim_start_matches(r"\\?\").to_string()
        }
        _ => abs.display().to_string(),
    }
}

/// A size in words, the GB with a decimal.
pub fn size_text(n: u64) -> String {
    crate::runs_view::size_text(n)
}

/// What is left free of a disk that a run must not use up: 5 % of it, a gigabyte at least.
pub fn margin(total: u64) -> u64 {
    (total / 20).max(GB)
}

/// How what a run writes fits a disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// With room to spare.
    Fits,
    /// It fits, but leaves less than a tenth of the disk free.
    Tight,
    /// It leaves less than the margin, or does not fit.
    Over,
}

/// How `bytes` fit a disk: over when they exceed the free space less the margin; tight when they leave less than a
/// tenth of the disk free.
pub fn fit(bytes: u64, space: Space) -> Fit {
    if bytes == 0 {
        return Fit::Fits;
    }
    if bytes > space.free.saturating_sub(margin(space.total)) {
        Fit::Over
    } else if space.free - bytes < space.total / 10 {
        Fit::Tight
    } else {
        Fit::Fits
    }
}

/// "The outputs may take about 3.2 GB; the disk (D:) has 1.1 GB free."
pub fn over_text(bytes: u64, space: Space, dir: &Path) -> String {
    format!(
        "The outputs may take about {}; the disk ({}) has {} free.",
        size_text(bytes),
        disk_name(dir),
        size_text(space.free)
    )
}

/// What to do before a run that writes about `bytes` into `dir`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Run.
    Go,
    /// Run, and tell this (it fits but leaves little).
    Warn(String),
    /// Ask first (it may not fit).
    Ask(String),
}

/// The plan for a run that writes about `bytes` into `dir`; `space` is the disk's, when it is already known.
pub fn plan(bytes: u64, dir: &Path, space: Option<Space>) -> Plan {
    let Some(sp) = space.or_else(|| self::space(dir)) else { return Plan::Go };
    match fit(bytes, sp) {
        Fit::Fits => Plan::Go,
        Fit::Over => Plan::Ask(over_text(bytes, sp, dir)),
        Fit::Tight => Plan::Warn(format!(
            "The outputs may take about {}: the disk ({}) will be left with {} free.",
            size_text(bytes),
            disk_name(dir),
            size_text(sp.free - bytes)
        )),
    }
}

/// The end of the cost line of a batch that keeps files: ", about 12 GB on disk (40 GB free)", and whether it does
/// not fit.
pub fn cost_note(bytes: u64, dir: &Path, space: Option<Space>) -> Option<(String, bool)> {
    if bytes == 0 {
        return None;
    }
    let sp = space.or_else(|| self::space(dir))?;
    Some((format!(", about {} on disk ({} free)", size_text(bytes), size_text(sp.free)), fit(bytes, sp) == Fit::Over))
}

/// The bytes the latest whole run of the model wrote, from the journal's entries (the oldest first): the latest run
/// of the same model (its `hash`) when there is one, else the latest. None when no whole run is kept.
pub fn from_journal(entries: &[Entry], hash: Option<&str>) -> Option<u64> {
    let whole = |e: &&Entry| matches!(e.kind, Kind::Run) && e.status == Status::Done && !e.outputs.is_empty();
    let same = hash.and_then(|h| entries.iter().rev().filter(whole).find(|e| e.model == h));
    let e = same.or_else(|| entries.iter().rev().find(whole))?;
    Some(e.outputs.iter().map(|w| w.size).sum())
}

/// A number in a setting or a literal input, whichever the node has it in.
fn number(n: &Node, key: &str) -> Option<u64> {
    let v = n.params.get(key).or(match n.inputs.get(key) {
        Some(Source::Literal(v)) => Some(v),
        _ => None,
    })?;
    v.as_u64().or_else(|| v.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64))
}

/// Every node, inside the containers too.
fn nodes(ns: &[Node]) -> Vec<&Node> {
    let mut out = Vec::new();
    for n in ns {
        out.push(n);
        if let Some(b) = &n.body {
            out.extend(nodes(&b.nodes));
        }
    }
    out
}

/// The cells of the raster in a file, from its header; none for an address on the web, a file that does not read.
fn cells_of(path: &str, dir: &Path) -> Option<u64> {
    if path.is_empty()
        || path.starts_with(['$', '@'])
        || terraflux_graph::is_sent(path)
        || terraflux_graph::is_remote(path)
    {
        return None;
    }
    let at = if Path::new(path).is_absolute() { PathBuf::from(path) } else { dir.join(path) };
    let first: Vec<PathBuf> = terraflux_ops::files::files_of(&at.to_string_lossy()).ok()?.into_iter().take(1).collect();
    let r = terraflux_ops::files::Rasters::open_files(&at.to_string_lossy(), first, None, None).ok()?;
    Some(r.grid.width as u64 * r.grid.height as u64)
}

/// The cells of the largest raster a model reads: its inputs' files, its `raster.read` nodes' and the size of the
/// neutral models it draws.
pub fn largest(m: &Model, dir: &Path) -> u64 {
    let mut best = 0;
    for input in m.inputs.values() {
        if let Input::Map { path, .. } | Input::Stack { path, .. } = input {
            best = best.max(cells_of(path, dir).unwrap_or(0));
        }
    }
    for n in nodes(&m.nodes) {
        if n.op == "raster.read"
            && let Some(p) = n.params.get("path").and_then(|v| v.as_str())
        {
            best = best.max(cells_of(p, dir).unwrap_or(0));
        } else if n.op.starts_with("nlm.") && !n.inputs.contains_key("like") {
            best = best.max(number(n, "rows").unwrap_or(256) * number(n, "columns").unwrap_or(256));
        }
    }
    best
}

/// The layers of a series the model writes: the steps of its loops and the `times` or `steps` it sets (a number, or a
/// list of dates), the largest; 1 when it says none.
pub fn steps(m: &Model) -> u64 {
    let mut best = 1;
    for n in nodes(&m.nodes) {
        for key in ["times", "steps"] {
            let listed = n.params.get(key).and_then(|v| v.as_array()).map(|a| a.len() as u64);
            let given = match n.inputs.get(key) {
                Some(Source::Literal(v)) => v.as_array().map(|a| a.len() as u64).or_else(|| v.as_u64()),
                _ => None,
            };
            best = best.max(listed.or(given).or_else(|| number(n, key)).unwrap_or(1));
        }
    }
    best
}

/// The bytes of a cell of a type (`u8`, `i16`, `f32`...), 4 when it is not told.
fn width_of(dtype: Option<&str>) -> u64 {
    match dtype.map(str::to_lowercase).as_deref() {
        Some("u8" | "i8" | "uint8" | "int8" | "byte") => 1,
        Some("u16" | "i16" | "uint16" | "int16") => 2,
        Some("f64" | "i64" | "u64" | "float64" | "int64" | "uint64") => 8,
        _ => CELL,
    }
}

/// What a reference (`node.port`) is, when it is a top-level node's.
fn type_of(m: &Model, r: &Registry, from: &str) -> Option<Type> {
    let (id, port) = from.split_once('.')?;
    let n = m.nodes.iter().find(|n| n.id == id)?;
    crate::editor::output_ports(n, r).into_iter().find(|(p, _)| p == port).map(|(_, t)| t)
}

/// The layers of a value a model writes: a series or a stack has the steps, a map one; none for what is not a raster.
fn layers_of(ty: Option<Type>, steps: u64) -> u64 {
    match ty {
        Some(Type::Series | Type::Stack) => steps,
        Some(Type::Map | Type::Raster | Type::Any) | None => 1,
        Some(_) => 0,
    }
}

/// The bytes a whole run of a model writes into its output folder, from the model: about, and 0 when it reads no
/// raster to size them by. `dir` is the model's folder.
pub fn from_model(m: &Model, r: &Registry, dir: &Path) -> u64 {
    let cells = largest(m, dir);
    if cells == 0 {
        return 0;
    }
    let steps = steps(m);
    let mut bytes = 0.0;
    for o in m.outputs.values() {
        let layers = if o.layer.is_some() { 1 } else { layers_of(type_of(m, r, &o.from), steps) };
        bytes += (cells * layers * width_of(o.dtype.as_deref())) as f64;
    }
    // the files its raster.write nodes write
    for n in nodes(&m.nodes).into_iter().filter(|n| n.op == "raster.write") {
        let from = match n.inputs.get("value") {
            Some(Source::Ref(f)) => Some(f.as_str()),
            _ => None,
        };
        let layers = layers_of(from.and_then(|f| type_of(m, r, f)), steps).max(1);
        let ty = n.params.get("type").and_then(|v| v.as_str());
        bytes += (cells * layers * width_of(ty)) as f64;
    }
    (bytes * (1.0 - COMPRESSED)) as u64
}

/// The bytes of one map of the model (its largest raster's cells, 4 bytes a cell, compressed): what each run of a
/// sensitivity analysis keeps for the maps of sensitivity. 0 when the model reads no raster to size it by.
pub fn map_bytes(m: &Model, dir: &Path) -> u64 {
    (largest(m, dir) as f64 * CELL as f64 * (1.0 - COMPRESSED)) as u64
}

/// The advice for check_model and the Problems pane: a warning when what a run writes (about, from the model) will
/// not fit the disk of its output folder (`dir`/output). None when it fits or nothing says how much it writes.
pub fn advice(m: &Model, r: &Registry, dir: &Path) -> Option<crate::advice::Advice> {
    advice_on(m, r, dir, space(&dir.join("output"))?)
}

/// [`advice`] for a disk of the space given.
fn advice_on(m: &Model, r: &Registry, dir: &Path, sp: Space) -> Option<crate::advice::Advice> {
    let bytes = from_model(m, r, dir);
    let out = dir.join("output");
    (fit(bytes, sp) == Fit::Over).then(|| crate::advice::Advice {
        level: crate::advice::Level::Warning,
        at: Vec::new(),
        message: format!(
            "the outputs may take about {}, the disk ({}) has {} free: free some space (Clean outputs... in the \
             application) or write the outputs to another disk before the run",
            size_text(bytes),
            disk_name(&out),
            size_text(sp.free)
        ),
        guide: "results",
    })
}

/// A run held back for the disk: what is to be told, and where the outputs go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub text: String,
}

/// What was answered to the window of a run that may not fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Run anyway.
    Anyway,
    /// Open *Clean outputs...*.
    Clean,
    Cancel,
}

/// The window of a run that may not fit the disk, while `ask` holds one; the answer once given (the window closed).
pub fn window(ctx: &egui::Context, ask: &mut Option<Ask>) -> Option<Answer> {
    let a = ask.as_ref()?;
    let mut answer = None;
    let mut open = true;
    egui::Window::new("Not much room on the disk").open(&mut open).resizable(false).collapsible(false).show(
        ctx,
        |ui| {
            ui.set_min_width(380.0);
            ui.label(&a.text);
            ui.label(
                RichText::new(
                    "An estimate from the run before, or from the model's maps. A run that fills the disk fails.",
                )
                .small()
                .color(crate::theme::weak()),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.add(crate::app::accent_button("Run anyway", false)).clicked() {
                    answer = Some(Answer::Anyway);
                }
                if ui.button("Clean outputs...").on_hover_text("delete the outputs of the runs before").clicked() {
                    answer = Some(Answer::Clean);
                }
                if ui.button("Cancel").clicked() {
                    answer = Some(Answer::Cancel);
                }
            });
        },
    );
    if !open {
        answer = Some(Answer::Cancel);
    }
    if answer.is_some() {
        *ask = None;
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_exec::journal::Written;

    fn run(model: &str, status: Status, sizes: &[u64]) -> Entry {
        Entry {
            model: model.into(),
            status,
            outputs: sizes.iter().map(|s| Written { size: *s, ..Default::default() }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn the_temp_folder_has_room_and_a_folder_not_made_yet_is_asked_of_its_parent() {
        let t = std::env::temp_dir();
        let s = space(&t).unwrap();
        assert!(s.total > 0 && s.free <= s.total);
        assert_eq!(free_space(&t.join("not").join("made").join("yet")).map(|f| f > 0), Some(true));
        assert!(!disk_name(&t).is_empty());
    }

    #[test]
    fn a_run_fits_leaves_little_or_does_not_fit() {
        let disk = Space { free: 100 * GB, total: 1000 * GB };
        assert_eq!(fit(0, disk), Fit::Fits);
        assert_eq!(fit(10 * GB, disk), Fit::Tight);
        assert_eq!(fit(60 * GB, disk), Fit::Over);
        let roomy = Space { free: 500 * GB, total: 1000 * GB };
        assert_eq!(fit(10 * GB, roomy), Fit::Fits);
        // the margin: a gigabyte at least, a twentieth of a big disk
        assert_eq!((margin(10 * GB), margin(1000 * GB)), (GB, 50 * GB));
        // the free space less the margin
        let small = Space { free: 3 * GB, total: 10 * GB };
        assert_eq!((fit(GB, small), fit(3 * GB, small)), (Fit::Fits, Fit::Over));
        let snug = Space { free: 4 * GB, total: 20 * GB };
        assert_eq!(fit(2 * GB + GB / 2, snug), Fit::Tight);
        let t = std::env::temp_dir();
        assert!(matches!(plan(1, &t, Some(small)), Plan::Go | Plan::Warn(_)));
        let Plan::Ask(text) = plan(3 * GB, &t, Some(small)) else { panic!() };
        assert!(text.contains("about 3.0 GB") && text.contains("3.0 GB free"), "{text}");
        let (note, over) = cost_note(4 * GB, &t, Some(small)).unwrap();
        assert!(over && note.contains("about 4.0 GB on disk (3.0 GB free)"), "{note}");
        assert!(cost_note(0, &t, Some(small)).is_none());
    }

    #[test]
    fn the_journal_gives_the_latest_whole_run_of_the_same_model_first() {
        let entries = [
            run("a", Status::Done, &[100, 20]),
            run("b", Status::Done, &[500]),
            run("a", Status::Failed("x".into()), &[900]),
            Entry { kind: Kind::To("n".into()), ..run("b", Status::Done, &[7000]) },
            run("c", Status::Done, &[]),
        ];
        assert_eq!(from_journal(&entries, Some("a")), Some(120));
        assert_eq!(from_journal(&entries, Some("zz")), Some(500));
        assert_eq!(from_journal(&entries, None), Some(500));
        assert_eq!(from_journal(&entries[2..], Some("a")), None);
        assert_eq!(from_journal(&[], None), None);
    }

    fn data(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name)
    }

    #[test]
    fn a_map_kept_by_a_run_is_one_layer_of_the_largest_raster() {
        let mut m = Model::new("sized");
        assert_eq!(map_bytes(&m, Path::new(".")), 0, "no raster to size it by");
        m.nodes.push(Node::new("land", "raster.read").param("path", data("sibuyan_landuse.tif").to_string_lossy()));
        let cells = largest(&m, Path::new("."));
        assert_eq!(map_bytes(&m, Path::new(".")), cells * CELL / 2);
    }

    #[test]
    fn a_model_is_sized_by_the_raster_it_reads_and_the_layers_it_writes() {
        let r = Registry::standard();
        let mut m = Model::new("sized");
        m.nodes.push(Node::new("land", "raster.read").param("path", data("sibuyan_landuse.tif").to_string_lossy()));
        // nothing written: nothing to size
        assert_eq!(from_model(&m, &r, Path::new(".")), 0);
        let cells = largest(&m, Path::new("."));
        assert!(cells > 0);
        m.outputs.insert(
            "land".into(),
            terraflux_graph::Output { from: "land.raster".into(), path: None, dtype: None, layer: None },
        );
        // a map: the cells, 4 bytes, halved
        assert_eq!(from_model(&m, &r, Path::new(".")), cells * 2);
        m.outputs.get_mut("land").unwrap().dtype = Some("u8".into());
        assert_eq!(from_model(&m, &r, Path::new(".")), cells / 2);
        // a raster.write node counts too
        m.nodes.push(Node::new("save", "raster.write").input("value", "land.raster").param("path", "x.tif"));
        assert_eq!(from_model(&m, &r, Path::new(".")), cells * 5 / 2);
        // the steps of a loop are the layers of a series
        let mut loops = Model::new("loops");
        loops.nodes.push(Node::new("land", "raster.read").param("path", data("sibuyan_landuse.tif").to_string_lossy()));
        loops.nodes.push(Node::new("again", "control.repeat").literal("times", 5));
        assert_eq!(steps(&loops), 5);
    }

    #[test]
    fn a_model_that_may_not_fit_the_disk_is_warned_about() {
        let r = Registry::standard();
        let mut m = Model::new("big");
        m.nodes.push(Node::new("a", "nlm.random").param("rows", 10000).param("columns", 10000));
        m.outputs
            .insert("a".into(), terraflux_graph::Output { from: "a.map".into(), path: None, dtype: None, layer: None });
        let dir = std::env::temp_dir();
        // 10000 x 10000 cells, 4 bytes, halved: about 190 MB
        let tight = Space { free: GB, total: 10 * GB };
        let a = advice_on(&m, &r, &dir, tight).unwrap();
        assert_eq!(a.level, crate::advice::Level::Warning);
        assert!(a.message.contains("may take about 190.7 MB") && a.message.contains("1.0 GB free"), "{}", a.message);
        assert!(advice_on(&m, &r, &dir, Space { free: 100 * GB, total: 1000 * GB }).is_none());
    }

    #[test]
    fn the_neutral_models_are_sized_by_their_rows_and_columns() {
        let mut m = Model::new("nlm");
        m.nodes.push(Node::new("a", "nlm.random").param("rows", 100).param("columns", 50));
        assert_eq!(largest(&m, Path::new(".")), 5000);
        m.nodes.push(Node::new("b", "nlm.random"));
        assert_eq!(largest(&m, Path::new(".")), 65536);
    }
}
