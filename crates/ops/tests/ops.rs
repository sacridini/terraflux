//! The operators on small cases worked by hand (the examples of landschaft's documentation).

use std::sync::Arc;

use terraflux_ops::dynamic::{distance_to, share_of};
use terraflux_ops::{Args, Ctx, Demand, Key, Map, Registry, Stack, Value};
use terraflux_raster::Grid;

fn map(ny: usize, nx: usize, v: &[f64]) -> Map {
    Map::new(Arc::new(Grid::new(nx, ny)), v.to_vec())
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Result<terraflux_ops::Outputs, String> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).map_err(|e| e.to_string())
}

#[test]
fn distance_to_a_class() {
    let land = map(2, 4, &[1.0, 1.0, 1.0, 3.0, 1.0, 1.0, 1.0, 1.0]);
    let d = distance_to(&land, &[3.0], 1.0).unwrap();
    let want = [3.0, 2.0, 1.0, 0.0, 10f64.sqrt(), 5f64.sqrt(), 2f64.sqrt(), 1.0];
    for (a, b) in d.data.iter().zip(want) {
        assert!((a - b).abs() < 1e-12, "{a} != {b}");
    }
    // no cell of the class: the diagonal everywhere; missing cells stay missing
    let land = map(1, 3, &[1.0, f64::NAN, 1.0]);
    let d = distance_to(&land, &[9.0], 2.0).unwrap();
    assert_eq!(d.data[0], 3f64.hypot(1.0) * 2.0);
    assert!(d.data[1].is_nan());
}

#[test]
fn share_of_a_class() {
    let land = map(3, 3, &[1.0, 2.0, 2.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0]);
    let s = share_of(&land, &[2.0], &vec![vec![1.0; 3]; 3]).unwrap();
    assert!((s.data[4] - 1.0 / 3.0).abs() < 1e-15);
    assert!(share_of(&land, &[2.0], &vec![vec![1.0; 2]; 2]).is_err());
}

#[test]
fn stacks_by_name_class_and_transition() {
    let g = Arc::new(Grid::new(2, 1));
    let s = Stack {
        grid: g.clone(),
        keys: vec![Key::Name("1".into()), Key::Name("2_to_3".into())],
        layers: vec![Arc::new(vec![0.1, 0.2]), Arc::new(vec![0.3, 0.4])],
    };
    let out = run("stack.classes", vec![("stack", Value::Stack(s.clone()))], serde_json::json!({})).unwrap();
    let Value::Stack(c) = &out["stack"] else { panic!() };
    assert_eq!(c.keys, vec![Key::Class(1.0), Key::Transition(2.0, 3.0)]);
    let out = run(
        "stack.set",
        vec![("stack", Value::Stack(s)), ("map", Value::Map(Map::new(g, vec![9.0, 9.0])))],
        serde_json::json!({"name": "slope"}),
    )
    .unwrap();
    let Value::Stack(c) = &out["stack"] else { panic!() };
    assert_eq!(c.by_name("slope").unwrap().as_slice(), &[9.0, 9.0]);
    assert_eq!(c.layers.len(), 3);
}

#[test]
fn cars_demand_from_an_area_demand() {
    let land = map(1, 4, &[1.0, 1.0, 2.0, f64::NAN]);
    let ok = Demand { classes: vec![1.0, 2.0], times: vec![0.0, 1.0], table: vec![vec![2.0, 1.0], vec![1.0, 2.0]] };
    let out = run(
        "plus.demand",
        vec![("start", Value::Map(land.clone())), ("demand", Value::Demand(Arc::new(ok)))],
        serde_json::json!({}),
    )
    .unwrap();
    assert!(matches!(out["steps"], Value::Integer(1)));
    let bad = Demand { classes: vec![1.0, 2.0], times: vec![0.0, 1.0], table: vec![vec![2.0, 1.0], vec![1.0, 1.0]] };
    let e = run(
        "plus.demand",
        vec![("start", Value::Map(land)), ("demand", Value::Demand(Arc::new(bad)))],
        serde_json::json!({}),
    )
    .unwrap_err();
    assert!(e.contains("adds up to"), "{e}");
}

#[test]
fn fitted_pieces_from_the_model_file() {
    let out = run(
        "markov.matrix",
        vec![],
        serde_json::json!({"classes": [1, 2], "single": [[0.81, 0.19], [0.0, 1.0]], "steps": 2}),
    )
    .unwrap();
    let Value::Markov(m) = &out["markov"] else { panic!() };
    assert!((m.multi[0] - 0.9).abs() < 1e-14 && (m.multi[1] - 0.1).abs() < 1e-14);
    let out = run(
        "glm.coefficients",
        vec![],
        serde_json::json!({"fits": [{"class": 1, "drivers": ["a", "b"], "coefficients": [0.5, null, 2.0],
                                     "pivot": [1, 2, 3], "rank": 2}]}),
    )
    .unwrap();
    let Value::Glm(g) = &out["model"] else { panic!() };
    assert!(g.fits[0].coefficients[1].is_nan());
    assert!(
        run(
            "glm.coefficients",
            vec![],
            serde_json::json!({"fits": [{"class": 1, "drivers": ["a"], "coefficients": [1.0],
        "pivot": [1], "rank": 1}]})
        )
        .is_err()
    );
}

#[test]
fn agreement_and_figure_of_merit() {
    use terraflux_ops::validate::agreement;
    let start = map(1, 4, &[1.0, 1.0, 1.0, 1.0]);
    let obs = map(1, 4, &[2.0, 2.0, 1.0, 1.0]);
    let sim = map(1, 4, &[2.0, 3.0, 2.0, 1.0]);
    let a = agreement(&obs, &sim, &start).unwrap();
    assert_eq!((a.hits, a.wrong_hits, a.misses, a.false_alarms), (1, 1, 0, 1));
    assert!((a.figure_of_merit - 1.0 / 3.0).abs() < 1e-15 && a.overall == 0.5);
    assert_eq!(a.quantity, vec![(1.0, -1), (2.0, 0), (3.0, 1)]);
}

#[test]
fn similarity_of_differences() {
    // landschaft's example: one cell changed, simulated one cell away
    let t0 = map(6, 6, &[1.0; 36]);
    let mut o = vec![1.0; 36];
    o[2 * 6 + 2] = 2.0;
    let mut s = vec![1.0; 36];
    s[2 * 6 + 3] = 2.0;
    let t =
        terraflux_ops::validate::similarity_of_differences(&t0, &map(6, 6, &o), &map(6, 6, &s), &[1, 3], true).unwrap();
    assert_eq!(t.rows[0][3], 0.0);
    assert!((t.rows[1][3] - 0.5f64.sqrt()).abs() < 1e-6, "{}", t.rows[1][3]);
}

/// MSPA gives the bits of GuidosToolbox's `mspa` executable (the outputs landschaft keeps of it, on GTB's
/// example map), for every connectivity, edge width and internal setting kept.
#[test]
fn mspa_is_guidos_toolbox() {
    let npz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extern/landschaft/tests/data/reference/gtb_input.npz");
    let arrays = terraflux_raster::npy::read_npz(&npz).unwrap();
    let input = &arrays["input"];
    let (ny, nx) = (input.shape[0], input.shape[1]);
    let land = Value::Map(map(ny, nx, &input.data));
    let mut checked = 0;
    for (name, want) in &arrays {
        let Some(rest) = name.strip_prefix('c') else { continue };
        let parts: Vec<i64> =
            rest.split(['_', 'e', 'i']).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
        let (c, e, i) = (parts[0], parts[1], parts[2] == 1);
        let out = run(
            "landscape.mspa",
            vec![("map", land.clone())],
            serde_json::json!({"connectivity": c, "edge_width": e, "internal": i}),
        )
        .unwrap();
        let Value::Map(got) = &out["map"] else { panic!() };
        let diff = got.data.iter().zip(&want.data).filter(|(a, b)| a != b).count();
        assert_eq!(diff, 0, "{name}: {diff} cells differ from GuidosToolbox");
        checked += 1;
    }
    assert_eq!(checked, 7);
    // a land use map: the foreground's classes given
    let lu = map(3, 3, &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 5.0]);
    let out =
        run("landscape.mspa", vec![("map", Value::Map(lu.clone()))], serde_json::json!({"foreground": 1})).unwrap();
    // landschaft's `ls.mspa` of the same map coded 0/1/2: class 5, background, a border-opening
    let Value::Map(m) = &out["map"] else { panic!() };
    assert_eq!(*m.data, [3.0, 17.0, 3.0, 17.0, 3.0, 3.0, 3.0, 3.0, 220.0]);
    assert!(run("landscape.mspa", vec![("map", Value::Map(lu))], serde_json::json!({})).is_err(), "not GTB's codes");
}

#[test]
fn rasters_are_read_and_written() {
    use terraflux_ops::files::{Rasters, as_port};
    let dir = std::env::temp_dir().join(format!("terraflux-ops-files-{}", std::process::id()));
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
    let whole = terraflux_raster::Raster::read(&data).unwrap();
    // read: the headers only, the file on disk; a map or a stack where a port takes one
    let read = run("raster.read", vec![], serde_json::json!({"path": data, "band": "2011"})).unwrap();
    let Value::Raster(r) = &read["map"] else { panic!() };
    assert_eq!((r.layers.len(), r.chosen), (2, 1));
    let Value::Map(m) = as_port(read["map"].clone(), terraflux_graph::Type::Map).unwrap() else { panic!() };
    let want = whole.bands[1].masked(whole.nodata);
    assert!(m.data.iter().zip(&want).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())));
    let Value::Stack(s) = as_port(read["stack"].clone(), terraflux_graph::Type::Stack).unwrap() else { panic!() };
    assert_eq!(s.keys.len(), 2);
    assert!(Arc::ptr_eq(&m.data, &s.layers[1]), "a layer is read once");
    // written from memory, in the smallest type
    let path = dir.join("out/lu.tif");
    let w = run("raster.write", vec![("value", Value::Stack(s))], serde_json::json!({"path": path})).unwrap();
    assert!(matches!(&w["path"], Value::Text(p) if p.ends_with("lu.tif")));
    let back = terraflux_raster::Raster::read(&path).unwrap();
    assert_eq!(back.dtype, terraflux_raster::DataType::U8);
    assert_eq!(back.band_names(), ["1997", "2011"]);
    // copied from disk in blocks, the same cells
    let copy = dir.join("copy.tif");
    run("raster.write", vec![("value", read["raster"].clone())], serde_json::json!({"path": copy})).unwrap();
    let c = terraflux_raster::Raster::read(&copy).unwrap();
    assert_eq!(c.band_names(), ["1997", "2011"]);
    for b in 0..2 {
        let (x, y) = (c.bands[b].masked(c.nodata), whole.bands[b].masked(whole.nodata));
        assert!(x.iter().zip(&y).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())), "band {b}");
    }
    // a folder, a pattern: a series, a layer per file
    let series = dir.join("series");
    std::fs::create_dir_all(&series).unwrap();
    for year in ["1997", "2011"] {
        let mut one = whole.clone();
        one.bands.retain(|b| b.name.as_deref() == Some(year));
        one.write(series.join(format!("lu_{year}.tif"))).unwrap();
    }
    std::fs::write(series.join("notes.txt"), "not a raster").unwrap();
    let all = Rasters::open(&series.to_string_lossy(), Some("lu_2011"), None).unwrap();
    assert_eq!(all.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["lu_1997", "lu_2011"]);
    assert_eq!(all.chosen, 1);
    let some = Rasters::open(&series.join("*_19*.TIF").to_string_lossy(), None, None).unwrap();
    assert_eq!(some.layers.len(), 1);
    // a folder as one stack: every band of every file (a file of two bands gives two layers)
    whole.write(series.join("both.tif")).unwrap();
    let read = run("raster.read", vec![], serde_json::json!({"path": series})).unwrap();
    let Value::Raster(r) = &read["stack"] else { panic!() };
    assert_eq!(
        r.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
        ["both 1997", "both 2011", "lu_1997", "lu_2011"]
    );
    let st = r.stack().unwrap();
    assert_eq!(st.layers.len(), 4);
    let same = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(x, y)| x == y || (x.is_nan() && y.is_nan()));
    assert!(same(&st.layers[1], &st.layers[3]), "the same band, from two files");
    std::fs::remove_file(series.join("both.tif")).unwrap();
    assert!(Rasters::open(&series.join("*.png").to_string_lossy(), None, None).is_err());
    assert!(run("raster.read", vec![], serde_json::json!({"path": dir.join("none.tif")})).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

/// Data prepared with GDAL: reprojected and back on the grid it came from, clipped; reclassified by Terraflux.
#[test]
fn rasters_are_prepared() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
    let lu = terraflux_raster::Raster::read(&data).unwrap();
    // reclassified: forest (1) kept, the rest 0, no data stays none
    let m = map(lu.grid.height, lu.grid.width, &lu.bands[0].masked(lu.nodata));
    let r =
        run("raster.reclassify", vec![("map", Value::Map(m.clone()))], serde_json::json!({"rules": "1 -> 1\n* -> 0"}))
            .unwrap();
    let Value::Map(f) = &r["map"] else { panic!() };
    for (a, b) in f.data.iter().zip(m.data.iter()) {
        assert!(if b.is_nan() { a.is_nan() } else { *a == f64::from(u8::from(*b == 1.0)) });
    }
    if let Err(e) = terraflux_gdal::gdal() {
        eprintln!("{e}: the GDAL operators are not tested");
        return;
    }
    let read = run("raster.read", vec![], serde_json::json!({"path": data, "band": "2011"})).unwrap();
    // to geographic coordinates (the file says no CRS: Sibuyan is in UTM 51N)
    let geo = run(
        "raster.reproject",
        vec![("raster", read["raster"].clone())],
        serde_json::json!({"crs": "EPSG:4326", "source_crs": "EPSG:32651"}),
    )
    .unwrap();
    let Value::Raster(g) = &geo["raster"] else { panic!() };
    assert_eq!(g.grid.geo.epsg(), Some(4326));
    assert_eq!(g.chosen, 1, "the band chosen follows");
    // and back on the grid it came from (its CRS given by a copy of it in UTM), by the nearest cell: the
    // classes nearly all where they were
    let utm = run(
        "raster.reproject",
        vec![("raster", read["raster"].clone())],
        serde_json::json!({"crs": "EPSG:32651", "source_crs": "EPSG:32651", "resolution": 250}),
    )
    .unwrap();
    let back = run(
        "raster.match",
        vec![("raster", geo["raster"].clone()), ("like", utm["raster"].clone())],
        serde_json::json!({"resampling": "near"}),
    )
    .unwrap();
    let Value::Raster(b) = &back["raster"] else { panic!() };
    assert_eq!((b.grid.width, b.grid.height), (lu.grid.width, lu.grid.height));
    let got = b.read(1).unwrap();
    let want = lu.bands[1].masked(lu.nodata);
    let same = got.iter().zip(&want).filter(|(a, b)| a == b).count();
    let valued = want.iter().filter(|v| !v.is_nan()).count();
    assert!(same as f64 > 0.85 * valued as f64, "{same} of {valued} cells kept their class");
    // clipped to the island's north-west quarter
    let t = lu.grid.transform;
    let (w, h) = (lu.grid.width as f64 * t[1], lu.grid.height as f64 * -t[5]);
    let clip = run(
        "raster.clip",
        vec![("raster", read["raster"].clone())],
        serde_json::json!({"extent": [t[0], t[3] - h / 2.0, t[0] + w / 2.0, t[3]]}),
    )
    .unwrap();
    let Value::Raster(c) = &clip["raster"] else { panic!() };
    assert_eq!((c.grid.width, c.grid.height), (lu.grid.width / 2, lu.grid.height / 2));
}

/// More of GDAL's preparation: resampled, converted, joined, measured as a surface, filled, distances.
#[test]
fn rasters_are_resampled_converted_joined_and_measured() {
    if let Err(e) = terraflux_gdal::gdal() {
        eprintln!("{e}: not tested");
        return;
    }
    let inputs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs");
    let lu = terraflux_raster::Raster::read(inputs.join("sibuyan_landuse.tif")).unwrap();
    let (w, h) = (lu.grid.width, lu.grid.height);
    let read = run("raster.read", vec![], serde_json::json!({"path": inputs.join("sibuyan_landuse.tif")})).unwrap();
    let raster = || read["raster"].clone();
    let rasters = |v: &Value| -> Arc<terraflux_ops::files::Rasters> {
        let Value::Raster(r) = v else { panic!("{v:?}") };
        r.clone()
    };
    // cells twice as large: half the cells a side, the most common class of each block
    let half = run("raster.resample", vec![("raster", raster())], serde_json::json!({"resolution": 500})).unwrap();
    let r = rasters(&half["raster"]);
    assert_eq!((r.grid.width, r.grid.height), (w / 2, h / 2));
    // Erdas Imagine, the same cells
    let img = run("raster.convert", vec![("raster", raster())], serde_json::json!({"format": "HFA"})).unwrap();
    let r = rasters(&img["raster"]);
    assert!(r.images[0].path().extension().is_some_and(|e| e == "img"));
    let same = r.read(0).unwrap();
    let want = lu.bands[0].masked(lu.nodata);
    assert!(same.iter().zip(&want).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())));
    // two halves cut, then joined: the whole map again
    let t = lu.grid.transform;
    let (wx, hy) = (w as f64 * t[1], h as f64 * -t[5]);
    let dir = std::env::temp_dir().join(format!("terraflux-mosaic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, x0, x1) in [("west", 0.0, wx / 2.0), ("east", wx / 2.0, wx)] {
        run(
            "raster.clip",
            vec![("raster", raster())],
            serde_json::json!({"extent": [t[0] + x0, t[3] - hy, t[0] + x1, t[3]], "path": dir.join(format!("{name}.tif"))}),
        )
        .unwrap();
    }
    let halves = run("raster.read", vec![], serde_json::json!({"path": dir})).unwrap();
    // (two files of two bands: four layers)
    assert_eq!(rasters(&halves["raster"]).layers.len(), 4);
    assert_eq!(rasters(&halves["raster"]).images.len(), 2);
    let whole = run("raster.mosaic", vec![("raster", halves["raster"].clone())], serde_json::json!({})).unwrap();
    let m = rasters(&whole["raster"]);
    assert_eq!((m.grid.width, m.grid.height), (w, h));
    // a surface's slope (the distance to the sea as one), of the same grid
    let dr =
        run("raster.read", vec![], serde_json::json!({"path": inputs.join("sibuyan_drivers.tif"), "band": "dist_sea"}))
            .unwrap();
    let slope =
        run("raster.terrain", vec![("raster", dr["raster"].clone())], serde_json::json!({"measure": "slope"})).unwrap();
    let s = rasters(&slope["raster"]);
    assert_eq!((s.grid.width, s.grid.height), (w, h));
    assert!(s.read(0).unwrap().iter().any(|v| *v > 0.0));
    // gaps filled: fewer cells without a value
    let filled = run("raster.fill", vec![("raster", raster())], serde_json::json!({"max_distance": 3})).unwrap();
    let f = rasters(&filled["raster"]).read(0).unwrap();
    assert!(f.iter().filter(|v| v.is_nan()).count() < want.iter().filter(|v| v.is_nan()).count());
    // the distance to forest (class 1): none in it, more away from it
    let d = run("distance.proximity", vec![("raster", raster())], serde_json::json!({"values": 1, "units": "cells"}))
        .unwrap();
    let d = rasters(&d["raster"]).read(0).unwrap();
    for (dist, class) in d.iter().zip(&want) {
        if *class == 1.0 {
            assert_eq!(*dist, 0.0);
        } else if !dist.is_nan() {
            assert!(*dist >= 1.0);
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}
