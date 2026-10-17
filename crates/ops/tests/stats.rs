//! Statistics of maps against scipy, pymannkendall 1.4 and PySAL's esda 2.10 (tests/data/reference/stats.npz,
//! tools/make_stats_reference.py): scipy's ndtr and the Mann-Kendall test with Sen's slope to the bit; esda's
//! global and local Moran's I and Getis and Ord's Gi* of a field, by rook and by queen, to rounding.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::special::ndtr;
use terraflux_ops::stats::{getis_ord, global_moran, lattice, local_moran, mann_kendall};
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;

fn reference() -> std::collections::BTreeMap<String, Array> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    terraflux_raster::npy::read_npz(dir.join("stats.npz")).unwrap()
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn exact(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if let Some(k) = got.iter().zip(want).position(|(a, b)| a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan())) {
        panic!("{what}: at {k}: {} {}", got[k], want[k]);
    }
}

fn close(what: &str, got: &[f64], want: &[f64], tol: f64) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        assert!((a - b).abs() <= tol * b.abs().max(1.0), "{what}: at {k}: {a} {b}");
    }
}

fn map(v: &Value) -> &Map {
    let Value::Map(m) = v else { panic!("{v:?}") };
    m
}

fn field(z: &std::collections::BTreeMap<String, Array>) -> Map {
    let f = &z["field"];
    let (ny, nx) = (f.shape[0], f.shape[1]);
    Map::new(Arc::new(Grid::new(nx, ny)), f.data.clone())
}

#[test]
fn scipys_normal_distribution() {
    let z = reference();
    let got: Vec<f64> = z["ndtr/x"].data.iter().map(|x| ndtr(*x)).collect();
    exact("ndtr", &got, &z["ndtr/y"].data);
}

#[test]
fn mann_kendall_is_pymannkendalls() {
    let z = reference();
    let (series, out) = (&z["mk/series"], &z["mk/out"]);
    let t = series.shape[1];
    let cols = ["s", "var_s", "z", "p", "tau", "slope", "intercept", "h", "trend"];
    for (i, row) in series.data.chunks(t).enumerate() {
        let m = mann_kendall(row, 0.05);
        let got = [m.s, m.var_s, m.z, m.p, m.tau, m.slope, m.intercept, m.trend.abs(), m.trend];
        for (c, name) in cols.iter().enumerate() {
            let want = out.data[i * cols.len() + c];
            assert_eq!(got[c].to_bits(), want.to_bits(), "series {i} {name}: {} {want}", got[c]);
        }
    }
}

#[test]
fn the_trend_of_every_cell() {
    let z = reference();
    let (series, out) = (&z["mk/series"], &z["mk/out"]);
    let (n, t) = (series.shape[0], series.shape[1]);
    let grid = Arc::new(Grid::new(n, 1));
    let maps =
        (0..t).map(|d| Value::Map(Map::new(grid.clone(), (0..n).map(|i| series.data[i * t + d]).collect()))).collect();
    let o = run("stats.trend", vec![("maps", Value::Series(maps))], serde_json::json!({}));
    for (port, c) in [("z", 2), ("p", 3), ("tau", 4), ("slope", 5), ("intercept", 6), ("trend", 8)] {
        let want: Vec<f64> = (0..n).map(|i| out.data[i * 9 + c]).collect();
        exact(port, &map(&o[port]).data, &want);
    }
}

#[test]
fn moran_lisa_and_gi_are_esdas() {
    let z = reference();
    let f = field(&z);
    let (ny, nx) = f.shape();
    for (kind, queen) in [("rook", false), ("queen", true)] {
        let l = lattice(&f.data, ny, nx, queen);
        let g = global_moran(&f.data, &l);
        let got = [g.i, g.ei, g.vi_norm, g.z_norm, g.p_norm, g.vi_rand, g.z_rand, g.p_rand];
        close(&format!("moran {kind}"), &got, &z[format!("moran/{kind}").as_str()].data, 1e-12);

        let lm = local_moran(&f.data, &l);
        let key = |k: &str| &z[format!("lisa/{kind}/{k}").as_str()].data;
        close(&format!("lisa {kind} I"), &lm.i, key("I"), 1e-12);
        exact(&format!("lisa {kind} q"), &lm.quadrant, key("q"));
        close(&format!("lisa {kind} EI"), &lm.ei, key("EI"), 1e-12);
        close(&format!("lisa {kind} VI"), &lm.vi, key("VI"), 1e-12);

        let (gs, zs) = getis_ord(&f.data, &l);
        let key = |k: &str| &z[format!("gi/{kind}/{k}").as_str()].data;
        close(&format!("gi {kind} G"), &gs, key("G"), 1e-12);
        close(&format!("gi {kind} Z"), &zs, key("Z"), 1e-10);
        let p: Vec<f64> = zs.iter().map(|v| ndtr(-v.abs())).collect();
        close(&format!("gi {kind} p"), &p, key("p"), 1e-9);
    }
}

#[test]
fn the_operators_of_a_map() {
    let z = reference();
    let mut f = field(&z);
    Arc::make_mut(&mut f.data)[0] = f64::NAN;
    let m = Value::Map(f.clone());
    let o = run("stats.moran", vec![("map", m.clone())], serde_json::json!({"permutations": 99}));
    let Value::Table(t) = &o["table"] else { panic!() };
    assert_eq!(t.rows[0][9], 499.0);
    assert!(t.rows[0][0] > 0.5 && t.rows[0][8] == 0.01, "{:?}", t.rows[0]);
    let o = run("stats.lisa", vec![("map", m.clone())], serde_json::json!({"permutations": 99, "contiguity": "rook"}));
    let (p, c) = (map(&o["p"]), map(&o["clusters"]));
    assert!(p.data[0].is_nan() && c.data[0].is_nan());
    assert!(c.data.iter().filter(|v| **v == 1.0).count() > 20 && c.data.iter().filter(|v| **v == 3.0).count() > 20);
    let o = run("stats.getis_ord", vec![("map", m)], serde_json::json!({}));
    let s = map(&o["spots"]);
    assert!(s.data.iter().any(|v| *v >= 2.0) && s.data.iter().any(|v| *v <= -1.0));
}
