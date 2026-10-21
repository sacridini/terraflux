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

/// The dendritic connectivity index and the reach of every segment are riverconn's (tools/make_river_dci_reference.py),
/// and so is the gain of removing each barrier.
#[test]
fn the_dci_is_riverconns() {
    use terraflux_ops::river_network::Segments;
    let refs: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/river_dci.json")).unwrap())
            .unwrap();
    let nums = |v: &serde_json::Value| -> Vec<f64> { serde_json::from_value(v.clone()).unwrap() };
    let near = |a: f64, b: f64, what: &str| assert!((a - b).abs() <= 1e-12 * b.abs().max(1e-3), "{what}: {a} {b}");
    for (name, c) in refs.as_object().unwrap() {
        let below: Vec<i64> = serde_json::from_value(c["below"].clone()).unwrap();
        let mut net = Segments {
            below: below.iter().map(|&b| (b >= 0).then_some(b as usize)).collect(),
            weight: nums(&c["length"]),
            up: nums(&c["up"]),
            down: nums(&c["down"]),
        };
        let r = &c["riverconn"];
        let total: f64 = net.weight.iter().sum();
        for (symmetric, key) in [(true, "full_sym"), (false, "full_asym")] {
            let reach = net.reach(symmetric, false);
            let full: f64 = net.weight.iter().zip(&reach).map(|(w, x)| w * x).sum::<f64>() / (total * total);
            near(full, nums(&r[key])[0], &format!("{name} {key}"));
        }
        for (symmetric, from, key) in
            [(true, false, "reach_to_sym"), (false, false, "reach_to_asym"), (false, true, "reach_from_asym")]
        {
            let want = nums(&r[key]);
            for (i, x) in net.reach(symmetric, from).iter().enumerate() {
                near(x / total, want[i], &format!("{name} {key} {i}"));
            }
        }
        let (p, d) = net.dci(true)[&0];
        near(p / 100.0, nums(&r["full_sym"])[0], &format!("{name} dci_p"));
        near(d / 100.0, nums(&r["reach_to_sym"])[0], &format!("{name} dci_d: the reach of the mouth"));
        // each barrier removed
        let ids: Vec<Option<String>> = serde_json::from_value(c["barrier"].clone()).unwrap();
        let removed: Vec<String> = serde_json::from_value(r["d_barrier"].clone()).unwrap();
        for (j, id) in removed.iter().enumerate() {
            let s = ids.iter().position(|x| x.as_deref() == Some(id.as_str())).unwrap();
            let kept = (net.up[s], net.down[s]);
            (net.up[s], net.down[s]) = (1.0, 1.0);
            let without = net.dci(true)[&0].0;
            (net.up[s], net.down[s]) = kept;
            near(without / 100.0, nums(&r["d_index_value"])[j], &format!("{name} without {id}"));
            near((without - p) / p * 100.0, nums(&r["d_index"])[j], &format!("{name} d_index of {id}"));
        }
    }
}

/// The links' table of a DEM's streams.
fn network(dem: &Map) -> (Outputs, Outputs, Outputs) {
    let fm = || Value::Map(dem.clone());
    let acc = run("hydro.flow_accumulation", vec![("dem", fm())], serde_json::json!({}));
    let streams =
        run("hydro.streams", vec![("accumulation", acc["map"].clone())], serde_json::json!({"threshold": 20}));
    let links =
        run("hydro.stream_links", vec![("dem", fm()), ("streams", streams["map"].clone())], serde_json::json!({}));
    (acc, streams, links)
}

/// The step from each cell to the one it drains to, from the D8 codes (ESRI's): a side or the diagonal.
fn steps(dem: &Map) -> (Vec<f64>, Vec<i64>) {
    let dir = data(&run("hydro.flow_direction", vec![("dem", Value::Map(dem.clone()))], serde_json::json!({})), "map");
    let (dx, dy) = dem.grid.resolution();
    let nx = dem.grid.width as i64;
    let ny = dem.grid.height as i64;
    let mut len = vec![dx.min(dy); dir.len()];
    let mut to = vec![-1i64; dir.len()];
    for (k, d) in dir.iter().enumerate() {
        let (dr, dc) = match *d as i64 {
            1 => (0, 1),
            2 => (1, 1),
            4 => (1, 0),
            8 => (1, -1),
            16 => (0, -1),
            32 => (-1, -1),
            64 => (-1, 0),
            128 => (-1, 1),
            _ => continue,
        };
        let (r, c) = (k as i64 / nx + dr, k as i64 % nx + dc);
        if r >= 0 && c >= 0 && r < ny && c < nx && !dem.data[(r * nx + c) as usize].is_nan() {
            to[k] = r * nx + c;
            len[k] = if dr != 0 && dc != 0 {
                dx.hypot(dy)
            } else if dr != 0 {
                dy
            } else {
                dx
            };
        }
    }
    (len, to)
}

#[test]
fn distances_and_reaches_follow_the_river() {
    let dem = filled("oregon");
    let n = dem.data.len();
    let nx = dem.grid.width;
    let fm = || Value::Map(dem.clone());
    let (_, streams, links) = network(&dem);
    let lt = table(&links, "table");
    let (er, ec) = (col(&lt, "end_row"), col(&lt, "end_col"));
    let (sr, sc) = (col(&lt, "start_row"), col(&lt, "start_col"));
    let (len, to) = steps(&dem);
    // the path down from a cell, each with its distance
    let path = |k: usize| {
        let mut out = vec![(k, 0.0)];
        let mut at = k;
        while to[at] >= 0 {
            let d = out.last().unwrap().1 + len[at];
            at = to[at] as usize;
            out.push((at, d));
        }
        out
    };
    // the first cells of a few links and the last of another, of the highest order (with a link below): points on the
    // river
    let (order, below) = (col(&lt, "order"), col(&lt, "downstream"));
    let mid = (0..lt.rows.len())
        .filter(|&i| below[i] > 0.0)
        .max_by(|i, j| order[*i].total_cmp(&order[*j]).then(j.cmp(i)))
        .unwrap();
    let picks: Vec<usize> = vec![
        sr[0] as usize * nx + sc[0] as usize,
        sr[3] as usize * nx + sc[3] as usize,
        er[mid] as usize * nx + ec[mid] as usize,
        sr[lt.rows.len() - 1] as usize * nx + sc[lt.rows.len() - 1] as usize,
    ];
    let ps: Vec<Xy> = picks.iter().map(|&k| centre(&dem.grid, (k / nx) as f64, (k % nx) as f64)).collect();
    let out = run(
        "hydro.network_distance",
        vec![("dem", fm()), ("points", points(&ps)), ("streams", streams["map"].clone())],
        serde_json::json!({}),
    );
    let t = table(&out, "table");
    assert_eq!(t.rows.len(), 6);
    for row in &t.rows {
        let (a, b) = (picks[row[0] as usize - 1], picks[row[1] as usize - 1]);
        let (pa, pb) = (path(a), path(b));
        let meet = pb.iter().find_map(|(k, db)| pa.iter().find(|(j, _)| j == k).map(|(_, da)| da + db));
        match meet {
            Some(d) => {
                assert!(close(row[2], d), "{row:?} {d}");
                assert_eq!(row[4], 1.0);
                let flows = pa.iter().any(|(k, _)| *k == b) || pb.iter().any(|(k, _)| *k == a);
                assert_eq!(row[5], f64::from(u8::from(flows)));
            }
            None => assert!(row[2].is_nan() && row[4] == 0.0),
        }
        let (qa, qb) = (ps[row[0] as usize - 1], ps[row[1] as usize - 1]);
        assert!(close(row[3], (qa[0] - qb[0]).hypot(qa[1] - qb[1])));
    }
    // up and down the river from the third point (the last cell of a link in the middle)
    let (dx, _) = dem.grid.resolution();
    let (up, down) = (12.0 * dx, 7.0 * dx);
    let out = run(
        "hydro.along_network",
        vec![("dem", fm()), ("points", points(&ps[2..3])), ("streams", streams["map"].clone())],
        serde_json::json!({"upstream": up, "downstream": down}),
    );
    let (m, dist) = (data(&out, "map"), data(&out, "distance"));
    let s = data(&streams, "map");
    let k0 = picks[2];
    let below: Vec<(usize, f64)> = path(k0).into_iter().filter(|(_, d)| *d <= down).collect();
    for k in 0..n {
        let downstream = below.iter().find(|(j, _)| *j == k).map(|(_, d)| *d);
        // upstream: a stream cell whose path down reaches k0 within the distance, on stream cells all the way
        let upstream = (s[k] == 1.0)
            .then(|| path(k))
            .and_then(|p| {
                let i = p.iter().position(|(j, _)| *j == k0)?;
                p[..i].iter().all(|(j, _)| s[*j] == 1.0).then_some(p[i].1)
            })
            .filter(|d| *d <= up);
        match downstream.or(upstream) {
            Some(d) => {
                assert_eq!(m[k], 1.0, "cell {k}");
                assert!(close(dist[k], d), "cell {k}: {} {d}", dist[k]);
            }
            None => assert!(m[k] == 0.0 && dist[k].is_nan(), "cell {k}: {} {}", m[k], dist[k]),
        }
    }
    let reached = m.iter().filter(|x| **x == 1.0).count();
    assert!(
        reached > 15,
        "{reached} cells, {} stream cells above, dx {dx}",
        (0..n).filter(|&k| s[k] == 1.0 && path(k).iter().any(|(j, _)| *j == k0)).count()
    );
}

#[test]
fn centrality_counts_the_paths_through_each_link() {
    let dem = filled("oregon");
    let (_, _, links) = network(&dem);
    let lt = table(&links, "table");
    let out = run("hydro.centrality", vec![("links", links["table"].clone())], serde_json::json!({}));
    let t = table(&out, "table");
    let n = t.rows.len();
    assert_eq!(col(&t, "link"), col(&lt, "link"));
    // the undirected graph of the links, its distances by breadth first search
    let ids = col(&lt, "link");
    let below = col(&lt, "downstream");
    let index = |l: f64| ids.iter().position(|x| *x == l);
    let mut adj = vec![Vec::new(); n];
    for i in 0..n {
        if let Some(j) = index(below[i]) {
            adj[i].push(j);
            adj[j].push(i);
        }
    }
    let bfs = |s: usize| {
        let mut d = vec![usize::MAX; n];
        d[s] = 0;
        let mut q = std::collections::VecDeque::from([s]);
        while let Some(v) = q.pop_front() {
            for &u in &adj[v] {
                if d[u] == usize::MAX {
                    d[u] = d[v] + 1;
                    q.push_back(u);
                }
            }
        }
        d
    };
    let all: Vec<Vec<usize>> = (0..n).map(bfs).collect();
    let length = col(&lt, "length");
    for v in 0..n {
        let reach: Vec<usize> = (0..n).filter(|&u| all[v][u] != usize::MAX).collect();
        let sum: usize = reach.iter().map(|&u| all[v][u]).sum();
        // v lies on the path of a, b when d(a, v) + d(v, b) = d(a, b)
        let mut through = 0;
        for &a in &reach {
            for &b in &reach {
                if a < b && a != v && b != v && all[a][v] + all[v][b] == all[a][b] {
                    through += 1;
                }
            }
        }
        let row = &t.rows[v];
        assert_eq!(row[3], adj[v].len() as f64, "degree");
        assert_eq!(row[6], through as f64, "betweenness of {}", ids[v]);
        if sum > 0 {
            assert!(close(row[7], 1.0 / sum as f64), "closeness of {}", ids[v]);
        }
        // above it: the links whose path down reaches it
        let up: Vec<usize> = (0..n)
            .filter(|&u| {
                let mut x = u;
                loop {
                    if x == v {
                        break true;
                    }
                    match index(below[x]) {
                        Some(j) => x = j,
                        None => break false,
                    }
                }
            })
            .collect();
        assert!(close(row[5], up.iter().map(|&u| length[u]).sum()), "upstream length");
        let sources = up.iter().filter(|&&u| !below.iter().any(|b| *b == ids[u])).count();
        assert_eq!(row[4], sources as f64, "magnitude");
    }
}

#[test]
fn a_dam_splits_the_river_and_its_removal_is_ranked() {
    let dem = filled("oregon");
    let n = dem.data.len();
    let nx = dem.grid.width;
    let fm = || Value::Map(dem.clone());
    let (acc, streams, links) = network(&dem);
    let lt = table(&links, "table");
    let a = data(&acc, "map");
    let s = data(&streams, "map");
    // the largest network, and two dams on it: on the last cells of two of its links, one impassable
    let outlet = (0..n).filter(|&k| s[k] == 1.0).max_by(|i, j| a[*i].total_cmp(&a[*j])).unwrap();
    let (er, ec) = (col(&lt, "end_row"), col(&lt, "end_col"));
    let mut ends: Vec<usize> = (0..lt.rows.len()).map(|i| er[i] as usize * nx + ec[i] as usize).collect();
    ends.retain(|&k| k != outlet && a[k] > 40.0);
    let (len, to) = steps(&dem);
    let drains_to = |mut k: usize, t: usize| loop {
        if k == t {
            break true;
        }
        if to[k] < 0 {
            break false;
        }
        k = to[k] as usize;
    };
    ends.retain(|&k| drains_to(k, outlet));
    ends.sort_by(|i, j| a[*j].total_cmp(&a[*i]));
    let dams = [ends[0], ends[ends.len() / 2]];
    let ps: Vec<Xy> = dams.iter().map(|&k| centre(&dem.grid, (k / nx) as f64, (k % nx) as f64)).collect();
    let barriers = Value::Vector(Arc::new(Vector {
        geometries: ps.iter().map(|p| Geometry::Point(*p)).collect(),
        attributes: vec![("passability".into(), Column::Numbers(vec![0.0, 0.5]))],
        crs: None,
    }));
    let out = run(
        "hydro.dci",
        vec![("dem", fm()), ("streams", streams["map"].clone()), ("barriers", barriers)],
        serde_json::json!({}),
    );
    let seg = data(&out, "segments");
    // the segments' lengths, as the stream cells' steps
    let mut weight = std::collections::HashMap::new();
    for k in 0..n {
        if seg[k] > 0.0 {
            *weight.entry(seg[k] as usize).or_insert(0.0) += len[k];
        }
    }
    let (w1, w2) = (weight[&1], weight[&2]);
    let s0 = seg[outlet] as usize;
    let w0 = weight[&s0];
    let total = w0 + w1 + w2;
    let t = table(&out, "table");
    let row = t.rows.iter().find(|r| r[0] == s0 as f64).unwrap();
    assert_eq!(row[2], 2.0, "two barriers");
    assert!(close(row[3], total));
    // c between segments: 0 across the first dam, 0.25 (0.5 up times 0.5 down) across the second
    let c = |i: usize, j: usize| -> f64 {
        if i == j {
            return 1.0;
        }
        let path = |x: usize| -> Vec<usize> {
            if x == 0 {
                vec![0]
            } else if x == 1 && drains_to(dams[0], dams[1]) {
                vec![1, 2, 0]
            } else if x == 2 && drains_to(dams[1], dams[0]) {
                vec![2, 1, 0]
            } else {
                vec![x, 0]
            }
        };
        let (pi, pj) = (path(i), path(j));
        let meet = pi.iter().find(|x| pj.contains(x)).unwrap();
        let factor = |x: usize| if x == 1 { 0.0 } else { 0.25 };
        pi.iter().take_while(|x| *x != meet).chain(pj.iter().take_while(|x| *x != meet)).map(|&x| factor(x)).product()
    };
    let ws = [w0, w1, w2];
    let want: f64 = (0..3).flat_map(|i| (0..3).map(move |j| (i, j))).map(|(i, j)| ws[i] * ws[j] * c(i, j)).sum::<f64>()
        / (total * total)
        * 100.0;
    assert!(close(row[4], want), "dci_p {} {want}", row[4]);
    let Value::Vector(b) = &out["barriers"] else { panic!("barriers") };
    let rank = match &b.attributes.iter().find(|(n, _)| n == "rank").unwrap().1 {
        Column::Numbers(x) => x.clone(),
        _ => panic!(),
    };
    let gain = match &b.attributes.iter().find(|(n, _)| n == "d_index").unwrap().1 {
        Column::Numbers(x) => x.clone(),
        _ => panic!(),
    };
    assert!(gain.iter().all(|g| *g > 0.0), "{gain:?}");
    assert_eq!(rank[0] < rank[1], gain[0] > gain[1]);
    // the reach of the mouth's segment
    let r = data(&out, "reach");
    let reach0: f64 = (0..3).map(|j| c(0, j) * ws[j]).sum::<f64>() / total * 100.0;
    assert!(close(r[outlet], reach0) && close(row[5], reach0), "{} {} {reach0}", r[outlet], row[5]);
}
