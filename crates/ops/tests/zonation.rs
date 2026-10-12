//! prioritize.zonation against Zonation 5 v2.4: the rank maps of the z5 binary that landschaft keeps
//! (extern/landschaft/tests/data/reference/prioritize), bit for bit, on the rasters of its cases
//! (tests/data/reference/zonation_inputs.npz, tools/make_zonation_inputs.py); with the transforms (conditions,
//! retention, pre-processing maps, the features as ranked) and the connectivity transforms, their maps too.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Map, Registry, Stack, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;

fn run(
    inputs: Vec<(&str, Value)>,
    params: serde_json::Map<String, serde_json::Value>,
) -> Result<terraflux_ops::Outputs, String> {
    let r = Registry::standard();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get("prioritize.zonation").unwrap().run(&mut Ctx::default(), &args).map_err(|e| e.to_string())
}

struct Cases {
    inputs: BTreeMap<String, Array>,
    all: serde_json::Value,
    root: std::path::PathBuf,
}

fn cases() -> Cases {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inputs = terraflux_raster::npy::read_npz(root.join("tests/data/reference/zonation_inputs.npz")).unwrap();
    let all: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("tests/data/reference/zonation_inputs.json")).unwrap())
            .unwrap();
    Cases { inputs, all, root }
}

impl Cases {
    /// The cases of a group, and the z5 binary's outputs of the group.
    fn group(&self, group: &str) -> (Vec<String>, BTreeMap<String, Array>, serde_json::Value) {
        let refdir = self.root.join("extern/landschaft/tests/data/reference/prioritize");
        let reference = terraflux_raster::npy::read_npz(refdir.join(format!("{group}.npz"))).unwrap();
        let meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(refdir.join(format!("{group}.json"))).unwrap()).unwrap();
        let names = self.all["cases"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(_, c)| c["group"] == group)
            .map(|(n, _)| n.clone())
            .collect();
        (names, reference, meta)
    }

    fn map(&self, name: &str) -> Map {
        let a = &self.inputs[name];
        Map::new(Arc::new(Grid::new(a.shape[1], a.shape[0])), a.data.clone())
    }

    /// A case run: its features, the maps of its settings, and every raster of its grid as the maps conditions,
    /// retention and interactions name.
    fn run(&self, name: &str) -> terraflux_ops::Outputs {
        self.run_with(name, Vec::new())
    }

    /// A case run with more inputs.
    fn run_with(&self, name: &str, extra: Vec<(&str, Value)>) -> terraflux_ops::Outputs {
        let case = &self.all["cases"][name];
        let layers: Vec<String> =
            case["layers"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()).collect();
        let first = self.map(&layers[0]);
        let stack_of = |names: &[String]| {
            Value::Stack(Stack {
                grid: first.grid.clone(),
                keys: names.iter().map(|l| Key::Name(l.clone())).collect(),
                layers: names.iter().map(|l| Arc::new(self.inputs[l].data.clone())).collect(),
            })
        };
        let mut params = case["params"].as_object().unwrap().clone();
        // the NoData values of the features' files
        let nodata: Vec<serde_json::Value> = layers.iter().map(|l| self.all["nodata"][l].clone()).collect();
        if nodata.iter().any(|v| !v.is_null()) {
            params.insert("nodata".into(), nodata.into());
        }
        let mut ins = vec![("features", stack_of(&layers))];
        for port in ["cost", "hierarchic_mask", "analysis_area", "solution"] {
            if let Some(serde_json::Value::String(r)) = params.remove(port) {
                ins.push((port, Value::Map(self.map(&r))));
            }
        }
        let same: Vec<String> = self
            .inputs
            .iter()
            .filter(|(_, a)| a.shape == self.inputs[&layers[0]].shape)
            .map(|(k, _)| k.clone())
            .collect();
        for port in ["condition_maps", "retention_maps", "resources"] {
            ins.push((port, stack_of(&same)));
        }
        ins.extend(extra);
        run(ins, params).unwrap_or_else(|e| panic!("{name}: {e}"))
    }
}

fn same(a: f64, b: f64) -> bool {
    (a as f32).to_bits() == (b as f32).to_bits() || (a.is_nan() && b.is_nan())
}

/// The rank and the iterations of a case as the z5 binary's.
fn check_rank(name: &str, out: &terraflux_ops::Outputs, reference: &BTreeMap<String, Array>, meta: &serde_json::Value) {
    let Value::Map(rank) = &out["rank"] else { panic!() };
    let want = &reference[&format!("{name}/rank")];
    let differ = rank.data.iter().zip(&want.data).filter(|(a, b)| !same(**a, **b)).count();
    assert_eq!(differ, 0, "{name}: {differ} cells differ from Zonation's rank");
    let stats = &meta["cases"][format!("{name}/stats")];
    let iterations: f64 =
        stats.get("Number of iterations").and_then(|v| v.as_str()).map_or(0.0, |v| v.parse().unwrap());
    let Value::Number(it) = out["iterations"] else { panic!() };
    assert_eq!(it, iterations, "{name}: iterations");
}

/// The features as ranked (tr/<input number>) as Zonation's -f maps.
fn check_transformed(name: &str, c: &Cases, out: &terraflux_ops::Outputs, reference: &BTreeMap<String, Array>) {
    let layers = c.all["cases"][name]["layers"].as_array().unwrap();
    let prefix = format!("{name}/tr/");
    let want: Vec<(&String, &Array)> = reference.iter().filter(|(k, _)| k.starts_with(&prefix)).collect();
    if want.is_empty() {
        assert!(!out.contains_key("transformed"), "{name}: transformed");
        return;
    }
    let Value::Stack(t) = &out["transformed"] else { panic!("{name}: no transformed") };
    assert_eq!(t.layers.len(), want.len(), "{name}: the transformed features");
    for (k, a) in want {
        let i: usize = k[prefix.len()..].parse().unwrap();
        let key = Key::Name(layers[i].as_str().unwrap().to_string());
        let at = t.keys.iter().position(|x| *x == key).unwrap_or_else(|| panic!("{name}: no {key}"));
        let differ = t.layers[at].iter().zip(&a.data).filter(|(x, y)| !same(**x, **y)).count();
        assert_eq!(differ, 0, "{name}: feature {i} as ranked: {differ} cells differ");
    }
}

/// Every case of the core group: every rule, 16 bits, weights (negative ones and groups), thresholds, ABF's z per
/// feature, the zero modes, presence and counts, value extraction, costs, hierarchic masks, analysis areas,
/// smoothing, few iterations, a coarser error, curves, LOAD and a larger stack.
#[test]
fn rank_is_zonations() {
    let c = cases();
    let (names, reference, meta) = c.group("core");
    for name in &names {
        check_rank(name, &c.run(name), &reference, &meta);
    }
    assert_eq!(names.len(), 41);
}

/// Conditions (renormalized or not), retention (everywhere, fractions per cell, absolute amounts), with weights,
/// costs and 16 bits; the pre-processing maps (richness and range-size rarity of the weight classes and groups,
/// weighted) and the features as ranked, bit for bit.
#[test]
fn transforms_are_zonations() {
    let c = cases();
    let (names, reference, meta) = c.group("transform");
    for name in &names {
        let out = c.run(name);
        check_rank(name, &out, &reference, &meta);
        check_transformed(name, &c, &out, &reference);
        let prefix = format!("{name}/pre/");
        let want: Vec<(&String, &Array)> = reference.iter().filter(|(k, _)| k.starts_with(&prefix)).collect();
        if want.is_empty() {
            continue;
        }
        let Value::Stack(pre) = &out["preprocessing"] else { panic!("{name}: no preprocessing") };
        assert_eq!(pre.layers.len(), want.len(), "{name}: the pre-processing maps");
        for (k, a) in want {
            let key = Key::Name(k[prefix.len()..].to_string());
            let at = pre.keys.iter().position(|x| *x == key).unwrap_or_else(|| panic!("{name}: no {key}"));
            let differ = pre.layers[at].iter().zip(&a.data).filter(|(x, y)| !same(**x, **y)).count();
            assert_eq!(differ, 0, "{name}: {key}: {differ} cells differ");
        }
    }
    assert_eq!(names.len(), 9);
}

/// The connectivity transforms: single-feature smoothing with every kernel (and times itself), interactions
/// positive and negative before and after the matrices, a connectivity matrix; 16 bits and ABF; the smoothed
/// features and the ranking bit for bit.
#[test]
fn connectivity_is_zonations() {
    let c = cases();
    let (names, reference, meta) = c.group("connectivity");
    for name in &names {
        let out = c.run(name);
        check_transformed(name, &c, &out, &reference);
        check_rank(name, &out, &reference, &meta);
    }
    assert_eq!(names.len(), 6);
}

/// C++ ostream's default output of a double (%g, 6 significant digits), as Zonation writes its tables.
fn g6(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    if !x.is_finite() {
        return if x.is_nan() {
            "nan".into()
        } else if x > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    let trim = |s: String| -> String {
        if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
    };
    let s = format!("{x:.5e}");
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    if !(-4..6).contains(&exp) {
        format!("{}e{}{:02}", trim(mant.to_string()), if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        trim(format!("{:.*}", (5 - exp) as usize, x))
    }
}

/// Zonation's fmt {} of a double (%g, with .0 on whole numbers).
fn fmt_g(x: f64) -> String {
    let s = g6(x);
    if s.contains(['.', 'e', 'n', 'i']) { s } else { s + ".0" }
}

fn table<'a>(out: &'a terraflux_ops::Outputs, name: &str) -> &'a terraflux_ops::Table {
    let Value::Table(t) = &out[name] else { panic!("no {name}") };
    t
}

/// The rows of a set of features: the weight class (pos, neg, zero) or the group.
fn of_set(t: &terraflux_ops::Table, set: &str) -> Vec<Vec<f64>> {
    let (wc, g) = (t.column("weights").unwrap(), t.column("group").unwrap());
    t.rows
        .iter()
        .enumerate()
        .filter(|(k, _)| match set {
            "pos" => wc[*k] == 1.0,
            "neg" => wc[*k] == -1.0,
            "zero" => wc[*k] == 0.0,
            g_ => g[*k] == g_.parse::<f64>().unwrap(),
        })
        .map(|(_, r)| r.clone())
        .collect()
}

/// Zonation's post-processing of the ranking (its LOAD run: the curves of the ranking returned): the statistics
/// of external areas, of a range of ranks and of its separate areas (the tables and coverage matrices word for
/// word, the masks bit for bit), the comparison with another solution, the histograms; the flexibility maps bit
/// for bit where Zonation's curves are those of its ranking (its last sort did not change the order), and else
/// mostly.
#[test]
fn postprocessing_is_zonations() {
    let c = cases();
    let (names, reference, _) = c.group("post");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("tests/data/reference/zonation_post.json")).unwrap())
            .unwrap();
    for name in &names {
        let want = &files[name];
        let words = |f: &str| -> Vec<Vec<String>> {
            serde_json::from_value(want[f].clone()).unwrap_or_else(|_| panic!("{name}: no {f}"))
        };
        let run_with = |extra: serde_json::Value, ins: Vec<(&str, Value)>| {
            let mut all = c.all.clone();
            let params = all["cases"][name]["params"].as_object_mut().unwrap();
            params.extend(extra.as_object().unwrap().clone());
            let cc = Cases { inputs: c.inputs.clone(), all, root: c.root.clone() };
            cc.run_with(name, ins)
        };
        let mask_of = |out: &terraflux_ops::Outputs, key: &str, tif: &str| {
            let Value::Map(m) = &out[key] else { panic!("{name}: no {key}") };
            let w = &reference[&format!("{name}/load/post/{tif}")];
            assert_eq!(m.data.as_slice(), w.data.as_slice(), "{name}: {tif}");
        };
        // the statistics of areas
        for (id, extra, ins) in [
            ("ext", serde_json::json!({}), vec![("areas", Value::Map(c.map("areas")))]),
            ("top", serde_json::json!({"area_statistics": [0.8, 1.0]}), vec![]),
            ("sep", serde_json::json!({"area_statistics": [0.7, 1.0, 5]}), vec![]),
        ] {
            let out = run_with(extra, ins);
            let (stats, cov) = (table(&out, "area_statistics"), table(&out, "coverage"));
            let sets: Vec<String> = want
                .as_object()
                .unwrap()
                .keys()
                .filter_map(|k| k.strip_prefix("area_stats.")?.strip_suffix(&format!(".{id}.csv")).map(String::from))
                .collect();
            assert!(!sets.is_empty());
            for set in &sets {
                let got: Vec<Vec<String>> = of_set(stats, set)
                    .iter()
                    .map(|r| {
                        let mut w = vec![format!("{}", r[2] as i64), format!("{}", r[3] as i64)];
                        w.extend(r[4..].iter().map(|v| g6(*v)));
                        w
                    })
                    .collect();
                assert_eq!(got, words(&format!("area_stats.{set}.{id}.csv"))[1..], "{name}: area_stats.{set}.{id}");
                let matrix = words(&format!("coverage_matrix.{set}.{id}.csv"));
                let got: Vec<Vec<String>> = of_set(cov, set)
                    .iter()
                    .map(|r| {
                        let mut w = vec![format!("{}", r[2] as i64)];
                        w.extend(r[3..].iter().filter(|v| !v.is_nan()).map(|v| g6(*v)));
                        w
                    })
                    .collect();
                assert_eq!(got, matrix[1..], "{name}: coverage_matrix.{set}.{id}");
            }
            match id {
                "top" => mask_of(&out, "area_mask", "area_statistics_rank_range.mask.top.tif"),
                "sep" => mask_of(&out, "area_mask", "area_statistics_separate_areas.mask.sep.tif"),
                _ => {}
            }
        }
        // the comparison with sp7
        let out = run_with(
            serde_json::json!({"comparison": [0.8, 1.0, 0.5, 1.0]}),
            vec![("compare", Value::Map(c.map("sp7")))],
        );
        let counts = &table(&out, "comparison").rows[0];
        let total: f64 = counts.iter().sum();
        let lines = words("solution_comparison.cmp.txt");
        for (line, k) in lines[2..].iter().zip([0, 3, 1, 2]) {
            let n = line.iter().position(|w| w == "is").unwrap() + 1;
            assert_eq!(line[n], format!("{}", counts[k] as i64), "{name}: comparison");
            assert_eq!(line[n + 1], format!("({}%", fmt_g(100.0 * counts[k] / total)), "{name}: comparison");
        }
        mask_of(&out, "comparison_map", "solution_comparison.mask.cmp.tif");
        // the histograms
        for (id, spec) in [("h", serde_json::json!([0.8, 1.0, 10])), ("", serde_json::json!([0.0, 1.0, 7]))] {
            let out = run_with(serde_json::json!({ "histogram": spec }), vec![]);
            let h = table(&out, "histogram");
            let suffix = if id.is_empty() { ".txt".to_string() } else { format!(".{id}.txt") };
            for f in want.as_object().unwrap().keys().filter(|k| k.starts_with("histogram.") && k.ends_with(&suffix)) {
                let set = &f["histogram.".len()..f.len() - suffix.len()];
                if set.contains('.') {
                    continue;
                }
                let got: Vec<Vec<String>> = of_set(h, set)
                    .iter()
                    .map(|r| vec![format!("{}", r[2] as i64), format!("{:.4}", r[3]), fmt_g(r[4])])
                    .collect();
                assert_eq!(got, words(f)[5..], "{name}: {f}");
            }
        }
        // the flexibility maps
        for (id, err, keys) in
            [("flex", 2.0, vec![("up", ".up"), ("down", ".down"), ("both", "")]), ("wide", 10.0, vec![("both", "")])]
        {
            let out = run_with(serde_json::json!({ "flexibility": err }), vec![]);
            let Value::Stack(f) = &out["flexibility"] else { panic!() };
            for (key, suffix) in keys {
                let at = f.keys.iter().position(|k| k.to_string() == key).unwrap();
                let w = &reference[&format!("{name}/post/flexibilitymap.{id}{suffix}.tif")];
                let pairs: Vec<(f64, f64)> =
                    f.layers[at].iter().zip(&w.data).filter(|(_, b)| b.is_finite()).map(|(a, b)| (*a, *b)).collect();
                let equal = pairs.iter().filter(|(a, b)| same(*a, *b)).count();
                if name == "post_presence" {
                    assert_eq!(equal, pairs.len(), "{name}: flexibility {id}{suffix}");
                } else {
                    assert!(equal as f64 > 0.9 * pairs.len() as f64, "{name}: flexibility {id}{suffix}: {equal}");
                }
            }
        }
    }
    assert_eq!(names.len(), 4);
}

/// Zonation's impact assessment, from its projects (read by landschaft): impact sources with footprints and rings,
/// value bins and subareas; direct habitat loss (sum, product) and cumulative pressures (variant1, variant2 with
/// multiply and max); with weights and ABF. The ranking after the losses and the impact maps as the z5 binary's,
/// bit for bit; the impacts accumulated per area, group, source, zone, feature and bin as landschaft's.
#[test]
fn impacts_are_zonations() {
    let c = cases();
    let (names, reference, meta) = c.group("impact");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tables: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("tests/data/reference/zonation_impacts.json")).unwrap(),
    )
    .unwrap();
    for name in &names {
        let layers = c.all["cases"][name]["layers"].as_array().unwrap();
        let first = &c.inputs[layers[0].as_str().unwrap()];
        let on_grid: Vec<String> =
            c.inputs.iter().filter(|(_, a)| a.shape == first.shape).map(|(k, _)| k.clone()).collect();
        let maps = Value::Stack(Stack {
            grid: Arc::new(Grid::new(first.shape[1], first.shape[0])),
            keys: on_grid.iter().map(|l| Key::Name(l.clone())).collect(),
            layers: on_grid.iter().map(|l| Arc::new(c.inputs[l].data.clone())).collect(),
        });
        let out = c.run_with(name, vec![("impact_maps", maps)]);
        check_rank(name, &out, &reference, &meta);
        // the impact maps
        let prefix = format!("{name}/impacts/");
        let want: Vec<(&String, &Array)> =
            reference.iter().filter(|(k, _)| k.starts_with(&prefix) && k.ends_with(".tif")).collect();
        let Value::Stack(m) = &out["impact_maps"] else { panic!("{name}: no impact maps") };
        assert_eq!(m.layers.len(), want.len(), "{name}: the impact maps");
        for (k, a) in want {
            let file = k.rsplit('/').next().unwrap().trim_end_matches(".tif");
            let at = m.keys.iter().position(|x| x.to_string() == file).unwrap_or_else(|| panic!("{name}: no {file}"));
            let differ = m.layers[at].iter().zip(&a.data).filter(|(x, y)| !same(**x, **y)).count();
            assert_eq!(differ, 0, "{name}: {file}: {differ} cells differ");
        }
        // the impacts accumulated
        for (key, port, ids) in [("zones", "impact_zones", 6), ("cumulative", "impact_cumulative", 4)] {
            let Some(want) = tables[name].get(key) else {
                assert!(!out.contains_key(port), "{name}: {port}");
                continue;
            };
            let got = table(&out, port);
            let rows = want["rows"].as_array().unwrap();
            assert_eq!(got.rows.len(), rows.len(), "{name}: {port} rows");
            let index = |r: &[f64]| -> Vec<i64> { r[..ids].iter().map(|v| *v as i64).collect() };
            let ours: BTreeMap<Vec<i64>, &Vec<f64>> = got.rows.iter().map(|r| (index(r), r)).collect();
            for w in rows {
                let w = w.as_array().unwrap();
                // landschaft's feature is its 0-based number, the phase a name
                let mut key: Vec<i64> = Vec::new();
                for (j, v) in w[..ids].iter().enumerate() {
                    let col = want["columns"][j].as_str().unwrap();
                    key.push(match (col, v) {
                        ("feature", v) => v.as_i64().unwrap() + 1,
                        ("phase", v) => {
                            if v.as_str() == Some("direct") {
                                1
                            } else {
                                2
                            }
                        }
                        (_, v) => v.as_i64().unwrap(),
                    });
                }
                let r = ours.get(&key).unwrap_or_else(|| panic!("{name}: {port}: no {key:?}"));
                for (j, x) in w[ids..].iter().enumerate() {
                    let (a, b) = (r[ids + j], x.as_f64().unwrap());
                    assert!(a == b, "{name}: {port} {key:?} column {}: {a} != {b}", ids + j);
                }
            }
        }
    }
    assert_eq!(names.len(), 5);
}

/// The rule's forms, empty features left out, and the settings checked.
#[test]
fn settings() {
    let grid = Arc::new(Grid::new(9, 8));
    let n = 72;
    let layer = |k: usize| Arc::new((0..n).map(|i| ((i * 7 + k * 13) % 17) as f64 / 17.0).collect::<Vec<_>>());
    let stack = |empty: bool| {
        Value::Stack(Stack {
            grid: grid.clone(),
            keys: (0..3).map(|k| Key::Name(format!("f{k}"))).collect(),
            layers: (0..3).map(|k| if empty && k == 1 { Arc::new(vec![0.0; n]) } else { layer(k) }).collect(),
        })
    };
    let p = |v: serde_json::Value| v.as_object().unwrap().clone();
    let a = run(vec![("features", stack(false))], p(serde_json::json!({"rule": "ABF 0.4"}))).unwrap();
    let b = run(vec![("features", stack(false))], p(serde_json::json!({"rule": "ABF", "rule_param": 0.4}))).unwrap();
    let (Value::Map(ra), Value::Map(rb)) = (&a["rank"], &b["rank"]) else { panic!() };
    assert_eq!(ra.data, rb.data);
    let mut ranks: Vec<f64> = ra.data.to_vec();
    ranks.sort_by(f64::total_cmp);
    assert_eq!(ranks[0], 0.0, "a permutation of 0..1");
    let out = run(vec![("features", stack(true))], p(serde_json::json!({}))).unwrap();
    let Value::Table(f) = &out["features"] else { panic!() };
    assert_eq!(f.rows.iter().map(|r| r[0]).collect::<Vec<_>>(), [1.0, 3.0], "the empty feature left out");
    for bad in [
        serde_json::json!({"rule": "XYZ"}),
        serde_json::json!({"rule": "ABF 0.3", "rule_param": 0.2}),
        serde_json::json!({"rule": "LOAD"}),
        serde_json::json!({"bits": 12}),
        serde_json::json!({"weights": [1, 2]}),
    ] {
        assert!(run(vec![("features", stack(false))], p(bad.clone())).is_err(), "{bad}");
    }
}
