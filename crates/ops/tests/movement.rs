//! Movement ecology (`move.*`) against landschaft's ls.move (tests/data/reference/move.npz and .json,
//! tools/make_move_reference.py): the track sets of landschaft's tests and move2's fishers, every number to the bit.

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
fn aeqd_and_brownian_are_landschafts() {
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
