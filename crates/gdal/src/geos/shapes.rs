//! GEOS's geometry operations on terraflux's geometries (centroid, hull, simplify, envelope, make valid, boundary,
//! union): each geometry goes into GEOS through its WKB reader and comes back through its WKB writer, so the
//! coordinates are GEOS's own, bit for bit (shapely's `centroid`, `convex_hull`, `concave_hull`, `simplify`, ...).

use std::ffi::c_void;

use terraflux_raster::vector::Geometry;

use super::{G, Geos, Session, geos};
use crate::Result;

/// An operation on one geometry that gives one geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unary {
    /// The centre of mass: of the area, else of the length, else of the points.
    Centroid,
    /// A point guaranteed to lie on the geometry (inside a polygon).
    PointOnSurface,
    ConvexHull,
    /// GEOS's concave hull (3.11 or later): `ratio` 1 is the convex hull, 0 the tightest.
    ConcaveHull {
        ratio: f64,
        allow_holes: bool,
    },
    /// Douglas-Peucker with the tolerance, which may collapse or cross rings.
    Simplify(f64),
    /// Douglas-Peucker that keeps the topology (valid in, valid out).
    TopologyPreserveSimplify(f64),
    /// The bounding box (a polygon; a point or a flat line gives the point or line).
    Envelope,
    /// GEOS's linework repair (3.8 or later); a valid geometry stays as it is.
    MakeValid,
    Boundary,
}

/// A geometry of GEOS's for a terraflux one, read through the WKB reader.
unsafe fn geos_of(s: &Session<'_>, reader: G, g: &Geometry) -> Result<G> {
    let wkb = g.to_wkb();
    let h = unsafe { (s.a.GEOSWKBReader_read_r)(s.ctx, reader, wkb.as_ptr(), wkb.len()) };
    if h.is_null() { Err("GEOS could not read the geometry".into()) } else { Ok(h) }
}

/// A terraflux geometry for one of GEOS's, through the WKB writer (2-D). An empty one is `Geometry::Empty`. A
/// geometry collection (what a hull of a mixed input or a repaired polygon can be) keeps the parts of its highest
/// dimension (the polygons, else the lines, else the points) as the multi geometry of them.
unsafe fn geometry_of(s: &Session<'_>, writer: G, h: G) -> Result<Geometry> {
    if unsafe { (s.a.GEOSisEmpty_r)(s.ctx, h) } != 0 {
        return Ok(Geometry::Empty);
    }
    let mut n = 0usize;
    let p = unsafe { (s.a.GEOSWKBWriter_write_r)(s.ctx, writer, h, &mut n) };
    if p.is_null() {
        return Err("GEOS could not write the geometry".into());
    }
    let g = Geometry::from_wkb(unsafe { std::slice::from_raw_parts(p, n) });
    unsafe { (s.a.GEOSFree_r)(s.ctx, p as *mut c_void) };
    g
}

/// A WKB reader and writer of a session, destroyed when dropped.
pub(super) struct Codec<'s, 'a> {
    s: &'s Session<'a>,
    reader: G,
    writer: G,
}

impl<'s, 'a> Codec<'s, 'a> {
    pub(super) fn new(s: &'s Session<'a>) -> Result<Self> {
        let reader = unsafe { (s.a.GEOSWKBReader_create_r)(s.ctx) };
        if reader.is_null() {
            return Err("GEOS could not make the WKB reader".into());
        }
        let writer = unsafe { (s.a.GEOSWKBWriter_create_r)(s.ctx) };
        if writer.is_null() {
            unsafe { (s.a.GEOSWKBReader_destroy_r)(s.ctx, reader) };
            return Err("GEOS could not make the WKB writer".into());
        }
        unsafe { (s.a.GEOSWKBWriter_setOutputDimension_r)(s.ctx, writer, 2) };
        Ok(Codec { s, reader, writer })
    }

    /// Whether the geometry is nothing for GEOS (left out of every operation).
    pub(super) fn nothing(g: &Geometry) -> bool {
        matches!(g, Geometry::Empty) || g.bounds().is_none()
    }

    pub(super) unsafe fn read(&self, g: &Geometry) -> Result<G> {
        unsafe { geos_of(self.s, self.reader, g) }
    }

    pub(super) unsafe fn write(&self, h: G) -> Result<Geometry> {
        unsafe { geometry_of(self.s, self.writer, h) }
    }
}

impl Drop for Codec<'_, '_> {
    fn drop(&mut self) {
        unsafe {
            (self.s.a.GEOSWKBReader_destroy_r)(self.s.ctx, self.reader);
            (self.s.a.GEOSWKBWriter_destroy_r)(self.s.ctx, self.writer);
        }
    }
}

/// `op` on every geometry (shapely's vectorised functions): the results in the same order; an empty geometry stays
/// empty. A result that is a geometry collection keeps its highest-dimension parts (see [`Geometry::from_wkb`]).
pub fn unary(geoms: &[Geometry], op: Unary) -> Result<Vec<Geometry>> {
    let g: &Geos = geos().map_err(String::from)?;
    let a = &g.api;
    match op {
        Unary::ConcaveHull { ratio, .. } => {
            if a.GEOSConcaveHull_r.is_none() {
                return Err(format!("the concave hull needs GEOS 3.11 or later (this is {})", g.version));
            }
            if !(0.0..=1.0).contains(&ratio) {
                return Err("the concave hull's ratio is between 0 and 1".into());
            }
        }
        Unary::MakeValid if a.GEOSMakeValid_r.is_none() => {
            return Err(format!("make valid needs GEOS 3.8 or later (this is {})", g.version));
        }
        Unary::Simplify(t) | Unary::TopologyPreserveSimplify(t) if !(t >= 0.0 && t.is_finite()) => {
            return Err("the simplification's tolerance is a number of 0 or more".into());
        }
        _ => {}
    }
    let s = g.session()?;
    let codec = Codec::new(&s)?;
    let mut out = Vec::with_capacity(geoms.len());
    for (i, geom) in geoms.iter().enumerate() {
        if Codec::nothing(geom) {
            out.push(Geometry::Empty);
            continue;
        }
        unsafe {
            let h = codec.read(geom).map_err(|e| format!("feature {i}: {e}"))?;
            let r = match op {
                Unary::Centroid => (a.GEOSGetCentroid_r)(s.ctx, h),
                Unary::PointOnSurface => (a.GEOSPointOnSurface_r)(s.ctx, h),
                Unary::ConvexHull => (a.GEOSConvexHull_r)(s.ctx, h),
                Unary::ConcaveHull { ratio, allow_holes } => {
                    (a.GEOSConcaveHull_r.expect("checked above"))(s.ctx, h, ratio, allow_holes as u32)
                }
                Unary::Simplify(t) => (a.GEOSSimplify_r)(s.ctx, h, t),
                Unary::TopologyPreserveSimplify(t) => (a.GEOSTopologyPreserveSimplify_r)(s.ctx, h, t),
                Unary::Envelope => (a.GEOSEnvelope_r)(s.ctx, h),
                Unary::MakeValid => (a.GEOSMakeValid_r.expect("checked above"))(s.ctx, h),
                Unary::Boundary => (a.GEOSBoundary_r)(s.ctx, h),
            };
            s.destroy(h);
            if r.is_null() {
                return Err(format!("GEOS could not compute {op:?} of feature {i}"));
            }
            let geometry = codec.write(r);
            s.destroy(r);
            out.push(geometry?);
        }
    }
    Ok(out)
}

/// The union of the geometries (shapely's `union_all`: GEOS's unary union of their collection); the empty ones
/// are left out, and nothing left gives `Geometry::Empty`.
pub fn union(geoms: &[Geometry]) -> Result<Geometry> {
    let g = geos().map_err(String::from)?;
    let s = g.session()?;
    let codec = Codec::new(&s)?;
    let a = &g.api;
    let mut handles: Vec<G> = Vec::with_capacity(geoms.len());
    unsafe {
        for (i, geom) in geoms.iter().enumerate().filter(|(_, x)| !Codec::nothing(x)) {
            match codec.read(geom) {
                Ok(h) => handles.push(h),
                Err(e) => {
                    handles.into_iter().for_each(|h| s.destroy(h));
                    return Err(format!("feature {i}: {e}"));
                }
            }
        }
        if handles.is_empty() {
            return Ok(Geometry::Empty);
        }
        // GEOS_GEOMETRYCOLLECTION: the collection takes the geometries
        let n = handles.len() as std::ffi::c_uint;
        let collection = (a.GEOSGeom_createCollection_r)(s.ctx, 7, handles.as_mut_ptr(), n);
        if collection.is_null() {
            return Err("GEOS could not make the collection".into());
        }
        let u = (a.GEOSUnaryUnion_r)(s.ctx, collection);
        s.destroy(collection);
        if u.is_null() {
            return Err("GEOS could not make the union".into());
        }
        let r = codec.write(u);
        s.destroy(u);
        r
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

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Geometry {
        Geometry::Polygon(vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]])
    }

    fn one(g: &Geometry, op: Unary) -> Geometry {
        unary(std::slice::from_ref(g), op).unwrap().remove(0)
    }

    fn area(g: &Geometry) -> f64 {
        let ring =
            |r: &Vec<[f64; 2]>| r.windows(2).map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1]).sum::<f64>().abs() / 2.0;
        g.polygons().iter().map(|p| ring(&p[0]) - p[1..].iter().map(ring).sum::<f64>()).sum()
    }

    #[test]
    fn geometries_round_trip_through_geos() {
        if !have_geos() {
            return;
        }
        let hole = Geometry::Polygon(vec![
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 0.0]],
            vec![[2.0, 2.0], [2.0, 4.0], [4.0, 4.0], [4.0, 2.0], [2.0, 2.0]],
        ]);
        let cases = [
            Geometry::Point([1.5, -2.25]),
            Geometry::MultiPoint(vec![[1.0, 2.0], [3.0, 4.0]]),
            Geometry::LineString(vec![[0.0, 0.0], [1.0, 1.0], [2.0, 0.5]]),
            Geometry::MultiLineString(vec![vec![[0.0, 0.0], [1.0, 1.0]], vec![[5.0, 5.0], [6.0, 7.0]]]),
            hole,
            Geometry::MultiPolygon(vec![
                vec![vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]]],
                vec![vec![[5.0, 5.0], [6.0, 5.0], [6.0, 6.0], [5.0, 6.0], [5.0, 5.0]]],
            ]),
            Geometry::Empty,
        ];
        // a valid geometry stays as it is under make valid
        let back = unary(&cases, Unary::MakeValid).unwrap();
        assert_eq!(back, cases);
    }

    #[test]
    fn each_operation_on_a_hand_case() {
        if !have_geos() {
            return;
        }
        let sq = rect(0.0, 0.0, 10.0, 10.0);
        assert_eq!(one(&sq, Unary::Centroid), Geometry::Point([5.0, 5.0]));
        // an L: its centroid is outside, the point on surface is not
        let l = Geometry::Polygon(vec![vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 1.0],
            [1.0, 1.0],
            [1.0, 10.0],
            [0.0, 10.0],
            [0.0, 0.0],
        ]]);
        let Geometry::Point(c) = one(&l, Unary::Centroid) else { panic!() };
        assert!(c[0] > 1.0 && c[1] > 1.0);
        let Geometry::Point(p) = one(&l, Unary::PointOnSurface) else { panic!() };
        assert!(p[0] <= 1.0 || p[1] <= 1.0);
        // hull of the corners and an interior point
        let pts = Geometry::MultiPoint(vec![[0.0, 0.0], [10.0, 0.0], [5.0, 5.0], [10.0, 10.0], [0.0, 10.0]]);
        assert_eq!(area(&one(&pts, Unary::ConvexHull)), 100.0);
        assert_eq!(area(&one(&l, Unary::ConvexHull)), 100.0 - 40.5);
        let concave = one(&l, Unary::ConcaveHull { ratio: 0.0, allow_holes: false });
        assert!(area(&concave) < 59.5 + 1e-9 && area(&concave) >= 19.0);
        // simplify
        let line = Geometry::LineString(vec![[0.0, 0.0], [1.0, 0.1], [2.0, 0.0]]);
        let flat = Geometry::LineString(vec![[0.0, 0.0], [2.0, 0.0]]);
        assert_eq!(one(&line, Unary::Simplify(0.5)), flat);
        assert_eq!(one(&line, Unary::TopologyPreserveSimplify(0.5)), flat);
        assert_eq!(one(&line, Unary::Simplify(0.05)), line);
        assert!(unary(&[line], Unary::Simplify(-1.0)).is_err());
        // envelope
        let tri = Geometry::LineString(vec![[1.0, 2.0], [4.0, 3.0], [2.0, 6.0]]);
        let env = one(&tri, Unary::Envelope);
        assert_eq!(area(&env), 3.0 * 4.0);
        assert_eq!(one(&Geometry::Point([3.0, 4.0]), Unary::Envelope), Geometry::Point([3.0, 4.0]));
        // a bow-tie becomes two triangles
        let bow = Geometry::Polygon(vec![vec![[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0], [0.0, 0.0]]]);
        let fixed = one(&bow, Unary::MakeValid);
        assert!(matches!(&fixed, Geometry::MultiPolygon(m) if m.len() == 2));
        assert_eq!(area(&fixed), 2.0);
        // boundaries
        let holed = Geometry::Polygon(vec![
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 0.0]],
            vec![[2.0, 2.0], [2.0, 4.0], [4.0, 4.0], [4.0, 2.0], [2.0, 2.0]],
        ]);
        assert!(matches!(one(&holed, Unary::Boundary), Geometry::MultiLineString(m) if m.len() == 2));
        let Geometry::MultiPoint(ends) = one(&tri, Unary::Boundary) else { panic!() };
        assert_eq!(ends, vec![[1.0, 2.0], [2.0, 6.0]]);
        // a closed line has no end points, a point no boundary
        let ring = Geometry::LineString(vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]);
        assert_eq!(one(&ring, Unary::Boundary), Geometry::Empty);
        assert_eq!(one(&Geometry::Point([1.0, 1.0]), Unary::Boundary), Geometry::Empty);
        // empties stay empty
        assert_eq!(unary(&[Geometry::Empty], Unary::Centroid).unwrap(), vec![Geometry::Empty]);
        assert!(unary(&[], Unary::Envelope).unwrap().is_empty());
        // a ratio out of range
        assert!(unary(&[sq], Unary::ConcaveHull { ratio: 2.0, allow_holes: false }).is_err());
    }

    #[test]
    fn a_union_merges_and_skips_empties() {
        if !have_geos() {
            return;
        }
        let u = union(&[rect(0.0, 0.0, 2.0, 2.0), Geometry::Empty, rect(1.0, 0.0, 3.0, 2.0)]).unwrap();
        assert!(matches!(u, Geometry::Polygon(_)));
        assert_eq!(area(&u), 6.0);
        assert_eq!(union(&[Geometry::Empty]).unwrap(), Geometry::Empty);
        assert_eq!(union(&[]).unwrap(), Geometry::Empty);
        let apart = union(&[rect(0.0, 0.0, 1.0, 1.0), rect(5.0, 5.0, 6.0, 6.0)]).unwrap();
        assert!(matches!(apart, Geometry::MultiPolygon(m) if m.len() == 2));
        // a mixed result keeps the polygons
        let mixed = union(&[rect(0.0, 0.0, 1.0, 1.0), Geometry::Point([5.0, 5.0])]).unwrap();
        assert!(matches!(mixed, Geometry::MultiPolygon(m) if m.len() == 1));
    }
}
