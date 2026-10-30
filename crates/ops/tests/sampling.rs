//! Landscape metrics of sample plots against landscapemetrics' sample_lsm and scale_sample (the plots and results
//! landschaft keeps in extern/landschaft/tests/data/reference/lsm_sample), the correlation of metrics against its
//! calculate_correlation (reference/lsm_utils), and the series and grid zones against landschaft
//! (tests/data/reference/lsm_series.json, tools/make_lsm_reference.py).

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_ops::lsm_catalog::METRICS;
use terraflux_ops::value::{Key, Stack};
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

/// Tolerances and bits of landschaft's own comparison (tests/test_lsm_reference.py).
const RTOL: f64 = 1e-10;
const ATOL: f64 = 1e-14;
const ULPS: u64 = 128;
const NOT_BITWISE: [&str; 3] = ["circle", "gyrate", "enn"];

fn landschaft() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extern/landschaft/tests/data")
}

fn read_map(path: &Path) -> Map {
    let r = terraflux_raster::Raster::read(path).unwrap();
    Map::new(Arc::new(r.grid.clone()), r.bands[0].masked(r.nodata))
}

fn gunzip(path: &Path) -> String {
    let mut text = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_string(&mut text).unwrap();
    text
}

fn same_bits(a: f64, b: f64) -> bool {
    if a.is_nan() || b.is_nan() {
        return a.is_nan() && b.is_nan();
    }
    a == b || (a.signum() == b.signum() && a.to_bits().abs_diff(b.to_bits()) <= ULPS)
}

fn close(a: f64, b: f64, rtol: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b || (a - b).abs() <= ATOL + rtol * b.abs()
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
}

fn table(out: &Outputs, port: &str) -> Table {
    let Value::Table(t) = &out[port] else { panic!("{port} is no table") };
    t.clone()
}

/// A CSV line's fields, quoted ones whole.
fn fields(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// The polygons of a WKT POLYGON or MULTIPOLYGON.
fn wkt(text: &str) -> Geometry {
    let body = &text[text.find('(').unwrap()..];
    // the rings: runs of "x y" pairs between the innermost parentheses
    let mut polygons: Vec<Vec<Vec<Xy>>> = Vec::new();
    let (mut depth, mut ring) = (0, String::new());
    let multi = text.trim_start().starts_with("MULTIPOLYGON");
    let base = if multi { 2 } else { 1 };
    for ch in body.chars() {
        match ch {
            '(' => {
                depth += 1;
                if depth == base {
                    polygons.push(Vec::new());
                }
            }
            ')' => {
                if depth == base + 1 {
                    let pts = ring
                        .split(',')
                        .map(|p| {
                            let v: Vec<f64> = p.split_whitespace().map(|x| x.parse().unwrap()).collect();
                            [v[0], v[1]]
                        })
                        .collect();
                    polygons.last_mut().unwrap().push(pts);
                    ring.clear();
                }
                depth -= 1;
            }
            c if depth == base + 1 => ring.push(c),
            _ => {}
        }
    }
    if multi { Geometry::MultiPolygon(polygons) } else { Geometry::Polygon(polygons.remove(0)) }
}

/// A case's plots: points (x, y) or polygons (wkt), with their plot_id when given.
fn plots(path: &Path, crs: Option<String>) -> Vector {
    let text = std::fs::read_to_string(path).unwrap();
    let mut lines = text.lines();
    let header = fields(lines.next().unwrap());
    let col = |n: &str| header.iter().position(|h| h == n);
    let (mut geometries, mut ids) = (Vec::new(), Vec::new());
    for line in lines.filter(|l| !l.is_empty()) {
        let f = fields(line);
        geometries.push(match col("wkt") {
            Some(j) => wkt(&f[j]),
            None => Geometry::Point([f[col("x").unwrap()].parse().unwrap(), f[col("y").unwrap()].parse().unwrap()]),
        });
        if let Some(j) = col("plot_id") {
            ids.push(f[j].parse::<f64>().unwrap());
        }
    }
    let attributes = if ids.is_empty() { Vec::new() } else { vec![("plot_id".to_string(), Column::Numbers(ids))] };
    Vector { geometries, attributes, crs }
}

/// (level, class, patch, metric, plot, size)
type Key6 = (String, Option<i64>, Option<i64>, String, i64, Option<i64>);

fn opt_int(s: &str) -> Option<i64> {
    if s.is_empty() || s == "NA" { None } else { Some(s.parse::<f64>().unwrap() as i64) }
}

fn of_nan(v: f64) -> Option<i64> {
    if v.is_nan() { None } else { Some(v as i64) }
}

/// Every landscapemetrics metric landschaft has, at every level, on the plots landschaft keeps: the same rows,
/// values (to the bit, but for the metrics R gives at random) and shares of the plots inside the map.
#[test]
fn samples_are_landscapemetrics() {
    let dir = landschaft().join("reference/lsm_sample");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    let mut cases = 0;
    for (fname, info) in manifest["files"].as_object().unwrap() {
        let map = read_map(&landschaft().join("lsm").join(info["input"].as_str().unwrap()));
        let mut params = serde_json::Map::new();
        let mut vector = false;
        for (k, v) in info["options"].as_object().unwrap() {
            match k.as_str() {
                "sizes" => {
                    let s: Vec<f64> = v.as_str().unwrap().split(',').map(|x| x.parse().unwrap()).collect();
                    params.insert("size".into(), serde_json::json!(s));
                }
                "vector" => vector = v.as_bool().unwrap(),
                "level" => {}
                _ => {
                    params.insert(k.clone(), v.clone());
                }
            }
        }
        // the map's coordinate system as GDAL reads it (a projection of its own: no EPSG code)
        let path = landschaft().join("lsm").join(info["input"].as_str().unwrap());
        let crs = if vector { terraflux_gdal::Dataset::open(&path).unwrap().meta.grid.geo.wkt.clone() } else { None };
        assert!(!vector || crs.is_some(), "{fname}: the map's coordinate system");
        let v = plots(&dir.join(info["plots"].as_str().unwrap()), crs);
        if v.column("plot_id").is_some() {
            params.insert("id_column".into(), "plot_id".into());
        }
        let several = params.get("size").is_some_and(|s| s.is_array());
        // R's rows: only the metrics landschaft has
        let text = gunzip(&dir.join(fname));
        let mut lines = text.lines();
        let header = fields(lines.next().unwrap());
        let col = |n: &str| header.iter().position(|h| h == n).unwrap();
        let mut theirs: BTreeMap<Key6, (f64, f64)> = BTreeMap::new();
        for line in lines.filter(|l| !l.is_empty()) {
            let f = fields(line);
            let (level, metric) = (f[col("level")].clone(), f[col("metric")].clone());
            if !METRICS.iter().any(|m| !m.extra && m.level == level && m.metric == metric) {
                continue;
            }
            let value = if f[col("value")] == "NA" || f[col("value")].is_empty() {
                f64::NAN
            } else {
                f[col("value")].parse().unwrap()
            };
            let size = if several { Some(f[col("size")].parse::<f64>().unwrap() as i64) } else { None };
            let key = (
                level,
                opt_int(&f[col("class")]),
                opt_int(&f[col("id")]),
                metric,
                opt_int(&f[col("plot_id")]).unwrap(),
                size,
            );
            theirs.insert(key, (value, f[col("percentage_inside")].parse().unwrap()));
        }
        let levels: Vec<String> = {
            let mut l: Vec<String> = theirs.keys().map(|k| k.0.clone()).collect();
            l.dedup();
            l.sort();
            l.dedup();
            l
        };
        let mut ours: BTreeMap<Key6, (f64, f64)> = BTreeMap::new();
        for level in &levels {
            let mut p = params.clone();
            p.insert("level".into(), level.clone().into());
            let out = run(
                "metrics.sample",
                vec![("map", Value::Map(map.clone())), ("plots", Value::Vector(Arc::new(v.clone())))],
                serde_json::Value::Object(p),
            );
            let t = table(&out, "table");
            let c = |n: &str| t.columns.iter().position(|h| h == n);
            let first = c("percentage_inside").unwrap() + 1;
            let mut previous = f64::NEG_INFINITY;
            for r in &t.rows {
                assert!(r[0] >= previous, "{fname}: the plots in order");
                previous = r[0];
                for (j, name) in t.columns.iter().enumerate().skip(first) {
                    let key = (
                        level.clone(),
                        c("class").and_then(|k| of_nan(r[k])),
                        c("patch").and_then(|k| of_nan(r[k])),
                        name.clone(),
                        r[0] as i64,
                        c("size").map(|k| r[k] as i64),
                    );
                    ours.insert(key, (r[j], r[first - 1]));
                }
            }
        }
        // circle is not measured on cells that are not square
        if !theirs.keys().any(|k| k.3.starts_with("circle")) {
            ours.retain(|k, _| !k.3.starts_with("circle"));
        }
        let only_ours: Vec<_> = ours.keys().filter(|k| !theirs.contains_key(*k)).take(5).collect();
        let only_theirs: Vec<_> = theirs.keys().filter(|k| !ours.contains_key(*k)).take(5).collect();
        assert!(
            only_ours.is_empty() && only_theirs.is_empty(),
            "{fname}: ours only {only_ours:?}, R's only {only_theirs:?}"
        );
        let wrong: Vec<_> = ours
            .iter()
            .filter(|(k, (v, p))| {
                let (tv, tp) = theirs[*k];
                !close(*v, tv, RTOL)
                    || !close(*p, tp, 1e-9)
                    || (!NOT_BITWISE.iter().any(|m| k.3.starts_with(m)) && !same_bits(*v, tv))
            })
            .map(|(k, v)| (k, *v, theirs[k]))
            .take(5)
            .collect();
        assert!(wrong.is_empty(), "{fname}: {wrong:?}");
        cases += 1;
    }
    assert_eq!(cases, 9);
}

/// calculate_correlation's lower triangles (column by column, as R) of patch, class and landscape metrics.
#[test]
fn correlations_are_landscapemetrics() {
    let dir = landschaft().join("reference/lsm_utils");
    // (its neighbourhood has NaN, which JSON has not)
    let text = std::fs::read_to_string(dir.join("manifest.json")).unwrap().replace("NaN", "null");
    let manifest: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut compared = 0;
    for (case, info) in manifest["files"].as_object().unwrap() {
        let map = read_map(&landschaft().join("lsm").join(info["input"].as_str().unwrap()));
        let mut tables = Vec::new();
        for (op, metrics) in [
            ("metrics.patch", vec!["area", "perim", "shape", "frac", "contig", "enn"]),
            ("metrics.class", vec!["ca", "np", "pland", "te", "lpi", "enn_mn"]),
        ] {
            let mut p = info["options"].clone();
            p["metrics"] = serde_json::json!(metrics);
            let t = table(&run(op, vec![("map", Value::Map(map.clone()))], p), "table");
            tables.push((if op == "metrics.patch" { "patch" } else { "class" }, t));
        }
        if case == "augusta" {
            // the landscape metrics of four maps, a row each
            let mut rows = Vec::new();
            let mut columns = Vec::new();
            let names = ["augusta.tif"]
                .into_iter()
                .chain(manifest["stack"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()));
            for name in names {
                let m = read_map(&landschaft().join("lsm").join(name));
                let p = serde_json::json!({"metrics": ["ta", "np", "shdi", "contag", "ed", "area_mn"]});
                let t = table(&run("metrics.landscape", vec![("map", Value::Map(m))], p), "table");
                columns = t.columns;
                rows.extend(t.rows);
            }
            tables.push(("landscape", Table { columns, rows, ..Default::default() }));
        }
        for (level, t) in &tables {
            for method in ["pearson", "spearman", "kendall"] {
                let out = run(
                    "metrics.correlation",
                    vec![("table", Value::Table(t.clone()))],
                    serde_json::json!({"method": method}),
                );
                let cor = table(&out, "table");
                let at = |a: &str, b: &str| {
                    let i = cor.columns.iter().position(|c| c == a).unwrap();
                    let j = cor.columns.iter().position(|c| c == b).unwrap();
                    cor.rows[i][j]
                };
                let text = gunzip(&dir.join(case).join(format!("correlation_{method}_{level}.csv.gz")));
                let mut n = 0;
                for line in text.lines().skip(1).filter(|l| !l.is_empty()) {
                    let f = fields(line);
                    let want: f64 = f[2].parse().unwrap();
                    let got = at(&f[0], &f[1]);
                    assert!(same_bits(got, want), "{case} {method} {level} {} {}: {got} against {want}", f[0], f[1]);
                    n += 1;
                }
                // and no correlation R leaves out (NA)
                let m = cor.columns.len();
                let ours = (0..m)
                    .flat_map(|i| (0..=i).map(move |j| (i, j)))
                    .filter(|&(i, j)| !cor.rows[i][j].is_nan())
                    .count();
                assert_eq!(ours, n, "{case} {method} {level}");
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 8 * 6 + 3);
}

fn series_reference() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/lsm_series.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn num(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

/// grid_zones' squares, every coordinate.
#[test]
fn grid_zones_are_landschafts() {
    let map = read_map(&landschaft().join("lsm/augusta.tif"));
    for case in series_reference()["grid_zones"].as_array().unwrap() {
        let out =
            run("vector.grid_zones", vec![("map", Value::Map(map.clone()))], serde_json::json!({"size": case["size"]}));
        let Value::Vector(v) = &out["zones"] else { panic!() };
        let want = case["zones"].as_array().unwrap();
        assert_eq!(v.len(), want.len());
        for (g, w) in v.geometries.iter().zip(want) {
            let Geometry::Polygon(rings) = g else { panic!() };
            let w: Vec<Xy> = w.as_array().unwrap().iter().map(|p| [num(&p[0]), num(&p[1])]).collect();
            assert_eq!(rings[0], w);
        }
    }
}

/// SpatioTemporalAnalysis' tables of landscape and class metrics, of the maps and of a grid of zones.
#[test]
fn series_are_landschafts() {
    let reference = series_reference();
    let s = &reference["series"];
    let a = read_map(&landschaft().join("lsm/augusta.tif"));
    // the change of tools/make_lsm_reference.py
    let nx = a.grid.width;
    let b: Vec<f64> = a
        .data
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let (r, c) = (i / nx, i % nx);
            if r < 60 && c < 80 && v == 42.0 {
                41.0
            } else if v == 82.0 {
                81.0
            } else {
                v
            }
        })
        .collect();
    let stack = Stack {
        grid: a.grid.clone(),
        keys: vec![Key::Name("2001".into()), Key::Name("2011".into())],
        layers: vec![Arc::new(a.data.to_vec()), Arc::new(b)],
    };
    let rows = |k: &str| -> Vec<Vec<f64>> {
        s[k].as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(num).collect()).collect()
    };
    let cases = [
        ("landscape", serde_json::json!({}), "landscape"),
        ("class", serde_json::json!({}), "class"),
        ("class", serde_json::json!({"fillna": false}), "class_na"),
        ("landscape", serde_json::json!({"grid": s["grid"]}), "zonal_landscape"),
        ("class", serde_json::json!({"grid": s["grid"]}), "zonal_class"),
    ];
    for (level, extra, key) in cases {
        let mut p = extra.clone();
        p["level"] = level.into();
        p["metrics"] = s[format!("{level}_metrics")].clone();
        let out = run("metrics.series", vec![("maps", Value::Stack(stack.clone()))], p);
        let t = table(&out, "table");
        let want = rows(key);
        let mut ours = t.rows.clone();
        let mut theirs = want.clone();
        let keys = t
            .columns
            .iter()
            .take_while(|c| !s[format!("{level}_metrics")].as_array().unwrap().iter().any(|m| m == *c))
            .count();
        let order = |x: &Vec<f64>, y: &Vec<f64>| x[..keys].partial_cmp(&y[..keys]).unwrap();
        ours.sort_by(order);
        theirs.sort_by(order);
        assert_eq!(ours.len(), theirs.len(), "{key}: rows");
        for (o, w) in ours.iter().zip(&theirs) {
            assert_eq!(o.len(), w.len(), "{key}");
            for (x, y) in o.iter().zip(w) {
                assert!((x.is_nan() && y.is_nan()) || x == y, "{key}: {o:?} against {w:?}");
            }
        }
    }
}
