//! The spatial functions of the vector calculator (`within(parks)`, `count(trees, "contains")`, `distance(roads)` in
//! `vector.calc` and `vector.filter`): a value per feature of a layer from the features of another, through GEOS's
//! spatial index, as the Python library's `tf.vector.relate`, `tf.vector.count` and `tf.vector.distance`. An empty
//! geometry never meets anything; the other layer is reprojected to the first one's coordinate system when both are
//! known and differ.

use terraflux_gdal::geos::{GeosIndex, Predicate};
use terraflux_raster::vector::{Geometry, Vector};

use crate::{Error, Result};

fn present(g: &Geometry) -> bool {
    !matches!(g, Geometry::Empty) && g.bounds().is_some()
}

/// The features of b in the coordinate system of a.
fn like(a: &Vector, b: &Vector) -> Result<Vector> {
    match (&a.crs, &b.crs) {
        (Some(x), Some(y)) if !terraflux_gdal::same_crs(y, x).unwrap_or(true) => {
            terraflux_gdal::reproject_vector(b, x).map_err(Error::msg)
        }
        _ => Ok(b.clone()),
    }
}

/// The pairs (i, j) of features of a and b with the predicate (`disjoint`: those that intersect).
fn pairs(a: &Vector, b: &Vector, predicate: &str, distance: Option<f64>) -> Result<Vec<(usize, usize)>> {
    let p = if predicate == "disjoint" { Some(Predicate::Intersects) } else { Predicate::from_name(predicate) };
    let p = p.ok_or_else(|| Error::msg(format!("unknown predicate {predicate:?}")))?;
    GeosIndex::new(&b.geometries).and_then(|t| t.pairs(&a.geometries, p, distance)).map_err(Error::msg)
}

/// 1 for each feature of a where the predicate holds with at least one feature of b, else 0 (`disjoint`: 1 where it
/// meets no feature of b; an empty geometry is 0 for all).
pub fn relate(a: &Vector, b: &Vector, predicate: &str, distance: Option<f64>) -> Result<Vec<f64>> {
    let b = like(a, b)?;
    let mut hit = vec![false; a.len()];
    for (i, _) in pairs(a, &b, predicate, distance)? {
        hit[i] = true;
    }
    Ok(hit
        .iter()
        .zip(&a.geometries)
        .map(|(&h, g)| if predicate == "disjoint" { present(g) && !h } else { h } as u8 as f64)
        .collect())
}

/// How many features of b the predicate holds with, for each feature of a (`disjoint`: those it does not meet).
pub fn count(a: &Vector, b: &Vector, predicate: &str, distance: Option<f64>) -> Result<Vec<f64>> {
    let b = like(a, b)?;
    let mut n = vec![0usize; a.len()];
    for (i, _) in pairs(a, &b, predicate, distance)? {
        n[i] += 1;
    }
    let live = b.geometries.iter().filter(|g| present(g)).count();
    Ok(n.iter()
        .zip(&a.geometries)
        .map(|(&n, g)| match (predicate == "disjoint", present(g)) {
            (true, true) => live.saturating_sub(n) as f64,
            (true, false) => 0.0,
            _ => n as f64,
        })
        .collect())
}

/// The distance from each feature of a to the nearest of b (NaN for an empty geometry, and with nothing in b).
pub fn distance(a: &Vector, b: &Vector) -> Result<Vec<f64>> {
    let b = like(a, b)?;
    let found = GeosIndex::new(&b.geometries).and_then(|t| t.nearest(&a.geometries, None)).map_err(Error::msg)?;
    Ok(found.into_iter().map(|f| f.map_or(f64::NAN, |(_, d)| d)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Geometry {
        Geometry::Polygon(vec![vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s], [x, y]]])
    }

    fn points(p: &[[f64; 2]]) -> Vector {
        Vector { geometries: p.iter().map(|&p| Geometry::Point(p)).collect(), ..Default::default() }
    }

    fn parks() -> Vector {
        Vector { geometries: vec![square(0.0, 0.0, 10.0), square(20.0, 0.0, 10.0)], ..Default::default() }
    }

    #[test]
    fn predicates_over_a_layer() {
        // inside, on an edge, outside, in the second park, empty
        let mut p = points(&[[5.0, 5.0], [10.0, 5.0], [15.0, 5.0], [25.0, 5.0]]);
        p.geometries.push(Geometry::Empty);
        let r = |name: &str| relate(&p, &parks(), name, None).unwrap();
        assert_eq!(r("within"), [1.0, 0.0, 0.0, 1.0, 0.0]);
        assert_eq!(r("covered_by"), [1.0, 1.0, 0.0, 1.0, 0.0]);
        assert_eq!(r("intersects"), [1.0, 1.0, 0.0, 1.0, 0.0]);
        assert_eq!(r("touches"), [0.0, 1.0, 0.0, 0.0, 0.0]);
        assert_eq!(r("disjoint"), [0.0, 0.0, 1.0, 0.0, 0.0]);
        assert_eq!(relate(&p, &parks(), "dwithin", Some(5.0)).unwrap(), [1.0, 1.0, 1.0, 1.0, 0.0]);
        assert_eq!(relate(&p, &parks(), "dwithin", Some(4.9)).unwrap(), [1.0, 1.0, 0.0, 1.0, 0.0]);
        // the parks over the points
        let q = points(&[[5.0, 5.0], [6.0, 6.0], [10.0, 5.0], [100.0, 0.0]]);
        assert_eq!(relate(&parks(), &q, "contains", None).unwrap(), [1.0, 0.0]);
        assert_eq!(relate(&parks(), &q, "contains_properly", None).unwrap(), [1.0, 0.0]);
        assert_eq!(count(&parks(), &q, "contains", None).unwrap(), [2.0, 0.0]);
        assert_eq!(count(&parks(), &q, "covers", None).unwrap(), [3.0, 0.0]);
        assert_eq!(count(&parks(), &q, "disjoint", None).unwrap(), [1.0, 4.0]);
        assert!(relate(&p, &parks(), "near", None).is_err());
    }

    #[test]
    fn distances_to_a_layer() {
        let mut p = points(&[[5.0, 5.0], [15.0, 5.0], [10.0, 20.0]]);
        p.geometries.push(Geometry::Empty);
        let d = distance(&p, &parks()).unwrap();
        assert_eq!(&d[..3], [0.0, 5.0, 10.0]);
        assert!(d[3].is_nan());
        let none = Vector::default();
        assert!(distance(&p, &none).unwrap().iter().all(|d| d.is_nan()));
        assert_eq!(relate(&p, &none, "intersects", None).unwrap(), [0.0; 4]);
        assert_eq!(relate(&p, &none, "disjoint", None).unwrap(), [1.0, 1.0, 1.0, 0.0]);
    }
}
