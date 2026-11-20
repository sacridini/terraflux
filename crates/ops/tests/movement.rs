//! Movement ecology (`move.*`) against the library's tf.move (tests/data/reference/move.npz and .json,
//! tools/make_move_reference.py): the track sets of the library's tests and move2's fishers, every number to the bit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use terraflux_ops::movement::time;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
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
