//! Vectors over rasters larger than memory: the statistics of zones, the values at points and the features burnt,
//! by strips of rows read from the rasters on disk, with the same values as the rasters in memory.
//!
//! The zones' statistics are landschaft's `lsio::zonal_stats` on strips (zones whose rows fit a strip, their runs
//! moved to it): each zone's statistics depend only on its runs and its cells (it is summed in blocks of runs of
//! its own), so a strip gives the bits of the whole map. A zone too large for any strip is read run after run, its
//! statistics summed as the C++ sums them (the same blocks, merged in the same order) and its order statistics
//! from its values (or their counts, for whole numbers). The values at points are `lsio::extract` on the strips
//! that hold them (with a row around for bilinear), as landschaft's extract of a dask raster. Features are burnt
//! strip by strip into a GeoTIFF: polygons by their runs, lines by GDAL, points by their cell.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_ls_sys::ffi;
use terraflux_raster::tiff::Window;
use terraflux_raster::vector::Geometry;
use terraflux_raster::{DataType, Grid};

use crate::Result;
use crate::files::Rasters;

/// Layers on one grid, in memory or on disk, read by windows.
pub enum Layers {
    Memory {
        grid: Arc<Grid>,
        names: Vec<String>,
        layers: Vec<Arc<Vec<f64>>>,
    },
    /// The layers chosen of rasters on disk.
    Disk {
        names: Vec<String>,
        rasters: Arc<Rasters>,
        layers: Vec<usize>,
    },
}

impl Layers {
    pub fn grid(&self) -> &Grid {
        match self {
            Layers::Memory { grid, .. } => grid,
            Layers::Disk { rasters, layers, .. } => rasters.grid_of(layers[0]),
        }
    }

    pub fn names(&self) -> &[String] {
        match self {
            Layers::Memory { names, .. } | Layers::Disk { names, .. } => names,
        }
    }

    pub fn bands(&self) -> usize {
        self.names().len()
    }

    /// Whether every value is a whole number (rasters of an integer type).
    fn whole_numbers(&self) -> bool {
        match self {
            Layers::Memory { .. } => false,
            Layers::Disk { rasters, .. } => !rasters.dtype().is_float(),
        }
    }

    /// The cells of rows [y0, y1) and columns [x0, x1) of every layer, layer after layer; NaN where there is no
    /// value.
    pub fn window(&self, y0: usize, y1: usize, x0: usize, x1: usize) -> Result<Vec<f64>> {
        let mut out = Vec::with_capacity(self.bands() * (y1 - y0) * (x1 - x0));
        match self {
            Layers::Memory { grid, layers, .. } => {
                let nx = grid.width;
                for l in layers {
                    out.extend((y0..y1).flat_map(|y| l[y * nx + x0..y * nx + x1].iter().copied()));
                }
            }
            Layers::Disk { rasters, layers, .. } => {
                let win = Window { x0: x0 as i64, y0: y0 as i64, width: x1 - x0, height: y1 - y0 };
                for &b in layers {
                    out.extend(rasters.window(b, win)?);
                }
            }
        }
        Ok(out)
    }
}

/// The rows, columns and runs of a zone of the runs.
struct Extent {
    y0: usize,
    y1: usize,
    x0: usize,
    x1: usize,
}

fn extent(runs: &ffi::RunsOut, z: usize) -> Option<Extent> {
    let (a, b) = (runs.offsets[z] as usize, runs.offsets[z + 1] as usize);
    if a == b {
        return None;
    }
    let mut e = Extent { y0: usize::MAX, y1: 0, x0: usize::MAX, x1: 0 };
    for r in a..b {
        e.y0 = e.y0.min(runs.y[r] as usize);
        e.y1 = e.y1.max(runs.y[r] as usize + 1);
        e.x0 = e.x0.min(runs.x0[r] as usize);
        e.x1 = e.x1.max(runs.x1[r] as usize);
    }
    Some(e)
}

/// landschaft's statistics of every zone (polygons burnt into runs on the layers' grid) and layer, as
/// `ffi::io_zonal_stats` gives them for the whole map: (zones, layers, stats). Strips of at most `budget` cells
/// of a layer are read.
pub fn zonal_stats(
    layers: &Layers,
    runs: &ffi::RunsOut,
    stats: &[i32],
    percentiles: &[f64],
    budget: usize,
) -> Result<Vec<f64>> {
    let (nx, nb, ns) = (layers.grid().width, layers.bands(), stats.len());
    let nz = runs.offsets.len() - 1;
    let mut out = vec![f64::NAN; nz * nb * ns];
    let budget = budget.max(nx);
    // the zones by their first row; those without cells together, on no cell
    let extents: Vec<Option<Extent>> = (0..nz).map(|z| extent(runs, z)).collect();
    let mut order: Vec<usize> = (0..nz).collect();
    order.sort_by_key(|&z| extents[z].as_ref().map_or((0, 0), |e| (1, e.y0)));
    let empty: Vec<usize> = order.iter().copied().take_while(|&z| extents[z].is_none()).collect();
    if !empty.is_empty() {
        let none = ffi::RunsOut { offsets: vec![0; empty.len() + 1], y: vec![], x0: vec![], x1: vec![] };
        let got = ffi::io_zonal_stats(&vec![f64::NAN; nb], nb as i32, 1, 1, &none, stats, percentiles)?;
        for (k, &z) in empty.iter().enumerate() {
            out[z * nb * ns..(z + 1) * nb * ns].copy_from_slice(&got[k * nb * ns..(k + 1) * nb * ns]);
        }
    }
    let mut rest = &order[empty.len()..];
    while let Some(&first) = rest.first() {
        let e = extents[first].as_ref().unwrap();
        if (e.y1 - e.y0) * nx > budget {
            // a zone taller than a strip: its window, or run after run
            let got = if (e.y1 - e.y0) * (e.x1 - e.x0) <= budget {
                window_stats(layers, runs, &[first], e, stats, percentiles)?
            } else {
                large_zone(layers, runs, first, e, stats, percentiles, budget)?
            };
            out[first * nb * ns..(first + 1) * nb * ns].copy_from_slice(&got);
            rest = &rest[1..];
            continue;
        }
        // the zones that start in the strip and end in it
        let (y0, mut y1, mut n) = (e.y0, e.y1, 0);
        while n < rest.len() {
            let f = extents[rest[n]].as_ref().unwrap();
            if f.y1.max(y1) - y0 > budget / nx {
                break;
            }
            y1 = y1.max(f.y1);
            n += 1;
        }
        let zones = &rest[..n];
        let strip = Extent { y0, y1, x0: 0, x1: nx };
        let got = window_stats(layers, runs, zones, &strip, stats, percentiles)?;
        for (k, &z) in zones.iter().enumerate() {
            out[z * nb * ns..(z + 1) * nb * ns].copy_from_slice(&got[k * nb * ns..(k + 1) * nb * ns]);
        }
        rest = &rest[n..];
    }
    Ok(out)
}

/// The statistics of zones within a window, by landschaft's C++ on the window's cells (their runs moved to it).
fn window_stats(
    layers: &Layers,
    runs: &ffi::RunsOut,
    zones: &[usize],
    w: &Extent,
    stats: &[i32],
    percentiles: &[f64],
) -> Result<Vec<f64>> {
    let mut moved = ffi::RunsOut { offsets: vec![0], y: vec![], x0: vec![], x1: vec![] };
    for &z in zones {
        for r in runs.offsets[z] as usize..runs.offsets[z + 1] as usize {
            moved.y.push(runs.y[r] - w.y0 as i32);
            moved.x0.push(runs.x0[r] - w.x0 as i32);
            moved.x1.push(runs.x1[r] - w.x0 as i32);
        }
        moved.offsets.push(moved.y.len() as i64);
    }
    let values = layers.window(w.y0, w.y1, w.x0, w.x1)?;
    let (ny, nx) = ((w.y1 - w.y0) as i32, (w.x1 - w.x0) as i32);
    Ok(ffi::io_zonal_stats(&values, layers.bands() as i32, ny, nx, &moved, stats, percentiles)?)
}

/// lsio's accumulator of a zone's values: count, cells without data, sum, Welford's mean and squares, extremes.
#[derive(Clone, Copy)]
struct Acc {
    n: i64,
    missing: i64,
    sum: f64,
    mean: f64,
    m2: f64,
    lo: f64,
    hi: f64,
}

impl Default for Acc {
    fn default() -> Acc {
        Acc { n: 0, missing: 0, sum: 0.0, mean: 0.0, m2: 0.0, lo: f64::INFINITY, hi: f64::NEG_INFINITY }
    }
}

impl Acc {
    fn add(&mut self, v: f64) {
        self.n += 1;
        self.sum += v;
        let d = v - self.mean;
        self.mean += d / self.n as f64;
        self.m2 += d * (v - self.mean);
        // std::min and std::max: the first unless the second is smaller (larger)
        if v < self.lo {
            self.lo = v;
        }
        if self.hi < v {
            self.hi = v;
        }
    }

    fn merge(&mut self, o: &Acc) {
        if o.n == 0 {
            self.missing += o.missing;
            return;
        }
        if self.n == 0 {
            let keep = self.missing;
            *self = *o;
            self.missing += keep;
            return;
        }
        let total = (self.n + o.n) as f64;
        let d = o.mean - self.mean;
        self.mean += d * o.n as f64 / total;
        self.m2 += o.m2 + d * d * self.n as f64 * o.n as f64 / total;
        self.n += o.n;
        self.missing += o.missing;
        self.sum += o.sum;
        if o.lo < self.lo {
            self.lo = o.lo;
        }
        if self.hi < o.hi {
            self.hi = o.hi;
        }
    }
}

/// A zone's values for its order statistics: each value's count (whole numbers), or the values.
enum Values {
    Counts(BTreeMap<i64, u64>),
    All(Vec<f64>),
}

impl Values {
    fn push(&mut self, v: f64) {
        match self {
            Values::Counts(c) => *c.entry(v as i64).or_default() += 1,
            Values::All(a) => a.push(v),
        }
    }

    /// The values in increasing order, each with its count.
    fn sorted(self) -> Vec<(f64, u64)> {
        match self {
            Values::Counts(c) => c.into_iter().map(|(v, n)| (v as f64, n)).collect(),
            Values::All(mut a) => {
                a.sort_by(f64::total_cmp);
                let mut out: Vec<(f64, u64)> = Vec::new();
                for v in a {
                    match out.last_mut() {
                        Some((w, n)) if *w == v => *n += 1,
                        _ => out.push((v, 1)),
                    }
                }
                out
            }
        }
    }
}

/// lsio's blocks of runs of a zone: at least this many cells each (its sums are merged block by block).
const BLOCK_CELLS: i64 = 1 << 16;

/// The statistics of one zone too large for a window, its runs read in order (strips of its columns held), summed
/// as lsio sums them.
fn large_zone(
    layers: &Layers,
    runs: &ffi::RunsOut,
    z: usize,
    e: &Extent,
    stats: &[i32],
    percentiles: &[f64],
    budget: usize,
) -> Result<Vec<f64>> {
    let nb = layers.bands();
    let keep = stats.iter().any(|&s| s >= 9);
    let whole = layers.whole_numbers();
    let mut total = vec![Acc::default(); nb];
    let mut block = vec![Acc::default(); nb];
    let mut values: Vec<Values> =
        (0..nb).map(|_| if whole { Values::Counts(BTreeMap::new()) } else { Values::All(Vec::new()) }).collect();
    let width = e.x1 - e.x0;
    let rows = (budget / width).max(1);
    let mut held: Option<(usize, usize, Vec<f64>)> = None;
    let (a, b) = (runs.offsets[z] as usize, runs.offsets[z + 1] as usize);
    let mut cells = 0i64;
    for r in a..b {
        let y = runs.y[r] as usize;
        if !held.as_ref().is_some_and(|(s0, s1, _)| (*s0..*s1).contains(&y)) {
            let s1 = (y + rows).min(e.y1);
            held = Some((y, s1, layers.window(y, s1, e.x0, e.x1)?));
        }
        let (s0, s1, data) = held.as_ref().unwrap();
        let plane = (s1 - s0) * width;
        let (x0, x1) = (runs.x0[r] as usize - e.x0, runs.x1[r] as usize - e.x0);
        for band in 0..nb {
            let row = &data[band * plane + (y - s0) * width..];
            for &v in &row[x0..x1] {
                if v.is_nan() {
                    block[band].missing += 1;
                } else {
                    block[band].add(v);
                    if keep {
                        values[band].push(v);
                    }
                }
            }
        }
        cells += (x1 - x0) as i64;
        if cells >= BLOCK_CELLS || r + 1 == b {
            for band in 0..nb {
                total[band].merge(&block[band]);
                block[band] = Acc::default();
            }
            cells = 0;
        }
    }
    let mut out = Vec::with_capacity(nb * stats.len());
    for (acc, vals) in total.iter().zip(values) {
        out.extend(finish(acc, vals.sorted(), stats, percentiles));
    }
    Ok(out)
}

/// NumPy's linear interpolation between two order statistics (its rounding too), as lsio's.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    let d = b - a;
    if t >= 0.5 { b - d * (1.0 - t) } else { a + d * t }
}

/// lsio's statistics of a zone from its sums and its values in order (with their counts).
fn finish(a: &Acc, sorted: Vec<(f64, u64)>, stats: &[i32], percentiles: &[f64]) -> Vec<f64> {
    let n: u64 = sorted.iter().map(|(_, c)| c).sum();
    // the k-th smallest value (0-based)
    let kth = |k: u64| -> f64 {
        let mut seen = 0;
        for &(v, c) in &sorted {
            seen += c;
            if k < seen {
                return v;
            }
        }
        f64::NAN
    };
    let pct = |q: f64| -> f64 {
        if n == 0 {
            return f64::NAN;
        }
        let pos = q / 100.0 * (n - 1) as f64;
        let lo = pos.floor() as u64;
        let t = pos - lo as f64;
        lerp(kth(lo), kth((lo + 1).min(n - 1)), t)
    };
    // the most and least frequent values (ties: the smallest), and how many there are
    let (mut majority, mut minority) = (f64::NAN, f64::NAN);
    let (mut best, mut worst) = (0, u64::MAX);
    for &(v, c) in &sorted {
        if c > best {
            best = c;
            majority = v;
        }
        if c < worst {
            worst = c;
            minority = v;
        }
    }
    let empty = a.n == 0;
    let nan_if = |v: f64| if empty { f64::NAN } else { v };
    stats
        .iter()
        .zip(percentiles)
        .map(|(&s, &q)| match s {
            0 => a.n as f64,
            1 => a.missing as f64,
            2 => nan_if(a.sum),
            3 => nan_if(a.mean),
            4 => nan_if(a.lo),
            5 => nan_if(a.hi),
            6 => nan_if(a.hi - a.lo),
            7 => nan_if((a.m2 / a.n as f64).sqrt()),
            8 => nan_if(a.m2 / a.n as f64),
            9 => pct(50.0),
            10 => majority,
            11 => minority,
            12 => sorted.len() as f64,
            _ => pct(q),
        })
        .collect()
}

/// The values of every layer at points (x, y on the layers' grid): (points, layers), as `ffi::io_extract` of the
/// whole map; strips of `rows` rows read (with a row around), only those holding points.
pub fn extract(layers: &Layers, x: &[f64], y: &[f64], bilinear: bool, rows: usize) -> Result<Vec<f64>> {
    let grid = layers.grid();
    let (nx, ny, nb) = (grid.width, grid.height, layers.bands());
    let t = grid.transform;
    let mut out = vec![f64::NAN; x.len() * nb];
    // the points by their row (those outside the rows: no value)
    let mut by_row: Vec<(usize, usize)> = Vec::new();
    for (i, &py) in y.iter().enumerate() {
        let row = ((py - t[3]) / t[5]).floor();
        if row >= 0.0 && row < ny as f64 {
            by_row.push((row as usize, i));
        }
    }
    by_row.sort_unstable();
    let rows = rows.max(1);
    let mut k = 0;
    while k < by_row.len() {
        let y0 = by_row[k].0 / rows * rows;
        let n = rows.min(ny - y0);
        let mut m = k;
        while m < by_row.len() && by_row[m].0 < y0 + n {
            m += 1;
        }
        let (a, b) = (y0.saturating_sub(1), (y0 + n + 1).min(ny));
        let values = layers.window(a, b, 0, nx)?;
        let (px, py): (Vec<f64>, Vec<f64>) = by_row[k..m].iter().map(|&(_, i)| (x[i], y[i])).unzip();
        let got = ffi::io_extract(
            &values,
            nb as i32,
            (b - a) as i32,
            nx as i32,
            &t,
            &px,
            &py,
            bilinear,
            a as i32,
            ny as i32,
        )?;
        for (j, &(_, i)) in by_row[k..m].iter().enumerate() {
            out[i * nb..(i + 1) * nb].copy_from_slice(&got[j * nb..(j + 1) * nb]);
        }
        k = m;
    }
    Ok(out)
}

/// Features burnt into a GeoTIFF on a grid, a strip of rows at a time: polygons as their runs (lsio's burn of the
/// runs moved to the strip), lines by GDAL in the grid's pixels, points by their cell; the later features over the
/// earlier, or added up. The cells of the whole grid, but those GDAL draws for a line through cells' corners with
/// all_touched.
#[allow(clippy::too_many_arguments)]
pub fn burn_to_file(
    path: &std::path::Path,
    grid: &Grid,
    v: &terraflux_raster::vector::Vector,
    values: &[f64],
    fill: f64,
    all_touched: bool,
    add: bool,
    rows: usize,
) -> Result<()> {
    let ny = grid.height;
    let burner = Burner::new(grid, v, values, all_touched)?;
    // f32 when every value written is one
    let exact32 = |x: f64| x.is_nan() || f64::from(x as f32) == x;
    let dtype = if !add && values.iter().all(|&x| exact32(x)) && exact32(fill) { DataType::F32 } else { DataType::F64 };
    let nodata = fill.is_nan().then_some(f64::NAN);
    let mut out = crate::files::RowsWriter::create(path, grid, "burnt", dtype, nodata)?;
    let rows = rows.max(1);
    for y0 in (0..ny).step_by(rows) {
        let n = rows.min(ny - y0);
        out.push(burner.strip(y0, n, fill, add)?)?;
    }
    out.finish()
}

/// Features burnt strip by strip: polygons by their runs, lines by GDAL, points by their cell.
struct Burner<'a> {
    grid: &'a Grid,
    runs: Option<ffi::RunsOut>,
    poly_values: Vec<f64>,
    lines: Vec<&'a Geometry>,
    line_values: Vec<f64>,
    points: Vec<&'a Geometry>,
    point_values: Vec<f64>,
    all_touched: bool,
}

impl<'a> Burner<'a> {
    fn new(grid: &'a Grid, v: &'a terraflux_raster::vector::Vector, values: &[f64], all_touched: bool) -> Result<Self> {
        let polys: Vec<usize> = (0..v.len()).filter(|&i| v.geometries[i].is_polygonal()).collect();
        let geoms: Vec<&Geometry> = polys.iter().map(|&i| &v.geometries[i]).collect();
        let runs = if geoms.is_empty() { None } else { Some(crate::vector::runs(&geoms, grid, all_touched)?) };
        let poly_values: Vec<f64> = polys.iter().map(|&i| values[i]).collect();
        let is_line = |g: &Geometry| matches!(g, Geometry::LineString(_) | Geometry::MultiLineString(_));
        let (lines, line_values): (Vec<&Geometry>, Vec<f64>) =
            v.geometries.iter().zip(values).filter(|(g, _)| is_line(g)).map(|(g, &x)| (g, x)).unzip();
        let (points, point_values): (Vec<&Geometry>, Vec<f64>) =
            v.geometries.iter().zip(values).filter(|(g, _)| g.is_point()).map(|(g, &x)| (g, x)).unzip();
        Ok(Burner { grid, runs, poly_values, lines, line_values, points, point_values, all_touched })
    }

    /// Rows y0..y0 + n burnt.
    fn strip(&self, y0: usize, n: usize, fill: f64, add: bool) -> Result<Vec<f64>> {
        let (grid, nx) = (self.grid, self.grid.width);
        let mut strip = vec![fill; n * nx];
        if let Some(runs) = &self.runs {
            let mut moved = ffi::RunsOut { offsets: vec![0], y: vec![], x0: vec![], x1: vec![] };
            for z in 0..runs.offsets.len() - 1 {
                for r in runs.offsets[z] as usize..runs.offsets[z + 1] as usize {
                    let y = runs.y[r] as usize;
                    if (y0..y0 + n).contains(&y) {
                        moved.y.push((y - y0) as i32);
                        moved.x0.push(runs.x0[r]);
                        moved.x1.push(runs.x1[r]);
                    }
                }
                moved.offsets.push(moved.y.len() as i64);
            }
            ffi::io_burn_runs(&mut strip, n as i32, nx as i32, &moved, &self.poly_values, add)?;
        }
        // lines by GDAL in the whole grid's pixels, moved up by the strip's first row: the same cells as on the
        // whole grid, but where GDAL draws a line through cells' corners with all_touched (that depends on where a
        // strip starts, as with GDAL's own swaths)
        if !self.lines.is_empty() {
            let moved: Vec<Geometry> = self.lines.iter().map(|g| in_pixels(g, grid, y0)).collect();
            terraflux_gdal::rasterize(
                &mut strip,
                nx,
                n,
                [0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                &moved,
                &self.line_values,
                self.all_touched,
                add,
            )
            .map_err(|e| crate::Error::msg(format!("lines are burnt by GDAL: {e}")))?;
        }
        // points by their cell of the whole grid
        let t = &grid.transform;
        for (g, &value) in self.points.iter().zip(&self.point_values) {
            let mut cells = Vec::new();
            g.points(&mut |p| {
                let col = ((p[0] - t[0]) / t[1]).floor();
                let row = ((p[1] - t[3]) / t[5]).floor();
                if col >= 0.0 && row >= y0 as f64 && (col as usize) < nx && (row as usize) < y0 + n {
                    cells.push((row as usize - y0) * nx + col as usize);
                }
            });
            for c in cells {
                if add { strip[c] += value } else { strip[c] = value }
            }
        }
        Ok(strip)
    }
}

/// The distance from every cell to the nearest cell of the features (vector.distance) by strips into a GeoTIFF:
/// the features burnt strip by strip; from the bottom up, each cell's nearest burnt row below (kept in a file next
/// to the output); from the top down, the nearest above, the squared distances along the columns as
/// vector::distance_to computes them, and its lower envelopes along the rows. The same distances as in memory.
pub fn distance_to_file(
    path: &std::path::Path,
    grid: &Grid,
    v: &terraflux_raster::vector::Vector,
    all_touched: bool,
    rows: usize,
) -> Result<()> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let (nx, ny) = (grid.width, grid.height);
    let (dx, dy) = (grid.transform[1].abs(), grid.transform[5].abs());
    let burner = Burner::new(grid, v, &vec![1.0; v.len()], all_touched)?;
    let rows = rows.max(1);
    let starts: Vec<usize> = (0..ny).step_by(rows).collect();
    let io = |e: std::io::Error| crate::Error::msg(format!("{}: {e}", path.display()));
    let below_path = path.with_extension("below.tmp");
    let mut below_file =
        std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&below_path).map_err(io)?;
    let result = (|| -> Result<()> {
        below_file.set_len((nx * ny * 8) as u64).map_err(io)?;
        // from the bottom up: the nearest burnt row at or below every cell (u64::MAX: none)
        let mut next = vec![u64::MAX; nx];
        for &y0 in starts.iter().rev() {
            let n = rows.min(ny - y0);
            let mask = burner.strip(y0, n, 0.0, false)?;
            let mut out = vec![0u8; n * nx * 8];
            for r in (0..n).rev() {
                for x in 0..nx {
                    if mask[r * nx + x] != 0.0 {
                        next[x] = (y0 + r) as u64;
                    }
                    out[(r * nx + x) * 8..(r * nx + x + 1) * 8].copy_from_slice(&next[x].to_le_bytes());
                }
            }
            below_file.seek(SeekFrom::Start((y0 * nx * 8) as u64)).map_err(io)?;
            below_file.write_all(&out).map_err(io)?;
        }
        // from the top down: the nearest above, the squared distances along the columns, the rows' envelopes
        let mut writer = crate::files::RowsWriter::create(path, grid, "distance", DataType::F64, None)?;
        let mut last = vec![u64::MAX; nx];
        for &y0 in &starts {
            let n = rows.min(ny - y0);
            let mask = burner.strip(y0, n, 0.0, false)?;
            let mut raw = vec![0u8; n * nx * 8];
            below_file.seek(SeekFrom::Start((y0 * nx * 8) as u64)).map_err(io)?;
            below_file.read_exact(&mut raw).map_err(io)?;
            let mut g = vec![f64::INFINITY; n * nx];
            for r in 0..n {
                let y = y0 + r;
                for x in 0..nx {
                    if mask[r * nx + x] != 0.0 {
                        last[x] = y as u64;
                    }
                    let k = r * nx + x;
                    let mut v = f64::INFINITY;
                    if last[x] != u64::MAX {
                        v = ((y - last[x] as usize) as f64 * dy).powi(2);
                    }
                    let b = u64::from_le_bytes(raw[k * 8..(k + 1) * 8].try_into().unwrap());
                    if b != u64::MAX {
                        v = v.min(((b as usize - y) as f64 * dy).powi(2));
                    }
                    g[k] = v;
                }
            }
            let strip: Vec<f64> =
                (0..n).flat_map(|r| crate::vector::envelope_row(&g[r * nx..(r + 1) * nx], dx)).collect();
            writer.push(strip)?;
        }
        writer.finish()
    })();
    drop(below_file);
    std::fs::remove_file(&below_path).ok();
    result
}

/// A line in the pixels of a grid as GDAL's rasterizer puts it there, its rows counted from row `y0` (exactly: a
/// whole number of rows taken off).
fn in_pixels(g: &Geometry, grid: &Grid, y0: usize) -> Geometry {
    let t = &grid.transform;
    let px = |p: &[f64; 2]| -> [f64; 2] { [(p[0] - t[0]) / t[1], (p[1] - t[3]) / t[5] - y0 as f64] };
    match g {
        Geometry::LineString(l) => Geometry::LineString(l.iter().map(px).collect()),
        Geometry::MultiLineString(m) => {
            Geometry::MultiLineString(m.iter().map(|l| l.iter().map(px).collect()).collect())
        }
        g => g.clone(),
    }
}

/// The pieces of exactextract's coverage of the boxes of the polygons as lsio::exact_sums cuts them.
const EXACT_PIECE: i64 = 30_000_000;

/// exactextractr's sums of the layers over polygons with the values read under each polygon only (lsio's
/// exact_sums loop: the coverage of the feature's box on the whole grid, in pieces; value times fraction summed
/// row by row in the window of the piece, as a float), the same sums as with the layers in memory.
pub fn exact_sums(layers: &Layers, geoms: &[&Geometry]) -> Result<Vec<f64>> {
    use rayon::prelude::*;
    let grid = layers.grid();
    let bounds = crate::vector::exact_bounds(grid)?;
    let (ny, nx) = (grid.height as i64, grid.width as i64);
    let bands = layers.bands();
    let sums: Vec<Vec<f64>> = geoms
        .par_iter()
        .map(|g| -> Result<Vec<f64>> {
            let (mut xy, mut ring_off, mut poly_off) = (Vec::new(), vec![0i64], vec![0i64]);
            for poly in g.polygons() {
                for ring in poly {
                    ring.iter().for_each(|p| xy.extend_from_slice(p));
                    ring_off.push((xy.len() / 2) as i64);
                }
                poly_off.push((ring_off.len() - 1) as i64);
            }
            let mut sum = vec![0.0f64; bands];
            let pieces = ffi::io_exact_piece_count(&bounds, ny, nx, &xy, &ring_off, &poly_off, EXACT_PIECE)?;
            for k in 0..pieces {
                let p = ffi::io_exact_piece(&bounds, ny, nx, &xy, &ring_off, &poly_off, EXACT_PIECE, k)?;
                if p.empty {
                    continue;
                }
                let (i0, j0, wrows, wcols) = (p.i0 as usize, p.j0 as usize, p.wrows as usize, p.wcols as usize);
                let values = layers.window(i0, i0 + wrows, j0, j0 + wcols)?;
                let plane = wrows * wcols;
                for (l, s) in sum.iter_mut().enumerate() {
                    for i in 0..p.rows {
                        let wi = i + p.y_off;
                        if wi < 0 || wi >= p.wrows {
                            continue;
                        }
                        for j in 0..p.cols {
                            let wj = j + p.x_off;
                            if wj < 0 || wj >= p.wcols {
                                continue;
                            }
                            let pct = p.fraction[(i * p.cols + j) as usize];
                            let val = values[l * plane + wi as usize * wcols + wj as usize];
                            if pct > 0.0 && !val.is_nan() {
                                *s += val * f64::from(pct);
                            }
                        }
                    }
                }
            }
            Ok(sum.into_iter().map(|s| f64::from(s as f32)).collect())
        })
        .collect::<Result<_>>()?;
    Ok(sums.concat())
}
