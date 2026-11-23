//! GEOS's nearest points of two geometries and the measure of a point along a line (shapely's `shortest_line` and
//! `line_locate_point`), for pairs of features of two layers: the coordinates are GEOS's own, bit for bit.

use terraflux_raster::vector::Geometry;

use super::{G, Session, geos};
use crate::Result;

/// The geometries of a layer read into GEOS when first needed, destroyed when dropped.
struct Loaded<'s, 'a> {
    s: &'s Session<'a>,
    reader: G,
    items: Vec<Option<G>>,
}

impl<'s, 'a> Loaded<'s, 'a> {
    fn new(s: &'s Session<'a>, n: usize) -> Result<Self> {
        let reader = unsafe { (s.a.GEOSWKBReader_create_r)(s.ctx) };
        if reader.is_null() {
            return Err("GEOS could not make the WKB reader".into());
        }
        Ok(Loaded { s, reader, items: vec![None; n] })
    }

    unsafe fn get(&mut self, g: &[Geometry], i: usize) -> Result<G> {
        if let Some(h) = self.items[i] {
            return Ok(h);
        }
        let wkb = g[i].to_wkb();
        let h = unsafe { (self.s.a.GEOSWKBReader_read_r)(self.s.ctx, self.reader, wkb.as_ptr(), wkb.len()) };
        if h.is_null() {
            return Err(format!("GEOS could not read the geometry of feature {i}"));
        }
        self.items[i] = Some(h);
        Ok(h)
    }
}

impl Drop for Loaded<'_, '_> {
    fn drop(&mut self) {
        unsafe {
            for h in self.items.iter().flatten() {
                self.s.destroy(*h);
            }
            (self.s.a.GEOSWKBReader_destroy_r)(self.s.ctx, self.reader);
        }
    }
}

/// The nearest points of `a[i]` and `b[j]` for each pair (i, j): the point on the first and on the second (GEOS's
/// `GEOSNearestPoints`, the two ends of shapely's `shortest_line`). The geometries must not be empty.
pub fn nearest_points(a: &[Geometry], b: &[Geometry], pairs: &[(usize, usize)]) -> Result<Vec<[[f64; 2]; 2]>> {
    let g = geos().map_err(String::from)?;
    let s = g.session()?;
    let (mut la, mut lb) = (Loaded::new(&s, a.len())?, Loaded::new(&s, b.len())?);
    let api = s.a;
    let mut out = Vec::with_capacity(pairs.len());
    for &(i, j) in pairs {
        unsafe {
            let (ha, hb) = (la.get(a, i)?, lb.get(b, j)?);
            let cs = (api.GEOSNearestPoints_r)(s.ctx, ha, hb);
            if cs.is_null() {
                return Err(format!("GEOS could not find the nearest points of features {i} and {j}"));
            }
            let mut p = [[0.0f64; 2]; 2];
            let mut n: std::ffi::c_uint = 0;
            let ok = (api.GEOSCoordSeq_getSize_r)(s.ctx, cs, &mut n) != 0
                && n >= 2
                && (0..2).all(|k| {
                    (api.GEOSCoordSeq_getXY_r)(s.ctx, cs, k as std::ffi::c_uint, &mut p[k][0], &mut p[k][1]) != 0
                });
            (api.GEOSCoordSeq_destroy_r)(s.ctx, cs);
            if !ok {
                return Err(format!("GEOS could not read the nearest points of features {i} and {j}"));
            }
            out.push(p);
        }
    }
    Ok(out)
}

/// The measure of each point along its line, `(feature of lines, point)` each (GEOS's `GEOSProject`: the distance
/// from the start of the line to the closest point on it; along a multilinestring, its parts in sequence).
pub fn project(lines: &[Geometry], items: &[(usize, [f64; 2])]) -> Result<Vec<f64>> {
    let g = geos().map_err(String::from)?;
    let s = g.session()?;
    let mut ll = Loaded::new(&s, lines.len())?;
    let api = s.a;
    let mut out = Vec::with_capacity(items.len());
    for &(i, p) in items {
        unsafe {
            let h = ll.get(lines, i)?;
            let pt = (api.GEOSGeom_createPointFromXY_r)(s.ctx, p[0], p[1]);
            if pt.is_null() {
                return Err("GEOS could not make a point".into());
            }
            let m = (api.GEOSProject_r)(s.ctx, h, pt);
            s.destroy(pt);
            if m < 0.0 {
                return Err(format!("GEOS could not measure along feature {i}"));
            }
            out.push(m);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_points_and_measures_on_a_hand_case() {
        if let Err(e) = geos() {
            eprintln!("skipped: {e}");
            return;
        }
        let line = Geometry::LineString(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]);
        let near = nearest_points(&[Geometry::Point([4.0, 3.0])], std::slice::from_ref(&line), &[(0, 0)]).unwrap();
        assert_eq!(near, vec![[[4.0, 3.0], [4.0, 0.0]]]);
        let far = nearest_points(&[Geometry::Point([12.0, 6.0])], std::slice::from_ref(&line), &[(0, 0)]).unwrap();
        assert_eq!(far[0][1], [10.0, 6.0]);
        let m = project(std::slice::from_ref(&line), &[(0, [4.0, 0.0]), (0, [10.0, 6.0]), (0, [-5.0, 1.0])]).unwrap();
        assert_eq!(m, vec![4.0, 16.0, 0.0]);
    }
}
