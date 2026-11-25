//! A segmenter's settings measured in cells, against the cell size of the image it reads. A scale, a superpixel's
//! side or a smoothing's sigma was chosen for an image of some cell size; when the file of the image is swapped for
//! one of finer or coarser cells, the same number gives other objects, and nothing in the model tells. Each run
//! records the cell size of the rasters it read ([`sizes`], in the journal's entry); check_model compares it with
//! the files now ([`advice`]) and warns of a segmenter whose cell settings stayed the same.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;
use terraflux_exec::journal::{Journal, Kind, Status};
use terraflux_graph::{Input, Model, Node, Ref, Source};
use terraflux_raster::Grid;

use crate::advice::{Advice, Level};

/// The operators with settings measured in cells, and those settings. One table for the warning and the tips: what
/// the settings' texts say is in cells (a window, a side, a sigma, a smallest size) or in a scale that grows with the
/// cells of an object (the multiresolution scale and ESP's start and step). Left out: those in the bands' units
/// (felzenszwalb `k`, meanshift `range_radius`), a count of objects (slic's `count`), quickshift's `max_dist` (colour
/// and position together), objects.resize `steps` (cells for most modes, the passes of refine).
pub const IN_CELLS: &[(&str, &[&str])] = &[
    ("segment.multiresolution", &["scale"]),
    ("segment.scale", &["start", "step"]),
    ("segment.slic", &["size"]),
    ("segment.snic", &["size"]),
    ("segment.felzenszwalb", &["sigma", "min_size"]),
    ("segment.meanshift", &["spatial_radius", "min_size"]),
    ("segment.quickshift", &["kernel_size", "sigma"]),
    ("segment.watershed", &["sigma"]),
    ("segment.chessboard", &["size"]),
    ("segment.quadtree", &["min_size"]),
    ("objects.split", &["min_size", "sigma"]),
];

/// The settings of an operator measured in cells.
pub fn cell_settings(op: &str) -> Option<&'static [&'static str]> {
    IN_CELLS.iter().find(|(o, _)| *o == op).map(|(_, s)| *s)
}

/// The operators that give the maps they read another cell size: what is upstream of them is not what the segmenter
/// gets.
const RESAMPLES: [&str; 3] = ["raster.resample", "raster.reproject", "raster.match"];

/// The grid of the raster in a file, from its header; none for an address on the web, a file that does not read.
fn grid_of(path: &str, dir: &Path) -> Option<Grid> {
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
    Some((*r.grid).clone())
}

/// The cell's width and height, when the grid has cells.
fn cell_of(g: &Grid) -> Option<[f64; 2]> {
    let (w, h) = g.resolution();
    (w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0).then_some([w, h])
}

/// The units of a grid's cells, for the text.
fn unit_of(g: &Grid) -> &'static str {
    match g.geo.is_geographic() {
        Some(true) => "degrees",
        Some(false) if g.geo.linear_unit() == 1.0 => "m",
        Some(false) => "ft",
        None => "units",
    }
}

/// What a source of maps reads: a top-level `raster.read` node (by its id) or an input of the model (`$name`).
fn file_of(m: &Model, key: &str) -> Option<String> {
    if let Some(name) = key.strip_prefix('$') {
        return match m.inputs.get(name)? {
            Input::Map { path, .. } | Input::Stack { path, .. } => Some(path.clone()),
            _ => None,
        };
    }
    let n = m.nodes.iter().find(|n| n.id == key && n.op == "raster.read")?;
    let p = n.params.get("path")?.as_str()?;
    match p.strip_prefix('$') {
        Some(name) => file_of(m, &format!("${name}")),
        None => Some(p.to_string()),
    }
}

/// The cell size of every raster a model reads (the top-level `raster.read` nodes by id, the model's raster inputs
/// by `$name`), from the files' headers: a file that does not read is left out. For the journal's entry.
pub fn sizes(m: &Model, dir: &Path) -> BTreeMap<String, [f64; 2]> {
    let mut keys: Vec<String> = m.inputs.keys().map(|k| format!("${k}")).collect();
    keys.extend(m.nodes.iter().filter(|n| n.op == "raster.read").map(|n| n.id.clone()));
    keys.into_iter()
        .filter_map(|k| {
            let g = grid_of(&file_of(m, &k)?, dir)?;
            Some((k, cell_of(&g)?))
        })
        .collect()
}

/// The sources of maps upstream of a node (in the top-level graph: a container's body is not followed), the nodes by
/// id and the inputs of the model as `$name`; not beyond an operator that resamples.
fn upstream(m: &Model, n: &Node) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut todo = vec![n];
    while let Some(x) = todo.pop() {
        for s in x.inputs.values() {
            let Source::Ref(r) = s else { continue };
            match Ref::parse(r) {
                Ok(Ref::Input(name)) => {
                    found.insert(format!("${name}"));
                }
                Ok(Ref::Port { node, .. }) if seen.insert(node.clone()) => {
                    let Some(up) = m.nodes.iter().find(|u| u.id == node) else { continue };
                    if up.op == "raster.read" {
                        found.insert(up.id.clone());
                    } else if !RESAMPLES.contains(&up.op.as_str()) {
                        todo.push(up);
                    }
                }
                _ => {}
            }
        }
    }
    found
}

/// What a setting of a node is in a model: its value (none: not set), or none of the outer when it is computed (a
/// node's output) and so unknown.
pub(crate) fn value(m: &Model, n: &Node, name: &str) -> Option<Option<Value>> {
    let v = match n.inputs.get(name) {
        Some(Source::Literal(v)) => Some(v),
        Some(Source::Ref(_)) => return None,
        None => n.params.get(name),
    };
    match v {
        Some(v) => match terraflux_graph::setting_ref(v) {
            // (an input of the model: its value; another reference is computed)
            Some(r) if r.starts_with('$') => Some(m.setting(r).cloned()),
            Some(_) => None,
            None => Some(Some(v.clone())),
        },
        None => Some(None),
    }
}

/// Whether two values of a setting are the same (a number as 50 or 50.0 alike).
fn same(a: &Option<Value>, b: &Option<Value>) -> bool {
    match (a, b) {
        (Some(Value::Number(x)), Some(Value::Number(y))) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}

/// The cell settings a node has set, and as they are in a node of the same operator then, when none of them changed:
/// none when one changed or is computed, an empty list when none was set (the defaults follow the image).
fn unchanged(
    now: (&Model, &Node),
    then: (&Model, &Node),
    names: &[&'static str],
) -> Option<Vec<(&'static str, Value)>> {
    let mut kept = Vec::new();
    for &name in names {
        let (a, b) = (value(now.0, now.1, name)?, value(then.0, then.1, name)?);
        if !same(&a, &b) {
            return None;
        }
        if let Some(v) = a {
            kept.push((name, v));
        }
    }
    Some(kept)
}

/// A number for the text: at most 4 decimals, without the trailing zeros.
fn num(x: f64) -> String {
    let t = format!("{x:.4}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The cells' size for the text: `3 m`, `3 x 4 m`.
pub(crate) fn cells_text(c: [f64; 2], unit: &str) -> String {
    if (c[0] - c[1]).abs() <= 1e-6 * c[0] {
        format!("{} {unit}", num(c[0]))
    } else {
        format!("{} x {} {unit}", num(c[0]), num(c[1]))
    }
}

/// A factor for the text: whole from 10 up, else one decimal when it has one.
fn times(f: f64) -> String {
    if f >= 10.0 { format!("{f:.0}") } else { num((f * 10.0).round() / 10.0) }
}

/// Whether the cell size changed by more than 1 % on an axis.
fn differs(was: [f64; 2], now: [f64; 2]) -> bool {
    (0..2).any(|i| (now[i] - was[i]).abs() > 0.01 * was[i])
}

/// What helps to choose the settings of a segmenter again, for the text.
fn helped_by(op: &str) -> &'static str {
    match op {
        "segment.multiresolution" => " (segment.scale or segment.optimize)",
        // (those segment.optimize varies a setting of by default)
        "segment.slic" | "segment.snic" | "segment.felzenszwalb" | "segment.watershed" | "segment.quickshift"
        | "segment.meanshift" => " (segment.optimize)",
        _ => "",
    }
}

/// How a source of maps reads in a message.
fn label(key: &str) -> String {
    if key.starts_with('$') { format!("the input \"{key}\"") } else { format!("the image read by \"{key}\"") }
}

/// What a source has now: its cell size and the units.
type Now = Option<([f64; 2], String)>;

/// The warnings of a model against a run of it: `then` the model as it ran, `was` the cell size of the sources that
/// run read, `current` what a source has now (its cell size and the units), `when` the run's date for the text. A
/// segmenter whose cell settings are all as they were and whose image has other cells now gets a warning, one each.
pub fn stale(
    now: &Model,
    then: &Model,
    was: &BTreeMap<String, [f64; 2]>,
    current: &dyn Fn(&str) -> Now,
    when: Option<&str>,
) -> Vec<Advice> {
    let mut out = Vec::new();
    for n in &now.nodes {
        let Some(names) = cell_settings(&n.op).filter(|_| !n.bypass) else { continue };
        let Some(t) = then.nodes.iter().find(|t| t.id == n.id && t.op == n.op) else { continue };
        let Some(kept) = unchanged((now, n), (then, t), names).filter(|k| !k.is_empty()) else { continue };
        for key in upstream(now, n) {
            let Some(&old) = was.get(&key) else { continue };
            let Some((new, unit)) = current(&key) else { continue };
            if !differs(old, new) {
                continue;
            }
            let (a, b) = (old[0] * old[1], new[0] * new[1]);
            let (f, way) = if a >= b { (a / b, "smaller") } else { (b / a, "larger") };
            let set: Vec<String> = kept.iter().map(|(k, v)| format!("{k} {v}")).collect();
            let (is, it) = if kept.len() == 1 { ("is", "it") } else { ("are", "them") };
            let at = when.map(|w| format!(" ({w})")).unwrap_or_default();
            out.push(Advice {
                level: Level::Warning,
                at: vec![n.id.clone()],
                message: format!(
                    "{}: {} has cells of {} now and had {} at the last run{at}, and {} stayed the same: it {is} \
                     measured in cells (or in a scale that grows with them), so the objects come out about {} times \
                     {way} in area. Choose {it} again{} or scale {it} with the cells.",
                    n.op,
                    label(&key),
                    cells_text(new, &unit),
                    cells_text(old, &unit),
                    set.join(", "),
                    times(f),
                    helped_by(&n.op),
                ),
                guide: "objects",
            });
            break;
        }
    }
    out
}

/// [`stale`] for a model's file: the last run of the journal that finished and recorded the cells of the rasters it
/// read, the model as it ran then, and the files' headers now. Nothing without a file, a journal, such a run or its
/// model.
pub fn advice(m: &Model, dir: &Path, file: Option<&Path>) -> Vec<Advice> {
    if !m.nodes.iter().any(|n| cell_settings(&n.op).is_some()) {
        return Vec::new();
    }
    let Some(journal) = file.and_then(Journal::of) else { return Vec::new() };
    let Some(last) = journal
        .entries()
        .into_iter()
        .rev()
        .find(|e| e.status == Status::Done && matches!(e.kind, Kind::Run | Kind::To(_)) && !e.cells.is_empty())
    else {
        return Vec::new();
    };
    let Some(then) = journal.model(&last.model) else { return Vec::new() };
    let when = crate::eta::stamp(last.when);
    let current = |key: &str| {
        let g = grid_of(&file_of(m, key)?, dir)?;
        Some((cell_of(&g)?, unit_of(&g).to_string()))
    };
    stale(m, &then, &last.cells, &current, when.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model(op: &str, set: &[(&str, Value)]) -> Model {
        let mut m = Model::new("m");
        m.nodes.push(Node::new("img", "raster.read").param("path", "img.tif"));
        let mut seg = Node::new("seg", op).input("image", "img.map");
        for (k, v) in set {
            seg = seg.param(k, v.clone());
        }
        m.nodes.push(seg);
        m
    }

    fn ran(cells: [f64; 2]) -> BTreeMap<String, [f64; 2]> {
        [("img".to_string(), cells)].into()
    }

    fn warn(now: &Model, then: &Model, was: [f64; 2], is: [f64; 2]) -> Vec<String> {
        let cur = move |k: &str| (k == "img").then(|| (is, "m".to_string()));
        stale(now, then, &ran(was), &cur, Some("2026-11-24 10:00")).iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn the_table_names_real_settings() {
        let r = terraflux_ops::Registry::standard();
        for (op, settings) in IN_CELLS {
            let spec = r.sig(op).unwrap_or_else(|| panic!("no operator {op}"));
            for s in *settings {
                assert!(spec.params.iter().any(|p| p.name == *s), "{op} has no setting {s}");
            }
        }
    }

    #[test]
    fn finer_cells_with_the_same_scale_warn() {
        let m = model("segment.multiresolution", &[("scale", json!(50))]);
        let w = warn(&m, &m, [10.0, 10.0], [3.0, 3.0]);
        assert_eq!(w.len(), 1, "{w:?}");
        for part in
            ["seg", "the image read by \"img\"", "3 m now", "10 m at the last run", "scale 50", "11 times smaller"]
        {
            assert!(w[0].contains(part), "{part}: {}", w[0]);
        }
        // (the same cells: nothing; within 1 %: nothing)
        assert!(warn(&m, &m, [3.0, 3.0], [3.0, 3.0]).is_empty());
        assert!(warn(&m, &m, [3.0, 3.0], [3.02, 3.0]).is_empty());
    }

    #[test]
    fn coarser_cells_make_larger_objects() {
        let m = model("segment.slic", &[("size", json!(10))]);
        let w = warn(&m, &m, [3.0, 3.0], [10.0, 10.0]);
        assert!(w.len() == 1 && w[0].contains("11 times larger") && w[0].contains("size 10"), "{w:?}");
        let w = warn(&m, &m, [3.0, 3.0], [4.5, 4.5]);
        assert!(w[0].contains("2.3 times larger"), "{w:?}");
    }

    #[test]
    fn a_setting_changed_since_the_run_says_nothing() {
        let then = model("segment.multiresolution", &[("scale", json!(50))]);
        let now = model("segment.multiresolution", &[("scale", json!(20))]);
        assert!(warn(&now, &then, [10.0, 10.0], [3.0, 3.0]).is_empty());
        // (set where it was not)
        let unset = model("segment.multiresolution", &[]);
        assert!(warn(&now, &unset, [10.0, 10.0], [3.0, 3.0]).is_empty());
        // (50 and 50.0 are the same)
        let float = model("segment.multiresolution", &[("scale", json!(50.0))]);
        assert_eq!(warn(&float, &then, [10.0, 10.0], [3.0, 3.0]).len(), 1);
    }

    #[test]
    fn nothing_set_follows_the_image() {
        // (the defaults of the scale come from the image: nothing was chosen at a cell size)
        let m = model("segment.multiresolution", &[]);
        assert!(warn(&m, &m, [10.0, 10.0], [3.0, 3.0]).is_empty());
    }

    #[test]
    fn only_the_cell_settings_count() {
        // (slic's count is not in cells: changed alone, the size still stayed the same)
        let then = model("segment.slic", &[("size", json!(10)), ("count", json!(500))]);
        let now = model("segment.slic", &[("size", json!(10)), ("count", json!(900))]);
        assert_eq!(warn(&now, &then, [10.0, 10.0], [3.0, 3.0]).len(), 1);
        // (an operator with no cell setting)
        let other = model("segment.threshold", &[("band", json!(1))]);
        assert!(warn(&other, &other, [10.0, 10.0], [3.0, 3.0]).is_empty());
    }

    #[test]
    fn several_settings_are_named_and_a_resample_cuts_the_trace() {
        let m = model("segment.felzenszwalb", &[("sigma", json!(1.5)), ("min_size", json!(30)), ("k", json!(1))]);
        let w = warn(&m, &m, [10.0, 10.0], [3.0, 3.0]);
        assert!(w[0].contains("sigma 1.5, min_size 30") && w[0].contains("are measured in cells"), "{w:?}");
        // the image goes through a resample to the same grid: its source's cells do not reach the segmenter
        let mut r = m.clone();
        r.nodes.insert(1, Node::new("fixed", "raster.resample").input("map", "img.map"));
        r.nodes[2].inputs.insert("image".into(), Source::Ref("fixed.map".into()));
        assert!(warn(&r, &r, [10.0, 10.0], [3.0, 3.0]).is_empty());
    }

    #[test]
    fn no_run_to_compare_with_says_nothing() {
        let m = model("segment.multiresolution", &[("scale", json!(50))]);
        let cur = |_: &str| Some(([3.0, 3.0], "m".to_string()));
        assert!(stale(&m, &m, &BTreeMap::new(), &cur, None).is_empty());
        // (no file, so no journal)
        assert!(advice(&m, Path::new("."), None).is_empty());
    }

    #[test]
    fn a_model_input_is_a_source_too() {
        let mut m = Model::new("m");
        m.inputs.insert(
            "img".into(),
            Input::Map { path: "img.tif".into(), band: None, nodata: None, note: None, data: None },
        );
        m.nodes.push(Node::new("seg", "segment.chessboard").input("image", "$img").param("size", 20));
        let was: BTreeMap<String, [f64; 2]> = [("$img".to_string(), [10.0, 10.0])].into();
        let cur = |k: &str| (k == "$img").then(|| ([5.0, 5.0], "m".to_string()));
        let w = stale(&m, &m, &was, &cur, None);
        assert!(
            w.len() == 1 && w[0].message.contains("the input \"$img\"") && w[0].message.contains("4 times smaller"),
            "{w:?}"
        );
    }
}
