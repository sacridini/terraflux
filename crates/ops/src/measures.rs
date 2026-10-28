//! The measures of features' geometries (`vector.geometry`, and `geometry.area`... in `vector.calc` and
//! `vector.filter`): areas, perimeters, lengths and centroids, in the features' units.

use terraflux_raster::vector::{Geometry, Vector, Xy};

/// The signed area of a ring (its last point the first or not): positive when it turns counterclockwise.
pub(crate) fn ring_area(r: &[Xy]) -> f64 {
    let n = r.len();
    (0..n).map(|i| r[i][0] * r[(i + 1) % n][1] - r[(i + 1) % n][0] * r[i][1]).sum::<f64>() / 2.0
}

/// The length of a line, or of a ring closed.
pub(crate) fn length(l: &[Xy], closed: bool) -> f64 {
    let d = |a: Xy, b: Xy| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    let open: f64 = l.windows(2).map(|w| d(w[0], w[1])).sum();
    match (closed, l.first(), l.last()) {
        (true, Some(a), Some(b)) => open + d(*b, *a),
        _ => open,
    }
}

/// The area of a polygon (its holes left out).
pub(crate) fn polygon_area(p: &[Vec<Xy>]) -> f64 {
    p.iter().enumerate().map(|(i, r)| if i == 0 { ring_area(r).abs() } else { -ring_area(r).abs() }).sum()
}

/// The centroid of a ring's area and its area (both by the ring's turn).
fn ring_centroid(r: &[Xy]) -> (Xy, f64) {
    let n = r.len();
    let (mut cx, mut cy) = (0.0, 0.0);
    for i in 0..n {
        let (a, b) = (r[i], r[(i + 1) % n]);
        let cross = a[0] * b[1] - b[0] * a[1];
        cx += (a[0] + b[0]) * cross;
        cy += (a[1] + b[1]) * cross;
    }
    let area = ring_area(r);
    ([cx / (6.0 * area), cy / (6.0 * area)], area)
}

/// The centroid of a geometry: of its area (polygons), its length (lines) or its points.
fn centroid(g: &Geometry) -> Xy {
    let weighted = |parts: Vec<(Xy, f64)>| {
        let w: f64 = parts.iter().map(|p| p.1).sum();
        if w > 0.0 {
            [parts.iter().map(|p| p.0[0] * p.1).sum::<f64>() / w, parts.iter().map(|p| p.0[1] * p.1).sum::<f64>() / w]
        } else {
            [f64::NAN, f64::NAN]
        }
    };
    let lines = |ls: &[&Vec<Xy>]| {
        weighted(
            ls.iter()
                .flat_map(|l| l.windows(2))
                .map(|s| ([(s[0][0] + s[1][0]) / 2.0, (s[0][1] + s[1][1]) / 2.0], length(s, false)))
                .collect(),
        )
    };
    match g {
        Geometry::Empty => [f64::NAN, f64::NAN],
        Geometry::Point(p) => *p,
        Geometry::MultiPoint(ps) => weighted(ps.iter().map(|p| (*p, 1.0)).collect()),
        Geometry::LineString(l) => lines(&[l]),
        Geometry::MultiLineString(ls) => lines(&ls.iter().collect::<Vec<_>>()),
        Geometry::Polygon(_) | Geometry::MultiPolygon(_) => weighted(
            g.polygons()
                .iter()
                .flat_map(|p| {
                    p.iter().enumerate().map(|(i, r)| {
                        let (c, a) = ring_centroid(r);
                        (c, if i == 0 { a.abs() } else { -a.abs() })
                    })
                })
                .collect(),
        ),
    }
}

/// The measures of a geometry, as `vector.geometry` gives them and map algebra reads them (`geometry.area`).
pub(crate) const MEASURES: [&str; 5] = ["area", "perimeter", "length", "x", "y"];

/// A measure of every feature's geometry, in the features' units: the area and perimeter of polygons (their holes
/// left out of the area, their rings in the perimeter), the length of lines, the centroid's x and y (of the area,
/// the length or the points). None for a name that is not one of [`MEASURES`].
pub(crate) fn measure(v: &Vector, name: &str) -> Option<Vec<f64>> {
    let polys = Geometry::polygons;
    let each = |f: &dyn Fn(&Geometry) -> f64| v.geometries.iter().map(f).collect();
    Some(match name {
        "area" => each(&|g| polys(g).iter().map(|p| polygon_area(p)).sum()),
        "perimeter" => each(&|g| polys(g).iter().flat_map(|p| p.iter()).map(|r| length(r, true)).sum()),
        "length" => each(&|g| match g {
            Geometry::LineString(l) => length(l, false),
            Geometry::MultiLineString(ls) => ls.iter().map(|l| length(l, false)).sum(),
            _ => 0.0,
        }),
        "x" => each(&|g| centroid(g)[0]),
        "y" => each(&|g| centroid(g)[1]),
        _ => return None,
    })
}
