//! Spatial predicates through GEOS's spatial index (terraflux_gdal::geos::GeosIndex) and the operators on them
//! (vector.select_by_location, vector.spatial_join, vector.nearest, vector.overlay), on hand-built cases with
//! exact answers and a seeded random case against a loop over the same GEOS predicates. Skipped without GDAL/GEOS.

use std::sync::Arc;

use serde_json::json;
use terraflux_gdal::geos::{GeosIndex, Predicate};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Value};
use terraflux_raster::vector::{Column, Geometry, Vector};

fn run(op: &str, ins: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let args = Args {
        node: "n",
        inputs: ins.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        params: params.as_object().unwrap(),
    };
    Registry::standard().get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn vector(out: &Outputs) -> Arc<Vector> {
    let Value::Vector(v) = &out["vector"] else { panic!() };
    v.clone()
}

fn layer(geometries: Vec<Geometry>, attributes: Vec<(&str, Column)>, crs: Option<&str>) -> Value {
    Value::Vector(Arc::new(Vector {
        geometries,
        attributes: attributes.into_iter().map(|(n, c)| (n.to_string(), c)).collect(),
        crs: crs.map(String::from),
    }))
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Geometry {
    Geometry::Polygon(vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]])
}

fn pt(x: f64, y: f64) -> Geometry {
    Geometry::Point([x, y])
}

fn numbers(v: &Vector, name: &str) -> Vec<f64> {
    v.column(name).unwrap_or_else(|| panic!("no {name}")).numbers()
}

fn texts(v: &Vector, name: &str) -> Vec<String> {
    match v.column(name).unwrap_or_else(|| panic!("no {name}")) {
        Column::Texts(t) => t.clone(),
        Column::Numbers(_) => panic!("{name} is numbers"),
    }
}

fn has_gdal() -> bool {
    if let Err(e) = terraflux_gdal::gdal() {
        eprintln!("skipped: {e}");
        return false;
    }
    true
}

fn holds(a: Geometry, b: Geometry, p: Predicate, d: Option<f64>) -> bool {
    !GeosIndex::new(&[b]).unwrap().pairs(&[a], p, d).unwrap().is_empty()
}

#[test]
fn predicates_on_hand_built_cases() {
    if !has_gdal() {
        return;
    }
    use Predicate::*;
    let sq = rect(0.0, 0.0, 10.0, 10.0);
    // a point on an edge
    let edge = pt(10.0, 5.0);
    assert!(!holds(edge.clone(), sq.clone(), Within, None));
    assert!(holds(edge.clone(), sq.clone(), CoveredBy, None));
    assert!(holds(edge.clone(), sq.clone(), Intersects, None));
    assert!(holds(edge.clone(), sq.clone(), Touches, None));
    assert!(!holds(edge, sq.clone(), ContainsProperly, None));
    // a point inside
    let inside = pt(5.0, 5.0);
    assert!(holds(inside.clone(), sq.clone(), Within, None));
    assert!(!holds(inside.clone(), sq.clone(), Touches, None));
    assert!(holds(sq.clone(), inside.clone(), Contains, None));
    assert!(holds(sq.clone(), inside.clone(), ContainsProperly, None));
    assert!(holds(sq.clone(), inside, Covers, None));
    // two polygons sharing an edge
    let next = rect(10.0, 0.0, 20.0, 10.0);
    assert!(holds(sq.clone(), next.clone(), Touches, None));
    assert!(!holds(sq.clone(), next.clone(), Overlaps, None));
    assert!(holds(sq.clone(), next, Intersects, None));
    // overlapping polygons
    assert!(holds(sq.clone(), rect(5.0, 5.0, 15.0, 15.0), Overlaps, None));
    // a line crossing a polygon
    let line = Geometry::LineString(vec![[-5.0, 5.0], [15.0, 5.0]]);
    assert!(holds(line.clone(), sq.clone(), Crosses, None));
    assert!(!holds(line, sq.clone(), Within, None));
    // the same polygon with another vertex order and start
    let same = Geometry::Polygon(vec![vec![[10.0, 10.0], [10.0, 0.0], [0.0, 0.0], [0.0, 10.0], [10.0, 10.0]]]);
    assert!(holds(sq.clone(), same, Equals, None));
    assert!(!holds(sq.clone(), rect(0.0, 0.0, 10.0, 11.0), Equals, None));
    // dwithin at exactly the distance
    assert!(holds(pt(13.0, 5.0), sq.clone(), Dwithin, Some(3.0)));
    assert!(!holds(pt(13.0, 5.0), sq.clone(), Dwithin, Some(2.999)));
    assert!(holds(pt(10.0, 5.0), sq.clone(), Dwithin, Some(0.0)));
    // disjoint ones meet in nothing
    assert!(!holds(pt(50.0, 50.0), sq.clone(), Intersects, None));
    // empty geometries never match, and an empty layer is fine
    assert!(!holds(Geometry::Empty, sq.clone(), Intersects, None));
    assert!(GeosIndex::new(&[]).unwrap().pairs(std::slice::from_ref(&sq), Intersects, None).unwrap().is_empty());
    assert!(
        GeosIndex::new(&[Geometry::Empty])
            .unwrap()
            .pairs(std::slice::from_ref(&sq), Intersects, None)
            .unwrap()
            .is_empty()
    );
    assert!(GeosIndex::new(&[sq]).unwrap().pairs(&[pt(1.0, 1.0)], Dwithin, None).is_err());
    assert_eq!(Predicate::from_name("covered_by"), Some(CoveredBy));
    assert_eq!(Predicate::from_name("nope"), None);
}

#[test]
fn select_by_location_keeps_and_inverts() {
    if !has_gdal() {
        return;
    }
    let pts = layer(
        vec![pt(5.0, 5.0), pt(10.0, 5.0), pt(30.0, 30.0), pt(13.0, 5.0)],
        vec![("id", Column::Numbers(vec![0.0, 1.0, 2.0, 3.0]))],
        None,
    );
    let polys = layer(vec![rect(0.0, 0.0, 10.0, 10.0)], vec![], None);
    let ids = |predicate: &str, extra: serde_json::Value| -> Vec<f64> {
        let mut p = json!({ "predicate": predicate });
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        numbers(
            &vector(&run("vector.select_by_location", vec![("vector", pts.clone()), ("other", polys.clone())], p)),
            "id",
        )
    };
    assert_eq!(ids("intersects", json!({})), [0.0, 1.0]);
    assert_eq!(ids("within", json!({})), [0.0]);
    assert_eq!(ids("covered_by", json!({})), [0.0, 1.0]);
    assert_eq!(ids("touches", json!({})), [1.0]);
    assert_eq!(ids("disjoint", json!({})), [2.0, 3.0]);
    assert_eq!(ids("intersects", json!({ "invert": true })), [2.0, 3.0]);
    assert_eq!(ids("within", json!({ "invert": true })), [1.0, 2.0, 3.0]);
    assert_eq!(ids("dwithin", json!({ "distance": 3.0 })), [0.0, 1.0, 3.0]);
    assert_eq!(ids("dwithin", json!({ "distance": 3.0, "invert": true })), [2.0]);
}

#[test]
fn the_other_layer_is_reprojected() {
    if !has_gdal() {
        return;
    }
    // UTM 23S, central meridian 45 W: x = 500000 on it, about 7.42 million m north at 23.3 S
    let polys = layer(
        vec![rect(450_000.0, 7_400_000.0, 550_000.0, 7_450_000.0)],
        vec![("zone", Column::Texts(vec!["utm".into()]))],
        Some("EPSG:32723"),
    );
    let pts = layer(
        vec![pt(-45.0, -23.3), pt(-40.0, -23.3)],
        vec![("id", Column::Numbers(vec![0.0, 1.0]))],
        Some("EPSG:4326"),
    );
    let r = vector(&run(
        "vector.select_by_location",
        vec![("vector", pts.clone()), ("other", polys.clone())],
        json!({ "predicate": "within" }),
    ));
    assert_eq!(numbers(&r, "id"), [0.0]);
    assert_eq!(r.crs.as_deref(), Some("EPSG:4326"));
    let j = vector(&run("vector.spatial_join", vec![("vector", pts), ("other", polys)], json!({})));
    assert_eq!(texts(&j, "zone"), ["utm", ""]);
}

fn join_inputs() -> Vec<(&'static str, Value)> {
    let pts = layer(
        vec![pt(7.0, 7.0), pt(2.0, 2.0), pt(50.0, 50.0)],
        vec![("name", Column::Texts(vec!["p0".into(), "p1".into(), "p2".into()]))],
        None,
    );
    // two overlapping squares: p0 is in both, p1 in the first only
    let polys = layer(
        vec![rect(0.0, 0.0, 10.0, 10.0), rect(5.0, 5.0, 15.0, 15.0)],
        vec![("name", Column::Texts(vec!["b0".into(), "b1".into()])), ("v", Column::Numbers(vec![10.0, 20.0]))],
        None,
    );
    vec![("vector", pts), ("other", polys)]
}

#[test]
fn spatial_join_modes() {
    if !has_gdal() {
        return;
    }
    let p = json!({ "predicate": "within" });
    // attributes: the lowest index of b
    let r = vector(&run("vector.spatial_join", join_inputs(), p.clone()));
    assert_eq!(texts(&r, "name"), ["p0", "p1", "p2"]);
    assert_eq!(texts(&r, "name_2"), ["b0", "b0", ""]);
    let v = numbers(&r, "v");
    assert_eq!(&v[..2], [10.0, 10.0]);
    assert!(v[2].is_nan());
    // all: a pair per row, by a then b, a feature meeting none once
    let r = vector(&run("vector.spatial_join", join_inputs(), json!({ "predicate": "within", "mode": "all" })));
    assert_eq!(r.len(), 4);
    assert_eq!(texts(&r, "name"), ["p0", "p0", "p1", "p2"]);
    assert_eq!(texts(&r, "name_2"), ["b0", "b1", "b0", ""]);
    let v = numbers(&r, "v");
    assert_eq!(&v[..3], [10.0, 20.0, 10.0]);
    assert!(v[3].is_nan());
    assert_eq!(r.geometries[3], pt(50.0, 50.0));
    // summary
    let r = vector(&run(
        "vector.spatial_join",
        join_inputs(),
        json!({ "predicate": "within", "mode": "summary", "fields": "v" }),
    ));
    assert_eq!(numbers(&r, "count"), [2.0, 1.0, 0.0]);
    assert_eq!(numbers(&r, "v_sum")[..2], [30.0, 10.0]);
    assert_eq!(numbers(&r, "v_mean")[..2], [15.0, 10.0]);
    // the default predicate is intersects: the same as before
    let r = vector(&run("vector.spatial_join", join_inputs(), json!({ "mode": "summary" })));
    assert_eq!(numbers(&r, "count"), [2.0, 1.0, 0.0]);
    // dwithin
    let r = vector(&run(
        "vector.spatial_join",
        join_inputs(),
        json!({ "predicate": "dwithin", "distance": 50.0, "mode": "summary" }),
    ));
    assert_eq!(numbers(&r, "count"), [2.0, 2.0, 1.0]);
}

#[test]
fn nearest_with_ties_and_a_maximum() {
    if !has_gdal() {
        return;
    }
    let a = layer(
        vec![pt(1.0, 0.0), pt(10.0, 0.0), pt(2.0, 0.0), Geometry::Empty],
        vec![("id", Column::Numbers(vec![0.0, 1.0, 2.0, 3.0]))],
        None,
    );
    // b0 and b1 are both at 1 from (1, 0): the lowest wins; (2, 0) is on b1
    let b = layer(
        vec![pt(0.0, 0.0), pt(2.0, 0.0), pt(5.0, 0.0)],
        vec![("name", Column::Texts(vec!["b0".into(), "b1".into(), "b2".into()]))],
        None,
    );
    let r = vector(&run("vector.nearest", vec![("vector", a.clone()), ("other", b.clone())], json!({})));
    assert_eq!(texts(&r, "name"), ["b0", "b2", "b1", ""]);
    let d = numbers(&r, "distance");
    assert_eq!(&d[..3], [1.0, 5.0, 0.0]);
    assert!(d[3].is_nan());
    let r = vector(&run("vector.nearest", vec![("vector", a), ("other", b)], json!({ "max_distance": 3.0 })));
    assert_eq!(texts(&r, "name"), ["b0", "", "b1", ""]);
    let d = numbers(&r, "distance");
    assert_eq!(d[0], 1.0);
    assert!(d[1].is_nan() && d[3].is_nan());
    // polygons: the distance is to the edge
    let a = layer(vec![pt(14.0, 5.0)], vec![], None);
    let b = layer(
        vec![rect(0.0, 0.0, 10.0, 10.0), rect(20.0, 0.0, 30.0, 10.0)],
        vec![("k", Column::Numbers(vec![1.0, 2.0]))],
        None,
    );
    let r = vector(&run("vector.nearest", vec![("vector", a), ("other", b)], json!({})));
    assert_eq!(numbers(&r, "k"), [1.0]);
    assert_eq!(numbers(&r, "distance"), [4.0]);
}

#[test]
fn overlay_uses_the_index_and_keeps_its_results() {
    if !has_gdal() {
        return;
    }
    let a = layer(
        vec![rect(0.0, 0.0, 10.0, 10.0), rect(100.0, 100.0, 110.0, 110.0)],
        vec![("a", Column::Numbers(vec![1.0, 2.0]))],
        None,
    );
    let b = layer(
        vec![rect(5.0, 5.0, 15.0, 15.0), rect(10.0, 0.0, 20.0, 10.0)],
        vec![("b", Column::Numbers(vec![1.0, 2.0]))],
        None,
    );
    // the second square of b only touches the first of a: no area, no feature
    let r = vector(&run("vector.overlay", vec![("vector", a), ("other", b)], json!({ "how": "intersection" })));
    assert_eq!(r.len(), 1);
    assert_eq!(numbers(&r, "a"), [1.0]);
    assert_eq!(numbers(&r, "b"), [1.0]);
}

/// A generator of numbers in [0, 1) (an LCG).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[test]
fn indexed_pairs_match_a_loop_over_the_same_predicates() {
    if !has_gdal() {
        return;
    }
    use Predicate::*;
    let mut rng = Lcg(20240607);
    // a: 300 points on a coarse grid (so many fall on edges), some lines, a few small polygons
    let mut a: Vec<Geometry> =
        (0..300).map(|_| pt((rng.next() * 25.0).round() * 4.0, (rng.next() * 25.0).round() * 4.0)).collect();
    for _ in 0..20 {
        let (x, y) = ((rng.next() * 25.0).round() * 4.0, (rng.next() * 25.0).round() * 4.0);
        a.push(Geometry::LineString(vec![[x, y], [x + (rng.next() * 40.0).round(), y + (rng.next() * 40.0).round()]]));
    }
    for _ in 0..15 {
        let (x, y) = ((rng.next() * 25.0).round() * 4.0, (rng.next() * 25.0).round() * 4.0);
        a.push(rect(x, y, x + 4.0 * (1.0 + (rng.next() * 3.0).floor()), y + 4.0 * (1.0 + (rng.next() * 3.0).floor())));
    }
    // b: 40 rectangles on the same grid, overlapping, with a few copies
    let mut b: Vec<Geometry> = (0..40)
        .map(|_| {
            let (x, y) = ((rng.next() * 20.0).round() * 4.0, (rng.next() * 20.0).round() * 4.0);
            rect(x, y, x + 4.0 * (1.0 + (rng.next() * 6.0).floor()), y + 4.0 * (1.0 + (rng.next() * 6.0).floor()))
        })
        .collect();
    b.push(b[3].clone());
    b.push(Geometry::Empty);
    let tree = GeosIndex::new(&b).unwrap();
    // the loop: each b alone in an index of one (its envelope meets or not, the predicate decides)
    let singles: Vec<GeosIndex> = b.iter().map(|g| GeosIndex::new(std::slice::from_ref(g)).unwrap()).collect();
    let mut any = 0;
    for (p, d) in [
        (Intersects, None),
        (Within, None),
        (Contains, None),
        (ContainsProperly, None),
        (Touches, None),
        (Crosses, None),
        (Overlaps, None),
        (Covers, None),
        (CoveredBy, None),
        (Equals, None),
        (Dwithin, Some(3.0)),
        (Dwithin, Some(0.0)),
    ] {
        let got = tree.pairs(&a, p, d).unwrap();
        let mut want: Vec<(usize, usize)> = Vec::new();
        for (j, s) in singles.iter().enumerate() {
            want.extend(s.pairs(&a, p, d).unwrap().into_iter().map(|(i, _)| (i, j)));
        }
        want.sort_unstable();
        assert_eq!(got, want, "{p:?}");
        any += got.len();
    }
    assert!(any > 500, "the case should have many pairs: {any}");
    // and intersects against OGR's own test on every pair
    let (ga, gb) = (terraflux_gdal::Geos::new(&a).unwrap(), terraflux_gdal::Geos::new(&b).unwrap());
    let want: Vec<(usize, usize)> = (0..a.len())
        .flat_map(|i| (0..b.len()).map(move |j| (i, j)))
        .filter(|&(i, j)| ga.intersects_other(i, &gb, j))
        .collect();
    assert_eq!(tree.pairs(&a, Intersects, None).unwrap(), want);
    // nearest: the exact best by distance, the lowest index on ties
    let near = tree.nearest(&a, None).unwrap();
    let dist: Vec<Vec<Option<(usize, f64)>>> = singles.iter().map(|s| s.nearest(&a, None).unwrap()).collect();
    for (i, got) in near.iter().enumerate() {
        let mut best: Option<(usize, f64)> = None;
        for (j, d) in dist.iter().enumerate() {
            if let Some((_, d)) = d[i]
                && best.is_none_or(|(_, bd)| d < bd)
            {
                best = Some((j, d));
            }
        }
        assert_eq!(*got, best, "nearest of {i}");
    }
}
