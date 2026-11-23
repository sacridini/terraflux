//! GEOS's Voronoi diagram and Delaunay triangulation of points (shapely's `voronoi_polygons` and
//! `delaunay_triangles`): the points go in as one multipoint and the parts of the result come back one by one
//! through the WKB writer, so the coordinates are GEOS's own, bit for bit.

use std::ffi::{c_int, c_uint};

use terraflux_raster::vector::Geometry;

use super::shapes::Codec;
use super::{G, Session, geos};
use crate::Result;

/// GEOS's flag of `GEOSVoronoiDiagram_r` to keep the cells in the order of the sites (3.12 and later).
const VORONOI_PRESERVE_ORDER: c_int = 2;

/// A multipoint of the points, in order (the collection takes them).
unsafe fn multipoint(s: &Session<'_>, xy: &[[f64; 2]]) -> Result<G> {
    let a = s.a;
    let mut pts: Vec<G> = xy.iter().map(|p| unsafe { (a.GEOSGeom_createPointFromXY_r)(s.ctx, p[0], p[1]) }).collect();
    if pts.iter().any(|p| p.is_null()) {
        pts.iter().filter(|p| !p.is_null()).for_each(|&p| unsafe { s.destroy(p) });
        return Err("GEOS could not make the points".into());
    }
    // GEOS_MULTIPOINT
    let m = unsafe { (a.GEOSGeom_createCollection_r)(s.ctx, 4, pts.as_mut_ptr(), pts.len() as c_uint) };
    if m.is_null() { Err("GEOS could not make the multipoint".into()) } else { Ok(m) }
}

/// The parts of a GEOS collection as geometries, in GEOS's order (the empty ones left out).
unsafe fn parts_of(s: &Session<'_>, codec: &Codec<'_, '_>, c: G) -> Result<Vec<Geometry>> {
    let a = s.a;
    let n = unsafe { (a.GEOSGetNumGeometries_r)(s.ctx, c) }.max(0);
    let mut out = Vec::with_capacity(n as usize);
    for k in 0..n {
        let part = unsafe { (a.GEOSGetGeometryN_r)(s.ctx, c, k) };
        if part.is_null() || unsafe { (a.GEOSisEmpty_r)(s.ctx, part) } != 0 {
            continue;
        }
        out.push(unsafe { codec.write(part) }?);
    }
    Ok(out)
}

/// The Voronoi cell of each site, in the order of the sites: GEOS's diagram of their multipoint (its default
/// extent: the envelope of the sites enlarged by its own margin) with the `tolerance`. The sites must be at least
/// two distinct points (GEOS's diagram of fewer has no cells). With GEOS 3.12 or later the cells come in order from
/// GEOS; otherwise each cell is matched to the first site it meets (the cell it contains).
pub fn voronoi(sites: &[[f64; 2]], tolerance: f64) -> Result<Vec<Geometry>> {
    if sites.len() < 2 {
        return Err("the Voronoi diagram needs two sites or more".into());
    }
    if !(tolerance >= 0.0 && tolerance.is_finite()) {
        return Err("the Voronoi tolerance is a number of 0 or more".into());
    }
    let g = geos().map_err(String::from)?;
    let s = g.session()?;
    let codec = Codec::new(&s)?;
    let a = &g.api;
    let ordered = g.version_number() >= (3, 12);
    unsafe {
        let multi = multipoint(&s, sites)?;
        let flags = if ordered { VORONOI_PRESERVE_ORDER } else { 0 };
        let diagram = (a.GEOSVoronoiDiagram_r)(s.ctx, multi, std::ptr::null_mut(), tolerance, flags);
        s.destroy(multi);
        if diagram.is_null() {
            return Err("GEOS could not make the Voronoi diagram".into());
        }
        let result = (|| -> Result<Vec<Geometry>> {
            let n = (a.GEOSGetNumGeometries_r)(s.ctx, diagram).max(0) as usize;
            if ordered {
                if n != sites.len() {
                    return Err("GEOS's Voronoi cells do not match the sites one to one".into());
                }
                let mut cells = Vec::with_capacity(n);
                for k in 0..n {
                    cells.push(codec.write((a.GEOSGetGeometryN_r)(s.ctx, diagram, k as c_int))?);
                }
                return Ok(cells);
            }
            let mut cells = vec![Geometry::Empty; sites.len()];
            let mut seen = vec![false; sites.len()];
            for k in 0..n {
                let cell = (a.GEOSGetGeometryN_r)(s.ctx, diagram, k as c_int);
                for (i, p) in sites.iter().enumerate() {
                    if seen[i] {
                        continue;
                    }
                    let pt = (a.GEOSGeom_createPointFromXY_r)(s.ctx, p[0], p[1]);
                    let hit = (a.GEOSIntersects_r)(s.ctx, cell, pt);
                    s.destroy(pt);
                    if hit == 1 {
                        seen[i] = true;
                        cells[i] = codec.write(cell)?;
                    }
                }
            }
            Ok(cells)
        })();
        s.destroy(diagram);
        result
    }
}

/// Each geometry intersected with `with` (shapely's `intersection`, GEOS's own): the results in the same order;
/// an empty geometry stays empty, and so does one that does not meet `with`. A collection result keeps its parts
/// of the highest dimension (see [`Geometry::from_wkb`]).
pub fn intersect(geoms: &[Geometry], with: &Geometry) -> Result<Vec<Geometry>> {
    let g = geos().map_err(String::from)?;
    let s = g.session()?;
    let codec = Codec::new(&s)?;
    let mut out = Vec::with_capacity(geoms.len());
    unsafe {
        let w = codec.read(with)?;
        let result = (|| -> Result<()> {
            for (i, geom) in geoms.iter().enumerate() {
                if matches!(geom, Geometry::Empty) || geom.bounds().is_none() {
                    out.push(Geometry::Empty);
                    continue;
                }
                let h = codec.read(geom).map_err(|e| format!("feature {i}: {e}"))?;
                let r = (g.api.GEOSIntersection_r)(s.ctx, h, w);
                s.destroy(h);
                if r.is_null() {
                    return Err(format!("GEOS could not intersect feature {i}"));
                }
                let geometry = codec.write(r);
                s.destroy(r);
                out.push(geometry?);
            }
            Ok(())
        })();
        s.destroy(w);
        result?;
    }
    Ok(out)
}

/// The Delaunay triangulation of the points (all of them, duplicates included, as one multipoint): the triangles
/// as polygons, or with `edges` the edges as lines, in the order of GEOS's result, with the `tolerance` to snap
/// by. Fewer than three non-collinear points give none.
pub fn delaunay(points: &[[f64; 2]], tolerance: f64, edges: bool) -> Result<Vec<Geometry>> {
    if !(tolerance >= 0.0 && tolerance.is_finite()) {
        return Err("the Delaunay tolerance is a number of 0 or more".into());
    }
    if points.is_empty() {
        return Ok(Vec::new());
    }
    let g = geos().map_err(String::from)?;
    let s = g.session()?;
    let codec = Codec::new(&s)?;
    unsafe {
        let multi = multipoint(&s, points)?;
        let tri = (g.api.GEOSDelaunayTriangulation_r)(s.ctx, multi, tolerance, edges as c_int);
        s.destroy(multi);
        if tri.is_null() {
            return Err("GEOS could not make the Delaunay triangulation".into());
        }
        let result = parts_of(&s, &codec, tri);
        s.destroy(tri);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn have_geos() -> bool {
        if let Err(e) = geos() {
            eprintln!("skipped: {e}");
            return false;
        }
        true
    }

    fn area(g: &Geometry) -> f64 {
        let ring =
            |r: &Vec<[f64; 2]>| r.windows(2).map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1]).sum::<f64>().abs() / 2.0;
        g.polygons().iter().map(|p| ring(&p[0]) - p[1..].iter().map(ring).sum::<f64>()).sum()
    }

    #[test]
    fn the_cells_come_in_the_order_of_the_sites() {
        if !have_geos() {
            return;
        }
        let sites = [[0.0, 0.0], [10.0, 0.0], [5.0, 8.0], [-3.0, 6.0]];
        let cells = voronoi(&sites, 0.0).unwrap();
        assert_eq!(cells.len(), 4);
        // two sites: the plane is cut in half between them
        for (cell, site) in cells.iter().zip(&sites) {
            let b = cell.bounds().unwrap();
            assert!(b[0] <= site[0] && site[0] <= b[2] && b[1] <= site[1] && site[1] <= b[3], "{cell:?}");
        }
        let two = voronoi(&[[0.0, 0.0], [10.0, 0.0]], 0.0).unwrap();
        assert_eq!(two[0].bounds().unwrap()[2], 5.0);
        assert_eq!(two[1].bounds().unwrap()[0], 5.0);
        assert!(voronoi(&[[1.0, 1.0]], 0.0).is_err());
        assert!(voronoi(&sites, -1.0).is_err());
    }

    #[test]
    fn delaunay_triangles_and_edges() {
        if !have_geos() {
            return;
        }
        // a square with its centre: 4 triangles, 8 edges
        let pts = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [5.0, 5.0]];
        let tri = delaunay(&pts, 0.0, false).unwrap();
        assert_eq!(tri.len(), 4);
        assert_eq!(tri.iter().map(area).sum::<f64>(), 100.0);
        let edges = delaunay(&pts, 0.0, true).unwrap();
        assert!(!edges.is_empty());
        assert!(edges.iter().all(|e| matches!(e, Geometry::LineString(_) | Geometry::MultiLineString(_))));
        // collinear points and too few give none
        assert!(delaunay(&[[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]], 0.0, false).unwrap().is_empty());
        assert!(delaunay(&[[0.0, 0.0]], 0.0, false).unwrap().is_empty());
        assert!(delaunay(&[], 0.0, false).unwrap().is_empty());
    }
}
