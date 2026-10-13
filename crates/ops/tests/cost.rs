//! Grid distances against landschaft's (gdistance's; tests/data/reference/cost.npz and cost.json,
//! tools/make_cost_reference.py): the transitions of every built-in function in 4, 8 and 16 directions,
//! symmetric and not, their geoCorrection, the accumulated cost, the least costs and the least-cost paths on a
//! projected grid, a longitude/latitude one and a global one that wraps around, to the bit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_ops::files::Rasters;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::npy::Array;
use terraflux_raster::vector::{Geometry, Vector};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn open(p: PathBuf) -> Map {
    Rasters::open(p.to_str().unwrap(), None, None).unwrap().map().unwrap()
}

fn example(name: &str) -> Map {
    open(root().join("extern/landschaft/src/landschaft/cost/data").join(format!("{name}.tif")))
}

fn points(xy: &serde_json::Value, crs: Option<String>) -> Value {
    let xy: Vec<[f64; 2]> = serde_json::from_value(xy.clone()).unwrap();
    Value::Vector(Arc::new(Vector {
        geometries: xy.into_iter().map(Geometry::Point).collect(),
        attributes: Vec::new(),
        crs,
    }))
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn check(what: &str, got: &[f64], want: &Array) {
    assert_eq!(got.len(), want.data.len(), "{what}: length");
    let bad = got.iter().zip(&want.data).position(|(a, b)| !same(*a, *b));
    if let Some(k) = bad {
        panic!("{what}: at {k}: {} {}", got[k], want.data[k]);
    }
}

fn layers(v: &Value) -> Vec<f64> {
    let Value::Stack(s) = v else { panic!("not a stack") };
    s.layers.iter().flat_map(|l| l.iter().copied()).collect()
}

fn map(v: &Value) -> Vec<f64> {
    let Value::Map(m) = v else { panic!("not a map") };
    m.data.to_vec()
}

fn table(v: &Value) -> Vec<f64> {
    let Value::Table(t) = v else { panic!("not a table") };
    t.rows.iter().flat_map(|r| r[1..].to_vec()).collect()
}

/// The cells a path's line goes through (its vertices are the cells' centres).
fn path_cells(v: &Value, k: usize, m: &Map) -> Vec<f64> {
    let Value::Vector(v) = v else { panic!("not a vector") };
    let g = &m.grid;
    let t = g.transform;
    match &v.geometries[k] {
        Geometry::LineString(p) => p
            .iter()
            .map(|[x, y]| {
                let (c, r) = (((x - t[0]) / t[1]).floor(), ((y - t[3]) / t[5]).floor());
                r * g.width as f64 + c
            })
            .collect(),
        Geometry::Empty => Vec::new(),
        g => panic!("{g:?}"),
    }
}

fn transition(m: &Map, f: &str, d: i64, s: bool) -> Value {
    run(
        "cost.transition",
        vec![("map", Value::Map(m.clone()))],
        serde_json::json!({"function": f, "directions": d, "symmetric": s}),
    )
    .remove("transition")
    .unwrap()
}

fn conductance(m: &Map, f: &str, d: i64, s: bool) -> Value {
    run(
        "cost.transition",
        vec![("map", Value::Map(m.clone()))],
        serde_json::json!({"function": f, "directions": d, "symmetric": s, "reciprocal": true}),
    )
    .remove("transition")
    .unwrap()
}

fn geo(t: &Value, kind: &str, scale: bool) -> Value {
    run("cost.geo_correction", vec![("transition", t.clone())], serde_json::json!({"type": kind, "scale": scale}))
        .remove("transition")
        .unwrap()
}

/// The accumulated cost, the least costs and the paths over a transition.
fn distances(
    z: &BTreeMap<String, Array>,
    prefix: &str,
    tr: &Value,
    m: &Map,
    pts: &serde_json::Value,
    to: Option<&serde_json::Value>,
    crs: Option<String>,
) {
    let all = pts.as_array().unwrap();
    let n = if prefix.starts_with('e') {
        2
    } else if prefix.starts_with('g') {
        1
    } else {
        3
    };
    let first = serde_json::Value::Array(all[..n].to_vec());
    let acc = run(
        "cost.acc_cost",
        vec![("transition", tr.clone()), ("origins", points(&first, crs.clone()))],
        serde_json::json!({}),
    );
    let key = if prefix.starts_with('m') { format!("{prefix}/acc/points") } else { format!("{prefix}/acc") };
    check(&key, &map(&acc["map"]), &z[&key]);
    if prefix.starts_with('m') {
        let mask = Map::new(m.grid.clone(), z["m/mask"].data.clone());
        let acc = run(
            "cost.acc_cost",
            vec![("transition", tr.clone()), ("origins", Value::Map(mask))],
            serde_json::json!({}),
        );
        check(&format!("{prefix}/acc/mask"), &map(&acc["map"]), &z[&format!("{prefix}/acc/mask")]);
    }
    let d = run(
        "cost.distance",
        vec![("transition", tr.clone()), ("from", points(pts, crs.clone()))],
        serde_json::json!({}),
    );
    check(&format!("{prefix}/dist"), &table(&d["table"]), &z[&format!("{prefix}/dist")]);
    if let Some(to) = to {
        let d = run(
            "cost.distance",
            vec![("transition", tr.clone()), ("from", points(pts, crs.clone())), ("to", points(to, crs.clone()))],
            serde_json::json!({}),
        );
        check(&format!("{prefix}/dist_to"), &table(&d["table"]), &z[&format!("{prefix}/dist_to")]);
    }
    let origin = serde_json::Value::Array(all[..1].to_vec());
    let goals = serde_json::Value::Array(all[1..].to_vec());
    let p = run(
        "cost.shortest_path",
        vec![("transition", tr.clone()), ("origin", points(&origin, crs.clone())), ("goals", points(&goals, crs))],
        serde_json::json!({}),
    );
    for k in 0..all.len() - 1 {
        let key = format!("{prefix}/path/{k}");
        check(&key, &path_cells(&p["lines"], k, m), &z[&key]);
    }
    // the share of the paths through each cell
    let share = map(&p["map"]);
    let paths: Vec<&Array> = (0..all.len() - 1).map(|k| &z[&format!("{prefix}/path/{k}")]).collect();
    for (c, s) in share.iter().enumerate() {
        let n = paths.iter().filter(|p| p.data.contains(&(c as f64))).count();
        assert!((s - n as f64 / paths.len() as f64).abs() < 1e-12, "{prefix} share at {c}");
    }
}

#[test]
fn transitions_and_distances_are_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/cost.npz")).unwrap();
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/cost.json")).unwrap()).unwrap();

    // maungawhau: projected
    let m = example("maungawhau");
    for f in ["mean", "min", "max", "sum", "prod", "from", "to", "diff", "harmonic", "geometric"] {
        check(&format!("m/tr/{f}/8/1"), &layers(&transition(&m, f, 8, true)), &z[&format!("m/tr/{f}/8/1")]);
    }
    for d in [4, 16] {
        check(&format!("m/tr/mean/{d}/1"), &layers(&transition(&m, "mean", d, true)), &z[&format!("m/tr/mean/{d}/1")]);
    }
    for d in [4, 8, 16] {
        for f in ["mean", "diff"] {
            let key = format!("m/tr/{f}/{d}/0");
            check(&key, &layers(&transition(&m, f, d, false)), &z[&key]);
        }
    }
    check("m/recip/mean/8/1", &layers(&conductance(&m, "mean", 8, true)), &z["m/recip/mean/8/1"]);
    for (d, s) in [(8, true), (16, false)] {
        let tr = conductance(&m, "mean", d, s);
        for kind in ["c", "r"] {
            for scale in [false, true] {
                let key = format!("m/geo/{d}/{}/{kind}/{}", u8::from(s), u8::from(scale));
                check(&key, &layers(&geo(&tr, kind, scale)), &z[&key]);
            }
        }
    }
    let mc = &cases["maungawhau"];
    let crs = terraflux_ops::prepare::crs_of(&m.grid);
    let sym = geo(&conductance(&m, "mean", 8, true), "c", false);
    distances(&z, "m/sym", &sym, &m, &mc["points"], Some(&mc["to"]), crs.clone());
    let asym = geo(&conductance(&m, "from", 16, false), "c", false);
    distances(&z, "m/asym", &asym, &m, &mc["points"], Some(&mc["to"]), crs);

    // europe: longitude/latitude
    let e = example("europe");
    let tr = conductance(&e, "mean", 8, true);
    for kind in ["c", "r"] {
        check(&format!("e/geo/{kind}"), &layers(&geo(&tr, kind, false)), &z[&format!("e/geo/{kind}")]);
    }
    distances(&z, "e", &geo(&tr, "c", false), &e, &cases["europe"]["points"], None, None);

    // a global grid: the columns wrap around
    let g = open(root().join("tests/data/reference/cost_global.tif"));
    let tr = transition(&g, "mean", 8, true);
    check("g/tr", &layers(&tr), &z["g/tr"]);
    let gg = geo(&conductance(&g, "mean", 8, true), "c", false);
    check("g/geo", &layers(&gg), &z["g/geo"]);
    distances(&z, "g", &gg, &g, &cases["global"]["points"], None, None);
}

/// A transition's stack carries its layout: geo_correction refuses any other stack.
#[test]
fn a_transition_is_not_any_stack() {
    let m = example("maungawhau");
    let tr = transition(&m, "mean", 4, true);
    let Value::Stack(s) = &tr else { panic!() };
    assert_eq!(s.layers.len(), 2);
    let mut plain = s.clone();
    let mut grid = (*plain.grid).clone();
    grid.geo.set_item(terraflux_ops::cost::ITEM, None);
    plain.grid = Arc::new(grid);
    let r = Registry::standard();
    let params = serde_json::Map::new();
    let args = Args {
        node: "n",
        inputs: [("transition".to_string(), Value::Stack(plain))].into_iter().collect(),
        params: &params,
    };
    assert!(r.get("cost.geo_correction").unwrap().run(&mut Ctx::default(), &args).is_err());
}

fn transition_of(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Vec<f64> {
    layers(&run(op, inputs, params)["transition"])
}

/// Random walks and the matrix's algebra against landschaft's (tests/data/reference/cost_walk.npz): commute
/// distances (one part, and europe's islands), passages with and without theta, randomised-shortest-path
/// distances, normalize, sumReciprocal, sums of transitions, R's maths and the summaries of cells, to the bit.
#[test]
fn random_walks_are_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/cost_walk.npz")).unwrap();
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/cost.json")).unwrap()).unwrap();
    let m = example("maungawhau");
    let crs = terraflux_ops::prepare::crs_of(&m.grid);
    let mc = &cases["maungawhau"];
    let all = mc["points"].as_array().unwrap();
    let pts = |r: std::ops::Range<usize>| points(&serde_json::Value::Array(all[r].to_vec()), crs.clone());
    let sym = geo(&conductance(&m, "mean", 8, true), "r", false);
    let asym = geo(&conductance(&m, "from", 16, false), "r", false);

    let c = run(
        "cost.commute_distance",
        vec![("transition", sym.clone()), ("points", pts(0..all.len()))],
        serde_json::json!({}),
    );
    check("m/commute", &table(&c["table"]), &z["m/commute"]);
    for compat in [true, false] {
        let p = run(
            "cost.passage",
            vec![("transition", sym.clone()), ("origins", pts(0..1)), ("goals", pts(1..2))],
            serde_json::json!({"gdistance_compat": compat}),
        );
        let key = format!("m/passage/{}", u8::from(compat));
        check(&key, &map(&p["map"]), &z[&key]);
    }
    for (name, tr) in [("sym", &sym), ("asym", &asym)] {
        for theta in [0.001, 0.01] {
            for tn in ["net", "total"] {
                let p = run(
                    "cost.passage",
                    vec![("transition", tr.clone()), ("origins", pts(0..1)), ("goals", pts(1..3))],
                    serde_json::json!({"theta": theta, "total_net": tn}),
                );
                let key = format!("m/{name}/rsp_passage/{theta}/{tn}");
                check(&key, &map(&p["map"]), &z[&key]);
            }
        }
        for method in [1, 2] {
            for tn in ["net", "total"] {
                let d = run(
                    "cost.rsp_distance",
                    vec![
                        ("transition", tr.clone()),
                        ("from", pts(0..all.len())),
                        ("to", points(&mc["to"], crs.clone())),
                    ],
                    serde_json::json!({"theta": 0.001, "total_net": tn, "method": method}),
                );
                let key = format!("m/{name}/rsp/{method}/{tn}");
                if method == 1 {
                    check(&key, &table(&d["table"]), &z[&key]);
                } else {
                    // method 2 goes through Eigen's SparseLU, whose last bits change from run to run in landschaft
                    // itself (its vectorised kernels follow the memory's alignment)
                    let got = table(&d["table"]);
                    for (a, b) in got.iter().zip(&z[&key].data) {
                        assert!(same(*a, *b) || (a - b).abs() <= 1e-12 * b.abs(), "{key}: {a} {b}");
                    }
                }
            }
        }
        for method in ["row", "col", "symm"] {
            let key = format!("m/{name}/normalize/{method}");
            check(
                &key,
                &transition_of(
                    "cost.normalize",
                    vec![("transition", tr.clone())],
                    serde_json::json!({"method": method}),
                ),
                &z[&key],
            );
        }
        for reduce in ["NZcolMeans", "NZrowMeans", "colSums", "rowSums", "colMeans", "rowMeans"] {
            let r = run("cost.to_map", vec![("transition", tr.clone())], serde_json::json!({"reduce": reduce}));
            let key = format!("m/{name}/to_map/{reduce}");
            check(&key, &map(&r["map"]), &z[&key]);
        }
    }
    let d = run(
        "cost.rsp_distance",
        vec![("transition", sym.clone()), ("from", pts(0..3))],
        serde_json::json!({"theta": 0.001}),
    );
    check("m/rsp_same", &table(&d["table"]), &z["m/rsp_same"]);

    let (a, b) = (conductance(&m, "mean", 8, true), conductance(&m, "max", 4, false));
    let ab = || vec![("a", a.clone()), ("b", b.clone())];
    check(
        "m/sum_reciprocal",
        &transition_of("cost.sum_reciprocal", ab(), serde_json::json!({})),
        &z["m/sum_reciprocal"],
    );
    for op in ["sum", "difference", "product"] {
        let key = format!("m/combine/{op}");
        check(&key, &transition_of("cost.combine", ab(), serde_json::json!({"operation": op})), &z[&key]);
    }
    for (f, v) in [("sqrt", 1.0), ("exp", 1.0), ("log", 1.0), ("pow", 2.5), ("multiply", 3.0), ("divide", 2.0)] {
        let key = format!("m/math/{f}");
        let got =
            transition_of("cost.math", vec![("transition", a.clone())], serde_json::json!({"function": f, "value": v}));
        check(&key, &got, &z[&key]);
    }

    // europe: separate parts (islands), each with its volume
    let e = example("europe");
    let tr = geo(&conductance(&e, "mean", 8, true), "r", false);
    let c = run(
        "cost.commute_distance",
        vec![("transition", tr), ("points", points(&cases["europe"]["points"], None))],
        serde_json::json!({}),
    );
    check("e/commute", &table(&c["table"]), &z["e/commute"]);
}
