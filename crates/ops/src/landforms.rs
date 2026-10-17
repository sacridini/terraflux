//! Landforms and potentials of a surface through landschaft's C++ (its `ls.surface`): geomorphons (GRASS's
//! r.geomorphon bit for bit), the hilltops of Brazil's Forest Code (APP de topo de morro, Lei 12.651/2012) and the
//! gravitational interaction potential (Dinamica EGO's CalcInteractionPotentialMap).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Geomorphons));
    r.add(Arc::new(Hilltop));
    r.add(Arc::new(Gravity));
}

/// The 10 landforms of the geomorphons, by their code.
pub const GEOMORPHON_FORMS: [&str; 10] =
    ["flat", "peak", "ridge", "shoulder", "spur", "slope", "hollow", "footslope", "valley", "pit"];

const MEASURES: [&str; 8] =
    ["intensity", "exposition", "range", "variance", "elongation", "azimuth", "extend", "width"];

fn fail<'a>(args: &'a Args<'_>) -> impl Fn(Error) -> Error + 'a {
    move |e| Error::msg(format!("node {:?}: {e}", args.node))
}

struct Geomorphons;

impl Operator for Geomorphons {
    fn signature(&self) -> OpSignature {
        let mut sig = OpSignature::new(
            "surface.geomorphons",
            "Landforms of a DEM by geomorphons (Jasiewicz & Stepinski 2013), GRASS's r.geomorphon (8.4) bit for bit \
             with four of its errors corrected, through landschaft's C++: from each cell the terrain is looked at \
             along 8 directions up to the search radius; the signs (higher, level, lower) name the landform (1 \
             flat, 2 peak, 3 ridge, 4 shoulder, 5 spur, 6 slope, 7 hollow, 8 footslope, 9 valley, 10 pit) and one of \
             498 patterns. A projected DEM.",
            Access::Global,
        )
        .input(PortSpec::new("dem", Type::Map, "the DEM (projected; read as float32, as GRASS reads it)"))
        .output(PortSpec::new("forms", Type::Map, "the landform of each cell, 1 to 10"))
        .output(PortSpec::new("ternary", Type::Map, "the ternary pattern (0 to 6560, 498 codes)"))
        .output(PortSpec::new("positive", Type::Map, "the directions higher than the threshold, as 8 bits"))
        .output(PortSpec::new("negative", Type::Map, "the directions lower than the threshold, as 8 bits"))
        .output(PortSpec::new("share", Type::Table, "form, cells, share"));
        for m in MEASURES {
            sig = sig.output(PortSpec::new(m, Type::Map, "a measure of the geomorphon (with measures)"));
        }
        sig.param(PortSpec::optional("search", Type::Number, "the outer search radius, in cells or map units (3)"))
            .param(PortSpec::optional("skip", Type::Number, "the inner radius not looked at (0)"))
            .param(PortSpec::optional("flat", Type::Number, "the flatness threshold, degrees (1)"))
            .param(PortSpec::optional("dist", Type::Number, "the flatness distance (0: none)"))
            .param(
                PortSpec::optional("units", Type::Text, "of search, skip and dist: cells (default) or map")
                    .choices(["cells", "map"]),
            )
            .param(
                PortSpec::optional("comparison", Type::Text, "how the angles decide a direction (anglev1)").choices([
                    "anglev1",
                    "anglev2",
                    "anglev2_distance",
                ]),
            )
            .param(PortSpec::optional(
                "extended",
                Type::Bool,
                "the extended correction (-e): ridges, peaks, shoulders and footslopes looked at again within half the \
                 radius",
            ))
            .param(PortSpec::optional("measures", Type::Bool, "also the measures (intensity, exposition...)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        geomorphons(args).map_err(fail(args))
    }
}

fn geomorphons(args: &Args) -> Result<Outputs> {
    let m = args.map("dem")?;
    let units: String = args.param("units", "cells".to_string())?;
    if units != "cells" && units != "map" {
        return Err(Error::msg(format!("units must be 'cells' or 'map', got {units:?}")));
    }
    let comparison: String = args.param("comparison", "anglev1".to_string())?;
    let comparison =
        ["anglev1", "anglev2", "anglev2_distance"].iter().position(|c| *c == comparison).ok_or_else(|| {
            Error::msg(format!("comparison must be anglev1, anglev2 or anglev2_distance, got {comparison:?}"))
        })?;
    let (search, skip, flat, dist): (f64, f64, f64, f64) =
        (args.param("search", 3.0)?, args.param("skip", 0.0)?, args.param("flat", 1.0)?, args.param("dist", 0.0)?);
    if flat.is_nan() || flat <= 0.0 {
        return Err(Error::msg("the flatness threshold must be greater than 0"));
    }
    let (e, lonlat) = crate::surface::extent_of(&m.grid);
    if lonlat {
        return Err(Error::msg("geomorphons need a projected DEM (distances in map units): reproject it first"));
    }
    let (ny, nx) = m.shape();
    let (ns_res, ew_res) = ((e[3] - e[2]) / ny as f64, (e[1] - e[0]) / nx as f64);
    let max_res = ns_res.max(ew_res);
    let meters = units == "map";
    let search_cells = if meters { (search / max_res) as i32 } else { search as i32 };
    if search_cells < 1 {
        return Err(Error::msg("the search radius must cover at least 1 cell"));
    }
    let skip_cells = if meters { (skip / max_res) as i32 } else { skip as i32 };
    if skip_cells >= search_cells {
        return Err(Error::msg("the skip radius must be at least 1 cell less than the search radius"));
    }
    let search_distance = if meters { search } else { ns_res * f64::from(search_cells) };
    let skip_distance = if meters { skip } else { ns_res * f64::from(skip_cells) };
    let pi = 4.0 * 1f64.atan();
    let flat_threshold = flat / (180.0 / pi);
    let mut flat_distance = if meters { dist } else { ns_res * dist };
    let flat_height = flat_threshold.tan() * flat_distance;
    if (0.0 < flat_distance && flat_distance <= skip_distance) || flat_distance >= search_distance {
        flat_distance = 0.0;
    }
    let measures: bool = args.param("measures", false)?;
    let outputs_mask: u32 =
        1 | 2 | 4 | 8 | if measures { (16..=2048).filter(|b: &u32| b.is_power_of_two()).sum() } else { 0 };
    let z: Vec<f32> = m.data.iter().map(|&v| v as f32).collect();
    let r = ffi::surf_geomorphons(
        &z,
        ny as i32,
        nx as i32,
        e[3],
        e[0],
        ns_res,
        ew_res,
        search_cells,
        skip_cells,
        search_distance,
        flat_threshold,
        flat_distance,
        flat_height,
        max_res,
        args.param("extended", false)?,
        comparison as i32,
        outputs_mask,
    )?;
    let grid = &m.grid;
    let codes = |v: &[i32], none: &dyn Fn(i32) -> bool| -> Value {
        Value::Map(Map::new(grid.clone(), v.iter().map(|&x| if none(x) { f64::NAN } else { f64::from(x) }).collect()))
    };
    let mut out = Outputs::new();
    out.insert("forms".into(), codes(&r.forms, &|x| x == 0));
    out.insert("ternary".into(), codes(&r.ternary, &|x| x < 0));
    out.insert("positive".into(), codes(&r.positive, &|x| x < 0));
    out.insert("negative".into(), codes(&r.negative, &|x| x < 0));
    let mut at = 0usize;
    for (k, name) in MEASURES.iter().enumerate() {
        let n = r.lengths[k] as usize;
        if n > 0 {
            out.insert((*name).into(), Value::Map(Map::new(grid.clone(), r.measures[at..at + n].to_vec())));
        }
        at += n;
    }
    let mut counts = [0i64; 10];
    for &f in &r.forms {
        if (1..=10).contains(&f) {
            counts[(f - 1) as usize] += 1;
        }
    }
    let total = counts.iter().sum::<i64>().max(1) as f64;
    let mut t = Table::new(vec!["form".into(), "cells".into(), "share".into()]);
    t.rows = (0..10).map(|k| vec![(k + 1) as f64, counts[k] as f64, counts[k] as f64 / total]).collect();
    out.insert("share".into(), Value::Table(t));
    Ok(out)
}

struct Hilltop;

impl Operator for Hilltop {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "surface.hilltop",
            "Hilltops as permanent protection areas (APP de topo de morro, Lei 12.651/2012, art. 4, IX), \
             landschaft's open implementation: every summit with its base (the saddle to higher terrain, the plain \
             or water body it meets, or the lowest cell of its area), its height and slope; the hills at least \
             min_height high and steeper than min_slope protected above base + fraction x height.",
            Access::Global,
        )
        .input(PortSpec::new("dem", Type::Map, "the DEM (metres), projected or in longitude / latitude"))
        .input(PortSpec::optional("plain", Type::Map, "plains and water bodies (non-zero): a hill's base where met"))
        .output(PortSpec::new("app", Type::Map, "1 in the upper part of the hills that qualify, 0 elsewhere"))
        .output(PortSpec::new("hill", Type::Map, "the hill whose upper part holds each cell"))
        .output(PortSpec::new(
            "hills",
            Type::Table,
            "per summit, highest first: hill, x, y, top, base, base_kind (0 saddle, 1 plain, 2 lowest), base_x, \
             base_y, height, mean_slope, line_slope, area, area_ha, qualifies, threshold, app_cells",
        ))
        .param(PortSpec::optional("min_height", Type::Number, "the least height of a hill, m (100)"))
        .param(PortSpec::optional("min_slope", Type::Number, "the slope a hill must exceed, degrees (25)"))
        .param(
            PortSpec::optional("slope", Type::Text, "mean (of the hill's cells, default) or line (summit to base)")
                .choices(["mean", "line"]),
        )
        .param(PortSpec::optional("fraction", Type::Number, "where the APP starts, of the height (2/3)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        hilltop(args).map_err(fail(args))
    }
}

fn hilltop(args: &Args) -> Result<Outputs> {
    let m = args.map("dem")?;
    let slope: String = args.param("slope", "mean".to_string())?;
    if slope != "mean" && slope != "line" {
        return Err(Error::msg(format!("slope must be 'mean' or 'line', got {slope:?}")));
    }
    let fraction: f64 = args.param("fraction", 2.0 / 3.0)?;
    if !(0.0..=1.0).contains(&fraction) {
        return Err(Error::msg("fraction must be between 0 and 1"));
    }
    let (ny, nx) = m.shape();
    let (e, lonlat) = crate::surface::extent_of(&m.grid);
    let sl = ffi::surf_terrain(&m.data, nx as i32, ny as i32, &e, lonlat, 8, true, false)?;
    let plain = match args.opt_map("plain")? {
        None => Vec::new(),
        Some(p) => {
            if p.shape() != m.shape() {
                return Err(Error::msg(format!("the plain mask has shape {:?}, the DEM {:?}", p.shape(), m.shape())));
            }
            p.data.iter().map(|v| u8::from(!v.is_nan() && *v != 0.0)).collect()
        }
    };
    let (dx, dy) = crate::hydro::cells(&m.grid);
    let r = ffi::surf_hilltops(
        &m.data,
        &sl,
        &plain,
        ny as i32,
        nx as i32,
        dx,
        dy,
        args.param("min_height", 100.0)?,
        args.param("min_slope", 25.0)?,
        slope == "line",
        fraction,
    )?;
    let app: Vec<f64> = r
        .label
        .iter()
        .zip(m.data.iter())
        .map(|(&l, z)| if z.is_nan() { f64::NAN } else { f64::from(u8::from(l >= 0)) })
        .collect();
    let hill: Vec<f64> = r.label.iter().map(|&l| if l >= 0 { f64::from(l) } else { f64::NAN }).collect();
    let (rx, ry) = ((e[1] - e[0]) / nx as f64, (e[3] - e[2]) / ny as f64);
    let xy = |c: i64| (e[0] + ((c as usize % nx) as f64 + 0.5) * rx, e[3] - ((c as usize / nx) as f64 + 0.5) * ry);
    let mut counts = vec![0i64; r.summit.len()];
    for &l in &r.label {
        if l >= 0 {
            counts[l as usize] += 1;
        }
    }
    let cols = [
        "hill",
        "x",
        "y",
        "top",
        "base",
        "base_kind",
        "base_x",
        "base_y",
        "height",
        "mean_slope",
        "line_slope",
        "area",
        "area_ha",
        "qualifies",
        "threshold",
        "app_cells",
    ];
    let mut t = Table::new(cols.iter().map(|c| c.to_string()).collect());
    for (h, &app_cells) in counts.iter().enumerate() {
        let (sx, sy) = xy(r.summit[h]);
        let (bx, by) = xy(r.base_cell[h]);
        let cells = r.cells[h] as f64;
        t.rows.push(vec![
            h as f64,
            sx,
            sy,
            r.top[h],
            r.base[h],
            f64::from(r.kind[h]),
            bx,
            by,
            r.height[h],
            r.mean_slope[h],
            r.line_slope[h],
            cells * (rx * ry).abs(),
            cells * dx * dy / 1e4,
            f64::from(r.qualifies[h]),
            r.threshold[h],
            app_cells as f64,
        ]);
    }
    let grid = &m.grid;
    Ok(outputs([
        ("app", Value::Map(Map::new(grid.clone(), app))),
        ("hill", Value::Map(Map::new(grid.clone(), hill))),
        ("hills", Value::Table(t)),
    ]))
}

// method "auto": direct sums with up to this many sources (or source-cell pairs), the FFT beyond (landschaft's)
const DIRECT_SOURCES: usize = 2000;
const DIRECT_PAIRS: f64 = 1e8;

struct Gravity;

impl Operator for Gravity {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "surface.gravity",
            "Gravitational interaction potential of source cells, Dinamica EGO's CalcInteractionPotentialMap \
             (landschaft's): every cell the sum over the sources of their mass over the distance to them raised to \
             the exponent, P = sum m / d^beta (a source's own cell at one cell); a pressure of towns, markets or \
             roads, a driver of land change.",
            Access::Global,
        )
        .input(PortSpec::new("sources", Type::Map, "the masses (no value or 0: not a source)"))
        .input(PortSpec::optional("mask", Type::Map, "the potential is missing where this is"))
        .output(PortSpec::new("map", Type::Map, "the potential"))
        .param(PortSpec::optional("exponent", Type::Number, "the distance decay beta (2; 0: none)"))
        .param(
            PortSpec::optional("unit", Type::Text, "distances in cells (default, Dinamica's) or map units")
                .choices(["cells", "map"]),
        )
        .param(
            PortSpec::optional(
                "method",
                Type::Text,
                "direct (every pair, in a fixed order), fft (a convolution, about 1e-15 of the largest term off), or \
                 auto (default: direct up to 2000 sources or 1e8 pairs)",
            )
            .choices(["auto", "direct", "fft"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        gravity(args).map_err(fail(args))
    }
}

fn gravity(args: &Args) -> Result<Outputs> {
    let s = args.map("sources")?;
    let unit: String = args.param("unit", "cells".to_string())?;
    if unit != "cells" && unit != "map" {
        return Err(Error::msg(format!("unit must be 'cells' or 'map', got {unit:?}")));
    }
    let mut method: String = args.param("method", "auto".to_string())?;
    if !["auto", "direct", "fft"].contains(&method.as_str()) {
        return Err(Error::msg(format!("method must be 'auto', 'direct' or 'fft', got {method:?}")));
    }
    let beta: f64 = args.param("exponent", 2.0)?;
    if !beta.is_finite() {
        return Err(Error::msg("exponent must be finite"));
    }
    let (ny, nx) = s.shape();
    let mask = match args.opt_map("mask")? {
        None => Vec::new(),
        Some(m) => {
            if m.shape() != s.shape() {
                return Err(Error::msg(format!("the mask has shape {:?}, the sources {:?}", m.shape(), s.shape())));
            }
            m.data.to_vec()
        }
    };
    let (dx, dy) = if unit == "cells" {
        (1.0, 1.0)
    } else {
        let t = s.grid.transform;
        (t[1].abs(), t[5].abs())
    };
    let self_d = (dx * dy).sqrt();
    let n_sources = s.data.iter().filter(|v| !v.is_nan() && **v != 0.0).count();
    if method == "auto" {
        let direct = n_sources <= DIRECT_SOURCES || n_sources as f64 * s.data.len() as f64 <= DIRECT_PAIRS;
        method = if direct { "direct" } else { "fft" }.to_string();
    }
    let out = if method == "direct" || n_sources == 0 {
        ffi::surf_gravity(&s.data, &mask, ny as i32, nx as i32, beta, dx, dy, self_d)?
    } else {
        let w = ffi::surf_gravity_weights(ny as i32, nx as i32, beta, dx, dy, self_d)?;
        let mass: Vec<f64> = s.data.iter().map(|v| if v.is_nan() { 0.0 } else { *v }).collect();
        let mut p = ffi::surf_gravity_fft(&mass, &w, ny as i32, nx as i32)?;
        for (v, m) in p.iter_mut().zip(&mask) {
            if m.is_nan() {
                *v = f64::NAN;
            }
        }
        p
    };
    Ok(outputs([("map", Value::Map(Map::new(s.grid.clone(), out)))]))
}
