//! Seasonal water yield (InVEST's seasonal_water_yield model, natcap.invest 3.20) as landschaft 1.54 computes it
//! (`ls.services.seasonal_water_yield`): the monthly quickflow of the curve numbers, the local recharge routed
//! down the slope and the baseflow it gives (landschaft's C++), every raster in InVEST's float32 / float64 type
//! and with its expression.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::Vector;

use super::ndr::{iterblocks, sum_f32};
use super::{LandUse, Routing, attribute, f32_map, f64_map, lookup, polygons, table_of, wide, zonal};
use crate::change::numpy_sum;
use crate::hydro::MFD_BLOCK;
use crate::{Args, Ctx, Error, Key, Operator, Outputs, Result, Stack, Table, Value, npmath};

pub(super) struct SeasonalWaterYield;

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const SOIL_COLUMNS: [&str; 4] = ["cn_a", "cn_b", "cn_c", "cn_d"];

impl Operator for SeasonalWaterYield {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "services.seasonal_water_yield",
            "Quickflow, local recharge and baseflow: InVEST's seasonal water yield model (natcap.invest 3.20) \
             bit for bit, through landschaft. The quickflow of each month from the curve numbers of the land use \
             and soil group and the rain events; the local recharge P - QF - AET, the evapotranspiration limited \
             by the water from upslope (alpha, beta, gamma); the baseflow the recharge gives the cells down to \
             the streams; with a recharge map instead (and no climate), its routing only. The DEM filled and \
             routed by pygeoprocessing's algorithms, as InVEST.",
            Access::Global,
        )
        .input(PortSpec::new("dem", Type::Map, "elevations (projected, metres)"))
        .input(PortSpec::optional("lulc", Type::Map, "land use (class codes; with the climate)"))
        .input(PortSpec::optional(
            "biophysical",
            Type::Table,
            "by class: lucode, cn_a, cn_b, cn_c, cn_d (curve numbers), kc_1 ... kc_12 (with the climate)",
        ))
        .input(PortSpec::new("watersheds", Type::Vector, "the watershed polygons (the totals by polygon)"))
        .input(PortSpec::optional("precipitation", Type::Stack, "the twelve months' precipitation, mm"))
        .input(PortSpec::optional("et0", Type::Stack, "the twelve months' reference evapotranspiration, mm"))
        .input(PortSpec::optional("soil_group", Type::Map, "hydrologic soil groups, 1 (A) to 4 (D)"))
        .input(PortSpec::optional("climate_zones", Type::Map, "climate zones (with the climate zone table)"))
        .input(PortSpec::optional(
            "climate_zone_table",
            Type::Table,
            "by zone: cz_id, jan ... dec (the rain events of each month)",
        ))
        .input(PortSpec::optional("recharge", Type::Map, "a local recharge map (mm), instead of the climate"))
        .output(PortSpec::new("quickflow", Type::Map, "the annual quickflow, mm"))
        .output(PortSpec::new("local_recharge", Type::Map, "the local recharge L, mm"))
        .output(PortSpec::new("l_avail", Type::Map, "the recharge available to the cell, mm"))
        .output(PortSpec::new("l_sum_avail", Type::Map, "the recharge available from upslope, mm"))
        .output(PortSpec::new("l_sum", Type::Map, "the recharge from upslope, mm"))
        .output(PortSpec::new("baseflow", Type::Map, "the baseflow the cell gives, mm"))
        .output(PortSpec::new("b_sum", Type::Map, "the baseflow from upslope, mm"))
        .output(PortSpec::new("vri", Type::Map, "the cell's share of the total recharge"))
        .output(PortSpec::new("aet", Type::Map, "the annual actual evapotranspiration, mm"))
        .output(PortSpec::new("annual_precipitation", Type::Map, "the annual precipitation, mm"))
        .output(PortSpec::new("stream", Type::Map, "the streams (1)"))
        .output(PortSpec::new("quickflow_monthly", Type::Stack, "the quickflow of each month (1 to 12), mm"))
        .output(PortSpec::new("watershed_table", Type::Table, "by watershed: qb (mean recharge), vri_sum, geom_id"))
        .output(PortSpec::new(
            "monthly_table",
            Type::Table,
            "by watershed and month: geom_id, month, quickflow, baseflow, precipitation (m³)",
        ))
        .param(PortSpec::new("threshold", Type::Number, "the flow accumulation (cells) of a stream"))
        .param(PortSpec::optional(
            "rain_events",
            Type::Any,
            "the rain events of each month, twelve numbers (or the climate zones)",
        ))
        .param(PortSpec::optional(
            "alpha",
            Type::Any,
            "the share of the annual recharge upslope available each month: a number (1/12) or twelve",
        ))
        .param(PortSpec::optional("beta", Type::Any, "the share of the upslope recharge available to the cell (1)"))
        .param(PortSpec::optional("gamma", Type::Any, "the share of the cell's recharge available downslope (1)"))
        .param(PortSpec::optional("method", Type::Text, "the routing: mfd (default) or d8").choices(["mfd", "d8"]))
        .param(
            PortSpec::optional(
                "precision",
                Type::Text,
                "the type of the precipitation, ET0 and recharge rasters in InVEST: float32, float64, or auto \
                 (default: float64 when a raster has a value float32 cannot hold)",
            )
            .choices(["auto", "float32", "float64"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        swy(args).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

/// A parameter as InVEST reads it: float(Fraction(v)), "1/12" too.
fn fraction(v: &serde_json::Value, what: &str) -> Result<f64> {
    let bad = || Error::msg(format!("{what} must be a number, not {v}"));
    match v {
        serde_json::Value::Number(n) => n.as_f64().ok_or_else(bad),
        serde_json::Value::String(s) => {
            let s = s.trim();
            match s.split_once('/') {
                Some((a, b)) => {
                    let (a, b): (f64, f64) =
                        (a.trim().parse().map_err(|_| bad())?, b.trim().parse().map_err(|_| bad())?);
                    if b == 0.0 { Err(bad()) } else { Ok(a / b) }
                }
                None => s.parse().map_err(|_| bad()),
            }
        }
        _ => Err(bad()),
    }
}

fn number_param(args: &Args, name: &str, default: f64) -> Result<f64> {
    match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Ok(default),
        Some(v) => fraction(v, name),
    }
}

/// Twelve numbers of a setting.
fn months_param(args: &Args, name: &str) -> Result<Option<Vec<f64>>> {
    match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Array(a)) => {
            let v: Vec<f64> = a.iter().map(|x| fraction(x, name)).collect::<Result<_>>()?;
            if v.len() != 12 {
                return Err(Error::msg(format!("{name} must hold 12 months, not {}", v.len())));
            }
            Ok(Some(v))
        }
        Some(v) => Err(Error::msg(format!("{name} is twelve numbers (a month each), not {v}"))),
    }
}

/// scipy's exp1 of a float32 (specfun's E1XB in float64, rounded to float32).
fn exp1(x: f32) -> f32 {
    let x = f64::from(x);
    if x == 0.0 {
        return f32::INFINITY;
    }
    let v = if x <= 1.0 {
        let (mut e1, mut r) = (1.0f64, 1.0f64);
        for k in 1..=25 {
            let kf = f64::from(k);
            r = -r * kf * x / ((kf + 1.0) * (kf + 1.0));
            e1 += r;
            if r.abs() <= e1.abs() * 1e-15 {
                break;
            }
        }
        -0.5772156649015328 - x.ln() + x * e1
    } else {
        let m = 20 + (80.0 / x) as i32;
        let mut t0 = 0.0f64;
        for k in (1..=m).rev() {
            let kf = f64::from(k);
            t0 = kf / (1.0 + kf / (x + t0));
        }
        (-x).exp() / (x + t0)
    };
    v as f32
}

/// The values of a raster in its type (InVEST's): float32 unless it is wide.
fn typed(v: &[f64], is_wide: bool) -> Vec<f64> {
    if is_wide { v.to_vec() } else { v.iter().map(|&x| f64::from(x as f32)).collect() }
}

/// InVEST's qf_op (eq. 1): the quickflow of a month (float32), NaN as InVEST's nodata; `p` in its type.
fn quickflow(p: &[f64], p_wide: bool, si: &[f32], n_m: &[f32], stream: &[u8]) -> Vec<f32> {
    let n = p.len();
    let mut qf = vec![f32::NAN; n];
    for i in 0..n {
        let (valid_p, valid_n) = (!p[i].is_nan(), !n_m[i].is_nan());
        if !(valid_p && valid_n) {
            continue;
        }
        let nm = n_m[i];
        let pi = p[i];
        let precip_ok = pi > 0.0 && nm > 0.0;
        let on_stream = stream[i] == 1;
        let valid = stream[i] != 255 && !si[i].is_nan();
        if !precip_ok {
            qf[i] = 0.0;
        } else if on_stream {
            qf[i] = pi as f32;
        } else if valid {
            let s = si[i];
            let den = nm * 25.4;
            let a: f32 = if p_wide { (pi / f64::from(den)) as f32 } else { pi as f32 / den };
            if s == 0.0 {
                qf[i] = pi as f32;
            } else if s / a > 100.0 {
                qf[i] = 0.0;
            } else {
                let r = s / a;
                let t1 = (a - s) * npmath::exp(-0.2 * s / a);
                let t2 = s * s / a * npmath::exp(0.8 * s / a) * exp1(r);
                qf[i] = 25.4 * nm * (t1 + t2);
            }
        }
        if valid && qf[i] < 0.0 {
            qf[i] = 0.0;
        }
    }
    qf
}

/// The twelve layers of a stack, on the land use's grid.
fn months_of<'a>(args: &'a Args, port: &str, what: &str, shape: (usize, usize)) -> Result<Vec<&'a [f64]>> {
    let s = args.stack(port)?;
    if s.layers.len() != 12 {
        return Err(Error::msg(format!("{what} must have 12 layers (months), not {}", s.layers.len())));
    }
    if (s.grid.height, s.grid.width) != shape {
        return Err(Error::msg(format!(
            "{what} has {:?} cells, the land use map {shape:?}",
            (s.grid.height, s.grid.width)
        )));
    }
    Ok(s.layers.iter().map(|l| l.as_slice()).collect())
}

fn swy(args: &Args) -> Result<Outputs> {
    let method: String = args.param("method", "mfd".to_string())?.to_lowercase();
    if method != "mfd" && method != "d8" {
        return Err(Error::msg("method is 'mfd' or 'd8'"));
    }
    let threshold: f64 = args.opt_param("threshold")?.ok_or_else(|| Error::msg("give the threshold"))?;
    if threshold.is_nan() || threshold <= 0.0 {
        return Err(Error::msg("threshold must be positive"));
    }
    let precision: String = args.param("precision", "auto".to_string())?;
    let (beta, gamma) = (number_param(args, "beta", 1.0)?, number_param(args, "gamma", 1.0)?);
    let dem_map = args.map("dem")?;
    let grid = dem_map.grid.clone();
    let shape = dem_map.shape();
    let (ny, nx) = (grid.height as i32, grid.width as i32);
    let n = grid.cells();
    let raster = |port: &str, what: &str| -> Result<&[f64]> {
        let m = args.map(port)?;
        if m.shape() != shape {
            return Err(Error::msg(format!("{what} has {:?} cells, the DEM {shape:?}", m.shape())));
        }
        Ok(&m.data)
    };
    let dem: &[f64] = &dem_map.data;
    let t = grid.transform;
    let pixel_area = (t[1] * t[5]).abs();
    // projected maps only, as InVEST
    if crate::surface::extent_of(&grid).1 {
        return Err(Error::msg("the DEM is in longitude / latitude: give it projected, in metres (as InVEST)"));
    }
    // the filled DEM in the DEM's type
    let dem_wide = wide("auto", dem);
    let filled = typed(&ffi::hydro_fill(dem, ny, nx, false)?, dem_wide);
    let r = Routing::new(&filled, ny, nx, &method)?;
    let stream: Vec<u8> = if method == "mfd" {
        ffi::hydro_mfd_streams(&r.acc, &r.mfd, ny, nx, threshold, 1.0, MFD_BLOCK)?
    } else {
        r.acc.iter().map(|a| if a.is_nan() { 255 } else { u8::from(*a > threshold) }).collect()
    };
    let sheds = polygons(crate::vector::vector_of(args, "watersheds")?, &grid)?;
    let mut out = Outputs::new();
    let (l32, la32): (Vec<f64>, Vec<f64>);
    let mut l_wide = false;
    let mut climate = None;
    if args.get("recharge").is_none() {
        for (port, what) in [
            ("lulc", "the land use"),
            ("biophysical", "the biophysical table"),
            ("precipitation", "the precipitation"),
            ("et0", "the ET0"),
            ("soil_group", "the soil groups"),
        ] {
            if args.get(port).is_none() {
                return Err(Error::msg(format!("give {what} (or the recharge map)")));
            }
        }
        let lu = LandUse::new(args.map("lulc")?, "the land use map")?;
        raster("lulc", "the land use map")?;
        let has_events = args.params.get("rain_events").is_some_and(|v| !v.is_null());
        if has_events == args.get("climate_zones").is_some() {
            return Err(Error::msg("give the rain events or the climate zones (and their table)"));
        }
        let alpha_m: Vec<f64> = match args.params.get("alpha") {
            Some(serde_json::Value::Array(_)) => months_param(args, "alpha")?.unwrap(),
            _ => vec![number_param(args, "alpha", 1.0 / 12.0)?; 12],
        };
        let mut kc_cols: Vec<String> = SOIL_COLUMNS.map(String::from).to_vec();
        kc_cols.extend((1..=12).map(|m| format!("kc_{m}")));
        let refs: Vec<&str> = kc_cols.iter().map(String::as_str).collect();
        let (codes, bio) = lookup(table_of(args, "biophysical")?, &refs, "the biophysical table", "lucode")?;
        let p_list = months_of(args, "precipitation", "the precipitation", shape)?;
        let e_list = months_of(args, "et0", "the ET0", shape)?;
        let p_wide = p_list.iter().any(|p| wide(&precision, p));
        let e_wide = e_list.iter().any(|e| wide(&precision, e));
        let soil = raster("soil_group", "the soil group map")?;
        // rain events per month: a constant (also off the DEM, as InVEST's new raster) or by zone
        let n_events: Vec<Vec<f32>> = if has_events {
            let ev = months_param(args, "rain_events")?.unwrap();
            ev.iter().map(|&e| vec![e as f32; n]).collect()
        } else {
            let tab = table_of(args, "climate_zone_table")
                .map_err(|_| Error::msg("give the climate zone table with the climate zones"))?;
            let cz = raster("climate_zones", "the climate zone map")?;
            let (zid, vals) = lookup(tab, &MONTHS, "the climate zone table", "cz_id")?;
            let mut zone = vec![-1i64; n];
            let mut bad: Vec<i64> = Vec::new();
            for i in 0..n {
                if !cz[i].is_nan() {
                    let z = cz[i] as i64;
                    match zid.binary_search(&z) {
                        Ok(k) => zone[i] = k as i64,
                        Err(_) => bad.push(z),
                    }
                }
            }
            if !bad.is_empty() {
                bad.sort_unstable();
                bad.dedup();
                return Err(Error::msg(format!("the climate zone map has zone(s) {bad:?} that the table lacks")));
            }
            (0..12)
                .map(|m| zone.iter().map(|&k| if k >= 0 { vals[m][k as usize] as f32 } else { f32::NAN }).collect())
                .collect()
        };
        // the curve numbers (float64: the land use and soil maps hold integers)
        let mut groups_bad: Vec<i64> = Vec::new();
        let mut missing: Vec<i64> = Vec::new();
        let mut cn = vec![f64::NAN; n];
        for i in 0..n {
            if lu.valid[i] == 1 && !soil[i].is_nan() {
                let g = soil[i] as i64;
                if !(1..=4).contains(&g) {
                    groups_bad.push(g);
                    continue;
                }
                match codes.binary_search(&lu.codes[i]) {
                    Ok(k) => cn[i] = f64::from(bio[(g - 1) as usize][k] as f32),
                    Err(_) => missing.push(lu.codes[i]),
                }
            }
        }
        if !groups_bad.is_empty() {
            groups_bad.sort_unstable();
            groups_bad.dedup();
            return Err(Error::msg(format!(
                "the soil group map must hold only the groups 1, 2, 3 and 4 (found {groups_bad:?})"
            )));
        }
        if !missing.is_empty() {
            missing.sort_unstable();
            missing.dedup();
            return Err(Error::msg(format!(
                "the land use map has class(es) {missing:?} that the biophysical table lacks"
            )));
        }
        let si: Vec<f32> = (0..n)
            .map(|i| {
                if !cn[i].is_nan() && cn[i] > 0.0 {
                    ((1000.0 / cn[i] - 10.0) * f64::from(u8::from(stream[i] != 1))) as f32
                } else {
                    f32::NAN
                }
            })
            .collect();
        let p_typed: Vec<Vec<f64>> = p_list.iter().map(|p| typed(p, p_wide)).collect();
        let qf_months: Vec<Vec<f32>> =
            (0..12).map(|m| quickflow(&p_typed[m], p_wide, &si, &n_events[m], &stream)).collect();
        let mut qf = vec![f32::NAN; n];
        for i in 0..n {
            if qf_months.iter().all(|q| !q[i].is_nan()) {
                let mut s = qf_months[0][i];
                for q in &qf_months[1..] {
                    s += q[i];
                }
                qf[i] = s;
            }
        }
        let cls = lu.classes(&codes)?;
        let kc: Vec<f32> = (0..12)
            .flat_map(|m| {
                let v = &bio[4 + m];
                cls.iter().map(move |&c| if c >= 0 { v[c as usize] as f32 } else { f32::NAN })
            })
            .collect();
        let flat32 = |l: &[&[f64]]| -> Vec<f32> { l.iter().flat_map(|v| v.iter().map(|&x| x as f32)).collect() };
        let flat64 = |l: &[&[f64]]| -> Vec<f64> { l.iter().flat_map(|v| v.iter().copied()).collect() };
        let (p32, p64) = if p_wide { (Vec::new(), flat64(&p_list)) } else { (flat32(&p_list), Vec::new()) };
        let (e32, e64) = if e_wide { (Vec::new(), flat64(&e_list)) } else { (flat32(&e_list), Vec::new()) };
        let qf_flat: Vec<f32> = qf_months.concat();
        let alpha32: Vec<f32> = alpha_m.iter().map(|&a| a as f32).collect();
        let rec = ffi::serv_swy_local_recharge(
            &r.mfd,
            &r.d8,
            ny,
            nx,
            MFD_BLOCK,
            &p32,
            &p64,
            &e32,
            &e64,
            &qf_flat,
            &kc,
            &alpha32,
            beta as f32,
            gamma as f32,
        )?;
        let f32of = |v: &[f64]| -> Vec<f64> { v.iter().map(|&x| f64::from(x as f32)).collect() };
        l32 = f32of(&rec[..n]);
        la32 = f32of(&rec[n..2 * n]);
        out.insert("quickflow".into(), f32_map(&grid, &qf));
        out.insert("l_sum_avail".into(), f64_map(&grid, f32of(&rec[2 * n..3 * n])));
        out.insert("aet".into(), f64_map(&grid, f32of(&rec[3 * n..4 * n])));
        out.insert("annual_precipitation".into(), f64_map(&grid, f32of(&rec[4 * n..5 * n])));
        out.insert(
            "quickflow_monthly".into(),
            Value::Stack(Stack {
                grid: grid.clone(),
                keys: (1..=12).map(|m| Key::Name(m.to_string())).collect(),
                layers: qf_months.iter().map(|q| Arc::new(q.iter().map(|&x| f64::from(x)).collect())).collect(),
            }),
        );
        climate = Some((qf_months, p_typed));
    } else {
        let rv = raster("recharge", "the recharge map")?;
        l_wide = wide(&precision, rv);
        let lt = typed(rv, l_wide);
        let g32 = gamma as f32;
        la32 = lt
            .iter()
            .map(|&l| {
                if l.is_nan() {
                    f64::NAN
                } else if l_wide {
                    (gamma * l).min(l)
                } else {
                    f64::from((g32 * l as f32).min(l as f32))
                }
            })
            .collect();
        l32 = lt;
    }
    let vri = vri_of(&l32, l_wide, ny as usize, nx as usize);
    let lsum = r.accumulate(&l32)?;
    // missing recharge (a map with holes) counts nothing, as in the accumulation
    let zero = |v: &[f64]| -> Vec<f64> { v.iter().map(|&x| if x.is_nan() { 0.0 } else { x }).collect() };
    let b = ffi::serv_swy_baseflow(&r.mfd, &r.d8, ny, nx, MFD_BLOCK, &zero(&l32), &zero(&la32), &lsum, &stream)?;
    let b32: Vec<f64> = b[..n].iter().map(|&x| f64::from(x as f32)).collect();
    let bsum32: Vec<f64> = b[n..].iter().map(|&x| f64::from(x as f32)).collect();
    out.insert("watershed_table".into(), Value::Table(aggregate(&sheds, &grid, &l32, &vri)?));
    if let Some((qf_months, p_typed)) = &climate {
        out.insert(
            "monthly_table".into(),
            Value::Table(monthly_table(&sheds, &grid, &b32, qf_months, p_typed, pixel_area)?),
        );
    }
    out.insert("local_recharge".into(), f64_map(&grid, l32));
    out.insert("l_avail".into(), f64_map(&grid, la32));
    out.insert("l_sum".into(), f64_map(&grid, lsum));
    out.insert("baseflow".into(), f64_map(&grid, b32));
    out.insert("b_sum".into(), f64_map(&grid, bsum32));
    out.insert("vri".into(), f32_map(&grid, &vri));
    out.insert("stream".into(), f64_map(&grid, stream.iter().map(|&c| f64::from(c)).collect()));
    Ok(out)
}

/// InVEST's _calculate_vri: L over its sum, the sum taken by pygeoprocessing's blocks (float32 out).
fn vri_of(l: &[f64], l_wide: bool, ny: usize, nx: usize) -> Vec<f32> {
    let mut out = vec![f32::NAN; l.len()];
    if l_wide {
        let mut qb = 0.0f64;
        for (rows, cols) in iterblocks(ny, nx) {
            let blk: Vec<f64> = rows
                .flat_map(|y| cols.clone().map(move |x| y * nx + x))
                .map(|i| l[i])
                .filter(|v| !v.is_nan() && !v.is_infinite())
                .collect();
            qb += numpy_sum(&blk);
        }
        if qb > 0.0 {
            for (o, &v) in out.iter_mut().zip(l) {
                if !v.is_nan() {
                    *o = (v / qb) as f32;
                }
            }
        }
    } else {
        let mut qb = 0.0f32;
        for (rows, cols) in iterblocks(ny, nx) {
            let blk: Vec<f32> = rows
                .flat_map(|y| cols.clone().map(move |x| y * nx + x))
                .map(|i| l[i] as f32)
                .filter(|v| !v.is_nan() && !v.is_infinite())
                .collect();
            qb += sum_f32(&blk);
        }
        if qb > 0.0 {
            for (o, &v) in out.iter_mut().zip(l) {
                if !v.is_nan() {
                    *o = v as f32 / qb;
                }
            }
        }
    }
    out
}

/// The watersheds' ids (ws_id, else 1..).
fn ids_of(sheds: &Vector) -> Vec<f64> {
    attribute(sheds, "ws_id", "watersheds").unwrap_or_else(|_| (1..=sheds.len()).map(|i| i as f64).collect())
}

/// InVEST's _aggregate_recharge: the mean recharge qb and vri_sum per polygon, geom_id its row.
fn aggregate(sheds: &Vector, grid: &Grid, l: &[f64], vri: &[f32]) -> Result<Table> {
    let v: Vec<f64> = vri.iter().map(|&x| f64::from(x)).collect();
    let st = zonal(sheds, grid, &[l, &v], &[2, 0])?;
    let mut t = Table::new(["ws_id", "qb", "vri_sum", "geom_id"].map(String::from).to_vec());
    for (z, id) in ids_of(sheds).iter().enumerate() {
        let (s, c) = (st[z * 4], st[z * 4 + 1]);
        let vs = st[z * 4 + 2];
        t.rows.push(vec![*id, if c > 0.0 { s / c } else { 0.0 }, if vs.is_nan() { 0.0 } else { vs }, z as f64]);
    }
    Ok(t)
}

/// InVEST's _generate_monthly_qf_b_p_csv: cubic metres per polygon and month.
fn monthly_table(
    sheds: &Vector,
    grid: &Grid,
    b: &[f64],
    qf: &[Vec<f32>],
    p: &[Vec<f64>],
    pixel_area: f64,
) -> Result<Table> {
    let mut maps: Vec<Vec<f64>> = vec![b.to_vec()];
    maps.extend(qf.iter().map(|q| q.iter().map(|&x| f64::from(x)).collect::<Vec<_>>()));
    maps.extend(p.iter().cloned());
    let st = zonal(sheds, grid, &maps.iter().map(|m| m.as_slice()).collect::<Vec<_>>(), &[2])?;
    let nm = maps.len();
    let k = 0.001 * pixel_area;
    let z0 = |x: f64| if x.is_nan() { 0.0 } else { x };
    let mut t = Table::new(["geom_id", "month", "quickflow", "baseflow", "precipitation"].map(String::from).to_vec());
    for fid in 0..sheds.len() {
        let bflow = z0(st[fid * nm]) * k / 12.0;
        for m in 0..12 {
            t.rows.push(vec![
                fid as f64,
                (m + 1) as f64,
                z0(st[fid * nm + 1 + m]) * k,
                bflow,
                z0(st[fid * nm + 13 + m]) * k,
            ]);
        }
    }
    Ok(t)
}
