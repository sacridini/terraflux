//! A watershed divided as QSWAT+ divides it (landschaft's `ls.hydro.swat.watershed`, bit for bit): the DEM filled and
//! drained along D8 directions, streams by a drainage area, one subbasin, channel and landscape unit per stream link,
//! HRUs (land use x soil x slope band in each landscape unit, then QSWAT+'s selection) and the routing, as the
//! `gis_*` tables the SWAT+ Editor imports, with the soils used. The terrain analysis is the kernels of
//! `hydro.*` (landschaft's C++); the tables and the HRU selection are QSWAT+'s methods (C. George, GPL-2.0-or-later;
//! `hrus.py`, `QSWATTopology.py`, `dataInC.pyx`) as landschaft ports them.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;

use crate::hydro::{Dem, cells, down, link_table, outlet_points, snap};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(WatershedOp));
}

// QSWAT+ parameters.py
const WIDTH_MULTIPLIER: f64 = 1.29;
const WIDTH_EXPONENT: f64 = 0.6;
const DEPTH_MULTIPLIER: f64 = 0.13;
const DEPTH_EXPONENT: f64 = 0.4;

/// The HRU selection methods.
pub const METHODS: [&str; 4] = ["dominant_hru", "dominant", "filter", "area"];

/// The names of the tables a watershed holds, in landschaft's order.
pub const TABLES: [&str; 10] = [
    "gis_subbasins",
    "gis_channels",
    "gis_lsus",
    "gis_hrus",
    "gis_aquifers",
    "gis_deep_aquifers",
    "gis_points",
    "gis_routing",
    "soils_sol",
    "soils_sol_layer",
];

const USERSOIL_LAYER: [&str; 12] =
    ["SOL_Z", "SOL_BD", "SOL_AWC", "SOL_K", "SOL_CBN", "CLAY", "SILT", "SAND", "ROCK", "SOL_ALB", "USLE_K", "SOL_EC"];
const LAYER_COLUMNS: [&str; 14] =
    ["dp", "bd", "awc", "soil_k", "carbon", "clay", "silt", "sand", "rock", "alb", "usle_k", "ec", "caco3", "ph"];

/// A watershed divided as QSWAT+ divides it (landschaft's SwatWatershed): QSWAT+'s project tables (`TABLES`; text
/// columns as the labels of numbers), and the grids on the DEM's cells the maps of the model are made from.
#[derive(Debug, Clone)]
pub struct Watershed {
    pub tables: Vec<(String, Table)>,
    /// The subbasin (= channel, SWAT+ numbering) of each cell, 0 outside the watershed, NaN where the DEM is missing.
    pub subbasins: Map,
    /// The HRU of each cell (`gis_hrus` id), NaN for cells whose HRU was removed by the selection, and outside.
    pub hrus: Map,
    /// The channel of each stream cell, 0 elsewhere.
    pub channels: Map,
    pub notes: Vec<String>,
}

impl Watershed {
    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|(n, _)| n == name).map(|(_, t)| t)
    }
}

/// The watershed as it is written: grids by their numbers (missing: null), tables likewise.
#[derive(Serialize, Deserialize)]
struct Raw {
    tables: Vec<(String, RawTable)>,
    grid: Grid,
    subbasins: Vec<Option<f64>>,
    hrus: Vec<Option<f64>>,
    channels: Vec<Option<f64>>,
    #[serde(default)]
    notes: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct RawTable {
    columns: Vec<String>,
    rows: Vec<Vec<Option<f64>>>,
    #[serde(default)]
    labels: BTreeMap<String, Vec<String>>,
}

fn opt(v: &[f64]) -> Vec<Option<f64>> {
    v.iter().map(|x| (!x.is_nan()).then_some(*x)).collect()
}

fn unopt(v: Vec<Option<f64>>) -> Vec<f64> {
    v.into_iter().map(|x| x.unwrap_or(f64::NAN)).collect()
}

impl Serialize for Watershed {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        Raw {
            tables: self
                .tables
                .iter()
                .map(|(n, t)| {
                    let rows = t.rows.iter().map(|r| opt(r)).collect();
                    (n.clone(), RawTable { columns: t.columns.clone(), rows, labels: t.labels.clone() })
                })
                .collect(),
            grid: (*self.subbasins.grid).clone(),
            subbasins: opt(&self.subbasins.data),
            hrus: opt(&self.hrus.data),
            channels: opt(&self.channels.data),
            notes: self.notes.clone(),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Watershed {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let raw = Raw::deserialize(d)?;
        let grid = Arc::new(raw.grid);
        let tables = raw
            .tables
            .into_iter()
            .map(|(n, t)| {
                let rows = t.rows.into_iter().map(unopt).collect();
                (n, Table { columns: t.columns, rows, labels: t.labels })
            })
            .collect();
        let map = |v: Vec<Option<f64>>| Map::new(grid.clone(), unopt(v));
        Ok(Watershed {
            tables,
            subbasins: map(raw.subbasins),
            hrus: map(raw.hrus),
            channels: map(raw.channels),
            notes: raw.notes,
        })
    }
}

/// The settings of the division (landschaft's keyword arguments).
#[derive(Debug, Clone)]
pub struct Settings {
    pub stream_threshold: Option<f64>,
    pub snap: Option<f64>,
    pub slope_bands: Vec<f64>,
    pub hru_method: String,
    pub thresholds: [f64; 3],
    pub min_area: f64,
    pub area_in_ha: bool,
    pub exempt: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            stream_threshold: None,
            snap: None,
            slope_bands: Vec::new(),
            hru_method: "dominant_hru".into(),
            thresholds: [0.0; 3],
            min_area: 0.0,
            area_in_ha: true,
            exempt: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------------------------------- tables

enum Col {
    Num(Vec<f64>),
    Text(Vec<Option<String>>),
}

/// A table from columns: text ones as the labels of numbers 1, 2... (NaN for none), in order of first appearance.
fn frame(cols: Vec<(&str, Col)>) -> Table {
    let n = cols.first().map_or(0, |(_, c)| match c {
        Col::Num(v) => v.len(),
        Col::Text(v) => v.len(),
    });
    let mut t = Table::new(cols.iter().map(|(c, _)| c.to_string()).collect());
    t.rows = vec![Vec::with_capacity(cols.len()); n];
    for (name, col) in cols {
        match col {
            Col::Num(v) => {
                for (r, x) in t.rows.iter_mut().zip(v) {
                    r.push(x);
                }
            }
            Col::Text(v) => {
                let mut names: Vec<String> = Vec::new();
                for (r, s) in t.rows.iter_mut().zip(v) {
                    r.push(match s {
                        None => f64::NAN,
                        Some(s) => {
                            let k = match names.iter().position(|x| *x == s) {
                                Some(k) => k,
                                None => {
                                    names.push(s);
                                    names.len() - 1
                                }
                            };
                            (k + 1) as f64
                        }
                    });
                }
                if !names.is_empty() {
                    t.labels.insert(name.to_string(), names);
                }
            }
        }
    }
    t
}

fn text(v: &[&str]) -> Col {
    Col::Text(v.iter().map(|s| Some(s.to_string())).collect())
}

/// A table read by column names without regard to case.
struct Df<'a> {
    t: &'a Table,
    upper: HashMap<String, usize>,
}

impl<'a> Df<'a> {
    fn new(t: &'a Table) -> Df<'a> {
        let mut upper = HashMap::new();
        for (j, c) in t.columns.iter().enumerate() {
            upper.entry(c.to_uppercase()).or_insert(j);
        }
        Df { t, upper }
    }

    fn has(&self, name: &str) -> bool {
        self.upper.contains_key(&name.to_uppercase())
    }

    fn col(&self, name: &str) -> Result<usize> {
        self.upper
            .get(&name.to_uppercase())
            .copied()
            .ok_or_else(|| Error::msg(format!("the soil table has no column {name:?}")))
    }

    fn num(&self, row: usize, name: &str) -> Result<f64> {
        Ok(self.t.rows[row][self.col(name)?])
    }

    /// The number, or None for none.
    fn opt_num(&self, row: usize, name: &str) -> Option<f64> {
        let v = self.t.rows[row][*self.upper.get(&name.to_uppercase())?];
        (!v.is_nan()).then_some(v)
    }

    fn text(&self, row: usize, name: &str) -> Result<String> {
        let j = self.col(name)?;
        Ok(self.t.cell(j, self.t.rows[row][j]).trim().to_string())
    }
}

/// {raster value: name} from a two-column table (value, name), the names in order of the values' first appearance
/// (a value given twice takes the later name).
fn lookup(t: &Table, what: &str) -> Result<Vec<(i64, String)>> {
    if t.columns.len() < 2 {
        return Err(Error::msg(format!("the {what} lookup needs two columns: raster value and name")));
    }
    let mut out: Vec<(i64, String)> = Vec::new();
    for r in &t.rows {
        if r[1].is_nan() {
            continue;
        }
        if r[0].is_nan() {
            return Err(Error::msg(format!("the {what} lookup has a row without a raster value")));
        }
        let k = r[0] as i64;
        let name = t.cell(1, r[1]).trim().to_string();
        match out.iter_mut().find(|(c, _)| *c == k) {
            Some(e) => e.1 = name,
            None => out.push((k, name)),
        }
    }
    if out.is_empty() {
        return Err(Error::msg(format!("the {what} lookup is empty")));
    }
    Ok(out)
}

/// soils_sol and soils_sol_layer of the soils used, from a usersoil table (ArcSWAT / QSWAT+'s layout) or SWAT+'s
/// own (one row per layer).
fn soil_tables(soil_table: &Table, names: &[String]) -> Result<(Table, Table)> {
    let df = Df::new(soil_table);
    let nrows = soil_table.rows.len();
    // soils: id, name, hyd_grp, dp_tot, anion_excl, perc_crk, texture; layers: soil_id, layer_num, the 14 values
    let (mut ids, mut hyd, mut dp_tot, mut anion, mut crk) = (vec![], vec![], vec![], vec![], vec![]);
    let mut texture: Vec<Option<String>> = Vec::new();
    let (mut soil_id, mut layer_num) = (Vec::new(), Vec::new());
    let mut layer_values: Vec<Vec<f64>> = vec![Vec::new(); LAYER_COLUMNS.len()];
    let mut push_layer = |sid: usize, k: usize, vals: Vec<f64>| {
        soil_id.push(sid as f64);
        layer_num.push(k as f64);
        for (c, v) in layer_values.iter_mut().zip(vals) {
            c.push(v);
        }
    };
    if df.has("SNAM") {
        // the last row of a name stands for it
        let mut by_name: HashMap<String, usize> = HashMap::new();
        for r in 0..nrows {
            by_name.insert(df.text(r, "SNAM")?, r);
        }
        for (i, name) in names.iter().enumerate() {
            let sid = i + 1;
            let r = *by_name.get(name).ok_or_else(|| Error::msg(format!("soil {name:?} is not in the soil table")))?;
            ids.push(sid as f64);
            hyd.push(Some(df.text(r, "HYDGRP")?));
            dp_tot.push(df.num(r, "SOL_ZMX")?);
            anion.push(df.num(r, "ANION_EXCL")?);
            crk.push(df.num(r, "SOL_CRK")?);
            texture.push(if df.has("TEXTURE") { Some(df.text(r, "TEXTURE")?) } else { None });
            let n = df.num(r, "NLAYERS")? as i64;
            for k in 1..=n.max(0) as usize {
                let mut vals = Vec::new();
                for src in USERSOIL_LAYER {
                    vals.push(df.num(r, &format!("{src}{k}"))?);
                }
                for src in ["SOL_CAL", "SOL_PH"] {
                    vals.push(df.opt_num(r, &format!("{src}{k}")).unwrap_or(f64::NAN));
                }
                push_layer(sid, k, vals);
            }
        }
    } else if df.has("NAME") {
        for (i, name) in names.iter().enumerate() {
            let sid = i + 1;
            let mut rows = Vec::new();
            for r in 0..nrows {
                if df.text(r, "NAME")? == *name {
                    rows.push(r);
                }
            }
            let Some(&r) = rows.first() else {
                return Err(Error::msg(format!("soil {name:?} is not in the soil table")));
            };
            ids.push(sid as f64);
            hyd.push(Some(df.text(r, "hyd_grp")?));
            dp_tot.push(df.num(r, "dp_tot")?);
            anion.push(df.num(r, "anion_excl")?);
            crk.push(df.num(r, "perc_crk")?);
            texture.push(if df.has("TEXTURE") { Some(df.text(r, "texture")?) } else { None });
            for (k, &lr) in rows.iter().enumerate() {
                let vals = LAYER_COLUMNS.iter().map(|c| df.opt_num(lr, c).unwrap_or(f64::NAN)).collect();
                push_layer(sid, k + 1, vals);
            }
        }
    } else {
        return Err(Error::msg(
            "the soil table is neither a usersoil table (SNAM, NLAYERS, ...) nor SWAT+'s (name, dp, ...)",
        ));
    }
    let n = ids.len();
    let soils = frame(vec![
        ("id", Col::Num(ids)),
        ("name", Col::Text(names.iter().map(|s| Some(s.clone())).collect())),
        ("hyd_grp", Col::Text(hyd)),
        ("dp_tot", Col::Num(dp_tot)),
        ("anion_excl", Col::Num(anion)),
        ("perc_crk", Col::Num(crk)),
        ("texture", Col::Text(texture)),
        ("description", Col::Num(vec![f64::NAN; n])),
    ]);
    let nl = soil_id.len();
    let mut cols = vec![
        ("id", Col::Num((1..=nl).map(|i| i as f64).collect())),
        ("soil_id", Col::Num(soil_id)),
        ("layer_num", Col::Num(layer_num)),
    ];
    for (name, v) in LAYER_COLUMNS.iter().zip(layer_values) {
        cols.push((name, Col::Num(v)));
    }
    Ok((soils, frame(cols)))
}

// ------------------------------------------------------------------------------------------- QSWAT+'s HRUs

/// QSWAT+'s slopeIndex: the first band whose upper limit is above the slope.
fn slope_band(percent: f64, limits: &[f64]) -> i64 {
    let mut out = limits.len() as i64;
    for i in (0..limits.len()).rev() {
        if percent < limits[i] {
            out = i as i64;
        }
    }
    out
}

/// QSWAT+'s slopeRange label, e.g. `0-9999`.
fn slope_range(i: i64, limits: &[f64]) -> String {
    let lo = if i == 0 { 0.0 } else { limits[i as usize - 1] };
    let hi = if i as usize == limits.len() { 9999.0 } else { limits[i as usize] };
    let fmt = |v: f64| if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") };
    format!("{}-{}", fmt(lo), fmt(hi))
}

/// QSWATUtils.getSlsubbsn: the average slope length (m) from the mean slope (m/m).
fn sl_subbsn(mean_slope: f64) -> f64 {
    if mean_slope < 0.01 {
        120.0
    } else if mean_slope < 0.02 {
        100.0
    } else if mean_slope < 0.03 {
        90.0
    } else if mean_slope < 0.05 {
        60.0
    } else {
        30.0
    }
}

/// A dictionary that keeps the order of insertion (Python's), by whole keys.
#[derive(Clone, Debug)]
struct Om<V>(Vec<(i64, V)>);

impl<V> Om<V> {
    fn new() -> Self {
        Om(Vec::new())
    }

    fn get(&self, k: i64) -> Option<&V> {
        self.0.iter().find(|(c, _)| *c == k).map(|(_, v)| v)
    }

    fn get_mut(&mut self, k: i64) -> Option<&mut V> {
        self.0.iter_mut().find(|(c, _)| *c == k).map(|(_, v)| v)
    }

    fn entry(&mut self, k: i64, make: impl FnOnce() -> V) -> &mut V {
        let at = match self.0.iter().position(|(c, _)| *c == k) {
            Some(at) => at,
            None => {
                self.0.push((k, make()));
                self.0.len() - 1
            }
        };
        &mut self.0[at].1
    }

    fn remove(&mut self, k: i64) {
        self.0.retain(|(c, _)| *c != k);
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn keys(&self) -> Vec<i64> {
        self.0.iter().map(|(k, _)| *k).collect()
    }

    fn iter(&self) -> impl Iterator<Item = (i64, &V)> {
        self.0.iter().map(|(k, v)| (*k, v))
    }

    fn values(&self) -> impl Iterator<Item = &V> {
        self.0.iter().map(|(_, v)| v)
    }
}

/// CellData of QSWAT+: totals of an HRU (or of a whole LSU).
#[derive(Clone, Debug)]
struct Cell {
    count: i64,
    area: f64,
    elev: f64,
    slope: f64,
    x: f64,
    y: f64,
}

impl Cell {
    fn multiply(&mut self, f: f64) {
        self.count = (self.count as f64 * f + 0.5) as i64;
        self.area *= f;
        self.slope *= f;
        self.elev *= f;
        self.x *= f;
        self.y *= f;
    }
}

/// LSUData of QSWAT+ (no landscapes, no water bodies).
struct Lsu {
    count: i64,
    area: f64,
    elev: f64,
    slope: f64,
    x: f64,
    y: f64,
    /// cropSoilSlopeArea
    css_area: f64,
    /// hruMap
    hru: BTreeMap<usize, Cell>,
    /// cropSoilSlopeNumbers: crop -> soil -> slope -> hru
    numbers: Om<Om<Om<usize>>>,
    orig_crop: Om<f64>,
    orig_soil: Om<f64>,
    orig_slope: Om<f64>,
    far_distance: f64,
    far_elevation: f64,
}

impl Lsu {
    fn new() -> Lsu {
        Lsu {
            count: 0,
            area: 0.0,
            elev: 0.0,
            slope: 0.0,
            x: 0.0,
            y: 0.0,
            css_area: 0.0,
            hru: BTreeMap::new(),
            numbers: Om::new(),
            orig_crop: Om::new(),
            orig_soil: Om::new(),
            orig_slope: Om::new(),
            far_distance: -1.0,
            far_elevation: 0.0,
        }
    }

    fn remove(&mut self, h: usize, crop: i64, soil: i64, slope: i64) {
        self.hru.remove(&h);
        if let Some(ss) = self.numbers.get_mut(crop) {
            if let Some(sl) = ss.get_mut(soil) {
                sl.remove(slope);
                if sl.is_empty() {
                    ss.remove(soil);
                }
            }
            if ss.is_empty() {
                self.numbers.remove(crop);
            }
        }
    }

    fn redistribute(&mut self, f: f64) {
        for c in self.hru.values_mut() {
            c.multiply(f);
        }
    }

    fn area_of(&self, h: usize) -> f64 {
        self.hru[&h].area
    }

    /// The area of a crop (summed from 0 in the order of the dictionaries).
    fn crop_area(&self, crop: i64) -> f64 {
        let mut s = 0.0;
        if let Some(ss) = self.numbers.get(crop) {
            for sl in ss.values() {
                for &h in sl.values() {
                    s += self.area_of(h);
                }
            }
        }
        s
    }

    fn soil_area(&self, crop: i64, soil: i64) -> f64 {
        let mut s = 0.0;
        if let Some(sl) = self.numbers.get(crop).and_then(|ss| ss.get(soil)) {
            for &h in sl.values() {
                s += self.area_of(h);
            }
        }
        s
    }

    fn crop_soil_areas(&self, crop: i64) -> Vec<(i64, f64)> {
        match self.numbers.get(crop) {
            Some(ss) => ss.keys().into_iter().map(|s| (s, self.soil_area(crop, s))).collect(),
            None => Vec::new(),
        }
    }

    fn slope_areas(&self, crop: i64, soil: i64) -> Vec<(i64, f64)> {
        match self.numbers.get(crop).and_then(|ss| ss.get(soil)) {
            Some(sl) => sl.iter().map(|(b, &h)| (b, self.area_of(h))).collect(),
            None => Vec::new(),
        }
    }

    fn dominant_hru(&self) -> (i64, i64, i64) {
        let (mut best, mut out) = (0.0, (-1, -1, -1));
        for (crop, ss) in self.numbers.iter() {
            for (soil, sl) in ss.iter() {
                for (slope, &h) in sl.iter() {
                    if self.area_of(h) > best {
                        best = self.area_of(h);
                        out = (crop, soil, slope);
                    }
                }
            }
        }
        out
    }
}

fn dominant_key(m: &Om<f64>) -> i64 {
    let (mut best, mut key) = (0.0, -1);
    for (k, &v) in m.iter() {
        if v > best {
            best = v;
            key = k;
        }
    }
    key
}

fn max_of(v: impl Iterator<Item = f64>) -> Option<f64> {
    v.reduce(f64::max)
}

/// Python's min(a, b).
fn min2(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The value of a key in a list of pairs.
fn at(v: &[(i64, f64)], k: i64) -> f64 {
    v.iter().find(|(c, _)| *c == k).map_or(0.0, |(_, a)| *a)
}

/// QSWAT+'s HRU selection in one landscape unit (hrus.py).
fn select(lsu: &mut Lsu, set: &Settings, exempt: &[i64]) -> Result<()> {
    let method = set.hru_method.as_str();
    if method == "dominant_hru" || method == "dominant" {
        let (crop, soil, slope) = if method == "dominant_hru" {
            lsu.dominant_hru()
        } else {
            (dominant_key(&lsu.orig_crop), dominant_key(&lsu.orig_soil), dominant_key(&lsu.orig_slope))
        };
        if crop < 0 {
            return Err(Error::msg("a landscape unit has no land use data"));
        }
        lsu.hru.clear();
        lsu.hru
            .insert(1, Cell { count: lsu.count, area: lsu.area, elev: lsu.elev, slope: lsu.slope, x: lsu.x, y: lsu.y });
        let mut sl = Om::new();
        sl.0.push((slope, 1usize));
        let mut ss = Om::new();
        ss.0.push((soil, sl));
        lsu.numbers = Om::new();
        lsu.numbers.0.push((crop, ss));
        return Ok(());
    }
    // redistributeNodataAndWater: cells without land use, soil or slope data
    let lost = lsu.area - lsu.css_area;
    if lsu.area > lost && lost > 0.0 {
        lsu.redistribute(lsu.area / (lsu.area - lost));
    }
    if method == "area" {
        let threshold = if set.area_in_ha { set.min_area * 10000.0 } else { lsu.css_area * set.min_area / 100.0 };
        let mut count = lsu.hru.len();
        let hrus_area = lsu.area;
        loop {
            let mut m = (0usize, 0i64, 0i64, 0i64);
            let mut m_area = threshold;
            for (crop, ss) in lsu.numbers.iter() {
                if exempt.contains(&crop) {
                    continue;
                }
                for (soil, sl) in ss.iter() {
                    for (slope, &h) in sl.iter() {
                        if lsu.area_of(h) < m_area {
                            m_area = lsu.area_of(h);
                            m = (h, crop, soil, slope);
                        }
                    }
                }
            }
            if m_area >= threshold {
                break;
            }
            let mut redistribute = 0.0;
            let mut done = false;
            if count > 1 {
                lsu.remove(m.0, m.1, m.2, m.3);
                count -= 1;
                redistribute = m_area;
            } else {
                done = true;
            }
            if redistribute > 0.0 {
                if hrus_area - redistribute == 0.0 {
                    return Err(Error::msg("a landscape unit has no HRUs left"));
                }
                lsu.redistribute(hrus_area / (hrus_area - redistribute));
            }
            if done {
                break;
            }
        }
        return Ok(());
    }
    // filter by land use, soil and slope percentages (removeSmallHRUsByThresholdPercent)
    let [min_crop, min_soil, min_slope] = set.thresholds;
    let crop_areas: Vec<(i64, f64)> = lsu.orig_crop.iter().map(|(k, &v)| (k, v)).collect();
    let hrus_area = lsu.area;
    let mut min_crop_area = lsu.css_area * min_crop / 100.0;
    if !lsu.numbers.keys().iter().any(|c| exempt.contains(c)) {
        min_crop_area = min2(min_crop_area, max_of(crop_areas.iter().map(|(_, a)| *a)).unwrap_or(0.0));
    }
    let mut redistribute = 0.0;
    for &(crop, area) in &crop_areas {
        if !exempt.contains(&crop) && area < min_crop_area {
            redistribute += area;
            let Some(ss) = lsu.numbers.get(crop) else { continue };
            let parts: Vec<(i64, Vec<(i64, usize)>)> =
                ss.iter().map(|(s, sl)| (s, sl.iter().map(|(b, &h)| (b, h)).collect())).collect();
            for (soil, sl) in parts {
                for (slope, h) in sl {
                    lsu.remove(h, crop, soil, slope);
                }
            }
        }
    }
    if redistribute > 0.0 {
        if hrus_area - redistribute == 0.0 {
            return Err(Error::msg("a landscape unit has no land use data"));
        }
        lsu.redistribute(hrus_area / (hrus_area - redistribute));
    }
    for crop in lsu.numbers.keys() {
        let crop_area = lsu.crop_area(crop);
        let soil_areas = lsu.crop_soil_areas(crop);
        let top =
            max_of(soil_areas.iter().map(|(_, a)| *a)).ok_or_else(|| Error::msg("a land use has no soil data"))?;
        let min_area = min2(crop_area * min_soil / 100.0, top);
        let mut removed = 0.0;
        for (soil, _) in soil_areas.iter().copied() {
            if at(&soil_areas, soil) < min_area {
                removed += at(&soil_areas, soil);
                let slopes: Vec<(i64, usize)> = match lsu.numbers.get(crop).and_then(|ss| ss.get(soil)) {
                    Some(sl) => sl.iter().map(|(b, &h)| (b, h)).collect(),
                    None => Vec::new(),
                };
                for (slope, h) in slopes {
                    lsu.remove(h, crop, soil, slope);
                }
            }
        }
        if removed > 0.0 {
            if crop_area - removed == 0.0 {
                return Err(Error::msg("a land use has no soil data"));
            }
            let f = crop_area / (crop_area - removed);
            let hs: Vec<usize> = match lsu.numbers.get(crop) {
                Some(ss) => ss.values().flat_map(|sl| sl.values().copied()).collect(),
                None => Vec::new(),
            };
            for h in hs {
                if let Some(c) = lsu.hru.get_mut(&h) {
                    c.multiply(f);
                }
            }
        }
    }
    for crop in lsu.numbers.keys() {
        let soils = lsu.numbers.get(crop).map(|ss| ss.keys()).unwrap_or_default();
        for soil in soils {
            let soil_area = lsu.soil_area(crop, soil);
            let slope_areas = lsu.slope_areas(crop, soil);
            let top = max_of(slope_areas.iter().map(|(_, a)| *a))
                .ok_or_else(|| Error::msg("a land use and soil have no slope data"))?;
            let min_area = min2(soil_area * min_slope / 100.0, top);
            let mut removed = 0.0;
            let slopes: Vec<(i64, usize)> = match lsu.numbers.get(crop).and_then(|ss| ss.get(soil)) {
                Some(sl) => sl.iter().map(|(b, &h)| (b, h)).collect(),
                None => Vec::new(),
            };
            for (slope, h) in slopes {
                if at(&slope_areas, slope) < min_area {
                    removed += at(&slope_areas, slope);
                    lsu.remove(h, crop, soil, slope);
                }
            }
            if removed > 0.0 {
                if soil_area - removed == 0.0 {
                    return Err(Error::msg("a land use and soil have no slope data"));
                }
                let f = soil_area / (soil_area - removed);
                let hs: Vec<usize> = match lsu.numbers.get(crop).and_then(|ss| ss.get(soil)) {
                    Some(sl) => sl.values().copied().collect(),
                    None => Vec::new(),
                };
                for h in hs {
                    if let Some(c) = lsu.hru.get_mut(&h) {
                        c.multiply(f);
                    }
                }
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------ the division

/// A raster's values on the DEM's cells (nearest neighbour when the grids differ).
fn on_grid(m: &Map, dem: &Map, what: &str) -> Result<Vec<f64>> {
    let (a, b) = (&*m.grid, &*dem.grid);
    let near = |x: f64, y: f64| (x - y).abs() <= 1e-9 * (1.0 + y.abs());
    if a.width == b.width && a.height == b.height && a.transform.iter().zip(&b.transform).all(|(x, y)| near(*x, *y)) {
        return Ok(m.data.to_vec());
    }
    terraflux_raster::resample::resample_band(&m.data, a, b, terraflux_raster::resample::Method::Nearest)
        .map_err(|e| Error::msg(format!("the {what} cannot be put on the DEM's cells: {e}")))
}

/// The index of each cell's code in a lookup (the default for no value); a code not in it is an error.
fn codes(values: &[f64], flat: &[usize], index: &[(i64, usize)], default: usize, what: &str) -> Result<Vec<usize>> {
    let map: HashMap<i64, usize> = index.iter().copied().collect();
    let mut unknown: Vec<i64> = Vec::new();
    let mut out = Vec::with_capacity(flat.len());
    for &i in flat {
        let v = values[i];
        if v.is_nan() {
            out.push(default);
            continue;
        }
        match map.get(&(v as i64)) {
            Some(&k) => out.push(k),
            None => {
                unknown.push(v as i64);
                out.push(default);
            }
        }
    }
    if !unknown.is_empty() {
        unknown.sort_unstable();
        unknown.dedup();
        unknown.truncate(10);
        return Err(Error::msg(format!("{what} codes {unknown:?} are not in the {what} lookup")));
    }
    Ok(out)
}

/// numpy's maximum: NaN wins.
fn np_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if b > a {
        b
    } else {
        a
    }
}

/// Divide a watershed as QSWAT+ does (landschaft's `watershed`).
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
pub fn delineate(
    dem: &Map,
    landuse: &Map,
    soils: &Map,
    landuse_lookup: &Table,
    soil_lookup: &Table,
    soil_table: &Table,
    outlets: Option<&[[f64; 2]]>,
    set: &Settings,
) -> Result<Watershed> {
    if !METHODS.contains(&set.hru_method.as_str()) {
        return Err(Error::msg(format!("hru_method must be one of {METHODS:?}")));
    }
    let Some(crs) = crate::prepare::crs_of(&dem.grid) else {
        return Err(Error::msg("the DEM needs a CRS (the model needs latitudes)"));
    };
    let (ny, nx) = dem.shape();
    let (niy, nix) = (ny as i32, nx as i32);
    let (dx, dy) = cells(&dem.grid);
    let cell_area = dx * dy;
    let z = &dem.data;
    let filled = ffi::hydro_fill(z, niy, nix, false)?;
    let fd = Dem { z: &filled, ny: niy, nx: nix, dx, dy };
    let d = fd.d8(true)?;
    let acc = ffi::hydro_d8_accumulation(&d, &[], niy, nix)?;
    let valid: Vec<bool> = d.iter().map(|&k| k != -2).collect();
    let thr = match set.stream_threshold {
        None => 0.01 * valid.iter().filter(|v| **v).count() as f64,
        Some(t) => t * 1e6 / cell_area,
    };
    let stream: Vec<u8> =
        acc.iter().zip(&valid).map(|(a, &v)| u8::from((if a.is_nan() { 0.0 } else { *a }) >= thr && v)).collect();
    if !stream.contains(&1) {
        return Err(Error::msg("no streams: lower stream_threshold"));
    }
    let mut breaks: Vec<u8> = Vec::new();
    if let Some(pts) = outlets {
        breaks = vec![0u8; stream.len()];
        for k in snap(pts, &stream, &dem.grid, set.snap, Some(&acc))? {
            breaks[k] = 1;
        }
    }
    let lk = ffi::hydro_stream_links(&d, &stream, &breaks, niy, nix)?;
    let lt = link_table(&fd, &d, &lk, &breaks);
    let n_links = lk.start.len();
    let column = |name: &str| lt.column(name).unwrap_or_default();
    // 1-based, index 0 for none
    let one = |v: Vec<f64>| std::iter::once(0.0).chain(v).collect::<Vec<f64>>();
    let (down_l, outlet_l) = (one(column("downstream")), one(column("outlet")));
    // the links kept (upstream of an outlet, the outlet links included), numbered 1..n
    let mut keep = vec![true; n_links + 1];
    if outlets.is_some() {
        let mut reach = vec![0i8; n_links + 1];
        for k0 in 1..=n_links {
            let mut path = Vec::new();
            let mut j = k0;
            while j > 0 && reach[j] == 0 && outlet_l[j] == 0.0 {
                path.push(j);
                j = down_l[j] as usize;
            }
            let v = if j > 0 && (reach[j] == 1 || outlet_l[j] != 0.0) { 1 } else { -1 };
            if j > 0 && outlet_l[j] != 0.0 {
                reach[j] = 1;
            }
            for p in path {
                reach[p] = v;
            }
        }
        keep = reach.iter().map(|&r| r == 1).collect();
    }
    keep[0] = false;
    let mut new_id = vec![0usize; n_links + 1];
    let mut n = 0;
    for k in 1..=n_links {
        if keep[k] {
            n += 1;
            new_id[k] = n;
        }
    }
    if n == 0 {
        return Err(Error::msg("no stream drains to the outlets"));
    }
    // the table of the links kept, 1..=n
    struct Link {
        down: usize,
        start_row: f64,
        start_col: f64,
        end_row: f64,
        end_col: f64,
        length: f64,
        z_start: f64,
        z_end: f64,
        order: f64,
    }
    let (sr, sc, er_, ec_) =
        (one(column("start_row")), one(column("start_col")), one(column("end_row")), one(column("end_col")));
    let (len_l, zs_l, ze_l, ord_l) =
        (one(column("length")), one(column("z_start")), one(column("z_end")), one(column("order")));
    let mut links: Vec<Link> = vec![Link {
        down: 0,
        start_row: 0.0,
        start_col: 0.0,
        end_row: 0.0,
        end_col: 0.0,
        length: 0.0,
        z_start: 0.0,
        z_end: 0.0,
        order: 0.0,
    }];
    for k in 1..=n_links {
        if keep[k] {
            links.push(Link {
                down: new_id[down_l[k] as usize],
                start_row: sr[k],
                start_col: sc[k],
                end_row: er_[k],
                end_col: ec_[k],
                length: len_l[k],
                z_start: zs_l[k],
                z_end: ze_l[k],
                order: ord_l[k],
            });
        }
    }
    let seeds: Vec<i32> = lk.link.iter().map(|&l| new_id[l as usize] as i32).collect();
    let basin = ffi::hydro_watersheds(&d, &seeds, niy, nix)?;

    // rasters on the DEM grid
    let lu = on_grid(landuse, dem, "land use")?;
    let so = on_grid(soils, dem, "soils")?;
    let lu_names = lookup(landuse_lookup, "land use")?;
    let soil_names = lookup(soil_lookup, "soil")?;
    let mut crops: Vec<String> = Vec::new();
    for (_, v) in &lu_names {
        if !crops.contains(v) {
            crops.push(v.clone());
        }
    }
    let crop_of: Vec<(i64, usize)> =
        lu_names.iter().map(|(k, v)| (*k, crops.iter().position(|c| c == v).unwrap())).collect();
    let default_crop = crop_of.iter().find(|(k, _)| *k == 0).unwrap_or(&crop_of[0]).1;
    let mut soil_list: Vec<String> = Vec::new();
    for (_, v) in &soil_names {
        if !soil_list.contains(v) {
            soil_list.push(v.clone());
        }
    }
    let soil_of: Vec<(i64, usize)> =
        soil_names.iter().map(|(k, v)| (*k, soil_list.iter().position(|c| c == v).unwrap())).collect();
    let default_soil = soil_of.iter().find(|(k, _)| *k == 0).unwrap_or(&soil_of[0]).1;

    let flat: Vec<usize> = (0..basin.len()).filter(|&i| basin[i] > 0).collect();
    let lsu: Vec<usize> = flat.iter().map(|&i| basin[i] as usize).collect();
    let crop = codes(&lu, &flat, &crop_of, default_crop, "land use")?;
    let soil = codes(&so, &flat, &soil_of, default_soil, "soil")?;
    let nxt = down(&d, ny as i64, nx as i64);
    let diag = dx.hypot(dy);
    let steps = [diag, dx, diag, dy, diag, dx, diag, dy];
    let limits = &set.slope_bands;
    let m_cells = flat.len();
    let (mut slope, mut band) = (Vec::with_capacity(m_cells), Vec::with_capacity(m_cells));
    for &i in &flat {
        let step = steps[d[i].clamp(0, 7) as usize];
        let s = if nxt[i] >= 0 { (filled[i] - filled[nxt[i] as usize]) / step } else { 0.0 };
        // numpy's maximum: a NaN stays
        let s = if s.is_nan() || s > 0.0 { s } else { 0.0 };
        slope.push(s);
        band.push(slope_band(s * 100.0, limits));
    }
    let elev: Vec<f64> = flat.iter().map(|&i| z[i]).collect();
    let e = crate::surface::extent_of(&dem.grid).0;
    let (xmin, xmax, ymin, ymax) = (e[0], e[1], e[2], e[3]);
    let (rx, ry) = ((xmax - xmin) / nx as f64, (ymax - ymin) / ny as f64);
    let xs: Vec<f64> = flat.iter().map(|&i| xmin + ((i % nx) as f64 + 0.5) * rx).collect();
    let ys: Vec<f64> = flat.iter().map(|&i| ymax - ((i / nx) as f64 + 0.5) * ry).collect();
    // flow distances to the channels and to the subbasin outlets (TauDEM's distances)
    let on_channel: Vec<u8> = seeds.iter().map(|&s| u8::from(s > 0)).collect();
    let dist_all = ffi::hydro_flow_distance(&d, &on_channel, niy, nix, dx, dy)?;
    let mut outlet_cells = vec![0u8; ny * nx];
    for l in &links[1..] {
        outlet_cells[l.end_row as usize * nx + l.end_col as usize] = 1;
    }
    let dist_st_all = ffi::hydro_flow_distance(&d, &outlet_cells, niy, nix, dx, dy)?;
    let dist_ch: Vec<f64> = flat.iter().map(|&i| dist_all[i]).collect();
    let dist_st: Vec<f64> = flat.iter().map(|&i| dist_st_all[i]).collect();

    // LSU and HRU data (BasinData.addCell over the cells in raster order)
    let mut lsus: Vec<Lsu> = (0..=n).map(|_| Lsu::new()).collect();
    let m = n + 1;
    let mut l_count = vec![0i64; m];
    let (mut l_x, mut l_y, mut l_slope, mut l_elev) = (vec![0.0; m], vec![0.0; m], vec![0.0; m], vec![0.0; m]);
    let mut sub_far = vec![0.0; m];
    let mut sub_min = vec![f64::INFINITY; m];
    let mut sub_max = vec![f64::NEG_INFINITY; m];
    // the farthest cell from the channel (the first in raster order of the largest distances)
    let mut first: Vec<Option<usize>> = vec![None; m];
    for (c, &l) in lsu.iter().enumerate() {
        l_count[l] += 1;
        l_x[l] += xs[c];
        l_y[l] += ys[c];
        l_slope[l] += slope[c];
        l_elev[l] += elev[c];
        sub_far[l] = np_max(sub_far[l], dist_st[c]);
        sub_min[l] = if elev[c] < sub_min[l] { elev[c] } else { sub_min[l] };
        sub_max[l] = if elev[c] > sub_max[l] { elev[c] } else { sub_max[l] };
        first[l] = match first[l] {
            None => Some(c),
            Some(f) => {
                let (cur, new) = (dist_ch[f], dist_ch[c]);
                // NaN last, then the largest, then the first
                if (cur.is_nan() && !new.is_nan()) || new > cur { Some(c) } else { Some(f) }
            }
        };
    }
    for c in 1..m {
        let l = &mut lsus[c];
        l.count = l_count[c];
        l.area = l.count as f64 * cell_area;
        l.css_area = l.area;
        l.x = l_x[c];
        l.y = l_y[c];
        l.slope = l_slope[c];
        l.elev = l_elev[c];
        if let Some(f) = first[c] {
            l.far_distance = dist_ch[f];
            l.far_elevation = elev[f];
        }
    }
    // the areas of the original land uses, soils and slope bands, in order of appearance
    let mut counts: Vec<(Om<i64>, Om<i64>, Om<i64>)> = (0..m).map(|_| (Om::new(), Om::new(), Om::new())).collect();
    for c in 0..m_cells {
        let k = &mut counts[lsu[c]];
        *k.0.entry(crop[c] as i64, || 0) += 1;
        *k.1.entry(soil[c] as i64, || 0) += 1;
        *k.2.entry(band[c], || 0) += 1;
    }
    for c in 1..m {
        let (a, b, s) = &counts[c];
        let area = |o: &Om<i64>| Om(o.0.iter().map(|(k, n)| (*k, *n as f64 * cell_area)).collect::<Vec<_>>());
        lsus[c].orig_crop = area(a);
        lsus[c].orig_soil = area(b);
        lsus[c].orig_slope = area(s);
    }
    // the combinations of land use, soil and slope band in each unit, in order of appearance
    struct Group {
        lsu: usize,
        crop: i64,
        soil: i64,
        band: i64,
        count: i64,
        x: f64,
        y: f64,
        slope: f64,
        elev: f64,
    }
    let mut groups: Vec<Group> = Vec::new();
    let mut group_of: HashMap<(usize, usize, usize, i64), usize> = HashMap::new();
    let mut cell_group: Vec<usize> = Vec::with_capacity(m_cells);
    for c in 0..m_cells {
        let key = (lsu[c], crop[c], soil[c], band[c]);
        let g = *group_of.entry(key).or_insert_with(|| {
            groups.push(Group {
                lsu: lsu[c],
                crop: crop[c] as i64,
                soil: soil[c] as i64,
                band: band[c],
                count: 0,
                x: 0.0,
                y: 0.0,
                slope: 0.0,
                elev: 0.0,
            });
            groups.len() - 1
        });
        let gr = &mut groups[g];
        gr.count += 1;
        gr.x += xs[c];
        gr.y += ys[c];
        gr.slope += slope[c];
        gr.elev += elev[c];
        cell_group.push(g);
    }
    for g in &groups {
        let l = &mut lsus[g.lsu];
        let h = l.hru.len() + 1;
        l.numbers.entry(g.crop, Om::new).entry(g.soil, Om::new).entry(g.band, || h);
        l.hru.insert(
            h,
            Cell { count: g.count, area: g.count as f64 * cell_area, elev: g.elev, slope: g.slope, x: g.x, y: g.y },
        );
    }
    let exempt_ids: Vec<i64> =
        set.exempt.iter().filter_map(|e| crops.iter().position(|c| c == e)).map(|k| k as i64).collect();
    for l in lsus.iter_mut().skip(1) {
        select(l, set, &exempt_ids)?;
    }

    let to_ll = terraflux_gdal::Transform::new(&crs, "EPSG:4326").map_err(Error::msg)?;
    let lonlat = |x: f64, y: f64| -> (f64, f64) {
        let (mut px, mut py) = ([x], [y]);
        to_ll.points(&mut px, &mut py);
        (py[0], px[0])
    };

    // gis tables (DBUtils / hrus.py), subbasins in SWAT+ order
    let (mut sub_id, mut sub_area, mut slo1, mut len1, mut sll) = (vec![], vec![], vec![], vec![], vec![]);
    let (mut sub_lat, mut sub_lon, mut sub_elev, mut sub_emin, mut sub_emax) = (vec![], vec![], vec![], vec![], vec![]);
    let (mut ch_areac, mut ch_strahler, mut ch_len2, mut ch_slo2, mut ch_wid2, mut ch_dep2) =
        (vec![], vec![], vec![], vec![], vec![], vec![]);
    let (mut ch_emin, mut ch_emax, mut ch_midlat, mut ch_midlon) = (vec![], vec![], vec![], vec![]);
    let (mut lsu_id, mut lsu_slope, mut lsu_len1, mut lsu_csl, mut lsu_wid1, mut lsu_dep1) =
        (vec![], vec![], vec![], vec![], vec![], vec![]);
    #[derive(Default)]
    struct Hrus {
        id: Vec<f64>,
        lsu: Vec<f64>,
        arsub: Vec<f64>,
        landuse: Vec<Option<String>>,
        arland: Vec<f64>,
        soil: Vec<Option<String>>,
        arso: Vec<f64>,
        slp: Vec<Option<String>>,
        arslp: Vec<f64>,
        slope: Vec<f64>,
        lat: Vec<f64>,
        lon: Vec<f64>,
        elev: Vec<f64>,
    }
    let mut hr = Hrus::default();
    let mut routing: Vec<(f64, &str, &str, f64, &str, f64)> = Vec::new();
    let mut lsu_hru_map: HashMap<(usize, i64, i64, i64), usize> = HashMap::new();
    let mut hru_id = 0usize;
    let area_ha_of = |l: &Lsu| l.area / 1e4;
    for c in 1..=n {
        let l = &lsus[c];
        let area_ha = area_ha_of(l);
        let mean_slope = if l.count == 0 { 0.0 } else { l.slope / l.count as f64 };
        let (lat, lon) = lonlat(l.x / l.count as f64, l.y / l.count as f64);
        let mean_elev = l.elev / l.count as f64;
        sub_id.push(c as f64);
        sub_area.push(area_ha);
        slo1.push(mean_slope * 100.0);
        len1.push(sub_far[c]);
        sll.push(sl_subbsn(mean_slope));
        sub_lat.push(lat);
        sub_lon.push(lon);
        sub_elev.push(mean_elev);
        sub_emin.push(sub_min[c]);
        sub_emax.push(sub_max[c]);
        let r = &links[c];
        let (er, ec) = (r.end_row as usize, r.end_col as usize);
        let drain_ha = acc[er * nx + ec] * cell_area / 1e4;
        let drain_km = drain_ha / 100.0;
        let length = r.length;
        let drop = r.z_start - r.z_end;
        let slope_pct = if length <= 0.0 { 0.0 } else { drop / length * 100.0 };
        let x0 = xmin + (r.start_col + 0.5) * rx;
        let y0 = ymax - (r.start_row + 0.5) * ry;
        let j = nxt[er * nx + ec];
        let (er1, ec1) = if j >= 0 { (j as usize / nx, j as usize % nx) } else { (er, ec) };
        let x1 = xmin + (ec1 as f64 + 0.5) * rx;
        let y1 = ymax - (er1 as f64 + 0.5) * ry;
        let (mlat, mlon) = lonlat((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        ch_areac.push(drain_ha);
        ch_strahler.push(r.order);
        ch_len2.push(length);
        ch_slo2.push(slope_pct);
        ch_wid2.push(WIDTH_MULTIPLIER * drain_km.powf(WIDTH_EXPONENT));
        ch_dep2.push(DEPTH_MULTIPLIER * drain_km.powf(DEPTH_EXPONENT));
        ch_emin.push(r.z_end);
        ch_emax.push(r.z_start);
        ch_midlat.push(mlat);
        ch_midlon.push(mlon);
        // the landscape unit
        let id = (c * 10) as f64;
        let source_elev = r.z_start;
        let trib_distance = l.far_distance;
        let trib_drop = (l.far_elevation - source_elev).abs();
        let area_km = area_ha / 100.0;
        lsu_id.push(id);
        lsu_slope.push(mean_slope * 100.0);
        lsu_len1.push(trib_distance);
        lsu_csl.push(if trib_distance < 1.0 { 0.0 } else { trib_drop / trib_distance * 100.0 });
        lsu_wid1.push(WIDTH_MULTIPLIER * area_km.powf(WIDTH_EXPONENT));
        lsu_dep1.push(DEPTH_MULTIPLIER * area_km.powf(DEPTH_EXPONENT));
        // HRUs (printLSUHRUs), in the order QSWAT+ keeps them
        for (cr, ss) in l.numbers.iter() {
            let arland = l.crop_area(cr) / 1e4;
            for (sl, slopes) in ss.iter() {
                let arso = l.soil_area(cr, sl) / 1e4;
                for (bd, &h) in slopes.iter() {
                    hru_id += 1;
                    let cd = &l.hru[&h];
                    let mult = cell_area / cd.area;
                    let (hlat, hlon) = lonlat(cd.x * mult, cd.y * mult);
                    let mut luse = crops[cr as usize].clone();
                    if luse.to_uppercase() == "WATR" {
                        luse = "WETW".into();
                    }
                    hr.id.push(hru_id as f64);
                    hr.lsu.push(id);
                    hr.arsub.push(area_ha);
                    hr.landuse.push((luse.to_uppercase() != "BARR").then_some(luse));
                    hr.arland.push(arland);
                    hr.soil.push(Some(soil_list[sl as usize].clone()));
                    hr.arso.push(arso);
                    hr.slp.push(Some(slope_range(bd, limits)));
                    hr.arslp.push(cd.area / 1e4);
                    hr.slope.push(cd.slope * mult * 100.0);
                    hr.lat.push(hlat);
                    hr.lon.push(hlon);
                    hr.elev.push(cd.elev * mult);
                    lsu_hru_map.insert((c, cr, sl, bd), hru_id);
                    routing.push((hru_id as f64, "HRU", "tot", c as f64, "CH", 100.0));
                }
            }
        }
    }
    // the HRU of each cell (removed combinations: NaN)
    let group_hru: Vec<f64> = groups
        .iter()
        .map(|g| lsu_hru_map.get(&(g.lsu, g.crop, g.soil, g.band)).map_or(f64::NAN, |&h| h as f64))
        .collect();
    let mut hru_raster = vec![f64::NAN; ny * nx];
    for (c, &i) in flat.iter().enumerate() {
        hru_raster[i] = group_hru[cell_group[c]];
    }

    // outlets of the subbasins, the watershed outlets and the routing
    let (mut pt_xpr, mut pt_ypr, mut pt_lat, mut pt_lon, mut pt_elev) = (vec![], vec![], vec![], vec![], vec![]);
    for c in 1..=n {
        let r = &links[c];
        let (er, ec) = (r.end_row as usize, r.end_col as usize);
        let x = xmin + (ec as f64 + 0.5) * rx;
        let y = ymax - (er as f64 + 0.5) * ry;
        let (plat, plon) = lonlat(x, y);
        pt_xpr.push(x);
        pt_ypr.push(y);
        pt_lat.push(plat);
        pt_lon.push(plon);
        pt_elev.push(z[er * nx + ec]);
    }
    for c in 1..=n {
        let ds = links[c].down;
        routing.push((c as f64, "CH", "tot", c as f64, "PT", 100.0));
        routing.push(if ds > 0 {
            (c as f64, "PT", "tot", ds as f64, "CH", 100.0)
        } else {
            (c as f64, "PT", "tot", 0.0, "X", 100.0)
        });
    }
    for c in 1..=n {
        routing.push((c as f64, "SUB", "tot", c as f64, "PT", 100.0));
    }
    for c in 1..=n {
        routing.push(((c * 10) as f64, "LSU", "tot", c as f64, "CH", 100.0));
        routing.push(((c * 10) as f64, "LSU", "rhg", (c * 10) as f64, "AQU", 100.0));
    }
    // aquifers: one shallow per subbasin, one deep per watershed outlet
    let finals = |mut c: usize| {
        while links[c].down > 0 {
            c = links[c].down;
        }
        c
    };
    let (mut aq_id, mut aq_out, mut aq_area, mut aq_lat, mut aq_lon, mut aq_elev) =
        (vec![], vec![], vec![], vec![], vec![], vec![]);
    let mut deep: Vec<(usize, [f64; 4])> = Vec::new();
    for c in 1..=n {
        let l = &lsus[c];
        let out = finals(c);
        let (cx, cy) = (l.x / l.count as f64, l.y / l.count as f64);
        let row = (((ymax - cy) / ry) as i64).max(0).min(ny as i64 - 1) as usize;
        let col = (((cx - xmin) / rx) as i64).max(0).min(nx as i64 - 1) as usize;
        let ez = z[row * nx + col];
        let ez = if ez.is_nan() { 0.0 } else { ez };
        let (lat, lon) = lonlat(cx, cy);
        aq_id.push((c * 10) as f64);
        aq_out.push(out as f64);
        aq_area.push(l.area / 1e4);
        aq_lat.push(lat);
        aq_lon.push(lon);
        aq_elev.push(ez);
        let slot = match deep.iter().position(|(o, _)| *o == out) {
            Some(k) => k,
            None => {
                deep.push((out, [0.0; 4]));
                deep.len() - 1
            }
        };
        let t = &mut deep[slot].1;
        *t = [t[0] + l.area, t[1] + ez * l.area, t[2] + cx * l.area, t[3] + cy * l.area];
        routing.push(((c * 10) as f64, "AQU", "tot", c as f64, "PT", 100.0));
        routing.push(((c * 10) as f64, "AQU", "rhg", out as f64, "DAQ", 100.0));
    }
    let (mut dq_id, mut dq_area, mut dq_lat, mut dq_lon, mut dq_elev) = (vec![], vec![], vec![], vec![], vec![]);
    for (out, [a, em, xm, ym]) in &deep {
        let (lat, lon) = lonlat(xm / a, ym / a);
        dq_id.push(*out as f64);
        dq_area.push(a / 1e4);
        dq_lat.push(lat);
        dq_lon.push(lon);
        dq_elev.push(em / a);
        routing.push((*out as f64, "DAQ", "tot", *out as f64, "PT", 100.0));
    }

    let mut used: Vec<usize> = soil.clone();
    used.sort_unstable();
    used.dedup();
    let used_names: Vec<String> = used.iter().map(|&k| soil_list[k].clone()).collect();
    let (soils_sol, soils_layer) = soil_tables(soil_table, &used_names)?;
    let ids = |n: usize| Col::Num((1..=n).map(|i| i as f64).collect());
    let zeros = |n: usize| Col::Num(vec![0.0; n]);
    let subs = frame(vec![
        ("id", ids(n)),
        ("area", Col::Num(sub_area.clone())),
        ("slo1", Col::Num(slo1)),
        ("len1", Col::Num(len1)),
        ("sll", Col::Num(sll)),
        ("lat", Col::Num(sub_lat.clone())),
        ("lon", Col::Num(sub_lon.clone())),
        ("elev", Col::Num(sub_elev.clone())),
        ("elevmin", Col::Num(sub_emin)),
        ("elevmax", Col::Num(sub_emax)),
        ("waterid", zeros(n)),
    ]);
    let chans = frame(vec![
        ("id", ids(n)),
        ("subbasin", ids(n)),
        ("areac", Col::Num(ch_areac)),
        ("strahler", Col::Num(ch_strahler)),
        ("len2", Col::Num(ch_len2)),
        ("slo2", Col::Num(ch_slo2)),
        ("wid2", Col::Num(ch_wid2)),
        ("dep2", Col::Num(ch_dep2)),
        ("elevmin", Col::Num(ch_emin)),
        ("elevmax", Col::Num(ch_emax)),
        ("midlat", Col::Num(ch_midlat)),
        ("midlon", Col::Num(ch_midlon)),
    ]);
    let lsu_table = frame(vec![
        ("id", Col::Num(lsu_id)),
        ("category", zeros(n)),
        ("channel", ids(n)),
        ("area", Col::Num(sub_area.clone())),
        ("slope", Col::Num(lsu_slope)),
        ("len1", Col::Num(lsu_len1)),
        ("csl", Col::Num(lsu_csl)),
        ("wid1", Col::Num(lsu_wid1)),
        ("dep1", Col::Num(lsu_dep1)),
        ("lat", Col::Num(sub_lat)),
        ("lon", Col::Num(sub_lon)),
        ("elev", Col::Num(sub_elev)),
    ]);
    let hrus_table = frame(vec![
        ("id", Col::Num(hr.id)),
        ("lsu", Col::Num(hr.lsu)),
        ("arsub", Col::Num(hr.arsub.clone())),
        ("arlsu", Col::Num(hr.arsub)),
        ("landuse", Col::Text(hr.landuse)),
        ("arland", Col::Num(hr.arland)),
        ("soil", Col::Text(hr.soil)),
        ("arso", Col::Num(hr.arso)),
        ("slp", Col::Text(hr.slp)),
        ("arslp", Col::Num(hr.arslp)),
        ("slope", Col::Num(hr.slope)),
        ("lat", Col::Num(hr.lat)),
        ("lon", Col::Num(hr.lon)),
        ("elev", Col::Num(hr.elev)),
    ]);
    let aquifers = frame(vec![
        ("id", Col::Num(aq_id)),
        ("category", zeros(n)),
        ("subbasin", ids(n)),
        ("deep_aquifer", Col::Num(aq_out)),
        ("area", Col::Num(aq_area)),
        ("lat", Col::Num(aq_lat)),
        ("lon", Col::Num(aq_lon)),
        ("elev", Col::Num(aq_elev)),
    ]);
    let deep_table = frame(vec![
        ("id", Col::Num(dq_id.clone())),
        ("subbasin", Col::Num(dq_id)),
        ("area", Col::Num(dq_area)),
        ("lat", Col::Num(dq_lat)),
        ("lon", Col::Num(dq_lon)),
        ("elev", Col::Num(dq_elev)),
    ]);
    let points = frame(vec![
        ("id", ids(n)),
        ("subbasin", ids(n)),
        ("ptype", text(&vec!["O"; n])),
        ("xpr", Col::Num(pt_xpr)),
        ("ypr", Col::Num(pt_ypr)),
        ("lat", Col::Num(pt_lat)),
        ("lon", Col::Num(pt_lon)),
        ("elev", Col::Num(pt_elev)),
    ]);
    let routing_table = frame(vec![
        ("sourceid", Col::Num(routing.iter().map(|r| r.0).collect())),
        ("sourcecat", text(&routing.iter().map(|r| r.1).collect::<Vec<_>>())),
        ("hyd_typ", text(&routing.iter().map(|r| r.2).collect::<Vec<_>>())),
        ("sinkid", Col::Num(routing.iter().map(|r| r.3).collect())),
        ("sinkcat", text(&routing.iter().map(|r| r.4).collect::<Vec<_>>())),
        ("percent", Col::Num(routing.iter().map(|r| r.5).collect())),
    ]);
    let tables: Vec<(String, Table)> = TABLES
        .iter()
        .zip([subs, chans, lsu_table, hrus_table, aquifers, deep_table, points, routing_table, soils_sol, soils_layer])
        .map(|(n, t)| (n.to_string(), t))
        .collect();
    let sub_r: Vec<f64> = basin.iter().zip(&valid).map(|(&b, &v)| if v { f64::from(b) } else { f64::NAN }).collect();
    let ch_r: Vec<f64> = seeds.iter().zip(&valid).map(|(&s, &v)| if v { f64::from(s) } else { f64::NAN }).collect();
    let grid = dem.grid.clone();
    Ok(Watershed {
        tables,
        subbasins: Map::new(grid.clone(), sub_r),
        hrus: Map::new(grid.clone(), hru_raster),
        channels: Map::new(grid, ch_r),
        notes: Vec::new(),
    })
}

// ----------------------------------------------------------------------------------------------- operator

struct WatershedOp;

fn table_in(args: &Args, port: &str) -> Result<Table> {
    match args.get(port) {
        Some(Value::Table(t)) => Ok(t.clone()),
        Some(v) => Err(Error::msg(format!("{port} is a {:?}, not a table", v.ty()))),
        None => Err(Error::msg(format!("connect the table {port}"))),
    }
}

impl Operator for WatershedOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "swat.watershed",
            "Divide a watershed into subbasins, channels and HRUs as QSWAT+ does (landschaft's watershed, bit for \
             bit): the DEM is filled and drains along D8 directions, every cell whose drainage area reaches the \
             stream threshold is a stream, each stream link (split at the outlets) is a channel with its subbasin and \
             one landscape unit; HRUs are the combinations of land use, soil and slope band in a landscape unit, \
             selected by the method chosen (QSWAT+'s: one HRU per unit, or every combination above thresholds or \
             above an area; removed HRUs give their area to the others). Gives QSWAT+'s project tables as a value \
             (the SWAT+ Editor's import) and the maps of the subbasins, channels and HRUs; channel width and depth \
             come from the drainage area (1.29 A^0.6, 0.13 A^0.4, A in km2).",
            Access::Global,
        )
        .input(PortSpec::new("dem", Type::Map, "the elevations (m), in a projected CRS (or longitude / latitude)"))
        .input(PortSpec::new(
            "landuse",
            Type::Map,
            "the land use codes, put on the DEM's cells (nearest neighbour) when its grid differs",
        ))
        .input(PortSpec::new("soils", Type::Map, "the soil codes, likewise"))
        .input(PortSpec::new(
            "landuse_lookup",
            Type::Table,
            "land use code -> SWAT+ plant or urban name (plants.plt / urban.urb, e.g. FRSD, PAST): two columns, the \
             raster value and the name; codes with the same name make one land use; missing cells take QSWAT+'s \
             default land use (code 0 if listed, else the first listed)",
        ))
        .input(PortSpec::new("soil_lookup", Type::Table, "soil code -> soil name (SNAM of the soil table), likewise"))
        .input(PortSpec::new(
            "soil_table",
            Type::Table,
            "the soils: an ArcSWAT / QSWAT+ usersoil table (SNAM, NLAYERS, HYDGRP, SOL_ZMX, ..., SOL_Z1..) or one \
             row per layer in SWAT+'s soils.sol columns (name, hyd_grp, dp_tot, anion_excl, perc_crk, texture, dp, \
             bd, awc, soil_k, carbon, clay, silt, sand, rock, alb, usle_k, ec, caco3, ph); only the soils used are kept",
        ))
        .input(PortSpec::optional(
            "outlets",
            Type::Vector,
            "watershed outlets (points), each moved onto the nearest stream cell within snap; without them every \
             stream that leaves the map (or ends in a pit) is an outlet",
        ))
        .output(PortSpec::new("watershed", Type::SwatWatershed, "the watershed: QSWAT+'s tables and the grids"))
        .output(PortSpec::new("subbasins", Type::Map, "the subbasin of each cell (0 outside the watershed)"))
        .output(PortSpec::new("channels", Type::Map, "the channel of each stream cell (0 elsewhere)"))
        .output(PortSpec::new(
            "hrus",
            Type::Map,
            "the HRU of each cell (no value where its HRU was removed by the selection, and outside)",
        ))
        .output(PortSpec::new("table", Type::Table, "the HRUs (QSWAT+'s gis_hrus)"))
        .param(PortSpec::optional(
            "stream_threshold",
            Type::Number,
            "the drainage area of a stream, in km2 (default: 1 % of the DEM's area, as QSWAT+ proposes)",
        ))
        .param(PortSpec::optional(
            "snap",
            Type::Number,
            "the largest move of an outlet, map units (300 m; 3 cells without a projection)",
        ))
        .param(PortSpec::optional(
            "slope_bands",
            Type::Any,
            "the slope limits in percent between the bands, a list, e.g. 5, 15 (default: one band, 0-9999)",
        ))
        .param(
            PortSpec::optional(
                "hru_method",
                Type::Text,
                "dominant_hru (default, QSWAT+'s: one HRU per landscape unit, the largest combination), dominant \
                 (the dominant land use, soil and slope), filter (every combination above thresholds) or area \
                 (above min_area)",
            )
            .choices(METHODS),
        )
        .param(PortSpec::optional(
            "thresholds",
            Type::Any,
            "of filter: the smallest land use, soil and slope in percent, a list of three (default 0, 0, 0)",
        ))
        .param(PortSpec::optional(
            "min_area",
            Type::Number,
            "of area: the smallest HRU, in ha (or percent with area_in_ha off)",
        ))
        .param(PortSpec::optional("area_in_ha", Type::Bool, "min_area is in hectares (default true), else percent"))
        .param(PortSpec::optional("exempt", Type::Any, "land use names the filters never remove, a list"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        (|| -> Result<Outputs> {
            let dem = args.map("dem")?;
            let (landuse, soils) = (args.map("landuse")?, args.map("soils")?);
            let thresholds: Vec<f64> = args.opt_param("thresholds")?.unwrap_or_else(|| vec![0.0; 3]);
            let [a, b, c] = thresholds.as_slice() else {
                return Err(Error::msg("thresholds are three numbers: land use, soil and slope, in percent"));
            };
            let set = Settings {
                stream_threshold: args.opt_param("stream_threshold")?,
                snap: args.opt_param("snap")?,
                slope_bands: args.opt_param("slope_bands")?.unwrap_or_default(),
                hru_method: args.param("hru_method", "dominant_hru".to_string())?,
                thresholds: [*a, *b, *c],
                min_area: args.param("min_area", 0.0)?,
                area_in_ha: args.param("area_in_ha", true)?,
                exempt: args.opt_param("exempt")?.unwrap_or_default(),
            };
            let outlets = match args.get("outlets") {
                Some(_) => Some(outlet_points(args, &dem.grid)?),
                None => None,
            };
            let w = delineate(
                dem,
                landuse,
                soils,
                &table_in(args, "landuse_lookup")?,
                &table_in(args, "soil_lookup")?,
                &table_in(args, "soil_table")?,
                outlets.as_deref(),
                &set,
            )?;
            let table = w.table("gis_hrus").cloned().unwrap_or_default();
            let (sub, ch, hru) = (w.subbasins.clone(), w.channels.clone(), w.hrus.clone());
            Ok(outputs([
                ("watershed", Value::SwatWatershed(Arc::new(w))),
                ("subbasins", Value::Map(sub)),
                ("channels", Value::Map(ch)),
                ("hrus", Value::Map(hru)),
                ("table", Value::Table(table)),
            ]))
        })()
        .map_err(fail)
    }
}
