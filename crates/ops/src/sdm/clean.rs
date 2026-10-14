//! Cleaning occurrence records (landschaft's clean_occurrences): the tests of CoordinateCleaner (Zizka et al.
//! 2019) — invalid coordinates, equal longitude and latitude, 0/0, capitals, country and province centroids,
//! the sea, towns, outside the country the record claims, geographic outliers of its species, GBIF's
//! headquarters, biodiversity institutions, duplicates and artificial hotspots — as landschaft runs them: the
//! circles of CoordinateCleaner to the bit (landschaft's C++), GEOS's point-in-polygon rule (the boundary
//! counts), R's statistics of the outliers.

use std::collections::{HashMap, HashSet};

use rayon::prelude::*;
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, RTree};
use serde_json::{Map as JsonMap, Value as Json};
use terraflux_ls_sys::ffi;
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::points::{Occurrences, quantile7, to_crs};
use super::refdata;

/// The tests in the order CoordinateCleaner runs them.
pub const TESTS: [&str; 13] = [
    "invalid",
    "equal",
    "zeros",
    "capitals",
    "centroids",
    "seas",
    "urban",
    "countries",
    "outliers",
    "gbif",
    "institutions",
    "duplicates",
    "aohi",
];

const GBIF_HQ: (f64, f64) = (12.58, 55.67);

// ------------------------------------------------------------------------------------- point-in-polygon

/// A ring with its segments binned by y, for GEOS's ray-crossing test.
struct IndexedRing {
    pts: Vec<[f64; 2]>,
    ymin: f64,
    height: f64,
    bins: Vec<Vec<u32>>,
}

enum Location {
    Exterior,
    Boundary,
    Interior,
}

impl IndexedRing {
    fn new(pts: Vec<[f64; 2]>) -> IndexedRing {
        let nseg = pts.len().saturating_sub(1);
        let (ymin, ymax) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p[1]), b.max(p[1])));
        let nb = (nseg / 8).clamp(1, 4096);
        let height = if ymax > ymin { (ymax - ymin) / nb as f64 } else { 1.0 };
        let mut r = IndexedRing { pts, ymin, height, bins: vec![Vec::new(); nb] };
        for k in 0..nseg {
            let (a, b) = (r.pts[k][1], r.pts[k + 1][1]);
            let (lo, hi) = (r.bin(a.min(b)), r.bin(a.max(b)));
            for bin in &mut r.bins[lo..=hi] {
                bin.push(k as u32);
            }
        }
        r
    }

    fn bin(&self, y: f64) -> usize {
        let b = ((y - self.ymin) / self.height).floor();
        if b.is_nan() || b < 0.0 { 0 } else { (b as usize).min(self.bins.len() - 1) }
    }

    /// GEOS's RayCrossingCounter: where the point lies against the ring.
    fn locate(&self, p: [f64; 2]) -> Location {
        let mut crossings = 0u32;
        for &k in &self.bins[self.bin(p[1])] {
            let (p1, p2) = (self.pts[k as usize], self.pts[k as usize + 1]);
            if p1[0] < p[0] && p2[0] < p[0] {
                continue;
            }
            if p[0] == p2[0] && p[1] == p2[1] {
                return Location::Boundary;
            }
            if p1[1] == p[1] && p2[1] == p[1] {
                let (lo, hi) = if p1[0] <= p2[0] { (p1[0], p2[0]) } else { (p2[0], p1[0]) };
                if p[0] >= lo && p[0] <= hi {
                    return Location::Boundary;
                }
                continue;
            }
            if (p1[1] > p[1] && p2[1] <= p[1]) || (p2[1] > p[1] && p1[1] <= p[1]) {
                let o = robust::orient2d(
                    robust::Coord { x: p1[0], y: p1[1] },
                    robust::Coord { x: p2[0], y: p2[1] },
                    robust::Coord { x: p[0], y: p[1] },
                );
                if o == 0.0 {
                    return Location::Boundary;
                }
                let left = if p2[1] < p1[1] { o < 0.0 } else { o > 0.0 };
                if left {
                    crossings += 1;
                }
            }
        }
        if crossings % 2 == 1 { Location::Interior } else { Location::Exterior }
    }
}

/// A polygon: its shell and holes.
struct Polygon {
    shell: IndexedRing,
    holes: Vec<IndexedRing>,
}

impl Polygon {
    /// GEOS's SimplePointInAreaLocator: the point is in the polygon or on its boundary.
    fn covers(&self, p: [f64; 2]) -> bool {
        match self.shell.locate(p) {
            Location::Exterior => false,
            Location::Boundary => true,
            Location::Interior => self.holes.iter().all(|h| !matches!(h.locate(p), Location::Interior)),
        }
    }
}

/// A (multi)polygon and its bounding box.
pub struct Shape {
    parts: Vec<Polygon>,
    bbox: [f64; 4],
}

impl Shape {
    fn new(rings: Vec<Vec<Vec<[f64; 2]>>>) -> Option<Shape> {
        let mut bbox = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        let mut parts = Vec::new();
        for poly in rings {
            let mut rings = poly.into_iter();
            let Some(shell) = rings.next().filter(|r| r.len() >= 4) else { continue };
            for p in &shell {
                bbox = [bbox[0].min(p[0]), bbox[1].min(p[1]), bbox[2].max(p[0]), bbox[3].max(p[1])];
            }
            parts.push(Polygon {
                shell: IndexedRing::new(shell),
                holes: rings.filter(|r| r.len() >= 4).map(IndexedRing::new).collect(),
            });
        }
        (!parts.is_empty()).then_some(Shape { parts, bbox })
    }

    /// The shape of a geometry (polygons; others have none).
    pub fn of(g: &Geometry) -> Option<Shape> {
        match g {
            Geometry::Polygon(r) => Shape::new(vec![r.clone()]),
            Geometry::MultiPolygon(p) => Shape::new(p.clone()),
            _ => None,
        }
    }

    fn covers(&self, p: [f64; 2]) -> bool {
        p[0] >= self.bbox[0]
            && p[0] <= self.bbox[2]
            && p[1] >= self.bbox[1]
            && p[1] <= self.bbox[3]
            && self.parts.iter().any(|q| q.covers(p))
    }
}

/// Shapes indexed by their boxes.
pub struct Shapes {
    shapes: Vec<Shape>,
    tree: RTree<GeomWithData<Rectangle<[f64; 2]>, usize>>,
}

impl Shapes {
    pub fn new(shapes: Vec<Shape>) -> Shapes {
        let items = shapes
            .iter()
            .enumerate()
            .map(|(k, s)| GeomWithData::new(Rectangle::from_corners([s.bbox[0], s.bbox[1]], [s.bbox[2], s.bbox[3]]), k))
            .collect();
        Shapes { shapes, tree: RTree::bulk_load(items) }
    }

    /// The shapes covering the point.
    fn covering(&self, p: [f64; 2]) -> impl Iterator<Item = usize> + '_ {
        self.tree
            .locate_in_envelope_intersecting(&AABB::from_point(p))
            .map(|g| g.data)
            .filter(move |&k| self.shapes[k].covers(p))
    }

    /// For each point, whether some shape covers it (the boundary counts), as shapely's covered_by.
    pub fn covered(&self, lon: &[f64], lat: &[f64]) -> Vec<bool> {
        (0..lon.len())
            .into_par_iter()
            .map(|k| {
                let p = [lon[k], lat[k]];
                p[0].is_finite() && p[1].is_finite() && self.covering(p).next().is_some()
            })
            .collect()
    }
}

/// The shapes of features in lon/lat.
fn shapes_lonlat(v: &Vector) -> Result<Vec<Shape>, String> {
    let reprojected;
    let v = match &v.crs {
        Some(c) if !terraflux_gdal::same_crs(c, "EPSG:4326")? => {
            reprojected = terraflux_gdal::reproject_vector(v, "EPSG:4326")?;
            &reprojected
        }
        _ => v,
    };
    Ok(v.geometries.iter().filter_map(Shape::of).collect())
}

/// The circles of radius `d` around the points (landschaft's C++): one shape per point (None where it has no
/// ring of four vertices).
fn circles(lon: &[f64], lat: &[f64], d: f64, kind: &str) -> Result<Vec<Option<Shape>>, String> {
    let c = ffi::sdm_circles(lon, lat, d, kind).map_err(|e| e.to_string())?;
    let (mut ring, mut at) = (0usize, 0usize);
    let mut out = Vec::with_capacity(lon.len());
    for &np in &c.parts {
        let mut polys = Vec::new();
        for _ in 0..np {
            let nv = c.vertices[ring] as usize;
            polys.push(vec![(0..nv).map(|v| [c.xy[at + 2 * v], c.xy[at + 2 * v + 1]]).collect()]);
            at += 2 * nv;
            ring += 1;
        }
        out.push(Shape::new(polys));
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------- options

/// A test's options from its setting: false / null (skip it), true (its defaults), a number (its main option)
/// or an object of options.
fn options(name: &str, value: &Json, main: &str, defaults: Json) -> Result<Option<JsonMap<String, Json>>, String> {
    let Json::Object(mut opts) = defaults else { unreachable!() };
    match value {
        Json::Null | Json::Bool(false) => Ok(None),
        Json::Bool(true) => Ok(Some(opts)),
        Json::Number(_) if !main.is_empty() => {
            opts.insert(main.into(), value.clone());
            Ok(Some(opts))
        }
        Json::Object(o) => {
            let mut unknown: Vec<&String> = o.keys().filter(|k| !opts.contains_key(*k)).collect();
            if !unknown.is_empty() {
                unknown.sort();
                let mut known: Vec<&String> = opts.keys().collect();
                known.sort();
                return Err(format!("{name}: unknown options {unknown:?}; expected some of {known:?}"));
            }
            for (k, v) in o {
                opts.insert(k.clone(), v.clone());
            }
            Ok(Some(opts))
        }
        _ => Err(format!("{name}: false, true, a number or an object of options, not {value}")),
    }
}

fn num(o: &JsonMap<String, Json>, k: &str) -> Result<f64, String> {
    o.get(k).and_then(Json::as_f64).ok_or_else(|| format!("the option {k} must be a number"))
}

fn flag_opt(o: &JsonMap<String, Json>, k: &str) -> Result<bool, String> {
    o.get(k).and_then(Json::as_bool).ok_or_else(|| format!("the option {k} must be true or false"))
}

fn text_opt(o: &JsonMap<String, Json>, k: &str) -> Result<Option<String>, String> {
    match o.get(k).unwrap_or(&Json::Null) {
        Json::Null => Ok(None),
        Json::String(s) => Ok(Some(s.clone())),
        v => Err(format!("the option {k} must be a text, not {v}")),
    }
}

// ---------------------------------------------------------------------------------------------- tests

/// The reference data given in place of CoordinateCleaner's.
#[derive(Default)]
pub struct Refs<'a> {
    pub capitals: Option<&'a Occurrences>,
    pub centroids: Option<&'a Occurrences>,
    pub institutions: Option<&'a Occurrences>,
    pub seas: Option<&'a Vector>,
    pub urban: Option<&'a Vector>,
    pub countries: Option<&'a Vector>,
}

fn lonlat_of(occ: &Occurrences) -> Result<(Vec<f64>, Vec<f64>), String> {
    let (mut x, mut y, crs) = occ.coordinates(None, None)?;
    to_crs(&mut x, &mut y, crs.as_deref(), Some("EPSG:4326"))?;
    Ok((x, y))
}

fn min_max(v: impl Iterator<Item = f64>) -> (f64, f64) {
    v.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| (a.min(x), b.max(x)))
}

/// The box of terra's circles of radius d around the points (CoordinateCleaner's crop), from the points that
/// can reach its edges.
fn circle_extent(lon: &[f64], lat: &[f64], d: f64) -> Result<Option<[f64; 4]>, String> {
    if lon.is_empty() {
        return Ok(None);
    }
    let dlat = d / 110_574.0 * 1.01 + 1e-9;
    let top = (lat.iter().fold(0.0f64, |a, v| a.max(v.abs())) + dlat).min(90.0);
    let dlon = if top >= 89.0 { 360.0 } else { d / (111_319.0 * top.to_radians().cos()) * 1.01 + 1e-9 };
    let (x0, x1) = min_max(lon.iter().copied());
    let (y0, y1) = min_max(lat.iter().copied());
    let idx: Vec<usize> = (0..lon.len())
        .filter(|&k| {
            let (x, y) = (lon[k], lat[k]);
            x <= x0 + 2.0 * dlon
                || x >= x1 - 2.0 * dlon
                || y <= y0 + 2.0 * dlat
                || y >= y1 - 2.0 * dlat
                || x.abs() >= 180.0 - dlon
                || y.abs() >= 90.0 - dlat
        })
        .collect();
    let (sx, sy): (Vec<f64>, Vec<f64>) = idx.iter().map(|&k| (lon[k], lat[k])).unzip();
    let c = ffi::sdm_circles(&sx, &sy, d, "terra").map_err(|e| e.to_string())?;
    if c.xy.is_empty() {
        return Ok(None);
    }
    let (ex0, ex1) = min_max(c.xy.iter().step_by(2).copied());
    let (ey0, ey1) = min_max(c.xy.iter().skip(1).step_by(2).copied());
    Ok(Some([ex0, ex1, ey0, ey1]))
}

/// The records within `buffer` metres of a reference point (cc_cap, cc_cen, cc_gbif, cc_inst, cc_aohi).
fn near_points(
    lon: &[f64],
    lat: &[f64],
    ref_lon: &[f64],
    ref_lat: &[f64],
    buffer: f64,
    geod: bool,
    crop: bool,
) -> Result<Vec<bool>, String> {
    let mut refs: Vec<(f64, f64)> =
        ref_lon.iter().zip(ref_lat).filter(|(x, y)| x.is_finite() && y.is_finite()).map(|(&x, &y)| (x, y)).collect();
    let buffer = if buffer == 0.0 { 1e-11 } else { buffer };
    let ok: Vec<usize> = (0..lon.len()).filter(|&k| lon[k].is_finite() && lat[k].is_finite()).collect();
    if crop && !ok.is_empty() {
        let (ox, oy): (Vec<f64>, Vec<f64>) = ok.iter().map(|&k| (lon[k], lat[k])).unzip();
        if let Some(e) = circle_extent(&ox, &oy, buffer)? {
            refs.retain(|&(x, y)| x >= e[0] && x <= e[1] && y >= e[2] && y <= e[3]);
        }
    }
    if !refs.is_empty() && !ok.is_empty() {
        // only the reference points that can reach a record (a speed-up that changes nothing)
        let dlat = buffer / 110_574.0 * 1.01 + 1e-9;
        let amax = ok.iter().map(|&k| lat[k].abs()).chain(refs.iter().map(|r| r.1.abs())).fold(0.0f64, f64::max);
        let top = (amax + dlat).min(90.0);
        if top < 89.0 {
            let dlon = buffer / (111_319.0 * top.to_radians().cos()) * 1.01 + 1e-9;
            let (xa, xb) = min_max(ok.iter().map(|&k| lon[k]));
            let (ya, yb) = min_max(ok.iter().map(|&k| lat[k]));
            let (x0, x1) = (xa - dlon, xb + dlon);
            let wrap = !(x0 > -180.0 && x1 < 180.0);
            refs.retain(|&(x, y)| y >= ya - dlat && y <= yb + dlat && (wrap || (x >= x0 && x <= x1)));
        }
    }
    if refs.is_empty() {
        return Ok(vec![false; lon.len()]);
    }
    let (rx, ry): (Vec<f64>, Vec<f64>) = refs.into_iter().unzip();
    let shapes = circles(&rx, &ry, buffer, if geod { "geosphere" } else { "terra" })?;
    Ok(Shapes::new(shapes.into_iter().flatten().collect()).covered(lon, lat))
}

/// The species of a record (None missing).
type Species = Vec<Option<String>>;

/// `verify` of cc_cap, cc_cen and cc_gbif: a flag kept only where more flagged records (of any species) share
/// its coordinates than its species has flagged records.
fn verify(mut flag: Vec<bool>, lon: &[f64], lat: &[f64], sp: &Species) -> Vec<bool> {
    if !flag.iter().any(|&f| f) {
        return flag;
    }
    let mut coord: HashMap<(u64, u64), usize> = HashMap::new();
    let mut spc: HashMap<&Option<String>, usize> = HashMap::new();
    for k in (0..flag.len()).filter(|&k| flag[k]) {
        *coord.entry((lon[k].to_bits(), lat[k].to_bits())).or_default() += 1;
        *spc.entry(&sp[k]).or_default() += 1;
    }
    for k in 0..flag.len() {
        if flag[k] {
            flag[k] = coord[&(lon[k].to_bits(), lat[k].to_bits())] > spc[&sp[k]];
        }
    }
    flag
}

fn test_zeros(lon: &[f64], lat: &[f64], o: &JsonMap<String, Json>) -> Result<Vec<bool>, String> {
    let b = num(o, "buffer")?;
    let r = if b != 0.0 { b } else { 1e-14 };
    let circle = Shapes::new(circles(&[0.0], &[0.0], r, "planar")?.into_iter().flatten().collect());
    let inside = circle.covered(lon, lat);
    Ok((0..lon.len()).map(|k| lon[k] == 0.0 || lat[k] == 0.0 || inside[k]).collect())
}

fn test_near(
    lon: &[f64],
    lat: &[f64],
    o: &JsonMap<String, Json>,
    sp: &Species,
    refs: (Vec<f64>, Vec<f64>),
    crop: bool,
) -> Result<Vec<bool>, String> {
    let flag = near_points(lon, lat, &refs.0, &refs.1, num(o, "buffer")?, flag_opt(o, "geod")?, crop)?;
    Ok(if o.contains_key("verify") && flag_opt(o, "verify")? { verify(flag, lon, lat, sp) } else { flag })
}

fn test_institutions(
    lon: &[f64],
    lat: &[f64],
    o: &JsonMap<String, Json>,
    sp: &Species,
    refs: (Vec<f64>, Vec<f64>),
) -> Result<Vec<bool>, String> {
    let b = num(o, "buffer")?;
    let buffer = if b != 0.0 { b } else { 1e-11 };
    let geod = flag_opt(o, "geod")?;
    let mut flag = near_points(lon, lat, &refs.0, &refs.1, buffer, geod, false)?;
    if flag_opt(o, "verify")? && flag.iter().any(|&f| f) {
        let idx: Vec<usize> = (0..flag.len()).filter(|&k| flag[k]).collect();
        let (cx, cy): (Vec<f64>, Vec<f64>) = idx.iter().map(|&k| (lon[k], lat[k])).unzip();
        let kind = if geod { "geosphere" } else { "terra" };
        let shapes = circles(&cx, &cy, buffer * num(o, "verify_mltpl")?, kind)?;
        for (c, &i) in shapes.into_iter().zip(&idx) {
            let same: Vec<usize> = (0..lon.len()).filter(|&k| sp[i].is_some() && sp[k] == sp[i]).collect();
            let n_in = match c {
                Some(c) => same
                    .iter()
                    .filter(|&&k| lon[k].is_finite() && lat[k].is_finite() && c.covers([lon[k], lat[k]]))
                    .count(),
                None => 0,
            };
            let n_same = same.iter().filter(|&&k| flag[k] && lon[k] == lon[i] && lat[k] == lat[i]).count();
            flag[i] = n_in <= n_same;
        }
    }
    Ok(flag)
}

fn test_seas(lon: &[f64], lat: &[f64], land: &Shapes) -> Vec<bool> {
    // each place once
    let mut first: HashMap<(u64, u64), usize> = HashMap::new();
    let (mut ux, mut uy, mut at) = (Vec::new(), Vec::new(), Vec::with_capacity(lon.len()));
    for k in 0..lon.len() {
        let n = first.len();
        let j = *first.entry((lon[k].to_bits(), lat[k].to_bits())).or_insert_with(|| {
            ux.push(lon[k]);
            uy.push(lat[k]);
            n
        });
        at.push(j);
    }
    let on_land = land.covered(&ux, &uy);
    (0..lon.len()).map(|k| lon[k].is_finite() && lat[k].is_finite() && !on_land[at[k]]).collect()
}

/// A column's values as texts for the comparison of codes (None missing).
fn codes_of(c: Column) -> Vec<Option<String>> {
    match c {
        Column::Texts(t) => t
            .into_iter()
            .map(|s| {
                let s = s.trim().to_uppercase();
                (!s.is_empty()).then_some(s)
            })
            .collect(),
        Column::Numbers(x) => x.into_iter().map(|v| (!v.is_nan()).then(|| format!("{v:?}"))).collect(),
    }
}

fn test_countries(
    lon: &[f64],
    lat: &[f64],
    o: &JsonMap<String, Json>,
    occ: &Occurrences,
    given: Option<&Vector>,
) -> Result<Vec<bool>, String> {
    let col = text_opt(o, "column")?;
    let Some(column) = col.as_deref().and_then(|c| occ.column(c)) else {
        return Err("the countries test needs the column of country codes: countries = \"countryCode\" or \
                    {\"column\": ...}"
            .into());
    };
    let codes = codes_of(column);
    let loaded;
    let reference = match given {
        Some(v) => v,
        None => {
            loaded = refdata::natural_earth("countries50")?;
            &loaded
        }
    };
    let ref_col = match text_opt(o, "ref_col")? {
        Some(c) => c,
        None => {
            let lengths: HashSet<usize> = codes.iter().flatten().map(|c| c.chars().count()).collect();
            if lengths.len() == 1 && lengths.contains(&2) { "iso_a2_eh" } else { "iso_a3_eh" }.to_string()
        }
    };
    let Some((_, ref_codes)) = reference.attributes.iter().find(|(n, _)| *n == ref_col) else {
        return Err(format!("the country polygons have no column {ref_col:?}"));
    };
    let ref_codes: Vec<Option<String>> = match ref_codes {
        Column::Texts(t) => t.iter().map(|s| (!s.is_empty()).then(|| s.clone())).collect(),
        Column::Numbers(x) => x.iter().map(|v| (!v.is_nan()).then(|| format!("{v:?}"))).collect(),
    };
    let geoms: Vec<(Shape, Option<String>)> = {
        let v = match &reference.crs {
            Some(c) if !terraflux_gdal::same_crs(c, "EPSG:4326")? => {
                terraflux_gdal::reproject_vector(reference, "EPSG:4326")?
            }
            _ => reference.clone(),
        };
        v.geometries.iter().zip(ref_codes).filter_map(|(g, c)| Shape::of(g).map(|s| (s, c))).collect()
    };
    let (shapes, polygon_codes): (Vec<Shape>, Vec<Option<String>>) = geoms.into_iter().unzip();
    let index = Shapes::new(shapes);
    Ok((0..lon.len())
        .into_par_iter()
        .map(|k| {
            let Some(code) = &codes[k] else { return true };
            let p = [lon[k], lat[k]];
            if !(p[0].is_finite() && p[1].is_finite()) {
                return true;
            }
            !index.covering(p).any(|j| polygon_codes[j].as_ref() == Some(code))
        })
        .collect())
}

/// A value of a record for the comparison of duplicates (NaN equal to NaN, -0 to 0).
#[derive(PartialEq, Eq, Hash)]
enum Key {
    Number(u64),
    Text(Option<String>),
}

fn number_key(v: f64) -> Key {
    Key::Number(if v.is_nan() {
        f64::NAN.to_bits()
    } else if v == 0.0 {
        0
    } else {
        v.to_bits()
    })
}

fn test_duplicates(
    lon: &[f64],
    lat: &[f64],
    sp: &Species,
    occ: &Occurrences,
    o: &JsonMap<String, Json>,
) -> Result<Vec<bool>, String> {
    let mut extra: Vec<Vec<Key>> = Vec::new();
    match &o["additions"] {
        Json::Null => {}
        Json::Array(cols) => {
            for c in cols {
                let name = c.as_str().ok_or("additions: the names of columns")?;
                let col = occ.column(name).ok_or_else(|| format!("no column {name:?}"))?;
                extra.push(match col {
                    Column::Numbers(x) => x.into_iter().map(number_key).collect(),
                    Column::Texts(t) => t.into_iter().map(|s| Key::Text((!s.is_empty()).then_some(s))).collect(),
                });
            }
        }
        v => return Err(format!("additions: a list of columns, not {v}")),
    }
    let mut seen = HashSet::new();
    Ok((0..lon.len())
        .map(|k| {
            let mut key = vec![number_key(lon[k]), number_key(lat[k]), Key::Text(sp[k].clone())];
            for e in &mut extra {
                key.push(std::mem::replace(&mut e[k], Key::Number(0)));
            }
            !seen.insert(key)
        })
        .collect())
}

/// R's median (NaN left out; the mean of the middle two, R's 80-bit mean).
fn r_median(x: &[f64]) -> f64 {
    let mut x: Vec<f64> = x.iter().copied().filter(|v| !v.is_nan()).collect();
    if x.is_empty() {
        return f64::NAN;
    }
    x.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = x.len();
    let half = n.div_ceil(2);
    if n % 2 == 1 { x[half - 1] } else { ffi::cost_r_mean(&x[half - 1..half + 1]) }
}

/// The records a species' statistic marks as outliers (cc_outl's thresholds).
fn outlier_flags(stat: &[f64], o: &JsonMap<String, Json>, method: &str) -> Result<Vec<bool>, String> {
    let thr = match method {
        "distance" => num(o, "tdi")?,
        "quantile" => {
            if stat.iter().all(|v| v.is_nan()) {
                return Ok(vec![false; stat.len()]);
            }
            let q = quantile7(stat, &[0.25, 0.75]);
            q[1] + (q[1] - q[0]) * num(o, "mltpl")?
        }
        "mad" => {
            let med = r_median(stat);
            let dev: Vec<f64> = stat.iter().map(|v| (v - med).abs()).collect();
            med + 1.4826 * r_median(&dev) * num(o, "mltpl")?
        }
        _ => return Err("outliers method must be quantile, mad or distance".into()),
    };
    Ok(stat.iter().map(|&v| v > thr).collect())
}

/// cc_outl's grid (ras_create): the extent of every record widened by 2 cells (within the globe), cells of
/// about `res`.
fn outlier_grid(lon: &[f64], lat: &[f64], res: f64) -> (f64, f64, f64, f64, i64, i64) {
    let ok: Vec<usize> = (0..lon.len()).filter(|&k| lon[k].is_finite() && lat[k].is_finite()).collect();
    let (xa, xb) = min_max(ok.iter().map(|&k| lon[k]));
    let (ya, yb) = min_max(ok.iter().map(|&k| lat[k]));
    let (xmin, xmax) = ((xa - 2.0 * res).max(-180.0), (xb + 2.0 * res).min(180.0));
    let (ymin, ymax) = ((ya - 2.0 * res).max(-90.0), (yb + 2.0 * res).min(90.0));
    // C's round(): half away from zero
    let nc = (((xmax - xmin) / res).round() as i64).max(1);
    let nr = (((ymax - ymin) / res).round() as i64).max(1);
    (xmin, xmax, ymin, ymax, nr, nc)
}

fn test_outliers(lon: &[f64], lat: &[f64], sp: &Species, o: &JsonMap<String, Json>) -> Result<Vec<bool>, String> {
    let method = text_opt(o, "method")?.unwrap_or_else(|| "quantile".into());
    if !matches!(method.as_str(), "quantile" | "mad" | "distance") {
        return Err("outliers method must be quantile, mad or distance".into());
    }
    let use_min = method == "distance";
    let (size, min_occs) = (num(o, "size")?, num(o, "min_occs")?);
    let thinning = flag_opt(o, "thinning")?;
    let res = num(o, "thinning_res")?;
    let ok = |k: usize| lon[k].is_finite() && lat[k].is_finite();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for k in (0..lon.len()).filter(|&k| ok(k)) {
        if let Some(s) = &sp[k] {
            *counts.entry(s).or_default() += 1;
        }
    }
    let mut groups: Vec<&str> = counts.iter().filter(|(_, c)| **c as f64 > size).map(|(s, _)| *s).collect();
    groups.sort();
    let mut flag = vec![false; lon.len()];
    let mut grid = None;
    for s in groups {
        let idx: Vec<usize> = (0..lon.len()).filter(|&k| ok(k) && sp[k].as_deref() == Some(s)).collect();
        let (x, y): (Vec<f64>, Vec<f64>) = idx.iter().map(|&k| (lon[k], lat[k])).unzip();
        let places: HashSet<(u64, u64)> = x.iter().zip(&y).map(|(a, b)| (a.to_bits(), b.to_bits())).collect();
        if (places.len() as f64) < min_occs {
            continue;
        }
        if idx.len() < 10_000 && !thinning {
            let stat = ffi::sdm_outlier_statistic(&x, &y, use_min).map_err(|e| e.to_string())?;
            for (j, f) in outlier_flags(&stat, o, &method)?.into_iter().enumerate() {
                flag[idx[j]] = f;
            }
            continue;
        }
        let (xmin, xmax, ymin, ymax, nr, nc) = *grid.get_or_insert_with(|| outlier_grid(lon, lat, res));
        let (fx, fy) = (nc as f64 / (xmax - xmin), nr as f64 / (ymax - ymin));
        let mut order: HashMap<i64, usize> = HashMap::new();
        let (mut cells, mut cnt, mut at) = (Vec::new(), Vec::new(), Vec::with_capacity(idx.len()));
        for j in 0..idx.len() {
            let col = if x[j] == xmax { nc - 1 } else { ((x[j] - xmin) * fx).floor() as i64 };
            let row = if y[j] == ymin { nr - 1 } else { ((ymax - y[j]) * fy).floor() as i64 };
            let cell = row * nc + col;
            let n = order.len();
            let c = *order.entry(cell).or_insert_with(|| {
                cells.push(cell);
                cnt.push(0.0);
                n
            });
            cnt[c] += 1.0;
            at.push(c);
        }
        let (xres, yres) = ((xmax - xmin) / nc as f64, (ymax - ymin) / nr as f64);
        let cx: Vec<f64> = cells.iter().map(|&c| xmin + ((c % nc) as f64 + 0.5) * xres).collect();
        let cy: Vec<f64> = cells.iter().map(|&c| ymax - ((c / nc) as f64 + 0.5) * yres).collect();
        let stat = if thinning {
            ffi::sdm_outlier_statistic(&cx, &cy, use_min)
        } else {
            let within = 100.0 * ffi::cost_r_mean(&[xres, yres]) / 2.0;
            ffi::sdm_outlier_statistic_weighted(&cx, &cy, &cnt, within, use_min)
        }
        .map_err(|e| e.to_string())?;
        let cell_flag = outlier_flags(&stat, o, &method)?;
        for (j, &k) in idx.iter().enumerate() {
            flag[k] = cell_flag[at[j]];
        }
    }
    Ok(flag)
}

// ------------------------------------------------------------------------------------------- the API

/// The result of the cleaning: each test run and its flags (true suspicious), and any of them.
pub struct Cleaned {
    pub flags: Vec<(String, Vec<bool>)>,
    pub any: Vec<bool>,
}

/// The species column: the one given, else species, scientificName or label when present.
fn species_of(occ: &Occurrences, species: Option<&str>) -> Result<Species, String> {
    let columns = occ.columns();
    let name = match species {
        Some(s) if !columns.iter().any(|c| c == s) => return Err(format!("no column {s:?}")),
        Some(s) => Some(s.to_string()),
        None => ["species", "scientificName", "label"]
            .iter()
            .find(|c| columns.iter().any(|n| n == *c))
            .map(|c| c.to_string()),
    };
    Ok(match name.and_then(|n| occ.column(&n)) {
        None => vec![Some("all".into()); occ.len()],
        Some(Column::Texts(t)) => t.into_iter().map(|s| (!s.is_empty()).then_some(s)).collect(),
        Some(Column::Numbers(x)) => x.into_iter().map(|v| (!v.is_nan()).then(|| format!("{v:?}"))).collect(),
    })
}

/// The tests' settings by name (absent: the test's default).
pub type Settings<'a> = HashMap<&'a str, Json>;

/// CoordinateCleaner's tests of the records (landschaft's clean_occurrences with action "flag").
pub fn clean(
    occ: &Occurrences,
    species: Option<&str>,
    settings: &Settings,
    refs: &Refs,
    x: Option<&str>,
    y: Option<&str>,
) -> Result<Cleaned, String> {
    let (mut lon, mut lat, crs) = occ.coordinates(x, y)?;
    to_crs(&mut lon, &mut lat, crs.as_deref(), Some("EPSG:4326"))?;
    for v in lon.iter_mut().chain(lat.iter_mut()) {
        if *v == 0.0 {
            *v = 0.0;
        }
    }
    let invalid: Vec<bool> = (0..lon.len())
        .map(|k| {
            !(lon[k].is_finite()
                && lat[k].is_finite()
                && (-180.0..=180.0).contains(&lon[k])
                && (-90.0..=90.0).contains(&lat[k]))
        })
        .collect();
    let vx: Vec<f64> = (0..lon.len()).map(|k| if invalid[k] { f64::NAN } else { lon[k] }).collect();
    let vy: Vec<f64> = (0..lon.len()).map(|k| if invalid[k] { f64::NAN } else { lat[k] }).collect();
    let sp = species_of(occ, species)?;
    let defaults: [(&str, Json, &str, Json); 12] = [
        ("equal", Json::Bool(true), "", serde_json::json!({})),
        ("zeros", 0.5.into(), "buffer", serde_json::json!({"buffer": 0.5})),
        ("capitals", 10_000.0.into(), "buffer", serde_json::json!({"buffer": 10_000.0, "geod": true, "verify": false})),
        (
            "centroids",
            1_000.0.into(),
            "buffer",
            serde_json::json!({"buffer": 1_000.0, "detail": "both", "geod": true, "verify": false}),
        ),
        ("seas", Json::Bool(true), "scale", serde_json::json!({"scale": 50})),
        ("urban", Json::Bool(false), "", serde_json::json!({})),
        ("countries", Json::Bool(false), "", serde_json::json!({"column": null, "ref_col": null})),
        (
            "outliers",
            Json::Bool(true),
            "mltpl",
            serde_json::json!({"method": "quantile", "mltpl": 5.0, "tdi": 1000.0, "size": 7, "min_occs": 7,
                               "thinning": false, "thinning_res": 0.5}),
        ),
        ("gbif", 1_000.0.into(), "buffer", serde_json::json!({"buffer": 1_000.0, "geod": true, "verify": false})),
        (
            "institutions",
            100.0.into(),
            "buffer",
            serde_json::json!({"buffer": 100.0, "geod": false, "verify": false, "verify_mltpl": 10.0}),
        ),
        ("duplicates", Json::Bool(false), "", serde_json::json!({"additions": null})),
        (
            "aohi",
            Json::Bool(false),
            "buffer",
            serde_json::json!({"buffer": 10_000.0, "geod": true, "taxa": ["Aves", "Insecta", "Mammalia", "Plantae"]}),
        ),
    ];
    for k in settings.keys() {
        if !TESTS[1..].contains(k) {
            return Err(format!("no test {k:?}; the tests: {}", TESTS[1..].join(", ")));
        }
    }
    let mut flags: Vec<(String, Vec<bool>)> = vec![("invalid".into(), invalid.clone())];
    for (name, default, main, opts) in defaults {
        let value = match settings.get(name) {
            Some(Json::String(s)) if name == "countries" => serde_json::json!({ "column": s }),
            Some(v) => v.clone(),
            None => default,
        };
        let Some(o) = options(name, &value, main, opts)? else { continue };
        let at = |e: String| format!("{name}: {e}");
        let f = match name {
            "equal" => (0..vx.len()).map(|k| vx[k].abs() == vy[k].abs()).collect(),
            "zeros" => test_zeros(&vx, &vy, &o).map_err(at)?,
            "capitals" => {
                let r = match refs.capitals {
                    Some(r) => lonlat_of(r).map_err(at)?,
                    None => {
                        let t = refdata::table("countryref")?.filter_by("capital", |v| v.is_some());
                        (t.numbers("capital.lon"), t.numbers("capital.lat"))
                    }
                };
                test_near(&vx, &vy, &o, &sp, r, true).map_err(at)?
            }
            "centroids" => {
                let detail = text_opt(&o, "detail").map_err(at)?.unwrap_or_default();
                let kind = match detail.as_str() {
                    "both" => None,
                    "country" => Some("country"),
                    "provinces" => Some("province"),
                    _ => return Err("centroids detail must be both, country or provinces".into()),
                };
                let r = match refs.centroids {
                    Some(r) => lonlat_of(r).map_err(at)?,
                    None => {
                        let t = refdata::table("countryref")?.filter_by("type", |v| kind.is_none_or(|k| v == Some(k)));
                        (t.numbers("centroid.lon"), t.numbers("centroid.lat"))
                    }
                };
                test_near(&vx, &vy, &o, &sp, r, true).map_err(at)?
            }
            "seas" => {
                let land = match refs.seas {
                    Some(v) => shapes_lonlat(v).map_err(at)?,
                    None => {
                        let layer = match num(&o, "scale").map_err(at)? as i64 {
                            110 => "land110",
                            50 => "land50",
                            10 => "land10",
                            s => return Err(format!("seas: the scale is 110, 50 or 10, not {s}")),
                        };
                        shapes_lonlat(&refdata::natural_earth(layer).map_err(at)?).map_err(at)?
                    }
                };
                test_seas(&vx, &vy, &Shapes::new(land))
            }
            "urban" => {
                let urban = match refs.urban {
                    Some(v) => shapes_lonlat(v).map_err(at)?,
                    None => shapes_lonlat(&refdata::natural_earth("urban50").map_err(at)?).map_err(at)?,
                };
                Shapes::new(urban).covered(&vx, &vy)
            }
            "countries" => test_countries(&vx, &vy, &o, occ, refs.countries).map_err(at)?,
            "outliers" => test_outliers(&vx, &vy, &sp, &o).map_err(at)?,
            "gbif" => test_near(&vx, &vy, &o, &sp, (vec![GBIF_HQ.0], vec![GBIF_HQ.1]), false).map_err(at)?,
            "institutions" => {
                let r = match refs.institutions {
                    Some(r) => lonlat_of(r).map_err(at)?,
                    None => {
                        let t = refdata::table("institutions")?;
                        (t.numbers("decimalLongitude"), t.numbers("decimalLatitude"))
                    }
                };
                test_institutions(&vx, &vy, &o, &sp, r).map_err(at)?
            }
            "duplicates" => test_duplicates(&vx, &vy, &sp, occ, &o).map_err(at)?,
            _ => {
                let taxa: Vec<String> = match &o["taxa"] {
                    Json::Array(t) => t.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
                    Json::String(s) => vec![s.clone()],
                    v => return Err(format!("aohi: taxa is a list of names, not {v}")),
                };
                let t = refdata::table("aohi")?.filter_by("taxa", |v| v.is_some_and(|x| taxa.iter().any(|t| t == x)));
                let r = (t.numbers("decimalLongitude"), t.numbers("decimalLatitude"));
                test_near(&vx, &vy, &o, &sp, r, false).map_err(at)?
            }
        };
        let f: Vec<bool> = f.into_iter().zip(&invalid).map(|(a, &b)| a && !b).collect();
        flags.push((name.to_string(), f));
    }
    let any = (0..lon.len()).map(|k| flags.iter().any(|(_, f)| f[k])).collect();
    Ok(Cleaned { flags, any })
}
