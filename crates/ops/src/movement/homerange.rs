//! Home ranges of tracks (the library's `tf.move`, from R's amt 0.3.1, Signer, Fieberg and Avgar 2019): minimum convex
//! polygons, kernel density estimates (KernSmooth's `bkde2D`, with the bandwidths of `hr_kde_ref`, `hr_kde_ref_scaled`,
//! `hr_kde_pi` and `hr_kde_lscv`), local convex hulls, the overlap indices of Fieberg and Kochanny (2005) and the
//! intensity of use. The numbers (R's `chull`, `quantile`, KernSmooth and its fft, `contourLines`, the neighbours of
//! LoCoH, the areas of GDAL's OGR) are the library's C++ (`terraflux-ls-sys`), the polygons' unions, hulls, buffers and
//! intersections GEOS's (the library the library's shapely loads; the GEOS of GDAL here, so a version apart may differ
//! in the last digits of LoCoH's and the overlaps' areas). Each track is estimated alone; the cells of a kernel
//! estimate are those of one template for all of them, so that their UDs and overlaps can be compared.

use std::sync::Arc;

use terraflux_gdal::geos::{Geos, Poly, Ring};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::{TRACK_NAMES, codes, fail, is_lonlat, threads, tracks_of};
use crate::{Args, Ctx, Key, Operator, Outputs, Registry, Result, Stack, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(HomeRange));
    r.add(Arc::new(HomeRangeOverlap));
    r.add(Arc::new(IntensityUse));
}

/// The settings (and the string) of an error inside the helpers, which the operators tag with their node.
type Res<T> = std::result::Result<T, String>;

fn ffi_err(e: cxx::Exception) -> String {
    e.what().to_string()
}

// ---------------------------------------------------------------------------------------------- the tracks

struct Track {
    rows: Vec<usize>,
    x: Vec<f64>,
    y: Vec<f64>,
}

struct Tracks {
    items: Vec<Track>,
    /// the name of the track column (`track` without one)
    name: String,
    column: Option<Column>,
}

fn number_label(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") }
}

impl Tracks {
    fn label(&self, k: usize) -> String {
        match &self.column {
            Some(Column::Texts(t)) => t[self.items[k].rows[0]].clone(),
            Some(Column::Numbers(n)) => number_label(n[self.items[k].rows[0]]),
            None => "1".to_string(),
        }
    }

    fn labels(&self) -> Vec<String> {
        (0..self.items.len()).map(|k| self.label(k)).collect()
    }

    /// The track column of features of the tracks `ks`.
    fn label_column(&self, ks: &[usize]) -> Column {
        match &self.column {
            Some(Column::Texts(t)) => Column::Texts(ks.iter().map(|&k| t[self.items[k].rows[0]].clone()).collect()),
            Some(Column::Numbers(n)) => Column::Numbers(ks.iter().map(|&k| n[self.items[k].rows[0]]).collect()),
            None => Column::Numbers(vec![1.0; ks.len()]),
        }
    }

    fn all_xy(&self) -> (Vec<f64>, Vec<f64>) {
        (
            self.items.iter().flat_map(|t| t.x.iter().copied()).collect(),
            self.items.iter().flat_map(|t| t.y.iter().copied()).collect(),
        )
    }
}

/// The tracks of a port, each as its locations in the order they appear (the tracks in the order they first appear):
/// points with a location in a projected coordinate system (amt's areas of longitude / latitude come from s2 on a
/// plane computed in degrees: project them first), a track column by its setting or its usual names, or one track.
fn read_tracks(args: &Args, v: &Vector, what: &str) -> Result<Tracks> {
    let node = args.node;
    if v.is_empty() {
        return Err(fail(node, "tracks: no locations"));
    }
    let (mut x, mut y) = (Vec::with_capacity(v.len()), Vec::with_capacity(v.len()));
    for g in &v.geometries {
        match g {
            Geometry::Point(p) => {
                x.push(p[0]);
                y.push(p[1]);
            }
            Geometry::Empty => {
                return Err(fail(node, format!("some locations are empty: drop them before {what} (vector.filter)")));
            }
            _ => return Err(fail(node, "the geometry must be points")),
        }
    }
    if !(x.iter().all(|a| a.is_finite()) && y.iter().all(|a| a.is_finite())) {
        return Err(fail(node, format!("some locations have non-finite coordinates: remove them before {what}")));
    }
    if is_lonlat(v.crs.as_deref()).map_err(|e| fail(node, e))? {
        return Err(fail(
            node,
            format!(
                "{what}: the tracks are in longitude / latitude, where amt's areas come from s2 on a plane computed \
                 in degrees: project them first (vector.reproject to the coordinate system of move.aeqd_crs)"
            ),
        ));
    }
    let name = match args.opt_param::<String>("track")?.filter(|s| !s.is_empty()) {
        Some(g) if v.column(&g).is_some() => Some(g),
        Some(g) => return Err(fail(node, format!("track column {g:?} not found"))),
        None => TRACK_NAMES.iter().find(|n| v.column(n).is_some()).map(|n| n.to_string()),
    };
    let Some(col) = name.as_ref().and_then(|n| v.column(n)) else {
        return Ok(Tracks {
            items: vec![Track { rows: (0..x.len()).collect(), x, y }],
            name: "track".into(),
            column: None,
        });
    };
    let code = codes(col);
    let k = code.iter().max().map_or(0, |&m| m as usize + 1);
    let mut rows: Vec<Vec<usize>> = vec![Vec::new(); k];
    for (i, &c) in code.iter().enumerate() {
        rows[c as usize].push(i);
    }
    let items = rows
        .into_iter()
        .map(|r| Track { x: r.iter().map(|&i| x[i]).collect(), y: r.iter().map(|&i| y[i]).collect(), rows: r })
        .collect();
    Ok(Tracks { items, name: name.unwrap(), column: Some(col.clone()) })
}

/// A setting that is a number or a list of them.
fn number_list(args: &Args, name: &str) -> Result<Option<Vec<f64>>> {
    let bad = || fail(args.node, format!("{name}: a number or a list of numbers"));
    match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(Some(vec![n.as_f64().ok_or_else(bad)?])),
        Some(serde_json::Value::Array(a)) => {
            a.iter().map(|v| v.as_f64().ok_or_else(bad)).collect::<Result<Vec<f64>>>().map(Some)
        }
        Some(_) => Err(bad()),
    }
}

/// The levels: numbers in [0, 1], ascending.
fn levels_of(args: &Args, default: &[f64]) -> Result<Vec<f64>> {
    let mut v = number_list(args, "levels")?.unwrap_or_else(|| default.to_vec());
    if v.is_empty() || v.iter().any(|l| !l.is_finite() || !(0.0..=1.0).contains(l)) {
        return Err(fail(args.node, "levels: numbers between 0 and 1"));
    }
    v.sort_by(|a, b| a.total_cmp(b));
    Ok(v)
}

// ------------------------------------------------------------------------------------------- the geometry

fn ring_area(r: &Ring) -> f64 {
    let flat: Vec<f64> = r.iter().flat_map(|p| *p).collect();
    ffi::move_hr_ring_area(&flat)
}

/// `sf::st_area` of a polygon in a projected CRS: GDAL's OGR sums (not GEOS's area), the holes subtracted.
fn poly_area(p: &Poly) -> f64 {
    let Some((shell, holes)) = p.split_first() else { return 0.0 };
    let mut a = ring_area(shell);
    for h in holes {
        a -= ring_area(h);
    }
    a
}

fn shape_area(ps: &[Poly]) -> f64 {
    let mut s = 0.0;
    for p in ps {
        s += poly_area(p);
    }
    s
}

/// The polygons as a feature: a polygon, a multipolygon (`multi`, or for several polygons), none: empty.
fn geometry_of(shape: &[Poly], multi: bool) -> Geometry {
    match shape {
        [] => Geometry::Empty,
        [p] if !multi => Geometry::Polygon(p.clone()),
        _ => Geometry::MultiPolygon(shape.to_vec()),
    }
}

fn polys_of(g: &Geometry) -> Vec<Poly> {
    match g {
        Geometry::Polygon(p) => vec![p.clone()],
        Geometry::MultiPolygon(ps) => ps.clone(),
        _ => Vec::new(),
    }
}

fn geos() -> Res<&'static Geos> {
    terraflux_gdal::geos::geos().map_err(|e| format!("GEOS is needed for the polygons ({e})"))
}

// ---------------------------------------------------------------------------------------------------- MCP

/// The polygon of each level: the convex hull (R's `chull`) of the locations whose squared distance to the mean location
/// is at most the quantile (type 7) of the level, a ring of at least 4 vertices as shapely's polygon.
fn mcp_rings(x: &[f64], y: &[f64], levels: &[f64]) -> Res<Vec<Ring>> {
    let (mx, my) = (ffi::move_hr_colmean(x), ffi::move_hr_colmean(y));
    let sqd: Vec<f64> = x
        .iter()
        .zip(y)
        .map(|(&a, &b)| {
            let (dx, dy) = (a - mx, b - my);
            dx * dx + dy * dy
        })
        .collect();
    let qts = ffi::move_hr_quantile7(&sqd, levels).map_err(ffi_err)?;
    let mut out = Vec::with_capacity(qts.len());
    for q in qts {
        let sel: Vec<usize> = (0..sqd.len()).filter(|&i| sqd[i] <= q).collect();
        let (sx, sy): (Vec<f64>, Vec<f64>) = sel.iter().map(|&i| (x[i], y[i])).unzip();
        let hull = ffi::move_hr_chull(&sx, &sy).map_err(ffi_err)?;
        let mut ring: Ring = hull.iter().map(|&h| [sx[h as usize], sy[h as usize]]).collect();
        ring.push(ring[0]);
        while ring.len() < 4 {
            // one or two vertices (too few locations, or all on a line): R's polygon has no area
            ring.push(ring[ring.len() - 1]);
        }
        out.push(ring);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------- bandwidths

#[derive(Clone, Copy, PartialEq)]
enum Rescale {
    None,
    Unitvar,
    Xvar,
}

fn sd(v: &[f64]) -> f64 {
    ffi::move_hr_var(v).sqrt()
}

fn prep_xy(x: &[f64], y: &[f64], rescale: Rescale) -> (Vec<f64>, Vec<f64>) {
    match rescale {
        Rescale::None => (x.to_vec(), y.to_vec()),
        Rescale::Unitvar => {
            let (sx, sy) = (sd(x), sd(y));
            (x.iter().map(|v| v / sx).collect(), y.iter().map(|v| v / sy).collect())
        }
        Rescale::Xvar => {
            let (sx, sy) = (sd(x), sd(y));
            (x.to_vec(), y.iter().map(|v| (v / sy) * sx).collect())
        }
    }
}

/// amt's `hr_kde_ref`: `sqrt((var(x) + var(y)) / 2) * n^(-1/6)`.
fn ref_one(x: &[f64], y: &[f64], rescale: Rescale) -> (f64, f64) {
    let (xs, ys) = prep_xy(x, y, rescale);
    let (vx, vy) = (ffi::move_hr_var(&xs), ffi::move_hr_var(&ys));
    let h = (0.5 * (vx + vy)).sqrt() * ffi::move_hr_pow(x.len() as f64, -1.0 / 6.0);
    if rescale == Rescale::Unitvar { (h * sd(x), h * sd(y)) } else { (h, h) }
}

fn min_max(x: &[f64]) -> (f64, f64) {
    (x.iter().copied().fold(f64::INFINITY, f64::min), x.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}

fn dpik(x: &[f64]) -> Res<f64> {
    let (a, b) = min_max(x);
    ffi::move_hr_dpik(x, a, b, 0, 2, 0, false, 401, true).map_err(ffi_err)
}

/// amt's `hr_kde_pi` with its defaults: KernSmooth's `dpik` of x and y, raised to the power 5/6.
fn pi_one(x: &[f64], y: &[f64], rescale: Rescale) -> Res<(f64, f64)> {
    let (xs, ys) = prep_xy(x, y, rescale);
    let (hx, hy) = (dpik(&xs)?, dpik(&ys)?);
    let (hx, hy) = if rescale == Rescale::Unitvar { (hx * sd(x), hx * sd(y)) } else { (hx, hy) };
    Ok((ffi::move_hr_pow(hx, 5.0 / 6.0), ffi::move_hr_pow(hy, 5.0 / 6.0)))
}

/// amt's `local_minima` (1-based positions) of the criterion.
fn local_minima(x: &[f64]) -> Vec<usize> {
    let n = x.len();
    let down: Vec<bool> = (0..n)
        .map(|i| {
            let prev = if i == 0 { 2147483647.0 } else { x[i - 1] };
            x[i] - prev < 0.0
        })
        .collect();
    let mut change = vec![0usize];
    for i in 1..n {
        if down[i] != down[i - 1] {
            change.push(i);
        }
    }
    let mut ends: Vec<usize> = change[1..].to_vec();
    ends.push(n);
    let mut runs: Vec<usize> = ends.into_iter().step_by(2).collect();
    if x[0] == x[1] {
        runs.remove(0);
    }
    runs
}

/// amt's `hr_kde_lscv` with its defaults: the bandwidth of the lowest criterion (or the largest local minimum) over
/// 100 values from 0.1 to 2 times the reference bandwidth; false when it is at an end of the range.
fn lscv_one(x: &[f64], y: &[f64], rescale: Rescale, local: bool, threads: i32) -> Res<((f64, f64), bool)> {
    let (xs, ys) = prep_xy(x, y, rescale);
    let href = ref_one(x, y, Rescale::None).0;
    let rng = ffi::move_hr_seq(href * 0.1, href * 2.0, 100).map_err(ffi_err)?;
    let res = ffi::move_hr_lscv(&xs, &ys, &rng, threads).map_err(ffi_err)?;
    let h = if !local {
        let best =
            res.iter().enumerate().filter(|(_, v)| !v.is_nan()).min_by(|a, b| a.1.total_cmp(b.1).then(a.0.cmp(&b.0)));
        rng[best.ok_or("hr_kde_lscv: the criterion is not a number at any bandwidth")?.0]
    } else {
        let minima = local_minima(&res);
        rng[*minima.iter().max().ok_or("hr_kde_lscv: the criterion has no local minimum")? - 1]
    };
    let converged = !(rng[0] == h || rng[rng.len() - 1] == h);
    let hh = if rescale == Rescale::Unitvar { (h * sd(x), h * sd(y)) } else { (h, h) };
    Ok((hh, converged))
}

// ---------------------------------------------------------------------------------------------------- KDE

/// The cells of a template: the exact extent and the number of columns and rows.
#[derive(Clone, Copy)]
struct Spec {
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    ncol: usize,
    nrow: usize,
}

const MAX_CELLS: usize = 100_000_000;

impl Spec {
    /// The upper left corner and the cell sizes (terra's xres and yres).
    fn cell(&self) -> (f64, f64, f64, f64) {
        (self.xmin, self.ymax, (self.xmax - self.xmin) / self.ncol as f64, (self.ymax - self.ymin) / self.nrow as f64)
    }

    fn grid(&self, crs: Option<&String>) -> Grid {
        let (x0, y0, dx, dy) = self.cell();
        let mut g = Grid::new(self.ncol, self.nrow);
        g.transform = [x0, dx, 0.0, y0, 0.0, -dy];
        g.geo.wkt = crs.cloned();
        g
    }

    fn check(&self) -> Res<()> {
        if self.ncol < 2 || self.nrow < 2 {
            return Err("the template needs at least 2 columns and 2 rows".into());
        }
        if self.ncol.saturating_mul(self.nrow) > MAX_CELLS {
            return Err(format!(
                "the template would have {} x {} cells, more than {MAX_CELLS}: give a larger res",
                self.ncol, self.nrow
            ));
        }
        Ok(())
    }
}

fn spec_of(g: &Grid) -> Res<Spec> {
    let t = g.transform;
    if t[2] != 0.0 || t[4] != 0.0 || !t[1].is_finite() || t[1] <= 0.0 || !t[5].is_finite() || t[5] >= 0.0 {
        return Err("the template raster must be north up: x increasing, y decreasing and not rotated".into());
    }
    let s = Spec {
        xmin: t[0],
        xmax: t[0] + g.width as f64 * t[1],
        ymin: t[3] + g.height as f64 * t[5],
        ymax: t[3],
        ncol: g.width,
        nrow: g.height,
    };
    s.check()?;
    Ok(s)
}

fn round_half_up(v: f64) -> i64 {
    let f = v.floor();
    f as i64 + i64::from(v - f >= 0.5)
}

/// amt's `make_trast`: the bounding box of the locations grown by `factor`, in square cells of `res` (by default 1/100
/// of the largest extent), as terra's `rast(extent, res = res)` rounds them.
fn make_trast(x: &[f64], y: &[f64], factor: f64, res: Option<f64>) -> Res<Spec> {
    if !(factor >= 1.0 && factor.is_finite()) {
        return Err("factor: a number of at least 1".into());
    }
    let (x0, x1) = min_max(x);
    let (y0, y1) = min_max(y);
    let me = (x1 - x0).max(y1 - y0);
    let res = res.unwrap_or_else(|| (me / 100.0).max(1e-9));
    if !(res >= 1e-10 && res.is_finite()) {
        return Err("res: a number of at least 1e-10".into());
    }
    let bu = me * factor - me;
    let (xmin, xmax, ymin, ymax) = (x0 - bu, x1 + bu, y0 - bu, y1 + bu);
    let ncol = round_half_up((xmax - xmin) / res).max(1);
    let nrow = round_half_up((ymax - ymin) / res).max(1);
    if ncol as f64 * nrow as f64 > MAX_CELLS as f64 {
        return Err(format!("the template would have {ncol} x {nrow} cells, more than {MAX_CELLS}: give a larger res"));
    }
    let s = Spec {
        xmin,
        xmax: xmin + ncol as f64 * res,
        ymin,
        ymax: ymin + nrow as f64 * res,
        ncol: ncol as usize,
        nrow: nrow as usize,
    };
    s.check()?;
    Ok(s)
}

/// bkde2D's estimate of a track on the template, the rows from the top (and whether R warns of a coarse grid).
fn kde_values(x: &[f64], y: &[f64], h: (f64, f64), s: &Spec) -> Res<(Vec<f64>, bool)> {
    if !(h.0 > 0.0 && h.1 > 0.0) {
        return Err("'bandwidth' must be strictly positive".into());
    }
    let r = ffi::move_hr_bkde2d(x, y, h.0, h.1, s.xmin, s.xmax, s.ymin, s.ymax, s.ncol as i64, s.nrow as i64)
        .map_err(ffi_err)?;
    let mut out = vec![0.0; s.ncol * s.nrow];
    for row in 0..s.nrow {
        let from = (s.nrow - 1 - row) * s.ncol;
        out[row * s.ncol..(row + 1) * s.ncol].copy_from_slice(&r.fhat[from..from + s.ncol]);
    }
    Ok((out, r.coarse))
}

/// The union of the closed contour lines of the cumulative distribution at each level, as polygons (amt's isopleths).
fn isopleths(cud: &[f64], s: &Spec, levels: &[f64], geos: &Geos) -> Res<Vec<Vec<Poly>>> {
    let (xmin, ymax, dx, dy) = s.cell();
    let mut out = Vec::with_capacity(levels.len());
    for &lv in levels {
        let c =
            ffi::move_hr_contour_lines(cud, s.nrow as i64, s.ncol as i64, xmin, ymax, dx, dy, lv).map_err(ffi_err)?;
        if !c.ok {
            return Err("all the values of the template are equal (no location in it?): no isopleths; give a template (like) that holds the locations".into());
        }
        let lines = c.start.len() - 1;
        if lines == 0 {
            return Err(format!("no contour line at level {lv}"));
        }
        let mut polys: Vec<Poly> = Vec::with_capacity(lines);
        for k in 0..lines {
            let ring: Ring =
                (c.start[k] as usize..c.start[k + 1] as usize).map(|i| [c.xy[2 * i], c.xy[2 * i + 1]]).collect();
            if ring.len() < 4 || ring[0] != ring[ring.len() - 1] {
                return Err(format!(
                    "the contour at level {lv} reaches the edge of the raster (an open line is not a polygon): use \
                     a larger template raster (like, or a larger factor)"
                ));
            }
            polys.push(vec![ring]);
        }
        out.push(geos.union_all(&polys)?);
    }
    Ok(out)
}

/// amt's `hr_kde_ref_scaled`: the bandwidth halved between the ends of 0.01 to 1 times the reference one until the
/// isopleth at `level` has no more than `num_parts` polygons and the step is at most `tol`.
fn scaled_one(x: &[f64], y: &[f64], s: &Spec, level: f64, num_parts: usize, geos: &Geos) -> Res<(f64, bool)> {
    let (tol, max_it) = (0.1, 500);
    let href = ref_one(x, y, Rescale::None).0;
    let (mut hmin, mut hmax) = (href * 0.01, href * 1.0);
    let mut hcur = ffi::move_hr_mean(&[hmin, hmax]);
    let mut hnew = hcur;
    let mut success = false;
    for i in 1..=max_it {
        if i > 1 {
            hcur = hnew;
        }
        let (values, _) = kde_values(x, y, (hcur, hcur), s)?;
        let cud = ffi::move_hr_cud(&values);
        let geom = isopleths(&cud, s, &[level], geos)?;
        let n_holes = geom[0].len();
        if n_holes <= num_parts {
            hmax = hcur;
        } else {
            hmin = hcur;
        }
        hnew = ffi::move_hr_mean(&[hmax, hmin]);
        if (hcur - hnew).abs() <= tol && n_holes <= num_parts {
            success = true;
            break;
        }
    }
    Ok((hcur, success))
}

// ----------------------------------------------------------------------------------------------- LoCoH

/// R's `round(x, 2)` (4.0.0 and later: the nearest of the two candidates, a tie to the even one).
fn r_round(x: f64, dig: i32) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let sgn = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let l10x = std::f64::consts::LOG10_2 * (0.5 + x.log2().floor());
    if l10x + dig as f64 > 15.0 {
        return sgn * x;
    }
    let p10 = 10f64.powi(dig);
    let x10 = p10 * x;
    let i10 = x10.floor();
    let xd = i10 / p10;
    let xu = x10.ceil() / p10;
    let (du, dd) = (xu - x, x - xd);
    sgn * if dd < du || (dd == du && (i10 as i64) % 2 == 0) { xd } else { xu }
}

/// amt's `hr_locoh`: the hull of each location and its neighbours buffered, taken from the smallest and united until
/// they hold the share of the locations asked for. One (level reached, polygons, area) per level.
#[allow(clippy::too_many_arguments)]
fn locoh_one(
    geos: &Geos,
    x: &[f64],
    y: &[f64],
    n: f64,
    kind: i32,
    levels: &[f64],
    rand_buffer: f64,
    threads: i32,
) -> Res<Vec<(f64, Vec<Poly>, f64)>> {
    let m = x.len();
    let nb = ffi::move_hr_locoh_neighbours(x, y, kind, n, threads).map_err(ffi_err)?;
    let members: Vec<Vec<usize>> = (0..m)
        .map(|i| {
            let mut v = vec![i];
            v.extend(nb.ids[nb.offset[i] as usize..nb.offset[i + 1] as usize].iter().map(|&k| k as usize));
            v
        })
        .collect();
    let xy: Vec<[f64; 2]> = x.iter().zip(y).map(|(&a, &b)| [a, b]).collect();
    let buffered = geos.hull_buffers(&xy, &members, rand_buffer, 30)?;
    let areas: Vec<f64> = buffered.iter().map(|g| shape_area(g)).collect();
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&a, &b| areas[a].partial_cmp(&areas[b]).unwrap_or(std::cmp::Ordering::Equal));
    // the first polygon (in the order of the areas) that holds each location
    let mut seen = vec![false; m];
    let mut first = Vec::with_capacity(m);
    for (rank, &i) in order.iter().enumerate() {
        for &p in &members[i] {
            if !seen[p] {
                seen[p] = true;
                first.push(rank + 1);
            }
        }
    }
    let pp: Vec<f64> = (1..=m).map(|p| first.partition_point(|&r| r <= p) as f64 / m as f64).collect();
    let mut out = Vec::with_capacity(levels.len());
    for &lv in levels {
        let mut w = 0;
        let mut best = f64::INFINITY;
        for (k, p) in pp.iter().enumerate() {
            let d = (p - lv).abs();
            if d < best {
                best = d;
                w = k;
            }
        }
        let w = w + 1;
        let polys: Vec<Poly> = order[..w].iter().flat_map(|&i| buffered[i].iter().cloned()).collect();
        let g = geos.union_all(&polys)?;
        let area = shape_area(&g);
        out.push((r_round(pp[w - 1], 2), g, area));
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------- the operators

/// A feature of the polygons output: track, level, the polygons and their area.
struct Row {
    track: usize,
    level: f64,
    shape: Vec<Poly>,
    multi: bool,
    area: f64,
}

/// The rows as amt's `hr_isopleths` lists them: the tracks in order, each from its highest level down.
fn sort_rows(rows: &mut [Row]) {
    rows.sort_by(|a, b| a.track.cmp(&b.track).then(b.level.partial_cmp(&a.level).unwrap_or(std::cmp::Ordering::Equal)));
}

fn labels_table(columns: &[&str], names: &[String]) -> Table {
    let mut t = Table::new(columns.iter().map(|c| c.to_string()).collect());
    t.labels.insert(columns[0].to_string(), names.to_vec());
    t
}

enum Bandwidth {
    Reference,
    Scaled,
    PlugIn,
    Lscv,
    Given(f64, f64),
}

fn bandwidth_of(args: &Args) -> Result<Bandwidth> {
    let bad = || {
        fail(args.node, "bandwidth: ref, ref_scaled, pi, lscv, a number (metres, for x and y) or a list of two (x, y)")
    };
    match args.params.get("bandwidth") {
        None | Some(serde_json::Value::Null) => Ok(Bandwidth::Reference),
        Some(serde_json::Value::String(s)) => match s.as_str() {
            "" | "ref" => Ok(Bandwidth::Reference),
            "ref_scaled" => Ok(Bandwidth::Scaled),
            "pi" => Ok(Bandwidth::PlugIn),
            "lscv" => Ok(Bandwidth::Lscv),
            _ => Err(bad()),
        },
        Some(_) => {
            let v = number_list(args, "bandwidth")?.ok_or_else(bad)?;
            match v.as_slice() {
                [h] => Ok(Bandwidth::Given(*h, *h)),
                [hx, hy] => Ok(Bandwidth::Given(*hx, *hy)),
                _ => Err(bad()),
            }
        }
    }
}

fn rescale_of(args: &Args) -> Result<Rescale> {
    match args.param("rescale", "none".to_string())?.as_str() {
        "none" | "" => Ok(Rescale::None),
        "unitvar" => Ok(Rescale::Unitvar),
        "xvar" => Ok(Rescale::Xvar),
        _ => Err(fail(args.node, "rescale: none, unitvar or xvar")),
    }
}

/// The grid cells of the `like` input: one map or stack, or the first band of a raster.
fn like_grid(args: &Args) -> Result<Option<Arc<Grid>>> {
    Ok(match args.get("like") {
        None => None,
        Some(Value::Raster(r)) => Some(Arc::new(r.grid_of(r.chosen).clone())),
        Some(Value::Map(m)) => Some(m.grid.clone()),
        Some(Value::Stack(s)) => Some(s.grid.clone()),
        Some(v) => return Err(fail(args.node, format!("like: a raster, not a {}", v.ty().name()))),
    })
}

/// `move.home_range`.
struct HomeRange;

impl Operator for HomeRange {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.home_range",
            "The home range of each track (the library's home_range, from amt 0.3.1, Signer, Fieberg and Avgar 2019), \
             by one of three methods, as polygons with their areas. mcp: the minimum convex polygon of the locations \
             nearest to their mean (the squared distance at most the quantile of each level, R's type 7; Worton \
             1989): the polygon of level 0.95 is the hull of 95 % of them. kde: the kernel density estimate \
             (a bivariate normal kernel, KernSmooth's bkde2D, evaluated at the template's grid laid over its edges), \
             its isopleths the contour lines of the cumulative distribution (R's contourLines) at each level, \
             closed into polygons and united (a hole is filled); the template (like) must hold them: a contour that \
             reaches its edge is an error, so give a larger template. bandwidth ref (the reference bandwidth \
             sqrt((var x + var y) / 2) n^(-1/6): it oversmooths a track with several centres of use), ref_scaled \
             (the reference one reduced until the level's isopleth has at most num_parts polygons: the highest \
             level, between 0.0001 and 0.9999), pi (the plug-in one of KernSmooth's dpik, raised to the power 5/6 as \
             amt does), lscv (least-squares cross-validation over 100 bandwidths from 0.1 to 2 times the reference: \
             it undersmooths, and its minimum may be at an end of the range, a warning), a number (the same for x \
             and y) or a list (x, y). rescale none, unitvar or xvar scales the coordinates first (ref, pi, lscv). \
             locoh: the local convex hulls of Getz et al. (2007): the hull of each location and its neighbours (type k: \
             the n nearest, as a rule about the square root of the number of locations; r: those within the \
             distance n; a: those whose cumulative distance, nearest first, is at most n), buffered by rand_buffer, \
             taken from the smallest and united until they hold each level of the locations; the level reached \
             (to 2 digits) is the one recorded. The tracks must be projected (not longitude / latitude: amt's areas \
             of those come from s2 on a plane in degrees; project them with move.aeqd_crs) with a track column (or \
             one track). Each track alone; without like, one template made for all of them (amt's make_trast: the \
             bounding box of all the locations grown by factor, cells of res, by default 1/100 of its largest side), \
             so that their UDs can be compared. The polygons are listed as amt's hr_isopleths lists them: each \
             track from its highest level down, with the level, what (estimate) and area (the CRS's unit squared, \
             GDAL's OGR sums). The UD (kde only) has a band for each track, the cells summing to 1.",
            Access::None,
        )
        .input(PortSpec::new("tracks", Type::Vector, "the tracks: points in a projected coordinate system"))
        .input(PortSpec::optional(
            "like",
            Type::Raster,
            "kde: the raster whose cells the estimate takes (north up); else one made from the locations (res, factor)",
        ))
        .output(PortSpec::new(
            "polygons",
            Type::Vector,
            "a polygon for each track and level: track, level, what and area",
        ))
        .output(PortSpec::new("table", Type::Table, "the areas: track, level and area, as the polygons"))
        .output(PortSpec::optional("ud", Type::Stack, "kde: the UD of each track, the cells summing to 1"))
        .output(PortSpec::optional("bandwidths", Type::Table, "kde: the bandwidth of each track: hx and hy"))
        .param(PortSpec::optional("method", Type::Text, "mcp (default), kde or locoh").choices(["mcp", "kde", "locoh"]))
        .param(PortSpec::optional(
            "levels",
            Type::Any,
            "the share of the use each polygon holds: a number or a list, in [0, 1] (0.95)",
        ))
        .param(PortSpec::optional(
            "bandwidth",
            Type::Any,
            "kde: ref (default), ref_scaled, pi, lscv, a number or a list (x, y)",
        ))
        .param(
            PortSpec::optional("rescale", Type::Text, "kde bandwidths ref, pi, lscv: scale the coordinates first (none)")
                .choices(["none", "unitvar", "xvar"]),
        )
        .param(
            PortSpec::optional("which_min", Type::Text, "lscv: the lowest criterion (global) or the largest local minimum")
                .choices(["global", "local"]),
        )
        .param(PortSpec::optional("num_parts", Type::Integer, "ref_scaled: the most polygons wanted (1)"))
        .param(PortSpec::optional("res", Type::Number, "kde without like: the cell size (1/100 of the largest side)"))
        .param(PortSpec::optional("factor", Type::Number, "kde without like: the box grown by this factor, 1 or more (1.5)"))
        .param(PortSpec::optional("type", Type::Text, "locoh: k (default), a or r").choices(["k", "a", "r"]))
        .param(PortSpec::optional(
            "n",
            Type::Number,
            "locoh: the neighbours (k: a whole number, 10), the radius (r) or the cumulative distance (a)",
        ))
        .param(PortSpec::optional(
            "rand_buffer",
            Type::Number,
            "locoh: the buffer of the hulls, the CRS's units, so that collinear locations have an area (1e-5)",
        ))
        .param(PortSpec::optional(
            "track",
            Type::Text,
            "the column of the track (default: individual-local-identifier, track, id, individual or animal; none: one track)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let method: String = args.param("method", "mcp".to_string())?;
        if !["mcp", "kde", "locoh"].contains(&method.as_str()) {
            return Err(fail(node, "method: mcp, kde or locoh"));
        }
        let levels = levels_of(args, &[0.95])?;
        let tr = read_tracks(args, v, "move.home_range")?;
        let names = tr.labels();
        let tag = |e: String| fail(node, e);
        let mut rows: Vec<Row> = Vec::new();
        let mut extra: Outputs = Outputs::new();
        match method.as_str() {
            "mcp" => {
                for (k, t) in tr.items.iter().enumerate() {
                    for (lv, ring) in levels.iter().zip(mcp_rings(&t.x, &t.y, &levels).map_err(tag)?) {
                        let shape = vec![vec![ring]];
                        rows.push(Row { track: k, level: *lv, area: shape_area(&shape), shape, multi: false });
                    }
                }
            }
            "kde" => {
                let geos = geos().map_err(tag)?;
                let (all_x, all_y) = tr.all_xy();
                let (spec, grid) = match like_grid(args)? {
                    Some(g) => (spec_of(&g).map_err(tag)?, g),
                    None => {
                        let spec = make_trast(&all_x, &all_y, args.param("factor", 1.5)?, args.opt_param("res")?)
                            .map_err(tag)?;
                        (spec, Arc::new(spec.grid(v.crs.as_ref())))
                    }
                };
                let rescale = rescale_of(args)?;
                let local = match args.param("which_min", "global".to_string())?.as_str() {
                    "global" => false,
                    "local" => true,
                    _ => return Err(fail(node, "which_min: global or local")),
                };
                let num_parts: i64 = args.param("num_parts", 1)?;
                if num_parts < 1 {
                    return Err(fail(node, "num_parts: at least 1"));
                }
                let bw = bandwidth_of(args)?;
                if matches!(bw, Bandwidth::Scaled) {
                    let top = levels[levels.len() - 1];
                    if !(1e-4..=0.9999).contains(&top) {
                        return Err(fail(node, "bandwidth ref_scaled: the highest level between 0.0001 and 0.9999"));
                    }
                }
                if spec.ncol * spec.nrow > 100_000 {
                    ctx.warn(format!(
                        "node {node:?}: the template has more than 1e5 cells: amt's isopleths (terra::as.contour, \
                         maxcells = 1e5) are those of a regular sample of 1e5 cells; here the contours are those of \
                         the whole grid"
                    ));
                }
                let mut h = Vec::with_capacity(tr.items.len());
                for (k, t) in tr.items.iter().enumerate() {
                    let hk = match bw {
                        Bandwidth::Reference => ref_one(&t.x, &t.y, rescale),
                        Bandwidth::Given(a, b) => (a, b),
                        Bandwidth::PlugIn => pi_one(&t.x, &t.y, rescale).map_err(tag)?,
                        Bandwidth::Lscv => {
                            let (hh, ok) = lscv_one(&t.x, &t.y, rescale, local, threads()).map_err(tag)?;
                            if !ok {
                                ctx.warn(format!("node {node:?}: track {}: lscv did not converge.", names[k]));
                            }
                            hh
                        }
                        Bandwidth::Scaled => {
                            let top = levels[levels.len() - 1];
                            let (hh, ok) = scaled_one(&t.x, &t.y, &spec, top, num_parts as usize, geos).map_err(tag)?;
                            if !ok {
                                ctx.warn(format!("node {node:?}: track {}: ref_scaled did not converge.", names[k]));
                            }
                            (hh, hh)
                        }
                    };
                    h.push(hk);
                }
                let mut layers = Vec::with_capacity(tr.items.len());
                for (k, t) in tr.items.iter().enumerate() {
                    let (values, coarse) = kde_values(&t.x, &t.y, h[k], &spec).map_err(tag)?;
                    if coarse {
                        ctx.warn(format!(
                            "node {node:?}: track {}: the template's cells are too coarse for the bandwidth {:?}: use \
                             a finer template (res)",
                            names[k], h[k]
                        ));
                    }
                    let cud = ffi::move_hr_cud(&values);
                    let geoms = isopleths(&cud, &spec, &levels, geos).map_err(tag)?;
                    for (lv, g) in levels.iter().zip(geoms) {
                        rows.push(Row { track: k, level: *lv, area: shape_area(&g), shape: g, multi: true });
                    }
                    let total = ffi::move_r_sum(&values);
                    layers.push(Arc::new(values.iter().map(|c| c / total).collect::<Vec<f64>>()));
                }
                extra.insert(
                    "ud".into(),
                    Value::Stack(Stack { grid, keys: names.iter().cloned().map(Key::Name).collect(), layers }),
                );
                let mut t = labels_table(&[tr.name.as_str(), "hx", "hy"], &names);
                for (k, hk) in h.iter().enumerate() {
                    t.rows.push(vec![(k + 1) as f64, hk.0, hk.1]);
                }
                extra.insert("bandwidths".into(), Value::Table(t));
            }
            _ => {
                let geos = geos().map_err(tag)?;
                let kind = match args.param("type", "k".to_string())?.as_str() {
                    "k" => 0,
                    "r" => 1,
                    "a" => 2,
                    _ => return Err(fail(node, "type: k, a or r")),
                };
                let n: f64 = args.param("n", 10.0)?;
                if !(n.is_finite() && n > 0.0) || (kind == 0 && n.fract() != 0.0) {
                    return Err(fail(node, "n: a positive number (for type k a whole number of neighbours)"));
                }
                let rand_buffer: f64 = args.param("rand_buffer", 1e-5)?;
                for (k, t) in tr.items.iter().enumerate() {
                    if kind == 0 && n >= t.x.len() as f64 {
                        return Err(fail(
                            node,
                            format!(
                                "locoh, type k, track {}: n must be less than the number of locations ({}): the \
                                 neighbours of a location exclude itself",
                                names[k],
                                t.x.len()
                            ),
                        ));
                    }
                    for (lv, g, area) in
                        locoh_one(geos, &t.x, &t.y, n, kind, &levels, rand_buffer, threads()).map_err(tag)?
                    {
                        rows.push(Row { track: k, level: lv, shape: g, multi: false, area });
                    }
                }
            }
        }
        sort_rows(&mut rows);
        let ks: Vec<usize> = rows.iter().map(|r| r.track).collect();
        let level: Vec<f64> = rows.iter().map(|r| r.level).collect();
        let area: Vec<f64> = rows.iter().map(|r| r.area).collect();
        let polygons = Vector {
            geometries: rows.iter().map(|r| geometry_of(&r.shape, r.multi)).collect(),
            attributes: vec![
                (tr.name.clone(), tr.label_column(&ks)),
                ("level".into(), Column::Numbers(level.clone())),
                ("what".into(), Column::Texts(vec!["estimate".to_string(); rows.len()])),
                ("area".into(), Column::Numbers(area.clone())),
            ],
            crs: v.crs.clone(),
        };
        let mut table = labels_table(&[tr.name.as_str(), "level", "area"], &names);
        for (i, r) in rows.iter().enumerate() {
            table.rows.push(vec![(r.track + 1) as f64, level[i], area[i]]);
        }
        extra.insert("polygons".into(), Value::Vector(Arc::new(polygons)));
        extra.insert("table".into(), Value::Table(table));
        Ok(extra)
    }
}

// ----------------------------------------------------------------------------------------------- overlap

const KINDS: [&str; 6] = ["hr", "phr", "vi", "ba", "udoi", "hd"];

/// A home range of the overlap: its polygons by level (highest first), or its UD.
struct Unit {
    name: String,
    polygons: Vec<(f64, Vec<Poly>)>,
    ud: Vec<f64>,
}

/// The polygons of home ranges (the output of move.home_range) by track, in the order the tracks first appear.
fn units_of_polygons(args: &Args, v: &Vector) -> Result<Vec<Unit>> {
    let node = args.node;
    let name = match args.opt_param::<String>("track")?.filter(|s| !s.is_empty()) {
        Some(g) if v.column(&g).is_some() => g,
        Some(g) => return Err(fail(node, format!("track column {g:?} not found"))),
        None => v
            .attributes
            .first()
            .map(|(n, _)| n.clone())
            .ok_or_else(|| fail(node, "polygons: the output of move.home_range, with a track column"))?,
    };
    let Some(Column::Numbers(level)) = v.column("level") else {
        return Err(fail(node, "polygons: a column level (the output of move.home_range)"));
    };
    let col = v.column(&name).unwrap();
    let code = codes(col);
    let k = code.iter().max().map_or(0, |&m| m as usize + 1);
    let mut units: Vec<Unit> = Vec::with_capacity(k);
    for c in 0..k {
        let rows: Vec<usize> = (0..v.len()).filter(|&i| code[i] == c as i64).collect();
        let label = match col {
            Column::Texts(t) => t[rows[0]].clone(),
            Column::Numbers(n) => number_label(n[rows[0]]),
        };
        let mut polygons: Vec<(f64, Vec<Poly>)> =
            rows.iter().map(|&i| (level[i], polys_of(&v.geometries[i]))).collect();
        polygons.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        units.push(Unit { name: label, polygons, ud: Vec::new() });
    }
    Ok(units)
}

/// The UDs of a raster: a band each, named by the band.
fn units_of_ud(node: &str, value: &Value) -> Result<Vec<Unit>> {
    let value = crate::files::as_port(value.clone(), Type::Any)?;
    match &value {
        Value::Stack(s) => Ok(s
            .keys
            .iter()
            .zip(&s.layers)
            .enumerate()
            .map(|(i, (key, l))| Unit {
                name: match key {
                    Key::Name(n) if !n.is_empty() => n.clone(),
                    _ => format!("ud_{}", i + 1),
                },
                polygons: Vec::new(),
                ud: l.to_vec(),
            })
            .collect()),
        other => Err(fail(
            node,
            format!("ud: a raster with a band for each track (the ud of move.home_range), not a {}", other.ty().name()),
        )),
    }
}

/// amt's `hr_ud` of a UD: the cells over their sum.
fn normalised(u: &[f64]) -> Vec<f64> {
    let total = ffi::move_r_sum(u);
    u.iter().map(|v| v / total).collect()
}

/// The UD conditional on a level: the cells inside its isopleth, over the level.
fn conditional_ud(u: &[f64], level: f64) -> Vec<f64> {
    let cud = ffi::move_hr_cud(u);
    let mut ud = normalised(u);
    for (v, c) in ud.iter_mut().zip(&cud) {
        if *c > level {
            *v = 0.0;
        }
    }
    ud.iter().map(|v| v / level).collect()
}

/// amt's `base_ov`: the share of the area of x that y covers.
fn polygon_overlap(geos: &Geos, gx: &[Poly], gy: &[Poly]) -> Res<f64> {
    match geos.intersection(gx, gy)? {
        Some(both) => Ok(shape_area(&both) / shape_area(gx)),
        None => Ok(0.0),
    }
}

/// `move.home_range_overlap`.
struct HomeRangeOverlap;

impl Operator for HomeRangeOverlap {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.home_range_overlap",
            "The overlap between the home ranges of tracks (amt's hr_overlap, Fieberg and Kochanny 2005), for the \
             pairs of tracks of a move.home_range output: consecutive ones, all (each against each other, both \
             ways) or the first against the others (which). type hr: the share of the area of the first's polygon \
             that the second's covers, at each level: needs the polygons (the tracks must have the same levels). \
             The others are indices of two kernel UDs on the same grid (the ud of move.home_range): phr (the \
             probability the second animal uses the first's home range: the sum of the second's UD over the cells \
             where the first's is positive), vi (volume of intersection, the sum of the smaller of the two cells), \
             ba (Bhattacharyya's affinity), udoi (the UD overlap index, which exceeds 1 when both are \
             concentrated in the same cells) and hd (Hellinger's distance, 0 for equal UDs). With conditional the \
             UDs are cut to the cells inside each level's isopleth (the levels of the polygons, or levels) and \
             rescaled, a row for each level; else one row, at level 1. The UD is rescaled to sum to 1 again here, so \
             the indices differ from amt's in the last digits.",
            Access::None,
        )
        .input(PortSpec::optional("polygons", Type::Vector, "type hr: the polygons of move.home_range"))
        .input(PortSpec::optional("ud", Type::Any, "the other types: the ud of move.home_range, a band for each track"))
        .output(PortSpec::new("table", Type::Table, "from, to (the tracks), level and overlap"))
        .param(PortSpec::optional("type", Type::Text, "hr (default), phr, vi, ba, udoi or hd").choices(KINDS))
        .param(PortSpec::optional(
            "conditional",
            Type::Bool,
            "the UDs conditional on each level: the cells in the isopleth (false)",
        ))
        .param(
            PortSpec::optional("which", Type::Text, "the pairs: consecutive (default), all or one_to_all").choices([
                "consecutive",
                "all",
                "one_to_all",
            ]),
        )
        .param(PortSpec::optional("levels", Type::Any, "conditional: the levels (those of the polygons, else 0.95)"))
        .param(PortSpec::optional("track", Type::Text, "the column of the track in the polygons (the first)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let kind: String = args.param("type", "hr".to_string())?;
        let Some(code) = KINDS.iter().position(|k| *k == kind) else {
            return Err(fail(node, "type: hr, phr, vi, ba, udoi or hd"));
        };
        let conditional: bool = args.param("conditional", false)?;
        let which: String = args.param("which", "consecutive".to_string())?;
        if !["consecutive", "all", "one_to_all"].contains(&which.as_str()) {
            return Err(fail(node, "which: consecutive, all or one_to_all"));
        }
        let polygons = match args.get("polygons") {
            Some(Value::Vector(v)) => Some(units_of_polygons(args, v)?),
            Some(v) => return Err(fail(node, format!("polygons: features, not a {}", v.ty().name()))),
            None => None,
        };
        let tag = |e: String| fail(node, e);
        let units;
        let mut levels_of_polygons: Option<Vec<f64>> = None;
        if code == 0 {
            units = polygons.ok_or_else(|| fail(node, "type hr: connect the polygons (move.home_range)"))?;
        } else {
            let ud = args.get("ud").ok_or_else(|| fail(node, "the UD types: connect the ud (move.home_range, kde)"))?;
            units = units_of_ud(node, ud)?;
            if let Some(p) = &polygons {
                levels_of_polygons = p.first().map(|u| {
                    let mut l: Vec<f64> = u.polygons.iter().map(|g| g.0).collect();
                    l.reverse();
                    l
                });
            }
        }
        if units.len() < 2 {
            return Err(fail(node, "At least two home range estimates are needed"));
        }
        let names: Vec<String> = units.iter().map(|u| u.name.clone()).collect();
        if names.iter().any(|n| n.is_empty()) {
            return Err(fail(node, "Names must be different from ''"));
        }
        let mut sorted = names.clone();
        sorted.sort();
        if sorted.windows(2).any(|w| w[0] == w[1]) {
            return Err(fail(node, "Duplicated names are not permitted"));
        }
        if code == 0 {
            // every track has a polygon for each level (LoCoH's reached levels differ a little from track to track)
            if units[1..].iter().any(|u| u.polygons.len() != units[0].polygons.len()) {
                return Err(fail(node, "Not all levels of first home range est are in the others as well."));
            }
        } else if let Some(first) = units.first() {
            let n = first.ud.len();
            if units.iter().any(|u| u.ud.len() != n) {
                return Err(fail(node, "Extend and resolution differ, make sure to use a template raster."));
            }
        }
        let pairs: Vec<(usize, usize)> = match which.as_str() {
            "consecutive" => (0..units.len() - 1).map(|i| (i, i + 1)).collect(),
            "all" => {
                (0..units.len()).flat_map(|i| (0..units.len()).filter(move |&j| j != i).map(move |j| (i, j))).collect()
            }
            _ => (1..units.len()).map(|j| (0, j)).collect(),
        };
        let cond_levels = if conditional {
            match number_list(args, "levels")? {
                Some(mut l) => {
                    if l.is_empty() || l.iter().any(|v| !(*v > 0.0 && *v <= 1.0)) {
                        return Err(fail(node, "levels: numbers above 0 and up to 1"));
                    }
                    l.sort_by(|a, b| a.total_cmp(b));
                    l
                }
                None => levels_of_polygons.clone().unwrap_or_else(|| vec![0.95]),
            }
        } else {
            vec![1.0]
        };
        // the UDs conditional on each level, made once for each track
        let mut shaped: Vec<Vec<Vec<f64>>> = Vec::new();
        if code != 0 {
            for u in &units {
                shaped.push(if conditional {
                    cond_levels.iter().map(|&l| conditional_ud(&u.ud, l)).collect()
                } else {
                    vec![normalised(&u.ud)]
                });
            }
        }
        let geos = if code == 0 { Some(geos().map_err(tag)?) } else { None };
        let mut table = labels_table(&["from", "to", "level", "overlap"], &names);
        table.labels.insert("to".into(), names.clone());
        for (i, j) in pairs {
            if code == 0 {
                let (a, b) = (&units[i].polygons, &units[j].polygons);
                for (ga, gb) in a.iter().zip(b) {
                    let o = polygon_overlap(geos.unwrap(), &ga.1, &gb.1).map_err(tag)?;
                    table.rows.push(vec![(i + 1) as f64, (j + 1) as f64, ga.0, o]);
                }
            } else {
                for (l, &level) in cond_levels.iter().enumerate() {
                    let o = ffi::move_hr_vol_base(&shaped[i][l], &shaped[j][l], code as i32 - 1)?;
                    table.rows.push(vec![(i + 1) as f64, (j + 1) as f64, level, o]);
                }
            }
        }
        Ok(crate::outputs([("table", Value::Table(table))]))
    }
}

// ------------------------------------------------------------------------------------------ intensity use

/// `move.intensity_use`.
struct IntensityUse;

impl Operator for IntensityUse {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.intensity_use",
            "The intensity of use of each track (amt's intensity_use): the length of the track \
             (the sum of the distances between consecutive locations) over the square root of the area of its 100 % \
             minimum convex polygon, so that it is a number without a unit: how many times the animal went back and \
             forth over the ground it covered. The tracks must be projected (not longitude / latitude: project them \
             with move.aeqd_crs) with a track column, or are one track; the locations are taken in the order they are \
             given.",
            Access::None,
        )
        .input(PortSpec::new("tracks", Type::Vector, "the tracks: points in a projected coordinate system"))
        .output(PortSpec::new("table", Type::Table, "the track and its intensity_use"))
        .param(PortSpec::optional(
            "track",
            Type::Text,
            "the column of the track (default: individual-local-identifier, track, id, individual or animal; none: one track)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let tr = read_tracks(args, v, "move.intensity_use")?;
        let names = tr.labels();
        let mut table = labels_table(&[tr.name.as_str(), "intensity_use"], &names);
        for (k, t) in tr.items.iter().enumerate() {
            let group = vec![0i64; t.x.len()];
            let d = ffi::move_distances(&t.x, &t.y, &group, false, 1)?;
            let steps = &d[..d.len() - 1];
            let length = if steps.is_empty() { 0.0 } else { ffi::move_r_sum(steps) };
            let ring = mcp_rings(&t.x, &t.y, &[1.0]).map_err(|e| fail(node, e))?.remove(0);
            let area = ring_area(&ring);
            table.rows.push(vec![(k + 1) as f64, length / ffi::move_hr_pow(area, 0.5)]);
        }
        Ok(crate::outputs([("table", Value::Table(table))]))
    }
}
