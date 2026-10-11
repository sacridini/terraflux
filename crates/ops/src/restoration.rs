//! The inputs of restoration planning after plangea, from landschaft's C++ (the same bits as plangea's R):
//! ecological intactness (Beyer et al. 2020) from the human footprint and its gradient as cells are restored
//! (`restoration.intactness`, `restoration.intactness_gradient`), the original vegetation of converted land
//! (`restoration.original_vegetation`) and the area of habitat of species (Brooks et al. 2019;
//! `sdm.area_of_habitat`).
//!
//! Land covers are stacks of the share (0-1) of each class in every cell, by class name.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;

use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Intactness { gradient: false }));
    r.add(Arc::new(Intactness { gradient: true }));
    r.add(Arc::new(OriginalVegetation));
    r.add(Arc::new(AreaOfHabitat));
}

/// Beyer et al.'s gamma: a footprint of 4 leaves a fifth of the habitat's quality.
pub fn intactness_gamma() -> f64 {
    -(0.2f64.ln()) / 4.0
}

/// A cell's width and height in km: the setting `cell_size` (a size or [width, height]), else the grid's in a
/// projected CRS.
fn cell_km(args: &Args, grid: &Grid) -> Result<(f64, f64)> {
    match args.params.get("cell_size") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Number(n)) => {
            let s = n.as_f64().unwrap();
            return Ok((s, s));
        }
        Some(v) => {
            let [w, h]: [f64; 2] = serde_json::from_value(v.clone())
                .map_err(|e| Error::msg(format!("node {:?}: cell_size: {e} (km, or [width, height])", args.node)))?;
            return Ok((w, h));
        }
    }
    match grid.geo.is_geographic() {
        Some(false) => {
            let u = grid.geo.linear_unit();
            Ok((grid.transform[1].abs() * u / 1000.0, grid.transform[5].abs() * u / 1000.0))
        }
        Some(true) => {
            Err(Error::msg(format!("node {:?}: the map is in degrees: project it, or give cell_size (km)", args.node)))
        }
        None => Err(Error::msg(format!("node {:?}: the map has no coordinate system: give cell_size (km)", args.node))),
    }
}

/// A map input of the same grid as `like`, or none.
fn same_grid<'a>(args: &'a Args, port: &str, like: &Map) -> Result<Option<&'a Map>> {
    let m = args.opt_map(port)?;
    if let Some(m) = m
        && m.shape() != like.shape()
    {
        return Err(Error::msg(format!("node {:?}: {port} is not on the footprint's grid", args.node)));
    }
    Ok(m)
}

struct Intactness {
    gradient: bool,
}

impl Operator for Intactness {
    fn signature(&self) -> OpSignature {
        let sig = if self.gradient {
            OpSignature::new(
                "restoration.intactness_gradient",
                "How much the total ecological intactness grows per unit of each cell restored (its derivative), \
                 as plangea: restoring a cell lowers its footprint by the reduction given per unit. With restored \
                 shares, at the footprint they leave.",
                Access::Global,
            )
            .input(PortSpec::new("footprint", Type::Map, "the human footprint"))
            .input(PortSpec::new("reduction", Type::Map, "the footprint a cell loses per unit restored"))
            .input(PortSpec::optional("restored", Type::Map, "the share of each cell restored already"))
            .output(PortSpec::new(
                "map",
                Type::Map,
                "d(total intactness) / d(share restored) of each cell",
            ))
        } else {
            OpSignature::new(
                "restoration.intactness",
                "Ecological intactness (Beyer et al. 2020), as plangea computes it: the habitat's quality \
                 q = exp(-gamma footprint) of a cell and of those around it (within radius cells, weighted by \
                 exp(beta distance in km)), Q = mean of (q q')^z. With a reduction and restored shares, the \
                 intactness after restoring them.",
                Access::Global,
            )
            .input(PortSpec::new("footprint", Type::Map, "the human footprint"))
            .input(PortSpec::optional("reduction", Type::Map, "the footprint a cell loses per unit restored"))
            .input(PortSpec::optional("restored", Type::Map, "the share of each cell restored"))
            .output(PortSpec::new("map", Type::Map, "the intactness of each cell (0-1)"))
        };
        sig.param(PortSpec::optional("radius", Type::Integer, "the neighbourhood's radius in cells (default 3)"))
            .param(PortSpec::optional("gamma", Type::Number, "the footprint's decay of quality (default -ln(0.2)/4)"))
            .param(PortSpec::optional("beta", Type::Number, "the distance's decay per km (default -0.2)"))
            .param(PortSpec::optional("z", Type::Number, "the exponent of the pairs' quality (default 0.5)"))
            .param(PortSpec::optional(
                "cell_size",
                Type::Any,
                "a cell's size in km, or [width, height] (default: the grid's, projected)",
            ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let hfi = args.map("footprint")?;
        let radius: i64 = args.param("radius", 3)?;
        let radius = i32::try_from(radius)
            .ok()
            .filter(|r| *r >= 0)
            .ok_or_else(|| Error::msg(format!("node {:?}: the radius is a number of cells, 0 or more", args.node)))?;
        let gamma: f64 = args.param("gamma", intactness_gamma())?;
        let beta: f64 = args.param("beta", -0.2)?;
        let z: f64 = args.param("z", 0.5)?;
        let reduction = same_grid(args, "reduction", hfi)?;
        let restored = same_grid(args, "restored", hfi)?;
        let (dx, dy) = cell_km(args, &hfi.grid)?;
        let (ny, nx) = hfi.shape();
        let (ny, nx) = (ny as i64, nx as i64);
        let mut h: Vec<f64> = hfi.data.to_vec();
        if let (Some(a), Some(x)) = (reduction, restored) {
            h = ffi::rest_footprint(&h, &a.data, &x.data, ny, nx)?;
        }
        let out = if self.gradient {
            let a = reduction.ok_or_else(|| Error::msg(format!("node {:?}: no reduction", args.node)))?;
            ffi::rest_intactness_gradient(&h, &a.data, ny, nx, gamma, beta, z, radius, dx, dy)?
        } else {
            ffi::rest_intactness(&h, ny, nx, gamma, beta, z, radius, dx, dy)?
        };
        Ok(outputs([("map", Value::Map(Map::new(hfi.grid.clone(), out)))]))
    }
}

/// A list of class names from a setting (a name or a list).
fn names(args: &Args, name: &str) -> Result<Option<Vec<String>>> {
    match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(vec![s.clone()])),
        Some(v) => {
            let list: Vec<serde_json::Value> = serde_json::from_value(v.clone())
                .map_err(|e| Error::msg(format!("node {:?}: {name}: {e} (a class or a list)", args.node)))?;
            Ok(Some(
                list.iter()
                    .map(|x| match x {
                        serde_json::Value::String(s) => s.clone(),
                        x => x.to_string(),
                    })
                    .collect(),
            ))
        }
    }
}

/// The layers of a stack named `wanted`, in that order, k x n (no value: 0).
fn layers(args: &Args, port: &str, stack: &Stack, wanted: &[String]) -> Result<Vec<f64>> {
    let mut out = Vec::with_capacity(wanted.len() * stack.grid.width * stack.grid.height);
    for w in wanted {
        let i = stack.keys.iter().position(|k| k.to_string() == *w).ok_or_else(|| {
            Error::msg(format!(
                "node {:?}: {port} has no layer {w:?} (its layers: {})",
                args.node,
                stack.keys.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(", ")
            ))
        })?;
        out.extend(stack.layers[i].iter().map(|v| if v.is_nan() { 0.0 } else { *v }));
    }
    Ok(out)
}

/// A stack of `k` layers named `names` from k x n values.
fn stack_of(grid: &Arc<Grid>, names: &[String], values: Vec<f64>) -> Value {
    let n = grid.width * grid.height;
    Value::Stack(Stack {
        grid: grid.clone(),
        keys: names.iter().map(|c| Key::Name(c.clone())).collect(),
        layers: values.chunks(n.max(1)).map(|c| Arc::new(c.to_vec())).collect(),
    })
}

struct OriginalVegetation;

impl Operator for OriginalVegetation {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "restoration.original_vegetation",
            "The original vegetation of converted land, as plangea (harmonize_oa): for every cell with \
             anthropic land, the shares of the natural classes it had, from the past cover where it tells, \
             else from the ecoregion's natural shares, else from failsafe shares (the mean over the map); and \
             the shares restored land returns to. The land cover is a stack of each class's share (0-1).",
            Access::Global,
        )
        .input(PortSpec::new("land_cover", Type::Stack, "the share of each class, by name"))
        .input(PortSpec::optional("past", Type::Stack, "the past share of each natural class"))
        .input(PortSpec::optional("ecoregions", Type::Stack, "the ecoregion's share of each natural class"))
        .output(PortSpec::new("original", Type::Stack, "the original share of each natural class"))
        .output(PortSpec::new("restore", Type::Stack, "the shares of the natural classes restored land returns to"))
        .output(PortSpec::new("source", Type::Map, "where the original came from: 0 past, 1 ecoregion, 2 failsafe"))
        .output(PortSpec::new("anthropic", Type::Map, "the share of anthropic land"))
        .output(PortSpec::new("failsafe", Type::Table, "the failsafe share of each class filled (in natural's order)"))
        .param(PortSpec::new("natural", Type::Any, "the natural classes (names of the land cover's layers)"))
        .param(PortSpec::new("anthropic", Type::Any, "the anthropic classes"))
        .param(PortSpec::optional("failsafe", Type::Any, "the natural classes the failsafe fills (default all)"))
        .param(PortSpec::optional("threshold", Type::Number, "the share of land a source must explain (default 0.95)"))
        .param(PortSpec::optional(
            "restore_to",
            Type::Text,
            "original (default) or current: restored land returns to the current natural cover where it has some",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let cover = args.stack("land_cover")?;
        let natural =
            names(args, "natural")?.ok_or_else(|| Error::msg(format!("node {node:?}: no natural classes")))?;
        let anthropic =
            names(args, "anthropic")?.ok_or_else(|| Error::msg(format!("node {node:?}: no anthropic classes")))?;
        if natural.is_empty() || anthropic.is_empty() {
            return Err(Error::msg(format!("node {node:?}: at least one natural and one anthropic class are needed")));
        }
        if let Some(c) = natural.iter().find(|c| anthropic.contains(c)) {
            return Err(Error::msg(format!("node {node:?}: {c:?} is both natural and anthropic")));
        }
        let restore_to: String = args.param("restore_to", "original".to_string())?;
        if restore_to != "original" && restore_to != "current" {
            return Err(Error::msg(format!("node {node:?}: restore_to is original or current, not {restore_to:?}")));
        }
        let threshold: f64 = args.param("threshold", 0.95)?;
        let failsafe = names(args, "failsafe")?.unwrap_or_else(|| natural.clone());
        if let Some(c) = failsafe.iter().find(|c| !natural.contains(c)) {
            return Err(Error::msg(format!("node {node:?}: failsafe: {c:?} is not a natural class")));
        }
        let ncell = cover.grid.width * cover.grid.height;
        let nat = layers(args, "land_cover", cover, &natural)?;
        let ant = layers(args, "land_cover", cover, &anthropic)?;
        // the cells with land: their natural and anthropic classes summed in the land cover's order
        let mut land = vec![0.0; ncell];
        let mut first = true;
        for (i, k) in cover.keys.iter().enumerate() {
            let name = k.to_string();
            if natural.contains(&name) || anthropic.contains(&name) {
                for (l, v) in land.iter_mut().zip(cover.layers[i].iter()) {
                    let v = if v.is_nan() { 0.0 } else { *v };
                    *l = if first { v } else { *l + v };
                }
                first = false;
            }
        }
        let cells: Vec<usize> = (0..ncell).filter(|&c| land[c] > 0.0).collect();
        let flat = |stack: &[f64], k: usize| -> Vec<f64> {
            (0..k).flat_map(|j| cells.iter().map(move |&c| stack[j * ncell + c])).collect()
        };
        let (k, m, n) = (natural.len(), anthropic.len(), cells.len());
        let optional = |port: &str| -> Result<Vec<f64>> {
            match args.opt_stack(port)? {
                None => Ok(Vec::new()),
                Some(s) => {
                    if s.grid.width != cover.grid.width || s.grid.height != cover.grid.height {
                        return Err(Error::msg(format!("node {node:?}: {port} is not on the land cover's grid")));
                    }
                    Ok(flat(&layers(args, port, s, &natural)?, k))
                }
            }
        };
        let (nat_f, ant_f) = (flat(&nat, k), flat(&ant, m));
        let mut a_total = ant_f[..n].to_vec();
        for j in 1..m {
            for (t, v) in a_total.iter_mut().zip(&ant_f[j * n..(j + 1) * n]) {
                *t += v;
            }
        }
        let domain: Vec<u8> =
            if restore_to == "current" { Vec::new() } else { a_total.iter().map(|&a| u8::from(a > 0.0)).collect() };
        let fill: Vec<u8> = natural.iter().map(|c| u8::from(failsafe.contains(c))).collect();
        let v = ffi::rest_original_vegetation(
            &nat_f,
            k as i32,
            &ant_f,
            m as i32,
            n as i64,
            &optional("past")?,
            &optional("ecoregions")?,
            &domain,
            &fill,
            threshold,
            restore_to == "current",
        )?;
        // back on the grid, no value off the land
        let grid_of = |values: &[f64], k: usize| -> Vec<f64> {
            let mut out = vec![f64::NAN; k * ncell];
            for j in 0..k {
                for (i, &c) in cells.iter().enumerate() {
                    out[j * ncell + c] = values[j * n + i];
                }
            }
            out
        };
        let source: Vec<f64> = v.source.iter().map(|&s| f64::from(s)).collect();
        let mut failsafe_t = Table::new(vec!["class".into(), "share".into()]);
        failsafe_t.rows = (0..k).filter(|&j| fill[j] == 1).map(|j| vec![(j + 1) as f64, v.fill[j]]).collect();
        Ok(outputs([
            ("original", stack_of(&cover.grid, &natural, grid_of(&v.original, k))),
            ("restore", stack_of(&cover.grid, &natural, grid_of(&v.restore, k))),
            ("source", Value::Map(Map::new(cover.grid.clone(), grid_of(&source, 1)))),
            ("anthropic", Value::Map(Map::new(cover.grid.clone(), grid_of(&a_total, 1)))),
            ("failsafe", Value::Table(failsafe_t)),
        ]))
    }
}

/// The area of every cell in km²: of a projected grid from its cells' sides, of a geographic one between its
/// parallels and meridians on the WGS84 ellipsoid.
pub fn cell_area_km2(grid: &Grid) -> Option<Vec<f64>> {
    let t = grid.transform;
    let (w, h) = (grid.width, grid.height);
    match grid.geo.is_geographic()? {
        false => {
            let u = grid.geo.linear_unit();
            Some(vec![(t[1] * t[5]).abs() * u * u / 1e6; w * h])
        }
        true => {
            let a = 6_378_137.0f64;
            let f: f64 = 1.0 / 298.257_223_563;
            let e2 = f * (2.0 - f);
            let e = e2.sqrt();
            let b2 = a * a * (1.0 - e2);
            // the area from the equator to a latitude, per radian of longitude
            let zone = |lat: f64| {
                let s = lat.to_radians().sin();
                b2 / 2.0 * (s / (1.0 - e2 * s * s) + ((1.0 + e * s) / (1.0 - e * s)).ln() / (2.0 * e))
            };
            let dlon = t[1].abs().to_radians();
            let mut out = Vec::with_capacity(w * h);
            for r in 0..h {
                let y0 = t[3] + r as f64 * t[5];
                let y1 = y0 + t[5];
                let area = (zone(y0) - zone(y1)).abs() * dlon / 1e6;
                out.extend(std::iter::repeat_n(area, w));
            }
            Some(out)
        }
    }
}

struct AreaOfHabitat;

impl Operator for AreaOfHabitat {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "sdm.area_of_habitat",
            "The area of habitat of species (Brooks et al. 2019), as plangea (harmonize_bd): over the cells of \
             each species' range (within its elevation limits), the shares of the natural classes suitable to it \
             times the cell's area, now and before conversion (the original vegetation); and the habitat it gains \
             per unit of a cell restored. Rows in the order of the ranges' layers.",
            Access::Global,
        )
        .input(PortSpec::new("ranges", Type::Stack, "each species' range (cells above 0), by species"))
        .input(PortSpec::new("land_cover", Type::Stack, "the share of each class, by name"))
        .input(PortSpec::optional("original", Type::Stack, "the original share of each natural class"))
        .input(PortSpec::optional("restore", Type::Stack, "the shares restored land returns to"))
        .input(PortSpec::optional("anthropic", Type::Map, "the share of anthropic land (where restoring gains)"))
        .input(PortSpec::optional("elevation", Type::Map, "the elevation, for the species' limits"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "per species (in the ranges' order): its area of habitat now and potential (km²), its range's \
             cells, and whether it counts (1: habitat now and potential, and suitable classes)",
        ))
        .output(PortSpec::new("gain", Type::Stack, "the habitat each species gains per unit of a cell restored"))
        .param(PortSpec::new("natural", Type::Any, "the natural classes (names of the land cover's layers)"))
        .param(PortSpec::optional(
            "suitability",
            Type::Any,
            "each species' suitable classes: {species: [classes]} (the species not listed: every natural class)",
        ))
        .param(PortSpec::optional("include_unlisted", Type::Bool, "species not in suitability count (default true)"))
        .param(PortSpec::optional("elevation_limits", Type::Any, "[[species, lowest, highest], ...] (null: none)"))
        .param(PortSpec::optional("cell_area", Type::Number, "a cell's area in km² (default: the grid's)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let ranges = args.stack("ranges")?;
        let cover = args.stack("land_cover")?;
        let natural =
            names(args, "natural")?.ok_or_else(|| Error::msg(format!("node {node:?}: no natural classes")))?;
        let grid = &ranges.grid;
        let ncell = grid.width * grid.height;
        let same = |g: &Grid| g.width == grid.width && g.height == grid.height;
        if !same(&cover.grid) {
            return Err(Error::msg(format!("node {node:?}: the land cover is not on the ranges' grid")));
        }
        let k = natural.len();
        let current = layers(args, "land_cover", cover, &natural)?;
        let (orig, restore, domain) = match args.opt_stack("original")? {
            None => (current.clone(), None, Vec::new()),
            Some(o) => {
                if !same(&o.grid) {
                    return Err(Error::msg(format!(
                        "node {node:?}: the original vegetation is not on the ranges' grid"
                    )));
                }
                let raw: Vec<f64> = {
                    let mut out = Vec::with_capacity(k * ncell);
                    for c in &natural {
                        let i = o.keys.iter().position(|x| x.to_string() == *c).ok_or_else(|| {
                            Error::msg(format!("node {node:?}: the original vegetation has no layer {c:?}"))
                        })?;
                        out.extend(o.layers[i].iter());
                    }
                    out
                };
                // where the original has no value, the current cover
                let orig: Vec<f64> = raw.iter().zip(&current).map(|(o, c)| if o.is_nan() { *c } else { *o }).collect();
                let restore = match args.opt_stack("restore")? {
                    Some(r) => layers(args, "restore", r, &natural)?,
                    None => {
                        let mut r = vec![0.0; k * ncell];
                        for c in 0..ncell {
                            let total: f64 = (0..k).map(|j| orig[j * ncell + c]).sum();
                            for j in 0..k {
                                r[j * ncell + c] = if total > 0.0 { orig[j * ncell + c] / total } else { 0.0 };
                            }
                        }
                        r
                    }
                };
                let domain = match args.opt_map("anthropic")? {
                    Some(a) => a.data.iter().map(|&v| u8::from(!v.is_nan() && v > 0.0)).collect(),
                    None => Vec::new(),
                };
                (orig, Some(restore), domain)
            }
        };
        let area: Vec<f64> = match args.opt_param::<f64>("cell_area")? {
            Some(a) => vec![a; ncell],
            None => cell_area_km2(grid).ok_or_else(|| {
                Error::msg(format!("node {node:?}: the ranges have no coordinate system: give cell_area (km²)"))
            })?,
        };
        let elevation = args.opt_map("elevation")?;
        let mut limits: Vec<(String, f64, f64)> = Vec::new();
        if let Some(v) = args.params.get("elevation_limits").filter(|v| !v.is_null()) {
            let rows: Vec<(String, Option<f64>, Option<f64>)> = serde_json::from_value(v.clone()).map_err(|e| {
                Error::msg(format!("node {node:?}: elevation_limits: {e} ([[species, lowest, highest], ...])"))
            })?;
            limits = rows
                .into_iter()
                .map(|(s, lo, hi)| (s, lo.unwrap_or(f64::NEG_INFINITY), hi.unwrap_or(f64::INFINITY)))
                .collect();
        }
        let suitability: std::collections::BTreeMap<String, Vec<String>> = match args.params.get("suitability") {
            None | Some(serde_json::Value::Null) => Default::default(),
            Some(v) => serde_json::from_value(v.clone())
                .map_err(|e| Error::msg(format!("node {node:?}: suitability: {e} ({{species: [classes]}})")))?,
        };
        let include_unlisted: bool = args.param("include_unlisted", true)?;
        let species: Vec<String> = ranges.keys.iter().map(|k| k.to_string()).collect();
        let mut ptr = vec![0i64];
        let mut idx: Vec<i64> = Vec::new();
        let mut suit = vec![0u8; species.len() * k];
        let mut listed = Vec::new();
        for (s, name) in species.iter().enumerate() {
            let lim = limits.iter().find(|(n, ..)| n == name);
            for (c, &v) in ranges.layers[s].iter().enumerate() {
                if v.is_nan() || v <= 0.0 {
                    continue;
                }
                if let (Some(e), Some((_, lo, hi))) = (elevation, lim) {
                    let x = e.data[c];
                    if !x.is_nan() && !(x >= *lo && x <= *hi) {
                        continue;
                    }
                }
                idx.push(c as i64);
            }
            ptr.push(idx.len() as i64);
            let classes: Vec<&String> = match suitability.get(name) {
                Some(cls) => {
                    listed.push(true);
                    natural.iter().filter(|c| cls.contains(c)).collect()
                }
                None => {
                    listed.push(include_unlisted);
                    if include_unlisted { natural.iter().collect() } else { Vec::new() }
                }
            };
            for c in classes {
                suit[s * k + natural.iter().position(|x| x == c).unwrap()] = 1;
            }
        }
        let ns = species.len() as i64;
        let h = ffi::rest_habitat(&current, &orig, &area, ncell as i64, k as i32, &ptr, &idx, &suit, ns)?;
        let mut table = Table::new(["current", "potential", "range_cells", "valid"].map(String::from).to_vec());
        for s in 0..species.len() {
            let any = suit[s * k..(s + 1) * k].contains(&1);
            let valid = h.now[s] > 0.0 && h.potential[s] > 0.0 && any && listed[s];
            table.rows.push(vec![h.now[s], h.potential[s], (ptr[s + 1] - ptr[s]) as f64, f64::from(u8::from(valid))]);
        }
        let mut out = outputs([("table", Value::Table(table))]);
        if let Some(restore) = restore {
            let g = ffi::rest_habitat_gain(&restore, &area, ncell as i64, k as i32, &ptr, &idx, &suit, ns, &domain)?;
            let layers = (0..species.len())
                .map(|s| {
                    let mut m = vec![0.0; ncell];
                    for i in g.indptr[s] as usize..g.indptr[s + 1] as usize {
                        m[g.index[i] as usize] = g.gain[i];
                    }
                    Arc::new(m)
                })
                .collect();
            let keys = species.iter().map(|s| Key::Name(s.clone())).collect();
            out.insert("gain".into(), Value::Stack(Stack { grid: grid.clone(), keys, layers }));
        }
        Ok(out)
    }
}
