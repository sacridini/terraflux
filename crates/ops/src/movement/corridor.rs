//! Corridors seen in tracks and the distance between utilisation distributions (the library's `tf.move`, from R's
//! move2utils 0.4.6, Kranstauber, Safi and Scharf 2026): LaPoint et al.'s (2013) corridor segments, the earth mover's
//! distance of Rubner et al. (2000) by Cuturi's (2013) Sinkhorn iterations or the exact solver of R's emdist. The
//! kernels are the library's C++ (`terraflux-ls-sys`); the segments (and the products of Sinkhorn's matrices) run in
//! parallel there, so the result does not depend on the number of threads.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::{
    Times, check_order, codes, column_params, columns, fail, give, is_lonlat, set_column, speed_factor, take, threads,
    tracks_of,
};
use crate::{Args, Ctx, Operator, Outputs, Registry, Result, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Corridor));
    r.add(Arc::new(UdDistance));
}

const LABELS: [&str; 2] = ["corridor", "not corridor"];

/// R's `quantile(x, p, type = 7)` of values without missing ones (NaN for none).
fn quantile7(x: &[f64], p: f64) -> f64 {
    let mut x = x.to_vec();
    x.sort_by(|a, b| a.total_cmp(b));
    let n = x.len();
    if n == 0 {
        return f64::NAN;
    }
    let index = 1.0 + (n - 1) as f64 * p;
    let (lo, hi) = (index.floor() as usize, index.ceil() as usize);
    let mut qs = x[lo - 1];
    if index > lo as f64 && x[hi - 1] != qs {
        let h = index - lo as f64;
        qs = (1.0 - h) * qs + h * x[hi - 1];
    }
    qs
}

/// Python's `format(x, ".{sig}g")`.
fn gfmt(x: f64, sig: usize) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.into();
    }
    if x == 0.0 {
        return "0".into();
    }
    let sci = format!("{:.*e}", sig - 1, x);
    let (mantissa, exp) = sci.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let trim = |s: &str| -> String {
        if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s.to_string() }
    };
    if exp < -4 || exp >= sig as i32 {
        format!("{}e{}{:02}", trim(mantissa), if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        trim(&format!("{:.*}", (sig as i32 - 1 - exp).max(0) as usize, x))
    }
}

/// A threshold setting: a finite number, or none for the quantile of the tracks.
fn threshold(args: &Args, name: &str) -> Result<Option<f64>> {
    match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) if s.is_empty() || s.eq_ignore_ascii_case("none") => Ok(None),
        Some(_) => {
            let x: f64 = args.param(name, f64::NAN)?;
            if !x.is_finite() {
                return Err(fail(
                    args.node,
                    format!("{name}: a finite number, or none for the quantile of the tracks"),
                ));
            }
            Ok(Some(x))
        }
    }
}

/// `move.corridor`.
struct Corridor;

impl Operator for Corridor {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.corridor",
                "Mark the segments of the tracks that run along a corridor (move2utils' mt_corridor, LaPoint et al. \
                 2013). A segment is the step from a location to the next of its track. It is a corridor segment when \
                 (1) it is fast (its speed, m/s, is at least speed_threshold), (2) the directions of the segments \
                 around it agree (the circular variance of their pseudo-azimuths, (2 azimuth) mod 2 pi, so that two \
                 animals walking the same path in opposite directions agree, is at most circvar_threshold): the \
                 neighbours are the segments, of all tracks together, whose midpoints lie within half its own length \
                 from its midpoint (the circle of that radius as GEOS draws it, a polygon of 120 vertices, the \
                 boundary included), itself included, and (3) it has at least min_segments neighbours and strictly \
                 more neighbours that satisfy 1 and 2 than not. A threshold left out is a quantile of the tracks (the \
                 0.75 quantile of the speeds, the 0.25 quantile of the circular variances, R's type 7), with a \
                 warning: for corridors that can be compared between animals, give the thresholds, computed once on \
                 all of them. The search radius assumes a regular sampling interval (a warning where it is very \
                 irregular). Longitude / latitude is projected to a local azimuthal equidistant projection \
                 (move.aeqd_crs) for the neighbourhoods and the directions. Columns added to each location, for the \
                 segment leaving it (none at the last location of a track and at empty locations): corridor (\"corridor\" \
                 or \"not corridor\"), corridor_speed (m/s), corridor_azimuth (degrees, on the working projection), \
                 corridor_circvar (none with fewer than two neighbours) and corridor_n_neighbours. The tracks need a \
                 coordinate system.",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks: points, each track's rows consecutive in time"))
            .output(PortSpec::new("tracks", Type::Vector, "the tracks with the columns of the corridors"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .output(PortSpec::new("lines", Type::Vector, "the corridor segments as two-point lines, with their columns"))
            .output(PortSpec::new("speed_threshold", Type::Number, "the speed threshold used, m/s"))
            .output(PortSpec::new("circvar_threshold", Type::Number, "the circular variance threshold used"))
            .output(PortSpec::new(
                "thresholds",
                Type::Table,
                "a row: speed_threshold, circvar_threshold, min_segments",
            ))
            .param(PortSpec::optional(
                "speed_threshold",
                Type::Number,
                "the speed (m/s) from which a segment is fast (the 0.75 quantile of the speeds)",
            ))
            .param(PortSpec::optional(
                "circvar_threshold",
                Type::Number,
                "the circular variance, in [0, 1], up to which the directions around a segment agree (the 0.25 \
                 quantile of the variances)",
            ))
            .param(PortSpec::optional(
                "min_segments",
                Type::Number,
                "the fewest neighbours (all of them, the segment itself included) of a corridor segment (2)",
            )),
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let speed_threshold = threshold(args, "speed_threshold")?;
        let circvar_threshold = threshold(args, "circvar_threshold")?;
        let min_segments: f64 = args.param("min_segments", 2.0)?;
        let (t, k) = columns(args, v)?;
        let n = v.len();
        // the empty locations are left out of the computation
        let mut keep = Vec::with_capacity(n);
        let (mut x, mut y) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for (i, g) in v.geometries.iter().enumerate() {
            match g {
                Geometry::Empty => {}
                Geometry::Point(p) => {
                    keep.push(i);
                    x.push(p[0]);
                    y.push(p[1]);
                }
                _ => return Err(fail(node, "the geometry must be points")),
            }
        }
        if keep.len() < 2 {
            return Err(fail(node, "need at least 2 non-empty locations for corridor detection"));
        }
        if !(x.iter().all(|a| a.is_finite()) && y.iter().all(|a| a.is_finite())) {
            return Err(fail(node, "some locations have non-finite coordinates: remove them first"));
        }
        let code = codes(&take(v.column(&k).unwrap(), &keep));
        let times = Times::of(&take(v.column(&t).unwrap(), &keep)).map_err(|e| fail(node, e))?;
        check_order(&code, Some(&times.seconds()), false).map_err(|e| fail(node, e))?;
        let Some(crs) = v.crs.as_deref() else {
            return Err(fail(node, "the tracks have no CRS: the speeds (m/s) and the search radius need one"));
        };
        let lonlat = is_lonlat(Some(crs)).map_err(|e| fail(node, e))?;

        // the segments: from each location to the next of its track
        let m = keep.len();
        let seg: Vec<usize> = (0..m - 1).filter(|&i| code[i + 1] == code[i]).collect();
        if seg.len() < 2 {
            return Err(fail(node, "need at least 2 segments across all tracks for corridor detection"));
        }
        let dist = terraflux_ls_sys::ffi::move_distances(&x, &y, &code, lonlat, threads())?;
        let lag: Vec<f64> =
            (0..m).map(|i| if i + 1 < m && code[i + 1] == code[i] { times.lag(i, i + 1) } else { f64::NAN }).collect();
        let (length, duration) = speed_factor(Some("m/s"), Some(crs), lonlat).map_err(|e| fail(node, e))?;
        let speed: Vec<f64> = seg
            .iter()
            .map(|&i| {
                let s = dist[i] / lag[i];
                if (length, duration) != (1.0, 1.0) { s / length * duration } else { s }
            })
            .collect();
        // the buffer's radius is half the segment's length in the units of the working coordinates (metres for
        // longitude / latitude, the CRS's units otherwise)
        let radius: Vec<f64> = seg.iter().map(|&i| dist[i] / 2.0).collect();

        // irregular sampling: the search radius assumes roughly uniform sampling
        let mut tl: Vec<f64> = lag.iter().copied().filter(|l| l.is_finite() && *l > 0.0).collect();
        if tl.len() >= 2 {
            tl.sort_by(|a, b| a.total_cmp(b));
            let med =
                if tl.len() % 2 == 1 { tl[tl.len() / 2] } else { (tl[tl.len() / 2 - 1] + tl[tl.len() / 2]) / 2.0 };
            let iqr = quantile7(&tl, 0.75) - quantile7(&tl, 0.25);
            if med > 0.0 && iqr / med > 1.0 {
                ctx.warn(format!(
                    "node {node:?}: Sampling interval is highly irregular (IQR / median = {}). The half-segment-length \
                     search radius assumes roughly uniform sampling; corridor flags on very short or very long \
                     segments may be misleading.",
                    gfmt(iqr / med, 2)
                ));
            }
        }

        // the working coordinates: a local projection for longitude / latitude
        let (mut wx, mut wy) = (x.clone(), y.clone());
        if lonlat {
            let work = super::geometry::aeqd(v, &"centroid".into(), "m").map_err(|e| fail(node, e))?;
            terraflux_gdal::Transform::new(crs, &work).map_err(|e| fail(node, e))?.points(&mut wx, &mut wy);
            if !(wx.iter().all(|a| a.is_finite()) && wy.iter().all(|a| a.is_finite())) {
                return Err(fail(node, "some locations cannot be projected to the local projection"));
            }
        }
        let pick = |a: &[f64], d: usize| -> Vec<f64> { seg.iter().map(|&i| a[i + d]).collect() };
        let nb = terraflux_ls_sys::ffi::move_corridor_neighbourhoods(
            &pick(&wx, 0),
            &pick(&wy, 0),
            &pick(&wx, 1),
            &pick(&wy, 1),
            &radius,
            threads(),
        )?;
        let circvar = &nb.circvar;
        let n_nb: Vec<f64> = nb.offset.windows(2).map(|w| (w[1] - w[0]) as f64).collect();

        let speed_threshold = speed_threshold.unwrap_or_else(|| {
            let valid: Vec<f64> = speed.iter().copied().filter(|s| !s.is_nan()).collect();
            let q = quantile7(&valid, 0.75);
            ctx.warn(format!(
                "node {node:?}: `speed_threshold` not supplied; using within-object 0.75 quantile of segment speed = \
                 {} m/s. For cross-individual or cross-population comparability, supply an explicit value.",
                gfmt(q, 4)
            ));
            q
        });
        let circvar_threshold = circvar_threshold.unwrap_or_else(|| {
            let valid: Vec<f64> = circvar.iter().copied().filter(|s| !s.is_nan()).collect();
            let q = if valid.len() >= 2 { quantile7(&valid, 0.25) } else { f64::NAN };
            ctx.warn(format!(
                "node {node:?}: `circvar_threshold` not supplied; using within-object 0.25 quantile of valid circular \
                 variance = {}. For cross-individual or cross-population comparability, supply an explicit value.",
                gfmt(q, 4)
            ));
            q
        });

        let fast: Vec<bool> = speed.iter().map(|&s| !s.is_nan() && s >= speed_threshold).collect();
        let is_corridor: Vec<bool> = if circvar_threshold.is_nan() {
            // R: cv <= NA is NA, so a fast segment with a variance makes `if (any(is_candidate))` fail
            if fast.iter().zip(circvar).any(|(&f, c)| f && !c.is_nan()) {
                return Err(fail(
                    node,
                    "the circular variance threshold is missing (fewer than two valid variances): give \
                     circvar_threshold",
                ));
            }
            vec![false; seg.len()]
        } else {
            let candidate: Vec<u8> =
                fast.iter().zip(circvar).map(|(&f, &c)| u8::from(f && !c.is_nan() && c <= circvar_threshold)).collect();
            if candidate.contains(&1) {
                let flags = terraflux_ls_sys::ffi::move_corridor_flags(
                    &nb.offset,
                    &nb.index,
                    &candidate,
                    min_segments,
                    threads(),
                )?;
                flags.iter().map(|&f| f != 0).collect()
            } else {
                vec![false; seg.len()]
            }
        };

        // one value per input row
        let rows: Vec<usize> = seg.iter().map(|&i| keep[i]).collect();
        let mut label = vec![LABELS[1].to_string(); n];
        let spread = |values: &[f64]| -> Column {
            let mut c = vec![f64::NAN; n];
            for (&r, &s) in rows.iter().zip(values) {
                c[r] = s;
            }
            Column::Numbers(c)
        };
        for (&r, &is) in rows.iter().zip(&is_corridor) {
            if is {
                label[r] = LABELS[0].to_string();
            }
        }
        let azimuth: Vec<f64> = nb.azimuth.iter().map(|a| a * 180.0 / std::f64::consts::PI).collect();
        let (c_speed, c_azimuth, c_circvar, c_nb) = (spread(&speed), spread(&azimuth), spread(circvar), spread(&n_nb));
        let mut out = (**v).clone();
        set_column(&mut out, "corridor", Column::Texts(label));
        set_column(&mut out, "corridor_speed", c_speed.clone());
        set_column(&mut out, "corridor_azimuth", c_azimuth.clone());
        set_column(&mut out, "corridor_circvar", c_circvar.clone());
        set_column(&mut out, "corridor_n_neighbours", c_nb.clone());

        // the corridor segments as lines: from the location to the next one of its track
        let flagged: Vec<usize> = (0..seg.len()).filter(|&s| is_corridor[s]).collect();
        let starts: Vec<usize> = flagged.iter().map(|&s| rows[s]).collect();
        let geometries = flagged
            .iter()
            .map(|&s| match (&v.geometries[rows[s]], &v.geometries[keep[seg[s] + 1]]) {
                (Geometry::Point(a), Geometry::Point(b)) => Geometry::LineString(vec![*a, *b]),
                _ => Geometry::Empty,
            })
            .collect();
        let attributes = vec![
            (k.clone(), take(v.column(&k).unwrap(), &starts)),
            (t.clone(), take(v.column(&t).unwrap(), &starts)),
            ("speed".to_string(), take(&c_speed, &starts)),
            ("azimuth".to_string(), take(&c_azimuth, &starts)),
            ("circvar".to_string(), take(&c_circvar, &starts)),
            ("n_neighbours".to_string(), take(&c_nb, &starts)),
        ];
        let lines = Vector { geometries, attributes, crs: v.crs.clone() };

        let mut thresholds =
            Table::new(["speed_threshold", "circvar_threshold", "min_segments"].map(String::from).to_vec());
        thresholds.rows.push(vec![speed_threshold, circvar_threshold, min_segments]);
        let mut o = give("tracks", out);
        o.insert("lines".into(), Value::Vector(Arc::new(lines)));
        o.insert("speed_threshold".into(), Value::Number(speed_threshold));
        o.insert("circvar_threshold".into(), Value::Number(circvar_threshold));
        o.insert("thresholds".into(), Value::Table(thresholds));
        Ok(o)
    }
}

/// The grid and the UDs (name, values) of a layer each.
type Uds = (Arc<Grid>, Vec<(String, Arc<Vec<f64>>)>);

/// The UDs of a port: (name, values) of each layer on one grid. A map is one UD, a stack one a layer.
fn uds_of(node: &str, port: &str, value: &Value, first: usize) -> Result<Uds> {
    let value = crate::files::as_port(value.clone(), Type::Any)?;
    match &value {
        Value::Map(m) => Ok((m.grid.clone(), vec![(format!("ud_{}", first + 1), m.data.clone())])),
        Value::Stack(s) => Ok((
            s.grid.clone(),
            s.keys
                .iter()
                .zip(&s.layers)
                .enumerate()
                .map(|(i, (key, l))| {
                    let name = match key {
                        crate::Key::Name(n) if !n.is_empty() => n.clone(),
                        _ => format!("ud_{}", first + i + 1),
                    };
                    (name, l.clone())
                })
                .collect(),
        )),
        other => Err(fail(node, format!("{port}: a raster with a band for each UD, not a {}", other.ty().name()))),
    }
}

/// `move.ud_distance`.
struct UdDistance;

impl Operator for UdDistance {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.ud_distance",
            "The earth mover's distance between utilisation distributions (move2utils' emd): the cost of moving one \
             UD's probability onto the other's, the sum over pairs of cells of the probability moved times the \
             distance between the cells' centres, least over all ways of moving it (Wasserstein-1, Rubner et al. \
             2000). The UDs must share one grid: the bands of a raster (one per track, such as a dBBMM UD of several \
             tracks) or two rasters (ud and ud_b, one UD each). Each UD is first cut to the cells inside its \
             mask_quantile volume contour (the cells holding the most probability, together that much of it) and \
             its weights renormalised. method sinkhorn (default) is Cuturi's (2013) entropic regularisation: the \
             distances scaled to [0, 1] by their largest, reg the regularisation, iterated until the scaling vector \
             changes less than tol or max_iter times; close to the exact value (a fraction of a percent on typical \
             UDs) and far faster. method exact is Rubner's transportation solver as R's emdist has it (Euclidean \
             cost, the weights and coordinates in single precision, at most exact_max_iter improving iterations): \
             practical on coarse grids only, its memory grows with the square of the cells (the cells of both UDs \
             together). The distance is in the grid's units (metres with gc); gc uses great-circle (haversine, on a \
             sphere of 6 378 137 m) distances for UDs on longitude / latitude grids (a warning for such a grid \
             without it). threshold clips the cost at a distance (the EMD-hat of Pele and Werman 2009; not with \
             exact). For two UDs the distance is a number; the table has a row for each pair of bands (each UD \
             before the ones after it), with their names.",
            Access::None,
        )
        .input(PortSpec::new("ud", Type::Any, "the UDs: a raster with a band for each, or one UD with ud_b"))
        .input(PortSpec::optional("ud_b", Type::Any, "the other UD, on the same grid, to give a number"))
        .output(PortSpec::new("table", Type::Table, "a row for each pair: a, b (the names of the bands) and distance"))
        .output(PortSpec::optional("distance", Type::Number, "the distance, when there are two UDs"))
        .param(PortSpec::optional("method", Type::Text, "sinkhorn (default) or exact").choices(["sinkhorn", "exact"]))
        .param(PortSpec::optional(
            "mask_quantile",
            Type::Number,
            "in (0, 1]: the cells outside this volume contour are dropped (0.999); 1 keeps (almost) all",
        ))
        .param(PortSpec::optional(
            "reg",
            Type::Number,
            "Sinkhorn's regularisation of the distance scaled to [0, 1] (0.05): smaller is closer to the exact \
             value and needs more iterations (below 0.01 it may underflow)",
        ))
        .param(PortSpec::optional("max_iter", Type::Integer, "Sinkhorn's most iterations (500)"))
        .param(PortSpec::optional(
            "tol",
            Type::Number,
            "Sinkhorn stops when the scaling vector changes (largest absolute difference) less than this (1e-9)",
        ))
        .param(PortSpec::optional(
            "gc",
            Type::Bool,
            "great-circle distances in metres, for UDs on longitude / latitude grids (false)",
        ))
        .param(PortSpec::optional(
            "threshold",
            Type::Number,
            "clip the cost at this distance (metres with gc, else the grid's units); Sinkhorn only",
        ))
        .param(PortSpec::optional(
            "exact_max_iter",
            Type::Integer,
            "the most iterations of the exact solver (500)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        use terraflux_ls_sys::ffi;
        let node = args.node;
        let method: String = args.param("method", "sinkhorn".to_string())?;
        if method != "sinkhorn" && method != "exact" {
            return Err(fail(node, "method: \"sinkhorn\" or \"exact\""));
        }
        let mask_quantile: f64 = args.param("mask_quantile", 0.999)?;
        if !(mask_quantile > 0.0 && mask_quantile <= 1.0) {
            return Err(fail(node, "mask_quantile: a number in (0, 1]"));
        }
        let reg: f64 = args.param("reg", 0.05)?;
        if reg.is_nan() || reg <= 0.0 {
            return Err(fail(node, "reg: a positive number"));
        }
        let max_iter: i64 = args.param("max_iter", 500)?;
        let exact_max_iter: i64 = args.param("exact_max_iter", 500)?;
        if max_iter < 1 || exact_max_iter < 1 {
            return Err(fail(node, "max_iter: at least 1"));
        }
        let (max_iter, exact_max_iter) =
            (max_iter.min(i32::MAX as i64) as i32, exact_max_iter.min(i32::MAX as i64) as i32);
        let tol: f64 = args.param("tol", 1e-9)?;
        let gc: bool = args.param("gc", false)?;
        let thr: Option<f64> = args.opt_param("threshold")?;
        if thr.is_some_and(|t| !(t.is_finite() && t > 0.0)) {
            return Err(fail(node, "threshold: a positive number or none"));
        }
        if method == "exact" && gc {
            return Err(fail(
                node,
                "method \"exact\" uses the Euclidean distance and cannot honour gc = true: use method \"sinkhorn\"",
            ));
        }
        if method == "exact" && thr.is_some() {
            return Err(fail(node, "method \"exact\" cannot honour threshold: use method \"sinkhorn\""));
        }
        let ud = args.get("ud").ok_or_else(|| fail(node, "no input \"ud\""))?;
        let (grid, mut layers) = uds_of(node, "ud", ud, 0)?;
        let mut pair = layers.len() == 2;
        if let Some(b) = args.get("ud_b") {
            let (grid_b, other) = uds_of(node, "ud_b", b, 1)?;
            if layers.len() != 1 || other.len() != 1 {
                return Err(fail(node, "with ud_b, ud and ud_b are one UD each"));
            }
            if !grid.same_as(&grid_b) {
                return Err(fail(node, "All UDs must share the same raster geometry."));
            }
            layers.extend(other);
            pair = true;
        }
        if layers.len() < 2 {
            return Err(fail(node, "need at least two UDs"));
        }
        let t = grid.transform;
        if t[2] != 0.0 || t[4] != 0.0 {
            return Err(fail(node, "rotated rasters are not supported"));
        }
        if let Some(crs) = crate::prepare::crs_of(&grid)
            && !gc
            && terraflux_gdal::is_geographic(&crs).unwrap_or(false)
        {
            ctx.warn(format!(
                "node {node:?}: Input UDs appear to be on a longitude/latitude grid but `gc` is false. Euclidean \
                 distance on degrees mixes north-south metres with cosine-shrunk east-west metres and is not \
                 meaningful at non-equatorial latitudes. Either project the UDs to a planar CRS first, or set gc to \
                 true to use Haversine distances."
            ));
        }
        // the cells' centres
        let xs: Vec<f64> = (0..grid.width).map(|j| t[0] + (j as f64 + 0.5) * t[1]).collect();
        let ys: Vec<f64> = (0..grid.height).map(|i| t[3] + (i as f64 + 0.5) * t[5]).collect();
        let mut prepared = Vec::with_capacity(layers.len());
        for (name, values) in &layers {
            let c = ffi::move_ud_prepare(values, mask_quantile).map_err(|e| fail(node, format!("{name}: {e}")))?;
            let cx: Vec<f64> = c.cells.iter().map(|&k| xs[k as usize % grid.width]).collect();
            let cy: Vec<f64> = c.cells.iter().map(|&k| ys[k as usize / grid.width]).collect();
            prepared.push((cx, cy, c.weights));
        }
        let names: Vec<String> = layers.iter().map(|l| l.0.clone()).collect();
        let mut table = Table::new(["a", "b", "distance"].map(String::from).to_vec());
        table.labels.insert("a".into(), names.clone());
        table.labels.insert("b".into(), names);
        for i in 0..layers.len() - 1 {
            for j in i + 1..layers.len() {
                let ((ax, ay, p), (bx, by, q)) = (&prepared[i], &prepared[j]);
                let d = if method == "sinkhorn" {
                    ffi::move_emd_sinkhorn(
                        ax,
                        ay,
                        p,
                        bx,
                        by,
                        q,
                        gc,
                        thr.unwrap_or(f64::NAN),
                        reg,
                        max_iter,
                        tol,
                        threads(),
                    )?
                } else {
                    let r = ffi::move_emd_exact(ax, ay, p, bx, by, q, exact_max_iter)?;
                    if r.hit_max_iter {
                        ctx.warn(format!(
                            "node {node:?}: emd: Maximum number of iterations has been reached ({exact_max_iter})"
                        ));
                    }
                    r.distance
                };
                table.rows.push(vec![(i + 1) as f64, (j + 1) as f64, d]);
            }
        }
        let mut o = crate::outputs([("table", Value::Table(table.clone()))]);
        if pair {
            o.insert("distance".into(), Value::Number(table.rows[0][2]));
        }
        Ok(o)
    }
}
