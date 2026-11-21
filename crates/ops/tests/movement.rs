//! Movement ecology (`move.*`) against the library's tf.move (tests/data/reference/move.npz and .json,
//! tools/make_move_reference.py): the track sets of the library's tests and move2's fishers, every number to the bit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use terraflux_ops::movement::time;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;
use terraflux_raster::vector::{Column, Geometry, Vector};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(Registry::standard)
}

fn reference() -> &'static (BTreeMap<String, Array>, serde_json::Value) {
    static R: OnceLock<(BTreeMap<String, Array>, serde_json::Value)> = OnceLock::new();
    R.get_or_init(|| {
        let dir = root().join("tests/data/reference");
        let z = terraflux_raster::npy::read_npz(dir.join("move.npz")).unwrap();
        let meta = serde_json::from_str(&std::fs::read_to_string(dir.join("move.json")).unwrap()).unwrap();
        (z, meta)
    })
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

fn run(op: &str, ins: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: ins.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    registry().get(op).unwrap().run(&mut Ctx::default(), &args)
}

fn go(op: &str, ins: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    run(op, ins, params).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn vector(o: &Outputs, port: &str) -> Arc<Vector> {
    match &o[port] {
        Value::Vector(v) => v.clone(),
        v => panic!("{port}: {:?}", v.ty()),
    }
}

fn num(v: &Vector, c: &str) -> Vec<f64> {
    v.column(c).unwrap_or_else(|| panic!("no column {c}")).numbers()
}

fn want(key: &str) -> &'static [f64] {
    &reference().0.get(key).unwrap_or_else(|| panic!("no reference {key}")).data
}

/// The same bits (NaN where NaN).
fn same(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: {} values, {} wanted", got.len(), want.len());
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()), "{what}[{k}]: {a:e} != {b:e}");
    }
}

/// A track set of the reference: points (an empty one where x is NaN), time (datetimes as text), track.
fn set(name: &str) -> Value {
    let (z, meta) = reference();
    let x = &z[&format!("in/{name}/x")].data;
    let y = &z[&format!("in/{name}/y")].data;
    let t = &z[&format!("in/{name}/t")].data;
    let m = &meta["sets"][name];
    let track: Vec<String> = m["track"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let mut attributes = vec![
        ("time".to_string(), Column::Texts(t.iter().map(|&s| time::format((s * 1e9).round() as i64)).collect())),
        ("track".to_string(), Column::Texts(track)),
    ];
    for k in ["v1", "v2"] {
        if let Some(a) = z.get(&format!("in/{name}/{k}")) {
            attributes.push((k.to_string(), Column::Numbers(a.data.clone())));
        }
    }
    let geometries =
        x.iter().zip(y).map(|(&a, &b)| if a.is_nan() { Geometry::Empty } else { Geometry::Point([a, b]) }).collect();
    Value::Vector(Arc::new(Vector { geometries, attributes, crs: m["crs"].as_str().map(str::to_string) }))
}

/// move2's fishers as move.read_tracks reads them, with their row (0..) in `row`.
fn fishers() -> Value {
    let path = root().join("tests/data/move/fishers.csv.gz");
    let o = go("move.read_tracks", vec![], serde_json::json!({"path": path.to_string_lossy()}));
    let mut v = (*vector(&o, "tracks")).clone();
    let n = v.len();
    v.attributes.push(("row".into(), Column::Numbers((0..n).map(|i| i as f64).collect())));
    Value::Vector(Arc::new(v))
}

fn without_empty(v: &Value) -> Value {
    let Value::Vector(v) = v else { panic!() };
    let keep: Vec<usize> = (0..v.len()).filter(|&i| !matches!(v.geometries[i], Geometry::Empty)).collect();
    Value::Vector(Arc::new(v.select(&keep)))
}

fn column_of(op: &str, tracks: &Value, params: serde_json::Value, col: &str) -> Vec<f64> {
    num(&vector(&go(op, vec![("tracks", tracks.clone())], params), "tracks"), col)
}

#[test]
fn measures_are_move2s() {
    if !gdal() {
        return;
    }
    let f = without_empty(&fishers());
    for (name, t) in [("ll", set("ll")), ("utm", set("utm")), ("plain", set("plain")), ("fishers", f)] {
        let j = serde_json::json!({});
        same(
            &format!("{name} distance"),
            &column_of("move.distance", &t, j.clone(), "distance"),
            want(&format!("measures_{name}/distance")),
        );
        same(
            &format!("{name} azimuth"),
            &column_of("move.azimuth", &t, j.clone(), "azimuth"),
            want(&format!("measures_{name}/azimuth")),
        );
        same(
            &format!("{name} turn"),
            &column_of("move.turn_angle", &t, j.clone(), "turn_angle"),
            want(&format!("measures_{name}/turn_angle")),
        );
        same(
            &format!("{name} lags"),
            &column_of("move.time_lags", &t, j.clone(), "time_lag"),
            want(&format!("measures_{name}/time_lags")),
        );
        if reference().0.contains_key(&format!("measures_{name}/speed")) {
            same(
                &format!("{name} speed"),
                &column_of("move.speed", &t, j.clone(), "speed"),
                want(&format!("measures_{name}/speed")),
            );
        } else {
            assert!(run("move.speed", vec![("tracks", t.clone())], j).is_err(), "{name}: shared times");
        }
    }
    let (ll, utm) = (set("ll"), set("utm"));
    let u = |op: &str, t: &Value, units: &str, col: &str| column_of(op, t, serde_json::json!({"units": units}), col);
    same("km", &u("move.distance", &ll, "km", "distance"), want("units/km"));
    same("deg", &u("move.azimuth", &ll, "deg", "azimuth"), want("units/deg"));
    same("turn deg", &u("move.turn_angle", &ll, "deg", "turn_angle"), want("units/turn_deg"));
    same("utm km", &u("move.distance", &utm, "km", "distance"), want("units/utm_km"));
    same("utm km/h", &u("move.speed", &utm, "km/h", "speed"), want("units/utm_kmh"));
    same("lag h", &u("move.time_lags", &ll, "h", "time_lag"), want("units/lag_h"));
    // the threads do not change the bits
    terraflux_ops::threads::set_kernel_threads(1);
    same(
        "one thread",
        &column_of("move.distance", &ll, serde_json::json!({}), "distance"),
        want("measures_ll/distance"),
    );
    terraflux_ops::threads::set_kernel_threads(4);
}

#[test]
fn unique_is_move2s() {
    let dup = set("dup");
    let Value::Vector(all) = &dup else { panic!() };
    for (crit, extra) in [
        ("first", serde_json::json!({})),
        ("last", serde_json::json!({})),
        ("subsets", serde_json::json!({})),
        ("subsets_equal", serde_json::json!({})),
        ("sample", serde_json::json!({"seed": 7})),
    ] {
        let mut p = extra;
        p["criterion"] = crit.into();
        let kept = vector(&go("move.filter_unique", vec![("tracks", dup.clone())], p), "tracks");
        let mask = want(&format!("unique/{crit}"));
        let rows: Vec<usize> = (0..mask.len()).filter(|&i| mask[i] != 0.0).collect();
        let w = all.select(&rows);
        assert_eq!(kept.geometries, w.geometries, "{crit}");
        same(&format!("{crit} v1"), &num(&kept, "v1"), &num(&w, "v1"));
        same(&format!("{crit} v2"), &num(&kept, "v2"), &num(&w, "v2"));
    }
}

fn rows_kept(op: &str, t: &Value, p: serde_json::Value, key: &str) {
    let kept = vector(&go(op, vec![("tracks", t.clone())], p), "tracks");
    let mask = want(key);
    let rows: Vec<f64> = (0..mask.len()).filter(|&i| mask[i] != 0.0).map(|i| i as f64).collect();
    same(key, &num(&kept, "row"), &rows);
}

#[test]
fn per_interval_and_visible_are_move2s() {
    let f = fishers();
    let (_, meta) = reference();
    let Value::Vector(v) = &f else { panic!() };
    assert_eq!(v.len() as u64, meta["fishers"]["n"].as_u64().unwrap());
    for (key, unit, crit, seed) in [
        ("hour", "hour", "first", 1),
        ("last", "hour", "last", 1),
        ("m15", "15 mins", "first", 1),
        ("h2", "2 hours", "first", 1),
        ("h5", "5 hours", "first", 1),
        ("day", "day", "first", 1),
        ("d3", "3 days", "first", 1),
        ("week", "week", "first", 1),
        ("month", "month", "first", 1),
        ("random", "day", "random", 3),
    ] {
        rows_kept(
            "move.filter_per_interval",
            &f,
            serde_json::json!({"unit": unit, "criterion": crit, "seed": seed}),
            &format!("per_interval/{key}"),
        );
    }
    rows_kept("move.filter_movebank_visible", &f, serde_json::json!({}), "visible");
}

fn coords(o: &Outputs, key: &str, with_time: bool) {
    let v = vector(o, "tracks");
    let (x, y): (Vec<f64>, Vec<f64>) = v
        .geometries
        .iter()
        .map(|g| match g {
            Geometry::Point(p) => (p[0], p[1]),
            _ => (f64::NAN, f64::NAN),
        })
        .unzip();
    same(&format!("{key} x"), &x, want(&format!("{key}/x")));
    same(&format!("{key} y"), &y, want(&format!("{key}/y")));
    if with_time {
        let t: Vec<f64> = match v.column("time").unwrap() {
            Column::Texts(t) => t.iter().map(|s| time::parse(s).unwrap() as f64 / 1e9).collect(),
            Column::Numbers(t) => t.clone(),
        };
        same(&format!("{key} t"), &t, want(&format!("{key}/t")));
    }
}

#[test]
fn interpolation_is_move2s() {
    if !gdal() {
        return;
    }
    let (_, meta) = reference();
    let (gap, plain) = (set("gap"), set("gap_plain"));
    coords(&go("move.interpolate", vec![("tracks", gap.clone())], serde_json::json!({})), "interpolate_empty", true);
    coords(
        &go(
            "move.interpolate",
            vec![("tracks", gap.clone())],
            serde_json::json!({"times": meta["interpolate_times"], "max_time_lag": 7200}),
        ),
        "interpolate_times",
        true,
    );
    coords(
        &go("move.interpolate", vec![("tracks", gap)], serde_json::json!({"times": "1 hour", "omit": true})),
        "interpolate_hour_omit",
        true,
    );
    coords(
        &go("move.interpolate", vec![("tracks", plain)], serde_json::json!({"times": meta["interpolate_plain"]})),
        "interpolate_plain",
        true,
    );
}

#[test]
fn aeqd_and_brownian_match_the_library() {
    if gdal() {
        let (_, meta) = reference();
        let f = fishers();
        for how in ["centroid", "center"] {
            let o = go("move.aeqd_crs", vec![("tracks", f.clone())], serde_json::json!({"center": how}));
            let Value::Text(crs) = &o["crs"] else { panic!() };
            // pyproj's srs ends in +type=crs
            assert_eq!(crs, meta["aeqd"][how].as_str().unwrap().trim_end_matches(" +type=crs"), "{how}");
        }
    }
    let xy = |o: &Outputs| -> (Vec<f64>, Vec<f64>) {
        vector(o, "tracks")
            .geometries
            .iter()
            .map(|g| match g {
                Geometry::Point(p) => (p[0], p[1]),
                _ => panic!(),
            })
            .unzip()
    };
    let b1 = go(
        "move.simulate_brownian",
        vec![],
        serde_json::json!({"t": [0, 1, 2.5, 4, 8], "sigma": [1, 2, 0, 0.5], "tracks": 3, "seed": 11}),
    );
    let (x, y) = xy(&b1);
    same("brownian x1", &x, want("brownian/x1"));
    same("brownian y1", &y, want("brownian/y1"));
    let (x, y) = xy(&go("move.simulate_brownian", vec![], serde_json::json!({"seed": 5})));
    same("brownian x2", &x, want("brownian/x2"));
    same("brownian y2", &y, want("brownian/y2"));
}

fn steps(t: &Value, p: serde_json::Value) -> Arc<Vector> {
    vector(&go("move.steps", vec![("tracks", t.clone())], p), "steps")
}

#[test]
fn steps_and_bursts_are_amts() {
    if !gdal() {
        return;
    }
    for (name, p) in
        [("utm", serde_json::json!({})), ("plain", serde_json::json!({})), ("ll", serde_json::json!({"lonlat": true}))]
    {
        let s = steps(&set(name), p);
        for c in ["sl_", "direction_p", "ta_", "x2_", "y2_", "dt_"] {
            same(&format!("{name} {c}"), &num(&s, c), want(&format!("steps_{name}/{c}")));
        }
    }
    let utm = set("utm");
    let tracks = |o: Outputs| Value::Vector(vector(&o, "tracks"));
    let a =
        tracks(go("move.resample", vec![("tracks", utm.clone())], serde_json::json!({"rate": 60, "tolerance": 15})));
    let b = tracks(go(
        "move.resample",
        vec![("tracks", utm)],
        serde_json::json!({"rate": "15min", "tolerance": "2min", "start": 3}),
    ));
    let f = tracks(go("move.filter_min_n_burst", vec![("tracks", a.clone())], serde_json::json!({"min_n": 4})));
    for (key, d) in [("a", &a), ("b", &b), ("f", &f)] {
        let Value::Vector(v) = d else { panic!() };
        let t: Vec<f64> = match v.column("time").unwrap() {
            Column::Texts(t) => t.iter().map(|s| time::parse(s).unwrap() as f64 / 1e9).collect(),
            _ => panic!(),
        };
        same(&format!("t_{key}"), &t, want(&format!("resample/t_{key}")));
        same(&format!("burst_{key}"), &num(v, "burst_"), want(&format!("resample/burst_{key}")));
    }
    let s = steps(&a, serde_json::json!({}));
    for c in ["sl_", "direction_p", "ta_", "burst_"] {
        same(&format!("bursts {c}"), &num(&s, c), want(&format!("resample/steps_{c}")));
    }
}

#[test]
fn summaries_are_amts() {
    if !gdal() {
        return;
    }
    for name in ["utm", "ll"] {
        let t = set(name);
        same(
            &format!("{name} nsd"),
            &column_of("move.nsd", &t, serde_json::json!({}), "nsd"),
            want(&format!("summary_{name}/nsd")),
        );
        let o = vector(&go("move.track_summary", vec![("tracks", t)], serde_json::json!({})), "tracks");
        for (c, key) in [
            ("msd", "msd"),
            ("cum_dist", "cum_dist"),
            ("tot_dist", "tot_dist"),
            ("straightness", "straightness"),
            ("sinuosity", "sinuosity"),
            ("tac", "tac"),
        ] {
            same(&format!("{name} {c}"), &num(&o, c), want(&format!("summary_{name}/{key}")));
        }
    }
}

fn table_of(x: &[f64]) -> Value {
    let mut t = Table::new(vec!["x".into()]);
    t.rows = x.iter().map(|v| vec![*v]).collect();
    Value::Table(t)
}

fn params(o: &Outputs) -> Table {
    match &o["table"] {
        Value::Table(t) => t.clone(),
        _ => panic!(),
    }
}

#[test]
fn fits_and_random_numbers_are_amts() {
    if !gdal() {
        return;
    }
    let s = steps(&set("utm"), serde_json::json!({}));
    let track = match s.column("track").unwrap() {
        Column::Texts(t) => t.clone(),
        _ => panic!(),
    };
    let fits = want("fits");
    let mut k = 0;
    for name in ["p1", "p2"] {
        let rows: Vec<usize> = (0..s.len()).filter(|&i| track[i] == name).collect();
        let one = Value::Vector(Arc::new(s.select(&rows)));
        let sl: Vec<f64> = num(&s, "sl_").iter().zip(&track).filter(|(_, t)| *t == name).map(|(v, _)| *v).collect();
        let positive = table_of(&sl.iter().copied().filter(|&v| v > 0.0).collect::<Vec<_>>());
        let fit = |values: &Value, distr: &str, column: &str| {
            go(
                "move.fit_distribution",
                vec![("values", values.clone())],
                serde_json::json!({"name": distr, "column": column}),
            )
        };
        let g = fit(&one, "gamma", "sl_");
        let Value::StepDistribution(gd) = &g["distribution"] else { panic!() };
        let (_, vcov) = gd.vcov.clone().unwrap();
        let v = fit(&one, "vonmises", "ta_");
        let Value::StepDistribution(vd) = &v["distribution"] else { panic!() };
        let got = [
            params(&g).rows[0][0],
            params(&g).rows[0][1],
            vcov[0],
            vcov[1],
            vcov[2],
            vcov[3],
            params(&v).rows[0][0],
            vd.vcov.clone().unwrap().1[0],
            params(&fit(&positive, "exp", "x")).rows[0][0],
            params(&fit(&positive, "lnorm", "x")).rows[0][0],
            params(&fit(&positive, "lnorm", "x")).rows[0][1],
            params(&fit(&one, "hnorm", "sl_")).rows[0][0],
            params(&fit(&one, "unif", "ta_")).rows[0][0],
            params(&fit(&one, "unif", "ta_")).rows[0][1],
        ];
        same(&format!("fits {name}"), &got, &fits[k..k + got.len()]);
        k += got.len();
    }
    let (_, meta) = reference();
    let mut draws = Vec::new();
    for d in meta["random_numbers"].as_array().unwrap() {
        let mut p = d[1].clone();
        p["name"] = d[0].clone();
        let distr = go("move.distribution", vec![], p);
        let o = go(
            "move.random_numbers",
            vec![("distribution", distr["distribution"].clone())],
            serde_json::json!({"n": 300, "seed": 17}),
        );
        draws.extend(params(&o).rows.iter().map(|r| r[0]));
    }
    same("random numbers", &draws, want("random_numbers"));
}

/// Gamma draws on several threads at once are those drawn one after another: R's rgamma() keeps a cache of its
/// constants and the library pointed it to the caller's generator through one global (two draws at once used each
/// other's generator and crashed, about one parallel run of these tests in twenty).
#[test]
fn gamma_draws_on_several_threads_are_the_serial_ones() {
    let draw = |shape: f64, seed: i64| {
        let distr = go("move.distribution", vec![], serde_json::json!({"name": "gamma", "shape": shape, "scale": 3}));
        let o = go(
            "move.random_numbers",
            vec![("distribution", distr["distribution"].clone())],
            serde_json::json!({"n": 2000, "seed": seed}),
        );
        params(&o).rows.iter().map(|r| r[0]).collect::<Vec<f64>>()
    };
    // shapes below and above 1 take rgamma()'s two branches, each with its own cached constants
    let cases: Vec<(f64, i64)> = (0..16).map(|k| ([0.4, 0.9, 1.7, 6.0][k % 4], 100 + k as i64)).collect();
    let serial: Vec<Vec<f64>> = cases.iter().map(|&(s, seed)| draw(s, seed)).collect();
    for _round in 0..5 {
        let parallel: Vec<Vec<f64>> = std::thread::scope(|sc| {
            let hs: Vec<_> = cases.iter().map(|&(s, seed)| sc.spawn(move || draw(s, seed))).collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for (k, (p, s)) in parallel.iter().zip(&serial).enumerate() {
            same(&format!("gamma draws, case {k}"), p, s);
        }
    }
}

#[test]
fn random_steps_are_amts() {
    if !gdal() {
        return;
    }
    let utm = set("utm");
    let s = Value::Vector(steps(&utm, serde_json::json!({})));
    let r = vector(
        &go("move.random_steps", vec![("steps", s.clone())], serde_json::json!({"n_control": 5, "seed": 42})),
        "steps",
    );
    for c in ["x2_", "y2_", "sl_", "ta_", "case_", "step_id_"] {
        same(&format!("random {c}"), &num(&r, c), want(&format!("random_steps/{c}")));
    }
    let exp = go("move.distribution", vec![], serde_json::json!({"name": "exp", "rate": 0.01}));
    let unif = go("move.distribution", vec![], serde_json::json!({"name": "unif"}));
    let u = vector(
        &go(
            "move.random_steps",
            vec![("steps", s), ("sl_distr", exp["distribution"].clone()), ("ta_distr", unif["distribution"].clone())],
            serde_json::json!({"n_control": 3, "seed": 3}),
        ),
        "steps",
    );
    for c in ["x2_", "y2_", "sl_", "ta_"] {
        same(&format!("given {c}"), &num(&u, c), want(&format!("random_steps_given/{c}")));
    }
    let a =
        vector(&go("move.resample", vec![("tracks", utm)], serde_json::json!({"rate": 60, "tolerance": 15})), "tracks");
    let sb = Value::Vector(steps(&Value::Vector(a), serde_json::json!({})));
    let rb = vector(&go("move.random_steps", vec![("steps", sb)], serde_json::json!({"seed": 7})), "steps");
    for c in ["x2_", "y2_", "sl_", "ta_", "case_", "step_id_", "burst_"] {
        same(&format!("bursts {c}"), &num(&rb, c), want(&format!("random_steps_bursts/{c}")));
    }
    // in longitude / latitude: refused
    let ll = Value::Vector(steps(&set("ll"), serde_json::json!({})));
    assert!(run("move.random_steps", vec![("steps", ll)], serde_json::json!({})).is_err());
}

fn tod(v: &Vector, c: &str) -> Vec<f64> {
    let Column::Texts(t) = v.column(c).unwrap() else { panic!() };
    t.iter()
        .map(|s| match s.as_str() {
            "day" => 1.0,
            "dusk" => 2.0,
            "night" => 3.0,
            "dawn" => 4.0,
            _ => f64::NAN,
        })
        .collect()
}

#[test]
fn time_of_day_is_amts() {
    if !gdal() {
        return;
    }
    let ll = set("ll");
    let s = Value::Vector(steps(&ll, serde_json::json!({})));
    let t = |x: &Value, p: serde_json::Value| {
        tod(&vector(&go("move.time_of_day", vec![("tracks", x.clone())], p), "tracks"), "tod")
    };
    same("tod", &t(&ll, serde_json::json!({})), want("time_of_day/tod"));
    same("crep", &t(&ll, serde_json::json!({"include_crepuscule": true})), want("time_of_day/crep"));
    same("end", &t(&s, serde_json::json!({"include_crepuscule": true})), want("time_of_day/end"));
    same("start", &t(&s, serde_json::json!({"where": "start"})), want("time_of_day/start"));
    let f = without_empty(&fishers());
    same("fishers", &t(&f, serde_json::json!({"include_crepuscule": true})), want("time_of_day/fishers"));
}

#[test]
fn covariates_are_amts() {
    if !gdal() {
        return;
    }
    let (_, meta) = reference();
    let e = &meta["extract"];
    let (ny, nx) = (e["nrows"].as_u64().unwrap() as usize, e["ncols"].as_u64().unwrap() as usize);
    let mut g = Grid::new(nx, ny);
    for (k, v) in e["transform"].as_array().unwrap().iter().enumerate() {
        g.transform[k] = v.as_f64().unwrap();
    }
    g.geo.wkt = Some("EPSG:32723".into());
    let every = e["na_every"].as_f64().unwrap();
    let data: Vec<f64> = (1..=ny * nx).map(|i| i as f64).map(|v| if v % every == 0.0 { f64::NAN } else { v }).collect();
    let cover = Value::Map(Map::new(Arc::new(g), data.clone()));
    let utm = set("utm");
    let x = |t: &Value, p: serde_json::Value, c: &str| {
        num(
            &vector(&go("move.extract_covariates", vec![("tracks", t.clone()), ("cover", cover.clone())], p), "tracks"),
            c,
        )
    };
    same("track", &x(&utm, serde_json::json!({}), "cover"), want("extract/track"));
    let s = Value::Vector(steps(&utm, serde_json::json!({})));
    same("both start", &x(&s, serde_json::json!({"where": "both"}), "cover_start"), want("extract/both_start"));
    same("both end", &x(&s, serde_json::json!({"where": "both"}), "cover_end"), want("extract/both_end"));
    same("start", &x(&s, serde_json::json!({"where": "start"}), "cover"), want("extract/start"));
    let r = Value::Vector(vector(
        &go("move.random_steps", vec![("steps", s)], serde_json::json!({"n_control": 5, "seed": 42})),
        "steps",
    ));
    same("random", &x(&r, serde_json::json!({}), "cover"), want("extract/random"));
    // lon/lat tracks over a raster whose coordinate system is only an EPSG code (no WKT, as a download's)
    let mut g = Grid::new(nx, ny);
    for (k, v) in e["transform"].as_array().unwrap().iter().enumerate() {
        g.transform[k] = v.as_f64().unwrap();
    }
    g.geo.geokeys = Some(vec![1, 1, 0, 1, 3072, 0, 1, 32723]);
    let cover = Value::Map(Map::new(Arc::new(g), data));
    let ll = Value::Vector(vector(
        &go("vector.reproject", vec![("vector", utm)], serde_json::json!({"crs": "EPSG:4326"})),
        "vector",
    ));
    let got = num(
        &vector(
            &go("move.extract_covariates", vec![("tracks", ll), ("cover", cover)], serde_json::json!({})),
            "tracks",
        ),
        "cover",
    );
    same("lon/lat over EPSG", &got, want("extract/track"));
}

#[test]
fn lines_segments_and_track_data() {
    let f = without_empty(&fishers());
    let Value::Vector(v) = &f else { panic!() };
    let lines = vector(&go("move.track_lines", vec![("tracks", f.clone())], serde_json::json!({})), "lines");
    let n: f64 = num(&lines, "n_locations").iter().sum();
    assert_eq!(n as usize, v.len());
    assert!(lines.column("individual-taxon-canonical-name").is_some(), "the track's data");
    assert!(lines.column("row").is_none(), "a column that changes");
    let seg = vector(&go("move.segments", vec![("tracks", f.clone())], serde_json::json!({})), "segments");
    let points = seg.geometries.iter().filter(|g| matches!(g, Geometry::Point(_))).count();
    assert_eq!(points, lines.len(), "a point at the end of each track");
    let data = vector(&go("move.track_data", vec![("tracks", f)], serde_json::json!({})), "tracks");
    assert_eq!(data.len(), lines.len());
}

#[test]
fn errors() {
    let plain = set("plain");
    let Value::Vector(v) = &plain else { panic!() };
    // locations out of order
    let mut rows: Vec<usize> = (0..v.len()).collect();
    rows.swap(3, 100);
    let shuffled = Value::Vector(Arc::new(v.select(&rows)));
    assert!(run("move.distance", vec![("tracks", shuffled.clone())], serde_json::json!({})).is_err());
    // as_tracks orders them again
    let sorted = vector(&go("move.as_tracks", vec![("vector", shuffled)], serde_json::json!({})), "tracks");
    assert_eq!(sorted.geometries, v.geometries);
    // empty locations, a column not there, a distance without units
    assert!(run("move.distance", vec![("tracks", set("gap_plain"))], serde_json::json!({})).is_err());
    assert!(run("move.distance", vec![("tracks", plain.clone())], serde_json::json!({"time": "when"})).is_err());
    assert!(run("move.distance", vec![("tracks", plain.clone())], serde_json::json!({"units": "km"})).is_err());
    assert!(run("move.time_of_day", vec![("tracks", plain)], serde_json::json!({})).is_err());
    assert!(run("move.distribution", vec![], serde_json::json!({"name": "gamma", "scale": 2, "rate": 1})).is_err());
    assert!(run("move.distribution", vec![], serde_json::json!({"name": "beta"})).is_err());
}

// cleaning and thinning (move2utils): tests/data/reference/move.npz, keys `clean/...` and `in/m2u/...`, the cases'
// arguments in move.json (`clean`)

fn clean() -> &'static serde_json::Value {
    &reference().1["clean"]
}

/// Tracks of the cleaning cases, with their row (0..) in `row`: a set of the library's tests (`in/m2u/`), or the files
/// of fishers and move2utils' synthetic tracks read as they are (not sorted).
fn clean_set(name: &str) -> Value {
    let (z, meta) = reference();
    let mut v = if let Some(m) = meta["m2u_sets"].get(name) {
        let get = |k: &str| &z[&format!("in/m2u/{name}/{k}")].data;
        let (x, y, t) = (get("x"), get("y"), get("t"));
        let track: Vec<String> =
            m["track"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        let time = t.iter().map(|&s| if s.is_nan() { String::new() } else { time::format((s * 1e9).round() as i64) });
        let mut attributes =
            vec![("time".to_string(), Column::Texts(time.collect())), ("track".to_string(), Column::Texts(track))];
        let prefix = format!("in/m2u/{name}/");
        for (k, a) in z {
            if let Some(c) = k.strip_prefix(&prefix).filter(|c| !["x", "y", "t"].contains(c)) {
                attributes.push((c.to_string(), Column::Numbers(a.data.clone())));
            }
        }
        let geometries = x
            .iter()
            .zip(y)
            .map(|(&a, &b)| if a.is_nan() || b.is_nan() { Geometry::Empty } else { Geometry::Point([a, b]) })
            .collect();
        Vector { geometries, attributes, crs: m["crs"].as_str().map(str::to_string) }
    } else {
        let file = if name == "fishers" { "fishers.csv.gz" } else { "m2u_synthetic.csv.gz" };
        let path = root().join("tests/data/move").join(file);
        let o = go("move.read_tracks", vec![], serde_json::json!({"path": path.to_string_lossy(), "sort": false}));
        (*vector(&o, "tracks")).clone()
    };
    let n = v.len();
    v.attributes.push(("row".into(), Column::Numbers((0..n).map(|i| i as f64).collect())));
    Value::Vector(Arc::new(v))
}

fn no_empty_of(name: &str) -> Value {
    let v = clean_set(name);
    if name != "fishers" {
        return v;
    }
    // the references are of the locations left: their row is their place among them
    let Value::Vector(v) = without_empty(&v) else { panic!() };
    let mut v = (*v).clone();
    let n = v.len();
    set_row(&mut v, n);
    Value::Vector(Arc::new(v))
}

fn set_row(v: &mut Vector, n: usize) {
    let rows = Column::Numbers((0..n).map(|i| i as f64).collect());
    match v.attributes.iter_mut().find(|(c, _)| c == "row") {
        Some((_, c)) => *c = rows,
        None => v.attributes.push(("row".into(), rows)),
    }
}

/// The rows (their `row`) a reference mask keeps.
fn mask_rows(key: &str, rows: &[f64]) -> Vec<f64> {
    let mask = want(key);
    rows.iter().copied().filter(|&r| mask[r as usize] != 0.0).collect()
}

fn number_of(o: &Outputs, port: &str) -> f64 {
    match &o[port] {
        Value::Number(x) => *x,
        v => panic!("{port}: {:?}", v.ty()),
    }
}

#[test]
fn max_speed_is_hirt_et_al() {
    let c = &clean()["v_phys"];
    let masses: Vec<f64> = c["masses"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let (mut v, mut lo, mut hi, mut kmh) = (vec![], vec![], vec![], vec![]);
    for lv in c["levels"].as_array().unwrap() {
        for mode in c["modes"].as_array().unwrap() {
            for &m in &masses {
                let o = go("move.max_speed", vec![], serde_json::json!({"mass": m, "mode": mode, "ci_level": lv}));
                v.push(number_of(&o, "v_max"));
                lo.push(number_of(&o, "lower"));
                hi.push(number_of(&o, "upper"));
                kmh.push(number_of(&o, "kmh"));
                let want_row = vec![m, v[v.len() - 1], lo[lo.len() - 1], hi[hi.len() - 1], kmh[kmh.len() - 1]];
                assert_eq!(params(&o).rows, vec![want_row]);
            }
        }
    }
    same("v_max", &v, want("clean/v_phys/v"));
    same("lower", &lo, want("clean/v_phys/lo"));
    same("upper", &hi, want("clean/v_phys/hi"));
    same("kmh", &kmh, want("clean/v_phys/kmh"));
    // the defaults: flying, 95 %; a beginning of the mode's name; 5 kg, a golden eagle
    let one = |p: serde_json::Value| number_of(&go("move.max_speed", vec![], p), "v_max");
    assert_eq!(one(serde_json::json!({"mass": 5})), one(serde_json::json!({"mass": 5, "mode": "flying"})));
    assert_eq!(
        one(serde_json::json!({"mass": 5, "mode": "run"})),
        one(serde_json::json!({"mass": 5, "mode": "running"}))
    );
    assert_eq!((one(serde_json::json!({"mass": 5})) * 100.0).round(), 3089.0);
    // outside Hirt et al.'s data: a warning, the number all the same
    let mut ctx = Ctx::default();
    let p = serde_json::json!({"mass": 2e5});
    let args = Args { node: "n", inputs: Default::default(), params: p.as_object().unwrap() };
    registry().get("move.max_speed").unwrap().run(&mut ctx, &args).unwrap();
    assert!(ctx.warnings.iter().any(|w| w.contains("extrapolation")), "{:?}", ctx.warnings);
    for bad in [
        serde_json::json!({}),
        serde_json::json!({"mass": 0}),
        serde_json::json!({"mass": -1}),
        serde_json::json!({"mass": 5, "mode": "walking"}),
        serde_json::json!({"mass": 5, "mode": ""}),
        serde_json::json!({"mass": 5, "ci_level": 1}),
        serde_json::json!({"mass": 5, "ci_level": 0}),
    ] {
        assert!(run("move.max_speed", vec![], bad.clone()).is_err(), "{bad}");
    }
}

#[test]
fn flag_speed_is_move2utils() {
    if !gdal() {
        return;
    }
    let cases = clean()["speed"].as_object().unwrap();
    assert!(cases.len() >= 10);
    for (case, c) in cases {
        let t = no_empty_of(c["data"].as_str().unwrap());
        let mut p = serde_json::json!({});
        match c["v_max"].as_f64() {
            Some(v) => p["v_max"] = v.into(),
            None => {
                p["mass"] = c["mass"].clone();
                p["mode"] = c["mode"].clone();
            }
        }
        let jitter = c["jitter"].as_f64();
        if let Some(j) = jitter {
            p["jitter"] = j.into();
        }
        let o = go("move.flag_speed", vec![("tracks", t.clone())], p.clone());
        let out = vector(&o, "tracks");
        let Value::Vector(input) = &t else { panic!() };
        let key = |k: &str| format!("clean/{case}/{k}");
        same(&format!("{case} step_speed"), &num(&out, "step_speed"), want(&key("speed")));
        same(&format!("{case} above"), &num(&out, "is_speed_above_cap"), want(&key("above")));
        if jitter.is_some() {
            same(&format!("{case} below"), &num(&out, "is_step_below_jitter"), want(&key("below")));
        } else {
            assert!(out.column("is_step_below_jitter").is_none(), "{case}");
            assert!(!reference().0.contains_key(&key("below")));
        }
        same(&format!("{case} outlier"), &num(&out, "is_outlier"), want(&key("outlier")));
        same(&format!("{case} flagged"), &num(&out, "flagged_by_speed"), want(&key("flagged")));
        same(&format!("{case} loglr"), &num(&out, "loglr_speed"), want(&key("loglr")));
        assert_eq!(number_of(&o, "v_max").to_bits(), want(&key("vmax"))[0].to_bits(), "{case} v_max");
        // the columns of move2utils, after the input's, in its order
        let names: Vec<&str> = out.attributes.iter().map(|(n, _)| n.as_str()).collect();
        let mut added = vec!["is_outlier", "step_speed", "is_speed_above_cap"];
        if jitter.is_some() {
            added.push("is_step_below_jitter");
        }
        added.extend(["flagged_by_speed", "loglr_speed"]);
        assert_eq!(&names[input.attributes.len()..], added.as_slice(), "{case}");
        assert_eq!(out.geometries, input.geometries);
        // remove: only the locations not flagged
        p["remove"] = true.into();
        let gone = vector(&go("move.flag_speed", vec![("tracks", t)], p), "tracks");
        let outlier = want(&key("outlier"));
        assert_eq!(gone.len(), outlier.iter().filter(|&&f| f == 0.0).count(), "{case} remove");
        assert!(num(&gone, "is_outlier").iter().all(|&f| f == 0.0));
    }
    // the limit of the mass is the allometric maximum, with no margin
    let utm = clean_set("cl_utm");
    let v = number_of(&go("move.max_speed", vec![], serde_json::json!({"mass": 2, "mode": "running"})), "v_max");
    let by_mass =
        go("move.flag_speed", vec![("tracks", utm.clone())], serde_json::json!({"mass": 2, "mode": "running"}));
    let by_limit = go("move.flag_speed", vec![("tracks", utm.clone())], serde_json::json!({"v_max": v}));
    assert_eq!(number_of(&by_mass, "v_max"), v);
    for c in ["step_speed", "loglr_speed", "is_outlier"] {
        same("by mass", &num(&vector(&by_mass, "tracks"), c), &num(&vector(&by_limit, "tracks"), c));
    }
    // errors: both, neither, a mode without a mass, a mass without a mode (no default: a wrong mode is a wrong
    // limit), bad numbers
    for bad in [
        serde_json::json!({"mass": 2}),
        serde_json::json!({"v_max": 3, "mass": 2, "mode": "running"}),
        serde_json::json!({"v_max": 3, "mass": 2}),
        serde_json::json!({"v_max": 3, "mode": "running"}),
        serde_json::json!({"mode": "running"}),
        serde_json::json!({}),
        serde_json::json!({"mass": 2, "mode": "walking"}),
        serde_json::json!({"mass": -2}),
        serde_json::json!({"v_max": 0}),
        serde_json::json!({"v_max": -1}),
        serde_json::json!({"v_max": 1, "jitter": -1}),
    ] {
        assert!(run("move.flag_speed", vec![("tracks", utm.clone())], bad.clone()).is_err(), "{bad}");
    }
    // empty locations are refused
    let e = run("move.flag_speed", vec![("tracks", clean_set("fishers"))], serde_json::json!({"v_max": 1}))
        .map(|_| ())
        .unwrap_err();
    assert!(e.to_string().contains("empty"), "{e}");
    // the tracks one by one give the same numbers
    let Value::Vector(all) = &utm else { panic!() };
    let p = serde_json::json!({"v_max": 0.1});
    let both = vector(&go("move.flag_speed", vec![("tracks", utm.clone())], p.clone()), "tracks");
    let Column::Texts(tr) = all.column("track").unwrap() else { panic!() };
    for name in ["p", "q"] {
        let rows: Vec<usize> = (0..all.len()).filter(|&i| tr[i] == name).collect();
        let one = Value::Vector(Arc::new(all.select(&rows)));
        let part = vector(&go("move.flag_speed", vec![("tracks", one)], p.clone()), "tracks");
        for c in ["step_speed", "is_outlier", "loglr_speed"] {
            let a = num(&both, c);
            same(&format!("{name} {c}"), &num(&part, c), &rows.iter().map(|&i| a[i]).collect::<Vec<_>>());
        }
    }
}

#[test]
fn gps_quality_is_move2utils() {
    for (case, c) in clean()["gps"].as_object().unwrap() {
        let t = clean_set(c["data"].as_str().unwrap());
        let Value::Vector(v) = &t else { panic!() };
        let rows: Vec<f64> = (0..v.len()).map(|i| i as f64).collect();
        for (key, kw) in c["calls"].as_object().unwrap() {
            let kept = vector(&go("move.filter_gps_quality", vec![("tracks", t.clone())], kw.clone()), "tracks");
            same(&format!("{case} {key}"), &num(&kept, "row"), &mask_rows(&format!("clean/{case}/{key}"), &rows));
        }
        assert!(want(&format!("clean/{case}/default")).contains(&0.0), "{case}: it filters something");
    }
    // by hand: numbers as text count, text that is not a number and missing values do not drop a location
    let texts = |v: &[&str]| Column::Texts(v.iter().map(|s| s.to_string()).collect());
    let v = Vector {
        geometries: vec![
            Geometry::Point([0.0, 0.0]),
            Geometry::Point([1.0, 1.0]),
            Geometry::Point([2.0, 2.0]),
            Geometry::Empty,
        ],
        attributes: vec![
            ("time".into(), texts(&["2024-01-01 00:00", "2024-01-01 01:00", "2024-01-01 02:00", "2024-01-01 03:00"])),
            ("id".into(), texts(&["a"; 4])),
            ("gps:satellite-count".into(), texts(&["9", "4", "x", "7"])),
            ("gps:dop".into(), Column::Numbers(vec![1.2, 1.0, 14.0, f64::NAN])),
        ],
        crs: Some("EPSG:32723".into()),
    };
    let n = |v: &Vector, p: serde_json::Value| {
        let t = Value::Vector(Arc::new(v.clone()));
        vector(&go("move.filter_gps_quality", vec![("tracks", t)], p), "tracks").len()
    };
    assert_eq!(n(&v, serde_json::json!({})), 1);
    assert_eq!(n(&v, serde_json::json!({"drop_empty": false})), 2);
    assert_eq!(n(&v, serde_json::json!({"sat_min": null, "dop_max": null, "drop_empty": false})), 4);
    assert_eq!(n(&v, serde_json::json!({"sat_min": "none", "dop_max": "none", "drop_empty": false})), 4);
    assert_eq!(n(&v, serde_json::json!({"sat_min": 4, "dop_max": 14, "drop_empty": false})), 4);
    // the underscore and dash names; no column at all: only the empty locations go
    for names in [["gps_satellite_count", "gps_dop"], ["gps-satellite-count", "gps-dop"]] {
        let mut w = v.clone();
        w.attributes[2].0 = names[0].into();
        w.attributes[3].0 = names[1].into();
        assert_eq!(n(&w, serde_json::json!({})), 1, "{names:?}");
    }
    let mut w = v;
    w.attributes.truncate(2);
    assert_eq!(n(&w, serde_json::json!({})), 3);
}

/// The reference's selection of a thinning, from the set it was made on.
fn thin_check(op: &str, case: &str, t: &Value, p: serde_json::Value) {
    let Value::Vector(v) = t else { panic!() };
    let rows = num(v, "row");
    let key = format!("clean/{case}/selected");
    let selected = vector(&go(op, vec![("tracks", t.clone())], p.clone()), "tracks");
    same(case, &num(&selected, "row"), &mask_rows(&key, &rows));
    // output flag: every location, the column thin_selected
    let mut q = p;
    q["output"] = "flag".into();
    let flagged = vector(&go(op, vec![("tracks", t.clone())], q), "tracks");
    same(
        &format!("{case} flag"),
        &num(&flagged, "thin_selected"),
        &rows.iter().map(|&r| want(&key)[r as usize]).collect::<Vec<_>>(),
    );
    same(&format!("{case} flag rows"), &num(&flagged, "row"), &rows);
}

#[test]
fn thin_time_is_move2utils() {
    if !gdal() {
        return;
    }
    let cases = clean()["thin_time"].as_object().unwrap();
    assert!(cases.len() >= 10);
    for (case, c) in cases {
        let t = clean_set(c["data"].as_str().unwrap());
        let mut p = serde_json::json!({"interval": c["interval"], "criterion": c["criterion"]});
        if !c["tolerance"].is_null() {
            p["tolerance"] = c["tolerance"].clone();
        }
        thin_check("move.thin_time", case, &t, p);
    }
    // seconds, text and the default tolerance (a tenth of the interval)
    let t = clean_set("cl_utm");
    let sel =
        |p: serde_json::Value| num(&vector(&go("move.thin_time", vec![("tracks", t.clone())], p), "tracks"), "row");
    let base = sel(serde_json::json!({"interval": 600, "tolerance": 60}));
    assert!(!base.is_empty() && base.len() < 100);
    for p in [
        serde_json::json!({"interval": "10 mins", "tolerance": "1 min"}),
        serde_json::json!({"interval": "10min", "tolerance": "60 secs"}),
        serde_json::json!({"interval": "600 s", "tolerance": 60}),
        serde_json::json!({"interval": "10 minutes", "tolerance": null}),
        serde_json::json!({"interval": 600}),
        serde_json::json!({"interval": "1/6 hours"}),
    ] {
        if p["interval"] == "1/6 hours" {
            assert!(run("move.thin_time", vec![("tracks", t.clone())], p).is_err());
            continue;
        }
        same(&p.to_string(), &sel(p.clone()), &base);
    }
    // 0.1 hours is 360 s
    same(
        "hours",
        &sel(serde_json::json!({"interval": "0.1 hours", "tolerance": 36})),
        &sel(serde_json::json!({"interval": 360})),
    );
    for bad in [
        serde_json::json!({}),
        serde_json::json!({"interval": 0}),
        serde_json::json!({"interval": -5}),
        serde_json::json!({"interval": "10 min", "tolerance": "11 min"}),
        serde_json::json!({"interval": "10 min", "tolerance": -1}),
        serde_json::json!({"interval": "10 min", "criterion": "all"}),
        serde_json::json!({"interval": "soon"}),
        serde_json::json!({"interval": [1, 2]}),
        serde_json::json!({"interval": true}),
        serde_json::json!({"interval": 600, "output": "all"}),
    ] {
        assert!(run("move.thin_time", vec![("tracks", t.clone())], bad.clone()).is_err(), "{bad}");
    }
}

#[test]
fn thin_distance_is_move2utils() {
    if !gdal() {
        return;
    }
    let cases = clean()["thin_distance"].as_object().unwrap();
    assert!(cases.len() >= 10);
    for (case, c) in cases {
        let t = no_empty_of(c["data"].as_str().unwrap());
        let mut p = serde_json::json!({"distance": c["distance"], "method": c["method"], "criterion": c["criterion"]});
        if !c["tolerance"].is_null() {
            p["tolerance"] = c["tolerance"].clone();
        }
        thin_check("move.thin_distance", case, &t, p);
    }
    // a tolerance given to the step method is ignored, with a warning
    let t = clean_set("cl_utm");
    let p = serde_json::json!({"distance": 200, "tolerance": 30});
    let mut ctx = Ctx::default();
    let inputs = [("tracks".to_string(), t.clone())].into_iter().collect();
    let args = Args { node: "n", inputs, params: p.as_object().unwrap() };
    let a = registry().get("move.thin_distance").unwrap().run(&mut ctx, &args).unwrap();
    assert!(ctx.warnings.iter().any(|w| w.contains("ignored")), "{:?}", ctx.warnings);
    let b = go("move.thin_distance", vec![("tracks", t.clone())], serde_json::json!({"distance": 200}));
    same("ignored", &num(&vector(&a, "tracks"), "row"), &num(&vector(&b, "tracks"), "row"));
    for bad in [
        serde_json::json!({}),
        serde_json::json!({"distance": 0}),
        serde_json::json!({"distance": -1}),
        serde_json::json!({"distance": 10, "method": "greedy"}),
        serde_json::json!({"distance": 10, "method": "interval", "tolerance": 11}),
        serde_json::json!({"distance": 10, "method": "interval", "tolerance": -1}),
        serde_json::json!({"distance": 10, "criterion": "all"}),
        serde_json::json!({"distance": "far"}),
    ] {
        assert!(run("move.thin_distance", vec![("tracks", t.clone())], bad.clone()).is_err(), "{bad}");
    }
    // empty locations are refused
    let e = run("move.thin_distance", vec![("tracks", clean_set("fishers"))], serde_json::json!({"distance": 100}))
        .map(|_| ())
        .unwrap_err();
    assert!(e.to_string().contains("empty"), "{e}");
}

/// Eight long tracks of a random walk with some jumps (more locations than the kernels take to the threads).
fn big_tracks(lonlat: bool) -> Value {
    let mut s = 12345u64;
    let mut next = move || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut times, mut track, mut geometries) = (vec![], vec![], vec![]);
    for k in 0..8 {
        let (mut t, mut x, mut y) = (0i64, if lonlat { 10.0 } else { 5e5 }, if lonlat { 50.0 } else { 5e6 });
        for i in 0..7000 {
            t += [30, 45, 60, 90, 600][(next() * 5.0) as usize];
            let scale = if lonlat { 1e-4 } else { 8.0 };
            let jump = if i % 350 == 7 { 500.0 } else { 1.0 };
            x += (next() - 0.5) * 2.0 * scale * jump;
            y += (next() - 0.5) * 2.0 * scale * jump;
            times.push(time::format((1_714_521_600 + t) * 1_000_000_000));
            track.push(format!("t{k}"));
            geometries.push(Geometry::Point([x, y]));
        }
    }
    let crs = if lonlat { "EPSG:4326" } else { "EPSG:32632" };
    Value::Vector(Arc::new(Vector {
        geometries,
        attributes: vec![("time".into(), Column::Texts(times)), ("track".into(), Column::Texts(track))],
        crs: Some(crs.into()),
    }))
}

#[test]
fn one_and_four_threads_give_the_same_cleaning() {
    if !gdal() {
        return;
    }
    for lonlat in [false, true] {
        let t = big_tracks(lonlat);
        let Value::Vector(v) = &t else { panic!() };
        assert!(v.len() > 20000);
        let calls = [
            ("move.flag_speed", serde_json::json!({"v_max": 1.5, "jitter": 2.0})),
            ("move.thin_time", serde_json::json!({"interval": "20 min", "tolerance": "3 min", "output": "flag"})),
            ("move.thin_time", serde_json::json!({"interval": "5 min", "criterion": "first", "output": "flag"})),
            ("move.thin_distance", serde_json::json!({"distance": 500, "output": "flag"})),
            (
                "move.thin_distance",
                serde_json::json!({"distance": 500, "method": "interval", "tolerance": 100, "output": "flag"}),
            ),
        ];
        for (op, p) in calls {
            let by = |n: usize| {
                terraflux_ops::threads::set_kernel_threads(n);
                let o = vector(&go(op, vec![("tracks", t.clone())], p.clone()), "tracks");
                terraflux_ops::threads::set_kernel_threads(4);
                o
            };
            let (a, b) = (by(1), by(4));
            for c in ["is_outlier", "step_speed", "loglr_speed", "thin_selected"] {
                if let Some(x) = a.column(c) {
                    same(&format!("{op} {c}"), &x.numbers(), &num(&b, c));
                }
            }
            let flag = if op == "move.flag_speed" { "is_outlier" } else { "thin_selected" };
            let on = num(&a, flag).iter().filter(|&&f| f == 1.0).count();
            assert!(on > 0 && on < a.len(), "{op} {p}: {on} of {}", a.len());
        }
    }
}

// the values along the steps (tf.move.extract_covariates_along and along_summary): keys `in/along/...` and `along/...`,
// the rasters and cases in move.json (`along`)

fn along_meta() -> &'static serde_json::Value {
    &reference().1["along"]
}

/// The rasters of the library's ALONG_RASTERS, three layers of integers (cover, lvl, hill), on the exact geotransform
/// the library gave them: (name, map) of each layer.
fn along_layers(raster: &str) -> Vec<(String, Value)> {
    let r = &along_meta()["rasters"][raster];
    let f = |k: &str| r[k].as_f64().unwrap();
    let (nr, nc) = (r["nrows"].as_u64().unwrap() as usize, r["ncols"].as_u64().unwrap() as usize);
    let mut g = Grid::new(nc, nr);
    g.transform = [f("xmin"), f("xres"), 0.0, f("ymax"), 0.0, -f("yres")];
    g.geo.wkt = r["crs"].as_str().map(str::to_string);
    let g = Arc::new(g);
    let (mut cover, mut lvl, mut hill) = (vec![], vec![], vec![]);
    for cell in 0..nr * nc {
        let (row, col) = ((cell / nc) as i64, (cell % nc) as i64);
        cover.push(if (cell + 1) % 13 == 0 { f64::NAN } else { (cell + 1) as f64 });
        lvl.push(if (row + col) % 11 == 0 { f64::NAN } else { ((row * 31 + col * 17) % 101) as f64 });
        let (dr, dc) = (row - (nr / 2) as i64, col - (nc / 2) as i64);
        hill.push((dr * dr + dc * dc) as f64);
    }
    [("cover", cover), ("lvl", lvl), ("hill", hill)]
        .into_iter()
        .map(|(k, d)| (k.to_string(), Value::Map(Map::new(g.clone(), d))))
        .collect()
}

/// The steps of a case of the library, as move.steps gives them (the ends in x1_, y1_, x2_, y2_).
fn along_steps(case: &str) -> Value {
    let z = &reference().0;
    let col = |c: &str| z[&format!("in/along/{case}/{c}")].data.clone();
    let (x1, y1, x2, y2) = (col("x1"), col("y1"), col("x2"), col("y2"));
    let geometries = (0..x1.len()).map(|i| Geometry::LineString(vec![[x1[i], y1[i]], [x2[i], y2[i]]])).collect();
    let attributes = [("x1_", x1), ("y1_", y1), ("x2_", x2), ("y2_", y2)]
        .into_iter()
        .map(|(k, v)| (k.to_string(), Column::Numbers(v)))
        .collect();
    let crs = along_meta()["cases"][case]["crs"].as_str().map(str::to_string);
    Value::Vector(Arc::new(Vector { geometries, attributes, crs }))
}

fn along_run(steps: &Value, layers: &[(String, Value)], params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let mut ins = vec![("tracks", steps.clone())];
    ins.extend(layers.iter().map(|(k, v)| (k.as_str(), v.clone())));
    run("move.extract_covariates", ins, params)
}

fn table(o: &Outputs, port: &str) -> Table {
    match &o[port] {
        Value::Table(t) => t.clone(),
        v => panic!("{port}: {:?}", v.ty()),
    }
}

#[test]
fn values_along_the_steps_are_the_librarys() {
    if !gdal() {
        return;
    }
    let meta = along_meta();
    let stats: Vec<&str> = meta["stats"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
    let names: Vec<&str> = meta["layers"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
    let cases: Vec<String> = meta["cases"].as_object().unwrap().keys().cloned().collect();
    assert!(cases.len() >= 11);
    for case in cases {
        let raster = meta["cases"][&case]["raster"].as_str().unwrap();
        let steps = along_steps(&case);
        let o = along_run(&steps, &along_layers(raster), serde_json::json!({"where": "along"}))
            .unwrap_or_else(|e| panic!("{case}: {e}"));
        let v = vector(&o, "tracks");
        for name in &names {
            for s in &stats {
                same(
                    &format!("{case} {name}_along_{s}"),
                    &num(&v, &format!("{name}_along_{s}")),
                    want(&format!("along/{case}/{name}_{s}")),
                );
            }
        }
        // the table of the values: step (1-based), a column per layer, a row per cell in terra's order
        let t = table(&o, "along");
        // the inputs come in the order of their names
        assert_eq!(t.columns, ["step", "cover", "hill", "lvl"], "{case}");
        let off = want(&format!("along/{case}/off"));
        let n = *off.last().unwrap() as usize;
        assert_eq!(t.rows.len(), n, "{case}");
        let step: Vec<f64> =
            (0..off.len() - 1).flat_map(|i| vec![(i + 1) as f64; (off[i + 1] - off[i]) as usize]).collect();
        same(&format!("{case} step"), &t.rows.iter().map(|r| r[0]).collect::<Vec<_>>(), &step);
        for (k, name) in names.iter().enumerate() {
            let j = t.columns.iter().position(|c| c == name).unwrap();
            same(
                &format!("{case} {name}"),
                &t.rows.iter().map(|r| r[j]).collect::<Vec<_>>(),
                want(&format!("along/{case}/v{}", k + 1)),
            );
        }
        // the steps keep their columns
        assert_eq!(v.len(), off.len() - 1, "{case}");
        assert_eq!(num(&v, "x1_").len(), v.len());
    }
}

#[test]
fn the_stats_setting_picks_and_orders_the_columns() {
    if !gdal() {
        return;
    }
    let steps = along_steps("along_edges");
    let layers = along_layers("edges");
    let columns = |p: serde_json::Value| -> Vec<String> {
        let v = vector(&along_run(&steps, &layers, p).unwrap(), "tracks");
        v.attributes.iter().map(|(n, _)| n.clone()).filter(|n| n.contains("_along_")).collect()
    };
    let both = columns(serde_json::json!({"where": "along", "stats": "ascent, mean"}));
    assert_eq!(
        both,
        [
            "cover_along_ascent",
            "cover_along_mean",
            "hill_along_ascent",
            "hill_along_mean",
            "lvl_along_ascent",
            "lvl_along_mean"
        ]
    );
    let delta = columns(serde_json::json!({"where": "along", "stats": ["delta"]}));
    assert_eq!(delta, ["cover_along_delta", "hill_along_delta", "lvl_along_delta"]);
    assert_eq!(columns(serde_json::json!({"where": "along"})).len(), 18);
    let p = serde_json::json!({"where": "along", "stats": ["max", "descent"]});
    let v = vector(&along_run(&steps, &layers, p).unwrap(), "tracks");
    same("max", &num(&v, "hill_along_max"), want("along/along_edges/hill_max"));
    same("descent", &num(&v, "lvl_along_descent"), want("along/along_edges/lvl_descent"));
    // one layer: the table has the one column
    let one = along_run(&steps, &layers[..1], serde_json::json!({"where": "along", "stats": "mean"})).unwrap();
    assert_eq!(table(&one, "along").columns, ["step", "cover"]);
}

#[test]
fn one_and_four_threads_give_the_same_values_along() {
    if !gdal() {
        return;
    }
    for case in ["along_edges", "along_big", "along_fishers"] {
        let raster = along_meta()["cases"][case]["raster"].as_str().unwrap();
        let (steps, layers) = (along_steps(case), along_layers(raster));
        let by = |n: usize| {
            terraflux_ops::threads::set_kernel_threads(n);
            let o = along_run(&steps, &layers, serde_json::json!({"where": "along"})).unwrap();
            terraflux_ops::threads::set_kernel_threads(4);
            o
        };
        let (a, b) = (by(1), by(4));
        let (va, vb) = (vector(&a, "tracks"), vector(&b, "tracks"));
        let mut checked = 0;
        for (name, c) in &va.attributes {
            if name.contains("_along_") {
                same(&format!("{case} {name}"), &c.numbers(), &num(&vb, name));
                checked += 1;
            }
        }
        assert_eq!(checked, 18, "{case}");
        let (ta, tb) = (table(&a, "along"), table(&b, "along"));
        assert_eq!(ta.rows.len(), tb.rows.len());
        for (k, (ra, rb)) in ta.rows.iter().zip(&tb.rows).enumerate() {
            same(&format!("{case} row {k}"), ra, rb);
        }
    }
}

#[test]
fn values_along_errors() {
    if !gdal() {
        return;
    }
    let steps = along_steps("along_edges");
    let layers = along_layers("edges");
    let err = |s: &Value, l: &[(String, Value)], p: serde_json::Value| {
        along_run(s, l, p).map(|_| ()).unwrap_err().to_string()
    };
    let along = serde_json::json!({"where": "along"});
    // stats: a name that is not one, none at all
    let e = err(&steps, &layers, serde_json::json!({"where": "along", "stats": ["mean", "median"]}));
    assert!(e.contains("stats") && e.contains("median"), "{e}");
    assert!(err(&steps, &layers, serde_json::json!({"where": "along", "stats": []})).contains("stats"));
    assert!(err(&steps, &layers, serde_json::json!({"where": "somewhere"})).contains("along"));
    // points are not steps
    let points = set("utm");
    assert!(err(&points, &layers, along.clone()).contains("steps"));
    // infinite coordinates (terra's extract fails on them)
    let Value::Vector(v) = &steps else { panic!() };
    let mut inf = (**v).clone();
    if let Some((_, Column::Numbers(x))) = inf.attributes.iter_mut().find(|(n, _)| n == "x2_") {
        x[3] = f64::INFINITY;
    }
    assert!(err(&Value::Vector(Arc::new(inf)), &layers, along.clone()).contains("infinite"));
    // layers on two grids; a rotated raster; a raster south up
    let other = along_layers("origin");
    let mixed = vec![layers[0].clone(), ("other".to_string(), other[0].1.clone())];
    assert!(err(&steps, &mixed, along.clone()).contains("share one grid"));
    let tweak = |f: &dyn Fn(&mut Grid)| -> Vec<(String, Value)> {
        layers
            .iter()
            .map(|(k, m)| {
                let Value::Map(m) = m else { panic!() };
                let mut g = (*m.grid).clone();
                f(&mut g);
                (k.clone(), Value::Map(Map::new(Arc::new(g), (*m.data).clone())))
            })
            .collect()
    };
    assert!(err(&steps, &tweak(&|g| g.transform[2] = 0.1), along.clone()).contains("rotated"));
    assert!(err(&steps, &tweak(&|g| g.transform[5] = 1.0), along.clone()).contains("north up"));
}

// corridors and the distance between UDs (move2utils' mt_corridor and emd): keys `in/cor/...`, `corridor/...`,
// `in/emd/...` and `emd/...`, the cases in move.json (`corridor`, `emd`)

fn corridor_meta() -> &'static serde_json::Value {
    &reference().1["corridor"]
}

fn emd_meta() -> &'static serde_json::Value {
    &reference().1["emd"]
}

/// The tracks of a case's data: a set of the reference, or the track M1 of the fishers.
fn corridor_set(data: &str, track: Option<&str>) -> Value {
    if let Some(m) = corridor_meta()["sets"].get(data) {
        let z = &reference().0;
        let get = |k: &str| &z[&format!("in/cor/{data}/{k}")].data;
        let (x, y, t) = (get("x"), get("y"), get("t"));
        let tracks: Vec<String> =
            m["track"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        let time = Column::Texts(t.iter().map(|&s| time::format((s * 1e9).round() as i64)).collect());
        let geometries = x.iter().zip(y).map(|(&a, &b)| Geometry::Point([a, b])).collect();
        return Value::Vector(Arc::new(Vector {
            geometries,
            attributes: vec![("time".to_string(), time), ("track".to_string(), Column::Texts(tracks))],
            crs: m["crs"].as_str().map(str::to_string),
        }));
    }
    match (data, track) {
        ("fishers", Some(one)) => {
            let Value::Vector(v) = clean_set("fishers") else { panic!() };
            let Some(Column::Texts(ids)) = v.column("individual-local-identifier") else { panic!("no track column") };
            let rows: Vec<usize> = (0..v.len()).filter(|&i| ids[i] == one).collect();
            Value::Vector(Arc::new(v.select(&rows)))
        }
        ("utm" | "ll", _) => set(data),
        _ => clean_set(data),
    }
}

fn corridor_run(tracks: &Value, args: &serde_json::Value) -> (Outputs, Vec<String>) {
    let params = args.as_object().cloned().unwrap_or_default();
    let a = Args { node: "n", inputs: [("tracks".to_string(), tracks.clone())].into(), params: &params };
    let mut ctx = Ctx::default();
    let o = registry().get("move.corridor").unwrap().run(&mut ctx, &a).unwrap_or_else(|e| panic!("{args}: {e}"));
    (o, ctx.warnings)
}

fn flag_of(v: &Vector) -> Vec<f64> {
    let Some(Column::Texts(t)) = v.column("corridor") else { panic!("no corridor column") };
    t.iter().map(|s| f64::from(u8::from(s == "corridor"))).collect()
}

#[test]
fn corridors_are_the_librarys() {
    if !gdal() {
        return;
    }
    let cases = corridor_meta()["cases"].as_object().unwrap();
    assert!(cases.len() >= 20);
    let mut flagged = 0.0;
    for (case, c) in cases {
        let tracks = corridor_set(c["data"].as_str().unwrap(), c["track"].as_str());
        let (o, warnings) = corridor_run(&tracks, &c["args"]);
        let v = vector(&o, "tracks");
        let flag = flag_of(&v);
        same(&format!("{case} corridor"), &flag, want(&format!("corridor/{case}/flag")));
        same(&format!("{case} speed"), &num(&v, "corridor_speed"), want(&format!("corridor/{case}/speed")));
        same(&format!("{case} azimuth"), &num(&v, "corridor_azimuth"), want(&format!("corridor/{case}/azimuth")));
        same(&format!("{case} circvar"), &num(&v, "corridor_circvar"), want(&format!("corridor/{case}/circvar")));
        same(&format!("{case} n"), &num(&v, "corridor_n_neighbours"), want(&format!("corridor/{case}/nb")));
        let thr = want(&format!("corridor/{case}/thr"));
        same(&format!("{case} speed threshold"), &[number_of(&o, "speed_threshold")], &thr[..1]);
        same(&format!("{case} circvar threshold"), &[number_of(&o, "circvar_threshold")], &thr[1..]);
        let t = table(&o, "thresholds");
        assert_eq!(t.columns, ["speed_threshold", "circvar_threshold", "min_segments"], "{case}");
        same(&format!("{case} thresholds"), &t.rows[0][..2], thr);
        // a warning for each threshold left out
        let given = c["args"].as_object().unwrap();
        for name in ["speed_threshold", "circvar_threshold"] {
            let said = warnings.iter().any(|w| w.contains(&format!("`{name}` not supplied")));
            assert_eq!(said, !given.contains_key(name), "{case} {name}: {warnings:?}");
        }
        // the lines are the corridor segments, from a location to the next of its track
        let lines = vector(&o, "lines");
        let n_flag = flag.iter().sum::<f64>();
        assert_eq!(lines.len() as f64, n_flag, "{case}");
        let (rows, speed) = (num(&v, "corridor_speed"), num(&lines, "speed"));
        let at: Vec<usize> = (0..v.len()).filter(|&i| flag[i] == 1.0).collect();
        for (k, &i) in at.iter().enumerate() {
            let next = (i + 1..v.len()).find(|&j| !matches!(v.geometries[j], Geometry::Empty)).unwrap();
            let (Geometry::Point(a), Geometry::Point(b)) = (&v.geometries[i], &v.geometries[next]) else { panic!() };
            assert!(matches!(&lines.geometries[k], Geometry::LineString(p) if p == &vec![*a, *b]), "{case} line {k}");
            assert_eq!(speed[k].to_bits(), rows[i].to_bits(), "{case} line speed {k}");
        }
        for col in ["speed", "azimuth", "circvar", "n_neighbours"] {
            assert!(lines.column(col).is_some(), "{case}: no column {col} in the lines");
        }
        assert_eq!(lines.attributes.len(), 6, "{case}: the track, the time and four columns");
        assert_eq!(lines.crs, v.crs, "{case}");
        flagged += n_flag;
    }
    assert!(flagged > 200.0, "{flagged} corridor segments in all");
}

/// Tracks of one animal (a) with the given positions, a time lag of `step` seconds, in `crs`.
fn walk(geometries: Vec<Geometry>, crs: Option<&str>, step: i64) -> Value {
    let n = geometries.len();
    let time = (0..n as i64).map(|i| time::format((1_700_000_000 + i * step) * 1_000_000_000)).collect();
    Value::Vector(Arc::new(Vector {
        geometries,
        attributes: vec![
            ("time".into(), Column::Texts(time)),
            ("track".into(), Column::Texts(vec!["a".to_string(); n])),
        ],
        crs: crs.map(str::to_string),
    }))
}

#[test]
fn corridors_leave_empty_locations_alone_and_warn() {
    if !gdal() {
        return;
    }
    let n = 30;
    let crs = Some("EPSG:32723");
    let points: Vec<Geometry> = (0..n)
        .map(|i| if i == 5 { Geometry::Empty } else { Geometry::Point([10.0 * i as f64, 0.5 * (i % 2) as f64]) })
        .collect();
    let (o, warnings) = corridor_run(&walk(points, crs, 60), &serde_json::json!({}));
    let v = vector(&o, "tracks");
    let Some(Column::Texts(label)) = v.column("corridor") else { panic!() };
    assert_eq!(label[5], "not corridor");
    assert!(num(&v, "corridor_speed")[5].is_nan() && num(&v, "corridor_n_neighbours")[5].is_nan());
    // the segment before the empty location runs to the one after it
    let s = num(&v, "corridor_speed")[4];
    assert!((s - 20.0 / 120.0).abs() < 1e-12 * s, "{s}");
    assert!(num(&v, "corridor_speed")[n - 1].is_nan(), "no segment leaves the last location");
    assert!(warnings.iter().any(|w| w.contains("`speed_threshold` not supplied")), "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("`circvar_threshold` not supplied")), "{warnings:?}");
    // an irregular sampling interval
    let mut t = 1_700_000_000i64;
    let times: Vec<String> = (0..n)
        .map(|i| {
            t += [30, 60, 20000, 3600, 30, 20000, 60][i % 7];
            time::format(t * 1_000_000_000)
        })
        .collect();
    let mut irregular = walk((0..n).map(|i| Geometry::Point([10.0 * i as f64, 0.0])).collect(), crs, 60);
    if let Value::Vector(v) = &mut irregular {
        Arc::make_mut(v).attributes[0].1 = Column::Texts(times);
    }
    let (_, w) = corridor_run(&irregular, &serde_json::json!({"speed_threshold": 0.0, "circvar_threshold": 1.0}));
    assert!(w.iter().any(|w| w.contains("highly irregular")), "{w:?}");
}

#[test]
fn corridors_check_their_input() {
    if !gdal() {
        return;
    }
    let crs = Some("EPSG:32723");
    let straight: Vec<Geometry> = (0..30).map(|i| Geometry::Point([10.0 * i as f64, 0.0])).collect();
    let err = |t: &Value, p: serde_json::Value| {
        run("move.corridor", vec![("tracks", t.clone())], p).map(|_| ()).unwrap_err().to_string()
    };
    let given = serde_json::json!({"speed_threshold": 0.1, "circvar_threshold": 0.1});
    assert!(err(&walk(straight.clone(), None, 60), given.clone()).contains("no CRS"));
    assert!(err(&walk(straight[..1].to_vec(), crs, 60), given.clone()).contains("at least 2"));
    let ok = walk(straight.clone(), crs, 60);
    assert!(err(&ok, serde_json::json!({"speed_threshold": "fast"})).contains("speed_threshold"));
    let lines = vec![Geometry::LineString(vec![[0.0, 0.0], [1.0, 1.0]]); 3];
    assert!(err(&walk(lines, crs, 60), given.clone()).contains("points"));
    // a track that is not consecutive
    let Value::Vector(v) = &ok else { panic!() };
    let mut mixed = (**v).clone();
    if let Some((_, Column::Texts(t))) = mixed.attributes.iter_mut().find(|(n, _)| n == "track") {
        t[3] = "b".into();
    }
    assert!(err(&Value::Vector(Arc::new(mixed)), given.clone()).contains("consecutive"));
    // a variance threshold missing: two segments of which only one has two neighbours (a 20 m segment holding the
    // midpoint of a 4 m one 5 m off), and a fast segment with a variance
    let pts = [[0.0, 0.0], [20.0, 0.0], [13.0, 0.0], [17.0, 0.0]];
    let Value::Vector(base) = walk(pts.iter().map(|&p| Geometry::Point(p)).collect(), crs, 60) else { panic!() };
    let mut two = (*base).clone();
    two.attributes[1].1 = Column::Texts(["a", "a", "b", "b"].map(String::from).to_vec());
    two.attributes[0].1 =
        Column::Texts([0, 60, 0, 60].iter().map(|s| time::format((1_700_000_000 + s) * 1_000_000_000)).collect());
    let two = Value::Vector(Arc::new(two));
    let (o, _) = corridor_run(&two, &serde_json::json!({"speed_threshold": 100.0}));
    let v = vector(&o, "tracks");
    assert_eq!(flag_of(&v).iter().sum::<f64>(), 0.0);
    let counts: Vec<f64> =
        num(&v, "corridor_n_neighbours").iter().map(|x| if x.is_nan() { -1.0 } else { *x }).collect();
    assert_eq!(counts, [2.0, -1.0, 1.0, -1.0]);
    assert!(number_of(&o, "circvar_threshold").is_nan());
    assert!(err(&two, serde_json::json!({"speed_threshold": 0.0})).contains("circular variance threshold is missing"));
}

#[test]
fn one_and_four_threads_give_the_same_corridors() {
    if !gdal() {
        return;
    }
    for lonlat in [false, true] {
        let t = big_tracks(lonlat);
        let by = |n: usize| {
            terraflux_ops::threads::set_kernel_threads(n);
            let (o, _) = corridor_run(&t, &serde_json::json!({}));
            terraflux_ops::threads::set_kernel_threads(4);
            o
        };
        let (a, b) = (by(1), by(4));
        let (va, vb) = (vector(&a, "tracks"), vector(&b, "tracks"));
        for c in ["corridor_speed", "corridor_azimuth", "corridor_circvar", "corridor_n_neighbours"] {
            same(&format!("{lonlat} {c}"), &num(&va, c), &num(&vb, c));
        }
        let (fa, fb) = (flag_of(&va), flag_of(&vb));
        same(&format!("{lonlat} corridor"), &fa, &fb);
        let on = fa.iter().sum::<f64>();
        assert!(on > 10.0 && (on as usize) < fa.len(), "{lonlat}: {on} corridor segments of {}", fa.len());
        same("speed threshold", &[number_of(&a, "speed_threshold")], &[number_of(&b, "speed_threshold")]);
    }
}

/// The UDs of a case as a stack on the grid the reference built (cell centres from the corner and the size).
fn ud_stack(data: &str) -> Value {
    let m = &emd_meta()["uds"][data];
    let f = |k: &str| m[k].as_f64().unwrap();
    let (nx, ny) = (m["nx"].as_u64().unwrap() as usize, m["ny"].as_u64().unwrap() as usize);
    let mut g = Grid::new(nx, ny);
    g.transform = [f("xmin"), f("xres"), 0.0, f("ymax"), 0.0, -f("yres")];
    g.geo.wkt = m["crs"].as_str().map(str::to_string);
    let values = &reference().0[&format!("in/emd/{data}/ud")].data;
    let names: Vec<String> = m["bands"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_eq!(values.len(), names.len() * nx * ny);
    Value::Stack(terraflux_ops::Stack {
        grid: Arc::new(g),
        keys: names.iter().map(|n| terraflux_ops::Key::Name(n.clone())).collect(),
        layers: values.chunks(nx * ny).map(|c| Arc::new(c.to_vec())).collect(),
    })
}

/// One band of the stack as a map.
fn ud_map(stack: &Value, band: usize) -> Value {
    let Value::Stack(s) = stack else { panic!() };
    Value::Map(Map { grid: s.grid.clone(), data: s.layers[band].clone() })
}

fn ud_run(ins: Vec<(&str, Value)>, args: &serde_json::Value) -> terraflux_ops::Result<(Outputs, Vec<String>)> {
    let params = args.as_object().cloned().unwrap_or_default();
    let a = Args { node: "n", inputs: ins.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    let mut ctx = Ctx::default();
    let o = registry().get("move.ud_distance").unwrap().run(&mut ctx, &a)?;
    Ok((o, ctx.warnings))
}

#[test]
fn ud_distances_are_the_librarys() {
    if !gdal() {
        return;
    }
    let cases = emd_meta()["cases"].as_object().unwrap();
    assert!(cases.len() >= 18);
    for (key, c) in cases {
        let data = c["data"].as_str().unwrap();
        let (o, warnings) = ud_run(vec![("ud", ud_stack(data))], &c["args"]).unwrap_or_else(|e| panic!("{key}: {e}"));
        let t = table(&o, "table");
        assert_eq!(t.columns, ["a", "b", "distance"], "{key}");
        same(key, &t.rows.iter().map(|r| r[2]).collect::<Vec<_>>(), want(&format!("emd/{data}/{key}")));
        // the pairs (1, 2), (1, 3), (2, 3), with the names of the bands
        let names = emd_meta()["uds"][data]["bands"].as_array().unwrap();
        let pairs: Vec<(&str, &str)> =
            t.rows.iter().map(|r| (t.label("a", r[0]).unwrap(), t.label("b", r[1]).unwrap())).collect();
        let name = |i: usize| names[i].as_str().unwrap();
        assert_eq!(pairs, [(name(0), name(1)), (name(0), name(2)), (name(1), name(2))], "{key}");
        assert!(!o.contains_key("distance"), "{key}: a number for three UDs");
        // a warning where the exact solver stops, and for degrees without gc
        let hit = warnings.iter().any(|w| w.contains("Maximum number of iterations"));
        assert_eq!(hit, key == "exact_iter3", "{key}: {warnings:?}");
        let degrees = warnings.iter().any(|w| w.contains("longitude/latitude grid"));
        assert_eq!(degrees, key.starts_with("deg"), "{key}: {warnings:?}");
    }
}

#[test]
fn ud_distance_between_two_uds_is_a_number() {
    if !gdal() {
        return;
    }
    let stack = ud_stack("fishers_c");
    let none = serde_json::json!({});
    let (o, _) = ud_run(vec![("ud", ud_map(&stack, 0)), ("ud_b", ud_map(&stack, 2))], &none).unwrap();
    same("pair", &[number_of(&o, "distance")], want("emd/fishers_c/pair"));
    assert_eq!(table(&o, "table").rows.len(), 1);
    // two bands of one stack, in their order
    let Value::Stack(s) = &stack else { panic!() };
    let two = Value::Stack(terraflux_ops::Stack {
        grid: s.grid.clone(),
        keys: s.keys[1..].to_vec(),
        layers: s.layers[1..].to_vec(),
    });
    let (o, _) = ud_run(vec![("ud", two)], &none).unwrap();
    same("two bands", &[number_of(&o, "distance")], &[want("emd/fishers_c/sink")[2]]);
    // a UD against itself: Sinkhorn's entropic distance is small, not 0; the exact one is 0
    let (o, _) = ud_run(vec![("ud", ud_map(&stack, 0)), ("ud_b", ud_map(&stack, 0))], &none).unwrap();
    same("same", &[number_of(&o, "distance")], want("emd/fishers_c/same"));
    let exact = serde_json::json!({"method": "exact"});
    let (o, _) = ud_run(vec![("ud", ud_map(&stack, 0)), ("ud_b", ud_map(&stack, 0))], &exact).unwrap();
    same("same exact", &[number_of(&o, "distance")], want("emd/fishers_c/same_exact"));
    // the maps are ud_1 and ud_2
    let (o, _) = ud_run(vec![("ud", ud_map(&stack, 0)), ("ud_b", ud_map(&stack, 1))], &none).unwrap();
    let t = table(&o, "table");
    assert_eq!((t.label("a", 1.0), t.label("b", 2.0)), (Some("ud_1"), Some("ud_2")));
}

#[test]
fn one_and_four_threads_give_the_same_ud_distances() {
    if !gdal() {
        return;
    }
    let stack = ud_stack("fishers_c");
    let by = |n: usize| {
        terraflux_ops::threads::set_kernel_threads(n);
        let (o, _) = ud_run(vec![("ud", stack.clone())], &serde_json::json!({"mask_quantile": 1})).unwrap();
        terraflux_ops::threads::set_kernel_threads(4);
        table(&o, "table").rows.iter().map(|r| r[2]).collect::<Vec<_>>()
    };
    same("threads", &by(1), &by(4));
}

#[test]
fn ud_distance_errors() {
    if !gdal() {
        return;
    }
    let stack = ud_stack("fishers_c");
    let (f1, m1) = (ud_map(&stack, 0), ud_map(&stack, 2));
    let err = |ins: Vec<(&str, Value)>, p: serde_json::Value| ud_run(ins, &p).map(|_| ()).unwrap_err().to_string();
    let pair = || vec![("ud", f1.clone()), ("ud_b", m1.clone())];
    assert!(err(pair(), serde_json::json!({"method": "lp"})).contains("method"));
    assert!(err(pair(), serde_json::json!({"mask_quantile": 0})).contains("mask_quantile"));
    assert!(err(pair(), serde_json::json!({"mask_quantile": 1.5})).contains("mask_quantile"));
    assert!(err(pair(), serde_json::json!({"reg": -1})).contains("reg"));
    assert!(err(pair(), serde_json::json!({"max_iter": 0})).contains("max_iter"));
    assert!(err(pair(), serde_json::json!({"threshold": 0})).contains("threshold"));
    assert!(err(pair(), serde_json::json!({"method": "exact", "gc": true})).contains("gc"));
    assert!(err(pair(), serde_json::json!({"method": "exact", "threshold": 100})).contains("threshold"));
    assert!(err(vec![("ud", f1.clone())], serde_json::json!({})).contains("at least two"));
    assert!(err(vec![("ud", stack.clone()), ("ud_b", m1.clone())], serde_json::json!({})).contains("one UD each"));
    assert!(err(vec![("ud", Value::Number(1.0)), ("ud_b", m1.clone())], serde_json::json!({})).contains("raster"));
    // another grid
    let Value::Map(m) = &m1 else { panic!() };
    let mut g = (*m.grid).clone();
    g.transform[0] += 1.0;
    let moved = Value::Map(Map::new(Arc::new(g), (*m.data).clone()));
    assert!(err(vec![("ud", f1.clone()), ("ud_b", moved)], serde_json::json!({})).contains("same raster geometry"));
    // a UD without mass
    let Value::Map(m) = &f1 else { panic!() };
    let empty = Value::Map(Map::new(m.grid.clone(), vec![0.0; m.data.len()]));
    assert!(err(vec![("ud", empty), ("ud_b", m1.clone())], serde_json::json!({})).contains("mass"));
}

// ------------------------------------------------------------------------------------------ home ranges

fn hr_meta() -> &'static serde_json::Value {
    &reference().1["hr"]
}

/// A set of the home-range cases: points with their track in a column.
fn hr_set(name: &str) -> Value {
    let (z, _) = reference();
    let m = &hr_meta()["sets"][name];
    let (x, y) = (&z[&format!("in/hr/{name}/x")].data, &z[&format!("in/hr/{name}/y")].data);
    let track: Vec<String> = m["track"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    Value::Vector(Arc::new(Vector {
        geometries: x.iter().zip(y).map(|(&a, &b)| Geometry::Point([a, b])).collect(),
        attributes: vec![("track".to_string(), Column::Texts(track))],
        crs: m["crs"].as_str().map(str::to_string),
    }))
}

fn run_warn(
    op: &str,
    ins: Vec<(&str, Value)>,
    params: &serde_json::Value,
) -> terraflux_ops::Result<(Outputs, Vec<String>)> {
    let params = params.as_object().cloned().unwrap_or_default();
    let a = Args { node: "n", inputs: ins.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    let mut ctx = Ctx::default();
    let o = registry().get(op).unwrap().run(&mut ctx, &a)?;
    Ok((o, ctx.warnings))
}

/// The settings of a case for the operator: the library's, without the ones that say what to compare.
fn hr_params(method: &str, p: &serde_json::Value) -> serde_json::Value {
    let mut p = p.as_object().cloned().unwrap_or_default();
    for k in ["ud", "ud_track", "like"] {
        p.remove(k);
    }
    p.insert("method".into(), method.into());
    serde_json::Value::Object(p)
}

/// The template of a case, as a map on the grid the reference built.
fn hr_like(spec: &serde_json::Value, crs: Option<&str>) -> Value {
    let f = |k: &str| spec[k].as_f64().unwrap();
    let (nx, ny) = (spec["ncol"].as_u64().unwrap() as usize, spec["nrow"].as_u64().unwrap() as usize);
    let mut g = Grid::new(nx, ny);
    g.transform = [f("xmin"), f("res"), 0.0, f("ymax"), 0.0, -f("res")];
    g.geo.wkt = crs.map(str::to_string);
    Value::Map(Map::new(Arc::new(g), vec![0.0; nx * ny]))
}

fn hr_home_range(set: &str, method: &str, p: &serde_json::Value) -> terraflux_ops::Result<(Outputs, Vec<String>)> {
    let mut ins = vec![("tracks", hr_set(set))];
    if let Some(spec) = p.get("like") {
        ins.push(("like", hr_like(spec, hr_meta()["sets"][set]["crs"].as_str())));
    }
    run_warn("move.home_range", ins, &hr_params(method, p))
}

/// A feature's polygons as the library's hr_enc lists them: the polygons, their rings, the vertices.
fn hr_enc(g: &Geometry) -> Vec<f64> {
    let polys: Vec<&Vec<Vec<[f64; 2]>>> = match g {
        Geometry::Polygon(p) => vec![p],
        Geometry::MultiPolygon(ps) => ps.iter().collect(),
        Geometry::Empty => vec![],
        g => panic!("{g:?}"),
    };
    let mut out = vec![polys.len() as f64];
    for p in polys {
        out.push(p.len() as f64);
        for r in p {
            out.push(r.len() as f64);
            out.extend(r.iter().flat_map(|c| *c));
        }
    }
    out
}

/// Within a relative tolerance, NaN where NaN.
fn close(what: &str, got: &[f64], want: &[f64], rtol: f64) {
    assert_eq!(got.len(), want.len(), "{what}: {} values, {} wanted", got.len(), want.len());
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        let ok = (a.is_nan() && b.is_nan()) || a == b || (a - b).abs() <= rtol * a.abs().max(b.abs());
        assert!(ok, "{what}[{k}]: {a:e} != {b:e} (rtol {rtol:e})");
    }
}

/// The polygons of a home range against the library's, to the bit.
fn hr_same_polygons(key: &str, v: &Vector, tolerance: Option<f64>) {
    let (level, area) = (num(v, "level"), num(v, "area"));
    same(&format!("{key} level"), &level, want(&format!("hr/{key}/level")));
    let geoms: Vec<Vec<f64>> = v.geometries.iter().map(hr_enc).collect();
    let glen: Vec<f64> = geoms.iter().map(|g| g.len() as f64).collect();
    let flat: Vec<f64> = geoms.concat();
    match tolerance {
        None => {
            same(&format!("{key} area"), &area, want(&format!("hr/{key}/area")));
            same(&format!("{key} vertices per polygon"), &glen, want(&format!("hr/{key}/glen")));
            same(&format!("{key} vertices"), &flat, want(&format!("hr/{key}/geom")));
        }
        Some(rtol) => {
            close(&format!("{key} area"), &area, want(&format!("hr/{key}/area")), rtol);
            // the same polygons: as many vertices where GEOS's output agrees
            assert_eq!(glen.len(), want(&format!("hr/{key}/glen")).len(), "{key}");
        }
    }
}

#[test]
fn home_ranges_are_the_librarys() {
    if !gdal() {
        return;
    }
    let cases = hr_meta()["cases"].as_object().unwrap();
    assert!(cases.len() >= 25);
    for (key, c) in cases {
        let (set, method, p) = (c["set"].as_str().unwrap(), c["method"].as_str().unwrap(), &c["params"]);
        let (o, warnings) = hr_home_range(set, method, p).unwrap_or_else(|e| panic!("{key}: {e}"));
        let v = vector(&o, "polygons");
        // MCP and KDE to the bit; LoCoH too where the GEOS of the library's shapely and GDAL's agree
        hr_same_polygons(key, &v, None);
        assert_eq!(v.crs, hr_set_crs(set), "{key}");
        let areas = table(&o, "table");
        assert_eq!(areas.columns, ["track", "level", "area"], "{key}");
        same(&format!("{key} table area"), &areas.column("area").unwrap(), &num(&v, "area"));
        assert_eq!(v.attributes.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(), ["track", "level", "what", "area"]);
        if let Column::Texts(w) = v.column("what").unwrap() {
            assert!(w.iter().all(|w| w == "estimate"), "{key}");
        }
        if method == "kde" {
            let h = table(&o, "bandwidths");
            let flat: Vec<f64> = h.rows.iter().flat_map(|r| [r[1], r[2]]).collect();
            same(&format!("{key} h"), &flat, want(&format!("hr/{key}/h")));
            let Value::Stack(ud) = &o["ud"] else { panic!("{key}: no ud") };
            if let Some(track) = p.get("ud_track").and_then(|t| t.as_str()) {
                let at = ud.keys.iter().position(|k| matches!(k, terraflux_ops::Key::Name(n) if n == track)).unwrap();
                same(&format!("{key} ud"), &ud.layers[at], want(&format!("hr/{key}/ud")));
            } else if p.get("ud").is_some() {
                let all: Vec<f64> = ud.layers.iter().flat_map(|l| l.iter().copied()).collect();
                same(&format!("{key} ud"), &all, want(&format!("hr/{key}/ud")));
            }
            let g = &hr_meta()["cases"][key]["grid"];
            assert_eq!(
                (ud.grid.width as u64, ud.grid.height as u64),
                (g[0].as_u64().unwrap(), g[1].as_u64().unwrap()),
                "{key}"
            );
            for l in &ud.layers {
                let total: f64 = l.iter().sum();
                assert!((total - 1.0).abs() < 1e-12, "{key}: the UD sums to {total}");
            }
            // one band for each track, named by it
            let names: Vec<String> = ud
                .keys
                .iter()
                .map(|k| match k {
                    terraflux_ops::Key::Name(n) => n.clone(),
                    _ => panic!(),
                })
                .collect();
            let mut tracks: Vec<String> = hr_meta()["sets"][set]["track"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            tracks.dedup();
            assert_eq!(names, tracks, "{key}");
            assert!(
                warnings.iter().all(|w| !w.contains("did not converge"))
                    || key.contains("lscv")
                    || key.contains("scaled")
            );
        } else {
            assert!(!o.contains_key("ud") && !o.contains_key("bandwidths"), "{key}");
        }
    }
}

fn hr_set_crs(set: &str) -> Option<String> {
    hr_meta()["sets"][set]["crs"].as_str().map(str::to_string)
}

#[test]
fn intensity_of_use_is_the_librarys() {
    if !gdal() {
        return;
    }
    for set in ["hr_utm", "hr_plain", "hr_small", "hr_fishers_s"] {
        let o = go("move.intensity_use", vec![("tracks", hr_set(set))], serde_json::json!({}));
        let t = table(&o, "table");
        assert_eq!(t.columns, ["track", "intensity_use"]);
        same(set, &t.column("intensity_use").unwrap(), want(&format!("hr/iu/{set}")));
    }
}

#[test]
fn overlaps_of_home_ranges_are_the_librarys() {
    if !gdal() {
        return;
    }
    let cases = hr_meta()["overlaps"].as_object().unwrap();
    assert!(cases.len() >= 19);
    for (key, c) in cases {
        let (set, method, p) = (c["set"].as_str().unwrap(), c["method"].as_str().unwrap(), &c["params"]);
        let (home, _) = hr_home_range(set, method, p).unwrap_or_else(|e| panic!("{key}: {e}"));
        let kind = c["type"].as_str().unwrap();
        let mut ins = vec![("polygons", home["polygons"].clone())];
        if let Some(ud) = home.get("ud") {
            ins.push(("ud", ud.clone()));
        }
        let params = serde_json::json!({"type": kind, "conditional": c["conditional"], "which": c["which"]});
        let (o, _) = run_warn("move.home_range_overlap", ins, &params).unwrap_or_else(|e| panic!("{key}: {e}"));
        let t = table(&o, "table");
        assert_eq!(t.columns, ["from", "to", "level", "overlap"], "{key}");
        let col = |c: &str| t.column(c).unwrap();
        let w = |f: &str| want(&format!("hr/overlap/{key}/{f}"));
        same(&format!("{key} from"), &col("from"), w("from"));
        same(&format!("{key} to"), &col("to"), w("to"));
        same(&format!("{key} level"), &col("level"), w("level"));
        if kind == "hr" {
            same(&format!("{key} overlap"), &col("overlap"), w("overlap"));
        } else {
            // the UD is normalised again here, from the normalised one the operator gave: the last digits differ
            close(&format!("{key} overlap"), &col("overlap"), w("overlap"), 1e-12);
        }
        // the tracks' names
        let name = |v: f64| t.label("from", v).unwrap().to_string();
        let tracks = hr_meta()["sets"][set]["track"].as_array().unwrap();
        assert!(tracks.iter().any(|x| x.as_str().unwrap() == name(t.rows[0][0])), "{key}");
    }
}

#[test]
fn degenerate_tracks_succeed_or_fail_as_the_librarys() {
    if !gdal() {
        return;
    }
    let edge = &hr_meta()["edge"];
    let Value::Vector(all) = hr_set("hr_edge") else { panic!() };
    let labels: Vec<String> =
        edge["tracks"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let Some(Column::Texts(track)) = all.column("track") else { panic!() };
    let (mut refused, mut accepted) = (0, 0);
    for label in &labels {
        let rows: Vec<usize> = (0..all.len()).filter(|&i| &track[i] == label).collect();
        let one = Value::Vector(Arc::new(all.select(&rows)));
        for (method, p) in edge["cases"].as_object().unwrap() {
            let r = run_warn("move.home_range", vec![("tracks", one.clone())], &hr_params(method, p));
            let w = want(&format!("hr/edge/{method}/{label}"));
            let what = format!("{method} of {label}");
            if w[0] == 0.0 {
                assert!(r.is_err(), "{what}: the library refuses it");
                refused += 1;
            } else {
                let (o, _) = r.unwrap_or_else(|e| panic!("{what}: {e}"));
                let v = vector(&o, "polygons");
                let n = (w.len() - 1) / 2;
                same(&format!("{what} area"), &num(&v, "area"), &w[1..=n]);
                same(&format!("{what} level"), &num(&v, "level"), &w[n + 1..]);
                accepted += 1;
            }
        }
    }
    assert!(refused >= 1 && accepted >= 10, "{refused} refused, {accepted} accepted");
}

fn lonlat_points() -> Value {
    let pts: Vec<(f64, f64)> = (0..20).map(|i| (10.0 + 0.01 * i as f64, 50.0 + 0.02 * ((i * 7) % 11) as f64)).collect();
    Value::Vector(Arc::new(Vector {
        geometries: pts.iter().map(|&(x, y)| Geometry::Point([x, y])).collect(),
        attributes: vec![("track".to_string(), Column::Texts(vec!["a".to_string(); 20]))],
        crs: Some("EPSG:4326".into()),
    }))
}

#[test]
fn home_range_errors() {
    if !gdal() {
        return;
    }
    let err =
        |op: &str, ins: Vec<(&str, Value)>, p: serde_json::Value| run(op, ins, p).map(|_| ()).unwrap_err().to_string();
    let utm = hr_set("hr_utm");
    let hr = |p: serde_json::Value| err("move.home_range", vec![("tracks", utm.clone())], p);
    // longitude / latitude is refused, for all three
    for op in ["move.home_range", "move.intensity_use"] {
        assert!(err(op, vec![("tracks", lonlat_points())], serde_json::json!({})).contains("longitude / latitude"));
    }
    // settings
    assert!(hr(serde_json::json!({"method": "grid"})).contains("method"));
    assert!(hr(serde_json::json!({"levels": 1.5})).contains("levels"));
    assert!(hr(serde_json::json!({"levels": []})).contains("levels"));
    assert!(hr(serde_json::json!({"levels": "high"})).contains("levels"));
    assert!(hr(serde_json::json!({"track": "nope"})).contains("not found"));
    assert!(hr(serde_json::json!({"method": "kde", "bandwidth": "fast"})).contains("bandwidth"));
    assert!(hr(serde_json::json!({"method": "kde", "bandwidth": [1, 2, 3]})).contains("bandwidth"));
    assert!(hr(serde_json::json!({"method": "kde", "bandwidth": -4})).contains("strictly positive"));
    assert!(hr(serde_json::json!({"method": "kde", "rescale": "z"})).contains("rescale"));
    assert!(hr(serde_json::json!({"method": "kde", "which_min": "any", "bandwidth": "lscv"})).contains("which_min"));
    assert!(hr(serde_json::json!({"method": "kde", "factor": 0.5})).contains("factor"));
    assert!(hr(serde_json::json!({"method": "kde", "res": 0})).contains("res"));
    assert!(hr(serde_json::json!({"method": "kde", "res": 1e-8})).contains("cells"));
    assert!(hr(serde_json::json!({"method": "kde", "levels": 1.0, "bandwidth": "ref_scaled"})).contains("ref_scaled"));
    assert!(hr(serde_json::json!({"method": "locoh", "type": "x"})).contains("type"));
    assert!(hr(serde_json::json!({"method": "locoh", "n": 0})).contains("n:"));
    assert!(hr(serde_json::json!({"method": "locoh", "n": 2.5})).contains("n:"));
    // too few locations: the fewest track has 40 locations here
    assert!(hr(serde_json::json!({"method": "locoh", "n": 40})).contains("less than the number"));
    let Value::Vector(all) = hr_set("hr_edge") else { panic!() };
    let Some(Column::Texts(track)) = all.column("track") else { panic!() };
    let rows: Vec<usize> = (0..all.len()).filter(|&i| track[i] == "e1").collect();
    let single = Value::Vector(Arc::new(all.select(&rows)));
    assert!(
        err("move.home_range", vec![("tracks", single.clone())], serde_json::json!({"method": "kde"}))
            .contains("template")
    );
    assert!(
        err("move.home_range", vec![("tracks", single)], serde_json::json!({"method": "locoh", "n": 1}))
            .contains("less than")
    );
    // a template too small: the contour reaches its edge (or no contour at all)
    let small = serde_json::json!({"xmin": 190000.0, "ymax": 8250000.0, "res": 2.0, "ncol": 10, "nrow": 10});
    let e = err(
        "move.home_range",
        vec![("tracks", utm.clone()), ("like", hr_like(&small, Some("EPSG:32723")))],
        serde_json::json!({"method": "kde"}),
    );
    assert!(e.contains("template") || e.contains("contour"), "{e}");
    let mut g = Grid::new(10, 10);
    g.transform = [0.0, 1.0, 0.0, 10.0, 0.0, 1.0];
    let flipped = Value::Map(Map::new(Arc::new(g), vec![0.0; 100]));
    let e =
        err("move.home_range", vec![("tracks", utm.clone()), ("like", flipped)], serde_json::json!({"method": "kde"}));
    assert!(e.contains("north up"), "{e}");
    let e = err(
        "move.home_range",
        vec![("tracks", utm.clone()), ("like", Value::Number(1.0))],
        serde_json::json!({"method": "kde"}),
    );
    assert!(e.contains("like"), "{e}");
    // empty and other geometries
    let Value::Vector(u) = &utm else { panic!() };
    let mut with_empty = (**u).clone();
    with_empty.geometries[3] = Geometry::Empty;
    let e = err("move.home_range", vec![("tracks", Value::Vector(Arc::new(with_empty)))], serde_json::json!({}));
    assert!(e.contains("empty"), "{e}");
    let mut line = (**u).clone();
    line.geometries[3] = Geometry::LineString(vec![[0.0, 0.0], [1.0, 1.0]]);
    let e = err("move.home_range", vec![("tracks", Value::Vector(Arc::new(line)))], serde_json::json!({}));
    assert!(e.contains("points"), "{e}");
    let mut nan = (**u).clone();
    nan.geometries[3] = Geometry::Point([f64::NAN, 1.0]);
    let e = err("move.intensity_use", vec![("tracks", Value::Vector(Arc::new(nan)))], serde_json::json!({}));
    assert!(e.contains("non-finite"), "{e}");
    let e = err("move.home_range", vec![("tracks", Value::Vector(Arc::new(u.select(&[]))))], serde_json::json!({}));
    assert!(e.contains("no locations"), "{e}");
    assert!(err("move.home_range", vec![], serde_json::json!({})).contains("connect"));
}

#[test]
fn a_track_column_is_optional_and_named_by_the_setting() {
    if !gdal() {
        return;
    }
    let Value::Vector(u) = hr_set("hr_utm") else { panic!() };
    // no column: one track of everything, named 1
    let mut bare = (*u).clone();
    bare.attributes.clear();
    let o = go("move.home_range", vec![("tracks", Value::Vector(Arc::new(bare.clone())))], serde_json::json!({}));
    let v = vector(&o, "polygons");
    assert_eq!(v.len(), 1);
    assert_eq!(num(&v, "track"), [1.0]);
    // the same locations as one track by a column of its own name
    let mut named = bare;
    named.attributes.push(("animal".into(), Column::Numbers(vec![7.0; u.len()])));
    let o2 = go("move.home_range", vec![("tracks", Value::Vector(Arc::new(named.clone())))], serde_json::json!({}));
    let v2 = vector(&o2, "polygons");
    assert_eq!(num(&v2, "animal"), [7.0]);
    same("area", &num(&v, "area"), &num(&v2, "area"));
    let t = table(&o2, "table");
    assert_eq!(t.label("animal", 1.0), Some("7"), "a number label is written as the number");
    // a column given by its name
    named.attributes.push(("who".into(), Column::Texts((0..u.len()).map(|i| format!("w{}", i % 2)).collect())));
    let o3 =
        go("move.home_range", vec![("tracks", Value::Vector(Arc::new(named)))], serde_json::json!({"track": "who"}));
    let v3 = vector(&o3, "polygons");
    assert_eq!(v3.len(), 2);
    assert!(matches!(v3.column("who"), Some(Column::Texts(t)) if t == &["w0", "w1"]));
}

#[test]
fn overlap_errors() {
    if !gdal() {
        return;
    }
    let (home, _) = hr_home_range("hr_utm", "kde", &serde_json::json!({"levels": [0.5, 0.9]})).unwrap();
    let (mcp, _) = hr_home_range("hr_utm", "mcp", &serde_json::json!({"levels": [0.5, 0.9]})).unwrap();
    let (mcp1, _) = hr_home_range("hr_utm", "mcp", &serde_json::json!({"levels": 0.5})).unwrap();
    let err = |ins: Vec<(&str, Value)>, p: serde_json::Value| {
        run_warn("move.home_range_overlap", ins, &p).map(|_| ()).unwrap_err().to_string()
    };
    let (poly, ud) = (home["polygons"].clone(), home["ud"].clone());
    assert!(err(vec![("ud", ud.clone())], serde_json::json!({"type": "hr"})).contains("polygons"));
    assert!(err(vec![("polygons", poly.clone())], serde_json::json!({"type": "vi"})).contains("ud"));
    assert!(err(vec![("polygons", poly.clone())], serde_json::json!({"type": "jaccard"})).contains("type"));
    assert!(err(vec![("polygons", poly.clone())], serde_json::json!({"which": "some"})).contains("which"));
    assert!(
        err(vec![("ud", ud.clone())], serde_json::json!({"type": "vi", "conditional": true, "levels": [2]}))
            .contains("levels")
    );
    // the levels of the tracks must agree: one track with fewer
    let (Value::Vector(a), Value::Vector(b)) = (&mcp["polygons"], &mcp1["polygons"]) else { panic!() };
    let mut mixed = (**a).clone();
    mixed.geometries.truncate(a.len() - 1);
    for (_, c) in &mut mixed.attributes {
        match c {
            Column::Numbers(x) => {
                x.pop();
            }
            Column::Texts(x) => {
                x.pop();
            }
        }
    }
    assert!(err(vec![("polygons", Value::Vector(Arc::new(mixed)))], serde_json::json!({})).contains("levels"));
    // one track alone, a raster that is not a stack, a polygons input that is not features
    let rows: Vec<usize> = (0..b.len()).filter(|&i| i == 0).collect();
    assert!(err(vec![("polygons", Value::Vector(Arc::new(b.select(&rows))))], serde_json::json!({})).contains("two"));
    let Value::Stack(s) = &ud else { panic!() };
    let one = Value::Map(Map { grid: s.grid.clone(), data: s.layers[0].clone() });
    assert!(err(vec![("ud", one)], serde_json::json!({"type": "ba"})).contains("raster with a band"));
    assert!(err(vec![("polygons", Value::Number(1.0))], serde_json::json!({})).contains("features"));
    // the same name twice
    let Value::Vector(p) = &poly else { panic!() };
    let mut dup = (**p).clone();
    if let Some((_, Column::Texts(t))) = dup.attributes.first_mut() {
        for n in t.iter_mut() {
            *n = "same".into();
        }
    }
    assert!(err(vec![("polygons", Value::Vector(Arc::new(dup)))], serde_json::json!({})).contains("two"));
}

#[test]
fn one_and_four_threads_give_the_same_home_ranges() {
    if !gdal() {
        return;
    }
    let by = |n: usize, method: &str, p: serde_json::Value| {
        terraflux_ops::threads::set_kernel_threads(n);
        let r = hr_home_range("hr_small", method, &p);
        terraflux_ops::threads::set_kernel_threads(4);
        let (o, _) = r.unwrap();
        let v = vector(&o, "polygons");
        let mut all = num(&v, "area");
        all.extend(v.geometries.iter().flat_map(hr_enc));
        if let Some(Value::Table(t)) = o.get("bandwidths") {
            all.extend(t.rows.iter().flat_map(|r| r[1..].to_vec()));
        }
        all
    };
    for (method, p) in [
        ("kde", serde_json::json!({"bandwidth": "lscv", "levels": [0.5, 0.95], "res": 6})),
        ("locoh", serde_json::json!({"n": 5, "levels": [0.5, 0.9]})),
        ("locoh", serde_json::json!({"n": 60.0, "type": "a", "levels": [0.5, 0.9]})),
        ("locoh", serde_json::json!({"n": 60.0, "type": "r", "levels": 0.9})),
    ] {
        same(&format!("{method} {p}"), &by(1, method, p.clone()), &by(4, method, p));
    }
}

// ------------------------------------------------------------------------------ the dynamic Brownian bridge

fn ud_meta() -> &'static serde_json::Value {
    &reference().1["ud"]
}

fn ud_crs(set: &str) -> Option<String> {
    ud_meta()["sets"][set]["crs"].as_str().map(str::to_string)
}

fn is_lonlat_set(set: &str) -> bool {
    ud_crs(set).as_deref() == Some("EPSG:4326")
}

/// A set of the bridge's cases: the locations with their columns, the library's `prep` applied (the bursts' column
/// named as move.resample names it, the Argos classes as text instead of the e-obs estimate).
fn ud_set(name: &str, prep: Option<&str>) -> Value {
    let (z, _) = reference();
    let m = &ud_meta()["sets"][name];
    let (x, y, t) =
        (&z[&format!("in/ud/{name}/x")].data, &z[&format!("in/ud/{name}/y")].data, &z[&format!("in/ud/{name}/t")].data);
    let track: Vec<String> = m["track"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let mut attributes = vec![
        ("time".to_string(), Column::Texts(t.iter().map(|&s| time::format((s * 1e9).round() as i64)).collect())),
        ("track".to_string(), Column::Texts(track)),
    ];
    for c in m["columns"].as_array().unwrap() {
        let c = c.as_str().unwrap();
        let data = z[&format!("in/ud/{name}/{c}")].data.clone();
        match (c, prep) {
            ("burst", Some("burst")) => attributes.push(("burst_".into(), Column::Numbers(data))),
            ("eobs_horizontal_accuracy_estimate", Some("argos")) => {}
            _ => attributes.push((c.to_string(), Column::Numbers(data))),
        }
    }
    if prep == Some("argos") {
        let lc = &z[&format!("in/ud/{name}/lc")].data;
        let classes = ["3", "2", "1", "0", "A", "B", "Z"];
        attributes.push((
            "argos_lc".into(),
            Column::Texts(lc.iter().map(|&v| classes[v as usize - 1].to_string()).collect()),
        ));
    }
    let geometries = x.iter().zip(y).map(|(&a, &b)| Geometry::Point([a, b])).collect();
    Value::Vector(Arc::new(Vector { geometries, attributes, crs: ud_crs(name) }))
}

/// The rows of one track of a set of tracks.
fn only(v: &Value, track: &str) -> Value {
    let Value::Vector(v) = v else { panic!() };
    let Some(Column::Texts(t)) = v.column("track") else { panic!() };
    let keep: Vec<usize> = (0..v.len()).filter(|&i| t[i] == track).collect();
    Value::Vector(Arc::new(v.select(&keep)))
}

fn ud_settings(p: &serde_json::Value) -> serde_json::Value {
    let mut p = p.as_object().cloned().unwrap_or_default();
    p.remove("like");
    serde_json::Value::Object(p)
}

fn bridge_ud(o: &Outputs) -> Stack {
    match &o["ud"] {
        Value::Stack(s) => s.clone(),
        v => panic!("ud: {:?}", v.ty()),
    }
}

fn ud_cases_run(
    model: &str,
    set: &str,
    prep: Option<&str>,
    p: &serde_json::Value,
) -> terraflux_ops::Result<(Outputs, Vec<String>)> {
    let mut ins = vec![("tracks", ud_set(set, prep))];
    if let Some(spec) = p.get("like") {
        ins.push(("like", hr_like(spec, ud_crs(set).as_deref())));
    }
    run_warn(&format!("move.{model}_ud"), ins, &ud_settings(p))
}

/// The distribution of a case against the library's: its grid, its bands and every cell (to the bit, or within a
/// relative tolerance of the largest cell).
fn ud_same(key: &str, s: &Stack, tolerance: Option<f64>) {
    let m = &ud_meta()["cases"][key];
    let shape: Vec<usize> = m["shape"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect();
    let bands: Vec<String> = m["bands"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let names: Vec<String> = s
        .keys
        .iter()
        .map(|k| match k {
            terraflux_ops::Key::Name(n) => n.clone(),
            _ => panic!(),
        })
        .collect();
    assert_eq!(names, bands, "{key}: bands");
    assert_eq!((s.layers.len(), s.grid.height, s.grid.width), (shape[0], shape[1], shape[2]), "{key}: shape");
    let t: Vec<f64> = m["transform"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let g = s.grid.transform;
    match tolerance {
        None => same(&format!("{key} transform"), &g, &t),
        Some(rtol) => close(&format!("{key} transform"), &g, &t, rtol),
    }
    let got: Vec<f64> = s.layers.iter().flat_map(|l| l.iter().copied()).collect();
    let w = want(&format!("ud/{key}/ud"));
    match tolerance {
        None => same(key, &got, w),
        Some(_) => {
            let top = w.iter().copied().fold(0.0, f64::max);
            let worst = got.iter().zip(w).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
            eprintln!("{key}: largest difference {:e} of {:e} (relative {:e})", worst, top, worst / top);
            assert!(worst <= tolerance.unwrap() * top, "{key}: {worst:e} off of {top:e}");
        }
    }
    for (k, l) in s.layers.iter().enumerate() {
        let total: f64 = l.iter().sum();
        assert!((total - 1.0).abs() < 1e-9, "{key} band {k} sums to {total}");
    }
}

#[test]
fn bridge_variances_are_the_librarys() {
    if !gdal() {
        return;
    }
    let cases = ud_meta()["var"].as_object().unwrap();
    assert!(cases.len() >= 19);
    for (key, c) in cases {
        let (model, set) = (c["model"].as_str().unwrap(), c["set"].as_str().unwrap());
        let prep = c["prep"].as_str();
        let (o, _) = run_warn(&format!("move.{model}_variance"), vec![("tracks", ud_set(set, prep))], &c["params"])
            .unwrap_or_else(|e| panic!("{key}: {e}"));
        let v = vector(&o, "tracks");
        let columns: &[&str] = match model {
            "dbbmm" => &["motion_variance", "in_windows", "interest", "location_error"],
            _ => &["para_sd", "orth_sd", "n_estim", "seg_interest", "dbgb_location_error"],
        };
        for col in columns {
            let w = want(&format!("ud/var/{key}/{col}"));
            if is_lonlat_set(set) {
                // PROJ's versions differ in the last bits of the projected coordinates
                close(&format!("{key} {col}"), &num(&v, col), w, 1e-6);
            } else {
                same(&format!("{key} {col}"), &num(&v, col), w);
            }
        }
    }
}

#[test]
fn the_local_projection_is_the_librarys() {
    if !gdal() {
        return;
    }
    let mut bits = (0, 0);
    let mut worst = 0.0f64;
    for set in ["ud_ll", "ud_m1_ll"] {
        for how in ["centroid", "center"] {
            let o = go("move.aeqd_crs", vec![("tracks", ud_set(set, None))], serde_json::json!({"center": how}));
            let Value::Text(crs) = &o["crs"] else { panic!() };
            let lib = ud_meta()["aeqd"][format!("{set}/{how}")].as_str().unwrap();
            assert_eq!(crs, lib.trim_end_matches(" +type=crs"), "{set} {how}");
            let v = vector(&o, "tracks");
            let (wx, wy) = (want(&format!("ud/proj/{set}/{how}_x")), want(&format!("ud/proj/{set}/{how}_y")));
            for (i, g) in v.geometries.iter().enumerate() {
                let Geometry::Point(p) = g else { panic!() };
                for (a, b) in [(p[0], wx[i]), (p[1], wy[i])] {
                    bits.0 += usize::from(a.to_bits() == b.to_bits());
                    bits.1 += 1;
                    worst = worst.max((a - b).abs());
                }
            }
        }
    }
    eprintln!("projected coordinates: {} of {} bit for bit, the largest difference {worst:e} m", bits.0, bits.1);
    assert!(worst < 1e-6, "the projected coordinates differ by {worst:e} m");
}

#[test]
fn bridge_distributions_are_the_librarys() {
    if !gdal() {
        return;
    }
    let cases = ud_meta()["cases"].as_object().unwrap();
    for (key, c) in cases {
        let Some(set) = c["set"].as_str() else { continue };
        let (model, prep) = (c["model"].as_str().unwrap(), c["prep"].as_str());
        let (o, _) = ud_cases_run(model, set, prep, &c["params"]).unwrap_or_else(|e| panic!("{key}: {e}"));
        let tolerance = if is_lonlat_set(set) { Some(1e-4) } else { None };
        let s = bridge_ud(&o);
        if key.ends_with("_input") {
            // GDAL's warp picks the grid and PROJ differs: the same grid size, the same distribution within a tolerance
            let want_shape: Vec<usize> =
                c["shape"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect();
            assert_eq!(
                (s.layers.len(), s.grid.height, s.grid.width),
                (want_shape[0], want_shape[1], want_shape[2]),
                "{key}"
            );
            for l in &s.layers {
                assert!((l.iter().sum::<f64>() - 1.0).abs() < 1e-9 && l.iter().all(|v| *v >= 0.0), "{key}");
            }
            let w = want(&format!("ud/{key}/ud"));
            let got: Vec<f64> = s.layers.iter().flat_map(|l| l.iter().copied()).collect();
            let top = w.iter().copied().fold(0.0, f64::max);
            let worst = got.iter().zip(w).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
            eprintln!("{key}: largest difference {worst:e} of {top:e}");
            assert!(worst < 1e-2 * top, "{key}: {worst:e} off of {top:e}");
            continue;
        }
        ud_same(key, &s, tolerance);
    }
}

#[test]
fn a_distribution_from_the_variance_columns_is_the_librarys() {
    if !gdal() {
        return;
    }
    let u1 = only(&ud_set("ud_utm", None), "u1");
    let masks = &ud_meta()["masks"];
    for (model, pre) in [("dbbmm", ""), ("dbgb", "g")] {
        let field = if model == "dbbmm" { "interest" } else { "seg_interest" };
        let vars = serde_json::json!({"location_error": 10, "window_size": 21, "margin": 7});
        let o = go(&format!("move.{model}_variance"), vec![("tracks", u1.clone())], vars);
        let v = o["tracks"].clone();
        same(&format!("{model} before"), &num(&vector(&o, "tracks"), field), want(&format!("ud/mask/{model}/before")));
        // the columns as they are
        let (o, _) =
            run_warn(&format!("move.{model}_ud"), vec![("tracks", v.clone())], &serde_json::json!({"dim_size": 40}))
                .unwrap();
        ud_same(&format!("{pre}ud_columns"), &bridge_ud(&o), None);
        // another error given to the distribution
        let (o, _) = run_warn(
            &format!("move.{model}_ud"),
            vec![("tracks", v.clone())],
            &serde_json::json!({"dim_size": 40, "location_error": 12.0}),
        )
        .unwrap();
        ud_same(&format!("{pre}ud_override"), &bridge_ud(&o), None);
        // masked
        let o = go("move.mask_segments", vec![("tracks", v)], serde_json::json!({"segments": masks["segments"]}));
        same(&format!("{model} after"), &num(&vector(&o, "tracks"), field), want(&format!("ud/mask/{model}/after")));
        let (o2, _) = run_warn(
            &format!("move.{model}_ud"),
            vec![("tracks", o["tracks"].clone())],
            &serde_json::json!({"dim_size": 40}),
        )
        .unwrap();
        ud_same(&format!("{pre}ud_masked"), &bridge_ud(&o2), None);
        // the gaps: the segments whose lag is over three hours
        let u2 = only(&ud_set("ud_utm", None), "u2");
        let vars = serde_json::json!({"location_error": 10, "window_size": 21, "margin": 7});
        let o = go(&format!("move.{model}_variance"), vec![("tracks", u2)], vars.clone());
        same(
            &format!("{model} gap before"),
            &num(&vector(&o, "tracks"), field),
            want(&format!("ud/mask/{model}/gap_before")),
        );
        let gap = go(
            "move.mask_segments",
            vec![("tracks", o["tracks"].clone())],
            serde_json::json!({"gap": masks["gap_hours"]}),
        );
        same(&format!("{model} gap"), &num(&vector(&gap, "tracks"), field), want(&format!("ud/mask/{model}/gap")));
        assert!(!want(&format!("ud/mask/{model}/gap_idx")).is_empty());
        // a list of numbers by track
        let all = go(&format!("move.{model}_variance"), vec![("tracks", ud_set("ud_utm", None))], vars);
        let by = go(
            "move.mask_segments",
            vec![("tracks", all["tracks"].clone())],
            serde_json::json!({"segments": masks["by_track"]}),
        );
        same(
            &format!("{model} by track"),
            &num(&vector(&by, "tracks"), field),
            want(&format!("ud/mask/{model}/by_track")),
        );
        // a column of the tracks
        let mut t = (*vector(&all, "tracks")).clone();
        let before = num(&t, field);
        let mut mark = vec![0.0; t.len()];
        let ones: Vec<usize> = before.iter().enumerate().filter(|(_, v)| **v != 0.0).map(|(i, _)| i).take(3).collect();
        for &i in &ones {
            mark[i] = 1.0;
        }
        t.attributes.push(("skip".into(), Column::Numbers(mark)));
        let c = go(
            "move.mask_segments",
            vec![("tracks", Value::Vector(Arc::new(t)))],
            serde_json::json!({"column": "skip"}),
        );
        let after = num(&vector(&c, "tracks"), field);
        for (i, (a, b)) in before.iter().zip(&after).enumerate() {
            assert_eq!(*b, if ones.contains(&i) { 0.0 } else { *a }, "{model} column row {i}");
        }
    }
}

#[test]
fn volumes_and_outer_probabilities_are_the_librarys() {
    if !gdal() {
        return;
    }
    let m1 = ud_cases_run("dbbmm", "ud_m1", None, &ud_meta()["cases"]["ud_m1_dim"]["params"]).unwrap().0;
    let o = go("move.ud_volume", vec![("ud", m1["ud"].clone())], serde_json::json!({}));
    let Value::Stack(vol) = &o["volume"] else { panic!() };
    same("volume ud_m1_dim", &vol.layers[0], want("ud/volume/ud_m1_dim"));
    let pts = |x: &str, y: &str, crs: Option<String>| {
        let (x, y) = (want(x), want(y));
        Value::Vector(Arc::new(Vector {
            geometries: x.iter().zip(y).map(|(&a, &b)| Geometry::Point([a, b])).collect(),
            attributes: vec![("id".into(), Column::Numbers((0..x.len()).map(|i| i as f64).collect()))],
            crs,
        }))
    };
    let p = pts("ud/outer/ud_m1_dim/x", "ud/outer/ud_m1_dim/y", ud_crs("ud_m1"));
    let o = go("move.ud_outer_probability", vec![("points", p), ("ud", m1["ud"].clone())], serde_json::json!({}));
    same("outer ud_m1_dim", &num(&vector(&o, "points"), "outer_probability"), want("ud/outer/ud_m1_dim"));
    // several bands: by name and by position
    let utm = ud_cases_run("dbbmm", "ud_utm", None, &ud_meta()["cases"]["ud_utm"]["params"]).unwrap().0;
    let o = go("move.ud_volume", vec![("ud", utm["ud"].clone())], serde_json::json!({}));
    let Value::Stack(vol) = &o["volume"] else { panic!() };
    let all: Vec<f64> = vol.layers.iter().flat_map(|l| l.iter().copied()).collect();
    same("volume ud_utm", &all, want("ud/volume/ud_utm"));
    for band in [serde_json::json!("u2"), serde_json::json!(2)] {
        let p = pts("ud/outer/ud_utm/x", "ud/outer/ud_utm/y", ud_crs("ud_utm"));
        let o = go(
            "move.ud_outer_probability",
            vec![("points", p), ("ud", utm["ud"].clone())],
            serde_json::json!({"band": band}),
        );
        same("outer ud_utm", &num(&vector(&o, "points"), "outer_probability"), want("ud/outer/ud_utm"));
    }
    // contours of the volume: nested, the core inside the home range
    let o = go("move.ud_volume", vec![("ud", m1["ud"].clone())], serde_json::json!({"levels": [0.95, 0.5]}));
    let poly = vector(&o, "polygons");
    assert_eq!(poly.len(), 2);
    let (level, area) = (num(&poly, "level"), num(&poly, "area"));
    assert_eq!(level, [0.95, 0.5]);
    assert!(area[0] > area[1] && area[1] > 0.0, "{area:?}");
    let t = table(&o, "table");
    assert_eq!(t.columns, ["band", "level", "area"]);
}

#[test]
fn suggested_windows_are_the_librarys() {
    if !gdal() {
        return;
    }
    let fishers = without_empty(&fishers());
    for (key, hours) in ud_meta()["suggest"].as_object().unwrap() {
        let tracks = if key.starts_with("fishers") { fishers.clone() } else { ud_set("ud_utm", None) };
        let (o, _) =
            run_warn("move.suggest_window", vec![("tracks", tracks)], &serde_json::json!({"target_hours": hours}))
                .unwrap();
        let t = table(&o, "table");
        assert_eq!(t.columns, ["track", "window_size", "margin", "median_dt_secs", "window_hours", "n", "n_windows"]);
        let w = want(&format!("ud/suggest/{key}"));
        assert_eq!(w.len(), 6 * t.rows.len(), "{key}");
        for (i, row) in t.rows.iter().enumerate() {
            for (j, got) in row[1..].iter().enumerate() {
                let b = w[6 * i + j];
                // a track with no suggestion has its count of locations only
                assert!(
                    got.to_bits() == b.to_bits() || (got.is_nan() && b.is_nan()) || (j == 4 && b.is_nan()),
                    "{key} row {i} col {j}: {got} != {b}"
                );
            }
        }
    }
}

#[test]
fn bridge_results_do_not_depend_on_the_threads() {
    if !gdal() {
        return;
    }
    let by = |n: usize, model: &str, set: &str, prep: Option<&str>, p: &serde_json::Value| {
        terraflux_ops::threads::set_kernel_threads(n);
        let var = run_warn(&format!("move.{model}_variance"), vec![("tracks", ud_set(set, prep))], &ud_settings(p));
        let ud = ud_cases_run(model, set, prep, p);
        terraflux_ops::threads::set_kernel_threads(4);
        let mut all = Vec::new();
        let v = vector(&var.unwrap().0, "tracks");
        for c in v.attributes.iter().skip(2) {
            all.extend(c.1.numbers());
        }
        all.extend(bridge_ud(&ud.unwrap().0).layers.iter().flat_map(|l| l.iter().copied()));
        all
    };
    for key in ["ud_burst", "gud_burst", "ud_utm", "gud_utm", "ud_m1_dim", "gud_m1_dim"] {
        let c = &ud_meta()["cases"][key];
        let (model, set, prep) = (c["model"].as_str().unwrap(), c["set"].as_str().unwrap(), c["prep"].as_str());
        same(key, &by(1, model, set, prep, &c["params"]), &by(4, model, set, prep, &c["params"]));
    }
}

#[test]
fn bridge_errors_and_warnings() {
    if !gdal() {
        return;
    }
    let utm = ud_set("ud_utm", None);
    let m1 = ud_set("ud_m1", None);
    let err = |op: &str, ins: Vec<(&str, Value)>, p: serde_json::Value| -> String {
        match run_warn(op, ins, &p) {
            Ok(_) => panic!("{op} {p}: no error"),
            Err(e) => e.to_string(),
        }
    };
    let vars = |extra: serde_json::Value| {
        let mut p = serde_json::json!({"location_error": 10, "window_size": 21, "margin": 7});
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        p
    };
    // a track with fewer locations than the window: a warning with other tracks, an error alone
    let (o, w) = run_warn("move.dbbmm_variance", vec![("tracks", utm.clone())], &vars(serde_json::json!({}))).unwrap();
    assert!(w.iter().any(|m| m.contains("u3") && m.contains("skipped")), "{w:?}");
    let mv = num(&vector(&o, "tracks"), "motion_variance");
    assert!(mv[mv.len() - 9..].iter().all(|v| v.is_nan()));
    let e = err("move.dbbmm_variance", vec![("tracks", only(&utm, "u3"))], vars(serde_json::json!({})));
    assert!(e.contains("cannot be larger than the number of locations (9)"), "{e}");
    assert!(err("move.dbgb_variance", vec![("tracks", only(&utm, "u3"))], vars(serde_json::json!({}))).contains("(9)"));
    // the window and the margin
    for (p, what) in [
        (serde_json::json!({"window_size": 30}), "odd"),
        (serde_json::json!({"window_size": 21, "margin": 11}), "2 * margin + 1"),
        (serde_json::json!({"margin": 0}), "odd"),
    ] {
        let e = err("move.dbbmm_variance", vec![("tracks", utm.clone())], vars(p.clone()));
        assert!(e.contains(what), "{p}: {e}");
    }
    // the location error
    let e = err(
        "move.dbbmm_variance",
        vec![("tracks", utm.clone())],
        vars(serde_json::json!({"location_error": "nothing"})),
    );
    assert!(e.contains("names a column"), "{e}");
    let e =
        err("move.dbbmm_variance", vec![("tracks", utm.clone())], vars(serde_json::json!({"location_error": -1.0})));
    assert!(e.contains("non-negative"), "{e}");
    let e = err(
        "move.dbbmm_variance",
        vec![("tracks", utm.clone())],
        vars(serde_json::json!({"location_na": "x", "location_error_na": "mode"})),
    );
    assert!(e.contains("location_error_na"), "{e}");
    let mut bad = (*vector(
        &go("move.dbbmm_variance", vec![("tracks", only(&utm, "u1"))], vars(serde_json::json!({}))),
        "tracks",
    ))
    .clone();
    bad.attributes.push(("sigma".into(), Column::Numbers(vec![f64::NAN; bad.len()])));
    let e = err(
        "move.dbbmm_variance",
        vec![("tracks", Value::Vector(Arc::new(bad.clone())))],
        vars(serde_json::json!({"location_error": "sigma"})),
    );
    assert!(e.contains("entirely missing"), "{e}");
    let e = err(
        "move.dbbmm_variance",
        vec![("tracks", Value::Vector(Arc::new(bad)))],
        vars(serde_json::json!({"location_error": "sigma", "location_error_na": "approx"})),
    );
    assert!(e.contains("entirely missing") || e.contains("two known"), "{e}");
    let (_, w) = run_warn(
        "move.dbbmm_variance",
        vec![("tracks", utm.clone())],
        &vars(serde_json::json!({"location_error": "auto"})),
    )
    .unwrap();
    assert!(w.iter().any(|m| m.contains("no recognised quality column")), "{w:?}");
    // the same time twice, an empty location, no time
    let mut twice = (*vector(
        &go("move.dbbmm_variance", vec![("tracks", only(&utm, "u1"))], vars(serde_json::json!({}))),
        "tracks",
    ))
    .clone();
    if let Some((_, Column::Texts(t))) = twice.attributes.iter_mut().find(|(n, _)| n == "time") {
        t[5] = t[4].clone();
    }
    let e = err("move.dbbmm_variance", vec![("tracks", Value::Vector(Arc::new(twice)))], vars(serde_json::json!({})));
    assert!(e.contains("share their time"), "{e}");
    let mut empty = (*vector(
        &go("move.dbbmm_variance", vec![("tracks", only(&utm, "u1"))], vars(serde_json::json!({}))),
        "tracks",
    ))
    .clone();
    empty.geometries[3] = Geometry::Empty;
    let e = err("move.dbbmm_variance", vec![("tracks", Value::Vector(Arc::new(empty)))], vars(serde_json::json!({})));
    assert!(e.contains("empty"), "{e}");
    // the grid
    let ud = |p: serde_json::Value, like: Option<Value>| {
        let mut ins = vec![("tracks", m1.clone())];
        if let Some(l) = like {
            ins.push(("like", l));
        }
        run_warn("move.dbbmm_ud", ins, &p).map(|r| r.0).map_err(|e| e.to_string())
    };
    let e = ud(serde_json::json!({"location_error": 25, "res": 300, "dim_size": 50}), None).unwrap_err();
    assert!(e.contains("give only one of"), "{e}");
    let small = hr_like(
        &serde_json::json!({"xmin": -500.0, "ymax": 500.0, "res": 100.0, "ncol": 10, "nrow": 10}),
        ud_crs("ud_m1").as_deref(),
    );
    let e = ud(serde_json::json!({"location_error": 25}), Some(small.clone())).unwrap_err();
    assert!(e.contains("like is too small") && e.contains("A template is never enlarged"), "{e}");
    let e = ud(serde_json::json!({"location_error": 25, "res": 300, "max_cells": 100}), None).unwrap_err();
    assert!(e.contains("above the limit"), "{e}");
    for (p, what) in [
        (serde_json::json!({"dim_size": 1}), "dim_size"),
        (serde_json::json!({"res": -5.0}), "res"),
        (serde_json::json!({"ext_factor": -1.0}), "ext_factor"),
        (serde_json::json!({"crs": "native"}), "crs"),
        (serde_json::json!({"time_step": -2.0}), "time_step"),
    ] {
        let e = ud(p.clone(), None).unwrap_err();
        assert!(e.contains(what), "{p}: {e}");
    }
    // a template in another coordinate system
    let wgs = hr_like(
        &serde_json::json!({"xmin": -80.0, "ymax": 43.0, "res": 0.01, "ncol": 50, "nrow": 50}),
        Some("EPSG:4326"),
    );
    let e = ud(serde_json::json!({"location_error": 25}), Some(wgs)).unwrap_err();
    assert!(e.contains("projected in metres"), "{e}");
    // the dBGB needs a template as large as its windows
    let e = run_warn(
        "move.dbgb_ud",
        vec![("tracks", m1.clone()), ("like", small)],
        &serde_json::json!({"location_error": 25}),
    )
    .map(|_| ())
    .unwrap_err()
    .to_string();
    assert!(e.contains("like is too small"), "{e}");
    // the enlarged grid is told with verbose only
    let p = serde_json::json!({"location_error": 25, "dim_size": 40, "ext_factor": 0});
    let (_, w) = run_warn("move.dbbmm_ud", vec![("tracks", m1.clone())], &p).unwrap();
    assert!(w.iter().all(|m| !m.contains("enlarged")), "{w:?}");
    let p = serde_json::json!({"location_error": 25, "dim_size": 40, "ext_factor": 0, "verbose": true});
    let (_, w) = run_warn("move.dbbmm_ud", vec![("tracks", m1.clone())], &p).unwrap();
    assert!(w.iter().any(|m| m.contains("enlarged")), "{w:?}");
    // a track the variance skipped has no band
    let (o, w) = run_warn(
        "move.dbbmm_ud",
        vec![("tracks", utm.clone())],
        &serde_json::json!({"location_error": 10, "window_size": 21, "margin": 7, "dim_size": 30}),
    )
    .unwrap();
    assert_eq!(bridge_ud(&o).layers.len(), 2);
    assert!(w.iter().any(|m| m.contains("u3")), "{w:?}");
    // the masks
    let v = go("move.dbbmm_variance", vec![("tracks", only(&utm, "u1"))], vars(serde_json::json!({})));
    let t = v["tracks"].clone();
    let mask = |p: serde_json::Value| err("move.mask_segments", vec![("tracks", t.clone())], p);
    assert!(mask(serde_json::json!({})).contains("segments, gap or column"));
    assert!(mask(serde_json::json!({"segments": [5000]})).contains("integers in 0.."));
    assert!(mask(serde_json::json!({"segments": [-1]})).contains("integers in 0.."));
    assert!(mask(serde_json::json!({"segments": {"nobody": [1]}})).contains("names no track"));
    assert!(mask(serde_json::json!({"segments": [1], "model": "dbgb"})).contains("no seg_interest column"));
    assert!(mask(serde_json::json!({"column": "nothing"})).contains("not found"));
    assert!(mask(serde_json::json!({"gap": -1.0})).contains("gap"));
    let e = err("move.mask_segments", vec![("tracks", only(&utm, "u1"))], serde_json::json!({"segments": [1]}));
    assert!(e.contains("no interest column"), "{e}");
    // the outer probability and the volume
    let two = go(
        "move.dbbmm_ud",
        vec![("tracks", utm.clone())],
        serde_json::json!({"location_error": 10, "window_size": 21, "margin": 7, "dim_size": 30}),
    );
    let pts = Value::Vector(Arc::new(Vector {
        geometries: vec![Geometry::Point([0.0, 0.0])],
        attributes: vec![],
        crs: None,
    }));
    let e = err(
        "move.ud_outer_probability",
        vec![("points", pts.clone()), ("ud", two["ud"].clone())],
        serde_json::json!({}),
    );
    assert!(e.contains("several bands"), "{e}");
    let e = err(
        "move.ud_outer_probability",
        vec![("points", pts.clone()), ("ud", two["ud"].clone())],
        serde_json::json!({"band": "zz"}),
    );
    assert!(e.contains("is not one of"), "{e}");
    let e = err(
        "move.ud_outer_probability",
        vec![("points", pts), ("ud", two["ud"].clone())],
        serde_json::json!({"band": 3}),
    );
    assert!(e.contains("position from 1 to 2"), "{e}");
    let e = err("move.ud_volume", vec![("ud", two["ud"].clone())], serde_json::json!({"levels": [1.5]}));
    assert!(e.contains("levels"), "{e}");
    // the window suggestion: a track too short
    let (o, w) = run_warn("move.suggest_window", vec![("tracks", only(&utm, "u3"))], &serde_json::json!({})).unwrap();
    assert!(w.iter().any(|m| m.contains("too few locations")), "{w:?}");
    assert!(table(&o, "table").rows[0][1].is_nan());
    let e = err("move.suggest_window", vec![("tracks", utm)], serde_json::json!({"target_hours": 0}));
    assert!(e.contains("target_hours"), "{e}");
}

#[test]
fn bursts_make_bands_and_a_burst_column_can_be_named() {
    if !gdal() {
        return;
    }
    let b = ud_set("ud_burst", Some("burst"));
    let p = serde_json::json!({"location_error": 5, "window_size": 21, "margin": 7, "dim_size": 30});
    let (o, _) = run_warn("move.dbbmm_ud", vec![("tracks", b.clone())], &p).unwrap();
    let names: Vec<String> = bridge_ud(&o)
        .keys
        .iter()
        .map(|k| match k {
            terraflux_ops::Key::Name(n) => n.clone(),
            _ => panic!(),
        })
        .collect();
    assert_eq!(names, ["b1__1", "b1__2", "b2__1", "b2__2"]);
    // no bursts: a band for each track
    let mut q = p.clone();
    q["burst"] = "none".into();
    let (o, _) = run_warn("move.dbbmm_ud", vec![("tracks", b.clone())], &q).unwrap();
    assert_eq!(bridge_ud(&o).layers.len(), 2);
    // a column named by its name
    let Value::Vector(v) = &b else { panic!() };
    let mut v = (**v).clone();
    let col = v.column("burst_").unwrap().clone();
    v.attributes.retain(|(n, _)| n != "burst_");
    v.attributes.push(("part".into(), col));
    let mut q = p.clone();
    q["burst"] = "part".into();
    let (o2, _) = run_warn("move.dbbmm_ud", vec![("tracks", Value::Vector(Arc::new(v.clone())))], &q).unwrap();
    let a = bridge_ud(&o2);
    assert_eq!(a.layers.len(), 4);
    let (o1, _) = run_warn("move.dbbmm_ud", vec![("tracks", b)], &p).unwrap();
    for (x, y) in a.layers.iter().zip(&bridge_ud(&o1).layers) {
        same("named burst column", x, y);
    }
    q["burst"] = "missing".into();
    assert!(run_warn("move.dbbmm_ud", vec![("tracks", Value::Vector(Arc::new(v)))], &q).is_err());
}

#[test]
fn a_distribution_from_the_columns_of_longitude_latitude_tracks_stays_in_their_projection() {
    if !gdal() {
        return;
    }
    let ll = ud_set("ud_ll", None);
    let v = go("move.dbbmm_variance", vec![("tracks", ll.clone())], serde_json::json!({"location_error": 5}));
    let (o, _) =
        run_warn("move.dbbmm_ud", vec![("tracks", v["tracks"].clone())], &serde_json::json!({"dim_size": 40})).unwrap();
    let s = bridge_ud(&o);
    assert_eq!(s.layers.len(), 2);
    for l in &s.layers {
        assert!((l.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }
    // the grid is in the projection of the centroid, in metres, and the bands are those of the tracks
    let wkt = s.grid.geo.wkt.clone().unwrap();
    assert!(wkt.contains("Azimuthal_Equidistant") || wkt.contains("Azimuthal Equidistant"), "{wkt}");
    let centroid = go("move.aeqd_crs", vec![("tracks", ll)], serde_json::json!({"center": "centroid"}));
    let Value::Text(crs) = &centroid["crs"] else { panic!() };
    assert!(terraflux_gdal::same_crs(&wkt, crs).unwrap(), "{wkt} vs {crs}");
}
