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

// ---------------------------------------------------------------------------------- swat.from_tables, swat.wgn

use sha2::{Digest, Sha256};
use terraflux_ops::swat::Project;

fn project_reference() -> (BTreeMap<String, Array>, serde_json::Value) {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/swat_project.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/swat_project.json")).unwrap())
            .unwrap();
    (z, meta)
}

/// A table from a matrix, the columns named and some of them the labels of numbers.
fn matrix_table(columns: &[String], a: &Array, text: &BTreeMap<String, Vec<Option<String>>>) -> Table {
    let mut t = Table::new(columns.to_vec());
    let n = if columns.is_empty() { 0 } else { a.data.len() / columns.len() };
    t.rows = (0..n).map(|i| a.data[i * columns.len()..(i + 1) * columns.len()].to_vec()).collect();
    for (j, c) in columns.iter().enumerate() {
        let Some(values) = text.get(c) else { continue };
        let mut names: Vec<String> = Vec::new();
        for (i, v) in values.iter().enumerate() {
            t.rows[i][j] = match v {
                None => f64::NAN,
                Some(s) => {
                    let k = names.iter().position(|n| n == s).unwrap_or_else(|| {
                        names.push(s.clone());
                        names.len() - 1
                    });
                    (k + 1) as f64
                }
            };
        }
        if !names.is_empty() {
            t.labels.insert(c.clone(), names);
        }
    }
    t
}

fn strings(v: &serde_json::Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

/// The watershed of a case as landschaft made it (its tables; the grids are one cell).
fn ref_watershed(z: &BTreeMap<String, Array>, meta: &serde_json::Value, key: &str) -> Arc<Watershed> {
    let mut tables = Vec::new();
    for (t, spec) in meta["watersheds"][key].as_object().unwrap() {
        let columns = strings(&spec["columns"]);
        let text: BTreeMap<String, Vec<Option<String>>> = spec["text"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(c, v)| (c.clone(), v.as_array().unwrap().iter().map(|s| s.as_str().map(String::from)).collect()))
            .collect();
        tables.push((t.clone(), matrix_table(&columns, &z[&format!("w/{key}/{t}")], &text)));
    }
    let one = || Map::new(Arc::new(Grid::new(1, 1)), vec![1.0]);
    Arc::new(Watershed { tables, subbasins: one(), hrus: one(), channels: one(), notes: Vec::new() })
}

const WGN_NAMES: [&str; 14] = [
    "tmp_max_ave",
    "tmp_min_ave",
    "tmp_max_sd",
    "tmp_min_sd",
    "pcp_ave",
    "pcp_sd",
    "pcp_skew",
    "wet_dry",
    "wet_wet",
    "pcp_days",
    "pcp_hhr",
    "slr_ave",
    "dew_ave",
    "wnd_ave",
];

/// The weather of a case as tables: the stations, each series and, if given, the generator.
fn weather_inputs(z: &BTreeMap<String, Array>, rec: &serde_json::Value) -> Vec<(&'static str, Value)> {
    let n = rec["name"].as_str().unwrap();
    let mut v = vec![("stations", Value::Table(table_json(&rec["stations"])))];
    for var in ["pcp", "tmax", "tmin", "slr", "hmd", "wnd"] {
        if let Some(names) = rec["series"].get(var) {
            let mut columns = vec!["date".to_string()];
            columns.extend(strings(names));
            v.push((var, Value::Table(matrix_table(&columns, &z[&format!("{n}/in/{var}")], &BTreeMap::new()))));
        }
    }
    if let Some(g) = rec.get("wgn_in") {
        let mut columns = vec!["station".to_string(), "month".to_string()];
        columns.extend(strings(&g["columns"]));
        let mut t = matrix_table(&columns, &z[&format!("{n}/in/wgn")], &BTreeMap::new());
        // the first column holds the numbers 1... of the names, in order of appearance
        t.labels.insert("station".into(), strings(&g["stations"]));
        v.push(("wgn", Value::Table(t)));
    }
    v
}

fn project_params(rec: &serde_json::Value) -> serde_json::Value {
    let mut p = rec["opt"].as_object().cloned().unwrap_or_default();
    if let Some(m) = rec.get("monthly").filter(|m| !m.is_null()) {
        p.insert("monthly".into(), m.clone());
    }
    serde_json::Value::Object(p)
}

fn project_of(
    w: &Arc<Watershed>,
    inputs: &[(&'static str, Value)],
    params: serde_json::Value,
) -> Result<Outputs, terraflux_ops::Error> {
    let watershed = Value::SwatWatershed(w.clone());
    let mut refs: Vec<(&str, &Value)> = inputs.iter().map(|(k, v)| (*k, v)).collect();
    refs.push(("watershed", &watershed));
    op("swat.from_tables", &refs, params)
}

/// A file without its first line (the date it was written).
fn body(b: &[u8]) -> &[u8] {
    &b[b.iter().position(|c| *c == b'\n').unwrap() + 1..]
}

fn stored(z: &BTreeMap<String, Array>, case: &str, name: &str) -> Vec<u8> {
    z[&format!("{case}/file/{name}")].data.iter().map(|b| *b as u8).collect()
}

/// Every file of the project against landschaft's, byte for byte but for the first line (the date it was written);
/// the files the editor writes the same for every project by their hash. Returns the files compared.
fn same_files(case: &str, p: &Project, z: &BTreeMap<String, Array>, rec: &serde_json::Value) -> usize {
    let mut names: Vec<&str> = p.files.iter().map(|(n, _)| n.as_str()).collect();
    names.sort();
    let want = strings(&rec["files"]);
    assert_eq!(names, want.iter().map(String::as_str).collect::<Vec<_>>(), "{case}: the files");
    for (name, bytes) in &p.files {
        let got = body(bytes);
        if let Some(h) = rec["static"].get(name) {
            assert_eq!(hex::encode(Sha256::digest(got)), h.as_str().unwrap(), "{case}/{name}");
            continue;
        }
        let want = stored(z, case, name);
        if got != want.as_slice() {
            let (g, w) = (String::from_utf8_lossy(got), String::from_utf8_lossy(&want));
            let k = g.lines().zip(w.lines()).position(|(a, b)| a != b).unwrap_or(0);
            panic!(
                "{case}/{name} differs at line {}:\n ours: {:?}\n want: {:?}",
                k + 2,
                g.lines().nth(k),
                w.lines().nth(k)
            );
        }
    }
    want.len()
}

#[test]
fn from_tables_writes_landschafts_files() {
    let (z, meta) = project_reference();
    let mut files = 0;
    for rec in meta["cases"].as_array().unwrap() {
        let n = rec["name"].as_str().unwrap();
        let w = ref_watershed(&z, &meta, rec["watershed"].as_str().unwrap());
        let out = project_of(&w, &weather_inputs(&z, rec), project_params(rec)).unwrap_or_else(|e| panic!("{n}: {e}"));
        let Value::SwatProject(p) = &out["project"] else { panic!() };
        files += same_files(n, p, &z, rec);
        // the project keeps the watershed it came from
        assert!(Arc::ptr_eq(p.watershed.as_ref().unwrap(), &w), "{n}");
    }
    assert!(files > 150, "{files} files compared");
}

#[test]
fn dates_are_text_or_year_and_day_as_well() {
    let (z, meta) = project_reference();
    let rec = meta["cases"].as_array().unwrap().iter().find(|c| c["name"] == "syn").unwrap();
    let w = ref_watershed(&z, &meta, "syn");
    let numbers = weather_inputs(&z, rec);
    let as_text = |t: &Table| {
        let mut t = t.clone();
        let names: Vec<String> = t
            .rows
            .iter()
            .map(|r| {
                let d = r[0] as i64;
                format!("{:04}-{:02}-{:02}", d / 10000, d / 100 % 100, d % 100)
            })
            .collect();
        for (i, r) in t.rows.iter_mut().enumerate() {
            r[0] = (i + 1) as f64;
        }
        t.labels.insert("date".into(), names);
        t
    };
    let as_year_day = |t: &Table| {
        let columns = ["year".to_string(), "day".to_string()].into_iter().chain(t.columns[1..].iter().cloned());
        let mut out = Table::new(columns.collect());
        for r in &t.rows {
            let d = r[0] as i64;
            let (y, m, dd) = (d / 10000, d / 100 % 100, d % 100);
            let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
            let before = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][m as usize - 1];
            let doy = before + dd + i64::from(leap && m > 2);
            let mut row = vec![y as f64, doy as f64];
            row.extend_from_slice(&r[1..]);
            out.rows.push(row);
        }
        out
    };
    let base = project_of(&w, &numbers, project_params(rec)).unwrap();
    let Value::SwatProject(want) = &base["project"] else { panic!() };
    for convert in [&as_text as &dyn Fn(&Table) -> Table, &as_year_day] {
        let inputs: Vec<(&'static str, Value)> = numbers
            .iter()
            .map(|(k, v)| match v {
                Value::Table(t) if *k != "stations" => (*k, Value::Table(convert(t))),
                v => (*k, v.clone()),
            })
            .collect();
        let out = project_of(&w, &inputs, project_params(rec)).unwrap();
        let Value::SwatProject(got) = &out["project"] else { panic!() };
        assert_eq!(got.files.len(), want.files.len());
        for ((n, a), (_, b)) in got.files.iter().zip(&want.files) {
            assert_eq!(body(a), body(b), "{n}");
        }
    }
}

fn wgn_of(inputs: &[(&str, &Value)], params: serde_json::Value) -> Result<Table, terraflux_ops::Error> {
    match op("swat.wgn", inputs, params)?.remove("wgn") {
        Some(Value::Table(t)) => Ok(t),
        _ => panic!("no wgn"),
    }
}

/// The statistics against landschaft's, number for number: how many differ at all, and by how much.
fn wgn_diff(t: &Table, want: &Array, what: &str) -> (usize, f64) {
    assert_eq!(t.columns.len(), 17, "{what}");
    assert_eq!(t.rows.len() * 17, want.data.len(), "{what}: rows");
    let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
    let bad = got.iter().zip(&want.data).filter(|(a, b)| !same(**a, **b)).count();
    let worst = got.iter().zip(&want.data).map(|(a, b)| (a - b).abs() / b.abs().max(1.0)).fold(0.0, f64::max);
    (bad, worst)
}

#[test]
fn wgn_is_landschafts_to_the_bit() {
    let (z, meta) = project_reference();
    let mut checked = 0;
    for c in meta["wgn_cases"].as_array().unwrap() {
        let n = c["name"].as_str().unwrap();
        let inputs: Vec<(&str, Value)> = if n == "small" {
            let mut columns = vec!["date".to_string()];
            columns.extend(strings(&c["series"]));
            let t = |k: &str| Value::Table(matrix_table(&columns, &z[&format!("wgn/small_{k}")], &BTreeMap::new()));
            vec![("pcp", t("pcp")), ("tmax", t("tmax")), ("tmin", t("tmin"))]
        } else {
            let rec = meta["cases"].as_array().unwrap().iter().find(|r| r["name"] == c["case"]).unwrap();
            weather_inputs(&z, rec).into_iter().filter(|(k, _)| *k != "stations" && *k != "wgn").collect()
        };
        let refs: Vec<(&str, &Value)> = inputs.iter().map(|(k, v)| (*k, v)).collect();
        let params =
            if c["monthly"].is_null() { serde_json::json!({}) } else { serde_json::json!({ "monthly": c["monthly"] }) };
        let t = wgn_of(&refs, params).unwrap_or_else(|e| panic!("{n}: {e}"));
        let (bad, worst) = wgn_diff(&t, &z[&format!("wgn/{n}")], n);
        eprintln!("wgn {n}: {bad} of {} numbers differ, the largest relative difference {worst:e}", t.rows.len() * 17);
        assert_eq!(bad, 0, "{n}: {bad} numbers differ (the largest relative difference {worst:e})");
        // the stations come with their names, in order of appearance
        assert_eq!(t.labels["station"], strings(&c["stations"]), "{n}");
        assert_eq!(t.columns[2..16], WGN_NAMES, "{n}");
        checked += t.rows.len();
    }
    assert!(checked >= 72, "{checked} station months");
}

/// landschaft's test_weather_generator_statistics.
#[test]
fn wgn_statistics_are_what_they_say() {
    let new = || Table::new(vec!["date".into(), "a".into()]);
    let (mut pcp, mut tx, mut tn) = (new(), new(), new());
    for y in 2000..=2003i64 {
        for m in 1..=12i64 {
            let len = match m {
                2 if y % 4 == 0 => 29,
                2 => 28,
                4 | 6 | 9 | 11 => 30,
                _ => 31,
            };
            for day in 1..=len {
                let date = (y * 10000 + m * 100 + day) as f64;
                pcp.rows.push(vec![date, if day % 3 == 0 { 6.0 } else { 0.0 }]);
                let t = if day % 2 == 0 { 20.0 } else { 10.0 };
                tx.rows.push(vec![date, t]);
                tn.rows.push(vec![date, t - 10.0]);
            }
        }
    }
    let (p, a, b) = (Value::Table(pcp), Value::Table(tx), Value::Table(tn));
    let inputs = [("pcp", &p), ("tmax", &a), ("tmin", &b)];
    let monthly = serde_json::json!({ "monthly": { "pcp_hhr": 5.0, "slr_ave": 10.0, "dew_ave": 1.0, "wnd_ave": 2.0 } });
    let t = wgn_of(&inputs, monthly).unwrap();
    let jan: Vec<f64> = t.rows[0].clone();
    let at = |c: &str| jan[t.columns.iter().position(|x| x == c).unwrap()];
    // even days 20, odd days 10
    assert!((at("tmp_max_ave") - (15.0 * 20.0 + 16.0 * 10.0) / 31.0).abs() < 1e-12);
    assert!((at("tmp_min_ave") - (15.0 * 10.0) / 31.0).abs() < 1e-12);
    // days 3, 6, ..., 30 of each January
    assert!((at("pcp_days") - 10.0).abs() < 1e-12 && (at("pcp_ave") - 60.0).abs() < 1e-12);
    assert!(at("wet_wet") == 0.0 && at("wet_dry") > 0.0 && at("wet_dry") < 1.0);
    assert_eq!(at("rain_yrs"), 4.0);
    // what the series do not give, monthly must
    let e = wgn_of(&inputs, serde_json::json!({})).err().unwrap().to_string();
    assert!(
        e.contains("the weather generator needs ['pcp_hhr', 'slr_ave', 'dew_ave', 'wnd_ave']: give their daily series or monthly="),
        "{e}"
    );
    let e = wgn_of(&inputs, serde_json::json!({ "monthly": { "foo": 1 } })).err().unwrap().to_string();
    assert!(e.contains("unknown weather generator column 'foo'"), "{e}");
}

#[test]
fn from_tables_refuses_what_landschaft_refuses() {
    let (z, meta) = project_reference();
    let rec = meta["cases"].as_array().unwrap().iter().find(|c| c["name"] == "syn").unwrap();
    let w = ref_watershed(&z, &meta, "syn");
    let inputs = weather_inputs(&z, rec);
    let err = |inputs: &[(&'static str, Value)], params: serde_json::Value| {
        project_of(&w, inputs, params).err().unwrap().to_string()
    };
    // a date repeats
    let mut bad = inputs.clone();
    for (k, v) in bad.iter_mut() {
        if let (true, Value::Table(t)) = (*k == "pcp", &mut *v) {
            let first = t.rows[0].clone();
            t.rows.push(first);
        }
    }
    assert!(err(&bad, project_params(rec)).contains("pcp: dates repeat"));
    // a station the table lacks
    let mut bad = inputs.clone();
    for (k, v) in bad.iter_mut() {
        if let (true, Value::Table(t)) = (*k == "tmax", &mut *v) {
            t.columns[1] = "nowhere".into();
        }
    }
    assert!(err(&bad, project_params(rec)).contains("tmax: stations ['nowhere'] are not in the station table"));
    // a column of codes.bsn that is not
    let mut params = project_params(rec);
    params["codes"] = serde_json::json!({ "zzz": 1 });
    assert!(err(&inputs, params).contains("codes.bsn has no column 'zzz'"));
    // no pcp_hhr
    let mut params = project_params(rec);
    params.as_object_mut().unwrap().remove("monthly");
    assert!(err(&inputs, params).contains("the weather generator needs"));
    // a wetland land use
    let (l, case) = syn_case("syn_names");
    let wet = ws(&l, &case);
    let e = project_of(&wet, &inputs, project_params(rec)).err().unwrap().to_string();
    assert!(e.contains("wetland"), "{e}");
}

/// The chain of operators (swat.watershed, swat.from_tables, swat.run) against landschaft's from_rasters and run.
#[test]
fn the_chain_from_rasters_runs_as_landschafts() {
    let (z, meta) = project_reference();
    let (zw, wmeta) = reference();
    let lands: BTreeMap<String, Land> = ["syn", "ex"].iter().map(|n| (n.to_string(), land(&zw, &wmeta, n))).collect();
    let mut compared = 0;
    for rec in meta["cases"].as_array().unwrap() {
        let n = rec["name"].as_str().unwrap();
        if z.keys().all(|k| !k.starts_with(&format!("{n}/run/"))) {
            continue;
        }
        let (landscape, wname) = if n == "ex" { ("ex", "ex_default") } else { ("syn", "syn_filter_options") };
        let mut case = wmeta["cases"].as_array().unwrap().iter().find(|c| c["name"] == wname).unwrap().clone();
        if n == "syn_crops" {
            case["landuse_lookup"] = serde_json::json!({ "1": "CORN", "2": "WWHT", "3": "URMD" });
        }
        let w = ws(&lands[landscape], &case);
        let out = project_of(&w, &weather_inputs(&z, rec), project_params(rec)).unwrap_or_else(|e| panic!("{n}: {e}"));
        let project = out["project"].clone();
        // (the degrees of the tables come from PROJ 9.7 here and 9.5 there: files with them can differ in a digit)
        let Value::SwatProject(p) = &project else { panic!() };
        let differ = p
            .files
            .iter()
            .filter(|(f, b)| rec["static"].get(f).is_none() && body(b) != stored(&z, n, f).as_slice())
            .map(|(f, _)| f.as_str())
            .collect::<Vec<_>>();
        eprintln!("chain {n}: the files that are not byte for byte landschaft's: {differ:?}");
        let run = op("swat.run", &[("project", &project)], serde_json::json!({ "step": "year", "nyskip": 0 })).unwrap();
        for t in ["hru_wb", "hru_ls", "basin_wb", "channel_sd", "aquifer"] {
            let Value::Table(got) = &run[t] else { panic!("{n}/{t}") };
            let want = &z[&format!("{n}/run/{t}")];
            let width = got.columns.len();
            assert_eq!(got.rows.len() * width, want.data.len(), "{n}/{t}: shape");
            let mut bits = 0;
            for j in 0..width {
                let scale = (0..got.rows.len()).map(|i| want.data[i * width + j].abs()).fold(0.0, f64::max);
                for i in 0..got.rows.len() {
                    let (a, b) = (got.rows[i][j], want.data[i * width + j]);
                    bits += usize::from(!same(a, b));
                    // landschaft's SWAT tolerance
                    assert!(
                        (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-4 * (b.abs() + scale),
                        "{n}/{t}[{i}][{j}]: {a} against {b}"
                    );
                }
            }
            eprintln!("chain {n}/{t}: {bits} of {} values not bit for bit", want.data.len());
            compared += want.data.len();
        }
    }
    assert!(compared > 1000);
}

#[test]
fn a_project_with_its_watershed_is_written_and_read_back() {
    let (z, meta) = project_reference();
    let rec = meta["cases"].as_array().unwrap().iter().find(|c| c["name"] == "syn").unwrap();
    let w = ref_watershed(&z, &meta, "syn");
    let out = project_of(&w, &weather_inputs(&z, rec), project_params(rec)).unwrap();
    let Value::SwatProject(p) = &out["project"] else { panic!() };
    let text = serde_json::to_string(&**p).unwrap();
    let back: Project = serde_json::from_str(&text).unwrap();
    assert_eq!(back.files, p.files);
    let (a, b) = (p.watershed.as_ref().unwrap(), back.watershed.as_ref().unwrap());
    assert_eq!(a.tables.len(), b.tables.len());
    for ((n, x), (m, y)) in a.tables.iter().zip(&b.tables) {
        assert_eq!((n, &x.columns, &x.labels), (m, &y.columns, &y.labels));
        assert!(x.rows.iter().flatten().zip(y.rows.iter().flatten()).all(|(u, v)| same(*u, *v)), "{n}");
    }
    // swat.set keeps it; a project without one is written without
    let change = serde_json::json!({ "parameter": "awc", "change": "pctchg", "value": 5 });
    let set = op("swat.set", &[("project", &out["project"])], change).unwrap();
    let Value::SwatProject(s) = &set["project"] else { panic!() };
    assert!(s.watershed.is_some());
    let Value::SwatProject(plain) = load() else { panic!() };
    assert!(!serde_json::to_string(&*plain).unwrap().contains("watershed"));
    let again: Project = serde_json::from_str(&serde_json::to_string(&*plain).unwrap()).unwrap();
    assert!(again.watershed.is_none());
}

// ------------------------------------------------------------------ the results on the map
// (tests/data/reference/swat_maps.npz, tools/make_swat_maps_reference.py)

fn maps_reference() -> (BTreeMap<String, Array>, serde_json::Value) {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/swat_maps.npz")).unwrap();
    let meta =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/swat_maps.json")).unwrap())
            .unwrap();
    (z, meta)
}

/// The project of a case of the maps' reference: built as the chain test does, from swat_project's inputs (the
/// crops case with the whole weather, not its year).
fn maps_project(name: &str) -> Value {
    let (z, meta) = project_reference();
    let (zw, wmeta) = reference();
    let rec = meta["cases"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap();
    let (landscape, wname) = match name {
        "ex" => ("ex", "ex_default"),
        _ => ("syn", "syn_filter_options"),
    };
    let mut case = wmeta["cases"].as_array().unwrap().iter().find(|c| c["name"] == wname).unwrap().clone();
    if name == "syn_crops" {
        case["landuse_lookup"] = serde_json::json!({ "1": "CORN", "2": "WWHT", "3": "URMD" });
    }
    let w = ws(&land(&zw, &wmeta, landscape), &case);
    let mut params = project_params(rec);
    if name == "syn_crops" {
        params.as_object_mut().unwrap().retain(|k, _| k != "start" && k != "end");
    }
    project_of(&w, &weather_inputs(&z, rec), params).unwrap().remove("project").unwrap()
}

fn projects() -> &'static BTreeMap<String, Value> {
    static P: std::sync::OnceLock<BTreeMap<String, Value>> = std::sync::OnceLock::new();
    P.get_or_init(|| ["syn", "syn_crops", "ex"].iter().map(|n| (n.to_string(), maps_project(n))).collect())
}

fn pairs(v: &serde_json::Value, columns: [&str; 2]) -> Value {
    let mut t = Table::new(columns.iter().map(|c| c.to_string()).collect());
    t.rows = v.as_array().unwrap().iter().map(|p| vec![p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]).collect();
    Value::Table(t)
}

/// The inputs and settings of a case's operator.
fn maps_call(rec: &serde_json::Value) -> (&'static str, Vec<(&'static str, Value)>, serde_json::Value) {
    let project = projects()[rec["project"].as_str().unwrap()].clone();
    let mut inputs = vec![("project", project)];
    let mut params = serde_json::Map::new();
    let s = rec["settings"].as_object().unwrap();
    let series = rec.get("series").filter(|v| !v.is_null());
    let name = match rec["kind"].as_str().unwrap() {
        "hru" => {
            for (k, v) in s {
                if !(k == "variable" && series.is_some()) {
                    params.insert(k.clone(), v.clone());
                }
            }
            if let Some(v) = series {
                inputs.push(("values", pairs(v, ["hru", "value"])));
            }
            "swat.hru_map"
        }
        "channel" => {
            for (k, v) in s {
                match (k.as_str(), series) {
                    ("values", Some(_)) => {}
                    ("values", None) => {
                        params.insert("column".into(), v.clone());
                    }
                    _ => {
                        params.insert(k.clone(), v.clone());
                    }
                }
            }
            if let Some(v) = series {
                inputs.push(("values", pairs(v, ["channel", "value"])));
            }
            "swat.channel_map"
        }
        _ => "swat.reach_discharge",
    };
    (name, inputs, serde_json::Value::Object(params))
}

fn call(
    name: &str,
    inputs: &[(&'static str, Value)],
    params: &serde_json::Value,
) -> Result<Outputs, terraflux_ops::Error> {
    let refs: Vec<(&str, &Value)> = inputs.iter().map(|(k, v)| (*k, v)).collect();
    op(name, &refs, params.clone())
}

/// The values of a map or a table output against the reference array: the count of values not bit for bit.
fn maps_bits(case: &str, out: &Outputs, want: &Array) -> usize {
    match out.get("map").or(out.get("table")).unwrap() {
        Value::Map(m) => {
            assert_eq!(m.data.len(), want.data.len(), "{case}: shape");
            m.data.iter().zip(&want.data).filter(|(a, b)| !same(**a, **b)).count()
        }
        Value::Table(t) => {
            let width = t.columns.len();
            assert_eq!(t.rows.len() * width, want.data.len(), "{case}: shape");
            t.rows.iter().flatten().zip(&want.data).filter(|(a, b)| !same(**a, **b)).count()
        }
        _ => panic!("{case}: the output"),
    }
}

/// Every case against landschaft's, the project run by the operator itself (no table connected), to the bit.
#[test]
fn swat_maps_are_landschafts() {
    let (z, meta) = maps_reference();
    let mut compared = 0;
    for rec in meta["cases"].as_array().unwrap() {
        let n = rec["name"].as_str().unwrap();
        let (op_name, inputs, params) = maps_call(rec);
        let out = call(op_name, &inputs, &params).unwrap_or_else(|e| panic!("{n}: {e}"));
        let want = &z[n];
        let bad = maps_bits(n, &out, want);
        let finite = want.data.iter().filter(|v| v.is_finite()).count();
        eprintln!("maps {n}: {bad} of {} values not bit for bit ({finite} finite)", want.data.len());
        assert!(finite > 0, "{n}: a map with nothing on it");
        assert_eq!(bad, 0, "{n}");
        if let Some(Value::Table(t)) = out.get("table") {
            let names: Vec<String> =
                rec["channels"].as_array().unwrap().iter().map(|c| format!("channel_{c}")).collect();
            assert_eq!(&t.columns[2..], names.as_slice(), "{n}: channels");
        }
        compared += want.data.len();
    }
    assert!(compared > 100_000);
}

/// A swat.run of a project with the tables the maps read, by its step.
fn simulated(project: &Value, step: &str, tables: &[&str], nyskip: Option<i64>) -> Outputs {
    let mut params = serde_json::json!({ "step": step, "tables": tables });
    if let Some(n) = nyskip {
        params["nyskip"] = n.into();
    }
    op("swat.run", &[("project", project)], params).unwrap()
}

/// The same maps with the run's tables connected and without them (the node running the project itself), and with
/// the other settings the run has.
#[test]
fn swat_maps_read_the_run_as_they_run_it() {
    let (z, meta) = maps_reference();
    let rec = |n: &str| meta["cases"].as_array().unwrap().iter().find(|c| c["name"] == n).unwrap().clone();
    let project = projects()["syn"].clone();
    let aa = simulated(&project, "aa", &["hru_ls", "hru_wb", "channel_sd"], None);
    let day = simulated(&project, "day", &["hru_ls", "hru_wb", "channel_sd"], None);
    // (case, the table port, the run with the tables of its step)
    for (n, port, run) in [
        ("syn_hru_sedyld", "hru_ls", &aa),
        ("syn_hru_surq_annual", "hru_wb", &aa),
        ("syn_hru_sedyld_mean", "hru_ls", &day),
        ("syn_hru_sedyld_max", "hru_ls", &day),
        ("syn_hru_sedyld_sum", "hru_ls", &day),
        ("syn_hru_et_sum", "hru_wb", &day),
        ("syn_channel_flow", "channel_sd", &day),
        ("syn_channel_flow_max", "channel_sd", &day),
        ("syn_channel_sed_annual", "channel_sd", &aa),
        ("syn_channel_sed_sum", "channel_sd", &day),
        ("syn_reach", "channel_sd", &day),
    ] {
        let (op_name, mut inputs, params) = maps_call(&rec(n));
        let own = call(op_name, &inputs, &params).unwrap();
        inputs.push((port, run[port].clone()));
        let connected = call(op_name, &inputs, &params).unwrap_or_else(|e| panic!("{n}: {e}"));
        assert_eq!(maps_bits(n, &connected, &z[n]), 0, "{n}: connected");
        assert_eq!(maps_bits(n, &own, &z[n]), 0, "{n}: by itself");
        eprintln!("maps {n}: the table {port} connected and the project run by the node give landschaft's");
    }
    // a table of another print does not stand in for the one the variable is in: the node runs the project
    let (op_name, mut inputs, params) = maps_call(&rec("syn_hru_sedyld"));
    inputs.push(("hru_wb", aa["hru_wb"].clone()));
    assert_eq!(maps_bits("hru_wb", &call(op_name, &inputs, &params).unwrap(), &z["syn_hru_sedyld"]), 0);
    // nyskip and c_factor are those of swat.run, for the run the node makes
    let (op_name, inputs, _) = maps_call(&rec("syn_hru_sedyld"));
    let own = call(op_name, &inputs, &serde_json::json!({ "variable": "sedyld", "nyskip": 0 })).unwrap();
    let with = simulated(&project, "aa", &["hru_ls"], Some(0));
    let mut connected_in = inputs.clone();
    connected_in.push(("hru_ls", with["hru_ls"].clone()));
    let connected = call(op_name, &connected_in, &serde_json::json!({ "variable": "sedyld" })).unwrap();
    let (Value::Map(a), Value::Map(b)) = (&own["map"], &connected["map"]) else { panic!() };
    assert!(a.data.iter().zip(&*b.data).all(|(x, y)| same(*x, *y)));
    let default = call(op_name, &inputs, &serde_json::json!({ "variable": "sedyld" })).unwrap();
    let Value::Map(d) = &default["map"] else { panic!() };
    assert!(a.data.iter().zip(&*d.data).any(|(x, y)| !same(*x, *y)), "a year skipped changes nothing");
    let plant = call(op_name, &inputs, &serde_json::json!({ "variable": "sedyld", "c_factor": "plant" })).unwrap();
    let Value::Map(p) = &plant["map"] else { panic!() };
    assert!(d.data.iter().zip(&*p.data).any(|(x, y)| !same(*x, *y)), "C's method changes nothing");
}

#[test]
fn swat_maps_refuse_what_does_not_fit() {
    let project = projects()["syn"].clone();
    let aa = simulated(&project, "aa", &["hru_ls", "basin_wb", "channel_sd"], None);
    let year = simulated(&project, "year", &["hru_ls", "channel_sd"], Some(0));
    let day = simulated(&project, "day", &["hru_ls", "channel_sd"], None);
    let fail = |name: &str, inputs: Vec<(&str, &Value)>, params: serde_json::Value| -> String {
        op(name, &inputs, params).err().unwrap().to_string()
    };
    // a project of swat.load has no map of its HRUs
    let plain = load();
    let e = fail("swat.hru_map", vec![("project", &plain)], serde_json::json!({ "variable": "sedyld" }));
    assert!(e.contains("swat.from_tables"), "{e}");
    let e = fail("swat.channel_map", vec![("project", &plain)], serde_json::json!({}));
    assert!(e.contains("swat.from_tables"), "{e}");
    // ... and none at all
    let e = fail("swat.hru_map", vec![], serde_json::json!({ "variable": "sedyld" }));
    assert!(e.contains("connect a project"), "{e}");
    let e = fail("swat.reach_discharge", vec![], serde_json::json!({}));
    assert!(e.contains("connect a project"), "{e}");
    // the table of a step the result does not read: which setting of swat.run to change
    let both = serde_json::json!({ "variable": "sedyld", "how": "mean" });
    let e = fail("swat.hru_map", vec![("project", &project), ("hru_ls", &aa["hru_ls"])], both);
    assert!(e.contains("not a daily one") && e.contains("step to day"), "{e}");
    let annual = serde_json::json!({ "variable": "sedyld" });
    let e = fail("swat.hru_map", vec![("project", &project), ("hru_ls", &day["hru_ls"])], annual.clone());
    assert!(e.contains("several rows") && e.contains("step to aa"), "{e}");
    let e = fail(
        "swat.channel_map",
        vec![("project", &project), ("channel_sd", &year["channel_sd"])],
        serde_json::json!({ "how": "annual" }),
    );
    assert!(e.contains("step to aa"), "{e}");
    let e = fail("swat.reach_discharge", vec![("channel_sd", &aa["channel_sd"])], serde_json::json!({}));
    assert!(e.contains("step to day"), "{e}");
    // a table that is not the one of the port
    let e = fail("swat.hru_map", vec![("project", &project), ("hru_ls", &aa["basin_wb"])], annual.clone());
    assert!(e.contains("not the hru_ls of a swat.run"), "{e}");
    // the settings
    let e = fail("swat.hru_map", vec![("project", &project)], serde_json::json!({ "variable": "nothing" }));
    assert!(e.contains("unknown HRU output"), "{e}");
    let e = fail("swat.hru_map", vec![("project", &project)], serde_json::json!({}));
    assert!(e.contains("choose the variable"), "{e}");
    let e =
        fail("swat.hru_map", vec![("project", &project)], serde_json::json!({ "variable": "sedyld", "how": "median" }));
    assert!(e.contains("how must be"), "{e}");
    let e = fail("swat.channel_map", vec![("project", &project)], serde_json::json!({ "column": "nothing" }));
    assert!(e.contains("unknown channel output"), "{e}");
    // reach_discharge needs no watershed: the Hubbard Brook example gives a day per row and a column per channel
    let q = op("swat.reach_discharge", &[("project", &plain)], serde_json::json!({ "nyskip": 0 })).unwrap();
    let Value::Table(t) = &q["table"] else { panic!() };
    assert_eq!(&t.columns[..3], ["year", "day", "channel_1"]);
    assert!(t.rows.len() > 300 && t.rows.iter().flatten().any(|v| *v > 0.0));
}
