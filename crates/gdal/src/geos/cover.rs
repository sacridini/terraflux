//! GEOS's polygon coverage functions (3.12 or later): the simplification of a coverage whose polygons keep sharing
//! their edges (`GEOSCoverageSimplifyVW_r`) and its validation (`GEOSCoverageIsValid_r`, shapely's
//! `coverage_simplify` and `coverage_invalid_edges`). Both take the polygons as one geometry collection and give
//! one with a geometry per polygon, in the same order.

use std::ffi::{c_int, c_uint};

use terraflux_raster::vector::Geometry;

use super::shapes::Codec;
use super::{G, Geos, geos};
use crate::Result;

/// The polygons (and multipolygons) as a collection handle, the empty ones left out: it and the index in `geoms` of
/// each of its members.
unsafe fn collect(
    g: &Geos,
    s: &super::Session<'_>,
    codec: &Codec<'_, '_>,
    geoms: &[Geometry],
) -> Result<(G, Vec<usize>)> {
    let mut handles: Vec<G> = Vec::with_capacity(geoms.len());
    let mut index = Vec::with_capacity(geoms.len());
    for (i, geom) in geoms.iter().enumerate() {
        if Codec::nothing(geom) {
            continue;
        }
        if !matches!(geom, Geometry::Polygon(_) | Geometry::MultiPolygon(_)) {
            handles.into_iter().for_each(|h| unsafe { s.destroy(h) });
            return Err(format!("feature {i} is not a polygon (a coverage is polygons)"));
        }
        match unsafe { codec.read(geom) } {
            Ok(h) => {
                handles.push(h);
                index.push(i);
            }
            Err(e) => {
                handles.into_iter().for_each(|h| unsafe { s.destroy(h) });
                return Err(format!("feature {i}: {e}"));
            }
        }
    }
    // GEOS_GEOMETRYCOLLECTION: the collection takes the geometries
    let n = handles.len() as c_uint;
    let c = unsafe { (g.api.GEOSGeom_createCollection_r)(s.ctx, 7, handles.as_mut_ptr(), n) };
    if c.is_null() { Err("GEOS could not make the collection".into()) } else { Ok((c, index)) }
}

/// The geometries of a result collection put back at the positions of `index` (the others empty).
unsafe fn spread(
    g: &Geos,
    s: &super::Session<'_>,
    codec: &Codec<'_, '_>,
    result: G,
    index: &[usize],
    len: usize,
) -> Result<Vec<Geometry>> {
    let n = unsafe { (g.api.GEOSGetNumGeometries_r)(s.ctx, result) }.max(0) as usize;
    if n != index.len() {
        return Err(format!("GEOS gave {n} geometries for {} polygons", index.len()));
    }
    let mut out = vec![Geometry::Empty; len];
    for (k, &i) in index.iter().enumerate() {
        let h = unsafe { (g.api.GEOSGetGeometryN_r)(s.ctx, result, k as c_int) };
        out[i] = unsafe { codec.write(h) }?;
    }
    Ok(out)
}

/// The polygons of a coverage simplified together with Visvalingam-Whyatt (shapely's `coverage_simplify`): the
/// edges shared by neighbours stay shared. With `preserve_boundary` the outer boundary of the coverage is not
/// simplified. Empty polygons stay empty; a polygon that collapses is empty. GEOS 3.12 or later.
pub fn coverage_simplify(geoms: &[Geometry], tolerance: f64, preserve_boundary: bool) -> Result<Vec<Geometry>> {
    let g = geos().map_err(String::from)?;
    let Some(f) = g.api.GEOSCoverageSimplifyVW_r else {
        return Err(format!("coverage simplification needs GEOS 3.12 or later (this is {})", g.version));
    };
    if !(tolerance >= 0.0 && tolerance.is_finite()) {
        return Err("the simplification's tolerance is a number of 0 or more".into());
    }
    let s = g.session()?;
    let codec = Codec::new(&s)?;
    unsafe {
        let (collection, index) = collect(g, &s, &codec, geoms)?;
        if index.is_empty() {
            s.destroy(collection);
            return Ok(vec![Geometry::Empty; geoms.len()]);
        }
        let r = f(s.ctx, collection, tolerance, preserve_boundary as c_int);
        s.destroy(collection);
        if r.is_null() {
            return Err("GEOS could not simplify the coverage (are the polygons a valid coverage?)".into());
        }
        let out = spread(g, &s, &codec, r, &index, geoms.len());
        s.destroy(r);
        out
    }
}

/// The invalid edges of each polygon of a coverage (shapely's `coverage_invalid_edges`): the lines where it
/// overlaps a neighbour or is not matched by it, as a geometry per polygon (empty when the polygon is valid). With
/// `gap_width` edges closer than that to a neighbour are flagged too. GEOS 3.12 or later.
pub fn coverage_invalid_edges(geoms: &[Geometry], gap_width: f64) -> Result<Vec<Geometry>> {
    let g = geos().map_err(String::from)?;
    let Some(f) = g.api.GEOSCoverageIsValid_r else {
        return Err(format!("coverage validation needs GEOS 3.12 or later (this is {})", g.version));
    };
    if !(gap_width >= 0.0 && gap_width.is_finite()) {
        return Err("the gap width is a number of 0 or more".into());
    }
    let s = g.session()?;
    let codec = Codec::new(&s)?;
    unsafe {
        let (collection, index) = collect(g, &s, &codec, geoms)?;
        if index.is_empty() {
            s.destroy(collection);
            return Ok(vec![Geometry::Empty; geoms.len()]);
        }
        let mut edges: G = std::ptr::null_mut();
        let status = f(s.ctx, collection, gap_width, &mut edges);
        s.destroy(collection);
        if status == 2 || edges.is_null() {
            return Err("GEOS could not validate the coverage".into());
        }
        let out = spread(g, &s, &codec, edges, &index, geoms.len());
        s.destroy(edges);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn have_cover() -> bool {
        match geos() {
            Ok(g) if g.api.GEOSCoverageSimplifyVW_r.is_some() => true,
            Ok(g) => {
                eprintln!("skipped: GEOS {} has no coverage functions", g.version);
                false
            }
            Err(e) => {
                eprintln!("skipped: {e}");
                false
            }
        }
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Geometry {
        Geometry::Polygon(vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]])
    }

    #[test]
    fn a_coverage_is_validated() {
        if !have_cover() {
            return;
        }
        // two squares sharing an edge: valid
        let ok = coverage_invalid_edges(&[rect(0.0, 0.0, 1.0, 1.0), Geometry::Empty, rect(1.0, 0.0, 2.0, 1.0)], 0.0)
            .unwrap();
        assert_eq!(ok, vec![Geometry::Empty; 3]);
        // an overlap flags both
        let bad = coverage_invalid_edges(&[rect(0.0, 0.0, 2.0, 2.0), rect(1.0, 0.0, 3.0, 2.0)], 0.0).unwrap();
        assert!(bad.iter().all(|g| !matches!(g, Geometry::Empty)));
        // a narrow gap only with a gap width
        let gap = [rect(0.0, 0.0, 1.0, 1.0), rect(1.1, 0.0, 2.0, 1.0)];
        assert!(coverage_invalid_edges(&gap, 0.0).unwrap().iter().all(|g| matches!(g, Geometry::Empty)));
        assert!(coverage_invalid_edges(&gap, 0.5).unwrap().iter().all(|g| !matches!(g, Geometry::Empty)));
        assert!(coverage_invalid_edges(&[Geometry::Point([0.0, 0.0])], 0.0).is_err());
    }

    #[test]
    fn a_coverage_is_simplified_together() {
        if !have_cover() {
            return;
        }
        // two squares whose shared edge has a small bump: both lose it, and still share the edge
        let a = Geometry::Polygon(vec![vec![[0.0, 0.0], [1.0, 0.0], [1.05, 0.5], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]]]);
        let b = Geometry::Polygon(vec![vec![[1.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0], [1.05, 0.5], [1.0, 0.0]]]);
        let r = coverage_simplify(&[a, Geometry::Empty, b], 0.5, false).unwrap();
        assert_eq!(r[1], Geometry::Empty);
        let (Geometry::Polygon(p), Geometry::Polygon(q)) = (&r[0], &r[2]) else { panic!("{r:?}") };
        assert!(p[0].len() < 6 && q[0].len() < 6);
        assert!(!p[0].contains(&[1.05, 0.5]) && !q[0].contains(&[1.05, 0.5]));
        assert!(coverage_simplify(&[rect(0.0, 0.0, 1.0, 1.0)], -1.0, false).is_err());
        assert_eq!(coverage_simplify(&[], 1.0, true).unwrap(), vec![]);
    }
}
