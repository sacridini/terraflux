//! Surface metrics against landschaft's (tests/data/reference/surface.json and surface_window.npz,
//! tools/make_surface_reference.py): every metric of its example surfaces (geographic grids) and the moving
//! window of some, square and circular, to the bit.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::files::Rasters;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(op: &str, map: Map, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: [("map".to_string(), Value::Map(map))].into_iter().collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
}

fn example(name: &str) -> Map {
    let p = root().join("extern/landschaft/src/landschaft/surface/data").join(format!("{name}.tif"));
    Rasters::open(p.to_str().unwrap(), None, None).unwrap().map().unwrap()
}

/// The top-left `n` x `n` cells of a map (the same origin).
fn crop(m: &Map, n: usize) -> Map {
    let mut g = (*m.grid).clone();
    let nx = g.width;
    (g.width, g.height) = (n, n);
    Map::new(Arc::new(g), (0..n).flat_map(|i| m.data[i * nx..i * nx + n].to_vec()).collect())
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

#[test]
fn surface_metrics_are_landschafts() {
    let text = std::fs::read_to_string(root().join("tests/data/reference/surface.json")).unwrap();
    let reference: serde_json::Value = serde_json::from_str(&text).unwrap();
    let windows = terraflux_raster::npy::read_npz(root().join("tests/data/reference/surface_window.npz")).unwrap();
    let names: Vec<String> = serde_json::from_value(reference["window"].clone()).unwrap();
    for (name, want) in reference["metrics"].as_object().unwrap() {
        let map = example(name);
        let out = run("surface.metrics", map.clone(), serde_json::json!({}));
        let Value::Table(t) = &out["table"] else { panic!() };
        for (k, col) in t.columns.iter().enumerate() {
            let w = want[col].as_f64().unwrap_or(f64::NAN);
            assert!(same(t.rows[0][k], w), "{name} {col}: {} {w}", t.rows[0][k]);
        }
        assert_eq!(t.columns.len(), 27);
        let small = crop(&map, 40);
        for shape in ["square", "circle"] {
            let out = run(
                "surface.window",
                small.clone(),
                serde_json::json!({"metrics": names, "window": 7, "shape": shape}),
            );
            let Value::Stack(st) = &out["maps"] else { panic!() };
            for (k, m) in names.iter().enumerate() {
                let w = &windows[&format!("{name}/{shape}/{m}")];
                assert!(st.layers[k].iter().zip(&w.data).all(|(a, b)| same(*a, *b)), "{name} {shape} {m}");
            }
        }
    }
}
