//! SWAT+ against landschaft's (tests/data/reference/swat.npz, tools/make_swat_reference.py): the Hubbard Brook W2
//! example run at every step, the tables SWAT+ prints and the daily weather, to the bit.

use std::path::Path;

use terraflux_ops::{Args, Ctx, Outputs, Registry, Value};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: Default::default(), params: &params };
    r.get("swat.run").unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("swat.run: {e}"))
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn example() -> std::path::PathBuf {
    root().join("extern/landschaft/src/landschaft/hydro/swat/data/hubbard_brook.zip")
}

/// An operator of the family run with inputs and settings.
fn op(name: &str, inputs: &[(&str, &Value)], params: serde_json::Value) -> Result<Outputs, terraflux_ops::Error> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let inputs = inputs.iter().map(|(k, v)| (k.to_string(), (*v).clone())).collect();
    let args = Args { node: "n", inputs, params: &params };
    r.get(name).unwrap().run(&mut Ctx::default(), &args)
}

fn load() -> Value {
    op("swat.load", &[], serde_json::json!({ "path": example().to_str().unwrap() })).unwrap().remove("project").unwrap()
}

fn tables_equal(a: &Outputs, b: &Outputs, ports: &[&str]) -> usize {
    let mut n = 0;
    for p in ports {
        let (Value::Table(x), Value::Table(y)) = (&a[*p], &b[*p]) else { panic!("{p}") };
        assert_eq!(x.rows.len(), y.rows.len(), "{p}: rows");
        n += x.rows.iter().flatten().zip(y.rows.iter().flatten()).filter(|(u, v)| !same(**u, **v)).count();
    }
    n
}

#[test]
fn swat_is_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/swat.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/swat.json")).unwrap()).unwrap();
    let example = root().join("extern/landschaft/src/landschaft/hydro/swat/data/hubbard_brook.zip");
    for case in meta["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut params = case.as_object().unwrap().clone();
        params.remove("name");
        params.insert("path".into(), serde_json::json!(example.to_str().unwrap()));
        let out = run(serde_json::Value::Object(params));
        let mut checked = 0;
        for (key, want) in z.iter().filter(|(k, _)| k.starts_with(&format!("{name}/"))) {
            let port = &key[name.len() + 1..];
            let Value::Table(t) = &out[port] else { panic!("{key}") };
            assert_eq!(t.rows.len(), want.shape[0], "{key}: rows");
            assert_eq!(t.columns.len(), want.shape[1], "{key}: columns");
            let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
            let bad = got.iter().zip(&want.data).filter(|(a, b)| !same(**a, **b)).count();
            assert_eq!(bad, 0, "{key}: {bad} values differ");
            checked += 1;
        }
        assert!(checked > 1, "{name}");
    }
}

#[test]
fn a_loaded_project_runs_as_its_path_does() {
    let project = load();
    let Value::SwatProject(p) = &project else { panic!() };
    assert!(p.files.iter().any(|(n, _)| n == "file.cio"));
    let by_path = run(serde_json::json!({ "path": example().to_str().unwrap() }));
    let by_project = op("swat.run", &[("project", &project)], serde_json::json!({})).unwrap();
    let ports = ["basin_wb", "hru_wb", "hru_ls", "channel_sd", "aquifer", "weather"];
    assert_eq!(tables_equal(&by_path, &by_project, &ports), 0);
}

#[test]
fn a_run_takes_a_project_or_a_path_not_neither_nor_both() {
    let none = op("swat.run", &[], serde_json::json!({})).err().unwrap().to_string();
    assert!(none.contains("connect a project") && none.contains("TxtInOut"), "{none}");
    let both = op("swat.run", &[("project", &load())], serde_json::json!({ "path": example().to_str().unwrap() }))
        .err()
        .unwrap()
        .to_string();
    assert!(both.contains("not both"), "{both}");
    let lone = op("swat.load", &[], serde_json::json!({ "path": "no/such/folder" })).err().unwrap().to_string();
    assert!(lone.contains("not a folder or a zip"), "{lone}");
}

#[test]
fn set_is_landschafts_calibration() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/swat_set.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/swat_set.json")).unwrap())
            .unwrap();
    let base = load();
    let plain = op("swat.run", &[("project", &base)], serde_json::json!({})).unwrap();
    for case in meta["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut project = base.clone();
        for c in case["changes"].as_array().unwrap() {
            let mut params = c.as_object().unwrap().clone();
            if let Some(o) = params.get("objects") {
                assert!(o.is_array());
            }
            params.entry("change").or_insert(serde_json::json!("absval"));
            project = op("swat.set", &[("project", &project)], serde_json::Value::Object(params))
                .unwrap_or_else(|e| panic!("{name}: {e}"))
                .remove("project")
                .unwrap();
        }
        let out = op("swat.run", &[("project", &project)], serde_json::json!({})).unwrap();
        let mut checked = 0;
        for (key, want) in z.iter().filter(|(k, _)| k.starts_with(&format!("{name}/"))) {
            let port = &key[name.len() + 1..];
            let Value::Table(t) = &out[port] else { panic!("{key}") };
            assert_eq!(t.rows.len(), want.shape[0], "{key}: rows");
            assert_eq!(t.columns.len(), want.shape[1], "{key}: columns");
            let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
            let bad = got.iter().zip(&want.data).filter(|(a, b)| !same(**a, **b)).count();
            assert_eq!(bad, 0, "{key}: {bad} values differ");
            checked += 1;
        }
        assert!(checked > 5, "{name}");
        // the change reaches the tables
        assert!(tables_equal(&plain, &out, &["hru_wb", "basin_wb"]) > 0, "{name}: nothing changed");
    }
}

#[test]
fn set_leaves_the_project_it_was_given() {
    let base = load();
    let Value::SwatProject(before) = &base else { panic!() };
    let out = op(
        "swat.set",
        &[("project", &base)],
        serde_json::json!({ "parameter": "awc", "change": "pctchg", "value": 10 }),
    )
    .unwrap();
    let Value::SwatProject(after) = &out["project"] else { panic!() };
    let kept = before.files.clone();
    // the input is as it was; the copy differs in calibration.cal alone
    let Value::SwatProject(again) = &base else { panic!() };
    assert_eq!(again.files, kept);
    assert_eq!(before.files.len(), after.files.len());
    let differ: Vec<&str> =
        before.files.iter().zip(&after.files).filter(|(a, b)| a != b).map(|(a, _)| a.0.as_str()).collect();
    assert_eq!(differ, ["calibration.cal"]);
    // the row is added after the 12 the file has, and the count says so
    let text = String::from_utf8(after.files.iter().find(|(n, _)| n == "calibration.cal").unwrap().1.clone()).unwrap();
    assert_eq!(text.lines().nth(1).unwrap().trim(), "13");
    assert!(text.lines().nth(15).unwrap().starts_with("awc"), "{text}");
    // what the port does not calibrate is refused, with the reason
    let plant = op("swat.set", &[("project", &base)], serde_json::json!({ "parameter": "usle_c", "value": 0.1 }));
    assert!(plant.err().unwrap().to_string().contains("plants"));
    let soil = op(
        "swat.set",
        &[("project", &base)],
        serde_json::json!({ "parameter": "surlag", "value": 1, "first_layer": 2 }),
    );
    assert!(soil.err().unwrap().to_string().contains("no layers"));
}

#[test]
fn a_project_is_written_and_read_back_as_it_was() {
    let Value::SwatProject(p) = load() else { panic!() };
    let text = serde_json::to_string(&*p).unwrap();
    let back: terraflux_ops::swat::Project = serde_json::from_str(&text).unwrap();
    assert_eq!(back.files, p.files);
}

// ---------------------------------------------------------------------------------------- swat.watershed

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_ops::swat_watershed::Watershed;
use terraflux_ops::{Map, Table};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;
use terraflux_raster::vector::{Geometry, Vector};

fn reference() -> (BTreeMap<String, Array>, serde_json::Value) {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/swat_watershed.npz")).unwrap();
    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/data/reference/swat_watershed.json")).unwrap(),
    )
    .unwrap();
    (z, meta)
}

/// A grid from landschaft's extent (xmin, xmax, ymin, ymax) and shape, in a projected CRS.
fn grid_of(epsg: u64, shape: &serde_json::Value, extent: &serde_json::Value) -> Arc<Grid> {
    let sh: Vec<usize> = shape.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect();
    let e: Vec<f64> = extent.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let mut g = Grid::new(sh[1], sh[0]);
    g.transform = [e[0], (e[1] - e[0]) / sh[1] as f64, 0.0, e[3], 0.0, -(e[3] - e[2]) / sh[0] as f64];
    g.geo.wkt = Some(format!("EPSG:{epsg}"));
    Arc::new(g)
}

/// A table from columns and rows as the reference writes them: text as the labels of numbers, null as none.
fn table_json(t: &serde_json::Value) -> Table {
    let columns: Vec<String> =
        t["columns"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
    let rows = t["rows"].as_array().unwrap();
    let mut out = Table::new(columns.clone());
    out.rows = vec![Vec::new(); rows.len()];
    for (j, name) in columns.iter().enumerate() {
        let mut names: Vec<String> = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            out.rows[i].push(match &r[j] {
                serde_json::Value::Null => f64::NAN,
                serde_json::Value::String(s) => {
                    let k = names.iter().position(|n| n == s).unwrap_or_else(|| {
                        names.push(s.clone());
                        names.len() - 1
                    });
                    (k + 1) as f64
                }
                v => v.as_f64().unwrap(),
            });
        }
        if !names.is_empty() {
            out.labels.insert(name.clone(), names);
        }
    }
    out
}

/// A lookup (code, name) from pairs.
fn lookup_of(pairs: &serde_json::Value) -> Table {
    table_json(&serde_json::json!({ "columns": ["code", "name"], "rows": pairs }))
}

struct Land {
    inputs: Vec<(&'static str, Value)>,
    grid: Arc<Grid>,
    regrid: Option<(Arc<Grid>, Vec<f64>)>,
    meta: serde_json::Value,
}

fn land(z: &BTreeMap<String, Array>, meta: &serde_json::Value, name: &str) -> Land {
    let m = &meta["landscapes"][name];
    let epsg = m["epsg"].as_u64().unwrap();
    let grid = grid_of(epsg, &m["shape"], &m["extent"]);
    let map = |r: &str| Value::Map(Map::new(grid.clone(), z[&format!("{name}/{r}")].data.clone()));
    let inputs = vec![
        ("dem", map("dem")),
        ("landuse", map("landuse")),
        ("soils", map("soils")),
        ("landuse_lookup", Value::Table(lookup_of(&m["landuse_lookup"]))),
        ("soil_lookup", Value::Table(lookup_of(&m["soil_lookup"]))),
        ("soil_table", Value::Table(table_json(&m["soil_table"]))),
    ];
    let regrid = m
        .get("regrid")
        .map(|r| (grid_of(epsg, &r["shape"], &r["extent"]), z[&format!("{name}/regrid_landuse")].data.clone()));
    Land { inputs, grid, regrid, meta: m.clone() }
}

/// The inputs of a case and its settings, as the reference script gives them to landschaft.
fn case_inputs(land: &Land, case: &serde_json::Value) -> (Vec<(&'static str, Value)>, serde_json::Value) {
    let mut inputs = land.inputs.clone();
    let put = |inputs: &mut Vec<(&'static str, Value)>, k: &'static str, v: Value| {
        inputs.retain(|(n, _)| *n != k);
        inputs.push((k, v));
    };
    if let Some(l) = case.get("landuse_lookup") {
        let pairs: Vec<serde_json::Value> =
            l.as_object().unwrap().iter().map(|(k, v)| serde_json::json!([k.parse::<i64>().unwrap(), v])).collect();
        put(&mut inputs, "landuse_lookup", Value::Table(lookup_of(&serde_json::Value::Array(pairs))));
    }
    if case.get("soil_table").is_some() {
        put(&mut inputs, "soil_table", Value::Table(table_json(&land.meta["usersoil"])));
    }
    if case.get("regrid").is_some() {
        let (g, data) = land.regrid.as_ref().unwrap();
        put(&mut inputs, "landuse", Value::Map(Map::new(g.clone(), data.clone())));
    }
    if let Some(o) = case.get("outlets") {
        let pts: Vec<Geometry> = o
            .as_array()
            .unwrap()
            .iter()
            .map(|p| Geometry::Point([p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]))
            .collect();
        let v = Vector { geometries: pts, attributes: Vec::new(), crs: land.grid.geo.wkt.clone() };
        put(&mut inputs, "outlets", Value::Vector(Arc::new(v)));
    }
    let mut params = serde_json::Map::new();
    for k in ["stream_threshold", "snap", "slope_bands", "hru_method", "thresholds", "min_area", "area_in_ha", "exempt"]
    {
        if let Some(v) = case.get(k) {
            params.insert(k.into(), v.clone());
        }
    }
    (inputs, serde_json::Value::Object(params))
}

fn watershed(inputs: &[(&'static str, Value)], params: serde_json::Value) -> Result<Outputs, terraflux_ops::Error> {
    let refs: Vec<(&str, &Value)> = inputs.iter().map(|(k, v)| (*k, v)).collect();
    op("swat.watershed", &refs, params)
}

/// The columns whose degrees come out of a coordinate transformation (PROJ 9.5 for landschaft, 9.7 here).
const DEGREES: [&str; 4] = ["lat", "lon", "midlat", "midlon"];

/// Compare a table with landschaft's: every number bit for bit (the degrees within `tol`), text as it is. Returns
/// the largest difference of the degrees.
fn same_table(case: &str, name: &str, t: &Table, z: &Array, spec: &serde_json::Value, tol: f64) -> f64 {
    let columns: Vec<&str> = spec["columns"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    assert_eq!(t.columns, columns, "{case}/{name}: columns");
    assert_eq!(t.rows.len(), z.shape[0], "{case}/{name}: rows");
    let mut worst = 0.0f64;
    for (j, c) in columns.iter().enumerate() {
        if let Some(text) = spec["text"].get(*c) {
            for (i, want) in text.as_array().unwrap().iter().enumerate() {
                let got = t.cell(j, t.rows[i][j]);
                assert_eq!(got, want.as_str().unwrap_or(""), "{case}/{name}.{c}[{i}]");
            }
            continue;
        }
        for (i, row) in t.rows.iter().enumerate() {
            let (got, want) = (row[j], z.data[i * columns.len() + j]);
            if DEGREES.contains(c) && !same(got, want) {
                let d = (got - want).abs();
                assert!(d <= tol, "{case}/{name}.{c}[{i}]: {got} against {want}");
                worst = worst.max(d);
            } else {
                assert!(same(got, want), "{case}/{name}.{c}[{i}]: {got} against {want}");
            }
        }
    }
    worst
}

#[test]
fn watershed_is_landschafts() {
    let (z, meta) = reference();
    let lands: BTreeMap<String, Land> = ["syn", "ex"].iter().map(|n| (n.to_string(), land(&z, &meta, n))).collect();
    let mut worst = 0.0f64;
    let mut hrus = 0;
    for case in meta["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let l = &lands[case["landscape"].as_str().unwrap()];
        let (inputs, params) = case_inputs(l, case);
        let out = watershed(&inputs, params).unwrap_or_else(|e| panic!("{name}: {e}"));
        let Value::SwatWatershed(w) = &out["watershed"] else { panic!("{name}") };
        for (t, spec) in case["tables"].as_object().unwrap() {
            let got = w.table(t).unwrap_or_else(|| panic!("{name}: no table {t}"));
            worst = worst.max(same_table(name, t, got, &z[&format!("{name}/{t}")], spec, 1e-12));
        }
        assert_eq!(w.tables.len(), case["tables"].as_object().unwrap().len(), "{name}: tables");
        for (port, grid) in [("subbasins", &w.subbasins), ("hrus", &w.hrus), ("channels", &w.channels)] {
            let want = &z[&format!("{name}/{port}")];
            assert_eq!(grid.data.len(), want.data.len(), "{name}/{port}");
            let bad = grid.data.iter().zip(&want.data).filter(|(a, b)| !same(**a, **b)).count();
            assert_eq!(bad, 0, "{name}/{port}: {bad} cells differ");
            // the outputs are the value's grids
            let Value::Map(m) = &out[port] else { panic!() };
            assert!(m.data.iter().zip(grid.data.iter()).all(|(a, b)| same(*a, *b)), "{name}/{port}: the output");
        }
        let (Value::Table(t), Some(h)) = (&out["table"], w.table("gis_hrus")) else { panic!() };
        assert_eq!((&t.columns, &t.labels), (&h.columns, &h.labels));
        assert!(t.rows.iter().flatten().zip(h.rows.iter().flatten()).all(|(a, b)| same(*a, *b)), "{name}: table");
        hrus += h.rows.len();
    }
    assert!(hrus > 400, "{hrus} HRUs checked");
    eprintln!("watershed: the largest difference of the degrees: {worst:e}");
}

fn syn_case(name: &str) -> (Land, serde_json::Value) {
    let (z, meta) = reference();
    let l = land(&z, &meta, "syn");
    let case = meta["cases"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap().clone();
    (l, case)
}

fn ws(l: &Land, case: &serde_json::Value) -> Arc<Watershed> {
    let (inputs, params) = case_inputs(l, case);
    let out = watershed(&inputs, params).unwrap();
    let Value::SwatWatershed(w) = &out["watershed"] else { panic!() };
    w.clone()
}

fn col(t: &Table, c: &str) -> Vec<f64> {
    t.column(c).unwrap_or_else(|| panic!("no column {c}"))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// landschaft's test_watershed_tables_add_up, on its synthetic valley.
#[test]
fn watershed_tables_add_up() {
    let (l, case) = syn_case("syn_filter_options");
    let w = ws(&l, &case);
    let cell_ha = 30.0 * 30.0 / 1e4;
    // the whole synthetic DEM drains to one outlet
    let total = 60.0 * 50.0 * cell_ha;
    let subs = w.table("gis_subbasins").unwrap();
    assert!(close(col(subs, "area").iter().sum::<f64>(), total));
    let hrus = w.table("gis_hrus").unwrap();
    let lsus = w.table("gis_lsus").unwrap();
    let (lsu_of, arslp) = (col(hrus, "lsu"), col(hrus, "arslp"));
    for (id, area) in col(lsus, "id").iter().zip(col(lsus, "area")) {
        let sum: f64 = lsu_of.iter().zip(&arslp).filter(|(l, _)| *l == id).map(|(_, a)| a).sum();
        assert!(close(sum, area), "lsu {id}: {sum} against {area}");
    }
    let ch = w.table("gis_channels").unwrap();
    assert!(col(ch, "len2").iter().all(|v| *v > 0.0) && col(ch, "wid2").iter().all(|v| *v > 0.0));
    assert!(close(col(ch, "areac").iter().cloned().fold(0.0, f64::max), total));
    assert_eq!(w.table("gis_deep_aquifers").unwrap().rows.len(), 1);
    let routing = w.table("gis_routing").unwrap();
    let j = routing.columns.iter().position(|c| c == "hyd_typ").unwrap();
    let mut kinds: Vec<String> = routing.rows.iter().map(|r| routing.cell(j, r[j])).collect();
    kinds.sort();
    kinds.dedup();
    assert_eq!(kinds, ["rhg", "tot"]);
    // every cell of the map is in a subbasin, numbered 1..=n
    let n = subs.rows.len() as f64;
    assert!(w.subbasins.data.iter().all(|v| *v >= 1.0 && *v <= n));
    // the HRU map holds the HRU ids of the table
    let ids: std::collections::BTreeSet<u64> = col(hrus, "id").iter().map(|v| *v as u64).collect();
    let seen: std::collections::BTreeSet<u64> = w.hrus.data.iter().filter(|v| !v.is_nan()).map(|v| *v as u64).collect();
    assert!(seen.is_subset(&ids));
}

/// landschaft's test_hru_methods.
#[test]
fn watershed_hru_methods_nest() {
    let (l, base) = syn_case("syn_dominant_hru");
    let hru_count = |case: &serde_json::Value| ws(&l, case).table("gis_hrus").unwrap().rows.len();
    let mut kw = base.clone();
    let one = ws(&l, &kw);
    // one HRU per landscape unit, as large as the unit
    let (h, u) = (one.table("gis_hrus").unwrap(), one.table("gis_lsus").unwrap());
    assert_eq!(h.rows.len(), u.rows.len());
    for (a, b) in col(h, "arslp").iter().zip(col(u, "area")) {
        assert!(close(*a, b), "{a} against {b}");
    }
    kw["hru_method"] = "filter".into();
    kw["slope_bands"] = serde_json::json!([5, 20]);
    let all = hru_count(&kw);
    kw["thresholds"] = serde_json::json!([20, 20, 20]);
    let some = hru_count(&kw);
    assert!(all > some && some >= u.rows.len(), "{all} {some}");
    // the area method keeps the HRUs above min_area, or the only one of a unit
    kw["hru_method"] = "area".into();
    kw["min_area"] = 3.0.into();
    let area = ws(&l, &kw);
    let h = area.table("gis_hrus").unwrap();
    let lsu_of = col(h, "lsu");
    for (a, id) in col(h, "arslp").iter().zip(&lsu_of) {
        assert!(*a >= 3.0 || lsu_of.iter().filter(|x| *x == id).count() == 1, "{a} in {id}");
    }
}

#[test]
fn watershed_is_written_and_read_back_as_it_was() {
    let (l, case) = syn_case("syn_filter_options");
    let w = ws(&l, &case);
    let text = serde_json::to_string(&*w).unwrap();
    let back: Watershed = serde_json::from_str(&text).unwrap();
    assert_eq!(back.tables.len(), w.tables.len());
    for ((n, a), (m, b)) in w.tables.iter().zip(&back.tables) {
        assert_eq!(n, m);
        assert_eq!(a.columns, b.columns);
        assert_eq!(a.labels, b.labels);
        assert!(a.rows.iter().flatten().zip(b.rows.iter().flatten()).all(|(x, y)| same(*x, *y)), "{n}");
    }
    for (a, b) in [(&w.subbasins, &back.subbasins), (&w.hrus, &back.hrus), (&w.channels, &back.channels)] {
        assert!(a.data.iter().zip(b.data.iter()).all(|(x, y)| same(*x, *y)));
        assert!(a.grid.same_as(&b.grid));
    }
    // removed HRUs and soil layers without a pH stay without a value
    assert!(w.hrus.data.iter().any(|v| v.is_nan()));
}

#[test]
fn watershed_refuses_what_landschaft_refuses() {
    let (l, case) = syn_case("syn_dominant_hru");
    let (mut inputs, params) = case_inputs(&l, &case);
    let err = |inputs: &[(&'static str, Value)], params: serde_json::Value| {
        watershed(inputs, params).err().unwrap().to_string()
    };
    let mut bad = params.clone();
    bad["hru_method"] = "nope".into();
    assert!(err(&inputs, bad).contains("hru_method"));
    let mut bad = params.clone();
    bad["stream_threshold"] = 1000.0.into();
    assert!(err(&inputs, bad).contains("no streams"));
    // a land use code the lookup lacks
    inputs.retain(|(n, _)| *n != "landuse_lookup");
    inputs.push(("landuse_lookup", Value::Table(lookup_of(&serde_json::json!([[1, "FRSD"], [2, "PAST"]])))));
    assert!(err(&inputs, params.clone()).contains("land use codes [3]"));
    // no CRS
    let (mut inputs, params) = case_inputs(&l, &case);
    for (n, v) in inputs.iter_mut() {
        if let (true, Value::Map(m)) = (*n == "dem", &*v) {
            let mut g = (*m.grid).clone();
            g.geo.wkt = None;
            *v = Value::Map(Map::new(Arc::new(g), m.data.to_vec()));
        }
    }
    assert!(err(&inputs, params).contains("CRS"));
}
