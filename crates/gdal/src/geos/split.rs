//! Features cut by lines (the library's `tf.vector.split`: shapely's `union_all`, `boundary`, `union`, `polygonize`,
//! `difference`, `point_on_surface` and `contains`, all through GEOS's own functions, so the pieces are GEOS's bit for
//! bit). Each feature of one layer is cut by the union of the blades of another that intersect it.

use std::ffi::c_uint;

use terraflux_raster::vector::Geometry;

use super::shapes::Codec;
use super::{G, Geos, GeosIndex, Predicate, Session, Unary, geos, unary};
use crate::Result;

/// A piece with what orders it: its size (area, or length for a line) and the coordinates of its point on surface.
struct Piece {
    geom: Geometry,
    size: f64,
    x: f64,
    y: f64,
}

/// The piece for one of GEOS's geometries (borrowed): written out, measured and located.
unsafe fn piece(s: &Session<'_>, codec: &Codec<'_, '_>, h: G, area: bool) -> Result<Piece> {
    let a = s.a;
    let mut size = 0.0;
    let ok = unsafe { if area { (a.GEOSArea_r)(s.ctx, h, &mut size) } else { (a.GEOSLength_r)(s.ctx, h, &mut size) } };
    if ok == 0 {
        return Err("GEOS could not measure a piece".into());
    }
    let p = unsafe { (a.GEOSPointOnSurface_r)(s.ctx, h) };
    if p.is_null() {
        return Err("GEOS could not find a point on a piece".into());
    }
    let xy = unsafe { s.coords(p) };
    unsafe { s.destroy(p) };
    let xy = xy?;
    let c = xy.first().ok_or("a piece has no point on surface")?;
    Ok(Piece { geom: unsafe { codec.write(h) }?, size, x: c[0], y: c[1] })
}

/// The faces of a polygon part cut by the blade: those polygonized from the union of its boundary and the blade
/// whose point on surface lies within the part; no faces at all gives the part whole.
unsafe fn cut_polygon(s: &Session<'_>, codec: &Codec<'_, '_>, part: G, blade: G, out: &mut Vec<Piece>) -> Result<()> {
    let a = s.a;
    let bnd = unsafe { (a.GEOSBoundary_r)(s.ctx, part) };
    if bnd.is_null() {
        return Err("GEOS could not take the boundary of a polygon".into());
    }
    let u = unsafe { (a.GEOSUnion_r)(s.ctx, bnd, blade) };
    unsafe { s.destroy(bnd) };
    if u.is_null() {
        return Err("GEOS could not unite a boundary and a blade".into());
    }
    let arr = [u];
    let faces = unsafe { (a.GEOSPolygonize_r)(s.ctx, arr.as_ptr(), 1) };
    unsafe { s.destroy(u) };
    if faces.is_null() {
        return Err("GEOS could not polygonize a polygon cut".into());
    }
    let r = (|| -> Result<()> {
        let n = unsafe { (a.GEOSGetNumGeometries_r)(s.ctx, faces) }.max(0);
        if n == 0 {
            out.push(unsafe { piece(s, codec, part, true) }?);
            return Ok(());
        }
        for k in 0..n {
            let f = unsafe { (a.GEOSGetGeometryN_r)(s.ctx, faces, k) };
            let p = unsafe { (a.GEOSPointOnSurface_r)(s.ctx, f) };
            if p.is_null() {
                return Err("GEOS could not find a point on a face".into());
            }
            let inside = unsafe { (a.GEOSContains_r)(s.ctx, part, p) };
            unsafe { s.destroy(p) };
            match inside {
                1 => out.push(unsafe { piece(s, codec, f, true) }?),
                0 => {}
                _ => return Err("GEOS could not test a face".into()),
            }
        }
        Ok(())
    })();
    unsafe { s.destroy(faces) };
    r
}

/// The pieces of a feature cut by a blade (GEOS handles; the feature's own number of parts is returned too).
unsafe fn cut(s: &Session<'_>, codec: &Codec<'_, '_>, h: G, blade: G) -> Result<(Vec<Piece>, usize)> {
    let a = s.a;
    let parts = unsafe { (a.GEOSGetNumGeometries_r)(s.ctx, h) }.max(0) as usize;
    let kind = unsafe { (a.GEOSGeomTypeId_r)(s.ctx, h) };
    let mut out = Vec::new();
    if kind == 3 || kind == 6 {
        if kind == 3 {
            unsafe { cut_polygon(s, codec, h, blade, &mut out) }?;
        } else {
            for k in 0..parts {
                let part = unsafe { (a.GEOSGetGeometryN_r)(s.ctx, h, k as i32) };
                unsafe { cut_polygon(s, codec, part, blade, &mut out) }?;
            }
        }
    } else {
        let d = unsafe { (a.GEOSDifference_r)(s.ctx, h, blade) };
        if d.is_null() {
            return Err("GEOS could not subtract a blade from a line".into());
        }
        let r = (|| -> Result<()> {
            if unsafe { (a.GEOSisEmpty_r)(s.ctx, d) } == 1 {
                return Ok(());
            }
            let n = unsafe { (a.GEOSGetNumGeometries_r)(s.ctx, d) }.max(0);
            if unsafe { (a.GEOSGeomTypeId_r)(s.ctx, d) } == 1 {
                out.push(unsafe { piece(s, codec, d, false) }?);
            } else {
                for k in 0..n {
                    let p = unsafe { (a.GEOSGetGeometryN_r)(s.ctx, d, k) };
                    out.push(unsafe { piece(s, codec, p, false) }?);
                }
            }
            Ok(())
        })();
        unsafe { s.destroy(d) };
        r?;
    }
    Ok((out, parts))
}

/// Each polygon or line of `a` cut by the lines of `lines` (a polygon gives its boundary as blade), as
/// `tf.vector.split`: for each feature, the pieces in the order of the library (larger first; on a tie the larger y of
/// the point on surface, then the smaller x, then GEOS's order), or `None` for a feature that stays as it is (empty,
/// no blade meets it, or its pieces are no more than its parts). The caller checks the geometry kinds.
pub fn split(a: &[Geometry], lines: &[Geometry]) -> Result<Vec<Option<Vec<Geometry>>>> {
    let g: &Geos = geos().map_err(String::from)?;
    // the blades: a line as it is, a polygon its boundary
    let poly: Vec<usize> =
        (0..lines.len()).filter(|&j| matches!(lines[j], Geometry::Polygon(_) | Geometry::MultiPolygon(_))).collect();
    let mut blades: Vec<Geometry> = lines.to_vec();
    if !poly.is_empty() {
        let rings = unary(&poly.iter().map(|&j| lines[j].clone()).collect::<Vec<_>>(), Unary::Boundary)?;
        for (j, r) in poly.into_iter().zip(rings) {
            blades[j] = r;
        }
    }
    let hits = GeosIndex::new(&blades)?.pairs(a, Predicate::Intersects, None)?;
    let mut by: Vec<Vec<usize>> = vec![Vec::new(); a.len()];
    for (i, j) in hits {
        by[i].push(j);
    }
    let s = g.session()?;
    let codec = Codec::new(&s)?;
    let api = &g.api;
    let mut out: Vec<Option<Vec<Geometry>>> = Vec::with_capacity(a.len());
    for (i, geom) in a.iter().enumerate() {
        if by[i].is_empty() || Codec::nothing(geom) {
            out.push(None);
            continue;
        }
        let one = (|| -> Result<Option<Vec<Geometry>>> {
            unsafe {
                // union_all of the blades: GEOS's unary union of their collection
                let mut hs: Vec<G> = Vec::with_capacity(by[i].len());
                for &j in &by[i] {
                    match codec.read(&blades[j]) {
                        Ok(h) => hs.push(h),
                        Err(e) => {
                            hs.into_iter().for_each(|h| s.destroy(h));
                            return Err(e);
                        }
                    }
                }
                let collection = (api.GEOSGeom_createCollection_r)(s.ctx, 7, hs.as_mut_ptr(), hs.len() as c_uint);
                if collection.is_null() {
                    return Err("GEOS could not make the collection".into());
                }
                let blade = (api.GEOSUnaryUnion_r)(s.ctx, collection);
                s.destroy(collection);
                if blade.is_null() {
                    return Err("GEOS could not unite the blades".into());
                }
                let h = match codec.read(geom) {
                    Ok(h) => h,
                    Err(e) => {
                        s.destroy(blade);
                        return Err(e);
                    }
                };
                let cut = cut(&s, &codec, h, blade);
                s.destroy(h);
                s.destroy(blade);
                let (mut pieces, parts) = cut?;
                if pieces.len() <= parts {
                    return Ok(None);
                }
                // larger first, then the larger y, the smaller x, then GEOS's order (the sort is stable)
                let cmp = |p: &Piece, q: &Piece| {
                    use std::cmp::Ordering::Equal;
                    q.size
                        .partial_cmp(&p.size)
                        .unwrap_or(Equal)
                        .then(q.y.partial_cmp(&p.y).unwrap_or(Equal))
                        .then(p.x.partial_cmp(&q.x).unwrap_or(Equal))
                };
                pieces.sort_by(cmp);
                Ok(Some(pieces.into_iter().map(|p| p.geom).collect()))
            }
        })();
        out.push(one.map_err(|e| format!("feature {i}: {e}"))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Geometry {
        Geometry::Polygon(vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]])
    }

    #[test]
    fn a_square_cut_by_a_line_and_a_line_cut_by_a_line() {
        if let Err(e) = geos() {
            eprintln!("skipped: {e}");
            return;
        }
        let a = vec![
            rect(0.0, 0.0, 10.0, 10.0),
            rect(20.0, 0.0, 30.0, 10.0),
            Geometry::LineString(vec![[0.0, 20.0], [10.0, 20.0]]),
        ];
        let blades = vec![
            Geometry::LineString(vec![[3.0, -1.0], [3.0, 11.0]]),
            Geometry::LineString(vec![[5.0, 15.0], [5.0, 25.0]]),
        ];
        let r = split(&a, &blades).unwrap();
        let first = r[0].as_ref().unwrap();
        assert_eq!(first.len(), 2);
        // the larger piece (x from 3 to 10) first
        assert!(matches!(&first[0], Geometry::Polygon(p) if p[0].iter().any(|v| v[0] == 10.0)));
        assert!(r[1].is_none());
        assert_eq!(r[2].as_ref().unwrap().len(), 2);
    }
}
