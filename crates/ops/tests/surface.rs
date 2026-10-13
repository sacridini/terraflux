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

/// The terrain of a surface against landschaft's (tests/data/reference/surface_terrain.npz): every function with
/// some options on orelevation, the HLI of its slope and aspect, the planes and residuals, the peaks and
/// valleys, the bearing area curve and normforest's autocorrelation, to the bit (the sines and cosines landschaft
/// takes from numpy for VRM, the spherical standard deviation, TRASP and HLI too).
#[test]
fn the_terrain_is_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/surface_terrain.npz")).unwrap();
    let cases: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/data/reference/surface_terrain.json")).unwrap(),
    )
    .unwrap();
    let elevation = example("orelevation");
    let close = |a: &[f64], b: &[f64], _exact: bool, what: &str| {
        assert_eq!(a.len(), b.len(), "{what}");
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            let ok = same(*x, *y);
            assert!(ok, "{what}: {i}: {x} {y}");
        }
    };
    let map_of = |out: &Outputs, port: &str| -> Vec<f64> {
        let Value::Map(m) = &out[port] else { panic!("{port}") };
        m.data.as_ref().clone()
    };
    for case in cases["terrain"].as_array().unwrap() {
        let (key, op) = (case[0].as_str().unwrap(), case[1].as_str().unwrap());
        let out = run(&format!("surface.{op}"), elevation.clone(), case[2].clone());
        let exact = !["vrm", "spherical_sd", "trasp"].contains(&op);
        close(&map_of(&out, "map"), &z[key].data, exact, key);
    }
    // the heat load index of the slope and the aspect
    let slope = map_of(&run("surface.terrain", elevation.clone(), serde_json::json!({"v": "slope"})), "map");
    let aspect = map_of(&run("surface.terrain", elevation.clone(), serde_json::json!({"v": "aspect"})), "map");
    for (k, kw) in cases["hli"].as_array().unwrap().iter().enumerate() {
        let r = Registry::standard();
        let params = kw.as_object().cloned().unwrap_or_default();
        let m = |v: &Vec<f64>| Value::Map(Map::new(elevation.grid.clone(), v.clone()));
        let args = Args {
            node: "n",
            inputs: [("aspect".to_string(), m(&aspect)), ("slope".to_string(), m(&slope))].into_iter().collect(),
            params: &params,
        };
        let out = r.get("surface.hli").unwrap().run(&mut Ctx::default(), &args).unwrap();
        close(&map_of(&out, "map"), &z[&format!("hli_{k}")].data, false, &format!("hli {k}"));
    }
    for order in ["0", "2", "5"] {
        let out =
            run("surface.fit_plane", elevation.clone(), serde_json::json!({"order": order.parse::<i32>().unwrap()}));
        close(&map_of(&out, "plane"), &z[&format!("plane_{order}")].data, true, &format!("plane {order}"));
    }
    let out = run("surface.fit_plane", elevation.clone(), serde_json::json!({}));
    close(&map_of(&out, "plane"), &z["plane_best"].data, true, "best plane");
    close(&map_of(&out, "residuals"), &z["residuals_best"].data, true, "residuals");
    for (which, valleys) in [("peaks", false), ("valleys", true)] {
        let out = run("surface.peaks", elevation.clone(), serde_json::json!({"valleys": valleys}));
        let Value::Table(t) = &out["table"] else { panic!() };
        close(&t.rows.iter().flatten().copied().collect::<Vec<_>>(), &z[which].data, true, which);
    }
    let out = run("surface.bearing", elevation.clone(), serde_json::json!({}));
    let Value::Table(t) = &out["table"] else { panic!() };
    close(&t.rows.iter().flatten().copied().collect::<Vec<_>>(), &z["bearing"].data, true, "bearing");
    let out = run("surface.aacf", example("normforest"), serde_json::json!({}));
    close(&map_of(&out, "map"), &z["aacf"].data, true, "aacf");
}
