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

/// The cases of GuidosToolbox's outputs that landschaft keeps (digests of GTB's maps and its reports, of the
/// maps in `tests/data/reference/gtb_inputs.npz`), by tool.
fn gtb_cases(tool: &str) -> (Vec<serde_json::Value>, std::collections::BTreeMap<String, terraflux_raster::npy::Array>) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join("extern/landschaft/tests/data/reference/idl_reference.json")).unwrap();
    let all: serde_json::Value = serde_json::from_str(&text).unwrap();
    let inputs = terraflux_raster::npy::read_npz(root.join("tests/data/reference/gtb_inputs.npz")).unwrap();
    (all[tool].as_array().unwrap().clone(), inputs)
}

/// The digest of a map's cells as GTB's bytes (`u8`) or its 32-bit integers.
fn digest(values: &[f64], int32: bool) -> String {
    use sha2::Digest;
    let bytes: Vec<u8> = if int32 {
        values.iter().flat_map(|&v| (v as i32).to_le_bytes()).collect()
    } else {
        values.iter().map(|&v| v as u8).collect()
    };
    hex::encode(sha2::Sha256::digest(&bytes))
}

fn gtb_map(
    case: &serde_json::Value,
    inputs: &std::collections::BTreeMap<String, terraflux_raster::npy::Array>,
) -> Value {
    let a = &inputs[case["input"].as_str().unwrap()];
    assert_eq!(digest(&a.data, false), case["input_sha256"], "the input of {case}");
    Value::Map(map(a.shape[0], a.shape[1], &a.data))
}

fn table(out: &terraflux_ops::Outputs, port: &str) -> terraflux_ops::Table {
    let Value::Table(t) = &out[port] else { panic!("{port} is no table") };
    t.clone()
}

fn number(stats: &serde_json::Value, key: &str) -> f64 {
    stats[key].as_str().unwrap().trim_end_matches('.').parse().unwrap()
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1e-12)
}

/// SPA gives GTB's maps and object counts on every case kept (2, 3, 5 and 6 classes on six maps).
#[test]
fn spa_is_guidos_toolbox() {
    let (cases, inputs) = gtb_cases("spa");
    for case in &cases {
        let variant = case["variant"].as_i64().unwrap();
        let out = run("landscape.spa", vec![("map", gtb_map(case, &inputs))], serde_json::json!({"variant": variant}))
            .unwrap();
        let Value::Map(m) = &out["map"] else { panic!() };
        assert_eq!(digest(&m.data, false), case["sha256"], "SPA{variant} of {}", case["input"]);
        let objects = table(&out, "objects");
        let count = |k: &str| objects.column(k).unwrap()[0];
        let stats = &case["stats"];
        let keys: &[(&str, &str)] = if variant == 2 {
            &[("nr_FG", "foreground"), ("nr_Cores", "contiguous"), ("nr_margin", "small_linear_features")]
        } else {
            &[
                ("nr_FG", "foreground"),
                ("nr_Cores", "core"),
                ("nr_margin", "margin"),
                ("nr_holes", "core_opening"),
                ("nr_edge", "edge"),
                ("nr_perf", "perforation"),
                ("nr_islet", "islet"),
                ("nr_contiguous", "contiguous"),
            ]
        };
        for (gtb, ours) in keys {
            if stats.get(*gtb).is_some() {
                assert_eq!(count(ours), number(stats, gtb), "SPA{variant} of {}: {ours}", case["input"]);
            }
        }
        if variant != 2 {
            let Value::Number(p) = out["porosity"] else { panic!() };
            assert!(close(p, number(stats, "porosity"), 1e-12), "porosity {p}");
        }
    }
    assert_eq!(cases.len(), 24);
}

/// Accounting gives GTB's maps (the size classes, the objects' ids and areas) and its report.
#[test]
fn accounting_is_guidos_toolbox() {
    let (cases, inputs) = gtb_cases("accounting");
    let mut checked = 0;
    for case in &cases {
        // the classes scaled to a map's objects are landschaft's own (GTB's dialog), not an operator's setting
        if case.get("scaled_classes").is_some() {
            continue;
        }
        let params = serde_json::json!({
            "connectivity": case["connectivity"],
            "thresholds": case["thresholds"],
            "highlight_largest": case["highlight_largest"],
        });
        let out = run("landscape.accounting", vec![("map", gtb_map(case, &inputs))], params.clone()).unwrap();
        let what = format!("{} {params}", case["input"]);
        for (port, key, int32) in [("map", "classes", false), ("ids", "ids", true), ("area", "area", true)] {
            let Value::Map(m) = &out[port] else { panic!() };
            assert_eq!(digest(&m.data, int32), case["sha256"][key], "{port} of {what}");
        }
        let stats = table(&out, "stats");
        let list = |k: &str| -> Vec<f64> {
            case["stats"][k].as_str().unwrap().split(',').map(|v| v.trim_end_matches('.').parse().unwrap()).collect()
        };
        let classes = number(&case["stats"], "nr_cat") as usize + 1;
        assert_eq!(stats.column("objects").unwrap()[..classes], list("cat_id")[..], "{what}");
        assert_eq!(stats.column("cells").unwrap()[..classes], list("cat_area")[..], "{what}");
        let objects = table(&out, "objects");
        assert_eq!(objects.rows.len() as f64, number(&case["stats"], "tot_ids"), "{what}");
        checked += 1;
    }
    assert_eq!(checked, 35);
}

/// FAD-MS gives GTB's density at each of its five windows and their average.
#[test]
fn fad_multiscale_is_guidos_toolbox() {
    let (cases, inputs) = gtb_cases("fad_mscale");
    for case in &cases {
        let out =
            run("fragmentation.fad_multiscale", vec![("map", gtb_map(case, &inputs))], serde_json::json!({})).unwrap();
        let Value::Stack(s) = &out["scales"] else { panic!() };
        for (k, layer) in s.keys.iter().zip(&s.layers) {
            assert_eq!(digest(layer, false), case["sha256"][k.to_string()], "FAD {k} of {}", case["input"]);
        }
        let Value::Map(m) = &out["map"] else { panic!() };
        assert_eq!(digest(&m.data, false), case["sha256"]["mscale"], "FAD-MS of {}", case["input"]);
        // GTB's averages, in single precision
        let stats = table(&out, "stats");
        let (av, ru) = (stats.column("average").unwrap(), stats.column("avcon").unwrap());
        for i in 0..6 {
            assert!(close(av[i], case["fad_av"][i].as_f64().unwrap(), 1e-6), "{} average {i}", case["input"]);
            assert!(close(ru[i], case["fadru_av"][i].as_f64().unwrap(), 1e-6), "{} AVCON {i}", case["input"]);
        }
    }
    assert_eq!(cases.len(), 6);
    // one window: the same map as fragmentation.fad's, with the same classes
    let (_, inputs) = gtb_cases("fad_mscale");
    let a = &inputs["gtb_input"];
    let land = Value::Map(map(a.shape[0], a.shape[1], &a.data));
    let one = run("fragmentation.fad", vec![("map", land.clone())], serde_json::json!({"window": 27})).unwrap();
    let ms = run("fragmentation.fad_multiscale", vec![("map", land)], serde_json::json!({"windows": [27]})).unwrap();
    let (Value::Map(a), Value::Map(b)) = (&one["map"], &ms["map"]) else { panic!() };
    assert_eq!(a.data, b.data);
    let stats = table(&one, "stats");
    assert!(close(stats.column("average").unwrap()[0], 73.543212890625, 1e-6));
    assert!((stats.rows[0][1..7].iter().sum::<f64>() - 100.0).abs() < 1e-9, "the classes share the foreground");
}

/// FOS gives GTB's maps and report on every case kept: binary maps (with special backgrounds) by FAD, FED and FAC,
/// % covers by FAD and FED, in 5 and 6 classes.
#[test]
fn fos_is_guidos_toolbox() {
    let (cases, inputs) = gtb_cases("fos");
    for case in &cases {
        let (method, classes) = case["fosclass"].as_str().unwrap().split_once('_').unwrap();
        let params = serde_json::json!({
            "window": case["window"],
            "method": method,
            "classes": classes[..1].parse::<i64>().unwrap(),
            "connectivity": case["connectivity"],
            "gray_threshold": case["gray_threshold"],
        });
        let out = run("fragmentation.fos", vec![("map", gtb_map(case, &inputs))], params.clone()).unwrap();
        let what = format!("{} {params}", case["input"]);
        let Value::Map(m) = &out["map"] else { panic!() };
        assert_eq!(digest(&m.data, false), case["sha256"], "FOS of {what}");
        let stats = &case["stats"];
        let s = table(&out, "summary");
        let col = |k: &str| s.column(k).unwrap()[0];
        assert_eq!(col("foreground"), number(stats, "fgarea"), "{what}");
        assert_eq!(col("reporting_area"), number(stats, "n_ruarea"), "{what}");
        assert_eq!(col("patches"), number(stats, "obj_last"), "{what}");
        for (ours, gtb) in [
            ("average_patch_size", "aps"),
            ("eca", "ECA"),
            ("coherence", "COH"),
            ("coherence_ru", "COH_ru"),
            ("foreground_percent", "fareaperc"),
        ] {
            assert!(close(col(ours), number(stats, gtb), 1e-12), "{what}: {ours} {} != {}", col(ours), stats[gtb]);
        }
        // GTB works these out in single precision
        assert!(close(col("average"), number(stats, "fad_av"), 1e-6), "{what}: average");
        assert!(close(col("avcon"), number(stats, "fadru_av"), 1e-6), "{what}: AVCON");
        let shares: Vec<f64> = stats["classes"].as_str().unwrap().split(',').map(|v| v.parse().unwrap()).collect();
        let ours = table(&out, "stats").column("percent").unwrap();
        for (i, p) in ours.iter().enumerate().take(if classes.starts_with('6') { 6 } else { 4 }) {
            assert!(
                (p - shares[i]).abs() <= 1e-6 * shares[i].abs() + 1e-9,
                "{what}: class {}: {p} != {}",
                i + 1,
                shares[i]
            );
        }
    }
    assert_eq!(cases.len(), 13);
    // a land use map: its forest the foreground; maps that cannot be read so
    let land = map(
        30,
        30,
        &(0..900)
            .map(|i| if (5..20).contains(&(i / 30)) && (5..20).contains(&(i % 30)) { 7.0 } else { 3.0 })
            .collect::<Vec<_>>(),
    );
    let out = run(
        "fragmentation.fos",
        vec![("map", Value::Map(land.clone()))],
        serde_json::json!({"window": 5, "foreground": 7}),
    )
    .unwrap();
    let Value::Map(m) = &out["map"] else { panic!() };
    assert_eq!((m.data[0], m.data[12 * 30 + 12]), (101.0, 100.0));
    assert!(
        run(
            "fragmentation.fos",
            vec![("map", Value::Map(land.clone()))],
            serde_json::json!({"window": 5, "foreground": 9})
        )
        .is_err(),
        "no foreground"
    );
    assert!(
        run("fragmentation.fos", vec![("map", Value::Map(land))], serde_json::json!({"window": 31, "foreground": 7}))
            .is_err(),
        "too small"
    );
}

/// What landschaft gives for the GTB tools it keeps no digests of GTB's outputs for
/// (`tools/make_gtb_reference.py`), by tool.
fn landschaft_cases(tool: &str) -> Vec<serde_json::Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join("tests/data/reference/gtb_landschaft.json")).unwrap();
    let all: serde_json::Value = serde_json::from_str(&text).unwrap();
    all[tool].as_array().unwrap().clone()
}

/// The digest of a map's cells as 32-bit floats (no value: NaN) or unsigned integers.
fn digest_f32(values: &[f64]) -> String {
    use sha2::Digest;
    let bytes: Vec<u8> = values.iter().flat_map(|&v| (v as f32).to_le_bytes()).collect();
    hex::encode(sha2::Sha256::digest(&bytes))
}

fn digest_u32(values: &[f64]) -> String {
    use sha2::Digest;
    let bytes: Vec<u8> = values.iter().flat_map(|&v| (v as u32).to_le_bytes()).collect();
    hex::encode(sha2::Sha256::digest(&bytes))
}

fn input_map(name: &str) -> Value {
    let (_, inputs) = gtb_cases("spa");
    let a = &inputs[name];
    Value::Map(map(a.shape[0], a.shape[1], &a.data))
}

fn the_map<'a>(out: &'a terraflux_ops::Outputs, port: &str) -> &'a Map {
    let Value::Map(m) = &out[port] else { panic!("{port} is no map") };
    m
}

/// The same numbers, to the bit (the JSON read with serde_json's float_roundtrip, which gives every decimal's
/// nearest double).
fn assert_close(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len());
    for (a, b) in got.iter().zip(want) {
        assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()), "{got:?} != {want:?}");
    }
}

fn f(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap()
}

/// Contagion, clustering and the spatial entropy (of the map and in a window) give landschaft's maps.
#[test]
fn contagion_and_entropy_are_landschafts() {
    for (op, tool) in [("fragmentation.contagion", "contagion"), ("fragmentation.clustering", "clustering")] {
        for case in landschaft_cases(tool) {
            let out =
                run(op, vec![("map", input_map("gtb_input"))], serde_json::json!({"window": case["window"]})).unwrap();
            assert_eq!(digest(&the_map(&out, "map").data, false), case["sha256"], "{op} {}", case["window"]);
        }
    }
    let case = &landschaft_cases("entropy")[0];
    let out = run("fragmentation.entropy", vec![("map", input_map("gtb_input"))], serde_json::json!({})).unwrap();
    let Value::Number(index) = out["index"] else { panic!() };
    assert!(close(index, f(&case["index"]), 1e-12), "{index}");
    let pairs = table(&out, "pairs");
    let want: Vec<(f64, f64)> = case["histogram"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, n)| f(n) > 0.0)
        .map(|(k, n)| (k as f64, f(n)))
        .collect();
    assert_eq!(pairs.rows.iter().map(|r| (r[0], r[1])).collect::<Vec<_>>(), want);
    for case in landschaft_cases("entropy_map") {
        let out = run(
            "fragmentation.entropy_map",
            vec![("map", input_map("blobs_101"))],
            serde_json::json!({"window": case["window"]}),
        )
        .unwrap();
        assert_eq!(digest_f32(&the_map(&out, "map").data), case["sha256"], "entropy map {}", case["window"]);
    }
}

/// The landscape mosaic of 103 classes is GuidosToolbox's (the digests landschaft keeps of GTB's), that of 19
/// landschaft's; GraySpatCon's ten metrics are landschaft's.
#[test]
fn landscape_mosaic_and_gray_spatcon_are_guidos_toolbox() {
    let (cases, inputs) = gtb_cases("lm_heatmap");
    for case in &cases {
        let out = run(
            "fragmentation.landscape_mosaic",
            vec![("map", gtb_map(case, &inputs))],
            serde_json::json!({"window": case["window"], "classes": 103}),
        )
        .unwrap();
        assert_eq!(digest(&the_map(&out, "map").data, false), case["im103_sha256"], "LM103 of {case}");
        let stats = table(&out, "stats");
        assert!((stats.column("percent").unwrap().iter().sum::<f64>() - 100.0).abs() < 1e-9);
    }
    assert_eq!(cases.len(), 6);
    for case in landschaft_cases("landscape_mosaic_19") {
        let name = case["input"].as_str().unwrap();
        let out = run(
            "fragmentation.landscape_mosaic",
            vec![("map", input_map(name))],
            serde_json::json!({"window": case["window"]}),
        )
        .unwrap();
        assert_eq!(digest(&the_map(&out, "map").data, false), case["sha256"], "LM19 of {name}");
    }
    // a land cover of other codes: its classes given
    let lc = map(3, 3, &[10.0, 10.0, 20.0, 30.0, 10.0, 20.0, 30.0, 30.0, 99.0]);
    let given = serde_json::json!({"window": 3, "agriculture": 10, "natural": [20], "developed": 30});
    let coded = map(3, 3, &[1.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 3.0, 0.0]);
    let a = run("fragmentation.landscape_mosaic", vec![("map", Value::Map(lc))], given).unwrap();
    let b = run("fragmentation.landscape_mosaic", vec![("map", Value::Map(coded))], serde_json::json!({"window": 3}))
        .unwrap();
    assert_eq!(the_map(&a, "map").data, the_map(&b, "map").data);

    let mut checked = 0;
    for case in landschaft_cases("gray_convolution") {
        let params = serde_json::json!({
            "metric": case["metric"], "window": case["window"], "mask_missing": case["mask_missing"]
        });
        let out = run("fragmentation.gray_convolution", vec![("map", input_map("gscinput"))], params.clone()).unwrap();
        // GraySpatCon's missing value where the map has none
        let raw: Vec<f64> =
            the_map(&out, "map").data.iter().map(|&v| if v.is_nan() { -0.01f32 as f64 } else { v }).collect();
        assert_eq!(digest_f32(&raw), case["sha256"], "{params}");
        checked += 1;
    }
    assert_eq!(checked, 20);
}

/// Parcellation, contortion and the Euclidean distance give landschaft's maps and numbers.
#[test]
fn parcels_contortion_and_distance_are_landschafts() {
    for case in landschaft_cases("parcellation") {
        let name = case["input"].as_str().unwrap();
        let out = run(
            "landscape.parcellation",
            vec![("map", input_map(name))],
            serde_json::json!({"connectivity": case["connectivity"]}),
        )
        .unwrap();
        let what = format!("{name} {}", case["connectivity"]);
        assert_eq!(digest_u32(&the_map(&out, "parcels").data), case["sha256"], "{what}");
        let s = table(&out, "summary");
        let g = &case["global"];
        for (ours, theirs) in [
            ("parcels", "total_parcels"),
            ("cells", "data_area"),
            ("aps", "aps"),
            ("awaps", "awaps"),
            ("division", "division"),
            ("parc", "parc"),
        ] {
            assert_eq!(s.column(ours).unwrap()[0].to_bits(), f(&g[theirs]).to_bits(), "{what}: {ours}");
        }
        let counts: Vec<f64> = case["counts"].as_array().unwrap().iter().map(f).collect();
        assert_eq!(table(&out, "stats").column("parcels").unwrap(), counts, "{what}");
    }
    for case in landschaft_cases("contortion") {
        let out = run(
            "landscape.contortion",
            vec![("map", input_map("gtb_input"))],
            serde_json::json!({"min_area": case["min_area"]}),
        )
        .unwrap();
        assert_eq!(digest_u32(&the_map(&out, "map").data), case["sha256"], "contortion {}", case["min_area"]);
        assert_eq!(
            digest(&the_map(&out, "display").data, false),
            case["display_sha256"],
            "display {}",
            case["min_area"]
        );
    }
    let case = &landschaft_cases("euclidean_distance")[0];
    let out = run("distance.euclidean", vec![("map", input_map("gtb_input"))], serde_json::json!({})).unwrap();
    assert_eq!(digest_f32(&the_map(&out, "map").data), case["sha256"]);
    assert_eq!(digest(&the_map(&out, "display").data, false), case["display_sha256"]);
    let s = table(&out, "stats");
    let want: Vec<f64> = case["stats"].as_array().unwrap().iter().map(f).collect();
    assert_close(&s.rows[0][1..], &want[..3]);
    assert_close(&s.rows[1][1..], &want[3..]);
}

/// The simple, morphological and FOS changes give landschaft's maps and reports.
#[test]
fn changes_are_landschafts() {
    let two = || vec![("before", input_map("gtb_input")), ("after", input_map("gtb_input_changed"))];
    let case = &landschaft_cases("simple_change")[0];
    let out = run("change.simple", two(), serde_json::json!({})).unwrap();
    assert_eq!(digest(&the_map(&out, "map").data, false), case["sha256"]);
    let counts: Vec<f64> = case["counts"].as_array().unwrap().iter().map(f).collect();
    assert_eq!(table(&out, "stats").rows[0], counts);

    let case = &landschaft_cases("morphological_change")[0];
    let out = run("change.morphological", two(), serde_json::json!({})).unwrap();
    assert_eq!(digest(&the_map(&out, "map").data, false), case["sha256"]);
    let mut want: Vec<f64> = case["counts"].as_array().unwrap().iter().map(f).collect();
    want.push(f(&case["elasticity"]));
    assert_close(&table(&out, "stats").rows[0], &want);

    let case = &landschaft_cases("fos_change")[0];
    let fos = |name: &str| {
        let out = run("fragmentation.fos", vec![("map", input_map(name))], serde_json::json!({"window": 27})).unwrap();
        out["map"].clone()
    };
    let out = run(
        "change.fos",
        vec![("before", fos("gtb_input")), ("after", fos("gtb_input_changed"))],
        serde_json::json!({}),
    )
    .unwrap();
    assert_eq!(digest(&the_map(&out, "map").data, false), case["sha256"]);
    let mut want: Vec<f64> = case["counts"].as_array().unwrap().iter().map(f).collect();
    want.extend(case["percent"].as_array().unwrap().iter().map(f));
    assert_close(&table(&out, "stats").rows[0], &want);
    let matrix: Vec<Vec<f64>> =
        case["matrix"].as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(f).collect()).collect();
    let classes = table(&out, "classes");
    assert_eq!(classes.rows.iter().map(|r| r[1..].to_vec()).collect::<Vec<_>>(), matrix);
    // maps of different sizes
    let small = Value::Map(map(2, 2, &[1.0; 4]));
    assert!(
        run("change.simple", vec![("before", small), ("after", input_map("gtb_input"))], serde_json::json!({}))
            .is_err()
    );
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
