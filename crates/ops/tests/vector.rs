//! The vector operators and planning units of polygons: the polygons of landschaft's optimize cases
//! (tests/data/reference/vectors, tools/make_vector_inputs.py) read by OGR; burnt and their zones' statistics as
//! landschaft's rasterize and zonal_stats; exactextractr's sums, prioritizr's boundary and adjacency matrices and its
//! models of polygon planning units (extern/landschaft/tests/data/reference/optimize). Skipped without GDAL.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;
use terraflux_ops::{Args, Ctx, Key, Map, Registry, Stack, Value};
use terraflux_raster::npy::Array;
use terraflux_raster::vector::Vector;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gdal() -> bool {
    match terraflux_gdal::gdal() {
        Ok(_) => true,
        Err(e) => {
            eprintln!("skipped: {e}");
            false
        }
    }
}

fn run(op: &str, ins: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Outputs {
    let args = Args {
        node: "n",
        inputs: ins.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        params: params.as_object().unwrap(),
    };
    Registry::standard().get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn read(name: &str) -> Arc<Vector> {
    let path = root().join(format!("tests/data/reference/vectors/{name}.gpkg"));
    let out = run("vector.read", vec![], json!({"path": path.to_string_lossy()}));
    let Value::Vector(v) = &out["vector"] else { panic!() };
    v.clone()
}

/// The cases' grid (12 x 15 cells of 100 m from (0, 1200)) and its features f1..f5.
fn features() -> (Arc<terraflux_raster::Grid>, Value) {
    let d = terraflux_raster::npy::read_npz(root().join("tests/data/reference/optimize_inputs.npz")).unwrap();
    let f = &d["features"];
    let (ny, nx) = (f.shape[1], f.shape[2]);
    let mut grid = terraflux_raster::Grid::new(nx, ny);
    grid.transform = [0.0, 100.0, 0.0, ny as f64 * 100.0, 0.0, -100.0];
    let grid = Arc::new(grid);
    let stack = Value::Stack(Stack {
        grid: grid.clone(),
        keys: (1..=5).map(|k| Key::Name(format!("f{k}"))).collect(),
        layers: f.data.chunks(ny * nx).map(|c| Arc::new(c.to_vec())).collect(),
    });
    (grid, stack)
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) || (a == 0.0 && b == 0.0)
}

/// Read and written again: the same features (geometries, attributes, coordinate system).
#[test]
fn vectors_are_read_and_written() {
    if !gdal() {
        return;
    }
    let v = read("mixed");
    assert_eq!(v.len(), 6);
    assert!(v.crs.as_deref().is_some_and(|c| c.contains("32723")), "{:?}", v.crs);
    assert_eq!(v.column("v").unwrap().numbers(), [3.0, 5.0, 7.0, 11.0, 13.0, 17.0]);
    let dir = std::env::temp_dir().join(format!("terraflux_vector_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for ext in ["gpkg", "geojson"] {
        let path = dir.join(format!("mixed.{ext}"));
        run("vector.write", vec![("vector", Value::Vector(v.clone()))], json!({"path": path.to_string_lossy()}));
        let back = run("vector.read", vec![], json!({"path": path.to_string_lossy()}));
        let Value::Vector(w) = &back["vector"] else { panic!() };
        assert_eq!(w.geometries, v.geometries, "{ext}");
        assert_eq!(w.column("v").unwrap().numbers(), v.column("v").unwrap().numbers(), "{ext}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Polygons (the centre rule and all touched), lines and points burnt as landschaft's rasterize: an attribute or a
/// value, the later over the earlier or added up, a fill.
#[test]
fn rasterize_is_landschafts() {
    if !gdal() {
        return;
    }
    let refs = terraflux_raster::npy::read_npz(root().join("tests/data/reference/vectors_reference.npz")).unwrap();
    let (grid, _) = features();
    let like = Value::Map(Map::new(grid.clone(), vec![0.0; grid.width * grid.height]));
    let v = Value::Vector(read("mixed"));
    for (key, params) in [
        ("centre", json!({})),
        ("touched", json!({"all_touched": true})),
        ("field", json!({"field": "v"})),
        ("add", json!({"field": "v", "merge": "add", "all_touched": true})),
        ("fill", json!({"field": "v", "fill": null})),
    ] {
        let out = run("vector.rasterize", vec![("vector", v.clone()), ("like", like.clone())], params);
        let Value::Map(m) = &out["map"] else { panic!() };
        let want = &refs[&format!("rasterize/{key}")];
        let differ: Vec<usize> = (0..m.data.len()).filter(|&c| !same(m.data[c], want.data[c])).collect();
        assert!(differ.is_empty(), "{key}: cells {differ:?}");
    }
}

/// Statistics of the features in polygons as landschaft's zonal_stats (the centre rule and all touched).
#[test]
fn zonal_is_landschafts() {
    if !gdal() {
        return;
    }
    let refs = terraflux_raster::npy::read_npz(root().join("tests/data/reference/vectors_reference.npz")).unwrap();
    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/data/reference/vectors_reference.json")).unwrap(),
    )
    .unwrap();
    let (_, stack) = features();
    let stats = ["count", "sum", "mean", "min", "max", "std", "median", "majority", "unique", "percentile_90"];
    for zones in ["shapes", "pu_polygons_partial"] {
        for touched in [false, true] {
            let key = format!("zonal/{zones}/{}", if touched { "touched" } else { "centre" });
            let out = run(
                "vector.zonal",
                vec![("vector", Value::Vector(read(zones))), ("map", stack.clone())],
                json!({"stats": stats, "all_touched": touched}),
            );
            let Value::Table(t) = &out["table"] else { panic!() };
            let want = &refs[&key];
            let cols: Vec<String> = meta[&key].as_array().unwrap().iter().map(|c| c.as_str().unwrap().into()).collect();
            let (rows, ncol) = (want.shape[0], want.shape[1]);
            assert_eq!(t.rows.len(), rows, "{key}");
            for (k, c) in cols.iter().enumerate() {
                let Some(ours) = t.column(c) else { continue };
                for (r, &o) in ours.iter().enumerate() {
                    let w = want.data[r * ncol + k];
                    let ok = same(o, w) || (o - w).abs() <= 1e-12 * w.abs();
                    assert!(ok, "{key}: {c}[{r}] {o} != {w}");
                }
            }
        }
    }
}

struct Reference {
    arrays: BTreeMap<String, Array>,
    meta: serde_json::Value,
}

fn reference(group: &str) -> Reference {
    let dir = root().join("extern/landschaft/tests/data/reference/optimize");
    Reference {
        arrays: terraflux_raster::npy::read_npz(dir.join(format!("{group}.npz"))).unwrap(),
        meta: serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{group}.json"))).unwrap()).unwrap(),
    }
}

impl Reference {
    fn floats(&self, key: &str) -> Vec<f64> {
        self.arrays[key].data.clone()
    }
    fn texts(&self, key: &str) -> Vec<String> {
        self.meta["arrays"][key]["values"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect()
    }
}

/// exactextractr's sums of the features over the shapes (a triangle, a hole, a multipolygon, a concave star, one
/// smaller than a cell, one partly outside, one on the cells' edges, a rotated square), bit for bit; prioritizr's
/// boundary and adjacency matrices of polygons (terra's shared paths and perimeters, st_intersects).
#[test]
fn polygon_measures_are_prioritizrs() {
    if !gdal() {
        return;
    }
    let (_, stack) = features();
    let shapes = read("shapes");
    let out = run("vector.exact_sum", vec![("vector", Value::Vector(shapes.clone())), ("map", stack)], json!({}));
    let Value::Table(t) = &out["table"] else { panic!() };
    let want = reference("exact").floats("exact_shapes/sum");

    for (s, row) in t.rows.iter().enumerate() {
        for l in 0..5 {
            // shapes x layers, row by row
            let w = want[s * 5 + l];
            assert!(same(row[1 + l], w), "shape {s} layer {l}: {} != {w}", row[1 + l]);
        }
    }
    let m = reference("matrices");
    let partial = read("pu_polygons_partial");
    for (name, geoms) in [
        ("boundary", &partial.geometries),
        ("adjacency", &partial.geometries),
        ("shapes_boundary", &shapes.geometries),
        ("shapes_adjacency", &shapes.geometries),
    ] {
        let ours = if name.ends_with("boundary") {
            terraflux_ops::vector::boundary_matrix(geoms)
        } else {
            terraflux_ops::vector::adjacency_matrix(geoms)
        };
        let n = geoms.len();
        let mut dense = vec![0.0; n * n];
        for k in 0..ours.x.len() {
            dense[ours.i[k] * n + ours.j[k]] += ours.x[k];
        }
        let want = m.floats(&format!("matrices_polygons/{name}"));
        for i in 0..n {
            for j in 0..n {
                // symmetric: either order
                assert!(
                    same(dense[i * n + j], want[j * n + i]),
                    "{name}[{i}, {j}]: {} != {}",
                    dense[i * n + j],
                    want[j * n + i]
                );
            }
        }
    }
}

fn assert_model(name: &str, m: &terraflux_ops::milp::Model, r: &Reference) {
    let k = |s: &str| format!("{name}/model/{s}");
    let (ai, aj, ax) = (r.floats(&k("A_i")), r.floats(&k("A_j")), r.floats(&k("A_x")));
    let mut entries: Vec<(usize, usize, f64)> =
        (0..ax.len()).filter(|&e| ax[e] != 0.0).map(|e| (ai[e] as usize, aj[e] as usize, ax[e])).collect();
    entries.sort_by_key(|&(i, j, _)| (i, j));
    let mut ours = Vec::new();
    for i in 0..m.a.nrow {
        let (idx, v) = m.a.row(i);
        ours.extend(idx.iter().zip(v).map(|(&j, &x)| (i, j, x)));
    }
    assert_eq!(ours.len(), entries.len(), "{name}: entries");
    for (a, b) in ours.iter().zip(&entries) {
        assert!(a.0 == b.0 && a.1 == b.1 && a.2.to_bits() == b.2.to_bits(), "{name}: {a:?} != {b:?}");
    }
    let same = |what: &str, a: &[f64], b: &[f64]| {
        assert_eq!(a.len(), b.len(), "{name}: {what}");
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{name}: {what}[{i}] {x} != {y}");
        }
    };
    same("obj", &m.obj, &r.floats(&k("obj")));
    same("lb", &m.lb, &r.floats(&k("lb")));
    same("ub", &m.ub, &r.floats(&k("ub")));
    same("rhs", &m.rhs, &r.floats(&k("rhs")));
    assert_eq!(m.sense, r.texts(&k("sense")), "{name}: sense");
    assert_eq!(m.row_ids, r.texts(&k("row_ids")), "{name}: row ids");
    assert_eq!(m.col_ids, r.texts(&k("col_ids")), "{name}: column ids");
}

/// Planning units of polygons: prioritizr's compile() of its cases bit for bit (the amounts exactextractr's sums,
/// whole or partly covered cells; boundary penalties on the polygons' shared paths), and its optimum with
/// contiguity (polygons touching at a corner adjacent).
#[test]
fn polygon_units_are_prioritizrs() {
    if !gdal() {
        return;
    }
    let (_, stack) = features();
    let problem = |polygons: &str, params: serde_json::Value| {
        let ins: BTreeMap<String, Value> =
            [("features".to_string(), stack.clone()), ("polygons".to_string(), Value::Vector(read(polygons)))].into();
        let args = Args { node: "n", inputs: ins, params: params.as_object().unwrap() };
        terraflux_ops::prioritize::problem_of(&args).unwrap_or_else(|e| panic!("{polygons}: {e}"))
    };
    for (group, name, polygons, params) in [
        ("core", "minset_polygons", "pu_polygons", json!({"relative_targets": 0.3})),
        ("core", "minset_polygons_partial", "pu_polygons_partial", json!({"relative_targets": 0.3})),
        (
            "spatial",
            "boundary_polygons",
            "pu_polygons_partial",
            json!({"relative_targets": 0.3, "boundary_penalty": 0.2, "edge_factor": 0.75}),
        ),
    ] {
        let (m, _) = terraflux_ops::milp::compile(&problem(polygons, params)).unwrap();
        assert_model(name, &m, &reference(group));
    }
    // contiguity: the optimum, the selection one patch of polygons that touch
    let ins = vec![("features", stack.clone()), ("polygons", Value::Vector(read("pu_polygons")))];
    let out = run("prioritize.optimize", ins, json!({"relative_targets": 0.3, "contiguity": true, "gap": 0.0}));
    let Value::Table(s) = &out["summary"] else { panic!() };
    let ours = s.column("objective").unwrap()[0];
    let want = reference("spatial").floats("contiguity_polygons/objective")[0];
    assert!((ours - want).abs() <= 1e-9 * want.abs().max(1.0), "contiguity: {ours} != {want}");
}

/// Planning units of polygons with management zones (landschaft's polygon_zone_units): prioritizr's compile() of
/// its case bit for bit (a cost column per zone, two units out of `use`; every zone's own features summed over the
/// polygons exactly), and its optimum.
#[test]
fn polygon_zone_units_are_prioritizrs() {
    if !gdal() {
        return;
    }
    let (grid, _) = features();
    let zones = ["protect", "use", "restore"];
    let d = terraflux_raster::npy::read_npz(root().join("tests/data/reference/optimize_zones.npz")).unwrap();
    let zf = &d["zone_features"];
    let n = grid.width * grid.height;
    let stack = Value::Stack(Stack {
        grid: grid.clone(),
        keys: zones.iter().flat_map(|z| (1..=5).map(move |k| Key::Name(format!("{z}_f{k}")))).collect(),
        layers: zf.data[..3 * 5 * n].chunks(n).map(|c| Arc::new(c.to_vec())).collect(),
    });
    // R's matrix(c(...), ncol = 3) as features x zones
    let m = [0.2, 0.1, 0.15, 0.3, 0.1, 0.1, 0.1, 0.05, 0.2, 0.1, 0.05, 0.0, 0.1, 0.05, 0.05];
    let targets: Vec<Vec<f64>> = (0..5).map(|f| (0..3).map(|z| m[z * 5 + f]).collect()).collect();
    let params = json!({
        "cost_column": zones.map(|z| format!("cost_{z}")),
        "zones": zones,
        "feature_names": (1..=5).map(|k| format!("f{k}")).collect::<Vec<_>>(),
        "relative_targets": targets,
    });
    let ins: BTreeMap<String, Value> =
        [("zone_features".to_string(), stack.clone()), ("polygons".to_string(), Value::Vector(read("zone_polygons")))]
            .into();
    let args = Args { node: "n", inputs: ins, params: params.as_object().unwrap() };
    let p = terraflux_ops::prioritize::problem_of(&args).unwrap_or_else(|e| panic!("{e}"));
    let (model, _) = terraflux_ops::milp::compile(&p).unwrap();
    let r = reference("zones");
    assert_model("zones_polygons", &model, &r);
    let mut q = params.clone();
    q["gap"] = 0.0.into();
    let ins = vec![("zone_features", stack), ("polygons", Value::Vector(read("zone_polygons")))];
    let out = run("prioritize.optimize", ins, q);
    let Value::Table(s) = &out["summary"] else { panic!() };
    let ours = s.column("objective").unwrap()[0];
    let want = r.floats("zones_polygons/objective")[0];
    assert!((ours - want).abs() <= 1e-9 * want.abs().max(1.0), "optimum: {ours} != {want}");
}
