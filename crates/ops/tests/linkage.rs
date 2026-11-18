//! Linkage Pathways against an independent reference (tests/data/reference/linkage.json,
//! tools/make_linkage_reference.py): scikit-image's MCP_Geometric for the cost-weighted distances, numpy for
//! the allocation, the adjacency, the least-cost distances, the filters and the corridors, on a 40 x 50 grid of
//! cells of 30 x 20 units. The pinch points against the library's Circuitscape on the corridors of that
//! reference (the key `pinch`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_ops::linkage::{Lattice, cwd};
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::Geometry;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference() -> serde_json::Value {
    let p = root().join("tests/data/reference/linkage.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn num(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

fn nums(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(num).collect()
}

fn grid(r: &serde_json::Value) -> Arc<Grid> {
    let (ny, nx) = (r["ny"].as_u64().unwrap() as usize, r["nx"].as_u64().unwrap() as usize);
    let mut g = Grid::new(nx, ny);
    g.transform = [0.0, num(&r["dx"]), 0.0, ny as f64 * num(&r["dy"]), 0.0, -num(&r["dy"])];
    Arc::new(g)
}

fn maps(r: &serde_json::Value) -> (Map, Map) {
    let g = grid(r);
    let cores = nums(&r["cores"]).into_iter().map(|v| if v == 0.0 { f64::NAN } else { v }).collect();
    (Map::new(g.clone(), cores), Map::new(g, nums(&r["resistance"])))
}

fn exec(op: &str, cores: &Map, res: &Map, links: Option<&Table>, params: serde_json::Value) -> Result<Outputs, String> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let mut inputs: std::collections::BTreeMap<String, Value> =
        [("cores", Value::Map(cores.clone())), ("resistance", Value::Map(res.clone()))]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
    if let Some(t) = links {
        inputs.insert("links".into(), Value::Table(t.clone()));
    }
    let args = Args { node: "n", inputs, params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).map_err(|e| e.to_string())
}

fn run(cores: &Map, res: &Map, params: serde_json::Value) -> Result<Outputs, String> {
    exec("linkage.pathways", cores, res, None, params)
}

fn close(a: f64, b: f64, what: &str) {
    if a.is_nan() || b.is_nan() {
        assert!(a.is_nan() && b.is_nan(), "{what}: {a} against {b}");
    } else {
        assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{what}: {a} against {b}");
    }
}

fn data(v: &Value) -> Vec<f64> {
    let Value::Map(m) = v else { panic!("not a map") };
    m.data.to_vec()
}

#[test]
fn distances_are_mcp_geometrics() {
    let r = reference();
    let (cores, res) = maps(&r);
    let lat = Lattice::of(&cores.grid);
    for (id, want) in r["surfaces"].as_object().unwrap() {
        let id: f64 = id.parse().unwrap();
        let src: Vec<(usize, u32)> =
            cores.data.iter().enumerate().filter(|(_, v)| **v == id).map(|(c, _)| (c, 1)).collect();
        let (d, _) = cwd(&res.data, &lat, &src);
        let want = nums(want);
        for (c, (a, b)) in d.iter().zip(&want).enumerate() {
            let a = if a.is_finite() { *a } else { f64::NAN };
            close(a, *b, &format!("core {id} cell {c}"));
        }
    }
}

#[test]
fn links_and_corridors_are_the_references() {
    let r = reference();
    let (cores, res) = maps(&r);
    for (name, case) in r["cases"].as_object().unwrap() {
        let out = run(&cores, &res, case["params"].clone()).unwrap();
        let Value::Table(t) = &out["links"] else { panic!("no table") };
        let want = case["links"].as_array().unwrap();
        assert_eq!(t.rows.len(), want.len(), "{name}: links");
        let col = |c: &str| t.column(c).unwrap();
        for (i, w) in want.iter().enumerate() {
            let what = format!("{name}: link {i}");
            close(col("from")[i], num(&w["from"]), &format!("{what} from"));
            close(col("to")[i], num(&w["to"]), &format!("{what} to"));
            close(col("euclidean")[i], num(&w["euclidean"]), &format!("{what} euclidean"));
            close(col("lcd")[i], num(&w["lcd"]), &format!("{what} lcd"));
            close(col("path_length")[i], num(&w["path_length"]), &format!("{what} path_length"));
            assert_eq!(col("kept")[i] == 1.0, w["kept"].as_bool().unwrap(), "{what}: kept");
            assert_eq!(t.cell(7, col("reason")[i]), w["reason"].as_str().unwrap(), "{what}: reason");
            if w["kept"].as_bool().unwrap() {
                close(col("cwd_to_path")[i], num(&w["lcd"]) / num(&w["path_length"]), &format!("{what} cwd_to_path"));
            }
        }
        let got = data(&out["corridors"]);
        for (c, (a, b)) in got.iter().zip(nums(&case["corridors"])).enumerate() {
            close(*a, b, &format!("{name}: corridors cell {c}"));
        }
        // the paths: a line per kept link, from a cell of one core to a cell of the other
        let Value::Vector(v) = &out["paths"] else { panic!("no vector") };
        let kept: Vec<&serde_json::Value> = want.iter().filter(|w| w["kept"].as_bool().unwrap()).collect();
        assert_eq!(v.geometries.len(), kept.len(), "{name}: paths");
        let g = &cores.grid;
        let id_at = |p: &[f64; 2]| {
            let (c, r) = (
                ((p[0] - g.transform[0]) / g.transform[1]) as usize,
                ((p[1] - g.transform[3]) / g.transform[5]) as usize,
            );
            cores.data[r * g.width + c]
        };
        for (geo, w) in v.geometries.iter().zip(&kept) {
            let Geometry::LineString(p) = geo else { panic!("not a line") };
            assert_eq!(id_at(&p[0]), num(&w["from"]), "{name}: path starts in its first core");
            assert_eq!(id_at(p.last().unwrap()), num(&w["to"]), "{name}: path ends in its second core");
        }
    }
}

#[test]
fn the_width_cuts_the_corridors() {
    let r = reference();
    let (cores, res) = maps(&r);
    let all = data(&run(&cores, &res, serde_json::json!({})).unwrap()["corridors"]);
    let cut = data(&run(&cores, &res, serde_json::json!({"width": 400.0})).unwrap()["corridors"]);
    assert!(all.iter().any(|v| *v > 400.0), "the whole mosaic has cells past the width");
    for (a, c) in all.iter().zip(&cut) {
        if *a <= 400.0 {
            assert_eq!(a, c);
        } else {
            assert!(c.is_nan());
        }
    }
}

#[test]
fn the_nearest_cost_distance_is_the_least_of_the_cores() {
    let r = reference();
    let (cores, res) = maps(&r);
    let got = data(&run(&cores, &res, serde_json::json!({})).unwrap()["cwd"]);
    for (c, (a, b)) in got.iter().zip(nums(&r["cwd"])).enumerate() {
        close(*a, b, &format!("cwd cell {c}"));
    }
}

#[test]
fn a_different_grid_is_refused() {
    let r = reference();
    let (cores, res) = maps(&r);
    let mut g = (*res.grid).clone();
    g.transform[1] += 1.0;
    let other = Map::new(Arc::new(g), res.data.to_vec());
    let e = run(&cores, &other, serde_json::json!({})).unwrap_err();
    assert!(e.contains("not on one grid"), "{e}");
    let e = run(&cores, &res, serde_json::json!({"adjacency": "near"})).unwrap_err();
    assert!(e.contains("adjacency"), "{e}");
}

/// The links of linkage.pathways' table that `cfg` gives, and the pinch points of them.
fn pinch_of(cores: &Map, res: &Map, cfg: &serde_json::Value, params: serde_json::Value) -> Result<Outputs, String> {
    let Value::Table(links) = &run(cores, res, cfg.clone()).unwrap()["links"] else { panic!("no table") };
    exec("linkage.pinchpoints", cores, res, Some(links), params)
}

fn table(v: &Value) -> &Table {
    let Value::Table(t) = v else { panic!("not a table") };
    t
}

#[test]
fn pinch_points_are_the_librarys_circuits() {
    let r = reference();
    let (cores, res) = maps(&r);
    for (name, case) in r["pinch"].as_object().unwrap() {
        let params = serde_json::json!({"width": case["width"], "mode": case["mode"], "square": case["square"]});
        let out = pinch_of(&cores, &res, &case["links"], params).unwrap();
        let got = data(&out["current"]);
        for (c, (a, b)) in got.iter().zip(nums(&case["current"])).enumerate() {
            // the all-to-one problems are solved in batches: the last bits depend on them
            if a.is_nan() != b.is_nan() || (!a.is_nan() && (a - b).abs() > 1e-8 * b.abs().max(1.0)) {
                panic!("{name}: current cell {c}: {a} against {b}");
            }
        }
        let t = table(&out["links"]);
        let want = case["rows"].as_array().unwrap();
        assert_eq!(t.rows.len(), want.len(), "{name}: links");
        let col = |c: &str| t.column(c).unwrap();
        for (i, w) in want.iter().enumerate() {
            let what = format!("{name}: link {i}");
            close(col("from")[i], num(&w["from"]), &format!("{what} from"));
            close(col("to")[i], num(&w["to"]), &format!("{what} to"));
            close(col("lcd")[i], num(&w["lcd"]), &format!("{what} lcd"));
            if case["mode"] == "pairwise" {
                close(col("resistance")[i], num(&w["resistance"]), &format!("{what} resistance"));
                close(col("cwd_to_resistance")[i], num(&w["ratio"]), &format!("{what} ratio"));
                close(col("nodes")[i], num(&w["nodes"]), &format!("{what} nodes"));
            } else {
                assert!(col("resistance")[i].is_nan(), "{what}: no resistance in all-to-one");
            }
        }
    }
}

/// The current is only inside the corridors (a link's cells of a normalized distance within the width) and not
/// in the cores of all-to-one; a wider band gives current where the narrower one has none.
#[test]
fn the_current_is_inside_the_corridors() {
    let r = reference();
    let (cores, res) = maps(&r);
    let surf = |id: f64| nums(&r["surfaces"][format!("{}", id as i64)]);
    let cfg = serde_json::json!({});
    let Value::Table(links) = &run(&cores, &res, cfg.clone()).unwrap()["links"] else { panic!() };
    let (from, to, lcd) = (links.column("from").unwrap(), links.column("to").unwrap(), links.column("lcd").unwrap());
    for mode in ["pairwise", "all_to_one"] {
        let at = |width: f64| {
            data(&pinch_of(&cores, &res, &cfg, serde_json::json!({"width": width, "mode": mode})).unwrap()["current"])
        };
        let (narrow, wide) = (at(150.0), at(350.0));
        for (cur, width) in [(&narrow, 150.0), (&wide, 350.0)] {
            for (c, v) in cur.iter().enumerate() {
                if v.is_nan() {
                    continue;
                }
                // (a third core in a link's corridor is a cell like the others in its circuit)
                if mode == "all_to_one" {
                    assert!(cores.data[c].is_nan(), "{mode}: current in a core, cell {c}");
                }
                let inside = (0..from.len()).any(|k| surf(from[k])[c] + surf(to[k])[c] - lcd[k] <= width + 1e-9);
                assert!(inside, "{mode}: current outside the corridors, cell {c}");
            }
        }
        let count = |v: &[f64]| v.iter().filter(|x| !x.is_nan()).count();
        assert!(count(&narrow) > 0 && count(&narrow) < count(&wide), "{mode}: the width grows the band");
    }
}

#[test]
fn pinch_points_refuse_what_does_not_fit() {
    let r = reference();
    let (cores, res) = maps(&r);
    let mut out = run(&cores, &res, serde_json::json!({})).unwrap();
    let Value::Table(links) = out.remove("links").unwrap() else { panic!() };
    let go = |t: &Table, p: serde_json::Value| exec("linkage.pinchpoints", &cores, &res, Some(t), p);
    // no kept link
    let mut none = links.clone();
    let k = none.columns.iter().position(|c| c == "kept").unwrap();
    none.rows.iter_mut().for_each(|r| r[k] = 0.0);
    let e = go(&none, serde_json::json!({"width": 300.0})).unwrap_err();
    assert!(e.contains("no kept link"), "{e}");
    // a link table of another resistance
    let mut other = links.clone();
    let l = other.columns.iter().position(|c| c == "lcd").unwrap();
    other.rows[0][l] *= 1.5;
    let e = go(&other, serde_json::json!({"width": 300.0})).unwrap_err();
    assert!(e.contains("not linkage.pathways"), "{e}");
    // the width, the mode and the grid
    let e = go(&links, serde_json::json!({})).unwrap_err();
    assert!(e.contains("width"), "{e}");
    let e = go(&links, serde_json::json!({"width": 300.0, "mode": "all"})).unwrap_err();
    assert!(e.contains("mode"), "{e}");
    let mut g = (*res.grid).clone();
    g.transform[1] += 1.0;
    let moved = Map::new(Arc::new(g), res.data.to_vec());
    let e = exec("linkage.pinchpoints", &cores, &moved, Some(&links), serde_json::json!({"width": 300.0})).unwrap_err();
    assert!(e.contains("not on one grid"), "{e}");
}
