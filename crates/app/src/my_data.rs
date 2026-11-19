//! "Use my data", the engine: a model of any template, its drawn data (the nodes that stand for the user's: `data`,
//! the `nlm.*`) replaced by the user's files, as one change of the model and no window (the window is another
//! stage).
//!
//! [`sources`] lists what a model draws that a file can replace; [`plan`] makes a copy of the model with the files
//! chosen: each a model's input (so [`check`] reads it as the data of any model is read: the same findings), a
//! `raster.match` to the reference grid where its grid differs, a `raster.reclassify` of the table of classes
//! (the user's class -> the template's), a `raster.legend` with the template's names; the drawn node taken away
//! and what read it reading the end of the chain; the `nlm.*` left drawn on the reference grid.
//!
//! Decisions (see the report of the stage in IDEAS.md):
//! - The grids are compared by the files' headers: a `raster.match` is put only where the grid or the CRS differs
//!   from the reference's (`near` for classes and masks, `bilinear` for values, `auto` for the rest).
//! - `data` has ONE place: the LAST node of the chain (what the rest of the model reads) and, where the chain has
//!   no node (a file read straight as an input), the input itself (`data` of [`Input`]). A second run of Use my
//!   data finds it there ([`sources`] offers the nodes that carry it and the inputs that carry it with a file:
//!   [`Source::file`]) and takes the first chain away.
//! - Classes the table does not map, and a class mapped to "no data", become no data (`* -> nan` ends the rules).
//! - An `nlm.*` that is left is put on the reference grid (`like`; rows, columns, resolution and crs taken away);
//!   `nlm.curds` has no `like` and stays on its own grid, named in the notes.
//! - A stack is one file's bands (`Choice::bands`); a stack of several files is not offered yet.
//! - An image (data of kind `image`, or an `nlm.image`'s `image` output) is one multiband file: a `stack.rename` names
//!   its bands by their roles (blue, green, red, nir, swir1, swir2; guessed from the names, else `other`, dropped) and
//!   scales the reflectance to 0-1 (detected: none, divided by 10 000, Landsat collection 2); no node when the bands are
//!   named already and the scale is 0-1. An image can also be a folder (or a selection) of single-band files ([`Choice::files`]:
//!   sorted by band number, each an input, the coarser put on the finest by `raster.match`, a `stack.of`), or a cloud-free
//!   Sentinel-2 scene of an area ([`Scene`]: a `data.stac` node, nothing downloaded before the model runs).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::Value as Json;
use terraflux_exec::lint::{Finding, Level};
use terraflux_graph::{Data, DataKind, Input, Model, Node, Parameter, Ref, Severity, Source as Wire, Type};
use terraflux_ops::Registry;
use terraflux_raster::Grid;

use crate::editor;
use crate::rgb::Scale;

/// The `op` of a [`Source`] that is an input of the model (`$name`) with no file.
pub const INPUT: &str = "input";

/// What a model draws or leaves empty that a file of the user's can take the place of.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    /// The node (or the input, for [`INPUT`]).
    pub node: String,
    pub op: String,
    /// What it stands for, when the node says it.
    pub data: Option<Data>,
    /// What it is in a phrase: the data's, else the node's note, else its operator.
    pub what: String,
    /// The output port the rest of the model reads (an input: empty).
    pub port: String,
    /// A map or a stack, from what reads it.
    pub needs: Type,
    /// The file of an input that already holds the user's (a second run replaces it); none for what is drawn.
    pub file: Option<String>,
}

/// What the user chose for a source.
#[derive(Debug, Clone, Default)]
pub struct Choice {
    pub file: PathBuf,
    /// The band of a map, by name or 0-based index (none: the first).
    pub band: Option<String>,
    /// The bands of a stack of one file (none: all of them).
    pub bands: Option<Vec<String>>,
    /// The user's class -> the template's (none: no data), for the data of classes and masks.
    pub crosswalk: BTreeMap<i64, Option<i64>>,
    pub nodata: Option<f64>,
    /// For the first date of CLUE-S's model, with a file of three bands or more: every band of the file is a date
    /// (the demand reads them all as a stack; the simulation starts from the bands chosen for the two date sources).
    /// No table of classes with it (`raster.reclassify` takes a map, not a stack).
    pub dates: bool,
    /// For an image: the role of each band of the file (blue, green, red, nir, swir1, swir2 or other; the others are
    /// dropped), in the band order. None: guessed from the bands' names ([`crate::rgb::band_roles`]).
    pub roles: Option<Vec<String>>,
    /// For an image: the scale of its reflectance. None: detected from the file ([`image_info`]).
    pub scale: Option<Scale>,
    /// For an image of single-band files: the files in band order (see [`band_files`]); `file` is the first.
    pub files: Vec<PathBuf>,
    /// For an image with no file of the user's: a cloud-free Sentinel-2 scene of an area, read by `data.stac`.
    pub scene: Option<Scene>,
}

/// The assets of Earth Search's Sentinel-2 L2A a scene reads, in the order of their roles (blue, green, red, nir,
/// swir1, swir2: B02, B03, B04, B08, B11, B12).
pub const SCENE_ASSETS: [&str; 6] = ["blue", "green", "red", "nir", "swir16", "swir22"];

/// The preset of the data catalog a scene reads.
const SCENE_PRESET: &str = "es-sentinel-2-l2a";

/// A cloud-free Sentinel-2 scene of an area, for an image: the area, the cells and the dates.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// West, south, east, north, in degrees.
    pub extent: [f64; 4],
    /// The cells' side in degrees (0.00009 is about 10 m).
    pub cellsize: f64,
    /// A day or an interval (2025-11-19/2026-11-19), as `data.stac`'s `datetime`.
    pub dates: String,
}

/// The cells' sides a scene offers, in degrees: 10, 20 and 30 m at the equator.
pub const SCENE_CELLS: [(f64, &str); 3] = [(0.00009, "10 m"), (0.00018, "20 m"), (0.00027, "30 m")];

impl Scene {
    /// An area, the last 12 months, 10 m cells.
    pub fn new(extent: [f64; 4]) -> Self {
        Scene { extent, cellsize: SCENE_CELLS[0].0, dates: last_year() }
    }

    /// The grid the scene is read on: the extent in whole cells.
    pub fn grid(&self) -> Grid {
        let mut g = Grid::new(self.cols(), self.rows());
        g.transform = [self.extent[0], self.cellsize, 0.0, self.extent[3], 0.0, -self.cellsize];
        g
    }

    pub fn cols(&self) -> usize {
        ((self.extent[2] - self.extent[0]) / self.cellsize).round().max(1.0) as usize
    }

    pub fn rows(&self) -> usize {
        ((self.extent[3] - self.extent[1]) / self.cellsize).round().max(1.0) as usize
    }

    /// Why the scene cannot be asked for, if so.
    pub fn problem(&self) -> Option<String> {
        let [w, s, e, n] = self.extent;
        let bad = !self.extent.iter().all(|x| x.is_finite())
            || e <= w
            || n <= s
            || w < -180.0
            || e > 180.0
            || s < -90.0
            || n > 90.0;
        if bad {
            return Some("the area: west below east and south below north, in degrees".into());
        }
        if !(self.cellsize.is_finite() && self.cellsize > 0.0) {
            return Some("the cells: above 0".into());
        }
        if self.cols() * self.rows() > 100_000_000 {
            return Some(format!(
                "the area is {} x {} cells: choose a smaller one or larger cells",
                self.cols(),
                self.rows()
            ));
        }
        let parts: Vec<&str> = self.dates.trim().split('/').map(str::trim).collect();
        let one = |d: &&str| *d == ".." || terraflux_ops::data::parse_date(d).is_ok();
        if self.dates.trim().is_empty() || parts.len() > 2 || !parts.iter().all(one) {
            return Some("the dates: a day (2025-06-01) or an interval (2025-06-01/2025-08-31)".into());
        }
        None
    }
}

/// The last 12 months as an interval: a year ago today, to today.
pub fn last_year() -> String {
    let end = crate::report::today();
    let year: i64 = end[..4].parse().unwrap_or(2026);
    let day = if &end[5..] == "02-29" { "02-28" } else { &end[5..] };
    format!("{}-{day}/{end}", year - 1)
}

/// The band files of a folder (or of a selection) of an image's bands, in band order: with a folder, the `.tif`,
/// `.tiff` and `.jp2` files under it (three levels down) that have a band number in their names (B02, B8A, SR_B4:
/// the others, a true colour image or a scene classification, are left out), one for each band, the finest when
/// several resolutions are there (`_10m` before `_20m`); with files, those, sorted.
pub fn band_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    fn walk(d: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<(), String> {
        let rd = std::fs::read_dir(d).map_err(|e| format!("{}: {e}", d.display()))?;
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if depth < 3 {
                    walk(&p, depth + 1, out)?;
                }
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| ["tif", "tiff", "jp2"].contains(&x.to_lowercase().as_str()))
            {
                out.push(p);
            }
        }
        Ok(())
    }
    let folder = paths.len() == 1 && paths[0].is_dir();
    let mut found: Vec<PathBuf> = Vec::new();
    if folder {
        walk(&paths[0], 0, &mut found)?;
        found.retain(|p| crate::rgb::band_id(&stem(p)).is_some());
    } else {
        found = paths.to_vec();
    }
    // the band, then the finest resolution, then the name
    found.sort_by_key(|p| (band_rank(p), resolution(p), stem(p)));
    if folder {
        found.dedup_by_key(|p| band_rank(p));
    }
    if found.is_empty() {
        return Err("no single-band file with a band number in its name (B02, B8A, SR_B4...) there".into());
    }
    Ok(found)
}

/// A file's name without its extension.
pub fn stem(p: &Path) -> String {
    p.file_stem().map_or_else(|| p.display().to_string(), |s| s.to_string_lossy().into_owned())
}

/// The place of a band file among the bands: its number twice, one more for an `A` (B8 before B8A before B9), those
/// without one last.
fn band_rank(p: &Path) -> u32 {
    crate::rgb::band_id(&stem(p)).map_or(u32::MAX, |id| {
        let n: u32 = id.trim_end_matches('a').parse().unwrap_or(999);
        n * 2 + u32::from(id.ends_with('a'))
    })
}

/// The resolution a name says (`_10m`), 0 when none.
fn resolution(p: &Path) -> u32 {
    stem(p)
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .find_map(|w| w.strip_suffix('m').and_then(|n| n.parse::<u32>().ok()))
        .unwrap_or(0)
}

/// A band file's short name for an input: its band word (`b02`, `b4`), else the name made of letters and digits.
fn band_label(stem: &str) -> String {
    let lower = stem.to_lowercase();
    let word = lower.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).find(|w| {
        w.strip_prefix("band")
            .or_else(|| w.strip_prefix('b'))
            .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
    });
    word.map_or_else(|| lower.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect(), String::from)
}

/// What the files of an image's bands say: the index of the finest grid among them, the grids, and the names (the
/// stems).
fn files_header(files: &[PathBuf]) -> Result<(usize, Vec<Grid>, Vec<String>), String> {
    let mut grids = Vec::new();
    for f in files {
        let (g, bands) = header(f)?;
        if bands.len() != 1 {
            return Err(format!(
                "{} has {} bands: a file for each band takes single-band files",
                f.display(),
                bands.len()
            ));
        }
        grids.push(g);
    }
    let cell = |g: &Grid| g.transform[1].abs();
    let finest = (0..grids.len()).fold(0, |b, k| if cell(&grids[k]) < cell(&grids[b]) - 1e-12 { k } else { b });
    Ok((finest, grids, files.iter().map(|f| stem(f)).collect()))
}

/// The scale of the reflectance of single-band files: the one their values and names tell.
pub fn files_scale(files: &[PathBuf]) -> Result<Scale, String> {
    let mut out = Scale::None;
    for f in files {
        match image_scale(f, &[stem(f)])? {
            Scale::None => {}
            Scale::LandsatC2 => return Ok(Scale::LandsatC2),
            s => out = s,
        }
    }
    Ok(out)
}

/// The model with the user's data, what was done, and what was not.
#[derive(Debug, Clone)]
pub struct Plan {
    pub model: Model,
    /// What was done, a line each.
    pub notes: Vec<String>,
    /// The choices that were not made, with the reason.
    pub refused: Vec<String>,
    /// The inputs made for the files.
    pub inputs: Vec<String>,
    /// The input whose grid is the reference.
    pub reference: String,
}

/// The references a node reads (its inputs, settings and body).
fn reads(n: &Node) -> Vec<String> {
    let mut t = Model::new("t");
    t.nodes.push(n.clone());
    let mut out = Vec::new();
    editor::for_each_ref(&mut t, &[], &mut |r| out.push(r.clone()));
    out
}

/// What the model's outputs and Plots read.
fn outside_reads(m: &Model) -> Vec<String> {
    let mut t = m.clone();
    t.nodes.clear();
    let mut out = Vec::new();
    editor::for_each_ref(&mut t, &[], &mut |r| out.push(r.clone()));
    out
}

/// Who reads a node: the nodes (with the input port), the output ports they read, and whether the outputs or the
/// Plots do.
struct Readers {
    nodes: Vec<(String, Option<String>)>,
    ports: BTreeSet<String>,
    outside: bool,
}

fn readers(m: &Model, id: &str) -> Readers {
    let port_of = |r: &str| match Ref::parse(r) {
        Ok(Ref::Port { node, port }) if node == id => Some(port),
        _ => None,
    };
    let mut out = Readers { nodes: Vec::new(), ports: BTreeSet::new(), outside: false };
    for n in m.nodes.iter().filter(|n| n.id != id) {
        for r in reads(n) {
            if let Some(p) = port_of(&r) {
                out.ports.insert(p);
                let at = n.inputs.iter().find(|(_, s)| matches!(s, Wire::Ref(x) if *x == r)).map(|(k, _)| k.clone());
                out.nodes.push((n.id.clone(), at));
            }
        }
    }
    for r in outside_reads(m) {
        if let Some(p) = port_of(&r) {
            out.ports.insert(p);
            out.outside = true;
        }
    }
    out
}

fn source_of(m: &Model, r: &Registry, n: &Node) -> Source {
    let rd = readers(m, &n.id);
    let sig = r.sig(&n.op);
    let port = rd
        .ports
        .iter()
        .next()
        .cloned()
        .or_else(|| sig.and_then(|s| s.outputs.first().map(|o| o.name.clone())))
        .unwrap_or_else(|| "map".to_string());
    let stack_out = sig.and_then(|s| s.outputs.iter().find(|o| o.name == port)).is_some_and(|o| o.ty == Type::Stack);
    let stack_in = rd.nodes.iter().any(|(c, p)| {
        let Some(p) = p else { return false };
        let sig = m.node(c).and_then(|c| r.sig(&c.op));
        sig.and_then(|s| s.inputs.iter().find(|i| &i.name == p)).is_some_and(|i| i.ty == Type::Stack)
    });
    let stack =
        stack_out || stack_in || n.data.as_ref().is_some_and(|d| matches!(d.kind, DataKind::Stack | DataKind::Image));
    let what = n
        .data
        .as_ref()
        .map(|d| d.what.clone())
        .or_else(|| n.note.clone())
        .unwrap_or_else(|| format!("a map drawn by {}", n.op));
    Source {
        node: n.id.clone(),
        op: n.op.clone(),
        data: n.data.clone(),
        what,
        port,
        needs: if stack { Type::Stack } else { Type::Map },
        file: None,
    }
}

/// The data of the model a file can replace, at its top level: the nodes that stand for the user's data (`data`) in
/// the model's order, then the `nlm.*` without the mark that something other than a node of `data` reads (the
/// ones only such nodes read are inside a drawn map, as the suitability of a simulated change), the `raster.read`
/// without a file, and the model's inputs without a path or that carry `data` (the user's file, read straight).
pub fn sources(m: &Model, r: &Registry) -> Vec<Source> {
    let mut out: Vec<Source> = m.nodes.iter().filter(|n| n.data.is_some()).map(|n| source_of(m, r, n)).collect();
    for n in m.nodes.iter().filter(|n| n.data.is_none()) {
        let loose = if n.op.starts_with("nlm.") {
            let rd = readers(m, &n.id);
            let inside = !rd.outside
                && !rd.nodes.is_empty()
                && rd.nodes.iter().all(|(c, _)| m.node(c).is_some_and(|c| c.data.is_some()));
            !inside
        } else {
            n.op == "raster.read" && n.params.get("path").and_then(|p| p.as_str()).is_none_or(|p| p.trim().is_empty())
        };
        if loose {
            out.push(source_of(m, r, n));
        }
    }
    for (name, input) in &m.inputs {
        let (path, needs, note) = match input {
            Input::Map { path, note, .. } => (path, Type::Map, note),
            Input::Stack { path, note, .. } => (path, Type::Stack, note),
            Input::Value { .. } => continue,
        };
        let data = input.data().cloned();
        let empty = path.trim().is_empty();
        if empty || data.is_some() {
            let what = match (&data, note) {
                (Some(d), _) => d.what.clone(),
                (None, Some(n)) => n.clone(),
                _ => format!("the input ${name}"),
            };
            out.push(Source {
                node: name.clone(),
                op: INPUT.to_string(),
                data,
                what,
                port: String::new(),
                needs,
                file: (!empty).then(|| path.clone()),
            });
        }
    }
    out
}

/// A name not taken (`base`, else `base_2`...), taken from now on.
fn fresh(taken: &mut BTreeSet<String>, base: &str) -> String {
    let id = if !taken.contains(base) {
        base.to_string()
    } else {
        (2..).map(|k| format!("{base}_{k}")).find(|id| !taken.contains(id)).unwrap()
    };
    taken.insert(id.clone());
    id
}

/// The header of a file: its grid and band names.
fn header(path: &Path) -> Result<(Grid, Vec<String>), String> {
    let f = terraflux_ops::files::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let meta = f.meta();
    Ok((meta.grid.clone(), meta.band_names()))
}

/// The roles of an image's bands as a choice has them: the user's, else guessed from the bands' names.
pub fn image_roles(c: &Choice, names: &[String]) -> Vec<String> {
    c.roles.clone().unwrap_or_else(|| crate::rgb::band_roles(names).into_iter().map(String::from).collect())
}

/// The scale of an image's reflectance, from its bands' names and the greatest value of a sample of its bands
/// (every few cells, no-data left out).
pub fn image_scale(path: &Path, names: &[String]) -> Result<Scale, String> {
    let f = terraflux_ops::files::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let meta = f.meta();
    let (w, h) = (meta.grid.width, meta.grid.height);
    let step = w.max(h).div_ceil(512).max(1);
    let win = terraflux_raster::tiff::Window { x0: 0, y0: 0, width: w, height: h };
    let nodata = meta.nodata.filter(|n| !n.is_nan());
    let mut max = f64::NEG_INFINITY;
    for b in 0..names.len() {
        let v = f.sample(b, win, step).map_err(|e| format!("{}: {e}", path.display()))?;
        for x in v.into_iter().map(f64::from).filter(|x| x.is_finite() && Some(*x) != nodata) {
            max = max.max(x);
        }
    }
    Ok(Scale::detect(names, max))
}

/// What a file says of an image: its bands, their roles by their names, and its scale.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageInfo {
    pub names: Vec<String>,
    pub roles: Vec<String>,
    pub scale: Scale,
}

pub fn image_info(path: &Path) -> Result<ImageInfo, String> {
    let names = header(path)?.1;
    let roles = image_roles(&Choice::default(), &names);
    let scale = image_scale(path, &names)?;
    Ok(ImageInfo { names, roles, scale })
}

/// Whether a source is a multispectral image: data of kind image, or an `nlm.image`'s `image` output (the drawn image
/// the segmenters and `objects.features` read, with or without the mark).
pub fn is_image(s: &Source) -> bool {
    s.data.as_ref().is_some_and(|d| d.kind == DataKind::Image) || (s.op == "nlm.image" && s.port == "image")
}

/// A class of the user's for an image: the name it has in the template's rules (none: a class of the user's own),
/// the name it is given, and its colour (`#rrggbb`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserClass {
    pub from: Option<String>,
    pub name: String,
    pub colour: Option<String>,
}

/// The classes of each `objects.rules` of a model (its node and the classes in the list's order), as a start for the
/// user's. The samples a template classes by (`objects.samples`) have no list here: their names are the ones of the
/// vector the user draws or chooses.
pub fn rule_classes(m: &Model) -> Vec<(String, Vec<UserClass>)> {
    let mut out = Vec::new();
    for n in m.nodes.iter().filter(|n| n.op == "objects.rules") {
        let Some(list) = n.params.get("classes").and_then(Json::as_array) else { continue };
        let classes: Vec<UserClass> = list
            .iter()
            .filter_map(|c| {
                let name = c.get("name")?.as_str()?.to_string();
                let colour = c.get("colour").and_then(Json::as_str).map(String::from);
                Some(UserClass { from: Some(name.clone()), name, colour })
            })
            .collect();
        if !classes.is_empty() {
            out.push((n.id.clone(), classes));
        }
    }
    out
}

/// A model's `objects.rules` with the user's classes: each class renamed and coloured (its rule kept, and the
/// references to it in the rules and the daughters' `parent` renamed), the ones left out taken away (their daughters
/// become roots), the new ones added with the rule `0` (they take no object until a rule is written for them).
/// What was done, a line; none when the classes are the template's as they were.
pub fn apply_classes(m: &mut Model, node: &str, user: &[UserClass]) -> Result<Option<String>, String> {
    let n = m.nodes.iter_mut().find(|n| n.id == node).ok_or_else(|| format!("{node}: no such node"))?;
    let old = n.params.get("classes").and_then(Json::as_array).cloned().ok_or_else(|| format!("{node}: no classes"))?;
    if user.is_empty() {
        return Err(format!("{node}: no class: the rules need at least one"));
    }
    let mut seen = BTreeSet::new();
    if let Some(u) = user.iter().find(|u| u.name.trim().is_empty() || !seen.insert(u.name.trim().to_string())) {
        return Err(format!("{node}: the class {:?} has no name or the name is used twice", u.name));
    }
    let renames: Vec<(String, String)> = user
        .iter()
        .filter_map(|u| u.from.as_ref().map(|f| (f.clone(), u.name.trim().to_string())))
        .filter(|(f, t)| f != t)
        .collect();
    let kept: BTreeSet<&str> = user.iter().filter_map(|u| u.from.as_deref()).collect();
    let rename = |s: &str| -> String {
        let mut s = s.to_string();
        // (through a placeholder, so that a swap of two names does not undo itself)
        for (k, (f, _)) in renames.iter().enumerate() {
            for q in ['"', '\''] {
                s = s.replace(&format!("{q}{f}{q}"), &format!("{q}\u{1}{k}\u{1}{q}"));
            }
        }
        for (k, (_, t)) in renames.iter().enumerate() {
            s = s.replace(&format!("\u{1}{k}\u{1}"), t);
        }
        s
    };
    let mut list = Vec::new();
    for u in user {
        let name = u.name.trim().to_string();
        let mut c =
            match u.from.as_ref().and_then(|f| old.iter().find(|c| c.get("name").and_then(Json::as_str) == Some(f))) {
                Some(c) => c.clone(),
                None => serde_json::json!({ "rule": "0" }),
            };
        let o = c.as_object_mut().ok_or_else(|| format!("{node}: a class is not an object"))?;
        o.insert("name".into(), Json::from(name));
        match u.colour.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
            Some(c) => o.insert("colour".into(), Json::from(c)),
            None => o.remove("colour"),
        };
        if let Some(r) = o.get("rule").and_then(Json::as_str).map(rename) {
            o.insert("rule".into(), Json::from(r));
        }
        match o.get("parent").and_then(Json::as_str).map(String::from) {
            Some(p) if kept.contains(p.as_str()) => {
                o.insert("parent".into(), Json::from(rename(&format!("\"{p}\"")).trim_matches('"').to_string()));
            }
            Some(_) => {
                o.remove("parent");
            }
            None => {}
        }
        list.push(c);
    }
    let same = list == old;
    n.params.insert("classes".into(), Json::Array(list));
    if same {
        return Ok(None);
    }
    let added = user.iter().filter(|u| u.from.is_none()).count();
    let gone = old.len() - kept.len();
    Ok(Some(format!(
        "{node}: your classes ({}): {} kept, {added} added (rule 0: write theirs), {gone} taken away",
        user.iter().map(|u| u.name.trim()).collect::<Vec<_>>().join(", "),
        user.iter().filter(|u| u.from.is_some()).count()
    )))
}

fn same_grid(a: &Grid, b: &Grid) -> bool {
    let near = |x: &f64, y: &f64| (x - y).abs() <= 1e-9 * (1.0 + x.abs().max(y.abs()));
    let crs = match (a.geo.epsg(), b.geo.epsg()) {
        (Some(x), Some(y)) => x == y,
        _ => match (&a.geo.wkt, &b.geo.wkt) {
            (Some(x), Some(y)) => x == y,
            _ => true,
        },
    };
    a.width == b.width && a.height == b.height && a.transform.iter().zip(&b.transform).all(|(x, y)| near(x, y)) && crs
}

/// Whether a choice can be made for a source: the grid of its file, or why not.
fn vet(m: &Model, s: &Source, c: &Choice) -> Result<Grid, String> {
    if s.needs == Type::Map && c.bands.is_some() {
        return Err("a stack of bands for a map: choose one band".into());
    }
    if s.op != INPUT && readers(m, &s.node).ports.len() > 1 {
        return Err("the model reads more than one of its outputs: it cannot be replaced by a file".into());
    }
    if (c.scene.is_some() || !c.files.is_empty()) && !is_image(s) {
        return Err("bands of several files, or a Sentinel-2 scene, are for an image".into());
    }
    if c.scene.is_some() && !c.files.is_empty() {
        return Err("a scene or files, not both".into());
    }
    let (grid, names) = if let Some(sc) = &c.scene {
        if let Some(p) = sc.problem() {
            return Err(p);
        }
        (sc.grid(), SCENE_ASSETS.iter().map(|a| a.to_string()).collect())
    } else if !c.files.is_empty() {
        let (finest, mut grids, names) = files_header(&c.files)?;
        (grids.swap_remove(finest), names)
    } else {
        header(&c.file)?
    };
    let known = |b: &String| names.contains(b) || b.parse::<usize>().is_ok_and(|i| i < names.len());
    if let Some(b) = c.band.iter().chain(c.bands.iter().flatten()).find(|b| !known(b)) {
        return Err(format!("{} has no band {b:?} (its bands are {names:?})", c.file.display()));
    }
    if is_image(s) {
        if c.band.is_some() {
            return Err("an image takes all its bands: their roles are set, not one band".into());
        }
        if let Some(roles) = &c.roles {
            if roles.len() != names.len() {
                return Err(format!("{} roles for the {} bands of the file", roles.len(), names.len()));
            }
            if let Some(r) = roles.iter().find(|r| r.as_str() != "other" && !crate::rgb::ROLES.contains(&r.as_str())) {
                return Err(format!("{r:?} is no role of a band ({}, other)", crate::rgb::ROLES.join(", ")));
            }
            let mut seen = BTreeSet::new();
            if let Some(r) = roles.iter().filter(|r| r.as_str() != "other").find(|r| !seen.insert(r.as_str())) {
                return Err(format!("two bands are {r}: a role is one band's"));
            }
        }
        if image_roles(c, &names).iter().all(|r| r == "other") {
            return Err(format!(
                "no band of {} has a role (blue, green, red, nir, swir1, swir2): choose them on the Bands page",
                c.file.display()
            ));
        }
    } else if c.roles.is_some() || c.scale.is_some() {
        return Err("roles and a scale of reflectance are for an image".into());
    }
    if c.dates {
        if land(m) != Some(Land::Lulcc) || m.node("observed").is_none_or(|n| n.op != "stack.of") {
            return Err("every band as a date: only CLUE-S's model reads more than two dates".into());
        }
        if names.len() < 3 {
            return Err(format!("every band as a date: the file has {} bands, two dates are two sources", names.len()));
        }
        if !c.crosswalk.is_empty() {
            return Err("no table of classes with every band as a date: the classes are read as they are".into());
        }
    }
    if !c.crosswalk.is_empty() {
        let d = s.data.as_ref().ok_or("it takes no table of classes (it is not data of classes)")?;
        if !matches!(d.kind, DataKind::Classes | DataKind::Mask) {
            return Err(format!("it is data of {}: a table of classes is for classes and masks", d.kind.name()));
        }
        let targets: Vec<i64> = c.crosswalk.values().flatten().copied().collect();
        if targets.is_empty() {
            return Err("the table of classes maps no class".into());
        }
        if d.classes.is_empty() {
            if d.kind == DataKind::Classes {
                return Err("it takes any classes: they are kept as they are, no table".into());
            }
            if let Some(t) = targets.iter().find(|t| !matches!(t, 0 | 1)) {
                return Err(format!("class {t} is not 0 or 1, what a mask takes"));
            }
        } else if let Some(t) = targets.iter().find(|t| !d.classes.contains_key(t)) {
            return Err(format!("class {t} is not among the template's classes ({})", d.classes_line()));
        }
    }
    Ok(grid)
}

/// A file's path as the model writes it: relative to its folder when under it, else absolute.
fn path_text(file: &Path, base: &Path) -> String {
    let p = if base.as_os_str().is_empty() { None } else { file.strip_prefix(base).ok() };
    match p {
        Some(p) => p.to_string_lossy().replace('\\', "/"),
        None => file.to_string_lossy().into_owned(),
    }
}

/// The rules of a table of classes: the user's classes of each class of the template's a line, those mapped to
/// nothing (and the ones not in the table) no data.
fn rules_of(crosswalk: &BTreeMap<i64, Option<i64>>) -> String {
    let mut by: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    let mut none = Vec::new();
    for (user, to) in crosswalk {
        match to {
            Some(t) => by.entry(*t).or_default().push(*user),
            None => none.push(*user),
        }
    }
    let list = |v: &[i64]| v.iter().map(i64::to_string).collect::<Vec<_>>().join(" ");
    let mut lines: Vec<String> = by.iter().map(|(t, v)| format!("{} -> {t}", list(v))).collect();
    if !none.is_empty() {
        lines.push(format!("{} -> nan", list(&none)));
    }
    lines.push("* -> nan".to_string());
    lines.join("\n")
}

/// Every reference `old` read by the model's nodes, outputs and Plots read `new` instead (and the reroute points of
/// the wires changed gone).
fn repoint(m: &mut Model, old: &str, new: &str) {
    for n in m.nodes.iter_mut() {
        let ports: Vec<String> = n
            .inputs
            .iter_mut()
            .filter_map(|(k, s)| match s {
                Wire::Ref(r) if r == old => {
                    *r = new.to_string();
                    Some(k.clone())
                }
                _ => None,
            })
            .collect();
        for p in ports {
            n.wires.remove(&p);
        }
    }
    editor::for_each_ref(m, &[], &mut |r| {
        if r == old {
            *r = new.to_string();
        }
    });
}

/// The node or input a reference reads.
fn target_of(r: &str) -> Option<String> {
    match Ref::parse(r).ok()? {
        Ref::Port { node, .. } => Some(node),
        Ref::Input(name) => Some(format!("${name}")),
        Ref::Carried(_) => None,
    }
}

/// The model with the files chosen in place of the data they stand for (a copy: the model given is not changed).
/// `choices` is by the source's node; `reference` is the source whose grid the others are put on (none: the first
/// map chosen); `base` is the model's folder. The choices that cannot be made are left out and told in
/// [`Plan::refused`]; none that can: an error.
pub fn plan(
    m: &Model,
    r: &Registry,
    choices: &BTreeMap<String, Choice>,
    reference: Option<&str>,
    base: &Path,
) -> Result<Plan, String> {
    let srcs = sources(m, r);
    let mut refused = Vec::new();
    for k in choices.keys().filter(|k| !srcs.iter().any(|s| &s.node == *k)) {
        refused.push(format!("{k}: the model has no such data to replace"));
    }
    let mut ok: Vec<(&Source, &Choice, Grid)> = Vec::new();
    for s in &srcs {
        let Some(c) = choices.get(&s.node) else { continue };
        match vet(m, s, c) {
            Ok(g) => ok.push((s, c, g)),
            Err(e) => refused.push(format!("{}: {e}", s.node)),
        }
    }
    if ok.is_empty() {
        return Err(if refused.is_empty() {
            "choose the file of at least one data".into()
        } else {
            refused.join("; ")
        });
    }
    let (ref_node, ref_grid, ref_stack) = match reference {
        Some(k) => ok.iter().find(|(s, ..)| s.node == k).map(|(s, _, g)| (s.node.clone(), g.clone(), s.needs)),
        None => ok
            .iter()
            .find(|(s, ..)| s.needs == Type::Map)
            .or_else(|| ok.iter().find(|(_, c, _)| c.scene.is_none()))
            .or(ok.first())
            .map(|(s, _, g)| (s.node.clone(), g.clone(), s.needs)),
    }
    .map(|(n, g, t)| (n, g, t == Type::Stack))
    .ok_or_else(|| format!("the reference {:?} is not among the data chosen", reference.unwrap_or("")))?;

    let mut out = m.clone();
    let mut notes = Vec::new();
    let mut taken_in: BTreeSet<String> = out.inputs.keys().cloned().collect();
    let mut taken: BTreeSet<String> = out.nodes.iter().map(|n| n.id.clone()).collect();
    let mut names: HashMap<String, String> = HashMap::new();
    // (an image of band files: an input for each, the finest the source's own)
    let mut band_inputs: HashMap<String, Vec<String>> = HashMap::new();
    for (s, c, _) in &ok {
        if c.files.is_empty() {
            let name = if s.op == INPUT { s.node.clone() } else { fresh(&mut taken_in, &s.node) };
            names.insert(s.node.clone(), name);
            continue;
        }
        let (finest, ..) = files_header(&c.files)?;
        let mut list = Vec::new();
        for (k, f) in c.files.iter().enumerate() {
            let label = band_label(&stem(f));
            list.push(if k == finest && s.op == INPUT {
                s.node.clone()
            } else {
                fresh(&mut taken_in, &format!("{}_{label}", s.node))
            });
        }
        names.insert(s.node.clone(), list[finest].clone());
        band_inputs.insert(s.node.clone(), list);
    }
    let ref_in = names[&ref_node].clone();
    let mut inputs = Vec::new();
    // what the drawn nodes read, to take away what only they read
    let mut candidates: Vec<String> = Vec::new();
    for (s, c, grid) in &ok {
        let name = names[&s.node].clone();
        let file = if let Some(sc) = &c.scene {
            format!("a cloud-free Sentinel-2 scene of {} to {} ({})", sc.extent[0], sc.extent[2], sc.dates)
        } else if c.files.is_empty() {
            path_text(&c.file, base)
        } else {
            let at = c.files[0].parent().map(|p| path_text(p, base)).unwrap_or_default();
            format!("the {} band files in {at}", c.files.len())
        };
        let band = match (&c.band, &c.bands) {
            (Some(b), _) => format!(", band {b}"),
            (None, Some(b)) => format!(", bands {}", b.join(" ")),
            _ => String::new(),
        };
        let kind = s.data.as_ref().map(|d| d.kind);
        let note = format!("{}: your file {file}{band}", s.what);
        let what =
            if c.scene.is_some() || !c.files.is_empty() { file.clone() } else { format!("your file {file}{band}") };
        let mut chain: Vec<(Node, &str)> = Vec::new();
        let mut cur = format!("${name}");
        let why = if s.op == INPUT {
            format!("for the input ${}: {what}", s.node)
        } else {
            format!("in place of the drawn {}: {what}", s.node)
        };
        if let Some(sc) = &c.scene {
            // nothing is read now: the node searches the catalogue and reads the scenes when the model runs
            let p = crate::catalog::preset_of(SCENE_PRESET);
            let id = fresh(&mut taken, &format!("{}_scene", s.node));
            let mut n = Node::new(&id, "data.stac")
                .param("url", p.url)
                .param("collection", p.collection)
                .param("assets", serde_json::json!(SCENE_ASSETS))
                .param("datetime", sc.dates.trim())
                .param("query", serde_json::json!({"eo:cloud_cover": {"lt": 20}}).to_string())
                .param("composite", p.composite)
                .param("mask", p.mask)
                .param("mask_buffer", 1)
                .param("extent", serde_json::json!(sc.extent))
                .param("cellsize", sc.cellsize)
                .note(&format!(
                    "A Sentinel-2 L2A scene of your area without clouds: the scenes of {} with less than 20% of \
                     cloud, the clouds, shadows and cirrus of each taken out pixel by pixel, the median of the \
                     rest, on cells of {} degrees. Read from Earth Search when the model runs.",
                    sc.dates.trim(),
                    sc.cellsize
                ))
                .why(&why);
            n.refs = vec![p.reference.to_string()];
            if s.op == INPUT {
                out.inputs.remove(&s.node);
            }
            notes.push(format!(
                "{}: a data.stac node reads a cloud-free Sentinel-2 scene of the area {:?} ({}), nothing is downloaded until the model runs",
                s.node, sc.extent, sc.dates
            ));
            cur = format!("{id}.raster");
            chain.push((n, "raster"));
        } else if let Some(list) = band_inputs.get(&s.node) {
            // an input for each band file; the finer grid is the reference of the others; a stack of them, in band order
            let (finest, grids, fnames) = files_header(&c.files)?;
            let like = format!("${}", list[finest]);
            let mut wires = Vec::new();
            for (k, f) in c.files.iter().enumerate() {
                let path = path_text(f, base);
                out.inputs.insert(
                    list[k].clone(),
                    Input::Map {
                        path: path.clone(),
                        band: None,
                        nodata: c.nodata,
                        note: Some(format!("{}: band file {path}", s.what)),
                        data: None,
                    },
                );
                inputs.push(list[k].clone());
                let mut wire = format!("${}", list[k]);
                if k != finest && !same_grid(&grids[k], &grids[finest]) {
                    let id = fresh(&mut taken, &format!("{}_{}_match", s.node, band_label(&fnames[k])));
                    let n = Node::new(&id, "raster.match")
                        .input("raster", &wire)
                        .input("like", &like)
                        .param("resampling", "bilinear")
                        .note(&format!("The band {} put on the finest grid of the image's bands ({like}).", fnames[k]))
                        .why(&why);
                    notes.push(format!(
                        "{}: {} put on the grid of the finest band ({}) by raster.match (bilinear)",
                        s.node, fnames[k], fnames[finest]
                    ));
                    wire = format!("{id}.raster");
                    chain.push((n, "raster"));
                }
                wires.push(wire);
            }
            let id = fresh(&mut taken, &format!("{}_stack", s.node));
            let mut n = Node::new(&id, "stack.of")
                .note("The image's band files as one stack, in band order (the layers are named for the bands).")
                .why(&why);
            for (k, w) in wires.iter().enumerate() {
                n = n.input(&format!("b{:02}", k + 1), w);
            }
            cur = format!("{id}.stack");
            chain.push((n, "stack"));
            notes.push(format!("{}: reads {file}, in band order: {}", s.node, fnames.join(" ")));
        } else {
            let input = if s.needs == Type::Stack {
                Input::Stack {
                    path: file.clone(),
                    bands: c.bands.clone(),
                    nodata: c.nodata,
                    note: Some(note),
                    data: None,
                }
            } else {
                Input::Map { path: file.clone(), band: c.band.clone(), nodata: c.nodata, note: Some(note), data: None }
            };
            out.inputs.insert(name.clone(), input);
            inputs.push(name.clone());
            notes.push(format!("{}: reads your file {file}{band} as the input ${name}", s.node));
        }
        if s.node != ref_node {
            if same_grid(grid, &ref_grid) {
                notes.push(format!("{}: already on the grid of ${ref_in}: no raster.match", s.node));
            } else {
                let how = match kind {
                    Some(DataKind::Classes | DataKind::Mask) => "near",
                    Some(DataKind::Values) => "bilinear",
                    _ => "auto",
                };
                let id = fresh(&mut taken, &format!("{}_match", s.node));
                let n = Node::new(&id, "raster.match")
                    .input("raster", &cur)
                    .input("like", &format!("${ref_in}"))
                    .param("resampling", how)
                    .note(&format!(
                        "Your file put on the grid of ${ref_in} (its cells, extent and CRS), resampled by {how}."
                    ))
                    .why(&why);
                notes.push(format!(
                    "{}: put on the grid of ${ref_in} by raster.match ({how}): its grid differs",
                    s.node
                ));
                cur = format!("{id}.raster");
                chain.push((n, "raster"));
            }
        }
        if is_image(s) {
            let (fnames, scale) = if let Some(sc) = &c.scene {
                // (Earth Search's Sentinel-2 L2A: digital numbers, 0 to 10 000, with an offset of 1000 from the
                // processing baseline 04.00 on, 25 January 2022: dates from then on take it off)
                let offset = sc.dates.trim() >= "2022-01-25";
                let default = if offset { Scale::Sentinel2Offset } else { Scale::Div10000 };
                (SCENE_ASSETS.iter().map(|a| a.to_string()).collect(), c.scale.unwrap_or(default))
            } else if !c.files.is_empty() {
                let n = files_header(&c.files)?.2;
                (n, c.scale.unwrap_or_else(|| files_scale(&c.files).unwrap_or_default()))
            } else {
                let n = header(&c.file)?.1;
                let sc = c.scale.unwrap_or_else(|| image_scale(&c.file, &n).unwrap_or_default());
                (n, sc)
            };
            let roles = image_roles(c, &fnames);
            let names: Vec<&str> = roles.iter().map(|r| if r == "other" { "" } else { r.as_str() }).collect();
            let kept: Vec<&str> = names.iter().copied().filter(|n| !n.is_empty()).collect();
            let dropped = roles.len() - kept.len();
            let renamed = !c.files.is_empty() || names.iter().zip(&fnames).any(|(r, n)| r != n);
            if renamed || scale != Scale::None {
                let id = fresh(&mut taken, &format!("{}_bands", s.node));
                let mut n = Node::new(&id, "stack.rename")
                    .input("stack", &cur)
                    .param("names", serde_json::json!(names))
                    .note(
                        "The bands of your image named by their roles (blue, green, red, nir, swir1, swir2: the names \
                         the model's features and rules read), the bands with no role dropped, the reflectance on a \
                         scale of 0 to 1.",
                    )
                    .why(&why);
                if scale != Scale::None {
                    let (k, off) = scale.factors();
                    n = n.param("scale", k);
                    if off != 0.0 {
                        n = n.param("offset", off);
                    }
                }
                cur = format!("{id}.stack");
                chain.push((n, "stack"));
            }
            notes.push(format!(
                "{}: the bands {}{}; reflectance {}",
                s.node,
                kept.join(" "),
                if dropped > 0 { format!(" ({dropped} with no role dropped)") } else { String::new() },
                scale.label()
            ));
        }
        if !c.crosswalk.is_empty() {
            let rules = rules_of(&c.crosswalk);
            let id = fresh(&mut taken, &format!("{}_classes", s.node));
            let n = Node::new(&id, "raster.reclassify")
                .input("map", &cur)
                .param("rules", rules.clone())
                .note("Your classes into the ones the model reads (the rules are your class -> the model's); a class not in them has no data.")
                .why(&why);
            notes.push(format!(
                "{}: your classes into the model's by raster.reclassify: {}",
                s.node,
                rules.replace('\n', "; ")
            ));
            cur = format!("{id}.map");
            chain.push((n, "map"));
            if let Some(d) = s.data.as_ref().filter(|d| !d.classes.is_empty()) {
                let id = fresh(&mut taken, &format!("{}_legend", s.node));
                let classes: Vec<serde_json::Value> =
                    d.classes.iter().map(|(k, v)| serde_json::json!([k, v])).collect();
                let n = Node::new(&id, "raster.legend")
                    .input("map", &cur)
                    .param("classes", classes)
                    .note("The names of the model's classes, for the maps to show.")
                    .why(&why);
                cur = format!("{id}.map");
                chain.push((n, "map"));
            }
        }
        let end = chain.last().map(|(n, p)| format!("{}.{p}", n.id)).unwrap_or_else(|| format!("${name}"));
        let mut chain: Vec<Node> = chain.into_iter().map(|(n, _)| n).collect();
        // the data goes to the end of the chain: its last node, else the input read straight
        if let Some(d) = &s.data {
            match chain.last_mut() {
                Some(last) => last.data = Some(d.clone()),
                None => {
                    if let Some(Input::Map { data, .. } | Input::Stack { data, .. }) = out.inputs.get_mut(&name) {
                        *data = Some(d.clone());
                    }
                }
            }
        }
        let old = if s.op == INPUT { format!("${}", s.node) } else { format!("{}.{}", s.node, s.port) };
        if old != end {
            repoint(&mut out, &old, &end);
        }
        if s.op == INPUT {
            out.nodes.extend(chain);
            continue;
        }
        // the drawn node goes, its chain in its place
        let at = out.nodes.iter().position(|n| n.id == s.node).expect("a source is a node of the model");
        let old_node = out.nodes.remove(at);
        candidates.extend(reads(&old_node).iter().filter_map(|r| target_of(r)));
        if let Some([x, y]) = old_node.pos {
            for (k, n) in chain.iter_mut().enumerate() {
                n.pos = Some([x, y + 140.0 * k as f32]);
            }
        }
        let ids: Vec<String> = chain.iter().map(|n| n.id.clone()).collect();
        for f in out.notes.iter_mut().filter(|f| f.nodes.contains(&s.node)) {
            f.nodes.retain(|k| k != &s.node);
            f.nodes.extend(ids.clone());
        }
        let prefix = format!("{}.", s.node);
        out.parameters.retain(|p| !p.key.starts_with(&prefix));
        for sc in out.scenarios.iter_mut() {
            sc.values.retain(|k, _| !k.starts_with(&prefix));
        }
        out.nodes.splice(at..at, chain);
        notes.push(format!("{}: the drawn node taken away; what read it reads {end}", s.node));
    }

    // every band of a file as a date (CLUE-S): the demand reads a stack of the file, and the node that stacked the
    // two dates keeps the maps the simulation can start from (it is named for that)
    for (s, c, _) in ok.iter().filter(|(_, c, _)| c.dates) {
        let Some(at) = out.nodes.iter().position(|n| n.id == "observed" && n.op == "stack.of") else { continue };
        let file = path_text(&c.file, base);
        let bands = header(&c.file)?.1;
        let name = fresh(&mut taken_in, "observed");
        let note = format!("the observed maps, one per date: your file {file}, bands {}", bands.join(" "));
        let data = Data::new("the observed land use maps, one band per date", DataKind::Stack);
        out.inputs.insert(
            name.clone(),
            Input::Stack { path: file.clone(), bands: None, nodata: c.nodata, note: Some(note), data: Some(data) },
        );
        inputs.push(name.clone());
        repoint(&mut out, "observed.stack", &format!("${name}"));
        let id = fresh(&mut taken, "ends");
        for f in out.notes.iter_mut() {
            for k in f.nodes.iter_mut().filter(|k| *k == "observed") {
                *k = id.clone();
            }
        }
        let n = &mut out.nodes[at];
        n.id = id.clone();
        n.note = Some(
            "The maps of the first and the last date: what the simulation can start from (Use my data's Parameters \
             page). The demand reads all the dates, from the stack of your file."
                .into(),
        );
        n.why = Some(format!("every band of your file {file} is a date: {}", s.node));
        notes.push(format!(
            "{}: every band of your file {file} ({}) is a date: the demand reads them as the input ${name}",
            s.node,
            bands.join(" ")
        ));
    }

    // what only the nodes taken away read goes too
    loop {
        let mut read: BTreeSet<String> = BTreeSet::new();
        for n in &out.nodes {
            read.extend(reads(n).iter().filter_map(|r| target_of(r)));
        }
        read.extend(outside_reads(&out).iter().filter_map(|r| target_of(r)));
        let Some(k) = candidates.iter().position(|c| {
            !read.contains(c)
                && match c.strip_prefix('$') {
                    Some(name) => out.inputs.contains_key(name) && !inputs.iter().any(|i| i == name),
                    None => out.nodes.iter().any(|n| &n.id == c),
                }
        }) else {
            break;
        };
        let c = candidates.remove(k);
        match c.strip_prefix('$') {
            Some(name) => {
                out.inputs.remove(name);
                notes.push(format!("${name}: taken away, nothing reads it any more"));
            }
            None => {
                let at = out.nodes.iter().position(|n| n.id == c).unwrap();
                let n = out.nodes.remove(at);
                candidates.extend(reads(&n).iter().filter_map(|r| target_of(r)));
                notes.push(format!("{c}: taken away, only the drawn data read it"));
            }
        }
    }

    // the drawn maps left go on the reference grid
    for n in out.nodes.iter_mut().filter(|n| n.op.starts_with("nlm.")) {
        let like = r.sig(&n.op).is_some_and(|s| s.inputs.iter().any(|i| i.name == "like"));
        if !like {
            notes.push(format!(
                "{}: drawn on its own grid ({} has no `like`): it does not follow your data",
                n.id, n.op
            ));
        } else if n.inputs.contains_key("like") {
            continue;
        } else if ref_stack {
            notes.push(format!("{}: drawn on its own grid (the reference is a stack, not a map)", n.id));
        } else {
            n.inputs.insert("like".into(), Wire::Ref(format!("${ref_in}")));
            for k in ["rows", "columns", "resolution", "crs"] {
                n.params.remove(k);
            }
            notes.push(format!("{}: still drawn, on the grid of ${ref_in}", n.id));
        }
    }
    Ok(Plan { model: out, notes, refused, inputs, reference: ref_in })
}

/// The problems of the planned model: those of the model's check, and of its data as the data of any model is
/// read ([`terraflux_exec::load_inputs`] and [`terraflux_exec::lint::lint`]), without those of grids of the inputs
/// that a `raster.match` puts right.
pub fn check(plan: &Plan, base: &Path) -> Vec<Finding> {
    let r = Registry::standard();
    let mut out: Vec<Finding> = terraflux_graph::check(&plan.model, &r)
        .into_iter()
        .map(|i| Finding {
            level: if i.severity == Severity::Error { Level::Error } else { Level::Warning },
            at: i.at,
            message: i.message,
            fix: None,
            action: None,
        })
        .collect();
    match terraflux_exec::load_inputs(&plan.model, base, &Default::default()) {
        Err(e) => out.push(Finding {
            level: Level::Error,
            at: String::new(),
            message: e.to_string(),
            fix: None,
            action: None,
        }),
        Ok(inputs) => {
            let matched: BTreeSet<&str> = plan
                .model
                .nodes
                .iter()
                .filter(|n| n.op == "raster.match")
                .filter_map(|n| match n.inputs.get("raster") {
                    Some(Wire::Ref(r)) => r.strip_prefix('$'),
                    _ => None,
                })
                .collect();
            out.extend(terraflux_exec::lint::lint(&plan.model, &inputs).into_iter().filter(|f| {
                let name = f.at.trim_start_matches('$').split('/').next().unwrap_or("");
                !(f.message.contains("reference grid")
                    && (matched.contains(name) || plan.inputs.iter().any(|i| i == name)))
            }));
        }
    }
    out
}

/// The classes of a file's band (the first when none is given): each class's code, its name from the file's
/// legend, and its cells. An error when the band is not a map of classes: fractional values, or more than 256.
pub fn classes_of(path: &Path, band: Option<&str>) -> Result<Vec<(i64, Option<String>, usize)>, String> {
    let r = terraflux_ops::files::read_raster(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let i =
        match band {
            None => 0,
            Some(b) => match r.bands.iter().position(|x| x.name.as_deref() == Some(b)) {
                Some(i) => i,
                None => b.parse::<usize>().ok().filter(|i| *i < r.bands.len()).ok_or_else(|| {
                    format!("{} has no band {b:?} (its bands are {:?})", path.display(), r.band_names())
                })?,
            },
        };
    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    for v in r.bands[i].masked(r.nodata) {
        if v.is_nan() {
            continue;
        }
        if v.fract() != 0.0 {
            return Err("not a map of classes: it holds fractional values".into());
        }
        *counts.entry(v as i64).or_default() += 1;
        if counts.len() > 256 {
            return Err("not a map of classes: more than 256 values".into());
        }
    }
    let names: BTreeMap<i64, String> = terraflux_ops::legend::of_grid(&r.grid)
        .unwrap_or_default()
        .into_iter()
        .filter(|c| !c.name.trim().is_empty())
        .map(|c| (c.value as i64, c.name))
        .collect();
    Ok(counts.into_iter().map(|(k, n)| (k, names.get(&k).cloned(), n)).collect())
}

/// A first table of classes: the user's class of the template's name (letters of any case) is that class; when the
/// user's file names none of its classes, the same code is the same class; else no class (to be chosen).
pub fn suggest(template: &BTreeMap<i64, String>, user: &[(i64, Option<String>, usize)]) -> BTreeMap<i64, Option<i64>> {
    let named = user.iter().any(|(_, n, _)| n.is_some());
    user.iter()
        .map(|(code, name, _)| {
            let to = match name {
                Some(n) => template.iter().find(|(_, t)| t.trim().eq_ignore_ascii_case(n.trim())).map(|(k, _)| *k),
                None if !named => template.contains_key(code).then_some(*code),
                None => None,
            };
            (*code, to)
        })
        .collect()
}

/// The land change models the Parameters page knows (the templates of Dinamica EGO, CLUE-S and PLUS, and what the
/// presets of `terraflux new` make: the same nodes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Land {
    Dinamica,
    Lulcc,
    Plus,
}

/// The node at a path of ids (`simulation/allocate`).
fn at<'a>(m: &'a Model, path: &str) -> Option<&'a Node> {
    let mut parts = path.split('/');
    let mut node = m.node(parts.next()?)?;
    for id in parts {
        node = node.body.as_ref()?.nodes.iter().find(|n| n.id == id)?;
    }
    Some(node)
}

/// Which land change model it is, by its nodes (none: it is not one).
pub fn land(m: &Model) -> Option<Land> {
    let top = |id: &str| m.node(id).map(|n| n.op.as_str());
    let inner = |id: &str| at(m, &format!("simulation/{id}")).map(|n| n.op.as_str());
    if top("markov") == Some("markov.fit")
        && top("woe") == Some("woe.fit")
        && top("patches") == Some("patches.observe")
        && inner("allocate") == Some("dinamica.step")
    {
        Some(Land::Dinamica)
    } else if top("demand") == Some("demand.from_maps") && inner("allocate") == Some("lulcc.step") {
        Some(Land::Lulcc)
    } else if top("leas") == Some("ml.potential") && inner("cars") == Some("cars.step") {
        Some(Land::Plus)
    } else {
        None
    }
}

/// A setting the Parameters page offers: one of the model's exposed parameters (its id is the key), or one of a land
/// change model's (a word: `steps`, `method`...), with what the form needs.
#[derive(Debug, Clone)]
pub struct Setting {
    pub id: String,
    /// Its label, help, range, step and choices (its `key` is the id).
    pub param: Parameter,
    /// What it is now in the model (none: automatic, or the operator's default).
    pub now: Option<Json>,
    /// What the setting is when left automatic, and the value it takes when turned on; none: it is always a value.
    pub auto: Option<(String, Json)>,
}

fn setting(id: &str, label: &str, help: &str, now: Option<Json>) -> Setting {
    let param = Parameter { key: id.into(), label: Some(label.into()), help: Some(help.into()), ..Default::default() };
    Setting { id: id.into(), param, now, auto: None }
}

fn ranged(mut s: Setting, min: f64, max: f64, step: f64) -> Setting {
    s.param.min = Some(min);
    s.param.max = Some(max);
    s.param.step = Some(step);
    s
}

fn automatic(mut s: Setting, text: &str, on: Json) -> Setting {
    s.auto = Some((text.into(), on));
    s
}

/// The allocation of a CLUE-S or ordered model.
fn allocation_of(m: &Model) -> Option<&serde_json::Map<String, Json>> {
    at(m, "simulation/allocate")?.params.get("allocation")?.as_object()
}

/// The method of a CLUE-S or ordered model's allocation.
pub fn method_of(m: &Model) -> String {
    allocation_of(m).and_then(|a| a.get("method")).and_then(Json::as_str).unwrap_or("clue").to_string()
}

/// The key of the setting a land change model's id stands for (those that are one node's setting).
fn key_of(land: Land, id: &str) -> Option<&'static str> {
    Some(match (land, id) {
        (Land::Dinamica | Land::Plus, "steps") => "markov.steps",
        (Land::Dinamica, "simulate") => "simulation.times",
        (Land::Plus, "simulate") => "demand.steps",
        (Land::Dinamica, "increment") => "woe.increment",
        (Land::Dinamica, "expansion") => "patches.expansion",
        (Land::Dinamica, "isometry") => "patches.isometry",
        (Land::Dinamica, "seed") => "simulation/allocate.seed",
        (Land::Plus, "seed") => "simulation/cars.seed",
        (Land::Lulcc, "seed") => "rng.seed",
        (Land::Lulcc, "elasticity") => "simulation/allocate.elasticity",
        _ => return None,
    })
}

/// What a land change model's simulation starts from: the map of the second date (the future, as the templates do)
/// or the first (the observed period again, to compare what it gives with what happened).
const START_LAST: &str = "the second date";
const START_FIRST: &str = "the first date";

/// The settings the Parameters page offers for a model: its land change settings (the steps between
/// the dates and to simulate, the buffer increment, the Expander's share and the isometry of Dinamica's; the method,
/// the order of the classes, the elasticity and the dates of CLUE-S's and ordered; the seed), then the model's
/// exposed parameters (`parameters` of the model). Empty for a model with neither.
pub fn settings(m: &Model) -> Vec<Setting> {
    let land = land(m);
    let now = |id: &str| land.and_then(|l| key_of(l, id)).and_then(|k| m.setting(k)).cloned();
    let mut out = Vec::new();
    match land {
        Some(Land::Dinamica) => {
            let s = setting(
                "steps",
                "Time steps between the two maps",
                "the transition rates are per step: the root of the matrix of the period",
                now("steps"),
            );
            out.push(ranged(s, 1.0, 1000.0, 1.0));
            let s =
                setting("simulate", "Steps to simulate", "one iteration of Expander and Patcher each", now("simulate"));
            out.push(ranged(s, 1.0, 10000.0, 1.0));
            out.push(start_setting(m));
            let s = setting(
                "increment",
                "Buffer increment of the weights",
                "the width of the buffers the ranges of each variable are cut by",
                now("increment"),
            );
            out.push(automatic(s, "automatic (a hundredth of each variable's range)", Json::from(1.0)));
            let s = setting(
                "expansion",
                "Share of the changes that expand patches",
                "the rest is made by new patches (Patcher)",
                now("expansion"),
            );
            out.push(automatic(ranged(s, 0.0, 1.0, 0.01), "as observed", Json::from(0.5)));
            let s = setting(
                "isometry",
                "Isometry of the patches",
                "how round the new patches are (Dinamica's usual is 1.5)",
                now("isometry"),
            );
            out.push(ranged(s, 0.0, 3.0, 0.1));
            out.push(setting("seed", "Seed", "the seed of the simulation", now("seed")));
        }
        Some(Land::Lulcc) => {
            let help = "CLUE-S: the classes compete for the cells until the demand is met; ordered: they are served one after the other, in the order given, each taking the cells it suits best";
            let mut method = setting("method", "Allocation", help, Some(Json::from(method_of(m))));
            method.param.choices = vec![Json::from("clue"), Json::from("ordered")];
            out.push(method);
            out.push(start_setting(m));
            let order = allocation_of(m).and_then(|a| a.get("order")).cloned();
            out.push(setting(
                "order",
                "The order classes are served in",
                "the codes of the classes, the first served first",
                order,
            ));
            let s = setting(
                "elasticity",
                "Elasticity (resistance to change, 0-1)",
                "one number for every class, or one for each in the order of the classes (0.5, 0.2, 1)",
                now("elasticity"),
            );
            out.push(s);
            let s = setting(
                "times",
                "The date of each observed map",
                "the dates of the two maps (the names of their bands, when they are numbers)",
                m.setting("demand.times").cloned(),
            );
            out.push(s);
            let s = setting(
                "out_times",
                "The dates simulated",
                "the dates the demand is given for, the simulation one step to each after the first",
                m.setting("demand.out_times").cloned(),
            );
            out.push(automatic(s, "every whole date from the first map to the last", json_numbers(&[0.0, 1.0])));
            out.push(setting("seed", "Seed", "the seed of the simulation", now("seed")));
        }
        Some(Land::Plus) => {
            let s = setting(
                "steps",
                "Time steps between the two maps",
                "the transition probabilities of the period are per step",
                now("steps"),
            );
            out.push(ranged(s, 1.0, 1000.0, 1.0));
            let s = setting("simulate", "Steps to simulate", "the steps of the demand and of CARS", now("simulate"));
            out.push(ranged(s, 1.0, 10000.0, 1.0));
            out.push(setting("seed", "Seed", "the seed of the simulation", now("seed")));
        }
        None => {}
    }
    for p in &m.parameters {
        out.push(Setting { id: p.key.clone(), param: p.clone(), now: m.setting(&p.key).cloned(), auto: None });
    }
    out
}

fn start_setting(m: &Model) -> Setting {
    let help = "the map of the second date goes forward in time; the first one runs the observed period again, to compare it with what happened";
    let mut s = setting("start", "The simulation starts from", help, Some(Json::from(start_of(m))));
    s.param.choices = vec![Json::from(START_LAST), Json::from(START_FIRST)];
    s
}

/// Where the simulation starts now: the first date when it reads what the first date's map is read as.
fn start_of(m: &Model) -> &'static str {
    match (dates_of(m), at(m, "simulation").and_then(|n| n.inputs.get("landscape"))) {
        (Some((a, _)), Some(Wire::Ref(b))) if a == *b => START_FIRST,
        _ => START_LAST,
    }
}

/// The references the maps of the first two dates are read by: Dinamica's `markov` reads them as `t0` and `t1`,
/// the stack of the observed maps of CLUE-S's has them first and second.
fn dates_of(m: &Model) -> Option<(String, String)> {
    let r = |s: &Wire| match s {
        Wire::Ref(r) => Some(r.clone()),
        Wire::Literal(_) => None,
    };
    match land(m)? {
        Land::Dinamica => {
            let n = m.node("markov")?;
            Some((r(n.inputs.get("t0")?)?, r(n.inputs.get("t1")?)?))
        }
        Land::Lulcc => {
            // (with every band of a file a date, `ends` keeps the first and the last: see [`plan`])
            let mut it = m.node("observed").or_else(|| m.node("ends"))?.inputs.values();
            Some((r(it.next()?)?, r(it.next()?)?))
        }
        Land::Plus => None,
    }
}

/// The numbers of a value typed or kept: a number, a list, or text with them (`1, 2, 3`).
fn numbers(v: &Json) -> Result<Vec<f64>, String> {
    let one = |x: &Json| match x {
        Json::Number(n) => n.as_f64().ok_or_else(|| format!("{n} is not a number")),
        Json::String(t) => t.trim().parse::<f64>().map_err(|_| format!("{:?} is not a number", t.trim())),
        other => Err(format!("{other} is not a number")),
    };
    match v {
        Json::Array(a) => a.iter().map(one).collect(),
        Json::String(t) => {
            t.split([',', ';', ' ']).filter(|p| !p.trim().is_empty()).map(|p| one(&Json::from(p))).collect()
        }
        x => Ok(vec![one(x)?]),
    }
}

fn json_numbers(v: &[f64]) -> Json {
    let one = |x: &f64| if x.fract() == 0.0 && x.abs() < 1e15 { Json::from(*x as i64) } else { Json::from(*x) };
    Json::Array(v.iter().map(one).collect())
}

/// A node's setting or literal input set (none: a setting taken away, back to the operator's default).
fn put(m: &mut Model, key: &str, v: Option<Json>) -> Result<(), String> {
    match v {
        Some(v) => m.apply(&BTreeMap::from([(key.to_string(), v)])).map_err(|e| e.to_string()),
        None => {
            let (path, name) = key.rsplit_once('.').ok_or_else(|| format!("{key:?}: give node.setting"))?;
            let n = m.node_mut(path).ok_or_else(|| format!("{key:?}: the model has no node {path:?}"))?;
            n.params.remove(name);
            Ok(())
        }
    }
}

/// Settings set in a model (the planned one): `values` by setting id (none: automatic). The model's exposed
/// parameters are set by their key; a land change model's by what each is (the method of the allocation decides
/// what its order and elasticity do; the start is the end of the chain of one date or the other).
pub fn apply_settings(m: &mut Model, values: &BTreeMap<String, Option<Json>>) -> Result<(), String> {
    let land = land(m);
    let ordered = values.get("method").is_some_and(|x| x.as_ref().and_then(Json::as_str) == Some("ordered"));
    for (id, v) in values {
        if let Some(key) = land.and_then(|l| key_of(l, id)) {
            // (an ordered allocation has no elasticity: its method takes it away below)
            if id == "elasticity" && ordered {
                continue;
            }
            let v = match (id.as_str(), v) {
                ("elasticity", Some(e)) => {
                    let n = numbers(e).map_err(|e| format!("the elasticity: {e}"))?;
                    Some(if n.len() == 1 { Json::from(n[0]) } else { json_numbers(&n) })
                }
                _ => v.clone(),
            };
            put(m, key, v)?;
        } else if matches!(id.as_str(), "times" | "out_times") && land == Some(Land::Lulcc) {
            let v = match v {
                Some(t) => Some(json_numbers(&numbers(t).map_err(|e| format!("the dates: {e}"))?)),
                None => None,
            };
            put(m, &format!("demand.{id}"), v)?;
        } else if (id.starts_with('$') || id.contains('.'))
            && let Some(v) = v
        {
            put(m, id, Some(v.clone()))?;
        }
    }
    if land == Some(Land::Lulcc) {
        set_allocation(m, values)?;
    }
    if let Some(Some(Json::String(s))) = values.get("start") {
        set_start(m, s == START_FIRST)?;
    }
    Ok(())
}

/// The allocation of a CLUE-S or ordered model by the method and order chosen: ordered has the order of the classes
/// and no elasticity, CLUE-S keeps the template's settings of its iteration.
fn set_allocation(m: &mut Model, values: &BTreeMap<String, Option<Json>>) -> Result<(), String> {
    let method = match values.get("method") {
        Some(Some(Json::String(s))) => s.clone(),
        _ => method_of(m),
    };
    let order = match values.get("order") {
        Some(Some(o)) => Some(numbers(o).map_err(|e| format!("the order of the classes: {e}"))?),
        _ => None,
    };
    let n = m.node_mut("simulation/allocate").ok_or("the model has no allocation")?;
    let mut alloc = n.params.get("allocation").and_then(Json::as_object).cloned().unwrap_or_default();
    match method.as_str() {
        "ordered" => {
            let order = match order {
                Some(o) => o,
                None => alloc.get("order").map(numbers).transpose()?.unwrap_or_default(),
            };
            if order.is_empty() {
                return Err("an ordered allocation needs the order of the classes".into());
            }
            alloc = serde_json::Map::from_iter([
                ("method".to_string(), Json::from("ordered")),
                ("order".to_string(), json_numbers(&order)),
            ]);
            n.params.remove("elasticity");
        }
        "clue" => {
            alloc.insert("method".into(), Json::from("clue"));
            alloc.remove("order");
        }
        other => return Err(format!("{other:?}: the allocation is clue or ordered")),
    }
    n.params.insert("allocation".into(), Json::Object(alloc));
    Ok(())
}

/// The simulation started from the map of the first date, or the second's: what reads the other's map as its
/// `landscape` (the nodes that start from it) reads this one's. The demand and the stack of the observed maps keep
/// reading both.
fn set_start(m: &mut Model, first: bool) -> Result<(), String> {
    let Some((a, b)) = dates_of(m) else {
        return Err("only Dinamica's and CLUE-S's models start from a date".into());
    };
    let (from, to) = if first { (b, a) } else { (a, b) };
    for id in ["regression", "suitability", "history", "simulation"] {
        if let Some(n) = m.node_mut(id)
            && let Some(Wire::Ref(r)) = n.inputs.get_mut("landscape")
            && *r == from
        {
            *r = to.clone();
            n.wires.remove("landscape");
        }
    }
    Ok(())
}

/// What the dates of two maps give a land change model before the user says: the steps
/// between the dates (whole years), and for CLUE-S the dates of the maps (their band names), those simulated (as
/// many as the template has, from the second date, or every whole date when it starts from the first), and for the
/// ordered allocation its order (the classes of the first map). `dates` are the band names of the two maps as numbers
/// (none: not a number); `user` what the user set already (the start decides what is simulated).
pub fn derive(
    m: &Model,
    dates: &[Option<f64>],
    order: &[i64],
    user: &BTreeMap<String, Option<Json>>,
) -> BTreeMap<String, Option<Json>> {
    let mut out = BTreeMap::new();
    let Some(land) = land(m) else { return out };
    let (Some(Some(a)), Some(Some(b))) = (dates.first(), dates.get(1)) else {
        return out;
    };
    let years = b - a;
    let whole = years >= 1.0 && years.fract() == 0.0;
    // (CLUE-S with more than two dates: all of them, the step from the last two)
    let all: Option<Vec<f64>> = dates.iter().copied().collect();
    let (last, step) = match &all {
        Some(t) if t.len() > 2 => (t[t.len() - 1], t[t.len() - 1] - t[t.len() - 2]),
        _ => (*b, years),
    };
    match land {
        Land::Dinamica => {
            if whole {
                out.insert("steps".into(), Some(Json::from(years as i64)));
                out.insert("simulate".into(), Some(Json::from(years as i64)));
            }
        }
        Land::Plus => {
            if whole {
                out.insert("steps".into(), Some(Json::from(years as i64)));
            }
        }
        Land::Lulcc => {
            let times = match &all {
                Some(t) if t.len() > 2 => t.clone(),
                _ => vec![*a, *b],
            };
            out.insert("times".into(), Some(json_numbers(&times)));
            let start = match user.get("start") {
                Some(Some(Json::String(s))) => s.as_str(),
                _ => start_of(m),
            };
            let out_times = if start == START_FIRST {
                None
            } else {
                let n = m.setting("demand.out_times").and_then(Json::as_array).map_or(4, Vec::len).max(2);
                let step = if step > 0.0 { step } else { 1.0 };
                Some(json_numbers(&(0..n).map(|k| last + step * k as f64).collect::<Vec<_>>()))
            };
            out.insert("out_times".into(), out_times);
            if !order.is_empty() {
                out.insert("order".into(), Some(json_numbers(&order.iter().map(|c| *c as f64).collect::<Vec<_>>())));
            }
        }
    }
    out
}

/// Whether a setting is shown for the method of the allocation: the order for the ordered one, the elasticity for
/// CLUE-S.
pub fn shown(id: &str, method: &str) -> bool {
    match id {
        "order" => method == "ordered",
        "elasticity" => method != "ordered",
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::TEMPLATES;
    use terraflux_graph::DataKind;
    use terraflux_ops::legend::Class;
    use terraflux_ops::presets;
    use terraflux_raster::{DataType, Raster};

    fn test_file(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name)
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("terraflux-my-data-{name}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn template(name: &str) -> Model {
        TEMPLATES.iter().find(|t| t.name == name).unwrap().model()
    }

    /// A map of w x h cells of side `cell`: the value of each cell by its row and column.
    fn write(d: &Path, name: &str, size: (usize, usize), cell: f64, f: impl Fn(usize, usize) -> f64) -> PathBuf {
        let mut g = terraflux_raster::Grid::new(size.0, size.1);
        g.transform = [1000.0, cell, 0.0, 1000.0 + cell * size.1 as f64, 0.0, -cell];
        let data = (0..size.0 * size.1).map(|i| f(i / size.0, i % size.0)).collect();
        let p = d.join(name);
        Raster::single(g, data, DataType::F32, None).write(&p).unwrap();
        p
    }

    fn choice(file: &Path) -> Choice {
        Choice { file: file.to_path_buf(), ..Default::default() }
    }

    fn one(node: &str, c: Choice) -> BTreeMap<String, Choice> {
        BTreeMap::from([(node.to_string(), c)])
    }

    fn run(p: &Plan, base: &Path) -> terraflux_exec::Results {
        let inputs = terraflux_exec::load_inputs(&p.model, base, &Default::default()).unwrap();
        terraflux_exec::Engine::default().run(&p.model, &inputs, &mut terraflux_exec::Quiet).unwrap()
    }

    fn errors(p: &Plan, base: &Path) -> Vec<String> {
        check(p, base).into_iter().filter(|f| f.level == Level::Error).map(|f| f.to_string()).collect()
    }

    fn ids(s: &[Source]) -> Vec<&str> {
        s.iter().map(|s| s.node.as_str()).collect()
    }

    /// Sibuyan's land use of 1997 as the user's classes of the forest (1 2 -> 1, 3 4 -> 3, 5 no data).
    fn forest_choice() -> Choice {
        let mut c = choice(&test_file("sibuyan_landuse.tif"));
        c.band = Some("1997".into());
        c.crosswalk = BTreeMap::from([(1, Some(1)), (2, Some(1)), (3, Some(3)), (4, Some(3)), (5, None)]);
        c
    }

    #[test]
    fn the_sources_of_the_templates() {
        let r = Registry::standard();
        assert_eq!(ids(&sources(&template("MSPA of a forest"), &r)), ["landscape"]);
        assert_eq!(ids(&sources(&template("Circuit connectivity"), &r)), ["quality", "resistance"]);
        assert_eq!(ids(&sources(&template("Species distribution model"), &r)), ["temperature", "rainfall", "relief"]);
        // the suitability is inside the drawn maps: not offered
        let m = template("Land change analysis");
        assert!(m.node("aptitude").is_some_and(|n| n.data.is_none()));
        assert_eq!(ids(&sources(&m, &r)), ["land_2000", "land_2010", "land_2020"]);
        let m = template("Dinamica EGO");
        let s = sources(&m, &r);
        assert_eq!(ids(&s), ["land_2000", "land_2010", "drivers"]);
        assert_eq!((s[0].needs, s[2].needs, s[2].port.as_str()), (Type::Map, Type::Stack, "stack"));
        assert_eq!(s[0].data.as_ref().unwrap().kind, DataKind::Classes);
    }

    #[test]
    fn the_loose_nodes_and_inputs_are_offered_after_the_marked() {
        let r = Registry::standard();
        let mut m = Model::new("t");
        let empty =
            Input::Map { path: String::new(), band: None, nodata: None, note: Some("the x".into()), data: None };
        m.inputs.insert("x".into(), empty);
        m.inputs
            .insert("y".into(), Input::Map { path: "y.tif".into(), band: None, nodata: None, note: None, data: None });
        m.nodes.push(Node::new("loose", "nlm.fbm").note("a loose one"));
        m.nodes.push(Node::new("read", "raster.read"));
        m.nodes.push(Node::new("done", "raster.read").param("path", "a.tif"));
        m.nodes.push(Node::new("inner", "nlm.fbm"));
        let marked = Node::new("marked", "raster.calc").input("a", "inner.map");
        m.nodes.push(marked.data(Data::new("a map", DataKind::Values)));
        let s = sources(&m, &r);
        assert_eq!(ids(&s), ["marked", "loose", "read", "x"]);
        assert_eq!(s[1].what, "a loose one");
        assert_eq!((s[3].op.as_str(), s[3].what.as_str()), (INPUT, "the x"));
    }

    #[test]
    fn mspa_of_a_forest_on_a_map_of_classes() {
        let r = Registry::standard();
        let m = template("MSPA of a forest");
        let d = dir("mspa");
        let p = plan(&m, &r, &one("landscape", forest_choice()), None, &d).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        let md = &p.model;
        assert!(md.node("landscape").is_none() && md.node("landscape_match").is_none());
        let Some(Input::Map { path, band, note, .. }) = md.inputs.get("landscape") else { panic!("{:?}", md.inputs) };
        assert!(path.ends_with("sibuyan_landuse.tif") && band.as_deref() == Some("1997"));
        assert!(note.as_ref().unwrap().contains("a land cover map with a forest class"));
        let rc = md.node("landscape_classes").unwrap();
        assert_eq!(rc.op, "raster.reclassify");
        assert_eq!(rc.params["rules"], "1 2 -> 1\n3 4 -> 3\n5 -> nan\n* -> nan");
        assert_eq!(rc.inputs["map"], Wire::Ref("$landscape".into()));
        assert!(rc.why.as_ref().unwrap().contains("in place of the drawn landscape"));
        // the template's names, on the last node, which carries the data
        let lg = md.node("landscape_legend").unwrap();
        assert_eq!(lg.op, "raster.legend");
        assert_eq!(lg.data, m.node("landscape").unwrap().data);
        assert_eq!(lg.params["classes"], serde_json::json!([[1, "farming"], [2, "water"], [3, "forest"]]));
        // what read the drawn map reads the end of the chain
        assert_eq!(md.node("forest").unwrap().inputs["map"], Wire::Ref("landscape_legend.map".into()));
        assert_eq!(p.reference, "landscape");
        assert!(p.notes.iter().any(|n| n.contains("raster.reclassify")), "{:?}", p.notes);
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
        // it runs, on the user's grid
        let out = run(&p, &d);
        let Some(terraflux_ops::Value::Map(mspa)) = out.get("mspa.map") else { panic!("no MSPA") };
        assert_eq!((mspa.grid.width, mspa.grid.height), (128, 108));
        assert!(mspa.data.iter().any(|v| !v.is_nan()));
        // a second Use my data finds the data again, and takes the first chain away
        assert_eq!(ids(&sources(&p.model, &r)), ["landscape_legend"]);
        let mut c = forest_choice();
        c.band = Some("2011".into());
        let p2 = plan(&p.model, &r, &one("landscape_legend", c), None, &d).unwrap();
        assert_eq!(p2.model.nodes.iter().filter(|n| n.op == "raster.legend").count(), 1);
        assert!(errors(&p2, &d).is_empty(), "{:?}", errors(&p2, &d));
        assert_eq!(p2.model.inputs.len(), 1, "{:?}", p2.model.inputs.keys().collect::<Vec<_>>());
    }

    #[test]
    fn a_file_on_another_grid_is_matched_and_the_drawn_maps_left_follow_the_reference() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        let r = Registry::standard();
        let m = template("Land change analysis");
        let d = dir("another-grid");
        // the second date on a grid twice as fine
        let src = Raster::read(test_file("sibuyan_landuse.tif")).unwrap();
        let mut fine = src.grid.clone();
        fine.width *= 2;
        fine.height *= 2;
        fine.transform[1] /= 2.0;
        fine.transform[5] /= 2.0;
        let (moved, _) =
            terraflux_raster::resample::resample(&src, &fine, terraflux_raster::resample::Method::Nearest).unwrap();
        let fine_file = d.join("landuse_fine.tif");
        moved.write(&fine_file).unwrap();
        let mut first = forest_choice();
        first.crosswalk = BTreeMap::from([(1, Some(1)), (2, Some(2)), (3, Some(2)), (4, Some(3)), (5, Some(3))]);
        let mut second = first.clone();
        second.file = fine_file;
        second.band = Some("2011".into());
        let choices = BTreeMap::from([("land_2000".to_string(), first), ("land_2010".to_string(), second)]);
        let p = plan(&m, &r, &choices, None, &d).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        let md = &p.model;
        // the first is the reference: no match; the second is matched by near (classes), then reclassified
        assert!(md.node("land_2000_match").is_none());
        let mt = md.node("land_2010_match").unwrap();
        assert_eq!((mt.op.as_str(), mt.params["resampling"].as_str()), ("raster.match", Some("near")));
        assert_eq!(mt.inputs["raster"], Wire::Ref("$land_2010".into()));
        assert_eq!(mt.inputs["like"], Wire::Ref("$land_2000".into()));
        assert_eq!(md.node("land_2010_classes").unwrap().inputs["map"], Wire::Ref("land_2010_match.raster".into()));
        // the drawn nodes left follow the reference grid
        let ap = md.node("aptitude").unwrap();
        assert_eq!(ap.inputs["like"], Wire::Ref("$land_2000".into()));
        assert!(!ap.params.contains_key("rows") && !ap.params.contains_key("columns"));
        // (the third date, still drawn, reads the second)
        assert!(md.node("land_2020").is_some());
        // the findings of the grids of the inputs are not told: the match puts them right
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
        let out = run(&p, &d);
        let Some(terraflux_ops::Value::Map(last)) = out.get("land_2020.map") else { panic!("no third date") };
        assert_eq!((last.grid.width, last.grid.height), (128, 108));
    }

    #[test]
    fn a_stack_replaces_the_drawn_drivers_and_what_only_they_read() {
        let r = Registry::standard();
        let m = template("Dinamica EGO");
        let d = dir("drivers");
        let choices = BTreeMap::from([
            ("land_2000".to_string(), forest_choice()),
            ("land_2010".to_string(), {
                let mut c = forest_choice();
                c.band = Some("2011".into());
                c
            }),
            ("drivers".to_string(), choice(&test_file("sibuyan_drivers.tif"))),
        ]);
        let p = plan(&m, &r, &choices, None, &d).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        assert!(matches!(p.model.inputs.get("drivers"), Some(Input::Stack { bands: None, .. })));
        // the suitability and the roughness fed the drawn maps only
        assert!(p.model.node("aptitude").is_none() && p.model.node("roughness").is_none());
        assert!(p.notes.iter().any(|n| n.starts_with("aptitude: taken away")), "{:?}", p.notes);
        assert!(p.model.nodes.iter().all(|n| n.op != "raster.match"));
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
    }

    #[test]
    fn values_are_matched_by_bilinear() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        let r = Registry::standard();
        let m = template("Species distribution model");
        let d = dir("bilinear");
        let a = write(&d, "temperature.tif", (50, 50), 100.0, |r, c| (r + c) as f64 / 100.0);
        let b = write(&d, "rainfall.tif", (100, 100), 50.0, |r, c| r as f64 / 100.0 + c as f64 / 200.0);
        let choices = BTreeMap::from([("temperature".to_string(), choice(&a)), ("rainfall".to_string(), choice(&b))]);
        let p = plan(&m, &r, &choices, None, &d).unwrap();
        assert_eq!(p.reference, "temperature");
        let mt = p.model.node("rainfall_match").unwrap();
        assert_eq!(mt.params["resampling"], "bilinear");
        assert!(p.model.node("temperature_match").is_none());
        // the relief left drawn on the temperature's grid; the stack of the predictors reads the three
        assert_eq!(p.model.node("relief").unwrap().inputs["like"], Wire::Ref("$temperature".into()));
        let preds = p.model.node("predictors").unwrap();
        assert_eq!(preds.inputs["rainfall"], Wire::Ref("rainfall_match.raster".into()));
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
        // naming the reference: the rainfall's grid
        let q = plan(&m, &r, &choices, Some("rainfall"), &d).unwrap();
        assert!(q.model.node("rainfall_match").is_none() && q.model.node("temperature_match").is_some());
        assert_eq!(q.model.node("relief").unwrap().inputs["like"], Wire::Ref("$rainfall".into()));
        assert!(plan(&m, &r, &choices, Some("relief"), &d).is_err());
    }

    #[test]
    fn circuit_connectivity_on_a_map_of_values_runs() {
        let r = Registry::standard();
        let m = template("Circuit connectivity");
        let d = dir("circuit");
        let f = write(&d, "resistance.tif", (60, 60), 30.0, |r, c| 1.0 + ((r / 6 + c / 6) % 10) as f64 * 5.0);
        let p = plan(&m, &r, &one("resistance", choice(&f)), None, &d).unwrap();
        assert!(p.model.nodes.iter().all(|n| n.op != "raster.reclassify" && n.op != "raster.match"));
        assert!(p.model.node("resistance").is_none());
        assert_eq!(p.model.node("omniscape").unwrap().inputs["resistance"], Wire::Ref("$resistance".into()));
        // the quality (drawn, it makes the cores) is on the file's grid
        assert_eq!(p.model.node("quality").unwrap().inputs["like"], Wire::Ref("$resistance".into()));
        // (no node to carry the data: the input does, and its note says what it is)
        let Some(Input::Map { note, data, .. }) = p.model.inputs.get("resistance") else { panic!() };
        assert!(note.as_ref().unwrap().contains("a resistance"));
        assert!(data.as_ref().unwrap().what.contains("a resistance"));
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
        let out = run(&p, &d);
        let Some(terraflux_ops::Value::Map(o)) = out.get("omniscape.normalized") else { panic!("no omniscape") };
        assert_eq!((o.grid.width, o.grid.height), (60, 60));
    }

    #[test]
    fn data_of_any_classes_keeps_the_users_classes() {
        let r = Registry::standard();
        let m = template("Landscape metrics");
        assert!(m.node("cover").unwrap().data.as_ref().unwrap().classes.is_empty());
        let d = dir("any-classes");
        let mut c = choice(&test_file("sibuyan_landuse.tif"));
        c.band = Some("2011".into());
        let p = plan(&m, &r, &one("cover", c.clone()), None, &d).unwrap();
        assert!(p.model.nodes.iter().all(|n| n.op != "raster.reclassify" && n.op != "raster.legend"));
        assert_eq!(p.model.node("classes").unwrap().inputs["map"], Wire::Ref("$cover".into()));
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
        let out = run(&p, &d);
        assert!(out.get("classes.table").is_some());
        // a table of classes for it is refused
        c.crosswalk = BTreeMap::from([(1, Some(1))]);
        let e = plan(&m, &r, &one("cover", c), None, &d).unwrap_err();
        assert!(e.contains("any classes"), "{e}");
    }

    /// A file read straight (no chain) keeps what it stands for on the input: a second run finds it, with its file.
    #[test]
    fn a_file_read_straight_keeps_its_data_on_the_input_and_a_second_run_replaces_it() {
        let r = Registry::standard();
        let m = template("Landscape metrics");
        let d = dir("straight");
        let mut c = choice(&test_file("sibuyan_landuse.tif"));
        c.band = Some("2011".into());
        let p = plan(&m, &r, &one("cover", c.clone()), None, &d).unwrap();
        assert!(p.model.nodes.iter().all(|n| n.data.is_none() || n.op != "raster.legend"));
        let Some(Input::Map { data: Some(data), path, .. }) = p.model.inputs.get("cover") else { panic!() };
        assert_eq!(data, m.node("cover").unwrap().data.as_ref().unwrap());
        assert!(path.ends_with("sibuyan_landuse.tif"));
        // offered again, with its file; the drawn ones before it
        let s = sources(&p.model, &r);
        assert_eq!(ids(&s), ["cover"]);
        assert_eq!((s[0].op.as_str(), s[0].what.as_str()), (INPUT, data.what.as_str()));
        assert!(s[0].file.as_deref().is_some_and(|f| f.ends_with("sibuyan_landuse.tif")));
        assert_eq!(s[0].data.as_ref(), Some(data));
        // another band of the same file in its place: one input, the data kept
        c.band = Some("1997".into());
        let q = plan(&p.model, &r, &one("cover", c), None, &d).unwrap();
        let Some(Input::Map { band, data: kept, .. }) = q.model.inputs.get("cover") else { panic!() };
        assert_eq!((band.as_deref(), kept.as_ref()), (Some("1997"), Some(data)));
        assert_eq!(q.model.inputs.len(), 1);
        assert!(errors(&q, &d).is_empty(), "{:?}", errors(&q, &d));
        // a chain: the data on its last node, not on the input as well
        let m = template("MSPA of a forest");
        let p = plan(&m, &r, &one("landscape", forest_choice()), None, &d).unwrap();
        assert!(p.model.inputs["landscape"].data().is_none());
        assert!(p.model.node("landscape_legend").unwrap().data.is_some());
        // an input of a value, a file with no data and an input with a path but no data are not offered
        let mut m = Model::new("t");
        m.inputs
            .insert("v".into(), Input::Map { path: "a.tif".into(), band: None, nodata: None, note: None, data: None });
        assert!(sources(&m, &r).is_empty());
    }

    /// The Wizard's two dates of land use from the model's two dates, in the three templates.
    #[test]
    fn the_land_change_templates_offer_the_wizards_settings() {
        let ids_of = |m: &Model| settings(m).into_iter().map(|s| s.id).collect::<Vec<_>>();
        let (din, clue, plus) = (template("Dinamica EGO"), template("CLUE-S"), template("PLUS (LEAS and CARS)"));
        assert_eq!(
            (land(&din), land(&clue), land(&plus), land(&template("MSPA of a forest"))),
            (Some(Land::Dinamica), Some(Land::Lulcc), Some(Land::Plus), None)
        );
        assert_eq!(ids_of(&din), ["steps", "simulate", "start", "increment", "expansion", "isometry", "seed"]);
        assert_eq!(ids_of(&clue), ["method", "start", "order", "elasticity", "times", "out_times", "seed"]);
        assert_eq!(ids_of(&plus), ["steps", "simulate", "seed"]);
        assert!(settings(&template("MSPA of a forest")).is_empty());
        // what they are now
        let now = |m: &Model, id: &str| settings(m).into_iter().find(|s| s.id == id).unwrap().now;
        assert_eq!(
            (now(&din, "steps"), now(&din, "simulate"), now(&din, "increment")),
            (Some(1.into()), Some(3.into()), None)
        );
        assert_eq!(now(&din, "start"), Some("the second date".into()));
        assert_eq!(now(&clue, "method"), Some("clue".into()));
        assert_eq!(now(&clue, "times"), Some(serde_json::json!([2000.0, 2010.0])));
        assert_eq!(now(&clue, "seed"), Some(7.into()));
        // the exposed parameters come after, by their key
        let mut m = template("Circuit connectivity");
        m.parameters.push(Parameter { key: "omniscape.radius".into(), ..Default::default() });
        assert_eq!(ids_of(&m), ["omniscape.radius"]);
    }

    #[test]
    fn the_settings_are_set_in_the_planned_model() {
        let mut m = template("Dinamica EGO");
        let values: BTreeMap<String, Option<Json>> = BTreeMap::from([
            ("steps".to_string(), Some(Json::from(14))),
            ("simulate".to_string(), Some(Json::from(2))),
            ("increment".to_string(), Some(Json::from(0.5))),
            ("expansion".to_string(), Some(Json::from(0.25))),
            ("isometry".to_string(), Some(Json::from(1.0))),
            ("seed".to_string(), Some(Json::from(1))),
        ]);
        apply_settings(&mut m, &values).unwrap();
        let n = |m: &Model, id: &str, k: &str| m.setting(&format!("{id}.{k}")).cloned();
        assert_eq!(n(&m, "markov", "steps"), Some(14.into()));
        assert_eq!(n(&m, "simulation", "times"), Some(2.into()));
        assert_eq!(n(&m, "woe", "increment"), Some(0.5.into()));
        assert_eq!(n(&m, "patches", "expansion"), Some(0.25.into()));
        assert_eq!(n(&m, "patches", "isometry"), Some(1.0.into()));
        assert_eq!(n(&m, "simulation/allocate", "seed"), Some(1.into()));
        // automatic: the setting goes back to the operator's default
        apply_settings(&mut m, &BTreeMap::from([("increment".to_string(), None), ("expansion".to_string(), None)]))
            .unwrap();
        assert!(n(&m, "woe", "increment").is_none() && n(&m, "patches", "expansion").is_none());
        // the simulation from the first date: what read the second's map reads the first's
        let t1 = m.node("simulation").unwrap().inputs["landscape"].clone();
        let t0 = m.node("markov").unwrap().inputs["t0"].clone();
        assert_ne!(t0, t1);
        let first = BTreeMap::from([("start".to_string(), Some(Json::from("the first date")))]);
        apply_settings(&mut m, &first).unwrap();
        assert_eq!(m.node("simulation").unwrap().inputs["landscape"], t0);
        assert_eq!(settings(&m).iter().find(|s| s.id == "start").unwrap().now, Some("the first date".into()));
        let last = BTreeMap::from([("start".to_string(), Some(Json::from("the second date")))]);
        apply_settings(&mut m, &last).unwrap();
        assert_eq!(m.node("simulation").unwrap().inputs["landscape"], t1);
        // a key that is not a node's: said
        assert!(apply_settings(&mut m, &BTreeMap::from([("nothing.x".to_string(), Some(Json::from(1)))])).is_err());

        // CLUE-S: the elasticity as typed, the dates as text, the ordered allocation with its order, no elasticity
        let mut c = template("CLUE-S");
        let typed = BTreeMap::from([
            ("elasticity".to_string(), Some(Json::from("0.2"))),
            ("times".to_string(), Some(Json::from("1997, 2011"))),
            ("out_times".to_string(), None),
            ("seed".to_string(), Some(Json::from(1))),
        ]);
        apply_settings(&mut c, &typed).unwrap();
        assert_eq!(c.setting("simulation/allocate.elasticity"), Some(&Json::from(0.2)));
        assert_eq!(c.setting("demand.times"), Some(&serde_json::json!([1997, 2011])));
        assert!(c.setting("demand.out_times").is_none());
        assert_eq!(c.setting("rng.seed"), Some(&Json::from(1)));
        assert_eq!(method_of(&c), "clue");
        let scale = c.setting("simulation/allocate.allocation").unwrap()["scale"].clone();
        assert!(scale.as_f64().is_some(), "the template's iteration is kept: {scale}");
        // (ordered with no order: told)
        let ordered = BTreeMap::from([("method".to_string(), Some(Json::from("ordered")))]);
        assert!(apply_settings(&mut c, &ordered).unwrap_err().contains("order"));
        let with = BTreeMap::from([
            ("method".to_string(), Some(Json::from("ordered"))),
            ("order".to_string(), Some(Json::from("2, 1, 3"))),
        ]);
        apply_settings(&mut c, &with).unwrap();
        assert_eq!(
            c.setting("simulation/allocate.allocation"),
            Some(&serde_json::json!({"method": "ordered", "order": [2, 1, 3]}))
        );
        assert!(c.setting("simulation/allocate.elasticity").is_none());
        assert_eq!(method_of(&c), "ordered");
        // and back to CLUE-S's: no order
        apply_settings(&mut c, &BTreeMap::from([("method".to_string(), Some(Json::from("clue")))])).unwrap();
        assert!(c.setting("simulation/allocate.allocation").unwrap().get("order").is_none());
        assert!(apply_settings(&mut c, &BTreeMap::from([("method".to_string(), Some(Json::from("x")))])).is_err());
        assert!(
            apply_settings(&mut c, &BTreeMap::from([("elasticity".to_string(), Some(Json::from("lots")))])).is_err()
        );
    }

    #[test]
    fn the_dates_of_the_bands_give_the_first_settings() {
        let none = BTreeMap::new();
        let dates = [Some(1997.0), Some(2011.0)];
        // Dinamica: the steps between the dates, and as many to simulate (the Wizard's)
        let d = derive(&template("Dinamica EGO"), &dates, &[], &none);
        assert_eq!(
            d,
            BTreeMap::from([
                ("steps".to_string(), Some(Json::from(14))),
                ("simulate".to_string(), Some(Json::from(14)))
            ])
        );
        // not whole years, or no change of date: nothing
        assert!(derive(&template("Dinamica EGO"), &[Some(1.0), Some(1.5)], &[], &none).is_empty());
        assert!(derive(&template("Dinamica EGO"), &[Some(2000.0), None], &[], &none).is_empty());
        assert!(derive(&template("MSPA of a forest"), &dates, &[], &none).is_empty());
        // PLUS: the steps only
        assert_eq!(derive(&template("PLUS (LEAS and CARS)"), &dates, &[], &none).len(), 1);
        // CLUE-S: the dates, three steps of the period from the second date, the order of the classes
        let c = derive(&template("CLUE-S"), &dates, &[1, 2, 3], &none);
        assert_eq!(c["times"], Some(serde_json::json!([1997, 2011])));
        assert_eq!(c["out_times"], Some(serde_json::json!([2011, 2025, 2039, 2053])));
        assert_eq!(c["order"], Some(serde_json::json!([1, 2, 3])));
        // from the first date: every whole date
        let first = BTreeMap::from([("start".to_string(), Some(Json::from("the first date")))]);
        assert_eq!(derive(&template("CLUE-S"), &dates, &[], &first)["out_times"], None);
    }

    /// The model's maps of a run, step by step.
    fn steps_of(m: &Model, base: &Path, from: &str) -> Vec<terraflux_ops::Map> {
        let inputs = terraflux_exec::load_inputs(m, base, &Default::default()).unwrap();
        let out = terraflux_exec::Engine::default().run(m, &inputs, &mut terraflux_exec::Quiet).unwrap();
        let Some(terraflux_ops::Value::Series(s)) = out.get(from) else { panic!("{from} is no series") };
        s.iter()
            .map(|v| match v {
                terraflux_ops::Value::Map(m) => m.clone(),
                _ => panic!("a step that is no map"),
            })
            .collect()
    }

    /// The maps cell by cell (a cell with no data is the same as another).
    fn same_cells(a: &[terraflux_ops::Map], b: &[terraflux_ops::Map]) -> Result<(), String> {
        if a.len() != b.len() {
            return Err(format!("{} steps against {}", a.len(), b.len()));
        }
        for (k, (x, y)) in a.iter().zip(b).enumerate() {
            if (x.grid.width, x.grid.height) != (y.grid.width, y.grid.height) || x.data.len() != y.data.len() {
                return Err(format!("step {k}: the grids differ"));
            }
            let differ =
                x.data.iter().zip(y.data.iter()).filter(|(p, q)| !(p == q || (p.is_nan() && q.is_nan()))).count();
            if differ > 0 {
                return Err(format!("step {k}: {differ} of {} cells differ", x.data.len()));
            }
        }
        Ok(())
    }

    /// Sibuyan's two dates and drivers in Use my data's three sources, the user's classes kept as they are (as the
    /// Wizard reads them).
    fn sibuyan_choices() -> BTreeMap<String, Choice> {
        let mut first = choice(&test_file("sibuyan_landuse.tif"));
        first.band = Some("1997".into());
        let mut second = first.clone();
        second.band = Some("2011".into());
        BTreeMap::from([
            ("land_2000".to_string(), first),
            ("land_2010".to_string(), second),
            ("drivers".to_string(), choice(&test_file("sibuyan_drivers.tif"))),
        ])
    }

    /// What the Wizard's model was for Sibuyan's two dates (the Wizard is retired: its model written with the
    /// presets, the inputs as it made them): the steps between the dates, and as many simulated.
    fn dinamica_reference(simulate: usize) -> Model {
        let (lu, dr) = (test_file("sibuyan_landuse.tif"), test_file("sibuyan_drivers.tif"));
        let text = |p: &Path| p.to_string_lossy().replace('\\', "/");
        let band = |b: &str| Input::Map {
            path: text(&lu),
            band: Some(b.into()),
            nodata: None,
            note: Some(format!("land use in {b}")),
            data: None,
        };
        let mut m = Model::new("Dinamica EGO's classic model");
        let drivers = Input::Stack {
            path: text(&dr),
            bands: None,
            nodata: None,
            note: Some("the explanatory variables".into()),
            data: None,
        };
        m.inputs.insert("drivers".into(), drivers);
        m.inputs.insert("seed".into(), Input::Value { value: 1.into(), note: Some("the seed".into()) });
        m.inputs.insert("t0".into(), band("1997"));
        m.inputs.insert("t1".into(), band("2011"));
        let p = presets::Dinamica { steps: 14, simulate, isometry: 1.5, ..Default::default() };
        presets::dinamica(&mut m, &p);
        m
    }

    /// What the Wizard's CLUE-S or ordered model was for a file of land use with a band for each date, simulating
    /// from its first band (the Wizard is retired: its model written with the presets, the inputs as it made them).
    fn lulcc_reference(lu: &Path, dr: &Path, dates: &[f64], order: Option<&[i64]>) -> Model {
        let text = |p: &Path| p.to_string_lossy().replace('\\', "/");
        let mut m = Model::new("lulcc");
        let drivers = Input::Stack {
            path: text(dr),
            bands: None,
            nodata: None,
            note: Some("the explanatory variables".into()),
            data: None,
        };
        m.inputs.insert("drivers".into(), drivers);
        m.inputs.insert("seed".into(), Input::Value { value: 1.into(), note: Some("the seed".into()) });
        let observed = Input::Stack {
            path: text(lu),
            bands: None,
            nodata: None,
            note: Some("the observed maps, one per date".into()),
            data: None,
        };
        m.inputs.insert("observed".into(), observed);
        let start = Input::Map {
            path: text(lu),
            band: Some(format!("{}", dates[0])),
            nodata: None,
            note: Some("the map the simulation starts from".into()),
            data: None,
        };
        m.inputs.insert("start".into(), start);
        let allocation = match order {
            None => serde_json::json!({"method": "clue"}),
            Some(o) => serde_json::json!({"method": "ordered", "order": o}),
        };
        presets::lulcc(
            &mut m,
            &presets::Lulcc {
                observed: "observed".into(),
                drivers: "drivers".into(),
                seed: "seed".into(),
                start: "start".into(),
                restrictions: None,
                history: None,
                times: dates.to_vec(),
                formulas: None,
                allocation,
                elasticity: order.is_none().then(|| serde_json::json!(0.2)),
                allowed: None,
                neighbourhood: None,
            },
        );
        m
    }

    /// Parity with the Wizard (its model, built by the presets as it built it: [`dinamica_reference`]), Dinamica EGO's
    /// classic model. The Wizard's model simulated from its first date (the observed period again); the template's
    /// from the second (the future): "start from the first date" is the setting that makes them the same, and the
    /// maps are then the same cell by cell.
    #[test]
    fn dinamica_by_use_my_data_gives_what_the_wizard_gives() {
        let r = Registry::standard();
        let d = dir("parity-dinamica");
        let wm = dinamica_reference(2);
        let mut p = plan(&template("Dinamica EGO"), &r, &sibuyan_choices(), None, &d).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        let mut values = derive(&p.model, &[Some(1997.0), Some(2011.0)], &[], &BTreeMap::new());
        assert_eq!(values["steps"], Some(Json::from(14)));
        values.insert("simulate".into(), Some(Json::from(2)));
        values.insert("seed".into(), Some(Json::from(1)));
        values.insert("start".into(), Some(Json::from("the first date")));
        apply_settings(&mut p.model, &values).unwrap();
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
        // the findings are the Wizard's: the same data, the same words (the inputs' names apart)
        let inputs = terraflux_exec::load_inputs(&wm, Path::new(""), &Default::default()).unwrap();
        let mut wizard_findings: Vec<String> =
            terraflux_exec::lint::lint(&wm, &inputs).iter().map(|f| f.to_string()).collect();
        let mut mine: Vec<String> = check(&p, &d)
            .iter()
            .filter(|f| f.at.starts_with('$'))
            .map(|f| f.to_string().replace("$land_2000", "$t0").replace("$land_2010", "$t1"))
            .collect();
        wizard_findings.sort();
        mine.sort();
        assert_eq!(wizard_findings, mine);
        let (a, b) = (steps_of(&wm, Path::new(""), "simulation.maps"), steps_of(&p.model, &d, "simulation.maps"));
        assert_eq!(a.len(), 2);
        same_cells(&a, &b).unwrap();
        // from the second date it is another run: the template's way (the future)
        let mut q = plan(&template("Dinamica EGO"), &r, &sibuyan_choices(), None, &d).unwrap();
        values.insert("start".into(), Some(Json::from("the second date")));
        apply_settings(&mut q.model, &values).unwrap();
        assert!(same_cells(&a, &steps_of(&q.model, &d, "simulation.maps")).is_err());
    }

    /// Parity with the Wizard's CLUE-S and ordered models ([`lulcc_reference`]). The template's allocation iterates
    /// in smaller steps than lulcc's default (drawn maps of 25,600 cells): the test gives the Wizard's model the
    /// template's, the rest being the same.
    #[test]
    fn clue_s_and_ordered_by_use_my_data_give_what_the_wizard_gives() {
        let r = Registry::standard();
        let d = dir("parity-clue");
        let (lu, dr) = (test_file("sibuyan_landuse.tif"), test_file("sibuyan_drivers.tif"));
        for ordered in [false, true] {
            let kind = if ordered { "ordered" } else { "CLUE-S" };
            let order = classes_of(&lu, Some("1997")).unwrap();
            let order: Vec<i64> = order.iter().map(|c| c.0).collect();
            let mut wm = lulcc_reference(&lu, &dr, &[1997.0, 2011.0], ordered.then_some(&order[..]));
            let mut p = plan(&template("CLUE-S"), &r, &sibuyan_choices(), None, &d).unwrap();
            let mut values = derive(&p.model, &[Some(1997.0), Some(2011.0)], &order, &BTreeMap::new());
            values.insert("seed".into(), Some(Json::from(1)));
            values.insert("start".into(), Some(Json::from("the first date")));
            values.insert("elasticity".into(), Some(Json::from(0.2)));
            if ordered {
                values.insert("method".into(), Some(Json::from("ordered")));
            }
            // (the dates simulated: every whole date, as the Wizard's)
            let derive_first = derive(&p.model, &[Some(1997.0), Some(2011.0)], &order, &values);
            assert_eq!(derive_first["out_times"], None);
            values.extend(derive_first);
            apply_settings(&mut p.model, &values).unwrap();
            if !ordered {
                let template_allocation = p.model.setting("simulation/allocate.allocation").unwrap().clone();
                wm.node_mut("simulation/allocate").unwrap().params.insert("allocation".into(), template_allocation);
            }
            assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
            let (a, b) = (steps_of(&wm, Path::new(""), "simulation.maps"), steps_of(&p.model, &d, "simulation.maps"));
            assert_eq!(a.len(), 14, "{kind}: the 14 years of the period");
            if let Err(e) = same_cells(&a, &b) {
                panic!("{kind}: {e}");
            }
        }
    }

    /// Parity with the Wizard's CLUE-S and ordered models with three dates (the bands of a file of three: 1985, 1991,
    /// 1999; [`lulcc_reference`]): the same maps cell by cell (checked against the Wizard itself when it was retired:
    /// 14 steps of 215,698 cells, CLUE-S and ordered).
    #[test]
    fn clue_s_and_ordered_with_three_dates_give_what_the_wizard_gave() {
        let r = Registry::standard();
        let d = dir("parity-three-dates");
        let (lu, dr) = (test_file("pie_landuse.tif"), test_file("pie_drivers.tif"));
        for ordered in [false, true] {
            let kind = if ordered { "ordered" } else { "CLUE-S" };
            let order = classes_of(&lu, Some("1985")).unwrap();
            let order: Vec<i64> = order.iter().map(|c| c.0).collect();
            let dates = [1985.0, 1991.0, 1999.0];
            let mut wm = lulcc_reference(&lu, &dr, &dates, ordered.then_some(&order[..]));
            let mut first = choice(&lu);
            first.band = Some("1985".into());
            first.dates = true;
            let mut last = choice(&lu);
            last.band = Some("1999".into());
            let choices = BTreeMap::from([
                ("land_2000".to_string(), first),
                ("land_2010".to_string(), last),
                ("drivers".to_string(), choice(&dr)),
            ]);
            let mut p = plan(&template("CLUE-S"), &r, &choices, None, &d).unwrap();
            assert!(p.refused.is_empty(), "{:?}", p.refused);
            assert!(p.model.node("observed").is_none() && p.model.node("ends").is_some());
            assert!(matches!(p.model.inputs.get("observed"), Some(Input::Stack { bands: None, .. })));
            let dated = [Some(1985.0), Some(1991.0), Some(1999.0)];
            let mut values = derive(&p.model, &dated, &order, &BTreeMap::new());
            assert_eq!(values["times"], Some(serde_json::json!([1985, 1991, 1999])));
            values.insert("seed".into(), Some(Json::from(1)));
            values.insert("start".into(), Some(Json::from("the first date")));
            values.insert("elasticity".into(), Some(Json::from(0.2)));
            if ordered {
                values.insert("method".into(), Some(Json::from("ordered")));
            }
            values.extend(derive(&p.model, &dated, &order, &values));
            apply_settings(&mut p.model, &values).unwrap();
            if !ordered {
                let template_allocation = p.model.setting("simulation/allocate.allocation").unwrap().clone();
                wm.node_mut("simulation/allocate").unwrap().params.insert("allocation".into(), template_allocation);
            }
            assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
            let (a, b) = (steps_of(&wm, Path::new(""), "simulation.maps"), steps_of(&p.model, &d, "simulation.maps"));
            assert_eq!(a.len(), 14, "{kind}: the 14 years from 1985 to 1999");
            if let Err(e) = same_cells(&a, &b) {
                panic!("{kind}: {e}");
            }
        }
    }

    /// A stack of drivers on another grid is put on the land use map's by raster.match (the Wizard's Resample
    /// button), its method chosen by the data (`auto`: the mean of the cells where they are coarser, else the
    /// nearest), and the check of the grids says nothing of them.
    #[test]
    fn drivers_on_another_grid_are_matched_as_the_wizards_resample_does() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        let r = Registry::standard();
        let d = dir("drivers-grid");
        let src = Raster::read(test_file("sibuyan_drivers.tif")).unwrap();
        let mut fine = src.grid.clone();
        fine.width *= 2;
        fine.height *= 2;
        fine.transform[1] /= 2.0;
        fine.transform[5] /= 2.0;
        let (moved, _) =
            terraflux_raster::resample::resample(&src, &fine, terraflux_raster::resample::Method::Nearest).unwrap();
        let file = d.join("drivers_fine.tif");
        moved.write(&file).unwrap();
        let mut choices = sibuyan_choices();
        choices.insert("drivers".into(), choice(&file));
        let p = plan(&template("Dinamica EGO"), &r, &choices, Some("land_2000"), &d).unwrap();
        let mt = p.model.node("drivers_match").unwrap();
        assert_eq!((mt.op.as_str(), mt.params["resampling"].as_str()), ("raster.match", Some("auto")));
        assert_eq!(mt.inputs["like"], Wire::Ref("$land_2000".into()));
        // what read the drivers reads them on the grid of the land use
        assert_eq!(p.model.node("woe").unwrap().inputs["drivers"], Wire::Ref("drivers_match.raster".into()));
        assert!(errors(&p, &d).is_empty(), "{:?}", errors(&p, &d));
    }

    #[test]
    fn what_cannot_be_done_is_refused() {
        let r = Registry::standard();
        let m = template("MSPA of a forest");
        let d = dir("refused");
        let file = test_file("sibuyan_landuse.tif");
        // no choice
        assert!(plan(&m, &r, &BTreeMap::new(), None, &d).unwrap_err().contains("at least one"));
        // a stack for a map
        let mut c = choice(&file);
        c.bands = Some(vec!["1997".into(), "2011".into()]);
        let e = plan(&m, &r, &one("landscape", c), None, &d).unwrap_err();
        assert!(e.contains("a stack of bands for a map"), "{e}");
        // a class that is not the template's
        let mut c = forest_choice();
        c.crosswalk.insert(2, Some(9));
        let e = plan(&m, &r, &one("landscape", c), None, &d).unwrap_err();
        assert!(e.contains("class 9 is not among the template's classes"), "{e}");
        // a band the file has not, a file that is not there
        let mut c = choice(&file);
        c.band = Some("1900".into());
        let e = plan(&m, &r, &one("landscape", c), None, &d).unwrap_err();
        assert!(e.contains("has no band"), "{e}");
        assert!(plan(&m, &r, &one("landscape", choice(&d.join("none.tif"))), None, &d).is_err());
        // a table that maps nothing
        let mut c = choice(&file);
        c.crosswalk = BTreeMap::from([(1, None)]);
        assert!(plan(&m, &r, &one("landscape", c), None, &d).is_err());
        // a choice for what the model has not is told; the one that can be made is made
        let choices =
            BTreeMap::from([("landscape".to_string(), forest_choice()), ("nothing".to_string(), choice(&file))]);
        let p = plan(&m, &r, &choices, None, &d).unwrap();
        assert_eq!(p.refused.len(), 1);
        assert!(p.refused[0].starts_with("nothing:"), "{:?}", p.refused);
    }

    #[test]
    fn the_classes_of_a_file_and_a_first_table() {
        let d = dir("classes");
        let names = [(1.0, "Forest"), (2.0, "Water"), (7.0, "Crops")];
        let classes: Vec<Class> =
            names.iter().map(|(v, n)| Class { value: *v, name: n.to_string(), colour: String::new() }).collect();
        let g = terraflux_ops::legend::with_legend(&terraflux_raster::Grid::new(4, 3), Some(&classes));
        let data = vec![1.0, 1.0, 1.0, 2.0, 2.0, 7.0, 7.0, 7.0, 7.0, f64::NAN, 7.0, 7.0];
        let p = d.join("named.tif");
        Raster::single(g, data, DataType::F32, None).write(&p).unwrap();
        let cls = classes_of(&p, None).unwrap();
        let want = vec![
            (1, Some("Forest".to_string()), 3),
            (2, Some("Water".to_string()), 2),
            (7, Some("Crops".to_string()), 6),
        ];
        assert_eq!(cls, want);
        let template =
            BTreeMap::from([(1, "farming".to_string()), (2, "water".to_string()), (3, "forest".to_string())]);
        // by name, the case of letters apart; a name the template lacks, no class (Crops)
        assert_eq!(suggest(&template, &cls), BTreeMap::from([(1, Some(3)), (2, Some(2)), (7, None)]));
        let cls = vec![(3, Some(" FOREST".to_string()), 1), (4, Some("farming".to_string()), 1)];
        assert_eq!(suggest(&template, &cls), BTreeMap::from([(3, Some(3)), (4, Some(1))]));
        // without names, the same codes are the same classes
        let cls = vec![(1, None, 5), (2, None, 5), (5, None, 5)];
        assert_eq!(suggest(&template, &cls), BTreeMap::from([(1, Some(1)), (2, Some(2)), (5, None)]));
        // a map of values is not a map of classes; nor is one of many values
        let f = write(&d, "values.tif", (4, 4), 1.0, |r, c| (r * 4 + c) as f64 / 3.0);
        assert!(classes_of(&f, None).unwrap_err().contains("fractional"));
        let f = write(&d, "many.tif", (20, 20), 1.0, |r, c| (r * 20 + c) as f64);
        assert_eq!(classes_of(&f, None).unwrap_err(), "not a map of classes: more than 256 values");
        // a band of a file with bands
        let c = classes_of(&test_file("sibuyan_landuse.tif"), Some("2011")).unwrap();
        assert_eq!(c.iter().map(|c| c.2).sum::<usize>(), 4160 + 1404 + 1090 + 288 + 284);
        assert!(classes_of(&test_file("sibuyan_landuse.tif"), Some("nope")).is_err());
    }

    /// A file of bands, a band by name (none: unnamed): its cell `(row, column)` is `f(band, row, column)`.
    fn write_bands(d: &Path, name: &str, names: &[Option<&str>], f: impl Fn(usize, usize, usize) -> f64) -> PathBuf {
        let (w, h) = (20, 20);
        let mut g = terraflux_raster::Grid::new(w, h);
        g.transform = [1000.0, 10.0, 0.0, 1200.0, 0.0, -10.0];
        let bands = names
            .iter()
            .enumerate()
            .map(|(b, n)| terraflux_raster::Band {
                name: n.map(String::from),
                data: (0..w * h).map(|i| f(b, i / w, i % w)).collect(),
            })
            .collect();
        let p = d.join(name);
        Raster { grid: g, bands, dtype: DataType::F32, nodata: None }.write(&p).unwrap();
        p
    }

    /// nlm.image -> segment.slic -> objects.features, as the templates of OBIA are built.
    fn image_model(marked: bool) -> Model {
        let mut m = Model::new("obia");
        m.nodes.push(Node::new("land", "nlm.fbm").param("rows", 20).param("columns", 20));
        m.nodes.push(Node::new("classes", "nlm.classify").input("map", "land.map").param("n", 3));
        let image = Node::new("image", "nlm.image").input("classes", "classes.classes");
        m.nodes.push(if marked { image.data(Data::new("a multispectral image", DataKind::Image)) } else { image });
        m.nodes.push(Node::new("segments", "segment.slic").input("image", "image.image").param("size", 4));
        m.nodes.push(
            Node::new("features", "objects.features")
                .input("objects", "segments.objects")
                .input("image", "image.image"),
        );
        m
    }

    const S2: [Option<&str>; 7] =
        [Some("B01"), Some("B02"), Some("B03"), Some("B04"), Some("B08"), Some("B11"), Some("B12")];

    #[test]
    fn an_image_is_offered_as_data_of_kind_image_or_as_an_nlm_image() {
        let r = Registry::standard();
        for marked in [true, false] {
            let s = sources(&image_model(marked), &r);
            let img = s.iter().find(|s| s.node == "image").unwrap();
            assert!(is_image(img), "{marked}");
            assert_eq!((img.needs, img.port.as_str()), (Type::Stack, "image"));
        }
    }

    #[test]
    fn an_image_file_replaces_the_drawn_image_with_its_bands_named_and_scaled() {
        let r = Registry::standard();
        let d = dir("image");
        let f = write_bands(&d, "s2.tif", &S2, |b, row, col| 1000.0 * (b + 1) as f64 + (row + col) as f64 * 10.0);
        let info = image_info(&f).unwrap();
        assert_eq!(info.roles, ["other", "blue", "green", "red", "nir", "swir1", "swir2"]);
        assert_eq!(info.scale, Scale::Div10000);
        for marked in [true, false] {
            let p = plan(&image_model(marked), &r, &one("image", choice(&f)), None, &d).unwrap();
            assert!(p.refused.is_empty(), "{:?}", p.refused);
            let md = &p.model;
            for gone in ["image", "land", "classes"] {
                assert!(md.node(gone).is_none(), "{gone}");
            }
            let n = md.node("image_bands").expect("the rename");
            assert_eq!(n.op, "stack.rename");
            assert_eq!(n.params["names"], serde_json::json!(["", "blue", "green", "red", "nir", "swir1", "swir2"]));
            assert_eq!(n.params["scale"], serde_json::json!(1e-4));
            assert!(!n.params.contains_key("offset"));
            assert_eq!(n.data.is_some(), marked);
            assert!(matches!(n.inputs.get("stack"), Some(Wire::Ref(r)) if r == "$image"));
            for c in ["segments", "features"] {
                let read = md.node(c).unwrap().inputs.get("image");
                assert!(matches!(read, Some(Wire::Ref(r)) if r == "image_bands.stack"), "{c}: {read:?}");
            }
            assert!(matches!(md.inputs.get("image"), Some(Input::Stack { bands: None, .. })));
            assert_eq!(errors(&p, &d), Vec::<String>::new());
            let out = run(&p, &d);
            let Some(terraflux_ops::Value::Table(t)) = out.get("features.table") else { panic!("no features") };
            assert!(t.columns.iter().any(|c| c == "mean_nir"), "{:?}", t.columns);
            assert!(!t.columns.iter().any(|c| c.contains("B01") || c.contains("other")), "{:?}", t.columns);
            // (B08 is 5000 to 5400 over 10 000)
            let k = t.columns.iter().position(|c| c == "mean_nir").unwrap();
            assert!(t.rows.iter().all(|row| (0.5..0.55).contains(&row[k])), "{:?}", t.rows[0]);
        }
    }

    /// A folder of Sentinel-2's bands: B02, B03, B04 and B08 on 10 m cells, B11 and B12 on 20 m, and a true colour
    /// image that is no band.
    fn band_folder(name: &str) -> PathBuf {
        let d = dir(name);
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let _ = std::fs::remove_file(e.path());
        }
        for (k, band) in ["B02", "B03", "B04", "B08", "B11", "B12", "TCI"].iter().enumerate() {
            let coarse = matches!(*band, "B11" | "B12");
            let (size, cell) = if coarse { ((10, 10), 20.0) } else { ((20, 20), 10.0) };
            let f = |row: usize, col: usize| 1000.0 * (k + 1) as f64 + (row + col) as f64 * 10.0;
            let res = if coarse { "20m" } else { "10m" };
            write(&d, &format!("T32UMU_20240101T103021_{band}_{res}.tif"), size, cell, f);
        }
        d
    }

    #[test]
    fn a_folder_of_single_band_files_is_stacked_in_band_order_with_the_roles_guessed() {
        let r = Registry::standard();
        let d = band_folder("band-folder");
        let files = band_files(std::slice::from_ref(&d)).unwrap();
        let names: Vec<String> = files.iter().map(|f| stem(f)).collect();
        assert_eq!(names.len(), 6, "{names:?}");
        assert!(names[0].contains("B02") && names[3].contains("B08") && names[5].contains("B12"), "{names:?}");
        // (the same files picked one by one, in any order, come out in band order)
        let mut shuffled = files.clone();
        shuffled.reverse();
        assert_eq!(band_files(&shuffled).unwrap(), files);
        let roles = crate::rgb::band_roles(&names);
        assert_eq!(roles, ["blue", "green", "red", "nir", "swir1", "swir2"]);
        assert_eq!(files_scale(&files).unwrap(), Scale::Div10000);
        for marked in [true, false] {
            let c = Choice { file: files[0].clone(), files: files.clone(), ..Default::default() };
            let p = plan(&image_model(marked), &r, &one("image", c), None, &d).unwrap();
            assert!(p.refused.is_empty(), "{:?}", p.refused);
            let md = &p.model;
            assert_eq!(p.inputs.len(), 6);
            let stack = md.node("image_stack").expect("the stack");
            assert_eq!(stack.op, "stack.of");
            assert_eq!(stack.inputs.len(), 6);
            // the 20 m bands put on the 10 m grid
            let matched: Vec<&Node> = md.nodes.iter().filter(|n| n.op == "raster.match").collect();
            assert_eq!(matched.len(), 2, "{:?}", md.nodes.iter().map(|n| &n.id).collect::<Vec<_>>());
            assert!(matched.iter().all(|n| matches!(n.inputs.get("like"), Some(Wire::Ref(r)) if r == "$image_b02")));
            let n = md.node("image_bands").expect("the rename");
            assert_eq!(n.op, "stack.rename");
            assert_eq!(n.params["names"], serde_json::json!(["blue", "green", "red", "nir", "swir1", "swir2"]));
            assert_eq!(n.params["scale"], serde_json::json!(1e-4));
            assert_eq!(n.data.is_some(), marked);
            for c in ["segments", "features"] {
                let read = md.node(c).unwrap().inputs.get("image");
                assert!(matches!(read, Some(Wire::Ref(r)) if r == "image_bands.stack"), "{c}: {read:?}");
            }
            assert!(md.node("land").is_none() && md.node("image").is_none());
            assert_eq!(errors(&p, &d), Vec::<String>::new());
            let out = run(&p, &d);
            let Some(terraflux_ops::Value::Table(t)) = out.get("features.table") else { panic!("no features") };
            for col in ["mean_blue", "mean_nir", "mean_swir2"] {
                assert!(t.columns.iter().any(|c| c == col), "{col}: {:?}", t.columns);
            }
            // (B08 is the fourth file: 4000 to 4400 over 10 000)
            let k = t.columns.iter().position(|c| c == "mean_nir").unwrap();
            assert!(t.rows.iter().all(|row| (0.4..0.45).contains(&row[k])), "{:?}", t.rows[0]);
        }
        // a file of several bands is no band file
        let multi = write_bands(&d, "multi.tif", &S2, |b, _, _| b as f64);
        let c = Choice { file: multi.clone(), files: vec![multi], ..Default::default() };
        let err = plan(&image_model(true), &r, &one("image", c), None, &d).unwrap_err();
        assert!(err.contains("single-band"), "{err}");
        // and bands are for an image
        let c = Choice { file: files[0].clone(), files, ..Default::default() };
        let cls = template("Dinamica EGO");
        let node = sources(&cls, &r).first().unwrap().node.clone();
        assert!(plan(&cls, &r, &one(&node, c), None, &d).is_err());
    }

    #[test]
    fn a_folder_with_no_band_files_says_so() {
        let d = dir("no-band-folder");
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let _ = std::fs::remove_file(e.path());
        }
        write(&d, "elevation.tif", (4, 4), 10.0, |_, _| 1.0);
        assert!(band_files(&[d]).unwrap_err().contains("band number"));
    }

    #[test]
    fn a_scene_of_an_area_is_a_data_stac_node_and_the_roles_and_the_scale_follow() {
        let r = Registry::standard();
        let d = dir("scene");
        let scene =
            Scene { extent: [-47.9, -15.9, -47.88, -15.88], cellsize: 0.00018, dates: "2025-06-01/2025-08-31".into() };
        for marked in [true, false] {
            let c = Choice { scene: Some(scene.clone()), ..Default::default() };
            let p = plan(&image_model(marked), &r, &one("image", c), None, &d).unwrap();
            assert!(p.refused.is_empty(), "{:?}", p.refused);
            assert!(p.inputs.is_empty(), "nothing is read from a file");
            let md = &p.model;
            let n = md.node("image_scene").expect("the data.stac node");
            assert_eq!(n.op, "data.stac");
            assert_eq!(n.params["url"], "https://earth-search.aws.element84.com/v1");
            assert_eq!(n.params["collection"], "sentinel-2-l2a");
            assert_eq!(n.params["assets"], serde_json::json!(["blue", "green", "red", "nir", "swir16", "swir22"]));
            assert_eq!(n.params["datetime"], "2025-06-01/2025-08-31");
            assert_eq!(n.params["mask"], "auto");
            assert_eq!(n.params["extent"], serde_json::json!([-47.9, -15.9, -47.88, -15.88]));
            assert_eq!(n.params["cellsize"], serde_json::json!(0.00018));
            assert_eq!(n.params["query"], r#"{"eo:cloud_cover":{"lt":20}}"#);
            let b = md.node("image_bands").expect("the rename");
            assert_eq!(b.op, "stack.rename");
            assert_eq!(b.params["names"], serde_json::json!(["blue", "green", "red", "nir", "swir1", "swir2"]));
            assert_eq!(b.params["scale"], serde_json::json!(1e-4));
            // dates after 25 January 2022: the offset of the baseline 04.00 taken off
            assert_eq!(b.params["offset"], serde_json::json!(-0.1));
            assert_eq!(b.data.is_some(), marked);
            assert!(matches!(b.inputs.get("stack"), Some(Wire::Ref(r)) if r == "image_scene.raster"));
            for c in ["segments", "features"] {
                let read = md.node(c).unwrap().inputs.get("image");
                assert!(matches!(read, Some(Wire::Ref(r)) if r == "image_bands.stack"), "{c}: {read:?}");
            }
            assert!(md.node("image").is_none() && md.node("land").is_none());
            // (checked, not run: the run reads the catalogue)
            assert_eq!(errors(&p, &d), Vec::<String>::new());
        }
        // an area that cannot be asked for
        let bad = Scene { extent: [1.0, 1.0, 0.0, 2.0], ..scene.clone() };
        assert!(bad.problem().unwrap().contains("area"));
        let bad = Scene { dates: "last summer".into(), ..scene.clone() };
        assert!(bad.problem().unwrap().contains("dates"));
        assert!(Scene::new([0.0, 0.0, 0.01, 0.01]).problem().is_none());
        let c = Choice { scene: Some(bad), ..Default::default() };
        assert!(plan(&image_model(true), &r, &one("image", c), None, &d).is_err());
    }

    #[test]
    fn the_roles_and_the_scale_can_be_chosen() {
        let r = Registry::standard();
        let d = dir("image-roles");
        let f = write_bands(&d, "s2.tif", &S2, |b, _, _| 0.1 * (b + 1) as f64);
        // reflectance already: no scale, and the names are the only change
        assert_eq!(image_info(&f).unwrap().scale, Scale::None);
        let p = plan(&image_model(true), &r, &one("image", choice(&f)), None, &d).unwrap();
        let n = p.model.node("image_bands").unwrap();
        assert!(!n.params.contains_key("scale"));
        // the user's roles: the B01 is the blue, the B02 dropped; and the Landsat's scale
        let mut c = choice(&f);
        c.roles = Some(["blue", "other", "green", "red", "nir", "swir1", "other"].map(String::from).to_vec());
        c.scale = Some(Scale::LandsatC2);
        let p = plan(&image_model(true), &r, &one("image", c.clone()), None, &d).unwrap();
        let n = p.model.node("image_bands").unwrap();
        assert_eq!(n.params["names"], serde_json::json!(["blue", "", "green", "red", "nir", "swir1", ""]));
        assert_eq!((n.params["scale"].as_f64(), n.params["offset"].as_f64()), (Some(0.000_027_5), Some(-0.2)));
        // a role twice, a role that is none, a wrong number of roles, a band chosen, no role at all
        let bad = |roles: &[&str]| {
            let mut c = choice(&f);
            c.roles = Some(roles.iter().map(|s| s.to_string()).collect());
            plan(&image_model(true), &r, &one("image", c), None, &d).unwrap_err()
        };
        assert!(bad(&["blue", "blue", "red", "nir", "other", "other", "other"]).contains("two bands are blue"));
        assert!(bad(&["blue", "xx", "red", "nir", "other", "other", "other"]).contains("no role"));
        assert!(bad(&["blue"]).contains("1 roles for the 7 bands"));
        assert!(bad(&["other"; 7]).contains("has a role"));
        let mut c = choice(&f);
        c.band = Some("B02".into());
        assert!(plan(&image_model(true), &r, &one("image", c), None, &d).unwrap_err().contains("all its bands"));
    }

    #[test]
    fn bands_with_no_names_have_no_roles_and_the_scale_is_read_from_the_file() {
        let r = Registry::standard();
        let d = dir("image-unnamed");
        let f = write_bands(&d, "plain.tif", &[None, None, None], |b, _, _| b as f64);
        assert_eq!(image_info(&f).unwrap().roles, ["other"; 3]);
        let e = plan(&image_model(true), &r, &one("image", choice(&f)), None, &d).unwrap_err();
        assert!(e.contains("has a role"), "{e}");
        // Landsat's collection 2: SR_B names and digital numbers; Sentinel-2's 0 to 10 000; reflectance
        let l8 = [Some("SR_B2"), Some("SR_B3"), Some("SR_B4"), Some("SR_B5"), Some("SR_B6"), Some("SR_B7")];
        let f = write_bands(&d, "l8.tif", &l8, |b, row, _| 8000.0 + 1000.0 * b as f64 + row as f64);
        let info = image_info(&f).unwrap();
        assert_eq!(info.scale, Scale::LandsatC2);
        assert_eq!(info.roles, ["blue", "green", "red", "nir", "swir1", "swir2"]);
        let f = write_bands(&d, "s2.tif", &S2, |b, _, _| 100.0 * b as f64 + 300.0);
        assert_eq!(image_info(&f).unwrap().scale, Scale::Div10000);
        let f = write_bands(&d, "r.tif", &S2, |b, _, _| 0.05 * b as f64);
        assert_eq!(image_info(&f).unwrap().scale, Scale::None);
        // the roles of a file named blue, green...: nothing to rename and no scale: no node, the data on the input
        let named = [Some("blue"), Some("green"), Some("red"), Some("nir")];
        let f = write_bands(&d, "named.tif", &named, |b, _, _| 0.1 * b as f64);
        let p = plan(&image_model(true), &r, &one("image", choice(&f)), None, &d).unwrap();
        assert!(p.model.node("image_bands").is_none());
        let Some(Input::Stack { data: Some(data), .. }) = p.model.inputs.get("image") else { panic!("no data") };
        assert_eq!(data.kind, DataKind::Image);
        assert_eq!(errors(&p, &d), Vec::<String>::new());
    }

    fn rules_model() -> Model {
        let mut m = Model::new("rules");
        let classes = serde_json::json!([
            { "name": "water", "rule": "above(ndwi, 0, 0.2)", "colour": "#3366cc" },
            { "name": "wet", "rule": "below(ndvi, 0.2, 0.4)" },
            { "name": "green", "rule": "above(ndvi, 0.4, 0.6) and not border_to(\"water\") > 0.5", "parent": "wet" },
            { "name": "rest" },
        ]);
        m.nodes.push(Node::new("rules", "objects.rules").param("classes", classes));
        m
    }

    #[test]
    fn the_users_classes_replace_the_names_and_colours_of_the_rules() {
        let mut m = rules_model();
        let start = rule_classes(&m);
        assert_eq!(start.len(), 1);
        let names: Vec<&str> = start[0].1.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["water", "wet", "green", "rest"]);
        // the classes as they are: nothing done
        assert_eq!(apply_classes(&mut m, "rules", &start[0].1).unwrap(), None);
        let mut user = start[0].1.clone();
        user[0].name = "lake".into();
        user[0].colour = Some("#0000ff".into());
        user.remove(3);
        user.push(UserClass { from: None, name: "city".into(), colour: Some("#888888".into()) });
        let said = apply_classes(&mut m, "rules", &user).unwrap().unwrap();
        assert!(said.contains("1 added") && said.contains("1 taken away"), "{said}");
        let list = m.node("rules").unwrap().params["classes"].as_array().unwrap().clone();
        let name = |i: usize| list[i]["name"].as_str().unwrap().to_string();
        assert_eq!((name(0), name(3)), ("lake".to_string(), "city".to_string()));
        assert_eq!(list[0]["colour"], "#0000ff");
        // the rule of "green" reads "lake" now; "city" has the rule 0; "green" is still wet's daughter
        assert!(list[2]["rule"].as_str().unwrap().contains("border_to(\"lake\")"), "{}", list[2]);
        assert_eq!((list[3]["rule"].as_str(), list[2]["parent"].as_str()), (Some("0"), Some("wet")));
        // a name taken away: its daughters are roots; a swap of two names does not undo itself
        let mut user = rule_classes(&rules_model())[0].1.clone();
        user.remove(1);
        let mut m = rules_model();
        apply_classes(&mut m, "rules", &user).unwrap();
        assert!(m.node("rules").unwrap().params["classes"][1].get("parent").is_none());
        let mut user = rule_classes(&rules_model())[0].1.clone();
        (user[0].name, user[2].name) = ("green".into(), "water".into());
        let mut m = rules_model();
        apply_classes(&mut m, "rules", &user).unwrap();
        let rule = m.node("rules").unwrap().params["classes"][2]["rule"].as_str().unwrap().to_string();
        assert!(rule.contains("border_to(\"green\")"), "{rule}");
        // no name, a name twice, no class
        let mut user = rule_classes(&rules_model())[0].1.clone();
        user[1].name = " ".into();
        assert!(apply_classes(&mut rules_model(), "rules", &user).is_err());
        user[1].name = "water".into();
        assert!(apply_classes(&mut rules_model(), "rules", &user).is_err());
        assert!(apply_classes(&mut rules_model(), "rules", &[]).is_err());
    }
}
