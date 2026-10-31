//! The Files pane: the computer's folders, to take data to the model from, as a file manager does.
//!
//! - Where: back, forward and up (also alt+arrows and the mouse's side buttons), the folders of the path as
//!   buttons (a click on the bar's empty part, or ctrl+L, to type a path, with the folders that match offered),
//!   and the places: the folders pinned, the model's, the user's (documents, desktop, downloads), the drives, the
//!   folders seen last. The folder shown is kept between sessions.
//! - What: the data of the folder (rasters, vectors, models; every file if asked), with their size and age, sorted
//!   by any column, filtered by a part of their name or a pattern (`*.tif`); a folder opens in place (its arrow),
//!   or is gone into (a double click). The hidden files and the files that go with others (a shapefile's `.dbf`,
//!   a raster's `.aux.xml`) are left out.
//! - Choosing: a click, ctrl+click one more, shift+click a range, the arrows and the first letters of a name from
//!   the keyboard. The raster or vector features chosen are described below the list: a picture of them and what
//!   they are (size, coordinate system, cells, bands, no-data; features, kinds, attributes).
//! - Taking: what is chosen dragged to the model's canvas (several at once) is a node each that reads it where it
//!   is let go; a double click or enter adds it after the node selected; a right click offers the rest (a folder
//!   of rasters read as one stack, a model opened, the path copied, the file shown in the system's file manager).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant, SystemTime};

use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};
use terraflux_graph::Type;
use terraflux_ops::Registry;

use crate::app::Session;
use crate::builder::{DraggedFile, add_at, add_files, is_raster, is_table, is_vector};
use crate::metadata::Fact;
use crate::theme;

/// The height of a row of the list.
const ROW: f32 = 22.0;
/// How often the folders shown are looked at for changes.
const LOOK: Duration = Duration::from_millis(1500);
/// The folders seen last kept.
const RECENT: usize = 8;

/// What a file is, for the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Folder,
    Model,
    Raster,
    Vector,
    Table,
    Other,
}

impl Kind {
    fn data(self) -> bool {
        matches!(self, Kind::Raster | Kind::Vector | Kind::Table)
    }
}

#[derive(Debug, Clone)]
struct Entry {
    name: String,
    path: PathBuf,
    kind: Kind,
    size: u64,
    modified: Option<SystemTime>,
}

/// A folder read: what is in it (or why it could not be read) and when it last changed.
struct Folder {
    entries: Vec<Entry>,
    error: Option<String>,
    changed: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sort {
    Name,
    Size,
    Modified,
}

/// A picture of a file chosen: a raster's cells, or vector features' shapes (in 0..1, y down).
enum Picture {
    Image(egui::ColorImage),
    Shapes { lines: Vec<Vec<[f32; 2]>>, points: Vec<[f32; 2]>, aspect: f32 },
}

/// What is known of the file chosen.
struct Details {
    path: PathBuf,
    kind: Kind,
    facts: Vec<Fact>,
    picture: Option<Picture>,
    error: Option<String>,
    reading: bool,
}

/// A row of the list: an entry, how deep it is (a folder opened in place shows its own under it).
struct Row {
    entry: Entry,
    depth: usize,
}

/// What the user asked, done once the list is drawn.
enum Do {
    Go(PathBuf),
    Back,
    Forward,
    Up,
    Expand(PathBuf),
    Choose { path: PathBuf, add: bool, range: bool },
    Open(Entry),
    Add(Vec<PathBuf>),
    Stack(PathBuf),
    Model(PathBuf),
    Pin(PathBuf),
    Unpin(PathBuf),
    Reveal(PathBuf),
    Copy(PathBuf),
    Refresh,
}

/// The pane's state.
pub struct Files {
    dir: Option<PathBuf>,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    folders: std::collections::HashMap<PathBuf, Folder>,
    looked_at: Option<Instant>,
    /// The folders opened in place.
    open: HashSet<PathBuf>,
    /// The entries chosen (in the order chosen), where a shift+click's range starts, the row of the keyboard.
    chosen: Vec<PathBuf>,
    anchor: Option<PathBuf>,
    cursor: Option<PathBuf>,
    /// Scroll to the keyboard's row at the next frame.
    reveal: bool,
    filter: String,
    /// every file, not only the data
    all: bool,
    sort: Sort,
    ascending: bool,
    /// The path being typed, the folders that match it (for the text they are of), whether to focus the field.
    typed: Option<(String, bool)>,
    offered: (String, Vec<PathBuf>),
    /// The first letters of a name typed (and when), to go to it.
    letters: (String, Option<Instant>),
    /// The pane was clicked last (its keys are the pane's then).
    active: bool,
    drives: Option<Vec<PathBuf>>,
    details: Option<Details>,
    /// The height of the details, under the list.
    split: f32,
    texture: Option<(PathBuf, egui::TextureHandle)>,
    told: Sender<(PathBuf, Details)>,
    tell: Receiver<(PathBuf, Details)>,
}

impl Default for Files {
    fn default() -> Files {
        let (told, tell) = channel();
        Files {
            dir: None,
            back: Vec::new(),
            forward: Vec::new(),
            folders: Default::default(),
            looked_at: None,
            open: HashSet::new(),
            chosen: Vec::new(),
            anchor: None,
            cursor: None,
            reveal: false,
            filter: String::new(),
            all: false,
            sort: Sort::Name,
            ascending: true,
            typed: None,
            offered: (String::new(), Vec::new()),
            letters: (String::new(), None),
            active: false,
            drives: None,
            details: None,
            split: 280.0,
            texture: None,
            told,
            tell,
        }
    }
}

fn kind_of(p: &Path, dir: bool) -> Kind {
    if dir {
        Kind::Folder
    } else if is_table(p) {
        Kind::Table
    } else if is_vector(p) {
        Kind::Vector
    } else if is_raster(p) {
        Kind::Raster
    } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")) {
        Kind::Model
    } else {
        Kind::Other
    }
}

/// Whether a file is hidden: a name starting with a dot, or (on Windows) the hidden or system attribute.
fn hidden(name: &str, meta: Option<&std::fs::Metadata>) -> bool {
    if name.starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.is_some_and(|m| m.file_attributes() & 0x6 != 0) {
            return true;
        }
    }
    let _ = meta;
    false
}

/// Read a folder.
fn read(dir: &Path) -> Folder {
    let changed = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
    let list = match std::fs::read_dir(dir) {
        Ok(l) => l,
        Err(e) => return Folder { entries: Vec::new(), error: Some(e.to_string()), changed },
    };
    let mut entries = Vec::new();
    for e in list.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let path = e.path();
        // (a link: what it leads to)
        let meta = e.metadata().ok().filter(|m| !m.is_symlink()).or_else(|| std::fs::metadata(&path).ok());
        if hidden(&name, meta.as_ref()) {
            continue;
        }
        let dir = meta.as_ref().is_some_and(|m| m.is_dir());
        entries.push(Entry {
            kind: kind_of(&path, dir),
            size: meta.as_ref().map_or(0, |m| if dir { 0 } else { m.len() }),
            modified: meta.as_ref().and_then(|m| m.modified().ok()),
            name,
            path,
        });
    }
    Folder { entries, error: None, changed }
}

/// Names compared as people do: the numbers in them by their value (`map2` before `map10`), the letters
/// ignoring case.
fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(c) = x.next_if(|c| c.is_ascii_digit()) {
                    n.push(c);
                }
                let mut m = String::new();
                while let Some(d) = y.next_if(|d| d.is_ascii_digit()) {
                    m.push(d);
                }
                let (n, m) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let o = n.len().cmp(&m.len()).then_with(|| n.cmp(m));
                if o.is_ne() {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                let o = c.to_lowercase().cmp(d.to_lowercase());
                if o.is_ne() {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// Whether a name matches what is typed in the filter: a pattern of `*` and `?` when it has one, else a part
/// of the name; ignoring case.
fn matches(filter: &str, name: &str) -> bool {
    let (f, n) = (filter.trim().to_lowercase(), name.to_lowercase());
    if f.is_empty() {
        return true;
    }
    if !f.contains(['*', '?']) {
        return n.contains(&f);
    }
    let (p, s): (Vec<char>, Vec<char>) = (f.chars().collect(), n.chars().collect());
    let (mut i, mut j, mut star, mut mark) = (0, 0, None, 0);
    while j < s.len() {
        if i < p.len() && (p[i] == '?' || p[i] == s[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if let Some(k) = star {
            i = k + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    p[i..].iter().all(|&c| c == '*')
}

fn size_text(n: u64) -> String {
    match n {
        n if n >= 1 << 30 => format!("{:.1} GB", n as f64 / (1u64 << 30) as f64),
        n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
        n if n >= 1 << 10 => format!("{:.0} KB", n as f64 / 1024.0),
        n => format!("{n} B"),
    }
}

/// How long ago a file changed: minutes, hours, days, then the date.
fn age_text(t: SystemTime) -> String {
    let Ok(d) = SystemTime::now().duration_since(t) else { return "now".into() };
    let s = d.as_secs();
    match s {
        s if s < 60 => "now".into(),
        s if s < 3600 => format!("{} min ago", s / 60),
        s if s < 86_400 => format!("{} h ago", s / 3600),
        s if s < 2 * 86_400 => "yesterday".into(),
        s if s < 30 * 86_400 => format!("{} days ago", s / 86_400),
        _ => {
            // the date (UTC), from the days since 1970 (Howard Hinnant's civil_from_days)
            let days = t.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400) as i64;
            let z = days + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z - era * 146_097;
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let day = doy - (153 * mp + 2) / 5 + 1;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let year = yoe + era * 400 + i64::from(month <= 2);
            format!("{year}-{month:02}-{day:02}")
        }
    }
}

/// The user's folder.
fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

/// The user's documents (else the user's folder).
fn documents() -> Option<PathBuf> {
    let h = home()?;
    let docs = h.join("Documents");
    Some(if docs.is_dir() { docs } else { h })
}

/// The drives (Windows) or the places disks are put (elsewhere).
fn drives() -> Vec<PathBuf> {
    if cfg!(windows) {
        (b'A'..=b'Z').map(|d| PathBuf::from(format!("{}:\\", d as char))).filter(|p| p.is_dir()).collect()
    } else {
        let mut out = vec![PathBuf::from("/")];
        for base in ["/Volumes", "/media", "/mnt", "/run/media"] {
            if let Ok(l) = std::fs::read_dir(base) {
                out.extend(l.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
            }
        }
        out
    }
}

/// A short name of a folder: its last part (a drive as it is).
fn folder_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned())
}

/// The folders of a path, from the top: each one's name and path.
fn crumbs(p: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    let mut at = PathBuf::new();
    for c in p.components() {
        at.push(c);
        match c {
            std::path::Component::Prefix(x) => out.push((x.as_os_str().to_string_lossy().into_owned(), at.clone())),
            std::path::Component::RootDir => match out.last_mut() {
                // the drive and its root are one (`C:\`)
                Some(last) => {
                    last.0.push(std::path::MAIN_SEPARATOR);
                    last.1 = at.clone();
                }
                None => out.push(("/".into(), at.clone())),
            },
            std::path::Component::Normal(n) => out.push((n.to_string_lossy().into_owned(), at.clone())),
            _ => {}
        }
    }
    out
}

/// The folders whose path starts as typed (the folder of the text, the folders in it whose name begins with
/// what follows it).
fn offer(typed: &str) -> Vec<PathBuf> {
    let t = typed.trim();
    let (parent, start) = match t.rfind(['/', '\\']) {
        Some(k) => (&t[..=k], &t[k + 1..]),
        None => return Vec::new(),
    };
    let Ok(l) = std::fs::read_dir(parent) else { return Vec::new() };
    let start = start.to_lowercase();
    let mut out: Vec<PathBuf> = l
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|f| f.is_dir()))
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            n.starts_with(&start) && !n.starts_with('.')
        })
        .map(|e| e.path())
        .collect();
    out.sort_by(|a, b| natural(&folder_name(a), &folder_name(b)));
    out.truncate(8);
    out
}

/// Show a file in the system's file manager.
fn reveal(p: &Path) {
    let r = if cfg!(windows) {
        std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg("-R").arg(p).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(p.parent().unwrap_or(p)).spawn()
    };
    let _ = r;
}

/// A raster's picture: its first band, every `2^L`-th cell so that it is at most 256 pixels a side, in the
/// colours its values call for.
fn raster_picture(r: Arc<terraflux_ops::files::Rasters>) -> Option<egui::ColorImage> {
    use crate::map_view::{Source, TILE};
    let source = Source::Rasters(r, 0);
    let (w, h) = source.size();
    if w == 0 || h == 0 {
        return None;
    }
    let level = (0..32u32).find(|&l| w.max(h).div_ceil(1 << l) <= TILE)?;
    let tile = source.tile(level, 0, 0).ok()?;
    let step = 1usize << level;
    let (cw, ch) = (w.div_ceil(step).min(TILE), h.div_ceil(step).min(TILE));
    let mut values = Vec::with_capacity(cw * ch);
    for i in 0..ch {
        values.extend_from_slice(&tile[i * TILE..i * TILE + cw]);
    }
    let finite: Vec<f64> = values.iter().filter(|v| v.is_finite()).map(|&v| v as f64).collect();
    if finite.is_empty() {
        return None;
    }
    let coloring = crate::palette::Coloring::for_values(&finite);
    Some(crate::palette::image_f32(&values, cw, ch, &coloring))
}

/// Vector features' picture: their lines (polygons' rings) and points, at most a hundred thousand points.
fn vector_picture(v: &terraflux_raster::vector::Vector) -> Option<Picture> {
    use terraflux_raster::vector::Geometry;
    let b = v.bounds()?;
    let (w, h) = ((b[2] - b[0]).max(1e-12), (b[3] - b[1]).max(1e-12));
    let side = w.max(h);
    let at = |p: [f64; 2]| [((p[0] - b[0]) / side) as f32, ((b[3] - p[1]) / side) as f32];
    let (mut lines, mut points) = (Vec::new(), Vec::new());
    let mut left = 100_000usize;
    for g in &v.geometries {
        if left == 0 {
            break;
        }
        let mut line = |l: &Vec<[f64; 2]>, closed: bool| {
            let mut out: Vec<[f32; 2]> = l.iter().map(|p| at(*p)).collect();
            if closed && let Some(&first) = out.first() {
                out.push(first);
            }
            left = left.saturating_sub(out.len());
            lines.push(out);
        };
        match g {
            Geometry::Empty => {}
            Geometry::Point(p) => points.push(at(*p)),
            Geometry::MultiPoint(ps) => points.extend(ps.iter().map(|p| at(*p))),
            Geometry::LineString(l) => line(l, false),
            Geometry::MultiLineString(ls) => ls.iter().for_each(|l| line(l, false)),
            Geometry::Polygon(rs) => rs.iter().for_each(|r| line(r, true)),
            Geometry::MultiPolygon(ps) => ps.iter().flatten().for_each(|r| line(r, true)),
        }
    }
    Some(Picture::Shapes { lines, points, aspect: (w / h) as f32 })
}

/// What a raster or vector features are, read in a thread.
fn describe(path: PathBuf, kind: Kind) -> Details {
    let mut d = Details { path: path.clone(), kind, facts: Vec::new(), picture: None, error: None, reading: false };
    let text = path.to_string_lossy().into_owned();
    match kind {
        Kind::Raster => match terraflux_ops::files::Rasters::open(&text, None, None) {
            Ok(r) => {
                let r = Arc::new(r);
                d.facts = crate::metadata::facts(&terraflux_ops::Value::Raster(r.clone()));
                d.picture = raster_picture(r).map(Picture::Image);
            }
            Err(e) => d.error = Some(e.to_string()),
        },
        Kind::Table => match terraflux_ops::tables::read_table(&path) {
            Ok((t, text)) => {
                d.facts.push(("rows".into(), t.rows.len().to_string()));
                d.facts.push(("columns".into(), t.columns.join(", ")));
                if !text.is_empty() {
                    d.facts.push(("text (left out)".into(), text.join(", ")));
                }
                for (k, row) in t.rows.iter().take(5).enumerate() {
                    let cells: Vec<String> = row.iter().map(|v| crate::table_view::cell(*v)).collect();
                    d.facts.push((format!("row {}", k + 1), cells.join("  ")));
                }
            }
            Err(e) => d.error = Some(e.to_string()),
        },
        Kind::Vector => match terraflux_gdal::read_vector(&path, None) {
            Ok(v) => {
                d.picture = vector_picture(&v);
                d.facts = crate::metadata::facts(&terraflux_ops::Value::Vector(Arc::new(v)));
            }
            Err(e) => d.error = Some(e.to_string()),
        },
        _ => {}
    }
    // the file's name is the pane's already
    d.facts.retain(|(k, _)| k != "file" && k != "files");
    d
}

/// A small picture of what a file is: a folder, a raster's cells, a vector's shape, a model's nodes, a page.
fn icon(p: &egui::Painter, r: Rect, kind: Kind, v: &egui::Visuals) {
    let s = r.width();
    let (x, y) = (r.left(), r.top());
    match kind {
        Kind::Folder => {
            let c = if v.dark_mode { Color32::from_rgb(214, 170, 82) } else { Color32::from_rgb(196, 146, 46) };
            let tab = Rect::from_min_max(pos2(x + 1.0, y + s * 0.16), pos2(x + s * 0.48, y + s * 0.4));
            let body = Rect::from_min_max(pos2(x + 1.0, y + s * 0.28), pos2(x + s - 1.0, y + s * 0.88));
            p.rect_filled(tab, 1.5, c.gamma_multiply(0.75));
            p.rect_filled(body, 2.0, c);
        }
        Kind::Raster => {
            let c = crate::editor_view::type_colour(Type::Map, v.dark_mode).unwrap_or(v.text_color());
            let cell = (s - 4.0) / 3.0;
            let shade = [1.0, 0.55, 0.8, 0.45, 0.9, 0.6, 0.7, 1.0, 0.5];
            for i in 0..3 {
                for j in 0..3 {
                    let at = pos2(x + 2.0 + j as f32 * cell, y + 2.0 + i as f32 * cell);
                    let q = Rect::from_min_size(at, Vec2::splat(cell - 0.8));
                    p.rect_filled(q, 0.5, c.gamma_multiply(shade[i * 3 + j]));
                }
            }
        }
        Kind::Vector => {
            let c = crate::editor_view::type_colour(Type::Vector, v.dark_mode).unwrap_or(v.text_color());
            let pts: Vec<Pos2> = [[0.18, 0.62], [0.42, 0.18], [0.84, 0.32], [0.76, 0.8], [0.34, 0.86]]
                .iter()
                .map(|q| pos2(x + q[0] * s, y + q[1] * s))
                .collect();
            p.add(egui::Shape::convex_polygon(pts.clone(), c.gamma_multiply(0.25), Stroke::new(1.3, c)));
            for q in pts {
                p.circle_filled(q, 1.6, c);
            }
        }
        Kind::Model => {
            let c = theme::accent();
            let (a, b, d) =
                (pos2(x + s * 0.2, y + s * 0.5), pos2(x + s * 0.8, y + s * 0.24), pos2(x + s * 0.8, y + s * 0.76));
            p.line_segment([a, b], Stroke::new(1.2, c));
            p.line_segment([a, d], Stroke::new(1.2, c));
            for q in [a, b, d] {
                p.circle_filled(q, s * 0.14, c);
            }
        }
        Kind::Table => {
            let c = crate::editor_view::type_colour(Type::Table, v.dark_mode).unwrap_or(v.text_color());
            let t = Rect::from_min_max(pos2(x + 1.5, y + s * 0.18), pos2(x + s - 1.5, y + s * 0.84));
            p.rect_stroke(t, 1.5, Stroke::new(1.2, c), egui::StrokeKind::Inside);
            p.rect_filled(Rect::from_min_max(t.min, pos2(t.right(), t.top() + s * 0.2)), 1.5, c.gamma_multiply(0.7));
            for k in 1..3 {
                let yy = t.top() + s * 0.2 + k as f32 * (t.height() - s * 0.2) / 3.0;
                p.line_segment([pos2(t.left(), yy), pos2(t.right(), yy)], Stroke::new(0.8, c));
            }
            p.line_segment([pos2(t.center().x, t.top()), pos2(t.center().x, t.bottom())], Stroke::new(0.8, c));
        }
        Kind::Other => {
            let c = v.weak_text_color();
            let page = Rect::from_min_max(pos2(x + s * 0.22, y + s * 0.1), pos2(x + s * 0.78, y + s * 0.9));
            p.rect_stroke(page, 1.0, Stroke::new(1.0, c), egui::StrokeKind::Inside);
            for k in 0..3 {
                let yy = y + s * (0.38 + 0.15 * k as f32);
                p.line_segment([pos2(x + s * 0.34, yy), pos2(x + s * 0.66, yy)], Stroke::new(1.0, c));
            }
        }
    }
}

impl Files {
    /// The folder shown: the one kept, else the model's, else the user's documents.
    fn dir(&mut self, model: Option<&Path>) -> PathBuf {
        if self.dir.is_none() {
            let kept = crate::settings::get().files_dir.filter(|d| d.is_dir());
            let of_model = model.and_then(|m| m.parent()).filter(|d| d.is_dir()).map(Path::to_path_buf);
            self.dir = kept.or(of_model).or_else(documents).or_else(|| std::env::current_dir().ok());
        }
        self.dir.clone().unwrap_or_default()
    }

    /// Show another folder (kept for the next session, and among the folders seen last).
    pub fn go(&mut self, dir: PathBuf) {
        if self.dir.as_ref() == Some(&dir) {
            return;
        }
        if let Some(d) = self.dir.take() {
            self.back.push(d);
        }
        self.forward.clear();
        self.arrive(dir);
    }

    fn arrive(&mut self, dir: PathBuf) {
        crate::settings::update(|s| {
            s.files_dir = Some(dir.clone());
            s.files_recent.retain(|d| d != &dir);
            s.files_recent.insert(0, dir.clone());
            s.files_recent.truncate(RECENT);
        });
        self.folders.remove(&dir);
        self.dir = Some(dir);
        self.chosen.clear();
        self.anchor = None;
        self.cursor = None;
        self.typed = None;
        self.filter.clear();
    }

    fn go_back(&mut self) {
        if let Some(d) = self.back.pop() {
            self.forward.extend(self.dir.take());
            self.arrive(d);
        }
    }

    fn go_forward(&mut self) {
        if let Some(d) = self.forward.pop() {
            self.back.extend(self.dir.take());
            self.arrive(d);
        }
    }

    /// A folder's entries (read when not yet), sorted as asked.
    fn entries(&mut self, dir: &Path) -> &Folder {
        if !self.folders.contains_key(dir) {
            let mut f = read(dir);
            self.order(&mut f.entries);
            self.folders.insert(dir.to_path_buf(), f);
        }
        &self.folders[dir]
    }

    fn order(&self, entries: &mut [Entry]) {
        let (sort, up) = (self.sort, self.ascending);
        entries.sort_by(|a, b| {
            let folders = (a.kind != Kind::Folder).cmp(&(b.kind != Kind::Folder));
            let o = match sort {
                Sort::Name => natural(&a.name, &b.name),
                Sort::Size => a.size.cmp(&b.size).then_with(|| natural(&a.name, &b.name)),
                Sort::Modified => a.modified.cmp(&b.modified).then_with(|| natural(&a.name, &b.name)),
            };
            folders.then(if up { o } else { o.reverse() })
        });
    }

    /// Sort by a column (again: the other way).
    fn sort_by(&mut self, s: Sort) {
        if self.sort == s {
            self.ascending = !self.ascending;
        } else {
            self.sort = s;
            self.ascending = s == Sort::Name;
        }
        let mut folders = std::mem::take(&mut self.folders);
        for f in folders.values_mut() {
            self.order(&mut f.entries);
        }
        self.folders = folders;
    }

    /// The folders shown read again when they changed (looked at every moment, not read).
    fn look(&mut self, dir: &Path) {
        if self.looked_at.is_some_and(|t| t.elapsed() < LOOK) {
            return;
        }
        self.looked_at = Some(Instant::now());
        let shown: Vec<PathBuf> = std::iter::once(dir.to_path_buf()).chain(self.open.iter().cloned()).collect();
        for d in shown {
            let now = std::fs::metadata(&d).and_then(|m| m.modified()).ok();
            if self.folders.get(&d).is_some_and(|f| f.changed != now) {
                self.folders.remove(&d);
            }
        }
    }

    /// The rows: the folder's entries shown (the data, or every file; matching the filter), and under each
    /// folder opened in place its own.
    fn rows(&mut self, dir: &Path) -> Vec<Row> {
        let mut out = Vec::new();
        self.rows_of(dir, 0, &mut out);
        out
    }

    fn rows_of(&mut self, dir: &Path, depth: usize, out: &mut Vec<Row>) {
        let (all, filter) = (self.all, self.filter.clone());
        let entries: Vec<Entry> = self
            .entries(dir)
            .entries
            .iter()
            .filter(|e| match e.kind {
                Kind::Folder => filter.trim().is_empty() || matches(&filter, &e.name) || depth > 0,
                // (a shapefile's `.dbf`, a raster's `.aux.xml`... among them)
                Kind::Other => all,
                _ => true,
            })
            .filter(|e| e.kind == Kind::Folder || matches(&filter, &e.name))
            .cloned()
            .collect();
        for e in entries {
            let opened = e.kind == Kind::Folder && self.open.contains(&e.path) && depth < 12;
            let path = e.path.clone();
            out.push(Row { entry: e, depth });
            if opened {
                self.rows_of(&path, depth + 1, out);
            }
        }
    }

    /// Choose an entry: it alone, one more (or one less: `add`), or the range from the last chosen (`range`).
    fn choose(&mut self, rows: &[Row], path: &Path, add: bool, range: bool) {
        let at = |p: &Path| rows.iter().position(|r| r.entry.path == p);
        match (range, self.anchor.as_deref().and_then(at), at(path)) {
            (true, Some(a), Some(b)) => {
                let (a, b) = (a.min(b), a.max(b));
                if !add {
                    self.chosen.clear();
                }
                for r in &rows[a..=b] {
                    if !self.chosen.contains(&r.entry.path) {
                        self.chosen.push(r.entry.path.clone());
                    }
                }
            }
            _ if add => {
                if let Some(k) = self.chosen.iter().position(|p| p == path) {
                    self.chosen.remove(k);
                } else {
                    self.chosen.push(path.to_path_buf());
                }
                self.anchor = Some(path.to_path_buf());
            }
            _ => {
                self.chosen = vec![path.to_path_buf()];
                self.anchor = Some(path.to_path_buf());
            }
        }
        self.cursor = Some(path.to_path_buf());
    }

    /// The details of the file chosen, when it is one raster or vector: read in a thread, once.
    fn describe(&mut self, rows: &[Row], ctx: &egui::Context) {
        let one = match self.chosen.as_slice() {
            [p] => rows.iter().find(|r| &r.entry.path == p).map(|r| (r.entry.path.clone(), r.entry.kind)),
            _ => None,
        };
        while let Ok((p, d)) = self.tell.try_recv() {
            if self.details.as_ref().is_some_and(|x| x.path == p) {
                self.details = Some(d);
            }
        }
        match one {
            Some((path, kind)) if kind.data() => {
                if self.details.as_ref().is_none_or(|d| d.path != path) {
                    self.details = Some(Details {
                        path: path.clone(),
                        kind,
                        facts: Vec::new(),
                        picture: None,
                        error: None,
                        reading: true,
                    });
                    let (tx, ctx) = (self.told.clone(), ctx.clone());
                    std::thread::spawn(move || {
                        let d = describe(path.clone(), kind);
                        let _ = tx.send((path, d));
                        ctx.request_repaint();
                    });
                }
            }
            _ => self.details = None,
        }
    }
}

/// The Files pane: what the user asked of the application (a model to open).
pub enum Asked {
    Open(PathBuf),
}

/// A button of the bar: a symbol, enabled or not, what it does.
fn tool(ui: &mut Ui, text: &str, enabled: bool, hover: &str) -> bool {
    ui.add_enabled(enabled, egui::Button::new(RichText::new(text).size(14.0)).min_size(vec2(24.0, 22.0)))
        .on_hover_text(hover)
        .clicked()
}

/// The Files pane.
pub fn pane(ui: &mut Ui, f: &mut Files, s: &mut Session, r: &Registry) -> Option<Asked> {
    let mut asked = None;
    let mut todo: Vec<Do> = Vec::new();
    let dir = f.dir(s.path.as_deref());
    f.look(&dir);
    let pane = ui.max_rect();
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    // the pane's keys are its own once it was clicked, until something else is
    let (pressed, origin) = ui.input(|i| (i.pointer.any_pressed(), i.pointer.press_origin()));
    if pressed && let Some(o) = origin {
        f.active = pane.contains(o);
    }
    let over = ui.input(|i| i.pointer.latest_pos()).is_some_and(|p| pane.contains(p));
    // the mouse's side buttons: back and forward
    if over {
        ui.input(|i| {
            if i.pointer.button_pressed(egui::PointerButton::Extra1) {
                todo.push(Do::Back);
            }
            if i.pointer.button_pressed(egui::PointerButton::Extra2) {
                todo.push(Do::Forward);
            }
        });
    }
    if (f.active || over) && !typing {
        ui.input_mut(|i| {
            if i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowLeft) {
                todo.push(Do::Back);
            }
            if i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowRight) {
                todo.push(Do::Forward);
            }
            if i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowUp) {
                todo.push(Do::Up);
            }
        });
    }
    let v = ui.visuals().clone();
    let weak = v.weak_text_color();
    egui::Frame::new().inner_margin(egui::Margin::symmetric(6, 6)).show(ui, |ui| {
        // the bar: back, forward, up, read again; the path (its folders as buttons, or typed); pinned
        ui.horizontal(|ui| {
            // (narrow buttons: the five fit a narrow pane)
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().button_padding.x = 4.0;
            if tool(ui, "\u{23f4}", !f.back.is_empty(), "back (alt+left)") {
                todo.push(Do::Back);
            }
            if tool(ui, "\u{23f5}", !f.forward.is_empty(), "forward (alt+right)") {
                todo.push(Do::Forward);
            }
            if tool(ui, "\u{23f6}", dir.parent().is_some(), "the folder above (alt+up)") {
                todo.push(Do::Up);
            }
            if tool(ui, "\u{27f3}", true, "read the folder again") {
                todo.push(Do::Refresh);
            }
            let pinned = crate::settings::get().files_pinned.contains(&dir);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let star = if pinned { "\u{2605}" } else { "\u{2606}" };
                let hover = if pinned { "take the folder from the places" } else { "pin the folder to the places" };
                if tool(ui, star, true, hover) {
                    todo.push(if pinned { Do::Unpin(dir.clone()) } else { Do::Pin(dir.clone()) });
                }
            });
        });
        ui.add_space(3.0);
        path_bar(ui, f, &dir, &mut todo);
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut f.filter)
                    .hint_text("filter: a name, or *.tif")
                    .desired_width(ui.available_width() - 64.0),
            );
            if resp.changed() {
                f.chosen.clear();
            }
            ui.checkbox(&mut f.all, "all").on_hover_text("every file, not only rasters, vectors and models");
        });
        ui.add_space(2.0);
        places(ui, f, s.path.as_deref(), &dir, &mut todo);
        let rows = f.rows(&dir);
        f.describe(&rows, ui.ctx());
        // the folder's summary, or what is chosen
        let n = |k: Kind| rows.iter().filter(|r| r.depth == 0 && r.entry.kind == k).count();
        let summary = if f.chosen.len() > 1 {
            format!("{} chosen: drag them to the model, or enter", f.chosen.len())
        } else {
            let mut parts = Vec::new();
            for (k, one, many) in [
                (Kind::Raster, "raster", "rasters"),
                (Kind::Vector, "vector", "vectors"),
                (Kind::Table, "table", "tables"),
                (Kind::Model, "model", "models"),
                (Kind::Folder, "folder", "folders"),
            ] {
                match n(k) {
                    0 => {}
                    1 => parts.push(format!("1 {one}")),
                    c => parts.push(format!("{c} {many}")),
                }
            }
            if parts.is_empty() { "no data here".into() } else { parts.join(" \u{b7} ") }
        };
        // the list, the summary, and the file chosen described under them (as high as the handle above it is
        // dragged)
        let width = ui.available_width();
        let room = ui.available_height();
        let below = if f.details.is_some() { f.split.clamp(120.0, (room - 160.0).max(120.0)) + 8.0 } else { 0.0 };
        let list_h = (room - below - 20.0).max(60.0);
        ui.allocate_ui(vec2(width, list_h), |ui| {
            if let Some(e) = f.folders.get(&dir).and_then(|x| x.error.clone()) {
                ui.colored_label(v.warn_fg_color, e);
            }
            list(ui, f, &rows, &mut todo);
        });
        ui.label(RichText::new(summary).size(11.0).color(weak));
        if f.details.is_some() {
            let (handle, resp) = ui.allocate_exact_size(vec2(width, 8.0), Sense::drag());
            let colour = if resp.hovered() || resp.dragged() {
                v.widgets.hovered.fg_stroke.color
            } else {
                v.widgets.noninteractive.bg_stroke.color
            };
            ui.painter().hline(handle.x_range(), handle.center().y, Stroke::new(1.0, colour));
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            if resp.dragged() {
                f.split = (f.split - resp.drag_delta().y).clamp(120.0, (room - 160.0).max(120.0));
            }
            ui.allocate_ui(vec2(width, below - 8.0), |ui| details(ui, f, &mut todo));
        }
        keys(ui, f, &rows, typing, &mut todo);
    });
    // the files carried, by the pointer
    if let Some(p) = egui::DragAndDrop::payload::<DraggedFile>(ui.ctx())
        && let Some(at) = ui.input(|i| i.pointer.latest_pos())
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let name = match p.0.as_slice() {
            [one] => one.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            many => format!("{} files", many.len()),
        };
        egui::Area::new(egui::Id::new("file carried"))
            .fixed_pos(at + vec2(14.0, 10.0))
            .order(egui::Order::Tooltip)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new(name).strong());
                    ui.label(RichText::new("to the model's canvas").weak().size(10.5));
                });
            });
    }
    for d in todo {
        match d {
            Do::Go(p) => f.go(p),
            Do::Back => f.go_back(),
            Do::Forward => f.go_forward(),
            Do::Up => {
                if let Some(up) = dir.parent() {
                    let from = dir.clone();
                    f.go(up.to_path_buf());
                    // the folder come from, chosen
                    f.chosen = vec![from.clone()];
                    f.cursor = Some(from);
                    f.reveal = true;
                }
            }
            Do::Expand(p) => {
                if !f.open.remove(&p) {
                    f.open.insert(p);
                }
            }
            Do::Choose { path, add, range } => {
                let rows = f.rows(&dir);
                f.choose(&rows, &path, add, range);
            }
            Do::Open(e) => match e.kind {
                Kind::Folder => f.go(e.path),
                Kind::Model => asked = Some(Asked::Open(e.path)),
                _ => {
                    if let Some(m) = add_files(s, r, &[e.path]) {
                        asked = Some(Asked::Open(m));
                    }
                }
            },
            Do::Add(paths) => {
                if let Some(m) = add_files(s, r, &paths) {
                    asked = Some(Asked::Open(m));
                }
            }
            Do::Stack(p) => {
                let path = p.to_string_lossy().into_owned();
                if let Err(m) = add_at(s, r, "raster.read", &[("path", path.into())], None) {
                    s.editor.message = Some(m);
                }
            }
            Do::Model(p) => asked = Some(Asked::Open(p)),
            Do::Pin(p) => crate::settings::update(|s| {
                if !s.files_pinned.contains(&p) {
                    s.files_pinned.push(p);
                }
            }),
            Do::Unpin(p) => crate::settings::update(|s| s.files_pinned.retain(|x| x != &p)),
            Do::Reveal(p) => reveal(&p),
            Do::Copy(p) => ui.ctx().copy_text(p.to_string_lossy().into_owned()),
            Do::Refresh => {
                f.folders.clear();
                f.drives = None;
            }
        }
    }
    asked
}

/// The path: its folders as buttons (the first ones in a menu when they do not fit); a click on the bar's empty
/// part (or ctrl+L over the pane) to type one, the folders that match offered under it.
fn path_bar(ui: &mut Ui, f: &mut Files, dir: &Path, todo: &mut Vec<Do>) {
    let over = ui.input(|i| i.pointer.latest_pos()).is_some_and(|p| ui.max_rect().contains(p));
    if f.typed.is_none()
        && (f.active || over)
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::L))
    {
        f.typed = Some((dir.to_string_lossy().into_owned(), true));
    }
    if let Some((mut text, focus)) = f.typed.clone() {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut text)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Small)
                .hint_text("a folder's path"),
        );
        if focus {
            resp.request_focus();
            if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), resp.id) {
                let all = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(text.chars().count()),
                );
                st.cursor.set_char_range(Some(all));
                st.store(ui.ctx(), resp.id);
            }
        }
        if f.offered.0 != text {
            f.offered = (text.clone(), offer(&text));
        }
        let (enter, escape, tab) = ui.input_mut(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
                resp.has_focus() && i.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
            )
        });
        if tab && let Some(first) = f.offered.1.first() {
            text = format!("{}{}", first.display(), std::path::MAIN_SEPARATOR);
            resp.request_focus();
        }
        let mut chosen = None;
        if resp.has_focus() || resp.lost_focus() {
            for p in f.offered.1.clone() {
                if ui.add(egui::Button::new(RichText::new(folder_name(&p)).size(12.0)).frame(false)).clicked() {
                    chosen = Some(p);
                }
            }
        }
        f.typed = Some((text.clone(), false));
        if let Some(p) = chosen {
            todo.push(Do::Go(p));
        } else if resp.lost_focus() {
            let p = PathBuf::from(text.trim());
            if enter && p.is_dir() {
                todo.push(Do::Go(p));
            } else if enter && p.is_file() {
                // a file: its folder, the file chosen
                if let Some(up) = p.parent() {
                    todo.push(Do::Go(up.to_path_buf()));
                }
            } else if escape || !enter {
                f.typed = None;
            }
        }
        return;
    }
    // (as tall as a button, whatever the text's size)
    let height = ui.spacing().interact_size.y.max(22.0);
    let (rect, bar) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    ui.painter().rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
    if bar.on_hover_text("click to type a path (ctrl+L)").clicked() {
        f.typed = Some((dir.to_string_lossy().into_owned(), true));
    }
    let crumbs = crumbs(dir);
    // the last folders that fit, the others in a menu
    let width = |t: &str| {
        ui.painter().layout_no_wrap(t.to_string(), egui::FontId::proportional(12.0), Color32::WHITE).size().x + 14.0
    };
    let mut room = rect.width() - 30.0;
    let mut first = crumbs.len();
    while first > 0 && (first == crumbs.len() || room - width(&crumbs[first - 1].0) > 0.0) {
        room -= width(&crumbs[first - 1].0);
        first -= 1;
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(3.0, 1.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 2.0;
    if first > 0 {
        child.menu_button(RichText::new("\u{2026}").size(12.0), |ui| {
            for (name, p) in crumbs[..first].iter().rev() {
                if ui.button(name).clicked() {
                    todo.push(Do::Go(p.clone()));
                    ui.close();
                }
            }
        });
    }
    for (k, (name, p)) in crumbs.iter().enumerate().skip(first) {
        if k > first || first > 0 {
            child.label(RichText::new("\u{203a}").size(12.0).weak());
        }
        let last = k + 1 == crumbs.len();
        let text = if last { RichText::new(name).size(12.0).strong() } else { RichText::new(name).size(12.0) };
        if child.add(egui::Button::new(text).frame(false).truncate()).clicked() && !last {
            todo.push(Do::Go(p.clone()));
        }
    }
}

/// The places, as small buttons in rows: the folders pinned, the model's, the user's, the drives, the folders
/// seen last.
fn places(ui: &mut Ui, f: &mut Files, model: Option<&Path>, dir: &Path, todo: &mut Vec<Do>) {
    let st = crate::settings::get();
    let drives = f.drives.get_or_insert_with(drives).clone();
    let place = |ui: &mut Ui, name: &str, p: &Path, pinned: bool, todo: &mut Vec<Do>| {
        let resp = ui.add(egui::Button::selectable(dir == p, RichText::new(name).size(11.5)));
        if resp.clicked() {
            todo.push(Do::Go(p.to_path_buf()));
        }
        resp.on_hover_text(p.display().to_string()).context_menu(|ui| {
            if pinned {
                if ui.button("Take from the places").clicked() {
                    todo.push(Do::Unpin(p.to_path_buf()));
                    ui.close();
                }
            } else if ui.button("Pin to the places").clicked() {
                todo.push(Do::Pin(p.to_path_buf()));
                ui.close();
            }
        });
    };
    let group = |ui: &mut Ui, title: &str, items: &[(String, PathBuf, bool)], todo: &mut Vec<Do>| {
        if items.is_empty() {
            return;
        }
        ui.label(RichText::new(title).size(10.5).weak());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
            for (name, p, pinned) in items {
                place(ui, name, p, *pinned, todo);
            }
        });
    };
    egui::CollapsingHeader::new(RichText::new("Places").size(12.0)).id_salt("files places").default_open(true).show(
        ui,
        |ui| {
            let pinned: Vec<(String, PathBuf, bool)> =
                st.files_pinned.iter().map(|p| (format!("\u{2605} {}", folder_name(p)), p.clone(), true)).collect();
            group(ui, "pinned", &pinned, todo);
            let mut here = Vec::new();
            if let Some(m) = model.and_then(Path::parent).filter(|m| !m.as_os_str().is_empty()) {
                here.push(("the model's".to_string(), m.to_path_buf(), false));
            }
            if let Some(h) = home() {
                for (name, p) in [
                    ("home", h.clone()),
                    ("documents", h.join("Documents")),
                    ("desktop", h.join("Desktop")),
                    ("downloads", h.join("Downloads")),
                ] {
                    if p.is_dir() {
                        here.push((name.to_string(), p, false));
                    }
                }
            }
            here.extend(drives.iter().map(|d| (d.to_string_lossy().into_owned(), d.clone(), false)));
            group(ui, "this computer", &here, todo);
            let recent: Vec<(String, PathBuf, bool)> = st
                .files_recent
                .iter()
                .filter(|p| p.as_path() != dir)
                .take(6)
                .map(|p| (folder_name(p), p.clone(), false))
                .collect();
            group(ui, "seen last", &recent, todo);
        },
    );
}

/// The list: a header to sort by, and the rows (only those in view are laid out).
fn list(ui: &mut Ui, f: &mut Files, rows: &[Row], todo: &mut Vec<Do>) {
    let v = ui.visuals().clone();
    let weak = v.weak_text_color();
    let width = ui.available_width();
    // the columns there is room for: the size from 260 points, the age from 360
    let size_w = if width >= 260.0 { 70.0 } else { 0.0 };
    let age_w = if width >= 360.0 { 90.0 } else { 0.0 };
    let (head, _) = ui.allocate_exact_size(vec2(width, 18.0), Sense::hover());
    let header = |ui: &mut Ui, r: Rect, name: &str, s: Sort, right: bool, f: &mut Files| {
        let arrow = if f.sort == s { if f.ascending { " \u{23f6}" } else { " \u{23f7}" } } else { "" };
        let resp = ui.interact(r, ui.id().with(("sort", name)), Sense::click());
        let colour = if resp.hovered() { v.text_color() } else { weak };
        let (at, align) = if right {
            (r.right_center() - vec2(6.0, 0.0), egui::Align2::RIGHT_CENTER)
        } else {
            (r.left_center() + vec2(6.0, 0.0), egui::Align2::LEFT_CENTER)
        };
        ui.painter().text(at, align, format!("{name}{arrow}"), egui::FontId::proportional(11.0), colour);
        if resp.on_hover_text("sort by it (again: the other way)").clicked() {
            f.sort_by(s);
        }
    };
    let name_r = Rect::from_min_max(head.min, pos2(head.right() - size_w - age_w, head.bottom()));
    header(ui, name_r, "name", Sort::Name, false, f);
    if size_w > 0.0 {
        let r = Rect::from_min_size(pos2(name_r.right(), head.top()), vec2(size_w, head.height()));
        header(ui, r, "size", Sort::Size, true, f);
    }
    if age_w > 0.0 {
        let r = Rect::from_min_size(pos2(head.right() - age_w, head.top()), vec2(age_w, head.height()));
        header(ui, r, "changed", Sort::Modified, true, f);
    }
    ui.painter().hline(head.x_range(), head.bottom(), Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color));
    // the modifiers of the click itself (they may be let go in the same frame)
    let modifiers = ui.input(|i| {
        i.events
            .iter()
            .rev()
            .find_map(|e| match e {
                egui::Event::PointerButton { button: egui::PointerButton::Primary, modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or(i.modifiers)
    });
    let cursor_row = f.cursor.as_ref().and_then(|c| rows.iter().position(|r| &r.entry.path == c));
    let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt("files list");
    if f.reveal
        && let Some(k) = cursor_row
    {
        // (the row in the middle of the view, when it is out of it)
        let offset = ui.data(|d| d.get_temp::<f32>(ui.id().with("files offset"))).unwrap_or(0.0);
        let shown = ui.available_height().max(ROW);
        let y = k as f32 * ROW;
        if y < offset || y + ROW > offset + shown {
            area = area.vertical_scroll_offset((y - shown / 2.0).max(0.0));
        }
        f.reveal = false;
    }
    let out = area.show_rows(ui, ROW, rows.len(), |ui, range| {
        for k in range {
            let row = &rows[k];
            let e = &row.entry;
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click_and_drag());
            let chosen = f.chosen.contains(&e.path);
            let p = ui.painter_at(rect);
            if chosen {
                p.rect_filled(
                    rect,
                    crate::theme::rounded(2.0),
                    v.selection.bg_fill.gamma_multiply(if f.active { 0.9 } else { 0.5 }),
                );
            } else if resp.hovered() {
                p.rect_filled(rect, crate::theme::rounded(2.0), v.widgets.hovered.weak_bg_fill);
            }
            if cursor_row == Some(k) && f.active {
                p.rect_stroke(
                    rect.shrink(0.5),
                    2.0,
                    Stroke::new(1.0, v.selection.stroke.color),
                    egui::StrokeKind::Inside,
                );
            }
            let text_colour = if chosen {
                v.selection.stroke.color
            } else {
                match e.kind {
                    Kind::Other => weak,
                    _ => v.text_color(),
                }
            };
            let x = rect.left() + 4.0 + row.depth as f32 * 14.0;
            // a folder's arrow: it opens in place
            let arrow = Rect::from_center_size(pos2(x + 6.0, rect.center().y), Vec2::splat(12.0));
            if e.kind == Kind::Folder {
                let open = f.open.contains(&e.path);
                p.text(
                    arrow.center(),
                    egui::Align2::CENTER_CENTER,
                    if open { "\u{23f7}" } else { "\u{23f5}" },
                    egui::FontId::proportional(12.0),
                    if chosen { text_colour } else { weak },
                );
            }
            icon(&p, Rect::from_min_size(pos2(x + 14.0, rect.center().y - 7.5), Vec2::splat(15.0)), e.kind, &v);
            let name_right = rect.right() - size_w - age_w - 4.0;
            let name_at = pos2(x + 35.0, rect.center().y);
            // (one line, cut with an ellipsis where the column ends)
            let mut job =
                egui::text::LayoutJob::simple_singleline(e.name.clone(), egui::FontId::proportional(13.0), text_colour);
            job.wrap = egui::text::TextWrapping::truncate_at_width((name_right - name_at.x).max(10.0));
            let galley = ui.painter().layout_job(job);
            p.galley(pos2(name_at.x, rect.center().y - galley.size().y / 2.0), galley, text_colour);
            let small = egui::FontId::proportional(11.0);
            let side = if chosen { text_colour } else { weak };
            if size_w > 0.0 && e.kind != Kind::Folder {
                let at = pos2(rect.right() - age_w - 6.0, rect.center().y);
                p.text(at, egui::Align2::RIGHT_CENTER, size_text(e.size), small.clone(), side);
            }
            if age_w > 0.0
                && let Some(t) = e.modified
            {
                p.text(pos2(rect.right() - 6.0, rect.center().y), egui::Align2::RIGHT_CENTER, age_text(t), small, side);
            }
            resp.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, chosen, e.name.clone())
            });
            // the data chosen dragged: all of it, else this one
            if e.kind.data() {
                let carried: Vec<PathBuf> = if chosen {
                    let data: Vec<PathBuf> = rows
                        .iter()
                        .filter(|r| r.entry.kind.data() && f.chosen.contains(&r.entry.path))
                        .map(|r| r.entry.path.clone())
                        .collect();
                    data
                } else {
                    vec![e.path.clone()]
                };
                resp.dnd_set_drag_payload(DraggedFile(carried));
            }
            let on_arrow =
                e.kind == Kind::Folder && resp.interact_pointer_pos().is_some_and(|q| arrow.expand(3.0).contains(q));
            if resp.clicked() {
                if on_arrow {
                    todo.push(Do::Expand(e.path.clone()));
                } else {
                    todo.push(Do::Choose { path: e.path.clone(), add: modifiers.command, range: modifiers.shift });
                }
            }
            if resp.double_clicked() && !on_arrow {
                todo.push(Do::Open(e.clone()));
            }
            if resp.secondary_clicked() && !chosen {
                todo.push(Do::Choose { path: e.path.clone(), add: false, range: false });
            }
            let hover = match e.kind {
                Kind::Folder => "double-click to go into it, its arrow to open it here",
                Kind::Raster | Kind::Vector | Kind::Table => {
                    "drag it to the model, or double-click it, for a node that reads it"
                }
                Kind::Model => "double-click to open the model",
                Kind::Other => "",
            };
            let resp = if hover.is_empty() { resp } else { resp.on_hover_text(hover) };
            resp.context_menu(|ui| menu(ui, f, rows, e, todo));
        }
    });
    ui.data_mut(|d| d.insert_temp(ui.id().with("files offset"), out.state.offset.y));
    if rows.is_empty() {
        ui.label(
            RichText::new(if f.filter.trim().is_empty() { "no data in this folder" } else { "nothing matches" }).weak(),
        );
    }
}

/// What a right click on an entry offers.
fn menu(ui: &mut Ui, f: &Files, rows: &[Row], e: &Entry, todo: &mut Vec<Do>) {
    let data: Vec<PathBuf> = rows
        .iter()
        .filter(|r| r.entry.kind.data() && f.chosen.contains(&r.entry.path))
        .map(|r| r.entry.path.clone())
        .collect();
    match e.kind {
        Kind::Folder => {
            if ui.button("Go into it").clicked() {
                todo.push(Do::Go(e.path.clone()));
                ui.close();
            }
            if ui.button("Read its rasters as one stack").clicked() {
                todo.push(Do::Stack(e.path.clone()));
                ui.close();
            }
            let pinned = crate::settings::get().files_pinned.contains(&e.path);
            if !pinned && ui.button("Pin to the places").clicked() {
                todo.push(Do::Pin(e.path.clone()));
                ui.close();
            }
        }
        Kind::Model => {
            if ui.button("Open the model").clicked() {
                todo.push(Do::Model(e.path.clone()));
                ui.close();
            }
        }
        _ => {
            let label =
                if data.len() > 1 { format!("Add the {} to the model", data.len()) } else { "Add to the model".into() };
            if ui.button(label).clicked() {
                todo.push(Do::Add(if data.len() > 1 { data } else { vec![e.path.clone()] }));
                ui.close();
            }
        }
    }
    ui.separator();
    if ui.button("Copy the path").clicked() {
        todo.push(Do::Copy(e.path.clone()));
        ui.close();
    }
    let manager = if cfg!(windows) {
        "Show in Explorer"
    } else if cfg!(target_os = "macos") {
        "Show in Finder"
    } else {
        "Show in the file manager"
    };
    if ui.button(manager).clicked() {
        todo.push(Do::Reveal(e.path.clone()));
        ui.close();
    }
}

/// The keys of the list (the pane active, nothing typed in): the arrows go through the rows (shift: choosing
/// them), right and left open and close a folder in place, enter takes what is chosen, letters go to a name.
fn keys(ui: &mut Ui, f: &mut Files, rows: &[Row], typing: bool, todo: &mut Vec<Do>) {
    if !f.active || typing || rows.is_empty() {
        return;
    }
    let (up, down, right, left, enter, shift, text) = ui.input_mut(|i| {
        let shift = i.modifiers.shift;
        let m = if shift { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE };
        let text: String = i
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Text(t) if !i.modifiers.command => Some(t.clone()),
                _ => None,
            })
            .collect();
        (
            i.consume_key(m, egui::Key::ArrowUp),
            i.consume_key(m, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            shift,
            text,
        )
    });
    let at = f.cursor.as_ref().and_then(|c| rows.iter().position(|r| &r.entry.path == c));
    let step = |k: Option<usize>, d: isize| -> usize {
        match k {
            Some(k) => (k as isize + d).clamp(0, rows.len() as isize - 1) as usize,
            None => 0,
        }
    };
    if up || down {
        let k = step(at, if up { -1 } else { 1 });
        todo.push(Do::Choose { path: rows[k].entry.path.clone(), add: false, range: shift });
        f.reveal = true;
    }
    if let Some(k) = at {
        let e = &rows[k].entry;
        if right && e.kind == Kind::Folder {
            if f.open.contains(&e.path) {
                if rows.get(k + 1).is_some_and(|r| r.depth > rows[k].depth) {
                    todo.push(Do::Choose { path: rows[k + 1].entry.path.clone(), add: false, range: false });
                }
            } else {
                todo.push(Do::Expand(e.path.clone()));
            }
        }
        if left {
            if e.kind == Kind::Folder && f.open.contains(&e.path) {
                todo.push(Do::Expand(e.path.clone()));
            } else if let Some(parent) = rows[..k].iter().rev().find(|r| r.depth < rows[k].depth) {
                todo.push(Do::Choose { path: parent.entry.path.clone(), add: false, range: false });
                f.reveal = true;
            }
        }
        if enter {
            let data: Vec<PathBuf> = rows
                .iter()
                .filter(|r| r.entry.kind.data() && f.chosen.contains(&r.entry.path))
                .map(|r| r.entry.path.clone())
                .collect();
            if data.len() > 1 {
                todo.push(Do::Add(data));
            } else {
                todo.push(Do::Open(e.clone()));
            }
        }
    }
    // the first letters of a name: to it
    if !text.trim().is_empty() {
        let fresh = f.letters.1.is_none_or(|t| t.elapsed() > Duration::from_millis(900));
        if fresh {
            f.letters.0.clear();
        }
        f.letters.0.push_str(&text.to_lowercase());
        f.letters.1 = Some(Instant::now());
        let typed = f.letters.0.clone();
        let from = if fresh { at.map_or(0, |k| k + 1) } else { at.unwrap_or(0) };
        let found = (0..rows.len())
            .map(|i| (from + i) % rows.len())
            .find(|&i| rows[i].entry.name.to_lowercase().starts_with(&typed));
        if let Some(i) = found {
            todo.push(Do::Choose { path: rows[i].entry.path.clone(), add: false, range: false });
            f.reveal = true;
        }
    }
}

/// The file chosen described: its picture, what it is, a button to take it.
fn details(ui: &mut Ui, f: &mut Files, todo: &mut Vec<Do>) {
    let Some(d) = &f.details else { return };
    let v = ui.visuals().clone();
    let name = folder_name(&d.path);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
        icon(ui.painter(), r, d.kind, &v);
        ui.add(egui::Label::new(RichText::new(&name).strong()).truncate());
    });
    if d.reading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("reading...").weak());
        });
        return;
    }
    if let Some(e) = &d.error {
        ui.colored_label(v.warn_fg_color, e);
    }
    let path = d.path.clone();
    egui::ScrollArea::vertical().id_salt("files details").auto_shrink([false, false]).show(ui, |ui| {
        let width = ui.available_width();
        match &d.picture {
            Some(Picture::Image(img)) => {
                let tex = match &f.texture {
                    Some((p, t)) if *p == path => t.clone(),
                    _ => {
                        let t = ui.ctx().load_texture("files picture", img.clone(), egui::TextureOptions::NEAREST);
                        f.texture = Some((path.clone(), t.clone()));
                        t
                    }
                };
                let [w, h] = img.size;
                let scale = (width / w as f32).min(160.0 / h as f32);
                let size = vec2(w as f32, h as f32) * scale;
                ui.vertical_centered(|ui| ui.image((tex.id(), size)));
            }
            Some(Picture::Shapes { lines, points, aspect }) => {
                let h = (width / aspect.max(0.2)).min(160.0);
                let (r, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
                let side = (r.width()).min(r.height() * aspect).max(1.0);
                let box_ = Rect::from_center_size(r.center(), vec2(side, side / aspect.max(1e-3)));
                let scale = side;
                let colour = crate::editor_view::type_colour(Type::Vector, v.dark_mode).unwrap_or(v.text_color());
                let at = |q: &[f32; 2]| pos2(box_.left() + q[0] * scale, box_.top() + q[1] * scale);
                let painter = ui.painter_at(r);
                for l in lines {
                    painter.add(egui::Shape::line(l.iter().map(at).collect(), Stroke::new(1.0, colour)));
                }
                for q in points {
                    painter.circle_filled(at(q), 1.8, colour);
                }
            }
            None => {}
        }
        ui.add_space(4.0);
        egui::Grid::new("files facts").num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
            for (k, val) in &d.facts {
                ui.label(RichText::new(k).size(11.0).weak());
                let short: String = val.chars().take(160).collect();
                ui.add(egui::Label::new(RichText::new(short).size(11.0)).wrap()).on_hover_text(val);
                ui.end_row();
            }
        });
        ui.add_space(4.0);
        if ui.button("Add to the model").on_hover_text("a node that reads it, after the node selected").clicked() {
            todo.push(Do::Add(vec![path.clone()]));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_sort_and_match_as_people_expect() {
        let mut names = vec!["map10.tif", "Map2.tif", "map1.tif", "b.gpkg"];
        names.sort_by(|a, b| natural(a, b));
        assert_eq!(names, ["b.gpkg", "map1.tif", "Map2.tif", "map10.tif"]);
        assert!(matches("*.tif", "Land.TIF") && !matches("*.tif", "land.gpkg"));
        assert!(matches("land", "my_landuse.tif") && matches("l?nd*", "land.tif"));
        assert!(matches("", "anything"));
    }

    #[test]
    fn paths_split_into_their_folders() {
        let p = if cfg!(windows) { PathBuf::from(r"C:\data\maps") } else { PathBuf::from("/data/maps") };
        let c = crumbs(&p);
        assert_eq!(c.len(), 3);
        assert_eq!(c[2].0, "maps");
        assert_eq!(c[2].1, p);
        assert_eq!(c[1].1, p.parent().unwrap());
    }

    #[test]
    fn ages_read_well() {
        let now = SystemTime::now();
        assert_eq!(age_text(now), "now");
        assert_eq!(age_text(now - Duration::from_secs(300)), "5 min ago");
        assert_eq!(age_text(SystemTime::UNIX_EPOCH + Duration::from_secs(86_400 * 365)), "1971-01-01");
    }
}
