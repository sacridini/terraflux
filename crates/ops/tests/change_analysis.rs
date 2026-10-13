//! The analysis of land change against landschaft's (tests/data/reference/change.json,
//! tools/make_change_reference.py): the transitions, areas and matrix, the intensity analysis in km2 and in cells
//! with its stationarity, the components of the difference and the change count of pie_landuse (three dates) and
//! sibuyan_landuse (two), to the bit.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_ops::files::Rasters;
use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference() -> serde_json::Value {
    let text = std::fs::read_to_string(root().join("tests/data/reference/change.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
}

fn table(out: &Outputs, port: &str) -> Table {
    let Value::Table(t) = &out[port] else { panic!("{port}") };
    t.clone()
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) || (a == 0.0 && b == 0.0)
}

/// A cell of the reference as a number: a period "t0-t1" as its years, a stationarity's label as 1 (active,
/// targeted) or 0, a test Y as 1, Overall as none.
fn cells(v: &serde_json::Value) -> Vec<f64> {
    match v {
        serde_json::Value::Null => vec![f64::NAN],
        serde_json::Value::Number(n) => vec![n.as_f64().unwrap()],
        serde_json::Value::String(s) => {
            if let Some((a, b)) = s.split_once('-')
                && let (Ok(a), Ok(b)) = (a.parse::<f64>(), b.parse::<f64>())
            {
                return vec![a, b];
            }
            match s.as_str() {
                "Y" => vec![1.0],
                "N" => vec![0.0],
                "Overall" => vec![f64::NAN],
                s if s.starts_with("Active") || s.starts_with("targeted") => vec![1.0],
                s if s.starts_with("Dormant") || s.starts_with("avoided") => vec![0.0],
                s => panic!("{s}"),
            }
        }
        v => panic!("{v}"),
    }
}

fn rows(f: &serde_json::Value) -> Vec<Vec<f64>> {
    f["rows"].as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().flat_map(cells).collect()).collect()
}

/// The reference's rows, their columns reordered by `order` (indices of the flattened reference columns).
fn check(got: &Table, want: &[Vec<f64>], order: &[usize], what: &str) {
    assert_eq!(got.rows.len(), want.len(), "{what}: rows ({:?})", got.columns);
    for (i, (g, w)) in got.rows.iter().zip(want).enumerate() {
        let w: Vec<f64> = order.iter().map(|&k| w[k]).collect();
        assert_eq!(g.len(), w.len(), "{what}: row {i}");
        for (j, (a, b)) in g.iter().zip(&w).enumerate() {
            assert!(same(*a, *b), "{what}: row {i} {}: {a} {b}", got.columns[j]);
        }
    }
}

#[test]
fn change_analysis_is_landschafts() {
    let reference = reference();
    for case in reference["cases"].as_array().unwrap() {
        let file = case["file"].as_str().unwrap();
        let what = format!("{file} gain {} loss {}", case["gain"], case["loss"]);
        let path = root().join("tests/data/inputs").join(file);
        let maps = Value::Raster(Arc::new(Rasters::open(path.to_str().unwrap(), None, None).unwrap()));
        // transitions: Period (2), From, To, km2, cells, Interval, yearFrom, yearTo -> years, from, to, km2,
        // cells, interval
        let out = run("change.transitions", vec![("maps", maps.clone())], serde_json::json!({}));
        let order = [0, 1, 2, 3, 4, 5, 6];
        check(&table(&out, "table"), &rows(&case["table"]), &order, &format!("{what}: table"));
        check(&table(&out, "one_step"), &rows(&case["one_step"]), &order, &format!("{what}: one step"));
        let times: Vec<f64> = case["times"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let areas: Vec<Vec<f64>> = case["areas"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&times)
            .map(|(r, t)| {
                std::iter::once(*t).chain(r.as_array().unwrap().iter().map(|v| v.as_f64().unwrap())).collect()
            })
            .collect();
        let n = areas[0].len();
        check(&table(&out, "areas"), &areas, &(0..n).collect::<Vec<_>>(), &format!("{what}: areas"));
        let m = &case["matrix"];
        let mrows: Vec<Vec<f64>> = m["classes"]
            .as_array()
            .unwrap()
            .iter()
            .zip(m["counts"].as_array().unwrap())
            .map(|(c, r)| {
                std::iter::once(c.as_f64().unwrap())
                    .chain(r.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()))
                    .collect()
            })
            .collect();
        let k = mrows[0].len();
        check(&table(&out, "matrix"), &mrows, &(0..k).collect::<Vec<_>>(), &format!("{what}: matrix"));
        // the intensity analysis, in km2 and in cells
        for (key, area) in [("intensity_km2", true), ("intensity_cells", false)] {
            let p = serde_json::json!({"gain": case["gain"], "loss": case["loss"], "area": area});
            let out = run("change.intensity", vec![("maps", maps.clone())], p);
            let ia = &case[key];
            for (port, order) in [
                ("interval", vec![0, 1, 2, 3, 4]),
                ("category_gain", vec![0, 1, 2, 3, 4, 5, 6]),
                ("category_loss", vec![0, 1, 2, 3, 4, 5, 6]),
                ("transition_gain", vec![0, 1, 2, 3, 4, 5, 6, 7]),
                ("transition_loss", vec![0, 1, 2, 3, 4, 5, 6, 7]),
                ("category_gain_stationarity", vec![0, 1, 2, 3, 4]),
                ("category_loss_stationarity", vec![0, 1, 2, 3, 4]),
                ("transition_gain_stationarity", vec![0, 1, 2, 3, 4]),
                ("transition_loss_stationarity", vec![0, 1, 2, 3, 4]),
            ] {
                check(&table(&out, port), &rows(&ia[port]), &order, &format!("{what}: {key} {port}"));
            }
        }
        // the components of the difference from the first map to the last
        let st = match &maps {
            Value::Raster(r) => r.stack().unwrap(),
            _ => unreachable!(),
        };
        let layer = |k: usize| Value::Map(terraflux_ops::Map { grid: st.grid.clone(), data: st.layers[k].clone() });
        let last = st.layers.len() - 1;
        for (analysis, key) in [("change", "components"), ("error", "components_error")] {
            let out = run(
                "change.components",
                vec![("from", layer(0)), ("to", layer(last))],
                serde_json::json!({"analysis": analysis}),
            );
            check(&table(&out, "table"), &rows(&case[key]), &[0, 1, 2, 3, 4, 5, 6], &format!("{what}: {key}"));
            let o = &case["overall"];
            let want: Vec<f64> = ["quantity", "exchange", "shift", "allocation", "overall"]
                .iter()
                .map(|k| o[k].as_f64().unwrap())
                .collect();
            check(&table(&out, "overall"), &[want], &[0, 1, 2, 3, 4], &format!("{what}: overall"));
        }
        // the change count
        let out = run("change.count", vec![("maps", maps.clone())], serde_json::json!({}));
        let Value::Map(cm) = &out["map"] else { panic!() };
        let want: Vec<f64> = case["change_count"]["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .map(|v| if v < 0.0 { f64::NAN } else { v })
            .collect();
        assert_eq!(cm.data.len(), want.len());
        assert!(cm.data.iter().zip(&want).all(|(a, b)| same(*a, *b)), "{what}: the change count");
        check(&table(&out, "table"), &rows(&case["change_count"]["table"]), &[0, 1, 2], &format!("{what}: counts"));
    }
}

/// The transitions suggested: the largest first, those of 1 % of the change at least.
#[test]
fn transitions_suggested() {
    let path = root().join("tests/data/inputs/pie_landuse.tif");
    let maps = Value::Raster(Arc::new(Rasters::open(path.to_str().unwrap(), None, None).unwrap()));
    let out = run("change.suggest", vec![("maps", maps)], serde_json::json!({"least": 5}));
    let t = table(&out, "table");
    assert!(t.rows.windows(2).all(|w| w[0][2] >= w[1][2]), "the largest first");
    let total: f64 = t.rows.iter().map(|r| r[4]).sum();
    assert!((total - 100.0).abs() < 1e-9);
    for r in &t.rows {
        assert_eq!(r[6] == 1.0, r[4] >= 5.0);
    }
    let Value::Json(serde_json::Value::Array(kept)) = &out["transitions"] else { panic!() };
    assert_eq!(kept.len(), t.rows.iter().filter(|r| r[6] == 1.0).count());
    assert!(!kept.is_empty());
}
