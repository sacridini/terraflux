//! `objects.series` and `raster.phenology`: the Savitzky-Golay filter equal to scipy's and the harmonics to numpy's
//! least squares (tests/data/reference/phenology.json, tools/make_phenology_reference.py), curves whose season, peak,
//! amplitude, phase and integral are known, clouds, the same numbers for objects and cells, any number of threads, and
//! the refusals.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::objects::Objects;
use terraflux_ops::phenology::{self, PHENOLOGY, Settings, Smooth};
use terraflux_ops::{Args, Ctx, Key, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::Grid;

fn run_with(
    op: &str,
    inputs: Vec<(&str, Value)>,
    params: serde_json::Value,
) -> (terraflux_ops::Result<Outputs>, Vec<String>) {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    let mut ctx = Ctx::default();
    let out = r.get(op).unwrap().run(&mut ctx, &args);
    (out, ctx.warnings)
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    run_with(op, inputs, params).0
}

fn reference() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/phenology.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn numbers(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect()
}

fn close(what: &str, got: f64, want: f64, tol: f64) {
    assert!((got.is_nan() && want.is_nan()) || (got - want).abs() <= tol * want.abs().max(1.0), "{what}: {got} {want}");
}

/// The days since 1970-01-01 of a date.
fn day(y: i64, m: i64, d: i64) -> f64 {
    terraflux_ops::swat_weather::days_from_civil(y, m, d) as f64
}

fn name_of(d: f64) -> String {
    let (y, m, dd) = terraflux_ops::swat_weather::civil_from_days(d as i64);
    format!("{y:04}-{m:02}-{dd:02}")
}

/// The measures of a series by name.
fn pheno(t: &[f64], y: &[f64], s: &Settings) -> std::collections::BTreeMap<&'static str, f64> {
    PHENOLOGY.iter().copied().zip(phenology::phenology(t, y, s)).collect()
}

#[test]
fn savgol_is_scipys_and_the_harmonics_are_numpys() {
    let z = reference();
    for case in z["savgol"].as_array().unwrap() {
        let (x, want) = (numbers(&case["x"]), numbers(&case["y"]));
        let (w, o) = (case["window"].as_u64().unwrap() as usize, case["order"].as_u64().unwrap() as usize);
        let got = phenology::savgol_filter(&x, w, o).unwrap();
        for (i, (a, b)) in got.iter().zip(&want).enumerate() {
            close(&format!("savgol {w} {o} at {i}"), *a, *b, 1e-9);
        }
    }
    let h = &z["harmonics"];
    let (t, y) = (numbers(&h["t"]), numbers(&h["y"]));
    assert!(y.iter().any(|v| v.is_nan()), "the series has clouds");
    for fit in h["fits"].as_array().unwrap() {
        let (n, trend) = (fit["n"].as_u64().unwrap() as usize, fit["trend"].as_bool().unwrap());
        let want = numbers(&fit["measures"]);
        let got = phenology::harmonics(&t, &y, n, trend);
        assert_eq!(got.len(), phenology::harmonic_names(n, trend).len());
        for (k, (a, b)) in got.iter().zip(&want).enumerate() {
            close(&format!("harmonics n {n} trend {trend} measure {k}"), *a, *b, 1e-9);
        }
    }
    // too few values: 2n + 2 (2n + 3 with a trend)
    let few: Vec<f64> = y.iter().copied().filter(|v| v.is_finite()).take(5).collect();
    let tf: Vec<f64> = t.iter().take(5).copied().collect();
    assert!(phenology::harmonics(&tf, &few, 2, false).iter().all(|v| v.is_nan()));
    assert!(phenology::harmonics(&tf, &few, 1, false).iter().all(|v| v.is_finite()));
    assert!(phenology::harmonics(&tf, &few, 1, true).iter().all(|v| v.is_finite()));
    assert!(phenology::harmonics(&tf[..4], &few[..4], 1, true).iter().all(|v| v.is_nan()));
}

#[test]
fn a_sine_gives_its_amplitude_phase_peak_and_mean() {
    // a year of ten-day dates, a harmonic peaking on the 200th day after 1 January
    let jan1 = day(2023, 1, 1);
    let t: Vec<f64> = (0..37).map(|k| jan1 + 10.0 * k as f64).collect();
    let w = 2.0 * std::f64::consts::PI / 365.25;
    let y: Vec<f64> = t.iter().map(|d| 0.4 + 0.3 * (w * (d - jan1 - 200.0)).cos()).collect();
    let m = phenology::harmonics(&t, &y, 1, false);
    // hmean, amp1, phase1, rmse
    close("mean", m[0], 0.4, 1e-12);
    close("amplitude", m[1], 0.3, 1e-12);
    close("phase", m[2], w * 200.0, 1e-12);
    assert!(m[3] < 1e-12, "rmse {}", m[3]);
    // the second harmonic of a pure first has none, and a trend of none
    let m = phenology::harmonics(&t, &y, 2, true);
    close("trend", m[1], 0.0, 1e-9);
    assert!(m[4] < 1e-9, "second harmonic {}", m[4]);
    // the phenology: the peak is the 201st day of the year, the maximum is the sine's
    let p = pheno(&t, &y, &Settings::default());
    assert_eq!(p["peak_doy"], 201.0);
    assert_eq!(p["peak_date"], jan1 + 200.0);
    close("max", p["max"], 0.7, 1e-12);
    assert!((p["amplitude"] - 0.6).abs() < 0.01, "{}", p["amplitude"]);
    // the harmonic smoothing of the same gives the same peak
    let h = pheno(&t, &y, &Settings { smooth: Smooth::Harmonic, n: 1, ..Default::default() });
    assert_eq!(h["peak_doy"], 201.0);
    close("smoothed max", h["max"], 0.7, 1e-9);
}

#[test]
fn a_triangle_has_its_integral_and_season_and_a_plateau_its_crossings() {
    let d0 = day(2023, 5, 1);
    let t = [d0, d0 + 10.0, d0 + 20.0];
    let p = pheno(&t, &[0.0, 5.0, 0.0], &Settings::default());
    assert_eq!((p["max"], p["min"], p["amplitude"]), (5.0, 0.0, 5.0));
    // the trapezoids: two triangles of 10 days and 5: 50
    assert_eq!(p["integral"], 50.0);
    // the base 0, the level 2.5: the season is from the 5th to the 15th day, rising and falling at 0.5 a day
    assert_eq!((p["start_date"], p["end_date"], p["length"]), (d0 + 5.0, d0 + 15.0, 10.0));
    assert_eq!((p["rise_rate"], p["fall_rate"]), (0.5, 0.5));
    assert_eq!(p["peak_doy"], phenology::doy(d0 + 10.0));
    // a plateau: the first of the maxima is the peak; the level 4 is crossed 5 days after the second date on the way up
    // and 5 after the seventh on the way down
    let t: Vec<f64> = (0..10).map(|k| d0 + 10.0 * k as f64).collect();
    let y = [0.0, 0.0, 2.0, 6.0, 8.0, 8.0, 6.0, 2.0, 0.0, 0.0];
    let p = pheno(&t, &y, &Settings::default());
    assert_eq!(p["peak_date"], t[4]);
    assert_eq!((p["start_date"], p["end_date"]), (t[2] + 5.0, t[6] + 5.0));
    // each side from its own minimum: the left is 0, the right 2, so the start level is 0 + 0.5 * 8 = 4 and the end
    // level 2 + 0.5 * 6 = 5; 4 is crossed halfway between 2 and 6 (5 days after the third date), 5 halfway between 6
    // and 4 (5 days after the seventh)
    let y = [0.0, 0.0, 2.0, 6.0, 8.0, 8.0, 6.0, 4.0, 2.0, 2.0];
    let p = pheno(&t, &y, &Settings::default());
    assert_eq!(p["start_date"], t[2] + 5.0);
    assert_eq!(p["end_date"], t[6] + 5.0);
    assert_eq!(p["length"], t[6] + 5.0 - (t[2] + 5.0));
    // the rates: 4 up in the 15 days from the start to the peak, 3 down in the 25 from the peak to the end
    close("rise", p["rise_rate"], 4.0 / 15.0, 1e-12);
    close("fall", p["fall_rate"], 3.0 / 25.0, 1e-12);
    assert_eq!((p["min"], p["amplitude"]), (0.0, 8.0));
    // a right minimum of 4 makes the end level 4 + 0.5 * 4 = 6
    let y = [0.0, 0.0, 2.0, 6.0, 8.0, 8.0, 7.0, 4.0, 4.0, 4.0];
    let p = pheno(&t, &y, &Settings::default());
    // (crossed between 7 and 4, a third of the way)
    close("end", p["end_date"], t[6] + 10.0 / 3.0, 1e-12);
    // a threshold of 0.25 starts the season earlier and ends it later
    let early = pheno(&t, &y, &Settings { threshold: 0.25, ..Default::default() });
    assert!(early["start_date"] < p["start_date"] && early["end_date"] > p["end_date"]);
    // a series that peaks on its first date has nothing to its left: no start
    let y = [8.0, 6.0, 4.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let p = pheno(&t, &y, &Settings::default());
    assert!(p["start_date"].is_nan() && p["length"].is_nan() && p["rise_rate"].is_nan());
    assert!(p["end_date"].is_finite() && p["fall_rate"].is_finite());
    // too few values give nothing
    assert!(phenology::phenology(&t[..2], &y[..2], &Settings::default()).iter().all(|v| v.is_nan()));
}

#[test]
fn a_cloud_leaves_its_date_out_as_if_it_were_not_there() {
    let z = reference();
    let h = &z["harmonics"];
    let (t, y) = (numbers(&h["t"]), numbers(&h["y"]));
    let (tk, yk): (Vec<f64>, Vec<f64>) = t.iter().zip(&y).filter(|(_, v)| v.is_finite()).map(|(a, b)| (*a, *b)).unzip();
    let s = Settings::default();
    let (a, b) = (phenology::phenology(&t, &y, &s), phenology::phenology(&tk, &yk, &s));
    for (u, v) in a.iter().zip(&b) {
        assert_eq!(u.to_bits(), v.to_bits());
    }
    let (a, b) = (phenology::harmonics(&t, &y, 2, true), phenology::harmonics(&tk, &yk, 2, true));
    for (u, v) in a.iter().zip(&b) {
        assert_eq!(u.to_bits(), v.to_bits());
    }
    // Savitzky-Golay: the holes are filled by lines in time first
    let s = Settings { smooth: Smooth::Savgol(7, 2), ..Default::default() };
    assert!(phenology::phenology(&t, &y, &s)[0].is_finite());
}

/// Objects on a grid of `w` x `h` cells: the left half object 1, the right half object 2.
fn halves(w: usize, h: usize) -> Arc<Objects> {
    let labels: Vec<u32> = (0..w * h).map(|i| if i % w < w / 2 { 1 } else { 2 }).collect();
    Arc::new(Objects::from_labels(Arc::new(Grid::new(w, h)), labels, 2).unwrap())
}

/// A stack of layers named by `names`, each cell's value from `f(layer, cell)`.
fn stack(grid: &Arc<Grid>, names: &[String], f: impl Fn(usize, usize) -> f64) -> Value {
    let layers = (0..names.len()).map(|k| Arc::new((0..grid.cells()).map(|i| f(k, i)).collect())).collect();
    Value::Stack(Stack { grid: grid.clone(), keys: names.iter().cloned().map(Key::Name).collect(), layers })
}

fn table_of(out: &Outputs, name: &str) -> Table {
    let Value::Table(t) = &out[name] else { panic!("{name} is not a table") };
    t.clone()
}

#[test]
fn the_objects_get_their_values_phenology_and_harmonics_and_a_long_table() {
    let o = halves(8, 4);
    let jan = day(2023, 1, 1);
    let dates: Vec<f64> = (0..12).map(|k| jan + 30.0 * k as f64).collect();
    // layer names with the dates somewhere in them: ndvi_20230101, S2_2023-01-31_ndvi...
    let names: Vec<String> = dates
        .iter()
        .enumerate()
        .map(|(k, &d)| {
            if k % 2 == 0 { format!("ndvi_{}", name_of(d).replace('-', "")) } else { format!("S2_{}_ndvi", name_of(d)) }
        })
        .collect();
    // object 1 (the left cells): a triangle peaking on the 7th date; object 2: flat
    let tri = |k: usize| if k <= 6 { k as f64 / 6.0 } else { (12 - k) as f64 / 6.0 };
    let w = 8;
    let s = stack(&o.grid, &names, |k, i| if i % w < w / 2 { tri(k) } else { 0.5 });
    let out = run(
        "objects.series",
        vec![("objects", Value::Objects(o.clone())), ("series", s)],
        serde_json::json!({"variable": "ndvi", "groups": ["values", "phenology", "harmonics"], "n": 1}),
    )
    .unwrap();
    let t = table_of(&out, "table");
    let col = |c: &str| t.column(c).unwrap_or_else(|| panic!("no column {c}: {:?}", t.columns));
    // a column a date, named by it
    for (k, &d) in dates.iter().enumerate() {
        let v = col(&format!("ndvi_{}", name_of(d).replace('-', "")));
        close("value of object 1", v[0], tri(k), 1e-12);
        assert_eq!(v[1], 0.5);
    }
    // the phenology of the triangle: peak on the 7th date, 1 at its top, the season at the half
    assert_eq!(col("ndvi_max")[0], 1.0);
    assert_eq!(col("ndvi_min")[0], 0.0);
    assert_eq!(col("ndvi_peak_date")[0], dates[6]);
    assert_eq!(col("ndvi_peak_doy")[0], phenology::doy(dates[6]));
    // (the left minimum is 0, so the start level is 1/2, crossed at the fourth date; the right minimum is 1/6, so the end
    // level is 1/6 + (1 - 1/6) / 2 = 7/12, crossed between 4/6 (the ninth date) and 1/2 a quarter of the way)
    let (start, end) = (dates[3], dates[8] + (4.0 / 6.0 - 7.0 / 12.0) * 6.0 * 30.0);
    close("start", col("ndvi_start_date")[0], start, 1e-12);
    close("end", col("ndvi_end_date")[0], end, 1e-12);
    close("length", col("ndvi_length")[0], end - start, 1e-12);
    // the integral: the triangle of 360 days and 1 of height... the trapezoids over the 11 intervals of 30 days
    let want: f64 = (0..11).map(|k| (tri(k) + tri(k + 1)) / 2.0 * 30.0).sum();
    close("integral", col("ndvi_integral")[0], want, 1e-12);
    // the flat object has an amplitude of 0 and no season
    assert_eq!(col("ndvi_amplitude")[1], 0.0);
    assert!(col("ndvi_start_date")[1].is_nan() && col("ndvi_length")[1].is_nan());
    // the harmonics
    for c in ["ndvi_hmean", "ndvi_amp1", "ndvi_phase1", "ndvi_rmse"] {
        assert!(col(c)[0].is_finite(), "{c}");
    }
    close("flat mean", col("ndvi_hmean")[1], 0.5, 1e-12);
    assert!(col("ndvi_amp1")[1] < 1e-12);
    // the id column and the objects' own are kept
    assert_eq!(t.columns[0], "id");
    let Value::Objects(marked) = &out["objects"] else { panic!() };
    assert_eq!(marked.table.columns, t.columns);
    // the long table: a row per object and date, no smoothing: smoothed is the value
    let long = table_of(&out, "series");
    assert_eq!(long.columns, ["id", "date", "doy", "value", "smoothed"]);
    assert_eq!(long.rows.len(), 24);
    assert_eq!(long.rows[0], vec![1.0, dates[0], 1.0, tri(0), tri(0)]);
    assert!(long.rows.iter().all(|r| r[3] == r[4]));
    // the group asked for is the only one
    let only = run(
        "objects.series",
        vec![("objects", Value::Objects(o.clone())), ("series", stack(&o.grid, &names, |k, _| k as f64))],
        serde_json::json!({"groups": "harmonics"}),
    )
    .unwrap();
    let t = table_of(&only, "table");
    assert!(t.column("value_hmean").is_some() && t.column("value_max").is_none() && t.column("value_amp2").is_some());
}

#[test]
fn clouds_and_the_share_of_cells_a_date_needs() {
    let o = halves(8, 4);
    let jan = day(2023, 1, 1);
    let names: Vec<String> = (0..8).map(|k| name_of(jan + 20.0 * k as f64)).collect();
    // on the 4th date a cloud covers 3 of the 8 cells of object 1 (and its values are junk), and on the 6th the object has none
    let s = |junk: f64| {
        stack(&o.grid, &names, move |k, i| {
            let (col, row) = (i % 8, i / 8);
            let v = (k as f64).min(7.0 - k as f64);
            match (k, col < 4) {
                (3, true) if row < 3 => junk,
                (5, true) => f64::NAN,
                _ => v,
            }
        })
    };
    let go = |share: f64, junk: f64| {
        let out = run(
            "objects.series",
            vec![("objects", Value::Objects(o.clone())), ("series", s(junk))],
            serde_json::json!({"groups": ["values"], "min_share": share, "variable": "v"}),
        )
        .unwrap();
        table_of(&out, "table")
    };
    let date = |k: usize| format!("v_{}", name_of(jan + 20.0 * k as f64).replace('-', ""));
    // the cloud pixels (NaN) leave the mean to the others: 3 of the 4 cells of the first three rows... here the 3 cells
    // of the object's first rows are NaN, the other 5 keep the value
    let nan_cloud = |share: f64| {
        let o2 = o.clone();
        let names2 = names.clone();
        let st = stack(&o2.grid, &names2, |k, i| {
            let (col, row) = (i % 8, i / 8);
            if k == 3 && col < 4 && row < 3 { f64::NAN } else { (k as f64).min(7.0 - k as f64) }
        });
        let out = run(
            "objects.series",
            vec![("objects", Value::Objects(o2)), ("series", st)],
            serde_json::json!({"groups": ["values"], "min_share": share, "variable": "v"}),
        )
        .unwrap();
        table_of(&out, "table")
    };
    // 12 cells in object 1, 9 of them under the cloud: a quarter have a value
    let strict = nan_cloud(0.5);
    assert!(strict.column(&date(3)).unwrap()[0].is_nan());
    let lax = nan_cloud(0.2);
    assert_eq!(lax.column(&date(3)).unwrap()[0], 3.0);
    // the object whose cells all have none on a date has none, however lax
    assert!(go(0.0, 99.0).column(&date(5)).unwrap()[0].is_nan());
    assert_eq!(go(0.0, 99.0).column(&date(5)).unwrap()[1], 2.0);
}

#[test]
fn savgol_through_the_operator_is_scipys_and_says_when_the_dates_are_uneven() {
    let z = reference();
    let case = &z["savgol"][0];
    let x = numbers(&case["x"]);
    let want = numbers(&case["y"]);
    let jan = day(2023, 1, 1);
    let o = halves(4, 2);
    let names: Vec<String> = (0..x.len()).map(|k| name_of(jan + 7.0 * k as f64)).collect();
    let (out, warnings) = run_with(
        "objects.series",
        vec![("objects", Value::Objects(o.clone())), ("series", stack(&o.grid, &names, |k, _| x[k]))],
        serde_json::json!({"smooth": "savgol", "window": 7, "order": 2}),
    );
    let long = table_of(&out.unwrap(), "series");
    // (object 1 is the first 40 rows)
    for (k, w) in want.iter().enumerate() {
        close(&format!("smoothed {k}"), long.rows[k][4], *w, 1e-9);
        close("value", long.rows[k][3], x[k], 1e-12);
    }
    assert!(warnings.is_empty(), "{warnings:?}");
    // an uneven series: the warning
    let uneven: Vec<String> =
        (0..x.len()).map(|k| name_of(jan + 7.0 * k as f64 + if k > 20 { 6.0 } else { 0.0 })).collect();
    let (_, warnings) = run_with(
        "objects.series",
        vec![("objects", Value::Objects(o.clone())), ("series", stack(&o.grid, &uneven, |k, _| x[k]))],
        serde_json::json!({"smooth": "savgol"}),
    );
    assert!(warnings.iter().any(|w| w.contains("evenly")), "{warnings:?}");
}

#[test]
fn objects_and_cells_give_the_same_numbers_and_any_threads_the_same_bits() {
    // a map of 30 x 20 cells, each cell its own object, 14 dates of a noisy season with clouds
    let (w, h) = (30usize, 20usize);
    let grid = Arc::new(Grid::new(w, h));
    let labels: Vec<u32> = (1..=(w * h) as u32).collect();
    let o = Arc::new(Objects::from_labels(grid.clone(), labels, w * h).unwrap());
    let jan = day(2023, 1, 1);
    let names: Vec<String> = (0..14).map(|k| name_of(jan + 26.0 * k as f64)).collect();
    let f = |k: usize, i: usize| {
        let hash = ((i * 7919 + k * 104_729) % 1009) as f64 / 1009.0;
        if hash < 0.1 {
            return f64::NAN;
        }
        let peak = 4.0 + (i % 7) as f64;
        0.2 + 0.6 * (-((k as f64 - peak) / 3.0).powi(2)).exp() + 0.03 * hash
    };
    let series = stack(&grid, &names, f);
    let params = serde_json::json!({"groups": ["phenology", "harmonics"], "n": 2, "smooth": "savgol", "window": 5, "order": 2, "variable": ""});
    let go = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            let by_objects = run(
                "objects.series",
                vec![("objects", Value::Objects(o.clone())), ("series", series.clone())],
                params.clone(),
            )
            .unwrap();
            let by_cells = run("raster.phenology", vec![("series", series.clone())], params.clone()).unwrap();
            (by_objects, by_cells)
        })
    };
    let ((a, c1), (b, c8)) = (go(1), go(8));
    let (ta, tb) = (table_of(&a, "table"), table_of(&b, "table"));
    assert_eq!(ta.columns, tb.columns);
    let bits = |t: &Table| t.rows.iter().flatten().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&ta), bits(&tb));
    assert_eq!(bits(&table_of(&a, "series")), bits(&table_of(&b, "series")));
    let (Value::Stack(s1), Value::Stack(s8)) = (&c1["metrics"], &c8["metrics"]) else { panic!() };
    for (x, y) in s1.layers.iter().zip(&s8.layers) {
        assert_eq!(
            x.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            y.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
    }
    // a cell is an object: the cells' layers are the objects' columns (the columns after the id, cells and area)
    let names: Vec<String> = s1.keys.iter().map(|k| k.to_string()).collect();
    assert_eq!(&names[..PHENOLOGY.len()], PHENOLOGY.map(String::from));
    let finite = s1.layers[0].iter().filter(|v| v.is_finite()).count();
    assert!(finite > w * h * 8 / 10, "{finite}");
    for (j, n) in names.iter().enumerate() {
        // (the objects' columns are named `max`... with the empty variable)
        let col = ta.column(n).unwrap_or_else(|| panic!("no column {n}"));
        for (i, (u, v)) in col.iter().zip(s1.layers[j].iter()).enumerate() {
            assert!((u.is_nan() && v.is_nan()) || u.to_bits() == v.to_bits(), "{n} cell {i}: {u} {v}");
        }
    }
}

#[test]
fn the_series_that_do_not_make_sense_are_refused() {
    let o = halves(8, 4);
    let jan = day(2023, 1, 1);
    let names: Vec<String> = (0..6).map(|k| name_of(jan + 10.0 * k as f64)).collect();
    let good = || stack(&o.grid, &names, |k, _| k as f64);
    let go = |s: Value, params: serde_json::Value| {
        run("objects.series", vec![("objects", Value::Objects(o.clone())), ("series", s)], params)
            .map(|_| ())
            .expect_err("refused")
            .to_string()
    };
    // no dates in the names, and none given
    let plain: Vec<String> = (1..=6).map(|k| format!("layer{k}")).collect();
    let e = go(stack(&o.grid, &plain, |k, _| k as f64), serde_json::json!({}));
    assert!(e.contains("no date in the name") && e.contains("dates"), "{e}");
    // dates given: the wrong number, a bad one, not increasing
    let e = go(stack(&o.grid, &plain, |k, _| k as f64), serde_json::json!({"dates": ["2023-01-01"]}));
    assert!(e.contains("1 dates for 6 layers"), "{e}");
    let e = go(
        stack(&o.grid, &plain, |k, _| k as f64),
        serde_json::json!({"dates": ["2023-01-01", "2023-02-01", "x", "2023-04-01", "2023-05-01", "2023-06-01"]}),
    );
    assert!(e.contains("YYYY-MM-DD"), "{e}");
    let ok = serde_json::json!({"dates": ["2023-01-01", "2023-02-01", "2023-03-01", "2023-02-15", "2023-05-01", "2023-06-01"]});
    let e = go(stack(&o.grid, &plain, |k, _| k as f64), ok);
    assert!(e.contains("must increase"), "{e}");
    // the same dates twice
    let twice: Vec<String> = ["2023-01-01", "2023-01-01", "2023-02-01", "2023-03-01"].map(String::from).to_vec();
    assert!(go(stack(&o.grid, &twice, |k, _| k as f64), serde_json::json!({})).contains("must increase"));
    // too few dates
    let few: Vec<String> = names[..3].to_vec();
    assert!(go(stack(&o.grid, &few, |k, _| k as f64), serde_json::json!({})).contains("4 at least"));
    // settings
    assert!(go(good(), serde_json::json!({"season_threshold": 1.5})).contains("season_threshold"));
    assert!(go(good(), serde_json::json!({"smooth": "savgol", "window": 4})).contains("odd window"));
    assert!(go(good(), serde_json::json!({"smooth": "savgol", "window": 5, "order": 5})).contains("odd window"));
    assert!(go(good(), serde_json::json!({"smooth": "loess"})).contains("none, savgol or harmonic"));
    assert!(go(good(), serde_json::json!({"groups": ["seasons"]})).contains("group \"seasons\""));
    assert!(go(good(), serde_json::json!({"n": 0})).contains("1 to 6 harmonics"));
    assert!(go(good(), serde_json::json!({"min_share": 2.0})).contains("min_share"));
    // another grid
    let other = Arc::new(Grid::new(4, 4));
    assert!(go(stack(&other, &names, |k, _| k as f64), serde_json::json!({})).contains("grid"));
}
