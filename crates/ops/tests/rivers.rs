//! Rivers as a network (hydro.snap, hydro.upstream, hydro.upstream_catchment) against the kernels of hydro that
//! are landschaft's to the bit: the land upstream of a cell is what the D8 accumulation sums there, and a point's
//! catchment is hydro.watersheds of that point alone. On the DEMs of tests/data/reference/hydro.npz.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn data(out: &Outputs, port: &str) -> Vec<f64> {
    let Value::Map(m) = &out[port] else { panic!("{port}") };
    m.data.as_ref().clone()
}

fn table(out: &Outputs, port: &str) -> Table {
    let Value::Table(t) = &out[port] else { panic!("{port}") };
    t.clone()
}

fn col(t: &Table, name: &str) -> Vec<f64> {
    t.column(name).unwrap_or_else(|| panic!("no column {name} in {:?}", t.columns))
}

fn vector(out: &Outputs) -> Arc<Vector> {
    let Value::Vector(v) = &out["vector"] else { panic!("vector") };
    v.clone()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0) || (a.is_nan() && b.is_nan())
}

/// A DEM of the references, filled.
fn filled(name: &str) -> Map {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/hydro.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/hydro.json")).unwrap())
            .unwrap();
    let d = meta["dems"].as_array().unwrap().iter().find(|d| d["name"] == name).unwrap();
    let e: Vec<f64> = serde_json::from_value(d["extent"].clone()).unwrap();
    let raw = &z[&format!("{name}/dem")];
    let (ny, nx) = (raw.shape[0], raw.shape[1]);
    let mut g = Grid::new(nx, ny);
    g.transform = [e[0], (e[1] - e[0]) / nx as f64, 0.0, e[3], 0.0, -(e[3] - e[2]) / ny as f64];
    let dem = Map::new(Arc::new(g), raw.data.clone());
    let Value::Map(f) = &run("hydro.fill", vec![("dem", Value::Map(dem))], serde_json::json!({}))["map"] else {
        panic!()
    };
    f.clone()
}

/// The area of a cell of a DEM.
fn cell_area(dem: &Map) -> f64 {
    let (rx, ry) = dem.grid.resolution();
    rx * ry
}

fn map_like(dem: &Map, v: Vec<f64>) -> Value {
    Value::Map(Map::new(dem.grid.clone(), v))
}

/// Values of every cell that are not smooth (so a wrong cell shows), some missing.
fn values(n: usize) -> Vec<f64> {
    (0..n).map(|i| if i % 97 == 5 { f64::NAN } else { ((i * 7919) % 1000) as f64 / 10.0 }).collect()
}

/// Classes 1 to 4 of every cell, some without a class.
fn classes(n: usize) -> Vec<f64> {
    (0..n).map(|i| if i % 61 == 0 { f64::NAN } else { ((i * 31 + i / 50) % 4 + 1) as f64 }).collect()
}

/// The centre of a cell of the grid.
fn centre(g: &Grid, row: f64, col: f64) -> Xy {
    let t = g.transform;
    [t[0] + (col + 0.5) * t[1], t[3] + (row + 0.5) * t[5]]
}

fn points(ps: &[Xy]) -> Value {
    Value::Vector(Arc::new(Vector {
        geometries: ps.iter().map(|p| Geometry::Point(*p)).collect(),
        attributes: vec![("name".into(), Column::Texts(ps.iter().enumerate().map(|(i, _)| format!("p{i}")).collect()))],
        crs: None,
    }))
}

#[test]
fn the_land_upstream_of_the_links_is_what_drains_through_them() {
    for name in ["oregon", "walk"] {
        let dem = filled(name);
        let n = dem.data.len();
        let nx = dem.grid.width;
        let fm = || Value::Map(dem.clone());
        let acc = run("hydro.flow_accumulation", vec![("dem", fm())], serde_json::json!({}));
        let streams =
            run("hydro.streams", vec![("accumulation", acc["map"].clone())], serde_json::json!({"threshold": 20}));
        let links =
            run("hydro.stream_links", vec![("dem", fm()), ("streams", streams["map"].clone())], serde_json::json!({}));
        let lt = table(&links, "table");
        let (v, c) = (values(n), classes(n));
        let out = run(
            "hydro.upstream",
            vec![
                ("dem", fm()),
                ("links", links["map"].clone()),
                ("values", map_like(&dem, v.clone())),
                ("classes", map_like(&dem, c.clone())),
            ],
            serde_json::json!({"stats": ["count", "sum", "mean", "min", "max", "std"], "to_map": "sum"}),
        );
        let t = table(&out, "table");
        assert_eq!(
            t.columns,
            [
                "link",
                "downstream",
                "cells",
                "area",
                "count",
                "sum",
                "mean",
                "min",
                "max",
                "std",
                "share_1",
                "share_2",
                "share_3",
                "share_4"
            ]
        );
        assert_eq!(col(&t, "link"), col(&lt, "link"), "{name}: a row per link");
        assert_eq!(col(&t, "downstream"), col(&lt, "downstream"), "{name}: the link below, as stream_links'");
        // the D8 accumulations (of 1, of the values, of each class) at each link's last cell
        let weighted = |w: Vec<f64>| {
            data(
                &run(
                    "hydro.flow_accumulation",
                    vec![("dem", fm()), ("weights", map_like(&dem, w))],
                    serde_json::json!({}),
                ),
                "map",
            )
        };
        let ones = data(&acc, "map");
        let sum = weighted(v.clone());
        let count = weighted(v.iter().map(|x| f64::from(u8::from(!x.is_nan()))).collect());
        let classified = weighted(c.iter().map(|x| f64::from(u8::from(!x.is_nan()))).collect());
        let class_2 = weighted(c.iter().map(|x| f64::from(u8::from(*x == 2.0))).collect());
        let (er, ec) = (col(&lt, "end_row"), col(&lt, "end_col"));
        for (i, row) in t.rows.iter().enumerate() {
            let k = er[i] as usize * nx + ec[i] as usize;
            assert_eq!(row[2], ones[k], "{name} link {}: cells", row[0]);
            assert_eq!(row[3], ones[k] * cell_area(&dem), "{name}: area");
            assert_eq!(row[4], count[k], "{name}: count");
            assert!(close(row[5], sum[k]), "{name} link {}: sum {} {}", row[0], row[5], sum[k]);
            assert!(close(row[6], sum[k] / count[k]), "{name}: mean");
            assert!(close(row[11], class_2[k] / classified[k]), "{name}: share of class 2");
            let shares: f64 = row[10..14].iter().sum();
            assert!(close(shares, 1.0));
        }
        // min, max and std of a few links against their cells: the watershed of the last cell alone
        for i in [0, t.rows.len() / 2, t.rows.len() - 1] {
            let k = er[i] as usize * nx + ec[i] as usize;
            let mut seed = vec![0.0; n];
            seed[k] = 1.0;
            let ws = data(
                &run("hydro.watersheds", vec![("dem", fm()), ("seeds", map_like(&dem, seed))], serde_json::json!({})),
                "map",
            );
            let up: Vec<f64> = (0..n).filter(|&j| ws[j] == 1.0 && !v[j].is_nan()).map(|j| v[j]).collect();
            let mean = up.iter().sum::<f64>() / up.len() as f64;
            let std = (up.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / up.len() as f64).sqrt();
            let row = &t.rows[i];
            assert_eq!(row[7], up.iter().copied().fold(f64::INFINITY, f64::min), "{name}: min");
            assert_eq!(row[8], up.iter().copied().fold(f64::NEG_INFINITY, f64::max), "{name}: max");
            assert!(close(row[9], std), "{name}: std {} {std}", row[9]);
        }
        // the map: each link's sum on its cells
        let lm = data(&links, "map");
        let m = data(&out, "map");
        for k in 0..n {
            if lm[k] > 0.0 {
                assert_eq!(m[k], t.rows[lm[k] as usize - 1][5]);
            } else {
                assert!(m[k].is_nan());
            }
        }
        // points on the links' last cells: the same rows
        let ps: Vec<Xy> = (0..t.rows.len()).map(|i| centre(&dem.grid, er[i], ec[i])).collect();
        let by_points = run(
            "hydro.upstream",
            vec![
                ("dem", fm()),
                ("points", points(&ps)),
                ("streams", streams["map"].clone()),
                ("values", map_like(&dem, v.clone())),
            ],
            serde_json::json!({"stats": "sum, mean"}),
        );
        let pt = table(&by_points, "table");
        assert_eq!(pt.columns, ["point", "downstream", "row", "col", "cells", "area", "sum", "mean"]);
        assert_eq!(col(&pt, "cells"), col(&t, "cells"), "{name}: points as the links");
        assert_eq!(col(&pt, "downstream"), col(&t, "downstream"), "{name}: the point below the link below");
        assert_eq!(col(&pt, "row"), er);
    }
}

#[test]
fn a_catchment_is_all_that_drains_to_its_point() {
    let dem = filled("oregon");
    let n = dem.data.len();
    let nx = dem.grid.width;
    let fm = || Value::Map(dem.clone());
    let acc = run("hydro.flow_accumulation", vec![("dem", fm())], serde_json::json!({}));
    let streams =
        run("hydro.streams", vec![("accumulation", acc["map"].clone())], serde_json::json!({"threshold": 20}));
    let ones = data(&acc, "map");
    // the outlet of the largest river, a point up it, and one on another river; a point off the streams
    let mut big: Vec<usize> = (0..n).filter(|&k| !ones[k].is_nan()).collect();
    big.sort_by(|a, b| ones[*b].total_cmp(&ones[*a]));
    let outlet = big[0];
    let Value::Map(ws) = &run(
        "hydro.watersheds",
        vec![
            ("dem", fm()),
            ("seeds", {
                let mut s = vec![0.0; n];
                s[outlet] = 1.0;
                map_like(&dem, s)
            }),
        ],
        serde_json::json!({}),
    )["map"] else {
        panic!()
    };
    let up = *big.iter().find(|&&k| ws.data[k] == 1.0 && ones[k] < ones[outlet] / 2.0 && ones[k] > 50.0).unwrap();
    let other = *big.iter().find(|&&k| ws.data[k] != 1.0 && ones[k] > 50.0).unwrap();
    let at = |k: usize| centre(&dem.grid, (k / nx) as f64, (k % nx) as f64);
    let ps = [at(outlet), at(up), at(other), [-5000.0, -5000.0]];
    let out = run(
        "hydro.upstream_catchment",
        vec![("dem", fm()), ("points", points(&ps)), ("streams", streams["map"].clone())],
        serde_json::json!({}),
    );
    let t = table(&out, "table");
    assert_eq!(col(&t, "downstream")[..3], [0.0, 1.0, 0.0], "the second drains to the first");
    assert!(t.rows[3][4].is_nan(), "off the network: no catchment");
    let v = vector(&out);
    assert_eq!(v.len(), 4);
    assert!(matches!(v.geometries[3], Geometry::Empty));
    assert!(v.attributes.iter().any(|(n, _)| n == "name"), "the points' attributes kept");
    for (i, &k) in [outlet, up, other].iter().enumerate() {
        let mut seed = vec![0.0; n];
        seed[k] = 1.0;
        let alone = data(
            &run("hydro.watersheds", vec![("dem", fm()), ("seeds", map_like(&dem, seed))], serde_json::json!({})),
            "map",
        );
        let cells = alone.iter().filter(|x| **x == 1.0).count() as f64;
        assert_eq!(t.rows[i][4], cells, "point {i}: its cells");
        assert_eq!(t.rows[i][4], ones[k], "point {i}: the accumulation there");
        // the polygon's area: its rings' (holes taken away)
        let polys = match &v.geometries[i] {
            Geometry::Polygon(p) => vec![p.clone()],
            Geometry::MultiPolygon(ps) => ps.clone(),
            g => panic!("{g:?}"),
        };
        let ring = |r: &Vec<Xy>| r.windows(2).map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1]).sum::<f64>() / 2.0;
        let area: f64 =
            polys.iter().map(|p| ring(&p[0]).abs() - p[1..].iter().map(|h| ring(h).abs()).sum::<f64>()).sum();
        assert!(close(area, cells * cell_area(&dem)), "point {i}: area {area}");
    }
    // the map: the nested catchments, as hydro.watersheds of the three points
    let mut seed = vec![0.0; n];
    for (i, &k) in [outlet, up, other].iter().enumerate() {
        seed[k] = (i + 1) as f64;
    }
    let nested = data(
        &run("hydro.watersheds", vec![("dem", fm()), ("seeds", map_like(&dem, seed))], serde_json::json!({})),
        "map",
    );
    let m = data(&out, "map");
    assert!(m.iter().zip(&nested).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())));
}

#[test]
fn points_are_moved_onto_the_streams() {
    let dem = filled("oregon");
    let n = dem.data.len();
    let nx = dem.grid.width;
    let acc = run("hydro.flow_accumulation", vec![("dem", Value::Map(dem.clone()))], serde_json::json!({}));
    let streams =
        run("hydro.streams", vec![("accumulation", acc["map"].clone())], serde_json::json!({"threshold": 20}));
    let (s, a) = (data(&streams, "map"), data(&acc, "map"));
    let on: Vec<usize> = (0..n).filter(|&k| s[k] == 1.0).collect();
    let (rx, _) = dem.grid.resolution();
    let at = |k: usize| centre(&dem.grid, (k / nx) as f64, (k % nx) as f64);
    // the nearest of some cells to a point, of equally near ones the first in raster order
    let nearest = |p: Xy, cells: &[usize]| -> (f64, usize) {
        cells
            .iter()
            .map(|&k| ((at(k)[0] - p[0]).hypot(at(k)[1] - p[1]), k))
            .fold((f64::INFINITY, 0), |b, x| if x.0 < b.0 { x } else { b })
    };
    // a stream cell; points two and a third of a cell away from it, and one far off
    let c = at(on[on.len() / 3]);
    let ps = [[c[0] + 2.0 * rx, c[1]], [c[0] + 0.3 * rx, c[1] - 0.2 * rx], [-9000.0, 0.0]];
    let out = run(
        "hydro.snap",
        vec![("points", points(&ps)), ("streams", streams["map"].clone())],
        serde_json::json!({"distance": 2.5 * rx}),
    );
    let v = vector(&out);
    let t = table(&out, "table");
    for (i, p) in ps.iter().take(2).enumerate() {
        let (d, k) = nearest(*p, &on);
        assert_eq!(v.geometries[i], Geometry::Point(at(k)), "point {i}");
        assert_eq!(col(&t, "moved")[i], d);
        assert_eq!(col(&t, "row")[i], (k / nx) as f64);
        assert_eq!(col(&t, "stream")[i], 1.0);
    }
    assert_eq!(v.geometries[2], Geometry::Point([-9000.0, 0.0]), "too far: where it was");
    assert_eq!(col(&t, "snapped"), [1.0, 1.0, 0.0]);
    // with a minimum accumulation: the nearest cell of a river that large, farther
    let big = on.iter().map(|&k| a[k]).fold(0.0, f64::max) / 2.0;
    let out = run(
        "hydro.snap",
        vec![("points", points(&ps[..1])), ("streams", streams["map"].clone()), ("accumulation", acc["map"].clone())],
        serde_json::json!({"distance": 100000, "min_accumulation": big}),
    );
    let Geometry::Point(p) = vector(&out).geometries[0] else { panic!() };
    let t = table(&out, "table");
    let j = col(&t, "row")[0] as usize * nx + col(&t, "col")[0] as usize;
    assert!(a[j] >= big && s[j] == 1.0);
    let large: Vec<usize> = on.iter().copied().filter(|&k| a[k] >= big).collect();
    assert_eq!(p, at(nearest(ps[0], &large).1));
}
