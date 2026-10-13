//! Occurrence records and background points (landschaft's thin, one_per_cell and background): spThin's
//! thinning, a record per cell of a raster, and background points drawn as dismo's randomPoints (R's sampling,
//! weighted by the cells' widths on lon/lat grids or by a bias), within an area, a band of distances from the
//! records or outside their environmental envelope.

use std::sync::Arc;

use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::Table;

const X_NAMES: [&str; 6] = ["x", "lon", "long", "lng", "longitude", "decimallongitude"];
const Y_NAMES: [&str; 4] = ["y", "lat", "latitude", "decimallatitude"];
const LONLAT: [&str; 8] = ["lon", "long", "lng", "longitude", "decimallongitude", "lat", "latitude", "decimallatitude"];

/// The records' x and y and their coordinate system.
pub type Coordinates = (Vec<f64>, Vec<f64>, Option<String>);

/// Occurrence records: points, or a table with coordinate columns.
#[derive(Clone)]
pub enum Occurrences {
    Vector(Arc<Vector>),
    Table(Table),
}

fn coordinate_column(columns: &[String], given: Option<&str>, names: &[&str], axis: &str) -> Result<usize, String> {
    if let Some(g) = given {
        return columns
            .iter()
            .position(|c| c == g)
            .ok_or_else(|| format!("no column {g:?}; columns: {}", columns.join(", ")));
    }
    for n in names {
        if let Some(k) = columns.iter().position(|c| c.to_lowercase() == *n) {
            return Ok(k);
        }
    }
    Err(format!("no {axis} coordinate column found (looked for {}); set {axis}", names.join(", ")))
}

impl Occurrences {
    pub fn len(&self) -> usize {
        match self {
            Occurrences::Vector(v) => v.len(),
            Occurrences::Table(t) => t.rows.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The records' coordinates and their coordinate system (WKT or a code; lon/lat for tables of lon/lat
    /// columns).
    pub fn coordinates(&self, x: Option<&str>, y: Option<&str>) -> Result<Coordinates, String> {
        match self {
            Occurrences::Vector(v) => {
                let (mut xs, mut ys) = (Vec::with_capacity(v.len()), Vec::with_capacity(v.len()));
                for g in &v.geometries {
                    match g {
                        Geometry::Point(p) => {
                            xs.push(p[0]);
                            ys.push(p[1]);
                        }
                        Geometry::Empty => {
                            xs.push(f64::NAN);
                            ys.push(f64::NAN);
                        }
                        _ => return Err("occurrence records must be points".into()),
                    }
                }
                Ok((xs, ys, v.crs.clone()))
            }
            Occurrences::Table(t) => {
                let xc = coordinate_column(&t.columns, x, &X_NAMES, "x")?;
                let yc = coordinate_column(&t.columns, y, &Y_NAMES, "y")?;
                let lonlat = LONLAT.contains(&t.columns[xc].to_lowercase().as_str())
                    && LONLAT.contains(&t.columns[yc].to_lowercase().as_str());
                Ok((
                    t.rows.iter().map(|r| r[xc]).collect(),
                    t.rows.iter().map(|r| r[yc]).collect(),
                    lonlat.then(|| "EPSG:4326".to_string()),
                ))
            }
        }
    }

    /// The records kept.
    pub fn subset(&self, keep: &[bool]) -> Occurrences {
        match self {
            Occurrences::Vector(v) => {
                let idx: Vec<usize> = (0..v.len()).filter(|&k| keep[k]).collect();
                let attributes = v
                    .attributes
                    .iter()
                    .map(|(n, c)| {
                        let c = match c {
                            Column::Numbers(x) => Column::Numbers(idx.iter().map(|&k| x[k]).collect()),
                            Column::Texts(x) => Column::Texts(idx.iter().map(|&k| x[k].clone()).collect()),
                        };
                        (n.clone(), c)
                    })
                    .collect();
                Occurrences::Vector(Arc::new(Vector {
                    geometries: idx.iter().map(|&k| v.geometries[k].clone()).collect(),
                    attributes,
                    crs: v.crs.clone(),
                }))
            }
            Occurrences::Table(t) => {
                let mut out = Table::new(t.columns.clone());
                out.rows = t.rows.iter().zip(keep).filter(|(_, k)| **k).map(|(r, _)| r.clone()).collect();
                Occurrences::Table(out)
            }
        }
    }
}

/// Coordinates from one coordinate system to another (PROJ through GDAL); unchanged when either is unknown or
/// they are the same.
pub fn to_crs(xs: &mut [f64], ys: &mut [f64], src: Option<&str>, dst: Option<&str>) -> Result<(), String> {
    let (Some(src), Some(dst)) = (src, dst) else { return Ok(()) };
    if terraflux_gdal::same_crs(src, dst)? {
        return Ok(());
    }
    let ok: Vec<usize> = (0..xs.len()).filter(|&k| xs[k].is_finite() && ys[k].is_finite()).collect();
    let (mut ox, mut oy): (Vec<f64>, Vec<f64>) = ok.iter().map(|&k| (xs[k], ys[k])).unzip();
    terraflux_gdal::Transform::new(src, dst)?.points(&mut ox, &mut oy);
    for (j, &k) in ok.iter().enumerate() {
        xs[k] = ox[j];
        ys[k] = oy[j];
    }
    Ok(())
}

/// The grid's extent and cells: xmin, xmax, ymin, ymax, xres, yres, ny, nx.
pub fn grid_of(g: &Grid) -> (f64, f64, f64, f64, f64, f64, usize, usize) {
    let t = g.transform;
    let (ny, nx) = (g.height, g.width);
    let (xmin, ymax) = (t[0], t[3]);
    let (xmax, ymin) = (xmin + nx as f64 * t[1], ymax + ny as f64 * t[5]);
    (xmin, xmax, ymin, ymax, (xmax - xmin) / nx as f64, (ymax - ymin) / ny as f64, ny, nx)
}

/// The cell of each point (-1 outside), landschaft's grid_cells.
pub fn grid_cells(xs: &[f64], ys: &[f64], g: &Grid) -> Vec<i64> {
    let (xmin, xmax, ymin, ymax, xres, yres, ny, nx) = grid_of(g);
    xs.iter()
        .zip(ys)
        .map(|(&x, &y)| {
            let col = if x >= xmin && x < xmax {
                ((x - xmin) / xres).trunc()
            } else if x == xmax {
                (nx - 1) as f64
            } else {
                -1.0
            };
            let row = if y > ymin && y <= ymax {
                ((ymax - y) / yres).trunc()
            } else if y == ymin {
                (ny - 1) as f64
            } else {
                -1.0
            };
            if col >= 0.0 && row >= 0.0 && col < nx as f64 && row < ny as f64 {
                row as i64 * nx as i64 + col as i64
            } else {
                -1
            }
        })
        .collect()
}

/// The grid's coordinate system (WKT), if known.
pub fn grid_crs(g: &Grid) -> Option<String> {
    g.geo.wkt.clone()
}

/// spThin's thinning: the records left at least `distance` metres apart (great-circle), the best of `reps`
/// repetitions (the first that keeps the most); and how many each kept, most first.
pub fn thin(
    occ: &Occurrences,
    distance: f64,
    reps: i32,
    seed: i32,
    x: Option<&str>,
    y: Option<&str>,
) -> Result<(Occurrences, Vec<i64>), String> {
    if distance.is_nan() || distance < 0.0 {
        return Err("distance must be >= 0 (metres)".into());
    }
    if reps < 1 {
        return Err("reps must be >= 1".into());
    }
    let (mut lon, mut lat, crs) = occ.coordinates(x, y)?;
    to_crs(&mut lon, &mut lat, crs.as_deref(), Some("EPSG:4326"))?;
    let r = ffi::sdm_thin_spthin(&lon, &lat, distance / 1000.0, reps, seed).map_err(|e| e.to_string())?;
    let n = lon.len();
    let mut order: Vec<usize> = (0..reps as usize).collect();
    order.sort_by_key(|&k| std::cmp::Reverse(r.kept[k]));
    let best = order[0];
    let keep: Vec<bool> = (0..n).map(|k| r.keep[best * n + k] != 0).collect();
    Ok((occ.subset(&keep), order.iter().map(|&k| r.kept[k]).collect()))
}

/// A record per cell of the grid (the first), those outside it left out; with `drop_missing` those where any
/// layer has no value too.
pub fn one_per_cell(
    occ: &Occurrences,
    grid: &Grid,
    layers: &[&[f64]],
    drop_missing: bool,
    x: Option<&str>,
    y: Option<&str>,
) -> Result<Occurrences, String> {
    let (mut xs, mut ys, crs) = occ.coordinates(x, y)?;
    to_crs(&mut xs, &mut ys, crs.as_deref(), grid_crs(grid).as_deref())?;
    let cells = grid_cells(&xs, &ys, grid);
    let mut keep: Vec<bool> = cells.iter().map(|&c| c >= 0).collect();
    if drop_missing {
        for (k, &c) in cells.iter().enumerate() {
            if keep[k] && layers.iter().any(|l| l[c as usize].is_nan()) {
                keep[k] = false;
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    let first: Vec<bool> = cells.iter().zip(&keep).map(|(&c, &k)| k && seen.insert(c)).collect();
    Ok(occ.subset(&first))
}

/// The settings of background.
pub struct Background<'a> {
    pub n: usize,
    pub occurrences: Option<(&'a Occurrences, Option<&'a str>, Option<&'a str>)>,
    pub area: Option<&'a Vector>,
    pub bias: Option<&'a [f64]>,
    /// The band of distances from the records (from, to).
    pub distance: Option<(f64, f64)>,
    pub envelope: Option<f64>,
    pub exclude_occurrences: bool,
    pub tryf: f64,
    pub lonlat_correction: bool,
    pub seed: i32,
}

/// R's quantile type 7 (NaN left out).
fn quantile7(x: &[f64], probs: &[f64]) -> Vec<f64> {
    let mut x: Vec<f64> = x.iter().copied().filter(|v| !v.is_nan()).collect();
    x.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = x.len();
    probs
        .iter()
        .map(|&p| {
            let index = 1.0 + (n.saturating_sub(1)) as f64 * p;
            let (lo, hi) = (index.floor() as usize, index.ceil() as usize);
            let mut q = x[lo - 1];
            if index > lo as f64 && x[hi - 1] != q {
                let h = index - lo as f64;
                q = (1.0 - h) * q + h * x[hi - 1];
            }
            q
        })
        .collect()
}

/// Whether the grid may be lon/lat: its coordinate system is geographic, or (none known) its extent fits.
fn could_be_lonlat(g: &Grid) -> bool {
    if let Some(wkt) = grid_crs(g) {
        return terraflux_gdal::is_geographic(&wkt).unwrap_or(false);
    }
    let (xmin, xmax, ymin, ymax, ..) = grid_of(g);
    xmin > -365.0 && xmax < 365.0 && ymin > -90.1 && ymax < 90.1
}

/// Background points drawn from the cells of the predictors with values in every layer (landschaft's
/// background, dismo's randomPoints): the drawn cells and warnings.
pub fn background(grid: &Grid, layers: &[&[f64]], s: &Background) -> Result<(Vec<i64>, Vec<String>), String> {
    let mut warnings = Vec::new();
    let (xmin, _xmax, _ymin, ymax, xres, yres, ny, nx) = grid_of(grid);
    let ncell = ny * nx;
    let mut mask: Vec<bool> = (0..ncell).map(|k| layers.iter().all(|l| !l[k].is_nan())).collect();
    let mut occ_cells: Vec<i64> = Vec::new();
    if let Some((occ, x, y)) = s.occurrences {
        let (mut xs, mut ys, crs) = occ.coordinates(x, y)?;
        to_crs(&mut xs, &mut ys, crs.as_deref(), grid_crs(grid).as_deref())?;
        occ_cells = grid_cells(&xs, &ys, grid).into_iter().filter(|&c| c >= 0).collect();
    }
    if let Some(area) = s.area {
        let mut inside = vec![0.0; ncell];
        let values = vec![1.0; area.len()];
        terraflux_gdal::rasterize(&mut inside, nx, ny, grid.transform, &area.geometries, &values, true, false)?;
        for k in 0..ncell {
            mask[k] &= inside[k] == 1.0;
        }
    }
    if let Some((lo, hi)) = s.distance {
        if occ_cells.is_empty() {
            return Err("distance needs occurrences".into());
        }
        let mut target = vec![0u8; ncell];
        for &c in &occ_cells {
            target[c as usize] = 1;
        }
        let d = if could_be_lonlat(grid) {
            ffi::nlm_distance(&target, ny as i64, nx as i64, 1, 1.0, xmin, ymax, xres, yres)
        } else {
            ffi::nlm_distance(&target, ny as i64, nx as i64, 0, xres, 0.0, 0.0, 1.0, 1.0)
        }
        .map_err(|e| e.to_string())?;
        for k in 0..ncell {
            mask[k] &= d[k] > lo && d[k] <= hi;
        }
    }
    if let Some(q) = s.envelope {
        if occ_cells.is_empty() {
            return Err("envelope needs occurrences".into());
        }
        if !(0.0..0.5).contains(&q) {
            return Err("envelope must be in [0, 0.5)".into());
        }
        let mut cells = occ_cells.clone();
        cells.sort_unstable();
        cells.dedup();
        let mut inside = vec![true; ncell];
        for l in layers {
            let at: Vec<f64> = cells.iter().map(|&c| l[c as usize]).collect();
            let b = quantile7(&at, &[q, 1.0 - q]);
            for k in 0..ncell {
                inside[k] &= l[k] >= b[0] && l[k] <= b[1];
            }
        }
        for k in 0..ncell {
            mask[k] &= !inside[k];
        }
    }
    if let Some(b) = s.bias {
        if b.len() != ncell {
            return Err("bias must be a raster on the grid of env".into());
        }
        if (0..ncell).any(|k| mask[k] && b[k] < 0.0) {
            return Err("bias weights must not be negative".into());
        }
        for k in 0..ncell {
            mask[k] &= !b[k].is_nan();
        }
    }
    let mut rng = ffi::rrng_new(s.seed);
    let tryf = (s.tryf.round_ties_even() as usize).max(1);
    let mut n = s.n;
    if n > ncell {
        warnings.push(format!("n is larger than the number of cells: {ncell} points"));
        n = ncell;
    }
    let mut nn = (n * tryf).max(10);
    let cells: Vec<i64> = (0..ncell).filter(|&k| mask[k]).map(|k| k as i64).collect();
    if cells.is_empty() {
        return Err("no cell with data to draw from".into());
    }
    let lonlat = s.lonlat_correction && could_be_lonlat(grid);
    let dx: Vec<f64> = if lonlat {
        let yc: Vec<f64> = (0..ny).map(|r| ymax - (r as f64 + 0.5) * yres).collect();
        let width = ffi::cost_geodesic(&vec![0.0; ny], &yc, &vec![xres; ny], &yc).map_err(|e| e.to_string())?;
        cells.iter().map(|&c| width[c as usize / nx]).collect()
    } else {
        Vec::new()
    };
    let pick = if let Some(b) = s.bias {
        let mut prob: Vec<f64> = cells.iter().map(|&c| b[c as usize]).collect();
        if lonlat {
            let m = dx.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            for (p, d) in prob.iter_mut().zip(&dx) {
                *p *= d / m;
            }
        }
        nn = nn.min(prob.iter().filter(|p| **p > 0.0).count());
        ffi::rrng_sample_int(rng.pin_mut(), cells.len() as i64, nn as i64, false, &prob)
    } else {
        nn = nn.min(cells.len());
        ffi::rrng_sample_int(rng.pin_mut(), cells.len() as i64, nn as i64, false, &dx)
    }
    .map_err(|e| e.to_string())?;
    let mut drawn: Vec<i64> = pick.iter().map(|&k| cells[k as usize - 1]).collect();
    if s.exclude_occurrences && !occ_cells.is_empty() {
        let occ: std::collections::HashSet<i64> = occ_cells.iter().copied().collect();
        drawn.retain(|c| !occ.contains(c));
    }
    if drawn.len() > n {
        let k =
            ffi::rrng_sample_int(rng.pin_mut(), drawn.len() as i64, n as i64, false, &[]).map_err(|e| e.to_string())?;
        drawn = k.iter().map(|&i| drawn[i as usize - 1]).collect();
    } else if drawn.len() < n {
        if (drawn.len() as f64) < 0.1 * n as f64 {
            return Err(format!("only {} cells available for {n} points: widen the area", drawn.len()));
        }
        warnings.push(format!("only {} background points could be drawn ({n} requested)", drawn.len()));
    }
    Ok((drawn, warnings))
}

/// The centres of cells as points, with their cell.
pub fn cell_points(grid: &Grid, cells: &[i64]) -> Vector {
    let (xmin, _, _, ymax, xres, yres, _, nx) = grid_of(grid);
    let geometries = cells
        .iter()
        .map(|&c| {
            let (row, col) = (c / nx as i64, c % nx as i64);
            Geometry::Point([xmin + (col as f64 + 0.5) * xres, ymax - (row as f64 + 0.5) * yres])
        })
        .collect();
    Vector {
        geometries,
        attributes: vec![("cell".into(), Column::Numbers(cells.iter().map(|&c| c as f64).collect()))],
        crs: grid_crs(grid),
    }
}
