//! Resampling a raster to another grid of the same coordinate reference system: the method by the kind of
//! data (classes: the nearest cell, or the most common class when the new cells are larger; continuous
//! values: bilinear, or the mean when the new cells are larger).

use crate::{Band, DataType, Error, Grid, Raster, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// The cell whose centre is nearest.
    Nearest,
    /// The most common class among the cells within (the smallest in a tie).
    Mode,
    /// Bilinear interpolation of the four nearest centres (missing ones left out).
    Bilinear,
    /// The mean of the cells within (missing ones left out).
    Average,
    /// Mode or nearest for classes, average or bilinear for continuous values.
    Auto,
}

impl Method {
    pub fn parse(s: &str) -> Option<Method> {
        Some(match s {
            "nearest" => Method::Nearest,
            "mode" => Method::Mode,
            "bilinear" => Method::Bilinear,
            "average" => Method::Average,
            "auto" => Method::Auto,
            _ => return None,
        })
    }
}

/// Whether a band's values look like classes: whole numbers, at most 256 of them.
pub fn categorical(values: &[f64]) -> bool {
    categorical_at_most(values, 256)
}

/// Whether a band's values are whole numbers, at most `max` distinct.
fn categorical_at_most(values: &[f64], max: usize) -> bool {
    let mut seen: Vec<f64> = Vec::new();
    for &v in values {
        if v.is_nan() {
            continue;
        }
        if v.fract() != 0.0 {
            return false;
        }
        if !seen.contains(&v) {
            seen.push(v);
            if seen.len() > max {
                return false;
            }
        }
    }
    true
}

/// The method `Auto` stands for, for a band going from cells of `from` to cells of `to`.
pub fn choose(values: &[f64], from: &Grid, to: &Grid) -> Method {
    choose_typed(values, None, from, to)
}

/// As [`choose`], knowing the type of the file: integer rasters hold classes when they have up to 256
/// values, float rasters only when they hold up to 32 whole values (else they are continuous variables).
pub fn choose_typed(values: &[f64], dtype: Option<DataType>, from: &Grid, to: &Grid) -> Method {
    let coarser =
        to.resolution().0 > from.resolution().0 * 1.000001 || to.resolution().1 > from.resolution().1 * 1.000001;
    let classes = match dtype {
        Some(t) if t.is_float() => categorical_at_most(values, 32),
        _ => categorical(values),
    };
    match (classes, coarser) {
        (true, true) => Method::Mode,
        (true, false) => Method::Nearest,
        (false, true) => Method::Average,
        (false, false) => Method::Bilinear,
    }
}

fn check_grids(from: &Grid, to: &Grid) -> Result<()> {
    if from.transform[2] != 0.0 || from.transform[4] != 0.0 || to.transform[2] != 0.0 || to.transform[4] != 0.0 {
        return Err(Error::Raster("rotated grids cannot be resampled".into()));
    }
    if from.geo.geokeys.is_some() && to.geo.geokeys.is_some() && from.geo.epsg() != to.geo.epsg() {
        return Err(Error::Raster(format!(
            "the grids have different coordinate reference systems ({:?} and {:?}): reproject first (e.g. gdalwarp)",
            from.geo.epsg(),
            to.geo.epsg()
        )));
    }
    Ok(())
}

/// One band (cells of `from`, NaN missing) on the grid `to`.
pub fn resample_band(values: &[f64], from: &Grid, to: &Grid, method: Method) -> Result<Vec<f64>> {
    check_grids(from, to)?;
    let method = if method == Method::Auto { choose(values, from, to) } else { method };
    let (sx0, sdx, sy0, sdy) = (from.transform[0], from.transform[1], from.transform[3], from.transform[5]);
    let (tx0, tdx, ty0, tdy) = (to.transform[0], to.transform[1], to.transform[3], to.transform[5]);
    let (sw, sh) = (from.width as i64, from.height as i64);
    let get = |r: i64, c: i64| -> f64 {
        if r < 0 || c < 0 || r >= sh || c >= sw { f64::NAN } else { values[(r * sw + c) as usize] }
    };
    let mut out = vec![f64::NAN; to.cells()];
    for r in 0..to.height {
        for c in 0..to.width {
            // the centre of the new cell in the source's cell coordinates
            let x = tx0 + (c as f64 + 0.5) * tdx;
            let y = ty0 + (r as f64 + 0.5) * tdy;
            let fc = (x - sx0) / sdx;
            let fr = (y - sy0) / sdy;
            let v = match method {
                Method::Nearest | Method::Auto => get(fr.floor() as i64, fc.floor() as i64),
                Method::Bilinear => {
                    let (cc, rr) = (fc - 0.5, fr - 0.5);
                    let (c0, r0) = (cc.floor(), rr.floor());
                    let (ac, ar) = (cc - c0, rr - r0);
                    let (mut sum, mut wsum) = (0.0, 0.0);
                    for (dr, wr) in [(0, 1.0 - ar), (1, ar)] {
                        for (dc, wc) in [(0, 1.0 - ac), (1, ac)] {
                            let w = wr * wc;
                            let v = get(r0 as i64 + dr, c0 as i64 + dc);
                            if w > 0.0 && !v.is_nan() {
                                sum += w * v;
                                wsum += w;
                            }
                        }
                    }
                    if wsum > 0.0 { sum / wsum } else { f64::NAN }
                }
                Method::Mode | Method::Average => {
                    // the source cells whose centres fall in the new cell
                    let (x0, x1) = (tx0 + c as f64 * tdx, tx0 + (c + 1) as f64 * tdx);
                    let (y0, y1) = (ty0 + r as f64 * tdy, ty0 + (r + 1) as f64 * tdy);
                    let ca = ((x0.min(x1) - sx0) / sdx).min((x0.max(x1) - sx0) / sdx);
                    let cb = ((x0.min(x1) - sx0) / sdx).max((x0.max(x1) - sx0) / sdx);
                    let ra = ((y0 - sy0) / sdy).min((y1 - sy0) / sdy);
                    let rb = ((y0 - sy0) / sdy).max((y1 - sy0) / sdy);
                    let (c_lo, c_hi) = ((ca - 0.5).ceil() as i64, (cb - 0.5).ceil() as i64 - 1);
                    let (r_lo, r_hi) = ((ra - 0.5).ceil() as i64, (rb - 0.5).ceil() as i64 - 1);
                    if c_hi < c_lo || r_hi < r_lo {
                        get(fr.floor() as i64, fc.floor() as i64)
                    } else if method == Method::Average {
                        let (mut s, mut n) = (0.0, 0.0);
                        for rr in r_lo..=r_hi {
                            for cc in c_lo..=c_hi {
                                let v = get(rr, cc);
                                if !v.is_nan() {
                                    s += v;
                                    n += 1.0;
                                }
                            }
                        }
                        if n > 0.0 { s / n } else { f64::NAN }
                    } else {
                        let mut counts: Vec<(f64, usize)> = Vec::new();
                        for rr in r_lo..=r_hi {
                            for cc in c_lo..=c_hi {
                                let v = get(rr, cc);
                                if v.is_nan() {
                                    continue;
                                }
                                match counts.iter_mut().find(|(k, _)| *k == v) {
                                    Some(e) => e.1 += 1,
                                    None => counts.push((v, 1)),
                                }
                            }
                        }
                        counts
                            .into_iter()
                            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.partial_cmp(&a.0).unwrap()))
                            .map_or(f64::NAN, |(v, _)| v)
                    }
                }
            };
            out[r * to.width + c] = v;
        }
    }
    Ok(out)
}

/// Every band of a raster on another grid (of the same coordinate reference system); the method chosen per
/// band with `Auto`. Classes keep the raster's type; interpolated values become floats.
pub fn resample(r: &Raster, to: &Grid, method: Method) -> Result<(Raster, Vec<Method>)> {
    let mut bands = Vec::with_capacity(r.bands.len());
    let mut used = Vec::new();
    let mut float = false;
    for b in &r.bands {
        let v = b.masked(r.nodata);
        let m = if method == Method::Auto { choose_typed(&v, Some(r.dtype), &r.grid, to) } else { method };
        float |= matches!(m, Method::Bilinear | Method::Average);
        used.push(m);
        bands.push(Band { name: b.name.clone(), data: resample_band(&v, &r.grid, to, m)? });
    }
    let (dtype, nodata) =
        if float && !r.dtype.is_float() { (DataType::F32, Some(f64::NAN)) } else { (r.dtype, r.nodata) };
    let mut grid = to.clone();
    if grid.geo.geokeys.is_none() {
        grid.geo = r.grid.geo.clone();
    }
    Ok((Raster { grid, bands, dtype, nodata }, used))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(w: usize, h: usize, x0: f64, y0: f64, res: f64) -> Grid {
        let mut g = Grid::new(w, h);
        g.transform = [x0, res, 0.0, y0, 0.0, -res];
        g
    }

    #[test]
    fn same_grid_gives_the_same_cells() {
        let g = grid(3, 2, 0.0, 2.0, 1.0);
        let v = vec![1.0, 2.0, 3.0, 4.0, f64::NAN, 6.0];
        for m in [Method::Nearest, Method::Mode, Method::Average, Method::Bilinear] {
            let out = resample_band(&v, &g, &g, m).unwrap();
            for (a, b) in out.iter().zip(&v) {
                assert!(a == b || (a.is_nan() && b.is_nan()), "{m:?}: {a} != {b}");
            }
        }
    }

    #[test]
    fn coarser_cells_take_the_mode_or_the_mean() {
        let from = grid(4, 2, 0.0, 2.0, 1.0);
        let to = grid(2, 1, 0.0, 2.0, 2.0);
        let classes = vec![1.0, 1.0, 3.0, 2.0, 1.0, 2.0, 2.0, 3.0];
        assert_eq!(resample_band(&classes, &from, &to, Method::Auto).unwrap(), vec![1.0, 2.0]);
        let cont = vec![0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, f64::NAN];
        assert_eq!(resample_band(&cont, &from, &to, Method::Auto).unwrap(), vec![3.0, 12.5 / 3.0]);
    }

    #[test]
    fn shifted_and_finer_grids() {
        let from = grid(2, 1, 0.0, 1.0, 1.0);
        let to = grid(4, 1, 0.0, 1.0, 0.5);
        assert_eq!(resample_band(&[1.0, 2.0], &from, &to, Method::Nearest).unwrap(), vec![1.0, 1.0, 2.0, 2.0]);
        let b = resample_band(&[0.0, 10.0], &from, &to, Method::Bilinear).unwrap();
        assert_eq!(b, vec![0.0, 2.5, 7.5, 10.0]);
        // outside the source: missing
        let away = grid(2, 1, 5.0, 1.0, 1.0);
        assert!(resample_band(&[1.0, 2.0], &from, &away, Method::Nearest).unwrap().iter().all(|v| v.is_nan()));
    }

    #[test]
    fn other_crs_is_refused() {
        let mut a = grid(1, 1, 0.0, 1.0, 1.0);
        let mut b = a.clone();
        a.geo.geokeys = Some(vec![1, 1, 0, 1, 3072, 0, 1, 32633]);
        b.geo.geokeys = Some(vec![1, 1, 0, 1, 3072, 0, 1, 32634]);
        assert!(resample_band(&[1.0], &a, &b, Method::Nearest).is_err());
    }
}
