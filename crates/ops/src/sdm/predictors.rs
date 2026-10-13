//! The choice of predictors (landschaft's vif and select_predictors, after usdm's vif, vifstep and vifcor) and
//! the range of their spatial autocorrelation (landschaft's autocorrelation_range, after blockCV's
//! cv_spatial_autocor: gstat's variograms fitted to a sample of every layer). usdm's sampling of the cells with
//! R's generator, R's lm, cor and the variograms of landschaft's C++.

use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::Vector;

use super::extrapolation::Frame;
use super::points::{grid_crs, grid_of};

const ALL_CELLS_BELOW: usize = 7000;

/// Whether a grid is lon/lat (geographic, or its extent fits).
fn is_lonlat(g: &Grid) -> bool {
    if let Some(wkt) = grid_crs(g) {
        return terraflux_gdal::is_geographic(&wkt).unwrap_or(false);
    }
    let (xmin, xmax, ymin, ymax, ..) = grid_of(g);
    xmin >= -180.1 && xmax <= 180.1 && ymin >= -90.1 && ymax <= 90.1
}

/// Every row's weight on a lon/lat grid: |cos| of its latitude (R's cos).
fn row_weights(g: &Grid) -> Vec<f64> {
    let (_, _, _, ymax, _, yres, ny, _) = grid_of(g);
    let y: Vec<f64> = (0..ny).map(|r| std::f64::consts::PI * (ymax - (r as f64 + 0.5) * yres) / 180.0).collect();
    let sc = ffi::lsm_r_sincos(&y);
    sc[ny..].iter().map(|c| c.abs()).collect()
}

/// usdm's sample of the complete cells (terra's spatSample, regular or weighted by latitude).
fn sample_cells(
    g: &Grid,
    complete: &[bool],
    size: usize,
    rng: std::pin::Pin<&mut ffi::RRng>,
) -> Result<Vec<usize>, String> {
    let mut rng = rng;
    let (ny, nx) = (g.height, g.width);
    let ncell = ny * nx;
    let lonlat = is_lonlat(g);
    let esize = 2 * size * 5;
    if ncell <= 50 * esize {
        let nona: Vec<usize> = (0..ncell).filter(|&k| complete[k]).collect();
        if nona.is_empty() {
            return Ok(nona);
        }
        let take = esize.min(nona.len());
        let prob: Vec<f64> = if lonlat {
            let w = row_weights(g);
            nona.iter().map(|&c| w[c / nx]).collect()
        } else {
            Vec::new()
        };
        let i = ffi::rrng_sample_int(rng.as_mut(), nona.len() as i64, take as i64, false, &prob)
            .map_err(|e| e.to_string())?;
        return Ok(i.iter().take(2 * size).map(|&k| nona[k as usize - 1]).take(size).collect());
    }
    let mut found: Vec<usize> = Vec::new();
    for rnd in 0..10 {
        let cells: Vec<usize> = if lonlat {
            let w = row_weights(g);
            let rows = ffi::rrng_sample_int(rng.as_mut(), ny as i64, (2 * esize) as i64, true, &w)
                .map_err(|e| e.to_string())?;
            let cols = ffi::rrng_sample_int(rng.as_mut(), nx as i64, (2 * esize) as i64, true, &[])
                .map_err(|e| e.to_string())?;
            let mut seen = std::collections::HashSet::new();
            rows.iter()
                .zip(&cols)
                .map(|(&r, &c)| (r as usize - 1) * nx + (c as usize - 1))
                .filter(|c| seen.insert(*c))
                .collect()
        } else {
            let k = ffi::rrng_sample_int(rng.as_mut(), ncell as i64, ncell.min(esize) as i64, false, &[])
                .map_err(|e| e.to_string())?;
            k.iter().map(|&c| c as usize - 1).collect()
        };
        let cells: Vec<usize> = cells.into_iter().filter(|&c| complete[c]).take(2 * size).collect();
        if rnd == 0 {
            found = cells;
        } else {
            let mut seen: std::collections::HashSet<usize> = found.iter().copied().collect();
            found.extend(cells.into_iter().filter(|c| seen.insert(*c)));
        }
        if found.len() >= size {
            break;
        }
    }
    found.truncate(size);
    Ok(found)
}

/// The predictors of a stack clipped to an area (landschaft's clip with all_touched: the grid cut to the area's
/// bounds, the cells the area does not touch missing).
pub fn clip(grid: &Grid, layers: &[Vec<f64>], area: &Vector) -> Result<(Grid, Vec<Vec<f64>>), String> {
    let (ny, nx) = (grid.height, grid.width);
    let t = grid.transform;
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for g in &area.geometries {
        let pts: Vec<[f64; 2]> = match g {
            terraflux_raster::vector::Geometry::Polygon(rs) => rs.iter().flatten().copied().collect(),
            terraflux_raster::vector::Geometry::MultiPolygon(ps) => ps.iter().flatten().flatten().copied().collect(),
            _ => Vec::new(),
        };
        for p in pts {
            b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
        }
    }
    let [xmin, ymin, xmax, ymax] = b;
    let mut cols = [(xmin - t[0]) / t[1], (xmax - t[0]) / t[1]];
    cols.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut rows = [(ymax - t[3]) / t[5], (ymin - t[3]) / t[5]];
    rows.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (c0, c1) = (cols[0].floor().max(0.0) as usize, (cols[1].ceil() as usize).min(nx));
    let (r0, r1) = (rows[0].floor().max(0.0) as usize, (rows[1].ceil() as usize).min(ny));
    if c1 <= c0 || r1 <= r0 {
        return Err("the region does not overlap the raster".into());
    }
    let (h, w) = (r1 - r0, c1 - c0);
    let mut g = grid.clone();
    g.width = w;
    g.height = h;
    g.transform[0] = t[0] + c0 as f64 * t[1];
    g.transform[3] = t[3] + r0 as f64 * t[5];
    let mut inside = vec![0.0; h * w];
    terraflux_gdal::rasterize(&mut inside, w, h, g.transform, &area.geometries, &vec![1.0; area.len()], true, false)?;
    let out = layers
        .iter()
        .map(|l| {
            (0..h * w)
                .map(|k| {
                    let (r, c) = (k / w + r0, k % w + c0);
                    if inside[k] != 0.0 { l[r * nx + c] } else { f64::NAN }
                })
                .collect()
        })
        .collect();
    Ok((g, out))
}

/// The values the selection works on: every complete cell of the stack (or a sample of `size` of them on large
/// grids), or a table's complete rows (a sample of large ones).
pub fn values_of_stack(
    grid: &Grid,
    names: &[String],
    layers: &[Vec<f64>],
    size: Option<usize>,
    seed: i32,
) -> Result<Frame, String> {
    let n = grid.width * grid.height;
    let complete: Vec<bool> = (0..n).map(|k| layers.iter().all(|l| !l[k].is_nan())).collect();
    let cells: Vec<usize> = match size {
        Some(s) if n >= ALL_CELLS_BELOW => {
            let mut rng = ffi::rrng_new(seed);
            sample_cells(grid, &complete, s, rng.pin_mut())?
        }
        _ => (0..n).filter(|&k| complete[k]).collect(),
    };
    Ok(Frame { names: names.to_vec(), columns: layers.iter().map(|l| cells.iter().map(|&c| l[c]).collect()).collect() })
}

pub fn values_of_table(f: &Frame, size: Option<usize>, seed: i32) -> Result<Frame, String> {
    let t = f.dropna();
    let n = t.rows();
    match size {
        Some(s) if n >= ALL_CELLS_BELOW && n > s => {
            let mut rng = ffi::rrng_new(seed);
            let rows =
                ffi::rrng_sample_int(rng.pin_mut(), n as i64, s as i64, false, &[]).map_err(|e| e.to_string())?;
            Ok(Frame {
                names: t.names.clone(),
                columns: t.columns.iter().map(|c| rows.iter().map(|&r| c[r as usize - 1]).collect()).collect(),
            })
        }
        _ => Ok(t),
    }
}

/// usdm's VIF of the columns `which` (1 for a single column).
pub fn vif_of(f: &Frame, which: &[usize]) -> Result<Vec<f64>, String> {
    let m = f.names.len();
    if m < 2 {
        return Ok(vec![1.0; which.len()]);
    }
    let n = f.rows();
    let table: Vec<f64> = (0..n).flat_map(|i| f.columns.iter().map(move |c| c[i])).collect();
    let w: Vec<i64> = which.iter().map(|&k| k as i64).collect();
    ffi::sdm_vif(&table, n as i64, m as i64, &w).map_err(|e| e.to_string())
}

fn drop_column(f: &mut Frame, j: usize) -> String {
    f.columns.remove(j);
    f.names.remove(j)
}

/// usdm's vifstep: the predictor of the largest VIF (not kept) dropped until none reaches the threshold.
fn vifstep(f: &Frame, th: f64, keep: &[String]) -> Result<Vec<String>, String> {
    let mut f = f.clone();
    let mut excluded = Vec::new();
    while f.names.len() >= 2 {
        let v = vif_of(&f, &(0..f.names.len()).collect::<Vec<_>>())?;
        let cand: Vec<usize> = (0..f.names.len()).filter(|&i| !keep.contains(&f.names[i])).collect();
        if cand.is_empty() || cand.iter().all(|&i| v[i].is_nan()) {
            break;
        }
        let mut j = None;
        for &i in &cand {
            if !v[i].is_nan() && j.is_none_or(|b: usize| v[i] > v[b]) {
                j = Some(i);
            }
        }
        let j = j.unwrap();
        if v[j].is_nan() || v[j] < th {
            break;
        }
        excluded.push(drop_column(&mut f, j));
    }
    Ok(excluded)
}

/// usdm's vifcor: of the most correlated pair (not both kept) above the threshold, the one with the larger VIF
/// (or not kept) dropped, until no pair reaches it.
fn vifcor(f: &Frame, th: f64, keep: &[String], method: &str) -> Result<Vec<String>, String> {
    let mut f = f.clone();
    let mut excluded = Vec::new();
    while f.names.len() >= 2 {
        let n = f.names.len();
        let rows = f.rows();
        let colmajor: Vec<f64> = f.columns.iter().flatten().copied().collect();
        let c: Vec<f64> = ffi::lsm_r_cor(&colmajor, rows as i64, n as i64, method)
            .map_err(|e| e.to_string())?
            .iter()
            .map(|x| x.abs())
            .collect();
        let pairs: Vec<(usize, usize)> = (0..n).flat_map(|cc| (0..cc).map(move |r| (r, cc))).collect();
        let vals: Vec<f64> = pairs.iter().map(|&(r, cc)| c[r * n + cc]).collect();
        let mut order: Vec<usize> = (0..pairs.len()).collect();
        order.sort_by(|&a, &b| {
            let ka = (vals[a].is_nan(), if vals[a].is_nan() { 0.0 } else { -vals[a] });
            let kb = (vals[b].is_nan(), if vals[b].is_nan() { 0.0 } else { -vals[b] });
            ka.0.cmp(&kb.0).then(ka.1.partial_cmp(&kb.1).unwrap())
        });
        let pick = order
            .iter()
            .map(|&k| pairs[k])
            .find(|&(r, cc)| !keep.contains(&f.names[r]) || !keep.contains(&f.names[cc]));
        let Some((r, cc)) = pick else { break };
        if c[r * n + cc].is_nan() || c[r * n + cc] < th {
            break;
        }
        let (later, earlier) = (cc, r);
        let ex = if keep.contains(&f.names[later]) {
            earlier
        } else if keep.contains(&f.names[earlier]) {
            later
        } else {
            let v = vif_of(&f, &[later, earlier])?;
            if (v[0].is_nan() && !v[1].is_nan()) || v[1] > v[0] { earlier } else { later }
        };
        excluded.push(drop_column(&mut f, ex));
    }
    Ok(excluded)
}

/// The predictors selected: those kept, those excluded (in order) and the kept ones' VIF.
pub struct Selection {
    pub kept: Vec<String>,
    pub excluded: Vec<String>,
    pub vif: Vec<f64>,
}

pub fn select(
    f: &Frame,
    vif: Option<f64>,
    cor: Option<f64>,
    keep: &[String],
    method: &str,
) -> Result<Selection, String> {
    if vif.is_none() && cor.is_none() {
        return Err("give a vif and / or a cor threshold".into());
    }
    if !["pearson", "spearman", "kendall"].contains(&method) {
        return Err("method must be pearson, spearman or kendall".into());
    }
    if f.names.len() < 2 {
        return Err("need at least two predictors".into());
    }
    if let Some(k) = keep.iter().find(|k| !f.names.contains(k)) {
        return Err(format!("keep names an unknown predictor: {k:?}"));
    }
    let mut excluded = Vec::new();
    if let Some(c) = cor {
        excluded.extend(vifcor(f, c, keep, method)?);
    }
    if let Some(v) = vif {
        let rest: Vec<String> = f.names.iter().filter(|n| !excluded.contains(n)).cloned().collect();
        excluded.extend(vifstep(&f.select(&rest)?, v, keep)?);
    }
    let kept: Vec<String> = f.names.iter().filter(|n| !excluded.contains(n)).cloned().collect();
    let fk = f.select(&kept)?;
    let vif = vif_of(&fk, &(0..kept.len()).collect::<Vec<_>>())?;
    Ok(Selection { kept, excluded, vif })
}

const BOUNDS: [f64; 12] = [2.0, 4.0, 6.0, 9.0, 12.0, 15.0, 25.0, 35.0, 50.0, 65.0, 80.0, 100.0];

/// Vincenty-like great-circle distance (km) of landschaft's _gcdist (sp's spDists).
fn gcdist(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    if lon1 == lon2 && lat1 == lat2 {
        return 0.0;
    }
    let rad = std::f64::consts::PI / 180.0;
    let (a, f) = (6378.137, 1.0 / 298.257223563);
    let ff = (lat1 * rad + lat2 * rad) / 2.0;
    let g = (lat1 * rad - lat2 * rad) / 2.0;
    let l = (lon1 * rad - lon2 * rad) / 2.0;
    let sq = |x: f64| x * x;
    let (sg2, cg2) = (sq(g.sin()), sq(g.cos()));
    let (sf2, cf2) = (sq(ff.sin()), sq(ff.cos()));
    let (sl2, cl2) = (sq(l.sin()), sq(l.cos()));
    let s = sg2 * cl2 + cf2 * sl2;
    let c = cg2 * cl2 + sf2 * sl2;
    let w = (s / c).sqrt().atan();
    let r = (s * c).sqrt() / w;
    let d = 2.0 * w * a;
    let h1 = (3.0 * r - 1.0) / (2.0 * c);
    let h2 = (3.0 * r + 1.0) / (2.0 * s);
    d * (1.0 + f * h1 * sf2 * cg2 - f * h2 * cf2 * sg2)
}

/// A sample variogram: per class the pairs, their mean distance and the semivariance.
type Variogram = (Vec<f64>, Vec<f64>, Vec<f64>);

fn sample_variogram(x: &[f64], y: &[f64], z: &[f64], bounds: &[f64], lonlat: bool) -> Result<Variogram, String> {
    let v = ffi::sdm_sample_variogram(x, y, z, bounds, lonlat).map_err(|e| e.to_string())?;
    let k = v.len() / 3;
    let keep: Vec<usize> = (0..k).filter(|&i| v[i] > 0.0).collect();
    Ok((
        keep.iter().map(|&i| v[i]).collect(),
        keep.iter().map(|&i| v[k + i]).collect(),
        keep.iter().map(|&i| v[2 * k + i]).collect(),
    ))
}

/// A layer's fitted variogram: model, kappa, nugget, partial sill and range.
pub struct LayerFit {
    pub model: &'static str,
    pub kappa: f64,
    pub nugget: f64,
    pub sill: f64,
    pub range: f64,
}

fn autofit(x: &[f64], y: &[f64], z: &[f64], lonlat: bool) -> Result<LayerFit, String> {
    let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let diagonal =
        if lonlat { gcdist(min(x), min(y), max(x), max(y)) } else { (max(x) - min(x)).hypot(max(y) - min(y)) };
    let mut bounds: Vec<f64> = BOUNDS.iter().map(|b| b * diagonal * 0.35 / 100.0).collect();
    let (mut nh, mut dist, mut gamma) = sample_variogram(x, y, z, &bounds, lonlat)?;
    while nh.iter().any(|&n| n < 5.0) && bounds.len() > 1 {
        bounds.remove(0);
        (nh, dist, gamma) = sample_variogram(x, y, z, &bounds, lonlat)?;
    }
    let nugget0 = min(&gamma);
    let range0 = 0.1 * diagonal;
    let mut sorted = gamma.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let k = sorted.len();
    let median = if k % 2 == 1 { sorted[k / 2] } else { (sorted[k / 2 - 1] + sorted[k / 2]) / 2.0 };
    let sill0 = (max(&gamma) + median) / 2.0;
    let kappas: Vec<f64> = std::iter::once(0.05)
        .chain((2..=20).map(|i| format!("{:.10}", i as f64 * 0.1).parse::<f64>().unwrap()))
        .chain([5.0, 10.0])
        .collect();
    let mut best: Option<(LayerFit, f64)> = None;
    for model in ["Sph", "Exp", "Gau", "Ste"] {
        let ks: Vec<f64> = if model == "Ste" { kappas.clone() } else { vec![0.0] };
        for kappa in ks {
            let out = ffi::sdm_fit_variogram(model, kappa, &dist, &gamma, &nh, nugget0, sill0 - nugget0, range0)
                .map_err(|e| e.to_string())?;
            if out.is_empty() {
                continue;
            }
            let (nug, ps, r, sse) = (out[0], out[1], out[2], out[3]);
            if ps < 0.0 || r < 0.0 || !sse.is_finite() {
                continue;
            }
            if best.as_ref().is_none_or(|b| sse < b.1) {
                best = Some((LayerFit { model, kappa, nugget: nug, sill: ps, range: r }, sse));
            }
        }
    }
    best.map(|b| b.0).ok_or_else(|| "no variogram model could be fitted".into())
}

/// The ranges of autocorrelation of every layer (in metres on lon/lat grids), their median and the block size
/// it gives (in degrees on lon/lat grids).
pub fn autocorrelation_range(
    grid: &Grid,
    layers: &[Vec<f64>],
    n: usize,
    seed: i32,
    deg_to_metre: f64,
) -> Result<(f64, f64, Vec<LayerFit>), String> {
    let mut rng = ffi::rrng_new(seed);
    let (xmin, xmax, ymin, ymax, xres, yres, _, nx) = grid_of(grid);
    let lonlat = match grid_crs(grid) {
        Some(wkt) => terraflux_gdal::is_geographic(&wkt).unwrap_or(false),
        None => xmin > -365.0 && xmax < 365.0 && ymin > -90.1 && ymax < 90.1,
    };
    let mut fits = Vec::new();
    for l in layers {
        let size = l.len();
        let mut nn = n;
        if size < 5 * n {
            nn = n.min(l.iter().filter(|v| v.is_finite()).count());
        }
        let nona: Vec<usize> = (0..size).filter(|&k| !l[k].is_nan()).collect();
        let cells: Vec<usize> = if nn >= size {
            nona.iter().take(nn).copied().collect()
        } else {
            let take = (5 * nn).min(nona.len());
            let prob: Vec<f64> = if lonlat {
                let w = row_weights(grid);
                nona.iter().map(|&c| w[c / nx]).collect()
            } else {
                Vec::new()
            };
            let i = ffi::rrng_sample_int(rng.pin_mut(), nona.len() as i64, take as i64, false, &prob)
                .map_err(|e| e.to_string())?;
            i.iter().map(|&k| nona[k as usize - 1]).take(nn).collect()
        };
        let x: Vec<f64> = cells.iter().map(|&c| xmin + ((c % nx) as f64 + 0.5) * xres).collect();
        let y: Vec<f64> = cells.iter().map(|&c| ymax - ((c / nx) as f64 + 0.5) * yres).collect();
        let z: Vec<f64> = cells.iter().map(|&c| l[c]).collect();
        let mut fit = autofit(&x, &y, &z, lonlat)?;
        if lonlat {
            fit.range *= 1000.0;
        }
        fits.push(fit);
    }
    let mut ranges: Vec<f64> = fits.iter().map(|f| f.range).collect();
    ranges.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let k = ranges.len();
    let med = if k % 2 == 1 { ranges[k / 2] } else { (ranges[k / 2 - 1] + ranges[k / 2]) / 2.0 };
    let size = if lonlat { med / deg_to_metre } else { med };
    Ok((med, size, fits))
}
