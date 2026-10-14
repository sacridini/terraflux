//! Biodiverse's other imports (landschaft's BaseData.from_raster, from_vector, from_hex, from_h3 and
//! from_occurrences): rasters with a label per band (or values as labels), vector features laid over a fishnet
//! of cells (the cells they touch, or the area or length of their parts; GEOS through GDAL), records in hexagons
//! or H3 cells, and the occurrence records of a GBIF download or a Darwin Core Archive, cleaned.

use std::collections::{HashMap, HashSet};
use std::io::Read;

use serde_json::json;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

use super::basedata::{Axis, BaseData};
use super::names::{csv_field, dequote_element, perl_number};
use crate::{Error, Result};

fn err(m: impl std::fmt::Display) -> Error {
    Error::msg(m.to_string())
}

// ------------------------------------------------------------------------------------------- rasters

/// The settings of a raster import.
pub struct GridImport {
    /// Each layer a label (else the values are labels, each cell counting 1).
    pub labels_as_bands: bool,
    /// With labels as bands: values >= it count 1, others 0.
    pub threshold: Option<f64>,
    /// The group cells (default the raster's resolution).
    pub cell_sizes: Option<[f64; 2]>,
    pub cell_origins: [f64; 2],
    /// One label for every layer.
    pub given_label: Option<String>,
}

/// A basedata of the layers of a grid (landschaft's from_raster): every cell with a value snapped to the group
/// cell holding its centre.
pub fn from_grid(grid: &Grid, layers: &[(String, &[f64])], o: &GridImport, crs: Option<String>) -> Result<BaseData> {
    let t = grid.transform;
    let [cs_e, cs_n] = o.cell_sizes.unwrap_or([t[1].abs(), t[5].abs()]);
    let [ox, oy] = o.cell_origins;
    let (mut gx, mut gy, mut labels, mut counts) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut values: Vec<f64> = Vec::new();
    for (name, z) in layers {
        let label = dequote_element(o.given_label.as_deref().unwrap_or(name));
        for r in 0..grid.height {
            let row = r as f64 + 0.5;
            for c in 0..grid.width {
                let v = z[r * grid.width + c];
                if !v.is_finite() {
                    continue;
                }
                let col = c as f64 + 0.5;
                let xg = t[0] + col * t[1] + row * t[2];
                let yg = t[3] + col * t[4] + row * t[5];
                let xs = ((xg - ox) / cs_e).floor() * cs_e + ox + 0.5 * cs_e;
                let ys = ((yg - oy) / cs_n).floor() * cs_n + oy + 0.5 * cs_n;
                if o.labels_as_bands {
                    let v = match o.threshold {
                        Some(th) => {
                            if v >= th {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        None => v,
                    };
                    if v != 0.0 {
                        gx.push(xs);
                        gy.push(ys);
                        labels.push(Some(label.clone()));
                        counts.push(v);
                    }
                } else {
                    gx.push(xs);
                    gy.push(ys);
                    values.push(v);
                    counts.push(1.0);
                }
            }
        }
    }
    if !o.labels_as_bands {
        labels = values.iter().map(|&v| Some(perl_number(v + 0.0))).collect();
    }
    let mut bd = BaseData::from_records(
        vec![Axis::Numbers(gx), Axis::Numbers(gy)],
        labels,
        counts,
        vec![0.0, 0.0],
        Some(vec![ox, oy]),
        false,
        false,
        false,
        crs,
    )?;
    bd.cell_sizes = vec![cs_e, cs_n];
    Ok(bd)
}

// ------------------------------------------------------------------------------------------ features

/// The settings of a feature import.
pub struct FeatureImport<'a> {
    pub label_columns: &'a [String],
    pub count_columns: &'a [String],
    pub cell_sizes: [f64; 2],
    pub cell_origins: [f64; 2],
    /// None (the cells touched), "area" or "length".
    pub measure: Option<&'a str>,
    pub binarise: bool,
}

fn attribute<'a>(v: &'a Vector, name: &str) -> Result<&'a Column> {
    v.attributes
        .iter()
        .find(|(n, _)| n == name)
        .or_else(|| v.attributes.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)))
        .map(|(_, c)| c)
        .ok_or_else(|| err(format!("no attribute {name:?}")))
}

/// A value as the text Biodiverse reads ("" missing).
fn cell_text(c: &Column, i: usize) -> String {
    match c {
        Column::Numbers(x) if x[i].is_nan() => String::new(),
        Column::Numbers(x) => perl_number(x[i]),
        Column::Texts(t) => t[i].clone(),
    }
}

/// The label of every record: the label columns' texts joined with ':' (dequoted when one).
fn labels_of(cols: &[&Column], n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let joined = cols.iter().map(|c| csv_field(&cell_text(c, i))).collect::<Vec<_>>().join(":");
            if cols.len() <= 1 { dequote_element(&joined) } else { joined }
        })
        .collect()
}

/// GEOS's Area::ofRing.
fn ring_area(r: &[Xy]) -> f64 {
    if r.len() < 3 {
        return 0.0;
    }
    let x0 = r[0][0];
    let mut sum = 0.0;
    for i in 1..r.len() - 1 {
        sum += (r[i][0] - x0) * (r[i - 1][1] - r[i + 1][1]);
    }
    (sum / 2.0).abs()
}

/// GEOS's area of a polygonal geometry.
fn area(g: &Geometry) -> f64 {
    let poly = |rings: &Vec<Vec<Xy>>| {
        let mut a = rings.first().map_or(0.0, |s| ring_area(s));
        for h in rings.iter().skip(1) {
            a -= ring_area(h);
        }
        a
    };
    match g {
        Geometry::Polygon(r) => poly(r),
        Geometry::MultiPolygon(p) => p.iter().map(poly).sum(),
        _ => 0.0,
    }
}

/// GEOS's length (of lines; the perimeter of polygons).
fn length(g: &Geometry) -> f64 {
    let line = |l: &Vec<Xy>| {
        let mut s = 0.0;
        for w in l.windows(2) {
            let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
            s += (dx * dx + dy * dy).sqrt();
        }
        s
    };
    match g {
        Geometry::LineString(l) => line(l),
        Geometry::MultiLineString(v) => v.iter().map(line).sum(),
        Geometry::Polygon(r) => r.iter().map(line).sum(),
        Geometry::MultiPolygon(p) => p.iter().flatten().map(line).sum(),
        _ => 0.0,
    }
}

fn dimension(g: &Geometry) -> i32 {
    match g {
        Geometry::Empty => -1,
        Geometry::Point(_) | Geometry::MultiPoint(_) => 0,
        Geometry::LineString(_) | Geometry::MultiLineString(_) => 1,
        Geometry::Polygon(_) | Geometry::MultiPolygon(_) => 2,
    }
}

fn bounds(g: &Geometry) -> Option<[f64; 4]> {
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    g.points(&mut |p| b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
    b[0].is_finite().then_some(b)
}

fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Geometry {
    // shapely's box: counter-clockwise from (x1, y0)
    Geometry::Polygon(vec![vec![[x1, y0], [x1, y1], [x0, y1], [x0, y0], [x1, y0]]])
}

/// A basedata of vector features (landschaft's from_vector): points in the cell holding them; lines and polygons
/// over a fishnet spanning the layer, every cell they touch counting 1 (or the area or length of their part in
/// it).
pub fn from_features(v: &Vector, o: &FeatureImport) -> Result<BaseData> {
    let [sx, sy] = o.cell_sizes;
    let [ox, oy] = o.cell_origins;
    let label_cols: Vec<&Column> = o.label_columns.iter().map(|n| attribute(v, n)).collect::<Result<_>>()?;
    if label_cols.is_empty() {
        return Err(err("give the label attribute"));
    }
    let labels_all = labels_of(&label_cols, v.len());
    let counts_all: Option<Vec<f64>> = if o.count_columns.is_empty() {
        None
    } else {
        let mut sum: Option<Vec<f64>> = None;
        for name in o.count_columns {
            let x: Vec<f64> = match attribute(v, name)? {
                Column::Numbers(x) => x.iter().map(|v| if v.is_nan() { 0.0 } else { *v }).collect(),
                Column::Texts(t) => t.iter().map(|s| s.trim().parse().unwrap_or(0.0)).collect(),
            };
            sum = Some(match sum {
                None => x,
                Some(s) => s.iter().zip(&x).map(|(a, b)| a + b).collect(),
            });
        }
        sum
    };
    let mut total = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for g in &v.geometries {
        if let Some(b) = bounds(g) {
            total = [total[0].min(b[0]), total[1].min(b[1]), total[2].max(b[2]), total[3].max(b[3])];
        }
    }
    let fishnet = sx > 0.0 && sy > 0.0 && total[0].is_finite();
    let (mut gx0, mut gy1, mut cols, mut rows) = (0.0, 0.0, 0usize, 0usize);
    let (mut west, mut east, mut north, mut south) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    if fishnet {
        gx0 = (total[0] / sx).floor() * sx;
        let gy0 = (total[1] / sy).floor() * sy;
        let gx1 = (total[2] / sx).ceil() * sx;
        gy1 = (total[3] / sy).ceil() * sy;
        cols = ((gx1 - gx0) / sx).ceil() as usize;
        rows = ((gy1 - gy0) / sy).ceil() as usize;
        // cell edges as Biodiverse accumulates them (repeated additions)
        let (mut a, mut b) = (gx0, gx0 + sx);
        for _ in 0..cols {
            west.push(a);
            east.push(b);
            a += sx;
            b += sx;
        }
        let (mut n, mut s) = (gy1, gy1 - sy);
        for _ in 0..rows {
            north.push(n);
            south.push(s);
            n -= sy;
            s -= sy;
        }
    }
    let bt = 10e-14 * (sx + sy) / 2.0;
    let (mut xs, mut ys, mut labs, mut cnts) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (k, geom) in v.geometries.iter().enumerate() {
        if matches!(geom, Geometry::Empty) {
            continue;
        }
        let base = counts_all.as_ref().map_or(1.0, |c| c[k]);
        if matches!(geom, Geometry::Point(_) | Geometry::MultiPoint(_)) {
            geom.points(&mut |p| {
                xs.push(p[0]);
                ys.push(p[1]);
                labs.push(Some(labels_all[k].clone()));
                cnts.push(base);
            });
            continue;
        }
        if !fishnet {
            return Err(err("lines and polygons need positive cell sizes"));
        }
        let Some([bx0, by0, bx1, by1]) = bounds(geom) else { continue };
        let c_lo = (((bx0 - gx0) / sx).floor() as i64 - 1).max(0) as usize;
        let c_hi = ((((bx1 - gx0) / sx).ceil() as i64 + 1).max(0) as usize).min(cols);
        let r_lo = (((gy1 - by1) / sy).floor() as i64 - 1).max(0) as usize;
        let r_hi = ((((gy1 - by0) / sy).ceil() as i64 + 1).max(0) as usize).min(rows);
        let cells: Vec<(usize, usize)> = (c_lo..c_hi).flat_map(|c| (r_lo..r_hi).map(move |r| (c, r))).collect();
        let boxes: Vec<Geometry> = cells
            .iter()
            .map(|&(c, r)| match o.measure {
                None => rectangle(west[c] + bt, south[r] + bt, east[c] - bt, north[r] - bt),
                Some(_) => rectangle(west[c], south[r], east[c], north[r]),
            })
            .collect();
        let feature = terraflux_gdal::Geos::new(std::slice::from_ref(geom)).map_err(err)?;
        let hit: Vec<Geometry> = boxes
            .into_iter()
            .filter_map(|b| match feature.intersects_with(0, &b) {
                Ok(true) => Some(Ok(b)),
                Ok(false) => None,
                Err(e) => Some(Err(e)),
            })
            .collect::<std::result::Result<_, _>>()
            .map_err(err)?;
        match o.measure {
            None => {
                let geos = terraflux_gdal::Geos::new(&hit).map_err(err)?;
                for i in 0..hit.len() {
                    let c = geos.centroid(i).map_err(err)?;
                    xs.push(c[0]);
                    ys.push(c[1]);
                    labs.push(Some(labels_all[k].clone()));
                    cnts.push(base);
                }
            }
            Some(m) => {
                let keep_dim = if m == "area" { 2 } else { 1 };
                let pieces = terraflux_gdal::intersection(&hit, geom).map_err(err)?;
                let kept: Vec<Geometry> = pieces.into_iter().filter(|p| dimension(p) >= keep_dim).collect();
                let geos = terraflux_gdal::Geos::new(&kept).map_err(err)?;
                for (i, piece) in kept.iter().enumerate() {
                    let c = geos.centroid(i).map_err(err)?;
                    let size = if m == "area" { area(piece) } else { length(piece) };
                    xs.push(c[0]);
                    ys.push(c[1]);
                    labs.push(Some(labels_all[k].clone()));
                    cnts.push(if counts_all.is_none() { size } else { base });
                }
            }
        }
    }
    // the feature import snaps with a plain floor (no rounding step)
    if sx > 0.0 {
        xs.iter_mut().for_each(|x| *x = ox + ((*x - ox) / sx).floor() * sx + sx / 2.0);
    }
    if sy > 0.0 {
        ys.iter_mut().for_each(|y| *y = oy + ((*y - oy) / sy).floor() * sy + sy / 2.0);
    }
    let mut bd = BaseData::from_records(
        vec![Axis::Numbers(xs), Axis::Numbers(ys)],
        labs,
        cnts,
        vec![0.0, 0.0],
        Some(vec![ox, oy]),
        o.binarise,
        false,
        false,
        v.crs.clone(),
    )?;
    bd.cell_sizes = vec![sx, sy];
    Ok(bd)
}

// ----------------------------------------------------------------------------------- hexagons and H3

const SQRT3: f64 = 1.7320508075688772;

/// numpy's round to 10 decimals.
fn round10(x: f64) -> f64 {
    (x * 1e10).round_ties_even() / 1e10
}

/// The centres of the hexagons (circumradius `size`) holding the points (landschaft's hex_centres).
pub fn hex_centres(x: &[f64], y: &[f64], size: f64, flat: bool, origin: [f64; 2]) -> (Vec<f64>, Vec<f64>) {
    x.iter()
        .zip(y)
        .map(|(&x, &y)| {
            let (px, py) = ((x - origin[0]) / size, (y - origin[1]) / size);
            let (q, r) = if flat {
                (2.0 / 3.0 * px, -px / 3.0 + SQRT3 / 3.0 * py)
            } else {
                (SQRT3 / 3.0 * px - py / 3.0, 2.0 / 3.0 * py)
            };
            let s = -q - r;
            let (mut rq, mut rr, rs) = (q.round_ties_even(), r.round_ties_even(), s.round_ties_even());
            let (dq, dr, ds) = ((rq - q).abs(), (rr - r).abs(), (rs - s).abs());
            let fix_q = dq > dr && dq > ds;
            let fix_r = !fix_q && dr > ds;
            if fix_q {
                rq = -rr - rs;
            }
            if fix_r {
                rr = -rq - rs;
            }
            let (cx, cy) = if flat {
                (size * 1.5 * rq, size * SQRT3 * (rr + rq / 2.0))
            } else {
                (size * SQRT3 * (rq + rr / 2.0), size * 1.5 * rr)
            };
            (round10(cx + origin[0]), round10(cy + origin[1]))
        })
        .unzip()
}

/// A basedata of records grouped at given coordinates (landschaft's _grouped: from_dataframe with cells of 0).
fn grouped(gx: Vec<f64>, gy: Vec<f64>, labels: Vec<String>, counts: Option<Vec<f64>>) -> Result<BaseData> {
    let counts = counts.unwrap_or_else(|| vec![1.0; labels.len()]);
    let keep: Vec<usize> = (0..labels.len()).filter(|&i| counts[i] != 0.0).collect();
    BaseData::from_records(
        vec![
            Axis::Numbers(keep.iter().map(|&i| gx[i]).collect()),
            Axis::Numbers(keep.iter().map(|&i| gy[i]).collect()),
        ],
        keep.iter().map(|&i| Some(labels[i].clone())).collect(),
        keep.iter().map(|&i| counts[i]).collect(),
        vec![0.0, 0.0],
        None,
        false,
        false,
        false,
        None,
    )
}

/// Records' coordinates, labels and counts from columns of features or a table.
pub struct Records {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub labels: Vec<String>,
    pub counts: Option<Vec<f64>>,
}

/// A basedata of records in hexagons (landschaft's from_hex): the groups the hexagons' centres.
pub fn from_hex(r: Records, size: f64, flat: bool, origin: [f64; 2], crs: Option<String>) -> Result<BaseData> {
    if size.is_nan() || size <= 0.0 {
        return Err(err("the hexagons' size must be positive"));
    }
    let (cx, cy) = hex_centres(&r.x, &r.y, size, flat, origin);
    let mut bd = grouped(cx, cy, r.labels, r.counts)?;
    bd.crs = crs;
    bd.grid = Some(json!({"type": "hex", "size": size, "orientation": if flat { "flat" } else { "pointy" }}));
    Ok(bd)
}

/// A basedata of lon/lat records in H3 cells (landschaft's from_h3, h3o for the h3 package): the groups the
/// cells' centres, the cells' ids in the grid.
pub fn from_h3(r: Records, resolution: u8) -> Result<BaseData> {
    let res = h3o::Resolution::try_from(resolution).map_err(|_| err("the H3 resolution is 0 to 15"))?;
    let mut cells = Vec::with_capacity(r.x.len());
    for (&lon, &lat) in r.x.iter().zip(&r.y) {
        let ll = h3o::LatLng::new(lat, lon).map_err(|e| err(format!("({lon}, {lat}): {e}")))?;
        cells.push(ll.to_cell(res));
    }
    let mut centre: HashMap<h3o::CellIndex, (f64, f64)> = HashMap::new();
    for &c in &cells {
        centre.entry(c).or_insert_with(|| {
            let ll = h3o::LatLng::from(c);
            (round10(ll.lng()), round10(ll.lat()))
        });
    }
    let gx: Vec<f64> = cells.iter().map(|c| centre[c].0).collect();
    let gy: Vec<f64> = cells.iter().map(|c| centre[c].1).collect();
    let mut cell_of: HashMap<(u64, u64), String> = HashMap::new();
    for (k, c) in cells.iter().enumerate() {
        cell_of.insert((gx[k].to_bits(), gy[k].to_bits()), c.to_string());
    }
    let mut bd = grouped(gx, gy, r.labels, r.counts)?;
    bd.crs = Some("EPSG:4326".into());
    let ids: Vec<String> = (0..bd.n_groups())
        .map(|g| {
            cell_of.get(&(bd.coords[2 * g].to_bits(), bd.coords[2 * g + 1].to_bits())).cloned().unwrap_or_default()
        })
        .collect();
    bd.grid = Some(json!({"type": "h3", "resolution": resolution, "cells": ids}));
    Ok(bd)
}

// ------------------------------------------------------------------------------ GBIF and Darwin Core

/// The cleaning of occurrence records (landschaft's read_occurrences).
pub struct OccurrenceFilter<'a> {
    pub label: &'a [String],
    pub count: Option<&'a str>,
    pub max_uncertainty: Option<f64>,
    pub basis_of_record: Option<HashSet<String>>,
    pub exclude_issues: HashSet<String>,
    pub present_only: bool,
    pub drop_duplicates: bool,
}

/// The cleaned records: label, longitude, latitude and count.
#[derive(Default)]
pub struct Occurrences {
    pub label: Vec<String>,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub count: Vec<f64>,
    seen: HashSet<(String, u64, u64)>,
}

const COORDS: [&str; 2] = ["decimalLongitude", "decimalLatitude"];
const FILTER_COLUMNS: [&str; 5] =
    ["occurrenceStatus", "coordinateUncertaintyInMeters", "basisOfRecord", "issue", "hasGeospatialIssues"];

fn number_of(s: Option<&str>) -> f64 {
    s.and_then(|v| v.trim().parse().ok()).unwrap_or(f64::NAN)
}

fn zero_key(v: f64) -> u64 {
    if v == 0.0 { 0 } else { v.to_bits() }
}

/// A record's fields: the text of a column (None missing), its number, whether the table has the column.
struct Row<'a> {
    text: &'a dyn Fn(&str) -> Option<String>,
    number: &'a dyn Fn(&str) -> f64,
    has: &'a dyn Fn(&str) -> bool,
}

impl Occurrences {
    /// One record of the table, kept if it passes.
    fn push(&mut self, f: &OccurrenceFilter, row: &Row) {
        let get = row.text;
        let mut parts = Vec::with_capacity(f.label.len());
        for c in f.label {
            match get(c).map(|s| s.trim().to_string()) {
                Some(s) => parts.push(s),
                None => return,
            }
        }
        let label = parts.join(":");
        let (x, y) = ((row.number)(COORDS[0]), (row.number)(COORDS[1]));
        if label.is_empty() || !(-180.0..=180.0).contains(&x) || !(-90.0..=90.0).contains(&y) {
            return;
        }
        if f.present_only && get("occurrenceStatus").unwrap_or_else(|| "PRESENT".into()).to_uppercase() == "ABSENT" {
            return;
        }
        if let Some(m) = f.max_uncertainty {
            let u = (row.number)("coordinateUncertaintyInMeters");
            if u > m {
                return;
            }
        }
        if let Some(bor) = &f.basis_of_record
            && (row.has)("basisOfRecord")
            && !bor.contains(&get("basisOfRecord").unwrap_or_default().to_uppercase())
        {
            return;
        }
        if !f.exclude_issues.is_empty()
            && get("issue").unwrap_or_default().split(';').any(|i| f.exclude_issues.contains(i))
        {
            return;
        }
        let n = match f.count {
            Some(c) => {
                let v = (row.number)(c);
                if v.is_nan() { 1.0 } else { v }
            }
            None => 1.0,
        };
        if f.drop_duplicates && !self.seen.insert((label.clone(), zero_key(x), zero_key(y))) {
            return;
        }
        self.label.push(label);
        self.x.push(x);
        self.y.push(y);
        self.count.push(n);
    }
}

/// The records of features or a table with Darwin Core columns (e.g. sdm.download_gbif's), cleaned.
pub fn occurrences_of_columns(columns: &[(String, Column)], n: usize, f: &OccurrenceFilter) -> Result<Occurrences> {
    for c in f.label.iter().map(|s| s.as_str()).chain(COORDS) {
        if !columns.iter().any(|(n, _)| n == c) {
            return Err(err(format!("columns not found: {c:?}")));
        }
    }
    let by_name: HashMap<&str, &Column> = columns.iter().map(|(n, c)| (n.as_str(), c)).collect();
    let mut out = Occurrences::default();
    for i in 0..n {
        let text = |name: &str| -> Option<String> {
            let c = by_name.get(name)?;
            let t = cell_text(c, i);
            (!t.is_empty()).then_some(t)
        };
        let number = |name: &str| match by_name.get(name) {
            Some(Column::Numbers(x)) => x[i],
            Some(Column::Texts(t)) => number_of(Some(&t[i])),
            None => f64::NAN,
        };
        let has = |name: &str| by_name.contains_key(name);
        out.push(f, &Row { text: &text, number: &number, has: &has });
    }
    Ok(out)
}

/// A delimited occurrence table inside a file, a zip or a folder.
struct Source {
    /// The table, as GDAL opens it.
    path: String,
    sep: char,
    quote: Option<char>,
    skip: usize,
    columns: Option<Vec<String>>,
}

fn unescape(s: Option<String>, default: &str) -> String {
    let Some(s) = s else { return default.to_string() };
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(o) => {
                    out.push('\\');
                    out.push(o);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn xml_text(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// The attributes of an XML element's tag.
fn xml_attrs(tag: &str) -> HashMap<String, String> {
    let re = regex::Regex::new(r#"([A-Za-z_:][\w:.-]*)\s*=\s*("([^"]*)"|'([^']*)')"#).unwrap();
    re.captures_iter(tag)
        .map(|c| (c[1].to_string(), xml_text(c.get(3).or(c.get(4)).map_or("", |m| m.as_str()))))
        .collect()
}

/// The core table of a Darwin Core Archive's meta.xml.
fn read_meta(xml: &str, base: &str) -> Result<Source> {
    let core_re = regex::Regex::new(r"(?s)<(?:\w+:)?core\b([^>]*)>(.*?)</(?:\w+:)?core>").unwrap();
    let core = core_re.captures(xml).ok_or_else(|| err("meta.xml has no core table"))?;
    let attrs = xml_attrs(&core[1]);
    let body = &core[2];
    let loc = regex::Regex::new(r"(?s)<(?:\w+:)?location>(.*?)</(?:\w+:)?location>").unwrap();
    let location = xml_text(loc.captures(body).ok_or_else(|| err("meta.xml has no core location"))?[1].trim());
    let sep = unescape(attrs.get("fieldsTerminatedBy").cloned(), "\t");
    let quote = unescape(attrs.get("fieldsEnclosedBy").cloned(), "");
    let skip = attrs.get("ignoreHeaderLines").and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let mut cols: HashMap<usize, String> = HashMap::new();
    let field = regex::Regex::new(r"<(?:\w+:)?field\b([^>]*)/?>").unwrap();
    for c in field.captures_iter(body) {
        let a = xml_attrs(&c[1]);
        if let (Some(i), Some(t)) = (a.get("index").and_then(|v| v.parse().ok()), a.get("term")) {
            let term = t.trim_end_matches('/').rsplit(['/', '#']).next().unwrap_or(t).to_string();
            cols.insert(i, term);
        }
    }
    let id = regex::Regex::new(r"<(?:\w+:)?id\b([^>]*)/?>").unwrap();
    if let Some(c) = id.captures(body)
        && let Some(i) = xml_attrs(&c[1]).get("index").and_then(|v| v.parse().ok())
    {
        cols.entry(i).or_insert_with(|| "id".into());
    }
    let columns = (!cols.is_empty()).then(|| {
        let n = cols.keys().max().unwrap() + 1;
        (0..n).map(|i| cols.get(&i).cloned().unwrap_or_else(|| format!("_col{i}"))).collect()
    });
    Ok(Source {
        path: format!("{base}/{location}"),
        sep: sep.chars().next().unwrap_or('\t'),
        quote: quote.chars().next(),
        skip,
        columns,
    })
}

fn read_all(path: &str) -> Result<String> {
    let mut f = terraflux_gdal::VsiFile::open(path).map_err(err)?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).map_err(err)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

impl Source {
    fn new(path: &str) -> Result<Source> {
        let p = std::path::Path::new(path);
        let mut magic = [0u8; 4];
        let is_zip =
            std::fs::File::open(p).and_then(|mut f| f.read_exact(&mut magic)).is_ok() && &magic == b"PK\x03\x04";
        if is_zip {
            let base = format!("/vsizip/{}", p.to_string_lossy().replace('\\', "/"));
            let names = terraflux_gdal::read_dir(&base).map_err(err)?;
            if names.iter().any(|n| n == "meta.xml") {
                return read_meta(&read_all(&format!("{base}/meta.xml"))?, &base);
            }
            // a GBIF "simple CSV" download: one tab-separated table
            let table = names
                .iter()
                .find(|n| {
                    let l = n.to_lowercase();
                    l.ends_with(".csv") || l.ends_with(".txt") || l.ends_with(".tsv")
                })
                .ok_or_else(|| err(format!("no occurrence table in {path}")))?;
            return Ok(Source { path: format!("{base}/{table}"), sep: '\t', quote: None, skip: 0, columns: None });
        }
        if p.is_dir() && p.join("meta.xml").exists() {
            let base = p.to_string_lossy().replace('\\', "/");
            return read_meta(&read_all(&format!("{base}/meta.xml"))?, &base);
        }
        let mut head = vec![0u8; 4096];
        let n = std::fs::File::open(p).and_then(|mut f| f.read(&mut head)).map_err(|e| err(format!("{path}: {e}")))?;
        let head = String::from_utf8_lossy(&head[..n]);
        let first = head.trim_start_matches('\u{feff}').lines().next().unwrap_or("");
        Ok(Source {
            path: p.to_string_lossy().into_owned(),
            sep: if first.contains('\t') { '\t' } else { ',' },
            quote: None,
            skip: 0,
            columns: None,
        })
    }
}

/// The lines of a file through GDAL, a block at a time (the first's byte-order mark dropped).
struct Lines {
    file: terraflux_gdal::VsiFile,
    buf: Vec<u8>,
    at: usize,
    done: bool,
    first: bool,
}

impl Lines {
    fn new(path: &str) -> Result<Lines> {
        Ok(Lines {
            file: terraflux_gdal::VsiFile::open(path).map_err(err)?,
            buf: Vec::new(),
            at: 0,
            done: false,
            first: true,
        })
    }

    /// The next line without its terminator; None at the end.
    fn next_line(&mut self) -> Option<String> {
        loop {
            if let Some(k) = self.buf[self.at..].iter().position(|&b| b == b'\n') {
                let mut line = self.buf[self.at..self.at + k].to_vec();
                self.at += k + 1;
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return Some(self.text(&line));
            }
            if self.done {
                if self.at < self.buf.len() {
                    let line = self.buf[self.at..].to_vec();
                    self.at = self.buf.len();
                    let line = line.strip_suffix(b"\r").unwrap_or(&line).to_vec();
                    return Some(self.text(&line));
                }
                return None;
            }
            self.buf.drain(..self.at);
            self.at = 0;
            let mut block = vec![0u8; 1 << 20];
            let n = self.file.read(&mut block);
            if n == 0 {
                self.done = true;
            }
            self.buf.extend_from_slice(&block[..n]);
        }
    }

    fn text(&mut self, line: &[u8]) -> String {
        let s = String::from_utf8_lossy(line).into_owned();
        if std::mem::take(&mut self.first) { s.trim_start_matches('\u{feff}').to_string() } else { s }
    }
}

/// The fields of a record (pandas' C parser): split at the separator; with a quote character, quoted fields
/// (doubled quotes inside, separators and line ends too, then the next lines are read).
fn fields(lines: &mut Lines, first: String, sep: char, quote: Option<char>) -> Vec<Option<String>> {
    let Some(q) = quote else {
        return first.split(sep).map(|s| (!s.is_empty()).then(|| s.to_string())).collect();
    };
    let mut out = Vec::new();
    let mut field = String::new();
    let (mut quoted, mut in_quotes) = (false, false);
    let mut line: Vec<char> = first.chars().collect();
    let mut i = 0;
    loop {
        if i >= line.len() {
            if in_quotes && let Some(next) = lines.next_line() {
                field.push('\n');
                line = next.chars().collect();
                i = 0;
                continue;
            }
            out.push(Some(field).filter(|f| !f.is_empty()));
            return out;
        }
        let c = line[i];
        i += 1;
        if in_quotes {
            if c == q {
                if line.get(i) == Some(&q) {
                    field.push(q);
                    i += 1;
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else if c == q && field.is_empty() && !quoted {
            in_quotes = true;
            quoted = true;
        } else if c == sep {
            out.push(Some(std::mem::take(&mut field)).filter(|f| !f.is_empty()));
            quoted = false;
        } else {
            field.push(c);
        }
    }
}

/// The occurrence records of a GBIF download (the zip or its table), a Darwin Core Archive (a zip or a folder
/// with meta.xml: the core table) or a delimited file, cleaned (landschaft's read_occurrences), read a line at a
/// time.
pub fn read_occurrences(path: &str, f: &OccurrenceFilter) -> Result<(Occurrences, usize)> {
    let src = Source::new(path)?;
    let mut lines = Lines::new(&src.path)?;
    let names: Vec<String> = match &src.columns {
        Some(c) => {
            for _ in 0..src.skip {
                lines.next_line();
            }
            c.clone()
        }
        None => {
            let header = lines.next_line().unwrap_or_default();
            header.split(src.sep).map(|h| h.trim().trim_matches('"').to_string()).collect()
        }
    };
    for c in f.label.iter().map(|s| s.as_str()).chain(COORDS) {
        if !names.iter().any(|n| n == c) {
            return Err(err(format!("columns not found: {c:?}")));
        }
    }
    let wanted: HashSet<&str> =
        f.label.iter().map(|s| s.as_str()).chain(COORDS).chain(FILTER_COLUMNS).chain(f.count).collect();
    let index: HashMap<String, usize> =
        names.iter().enumerate().filter(|(_, n)| wanted.contains(n.as_str())).map(|(i, n)| (n.clone(), i)).collect();
    let mut out = Occurrences::default();
    let mut bad = 0;
    while let Some(line) = lines.next_line() {
        if line.is_empty() {
            continue;
        }
        let row = fields(&mut lines, line, src.sep, src.quote);
        if row.len() > names.len() {
            bad += 1;
            continue;
        }
        let text = |name: &str| -> Option<String> { index.get(name).and_then(|&i| row.get(i).cloned().flatten()) };
        let number = |name: &str| number_of(text(name).as_deref());
        let has = |name: &str| index.contains_key(name);
        out.push(f, &Row { text: &text, number: &number, has: &has });
    }
    Ok((out, bad))
}

/// A basedata of cleaned records in cells (landschaft's from_occurrences): longitude and latitude, reprojected
/// to `to_crs` if given, snapped as the text import does.
pub fn from_occurrences(
    mut occ: Occurrences,
    cell_sizes: Vec<f64>,
    cell_origins: Option<Vec<f64>>,
    to_crs: Option<&str>,
) -> Result<BaseData> {
    if cell_sizes.len() != 2 {
        return Err(err("two cell sizes (x and y)"));
    }
    let crs = match to_crs {
        Some(c) => {
            if !occ.x.is_empty() {
                terraflux_gdal::Transform::new("EPSG:4326", c).map_err(err)?.points(&mut occ.x, &mut occ.y);
            }
            c.to_string()
        }
        None => "EPSG:4326".to_string(),
    };
    BaseData::from_records(
        vec![Axis::Numbers(occ.x), Axis::Numbers(occ.y)],
        occ.label.into_iter().map(Some).collect(),
        occ.count,
        cell_sizes,
        cell_origins,
        false,
        false,
        false,
        Some(crs),
    )
}
