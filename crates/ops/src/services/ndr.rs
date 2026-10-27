//! Nutrient delivery ratio (InVEST's ndr model, natcap.invest 3.20) as landschaft 1.54 computes it
//! (`ls.services.ndr`): the nitrogen and phosphorus loads of the land uses, their effective retention down to the
//! streams (landschaft's C++) and the delivery ratio of the index of connectivity, every raster in InVEST's
//! float32 / float64 type and with its expression.

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use super::{LandUse, Routing, attribute, f32_map, f64_map, inside, lookup, polygons, table_of, thresholds, zonal};
use crate::hydro::MFD_BLOCK;
use crate::{Args, Ctx, Error, Operator, Outputs, Result, Table, Value, npmath};

pub(super) struct Ndr;

impl Operator for Ndr {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "services.ndr",
            "Nutrient export and retention: InVEST's nutrient delivery ratio model (natcap.invest 3.20) bit for \
             bit, through landschaft. The nitrogen and phosphorus loads of each land use (scaled by the runoff \
             proxy over its mean) split into a surface part and, for nitrogen, a dissolved subsurface part; the \
             surface load reaches a stream in the share NDR = (1 - eff') / (1 + exp((IC_0 - IC) / k)), eff' the \
             retention of the cells it crosses on its way down and IC the index of connectivity (Borselli et al. \
             2008), the subsurface load in the share 1 - eff_sub (1 - exp(-5 l / l_sub)). The DEM filled and \
             routed by pygeoprocessing's algorithms, as InVEST; the subsurface distance in metres (InVEST takes \
             it in cells).",
            Access::Global,
        )
        .input(PortSpec::new("dem", Type::Map, "elevations (projected, metres)"))
        .input(PortSpec::new("lulc", Type::Map, "land use (class codes)"))
        .input(PortSpec::new("runoff_proxy", Type::Map, "the capacity of the runoff to carry nutrients (any units)"))
        .input(PortSpec::new(
            "biophysical",
            Type::Table,
            "by class: lucode, load_n, eff_n, crit_len_n, proportion_subsurface_n (nitrogen); load_p, eff_p, \
             crit_len_p (phosphorus)",
        ))
        .input(PortSpec::new("watersheds", Type::Vector, "the watershed polygons (the model runs inside them)"))
        .output(PortSpec::new("n_surface_export", Type::Map, "the surface nitrogen that reaches the streams, kg/ha/yr"))
        .output(PortSpec::new("n_subsurface_export", Type::Map, "the dissolved nitrogen that reaches them"))
        .output(PortSpec::new("n_total_export", Type::Map, "surface plus subsurface nitrogen export"))
        .output(PortSpec::new("p_surface_export", Type::Map, "the phosphorus that reaches the streams, kg/ha/yr"))
        .output(PortSpec::new("ndr_n", Type::Map, "the surface delivery ratio of nitrogen"))
        .output(PortSpec::new("ndr_p", Type::Map, "the surface delivery ratio of phosphorus"))
        .output(PortSpec::new("effective_retention_n", Type::Map, "the nitrogen retained on its way down"))
        .output(PortSpec::new("effective_retention_p", Type::Map, "the phosphorus retained on its way down"))
        .output(PortSpec::new("ic", Type::Map, "the index of connectivity"))
        .output(PortSpec::new("stream", Type::Map, "the streams (1)"))
        .output(PortSpec::new("drains_to_stream", Type::Map, "1 where the flow reaches a stream"))
        .output(PortSpec::new(
            "watershed_table",
            Type::Table,
            "by watershed: the surface loads, the exports and the subsurface load (kg / yr)",
        ))
        .output(PortSpec::new("ic_0", Type::Number, "the IC_0 of the delivery ratio (half way between the extremes)"))
        .output(PortSpec::new("runoff_proxy_mean", Type::Number, "the mean the runoff proxy was divided by"))
        .param(PortSpec::new("threshold", Type::Number, "the flow accumulation (cells) of a stream"))
        .param(PortSpec::optional("k", Type::Number, "Borselli's k (2)"))
        .param(PortSpec::optional("nitrogen", Type::Bool, "model nitrogen (default true)"))
        .param(PortSpec::optional("phosphorus", Type::Bool, "model phosphorus (default true)"))
        .param(PortSpec::optional(
            "subsurface_critical_length",
            Type::Number,
            "the distance (m) over which the soil reaches its largest retention of dissolved nitrogen",
        ))
        .param(PortSpec::optional("subsurface_efficiency", Type::Number, "that largest retention, 0 to 1"))
        .param(PortSpec::optional("runoff_proxy_mean", Type::Number, "the proxy's mean (default: of the map)"))
        .param(PortSpec::optional(
            "load_type_n",
            Type::Any,
            "measured-runoff (default: the load as given) or application-rate (times 1 - eff_n), for every class; \
             or by class, {\"<lucode>\": type}",
        ))
        .param(PortSpec::optional("load_type_p", Type::Any, "the same for phosphorus"))
        .param(PortSpec::optional("method", Type::Text, "the routing: mfd (default) or d8").choices(["mfd", "d8"]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        ndr(args).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

/// Whether each class's load is an application rate: one type for all, or one per class code.
fn application_rates(args: &Args, port: &str, codes: &[i64]) -> Result<Vec<bool>> {
    let kind = |s: &str| -> Result<bool> {
        match s.trim().to_lowercase().as_str() {
            "application-rate" => Ok(true),
            "measured-runoff" => Ok(false),
            other => Err(Error::msg(format!("{port} is 'application-rate' or 'measured-runoff', not {other:?}"))),
        }
    };
    match args.params.get(port) {
        None | Some(serde_json::Value::Null) => Ok(vec![false; codes.len()]),
        Some(serde_json::Value::String(s)) => Ok(vec![kind(s)?; codes.len()]),
        Some(serde_json::Value::Object(by)) => codes
            .iter()
            .map(|c| match by.get(&c.to_string()) {
                Some(serde_json::Value::String(s)) => kind(s),
                _ => Err(Error::msg(format!("{port} lacks the class {c}"))),
            })
            .collect(),
        Some(_) => Err(Error::msg(format!("{port} is a type or an object of types by class code"))),
    }
}

/// pygeoprocessing.iterblocks over a raster tiled 256 x 256: its windows (rows, columns).
pub(super) fn iterblocks(ny: usize, nx: usize) -> Vec<(std::ops::Range<usize>, std::ops::Range<usize>)> {
    let (mut cols, mut rows) = (MFD_BLOCK as usize, MFD_BLOCK as usize);
    let largest = 1usize << 16;
    if largest / (cols * rows) > 0 {
        cols = (cols * (largest / (cols * rows))).min(nx);
    }
    if largest / (cols * rows) > 0 {
        rows = (rows * (largest / (cols * rows))).min(ny);
    }
    let mut out = Vec::new();
    for y0 in (0..ny).step_by(rows.max(1)) {
        for x0 in (0..nx).step_by(cols.max(1)) {
            out.push((y0..(y0 + rows).min(ny), x0..(x0 + cols).min(nx)));
        }
    }
    out
}

/// numpy's sum of a float32 array: pairwise (8 accumulators in blocks of up to 128, halves above), in float32.
pub(super) fn sum_f32(a: &[f32]) -> f32 {
    let n = a.len();
    if n < 8 {
        a.iter().fold(0.0, |s, x| s + x)
    } else if n <= 128 {
        let mut r = [0.0f32; 8];
        r.copy_from_slice(&a[..8]);
        let full = n - n % 8;
        for i in (8..full).step_by(8) {
            for j in 0..8 {
                r[j] += a[i + j];
            }
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        for x in &a[full..] {
            res += x;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        sum_f32(&a[..n2]) + sum_f32(&a[n2..])
    }
}

/// InVEST's _normalize_raster mean: float32 sums of the valid cells of each block, in the blocks' order.
fn proxy_mean(v: &[f32], ny: usize, nx: usize) -> f32 {
    let (mut total, mut count) = (0.0f32, 0usize);
    for (rows, cols) in iterblocks(ny, nx) {
        let data: Vec<f32> =
            rows.flat_map(|y| cols.clone().map(move |x| y * nx + x)).map(|i| v[i]).filter(|x| !x.is_nan()).collect();
        total += sum_f32(&data);
        count += data.len();
    }
    if count > 0 { total / count as f32 } else { total }
}

/// A number as pygeoprocessing stores it in a GeoTIFF's statistics: text with 14 significant digits.
fn g14(x: f64) -> f64 {
    format!("{x:.13e}").parse().unwrap_or(x)
}

fn ndr(args: &Args) -> Result<Outputs> {
    let method: String = args.param("method", "mfd".to_string())?.to_lowercase();
    if method != "mfd" && method != "d8" {
        return Err(Error::msg("method is 'mfd' or 'd8'"));
    }
    let threshold: f64 = args.opt_param("threshold")?.ok_or_else(|| Error::msg("give the threshold"))?;
    if threshold.is_nan() || threshold <= 0.0 {
        return Err(Error::msg("threshold must be positive"));
    }
    let k: f64 = args.param("k", 2.0)?;
    let nitrogen: bool = args.param("nitrogen", true)?;
    let phosphorus: bool = args.param("phosphorus", true)?;
    let nutrients: Vec<&str> =
        [("n", nitrogen), ("p", phosphorus)].iter().filter(|x| x.1).map(|x| x.0).collect::<Vec<_>>();
    if nutrients.is_empty() {
        return Err(Error::msg("model nitrogen, phosphorus or both"));
    }
    let (mut l_sub, mut e_sub) = (f64::NAN, f64::NAN);
    if nitrogen {
        let l: Option<f64> = args.opt_param("subsurface_critical_length")?;
        let e: Option<f64> = args.opt_param("subsurface_efficiency")?;
        let (Some(l), Some(e)) = (l, e) else {
            return Err(Error::msg("nitrogen needs the subsurface_critical_length and the subsurface_efficiency"));
        };
        if l.is_nan() || l <= 0.0 {
            return Err(Error::msg("the subsurface critical length must be positive"));
        }
        if !(0.0..=1.0).contains(&e) {
            return Err(Error::msg("the subsurface efficiency is between 0 and 1"));
        }
        (l_sub, e_sub) = (l, e);
    }
    let given_mean: Option<f64> = args.opt_param("runoff_proxy_mean")?;
    if given_mean.is_some_and(|m| m.is_nan() || m <= 0.0) {
        return Err(Error::msg("the runoff proxy mean must be positive"));
    }
    let lu = LandUse::new(args.map("lulc")?, "the land use map")?;
    let grid = lu.map.grid.clone();
    let (ny, nx) = (grid.height as i32, grid.width as i32);
    let n = grid.cells();
    let mut columns = Vec::new();
    for x in &nutrients {
        columns.extend([format!("load_{x}"), format!("eff_{x}"), format!("crit_len_{x}")]);
    }
    if nitrogen {
        columns.push("proportion_subsurface_n".into());
    }
    let cols: Vec<&str> = columns.iter().map(String::as_str).collect();
    let (codes, bio) = lookup(table_of(args, "biophysical")?, &cols, "the biophysical table", "lucode")?;
    let column = |name: &str| -> &Vec<f64> { &bio[cols.iter().position(|c| *c == name).unwrap()] };
    let mut values = Vec::new();
    for (port, what) in [("dem", "dem"), ("runoff_proxy", "runoff proxy")] {
        let m = args.map(port)?;
        if m.shape() != lu.map.shape() {
            return Err(Error::msg(format!(
                "the {what} raster has {:?} cells, the land use map {:?}",
                m.shape(),
                lu.map.shape()
            )));
        }
        values.push(&m.data);
    }
    // InVEST masks each input to the watersheds (cell centres) on its own
    let sheds = polygons(crate::vector::vector_of(args, "watersheds")?, &grid)?;
    let within = inside(&sheds, &grid)?;
    let masked = |v: &[f64]| -> Vec<f32> {
        (0..n).map(|i| if within[i] && !v[i].is_nan() { v[i] as f32 } else { f32::NAN }).collect()
    };
    let dem32 = masked(values[0]);
    let mut proxy = masked(values[1]);
    for p in proxy.iter_mut() {
        if *p == -1.0 {
            *p = f32::NAN; // the masked proxy's nodata
        }
    }
    let lulc_ok: Vec<u8> = (0..n).map(|i| u8::from(within[i] && lu.valid[i] == 1)).collect();
    lu.pixel()?;
    let t = grid.transform;
    let (xcell, ycell) = (t[1], t[5]);
    let cell_area = (xcell * ycell).abs();

    // routing and the index of connectivity
    let dem64: Vec<f64> = dem32.iter().map(|&z| f64::from(z)).collect();
    let filled: Vec<f64> = ffi::hydro_fill(&dem64, ny, nx, false)?.iter().map(|&z| f64::from(z as f32)).collect();
    let slope = ffi::serv_pgp_slope(&filled, ny, nx, xcell, ycell, super::threads())?;
    let slope_thr = thresholds(&slope);
    let r = Routing::new(&filled, ny, nx, &method)?;
    // InVEST's NDR keeps pygeoprocessing's default trace proportion (its SDR passes 0.7)
    let stream: Vec<u8> = if method == "mfd" {
        ffi::hydro_mfd_streams(&r.acc, &r.mfd, ny, nx, threshold, 1.0, MFD_BLOCK)?
    } else {
        r.acc.iter().map(|a| if a.is_nan() { 255 } else { u8::from(*a > threshold) }).collect()
    };
    let s_acc = r.accumulate(&slope_thr.iter().map(|&s| f64::from(s)).collect::<Vec<_>>())?;
    let s_bar: Vec<f32> = (0..n)
        .map(|i| if s_acc[i].is_nan() || r.acc[i].is_nan() { f32::NAN } else { (s_acc[i] / r.acc[i]) as f32 })
        .collect();
    let d_up: Vec<f32> = (0..n)
        .map(|i| {
            if s_bar[i].is_nan() || r.acc[i].is_nan() {
                f32::NAN
            } else {
                (f64::from(s_bar[i]) * (r.acc[i] * cell_area).sqrt()) as f32
            }
        })
        .collect();
    let s_inv: Vec<f64> = slope_thr
        .iter()
        .map(|&b| {
            if b.is_nan() {
                f64::NAN
            } else if b == 0.0 {
                0.0
            } else {
                f64::from(1.0 / b)
            }
        })
        .collect();
    let d_dn = r.distance(&stream, &s_inv)?;
    let dist = r.distance(&stream, &[])?;
    let drains: Vec<f64> =
        (0..n).map(|i| if r.missing(i) { 255.0 } else { f64::from(u8::from(!dist[i].is_nan())) }).collect();
    let mut ic = vec![f32::NAN; n];
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..n {
        let (u, d) = (d_up[i], d_dn[i]);
        if !u.is_nan() && !d.is_nan() && u != 0.0 && d != 0.0 {
            ic[i] = (f64::from(u) / d).log10() as f32;
            lo = lo.min(f64::from(ic[i]));
            hi = hi.max(f64::from(ic[i]));
        }
    }
    if lo > hi {
        return Err(Error::msg("no cell has an index of connectivity: no cell drains to a stream"));
    }
    // InVEST reads the IC's extremes from the statistics pygeoprocessing stores in the GeoTIFF, as text
    let ic_0 = (g14(lo) + g14(hi)) / 2.0;

    // the runoff proxy index
    let mean_out = given_mean.unwrap_or_else(|| f64::from(proxy_mean(&proxy, ny as usize, nx as usize)));
    let mean = mean_out as f32;
    let rpi: Vec<f32> = proxy.iter().map(|&a| if mean == 0.0 { a } else { a / mean }).collect();

    let index = lu.classes_of(&codes, &lulc_ok)?;
    let stream_ok = |i: usize| stream[i] != 255;
    let not_stream = |i: usize| f64::from(1u8.wrapping_sub(stream[i]));
    let pick32 = |v: &[f64]| -> Vec<f32> {
        index.iter().map(|&c| if c >= 0 { v[c as usize] as f32 } else { f32::NAN }).collect()
    };
    let mut maps: std::collections::BTreeMap<String, Vec<f32>> = Default::default();
    let mut wide: std::collections::BTreeMap<String, Vec<f64>> = Default::default();
    let mut retained = Vec::new();
    for x in &nutrients {
        let app = application_rates(args, &format!("load_type_{x}"), &codes)?;
        let (load, eff) = (column(&format!("load_{x}")), column(&format!("eff_{x}")));
        let load: Vec<f64> =
            (0..codes.len()).map(|c| if app[c] { load[c] * (1.0 - eff[c]) } else { load[c] }).collect();
        let load_map = pick32(&load);
        let modified: Vec<f32> = load_map.iter().zip(&rpi).map(|(a, b)| a * b).collect();
        let surface: Vec<f32> = if *x == "n" {
            let sub = column("proportion_subsurface_n");
            let mut surface = vec![f32::NAN; n];
            let mut sub_load = vec![f32::NAN; n];
            for i in 0..n {
                if !modified[i].is_nan() {
                    let s = sub[index[i] as usize];
                    surface[i] = (f64::from(modified[i]) * (1.0 - s)) as f32;
                    sub_load[i] = (f64::from(modified[i]) * s) as f32;
                }
            }
            maps.insert("sub_load_n".into(), sub_load);
            surface
        } else {
            modified.clone()
        };
        // the efficiency and critical length of each land use, 0 on the streams
        let by_class = |v: &[f64]| -> Vec<f32> {
            (0..n)
                .map(|i| {
                    if index[i] >= 0 && stream_ok(i) { (v[index[i] as usize] * not_stream(i)) as f32 } else { f32::NAN }
                })
                .collect()
        };
        let eff_map = by_class(eff);
        let crit_map = by_class(column(&format!("crit_len_{x}")));
        retained.push(ffi::serv_ndr_retention(
            &r.mfd,
            &r.d8,
            &stream,
            &eff_map,
            &crit_map,
            ny,
            nx,
            xcell.abs(),
            MFD_BLOCK,
        )?);
        maps.insert(format!("surface_load_{x}"), surface);
    }
    let (ic0, kk) = (ic_0 as f32, k as f32);
    for (x, ret) in nutrients.iter().zip(retained) {
        let ratio: Vec<f32> = (0..n)
            .map(|i| {
                if ret[i].is_nan() || ic[i].is_nan() {
                    f32::NAN
                } else {
                    (1.0 - ret[i]) / (1.0 + npmath::exp((ic0 - ic[i]) / kk))
                }
            })
            .collect();
        let export: Vec<f32> = maps[&format!("surface_load_{x}")].iter().zip(&ratio).map(|(a, b)| a * b).collect();
        maps.insert(format!("effective_retention_{x}"), ret);
        maps.insert(format!("ndr_{x}"), ratio);
        maps.insert(format!("{x}_surface_export"), export);
    }
    if nitrogen {
        let cell = xcell.abs();
        let sub_ndr: Vec<f64> = dist
            .iter()
            .map(|&dc| if dc.is_nan() { f64::NAN } else { 1.0 - e_sub * (1.0 - (-5.0 * (dc * cell) / l_sub).exp()) })
            .collect();
        let sub_export: Vec<f64> = maps["sub_load_n"].iter().zip(&sub_ndr).map(|(a, b)| f64::from(*a) * b).collect();
        let total: Vec<f64> =
            maps["n_surface_export"].iter().zip(&sub_export).map(|(a, b)| f64::from(*a) + b).collect();
        wide.insert("n_subsurface_export".into(), sub_export);
        wide.insert("n_total_export".into(), total);
    }

    // the totals by watershed, kg / yr
    let fields: Vec<(&str, &str)> = nutrients
        .iter()
        .flat_map(|x| match *x {
            "n" => vec![
                ("n_surface_load", "surface_load_n"),
                ("n_surface_export", "n_surface_export"),
                ("n_subsurface_export", "n_subsurface_export"),
                ("n_total_export", "n_total_export"),
                ("n_subsurface_load", "sub_load_n"),
            ],
            _ => vec![("p_surface_load", "surface_load_p"), ("p_surface_export", "p_surface_export")],
        })
        .collect();
    let report: Vec<Vec<f64>> = fields
        .iter()
        .map(|(_, key)| match wide.get(*key) {
            Some(v) => v.clone(),
            None => maps[*key].iter().map(|&v| f64::from(v)).collect(),
        })
        .collect();
    let sums = zonal(&sheds, &grid, &report.iter().map(|v| v.as_slice()).collect::<Vec<_>>(), &[2])?;
    let ids =
        attribute(&sheds, "ws_id", "watersheds").unwrap_or_else(|_| (1..=sheds.len()).map(|i| i as f64).collect());
    let ha = cell_area / 10000.0;
    let mut table = Table::new(std::iter::once("ws_id").chain(fields.iter().map(|f| f.0)).map(String::from).collect());
    let nf = fields.len();
    for (z, id) in ids.iter().enumerate() {
        table.rows.push(std::iter::once(*id).chain((0..nf).map(|k| sums[z * nf + k] * ha)).collect());
    }

    let mut out = Outputs::new();
    for x in &nutrients {
        for key in [format!("{x}_surface_export"), format!("ndr_{x}"), format!("effective_retention_{x}")] {
            out.insert(key.clone(), f32_map(&grid, &maps[&key]));
        }
    }
    for (key, v) in wide {
        out.insert(key, f64_map(&grid, v));
    }
    out.insert("ic".into(), f32_map(&grid, &ic));
    out.insert("stream".into(), f64_map(&grid, stream.iter().map(|&c| f64::from(c)).collect()));
    out.insert("drains_to_stream".into(), f64_map(&grid, drains));
    out.insert("watershed_table".into(), Value::Table(table));
    out.insert("ic_0".into(), Value::Number(ic_0));
    out.insert("runoff_proxy_mean".into(), Value::Number(mean_out));
    Ok(out)
}
