//! Duplicates, one location per interval and Movebank's outliers (move2's mt_filter_unique, mt_filter_per_interval
//! and mt_filter_movebank_visible).

use std::collections::HashSet;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::{Key, Times, check_order, codes, column_params, columns, fail, give, time, tracks_of};
use crate::{Args, Ctx, Operator, Outputs, Registry, Result};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Unique));
    r.add(Arc::new(PerInterval));
    r.add(Arc::new(Visible));
}

const OUTLIERS: [&str; 6] = [
    "manually-marked-outlier",
    "algorithm-marked-outlier",
    "import-marked-outlier",
    "manually_marked_outlier",
    "algorithm_marked_outlier",
    "import_marked_outlier",
];
const VALID: [&str; 2] = ["manually-marked-valid", "manually_marked_valid"];

/// The rows of each group of (track, keys...), the groups in dplyr's order (their keys sorted, missing last).
fn group_order(v: &Vector, track: &str, keys: &[Vec<Key>]) -> Vec<Vec<usize>> {
    let tk = v.column(track).unwrap();
    let n = v.len();
    let row_key =
        |i: usize| -> Vec<Key> { std::iter::once(Key::of(tk, i)).chain(keys.iter().map(|k| k[i].clone())).collect() };
    let rk: Vec<Vec<Key>> = (0..n).map(row_key).collect();
    let mut order: Vec<usize> = (0..n).collect();
    let cmp = |a: &Vec<Key>, b: &Vec<Key>| {
        a.iter().zip(b).map(|(x, y)| x.cmp(y)).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal)
    };
    order.sort_by(|&i, &j| cmp(&rk[i], &rk[j]).then(i.cmp(&j)));
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &i in &order {
        match groups.last_mut() {
            Some(g) if cmp(&rk[g[0]], &rk[i]).is_eq() => g.push(i),
            _ => groups.push(vec![i]),
        }
    }
    groups
}

/// sample.int(n, 1) for each group in turn after set.seed(seed): 0-based picks.
fn draw(sizes: &[usize], seed: i32) -> Result<Vec<usize>> {
    let mut rng = terraflux_ls_sys::RRng::new(seed);
    sizes
        .iter()
        .map(|&n| Ok(terraflux_ls_sys::ffi::rrng_sample_int(rng.pin_mut(), n as i64, 1, false, &[])?[0] as usize - 1))
        .collect()
}

/// A value of a record (a column's, or its location).
#[derive(Clone, PartialEq)]
enum Cell {
    Num(f64),
    Text(String),
    Geom(Geometry),
    Na,
}

/// identical(a, b) | is.na(b) for one value of two records (all.equal's tolerance with `near`).
fn equal(a: &Cell, b: &Cell, near: bool) -> bool {
    match (a, b) {
        (_, Cell::Na) => true,
        (Cell::Na, _) => false,
        (Cell::Geom(x), Cell::Geom(y)) => geom_equal(x, y, if near { 1.5e-8 } else { 0.0 }),
        (Cell::Num(x), Cell::Num(y)) => {
            if x == y {
                return true;
            }
            if !near {
                return false;
            }
            let den = x.abs();
            let diff = (x - y).abs();
            let r = diff / den;
            (if den > 0.0 && r.is_finite() { r } else { diff }) <= 1.5e-8
        }
        (Cell::Text(x), Cell::Text(y)) => x == y,
        _ => false,
    }
}

/// shapely's equals_exact of points (empty equals empty).
fn geom_equal(a: &Geometry, b: &Geometry, tol: f64) -> bool {
    match (a, b) {
        (Geometry::Point(p), Geometry::Point(q)) => {
            if tol == 0.0 {
                p == q
            } else {
                ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt() <= tol
            }
        }
        (Geometry::Empty, Geometry::Empty) => true,
        _ => a == b,
    }
}

fn cell_key(c: &Cell) -> String {
    match c {
        Cell::Na => "\0NA".into(),
        Cell::Num(v) => format!("n{}", v.to_bits()),
        Cell::Text(s) => format!("t{s}"),
        Cell::Geom(g) => format!("g{:?}", g.to_wkb()),
    }
}

/// move2's slice_subsets: the rows of a group left once records that are subsets of others are dropped.
fn subsets(rows: &[usize], records: &[Vec<Cell>], near: bool) -> Vec<usize> {
    if rows.len() == 1 {
        return vec![rows[0]];
    }
    let mut seen = HashSet::new();
    let mut kept: Vec<(usize, &Vec<Cell>)> = Vec::new();
    for (&r, rec) in rows.iter().zip(records) {
        let key: Vec<String> = rec.iter().map(cell_key).collect();
        if seen.insert(key) {
            kept.push((r, rec));
        }
    }
    if kept.len() == 1 {
        return vec![kept[0].0];
    }
    let (mut drop, mut pairs) = (HashSet::new(), HashSet::new());
    // expand.grid's order: the second record outer, the first inner
    for (j, &(rj, b)) in kept.iter().enumerate() {
        for (i, &(ri, a)) in kept.iter().enumerate() {
            if i == j {
                continue;
            }
            if a.iter().zip(b).all(|(x, y)| equal(x, y, near)) {
                let pair = (ri.min(rj), ri.max(rj));
                if !pairs.insert(pair) {
                    continue;
                }
                drop.insert(rj);
            }
        }
    }
    kept.iter().map(|k| k.0).filter(|r| !drop.contains(r)).collect()
}

fn keys_of(c: &Column) -> Vec<Key> {
    (0..c.len()).map(|i| Key::of(c, i)).collect()
}

/// A time column's keys (datetimes by their instant).
fn time_keys(times: &Times) -> Vec<Key> {
    match times {
        Times::Numbers(x) => x.iter().map(|&v| if v.is_nan() { Key::Na } else { Key::Num(v) }).collect(),
        Times::Instants(x) => x.iter().map(|t| t.map_or(Key::Na, |t| Key::Num(t as f64))).collect(),
    }
}

fn kept(v: &Vector, keep: &[bool]) -> Vector {
    let rows: Vec<usize> = (0..v.len()).filter(|&i| keep[i]).collect();
    v.select(&rows)
}

/// `move.filter_unique`.
struct Unique;

impl Operator for Unique {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.filter_unique",
                "The tracks without duplicated times within a track (move2's mt_filter_unique and mt_unique). \
                 criterion subsets (default): of the records of a track with the same time, those that only repeat \
                 another or hold less (each value equal to the other's or missing) are dropped, with a warning when \
                 some remain; subsets_equal the same with numbers equal within R's all.equal (1.5e-8); first, last: \
                 the first or last record of each time; sample: one at random (R's sample.int after set.seed(seed), \
                 move2's picks).",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
            .output(PortSpec::new("tracks", Type::Vector, "the records kept"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .param(
                PortSpec::optional("criterion", Type::Text, "subsets (default), subsets_equal, first, last or sample")
                    .choices(["subsets", "subsets_equal", "first", "last", "sample"]),
            )
            .param(PortSpec::optional(
                "columns_also",
                Type::Any,
                "more columns whose values must also match for records to be duplicates (a list)",
            ))
            .param(PortSpec::optional("seed", Type::Integer, "sample: the seed of R's generator (1)")),
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let (t, k) = columns(args, v)?;
        let times = Times::of(v.column(&t).unwrap()).map_err(|e| fail(node, e))?;
        let also: Vec<String> = args.param("columns_also", Vec::new())?;
        let mut keys = vec![time_keys(&times)];
        for c in &also {
            keys.push(keys_of(v.column(c).ok_or_else(|| fail(node, format!("no column {c:?}")))?));
        }
        let groups = group_order(v, &k, &keys);
        let mut keep = vec![false; v.len()];
        let criterion: String = args.param("criterion", "subsets".to_string())?;
        match criterion.as_str() {
            "first" | "last" => {
                for g in &groups {
                    keep[if criterion == "first" { g[0] } else { *g.last().unwrap() }] = true;
                }
            }
            "sample" => {
                let picks = draw(&groups.iter().map(Vec::len).collect::<Vec<_>>(), args.param("seed", 1)?)?;
                for (g, p) in groups.iter().zip(picks) {
                    keep[g[p]] = true;
                }
            }
            "subsets" | "subsets_equal" => {
                // every column but the track's, and the location
                let others: Vec<&Column> = v.attributes.iter().filter(|(n, _)| *n != k).map(|(_, c)| c).collect();
                let record = |i: usize| -> Vec<Cell> {
                    let mut r: Vec<Cell> = others
                        .iter()
                        .map(|c| match Key::of(c, i) {
                            Key::Na => Cell::Na,
                            Key::Num(x) => Cell::Num(x),
                            Key::Text(s) => Cell::Text(s),
                        })
                        .collect();
                    r.push(Cell::Geom(v.geometries[i].clone()));
                    r
                };
                let mut left = false;
                for g in &groups {
                    if g.len() == 1 {
                        keep[g[0]] = true;
                        continue;
                    }
                    let records: Vec<Vec<Cell>> = g.iter().map(|&i| record(i)).collect();
                    let rows = subsets(g, &records, criterion == "subsets_equal");
                    left |= rows.len() != 1;
                    for r in rows {
                        keep[r] = true;
                    }
                }
                if left {
                    ctx.warn(format!(
                        "node {node:?}: after removing all records that are subsets of other records there are \
                         still remaining duplicates"
                    ));
                }
            }
            c => return Err(fail(node, format!("criterion {c:?}: subsets, subsets_equal, first, last or sample"))),
        }
        Ok(give("tracks", kept(v, &keep)))
    }
}

/// `move.filter_per_interval`.
struct PerInterval;

impl Operator for PerInterval {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.filter_per_interval",
                "One location per time interval of each track (move2's mt_filter_per_interval): the intervals of \
                 lubridate's floor_date, multiples counted from the start of the next larger unit (minutes within \
                 the hour, hours within the day, days within the month), weeks from Sunday; the first, the last or \
                 one at random (R's sample.int after set.seed(seed), move2's picks). The times must be datetimes.",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
            .output(PortSpec::new("tracks", Type::Vector, "a location per interval"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .param(PortSpec::optional(
                "unit",
                Type::Text,
                "the interval: hour (default), 15 mins, 2 hours, day, week, month, year...",
            ))
            .param(
                PortSpec::optional("criterion", Type::Text, "first (default), last or random")
                    .choices(["first", "last", "random"]),
            )
            .param(PortSpec::optional("seed", Type::Integer, "random: the seed of R's generator (1)")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let (t, k) = columns(args, v)?;
        let times = Times::of(v.column(&t).unwrap()).map_err(|e| fail(node, e))?;
        check_order(&codes(v.column(&k).unwrap()), Some(&times.seconds()), false).map_err(|e| fail(node, e))?;
        let Times::Instants(instants) = &times else {
            return Err(fail(node, "one location per interval needs datetimes in the time column"));
        };
        let unit = time::unit(&args.param("unit", "hour".to_string())?).map_err(|e| fail(node, e))?;
        let floored: Vec<Key> =
            instants.iter().map(|t| t.map_or(Key::Na, |t| Key::Num(time::floor(t, unit) as f64))).collect();
        let groups = group_order(v, &k, &[floored]);
        let mut keep = vec![false; v.len()];
        match args.param("criterion", "first".to_string())?.as_str() {
            "random" => {
                let picks = draw(&groups.iter().map(Vec::len).collect::<Vec<_>>(), args.param("seed", 1)?)?;
                for (g, p) in groups.iter().zip(picks) {
                    keep[g[p]] = true;
                }
            }
            "first" => groups.iter().for_each(|g| keep[g[0]] = true),
            "last" => groups.iter().for_each(|g| keep[*g.last().unwrap()] = true),
            c => return Err(fail(node, format!("criterion {c:?}: first, last or random"))),
        }
        Ok(give("tracks", kept(v, &keep)))
    }
}

/// Movebank's flags as yes or no (numbers 1 / 0, or texts true / false); None missing.
fn flags(c: &Column, name: &str) -> std::result::Result<Vec<Option<bool>>, String> {
    match c {
        Column::Numbers(x) => x
            .iter()
            .map(|&v| match v {
                v if v.is_nan() => Ok(None),
                1.0 => Ok(Some(true)),
                0.0 => Ok(Some(false)),
                _ => Err(format!("the values of {name:?} must be yes or no (1 / 0, true / false)")),
            })
            .collect(),
        Column::Texts(x) => x
            .iter()
            .map(|s| match s.trim().to_lowercase().as_str() {
                "" => Ok(None),
                "true" | "1" => Ok(Some(true)),
                "false" | "0" => Ok(Some(false)),
                _ => Err(format!("the values of {name:?} must be yes or no (1 / 0, true / false)")),
            })
            .collect(),
    }
}

/// `move.filter_movebank_visible`.
struct Visible;

impl Operator for Visible {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.filter_movebank_visible",
            "The tracks without the locations Movebank hides (move2's mt_filter_movebank_visible): those of the \
             column visible when there is one; otherwise the records not marked as outliers (manually, by an \
             algorithm or on import), and those marked valid by hand.",
            Access::None,
        )
        .input(PortSpec::new("tracks", Type::Vector, "the tracks (move.read_tracks)"))
        .output(PortSpec::new("tracks", Type::Vector, "the locations Movebank shows"))
        .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let keep: Vec<bool> = if let Some(c) = v.column("visible") {
            flags(c, "visible")
                .map_err(|e| fail(node, e))?
                .into_iter()
                .map(|f| f.ok_or_else(|| fail(node, "the column visible has missing values")))
                .collect::<Result<_>>()?
        } else {
            let mut keep = vec![true; v.len()];
            for name in OUTLIERS {
                if let Some(c) = v.column(name) {
                    for (k, f) in keep.iter_mut().zip(flags(c, name).map_err(|e| fail(node, e))?) {
                        *k &= f != Some(true);
                    }
                }
            }
            for name in VALID {
                if let Some(c) = v.column(name) {
                    for (k, f) in keep.iter_mut().zip(flags(c, name).map_err(|e| fail(node, e))?) {
                        if f == Some(true) {
                            *k = true;
                        }
                    }
                }
            }
            keep
        };
        Ok(give("tracks", kept(v, &keep)))
    }
}
