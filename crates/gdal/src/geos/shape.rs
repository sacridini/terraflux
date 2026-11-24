//! Shape descriptors of polygons (the library's `tf.vector.shape`: shapely's `maximum_inscribed_circle`,
//! `minimum_bounding_radius`, `minimum_bounding_circle`, `oriented_envelope`, `area`, `length` and `buffer`, all
//! through GEOS's own functions, so the numbers and the geometries are GEOS's bit for bit).

use terraflux_raster::vector::Geometry;

use super::shapes::Codec;
use super::{G, Geos, Session, geos};
use crate::Result;

/// What the result's geometry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draw {
    /// The feature's own (`Shape::geometry` is `None`).
    Feature,
    /// The maximum inscribed circle as a polygon: the buffer of its centre by its radius (8 segments a quadrant).
    Inscribed,
    /// The minimum bounding circle.
    Bounding,
    /// The minimum rotated rectangle.
    Rectangle,
}

/// The descriptors of one polygon or multipolygon.
#[derive(Clone, Debug)]
pub struct Shape {
    pub inscribed_radius: f64,
    pub bounding_radius: f64,
    /// `1 - area / (pi * bounding_radius^2)`; NaN when the radius is not above 0.
    pub circle: f64,
    pub rect_length: f64,
    pub rect_width: f64,
    /// `1 - width / length`; NaN when the length is not above 0.
    pub elongation: f64,
    /// Degrees clockwise from north of the rectangle's long side, in [0, 180); NaN for a degenerate rectangle.
    pub orientation: f64,
    /// The geometry drawn, for anything but [`Draw::Feature`].
    pub geometry: Option<Geometry>,
}

/// numpy's `np.degrees(np.arctan2(dx, dy)) % 180.0` (Python's float modulo), with 180 turned into 0.
fn orientation(dx: f64, dy: f64) -> f64 {
    let d = dx.atan2(dy) * (180.0 / std::f64::consts::PI);
    let mut m = d % 180.0;
    if m != 0.0 {
        if m < 0.0 {
            m += 180.0;
        }
    } else {
        m = 0.0;
    }
    if m >= 180.0 { 0.0 } else { m }
}

/// The length, width and orientation of a rectangle from the first coordinates of its exterior as GEOS returns them
/// (A = c1 - c0, B = c2 - c1; B is 0 when there are fewer than 3; fewer than 2 give 0, 0 and NaN).
fn sides(xy: &[[f64; 2]]) -> (f64, f64, f64) {
    if xy.len() < 2 {
        return (0.0, 0.0, f64::NAN);
    }
    let a = [xy[1][0] - xy[0][0], xy[1][1] - xy[0][1]];
    let b = if xy.len() > 2 { [xy[2][0] - xy[1][0], xy[2][1] - xy[1][1]] } else { [0.0, 0.0] };
    let (la, lb) = (a[0].hypot(a[1]), b[0].hypot(b[1]));
    let (long, d) = if la >= lb { (la, a) } else { (lb, b) };
    (long, la.min(lb), orientation(d[0], d[1]))
}

/// The coordinates of a rectangle GEOS returned: its exterior ring (a polygon), or the line or point it degenerated to.
unsafe fn rect_coords(s: &Session<'_>, r: G) -> Result<Vec<[f64; 2]>> {
    let a = s.a;
    match unsafe { (a.GEOSGeomTypeId_r)(s.ctx, r) } {
        3 => {
            if unsafe { (a.GEOSisEmpty_r)(s.ctx, r) } == 1 {
                return Ok(Vec::new());
            }
            unsafe { s.coords((a.GEOSGetExteriorRing_r)(s.ctx, r)) }
        }
        _ => unsafe { s.coords(r) },
    }
}

/// The descriptors of one polygon (a GEOS handle, borrowed).
unsafe fn one(
    s: &Session<'_>,
    codec: &Codec<'_, '_>,
    g: &Geometry,
    h: G,
    tol: Option<f64>,
    draw: Draw,
) -> Result<Shape> {
    let a = s.a;
    let measure = |f: unsafe extern "C" fn(super::Ctx, G, *mut std::ffi::c_double) -> std::ffi::c_int, x: G| {
        let mut v = 0.0;
        if unsafe { f(s.ctx, x, &mut v) } == 0 { Err("GEOS could not measure a geometry".to_string()) } else { Ok(v) }
    };
    // the maximum inscribed circle: shapely passes 0 for no tolerance and the C code then takes
    // max(width, height) / 1000 of the feature's envelope
    let tolerance = match tol {
        Some(t) if t > 0.0 => t,
        _ => {
            let b = g.bounds().ok_or("a feature has no bounds")?;
            let (w, hh) = (b[2] - b[0], b[3] - b[1]);
            (if w < hh { hh } else { w }) / 1000.0
        }
    };
    let mic = unsafe { (a.GEOSMaximumInscribedCircle_r)(s.ctx, h, tolerance) };
    if mic.is_null() {
        return Err("GEOS could not find the maximum inscribed circle".into());
    }
    let r = (|| -> Result<Shape> {
        let radius = measure(a.GEOSLength_r, mic)?;
        let mut brad = 0.0;
        let mut centre: G = std::ptr::null_mut();
        let circle = unsafe { (a.GEOSMinimumBoundingCircle_r)(s.ctx, h, &mut brad, &mut centre) };
        if circle.is_null() {
            return Err("GEOS could not find the minimum bounding circle".into());
        }
        if !centre.is_null() {
            unsafe { s.destroy(centre) };
        }
        let area = measure(a.GEOSArea_r, h);
        let rect = unsafe { (a.GEOSMinimumRotatedRectangle_r)(s.ctx, h) };
        if rect.is_null() {
            unsafe { s.destroy(circle) };
            return Err("GEOS could not find the minimum rotated rectangle".into());
        }
        let done = (|| -> Result<Shape> {
            let area = area?;
            let xy = unsafe { rect_coords(s, rect) }?;
            let (length, width, orient) = sides(&xy);
            let pi_r2 = std::f64::consts::PI * (brad * brad);
            let geometry = match draw {
                Draw::Feature => None,
                Draw::Bounding => Some(unsafe { codec.write(circle) }?),
                Draw::Rectangle => Some(unsafe { codec.write(rect) }?),
                Draw::Inscribed => {
                    let c = unsafe { s.coords(mic) }?;
                    let c = c.first().ok_or("the inscribed circle has no centre")?;
                    let p = unsafe { (a.GEOSGeom_createPointFromXY_r)(s.ctx, c[0], c[1]) };
                    if p.is_null() {
                        return Err("GEOS could not make the circle's centre".into());
                    }
                    let params = unsafe { (a.GEOSBufferParams_create_r)(s.ctx) };
                    if params.is_null() {
                        unsafe { s.destroy(p) };
                        return Err("GEOS could not make the buffer parameters".into());
                    }
                    unsafe { (a.GEOSBufferParams_setQuadrantSegments_r)(s.ctx, params, 8) };
                    let b = unsafe { (a.GEOSBufferWithParams_r)(s.ctx, p, params, radius) };
                    unsafe { (a.GEOSBufferParams_destroy_r)(s.ctx, params) };
                    unsafe { s.destroy(p) };
                    if b.is_null() {
                        return Err("GEOS could not buffer the circle's centre".into());
                    }
                    let w = unsafe { codec.write(b) };
                    unsafe { s.destroy(b) };
                    Some(w?)
                }
            };
            Ok(Shape {
                inscribed_radius: radius,
                bounding_radius: brad,
                circle: if brad > 0.0 { 1.0 - area / pi_r2 } else { f64::NAN },
                rect_length: length,
                rect_width: width,
                elongation: if length > 0.0 { 1.0 - width / length } else { f64::NAN },
                orientation: orient,
                geometry,
            })
        })();
        unsafe { s.destroy(rect) };
        unsafe { s.destroy(circle) };
        done
    })();
    unsafe { s.destroy(mic) };
    r
}

/// The shape descriptors of each polygon or multipolygon (as `tf.vector.shape`), `None` for an empty one. `tolerance`
/// stops the inscribed circle's search (`None` or 0: shapely's default, `max(width, height) / 1000` of the feature's
/// envelope). The caller checks the geometry kinds.
pub fn shape(geoms: &[Geometry], tolerance: Option<f64>, draw: Draw) -> Result<Vec<Option<Shape>>> {
    if tolerance.is_some_and(|t| t < 0.0 || t.is_nan()) {
        return Err("the tolerance is a number of 0 or more".into());
    }
    let g: &Geos = geos().map_err(String::from)?;
    let s = g.session()?;
    let codec = Codec::new(&s)?;
    let mut out = Vec::with_capacity(geoms.len());
    for (i, geom) in geoms.iter().enumerate() {
        if Codec::nothing(geom) {
            out.push(None);
            continue;
        }
        let r = unsafe {
            let h = codec.read(geom).map_err(|e| format!("feature {i}: {e}"))?;
            let r = one(&s, &codec, geom, h, tolerance, draw);
            s.destroy(h);
            r.map_err(|e| format!("feature {i}: {e}"))?
        };
        out.push(Some(r));
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
    fn orientation_of_the_sides() {
        assert_eq!(orientation(1.0, 0.0), 90.0);
        assert_eq!(orientation(0.0, 1.0), 0.0);
        assert_eq!(orientation(-1.0, 0.0), 90.0);
        assert_eq!(orientation(0.0, -1.0), 0.0);
        assert!((orientation(1.0, 1.0) - 45.0).abs() < 1e-12);
        assert!((orientation(-1.0, 1.0) - 135.0).abs() < 1e-12);
    }

    #[test]
    fn a_long_rectangle_and_a_square() {
        if let Err(e) = geos() {
            eprintln!("skipped: {e}");
            return;
        }
        let r = shape(&[rect(0.0, 0.0, 100.0, 10.0), rect(0.0, 0.0, 10.0, 10.0), Geometry::Empty], None, Draw::Feature)
            .unwrap();
        let long = r[0].as_ref().unwrap();
        assert!((long.rect_length - 100.0).abs() < 1e-9 && (long.rect_width - 10.0).abs() < 1e-9);
        assert!((long.elongation - 0.9).abs() < 1e-12);
        assert!((long.orientation - 90.0).abs() < 1e-9, "{}", long.orientation);
        assert!((long.inscribed_radius - 5.0).abs() < 0.2, "{}", long.inscribed_radius);
        assert!(long.circle > 0.5 && long.geometry.is_none());
        let sq = r[1].as_ref().unwrap();
        assert!(sq.elongation.abs() < 1e-12);
        assert!(r[2].is_none());
        let c = shape(&[rect(0.0, 0.0, 10.0, 10.0)], Some(0.01), Draw::Inscribed).unwrap();
        assert!(matches!(c[0].as_ref().unwrap().geometry, Some(Geometry::Polygon(_))));
    }
}
