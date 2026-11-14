//! prioritize.optimize against landschaft on the same data, with the same solver (HiGHS, its default routing, 4
//! threads): every objective on every kind of planning unit (raster cells, tables, Marxan's rij, polygons, H3
//! hexagons; management zones on rasters and tables), on landschaft's optimize cases and on a larger set of rasters
//! (tests/data/reference/optimize_parity.json, tools/make_optimize_parity_reference.py). Each case: the compiled model
//! is landschaft's (size and sums), the status and objective are its (within the gap asked), the units selected are
//! its where the optimum is unique, and the time is at most 1.5 times its plus a quarter of a second.
//!
//! Release builds, one test at a time, four threads (the times mean nothing otherwise):
//! `RAYON_NUM_THREADS=4 OMP_NUM_THREADS=4 cargo test -p terraflux-ops --release --test ops -j 4 -- --ignored
//! --test-threads 1 --nocapture parity::` (`PARITY_ONLY=small_raster,medium_h3` runs the cases whose names hold one).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use serde_json::{Value as Json, json};
use terraflux_ops::{Args, Ctx, Key, Map, Registry, Stack, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;

const FEATURES: [&str; 5] = ["f1", "f2", "f3", "f4", "f5"];
const ZONES: [&str; 3] = ["protect", "use", "restore"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn data(file: &str) -> BTreeMap<String, Array> {
    terraflux_raster::npy::read_npz(root().join("tests/data/reference").join(file)).unwrap()
}

fn run_op(op: &str, ins: Vec<(&str, Value)>, params: Json) -> terraflux_ops::Outputs {
    let args = Args {
        node: "n",
        inputs: ins.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        params: params.as_object().unwrap(),
    };
    Registry::standard().get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

/// A set of rasters: the cost and the features (cells of 100 m, the top-left corner at (0, rows x 100)).
struct Set {
    ny: usize,
    nx: usize,
    cost: Vec<f64>,
    features: Vec<f64>,
    locks: Option<(Vec<f64>, Vec<f64>)>,
}

impl Set {
    fn new(name: &str) -> Set {
        let (d, locks) = if name == "small" {
            let d = data("optimize_inputs.npz");
            let locks = Some((d["locked_in"].data.clone(), d["locked_out"].data.clone()));
            (d, locks)
        } else {
            (data("optimize_parity_inputs.npz"), None)
        };
        let (ny, nx) = (d["pu_cost"].shape[0], d["pu_cost"].shape[1]);
        Set { ny, nx, cost: d["pu_cost"].data.clone(), features: d["features"].data.clone(), locks }
    }

    fn grid(&self, crs: bool) -> Arc<Grid> {
        let mut grid = Grid::new(self.nx, self.ny);
        grid.transform = [0.0, 100.0, 0.0, self.ny as f64 * 100.0, 0.0, -100.0];
        if crs {
            grid.geo.wkt = Some("EPSG:32723".to_string());
        }
        Arc::new(grid)
    }

    fn stack(&self, grid: &Arc<Grid>) -> Value {
        Value::Stack(Stack {
            grid: grid.clone(),
            keys: FEATURES.iter().map(|f| Key::Name(f.to_string())).collect(),
            layers: self.features.chunks(self.ny * self.nx).map(|c| Arc::new(c.to_vec())).collect(),
        })
    }

    /// The cells with a cost (row-major): the planning units.
    fn cells(&self) -> Vec<usize> {
        (0..self.cost.len()).filter(|&c| self.cost[c].is_finite()).collect()
    }

    /// The cells as a table (id, cost, the features' amounts with no value as 0, and the locks).
    fn table(&self, locks: bool) -> Table {
        let n = self.ny * self.nx;
        let mut columns: Vec<String> = ["id", "cost"].map(String::from).to_vec();
        columns.extend(FEATURES.map(String::from));
        let locked = self.locks.as_ref().filter(|_| locks);
        if locked.is_some() {
            columns.extend(["locked_in", "locked_out"].map(String::from));
        }
        let mut t = Table::new(columns);
        for (k, c) in self.cells().into_iter().enumerate() {
            let mut row = vec![(k + 1) as f64, self.cost[c]];
            row.extend((0..5).map(|f| {
                let v = self.features[f * n + c];
                if v.is_finite() { v } else { 0.0 }
            }));
            if let Some((i, o)) = locked {
                row.extend([f64::from(u8::from(i[c] == 1.0)), f64::from(u8::from(o[c] == 1.0))]);
            }
            t.rows.push(row);
        }
        t
    }

    /// The table's boundary matrix (as the tool's): rook neighbours 100 m, the exposed edges on the diagonal.
    fn boundary(&self) -> Table {
        let cells = self.cells();
        let pos: BTreeMap<usize, usize> = cells.iter().enumerate().map(|(k, &c)| (c, k)).collect();
        let mut t = Table::new(["i", "j", "x"].map(String::from).to_vec());
        for (k, &c) in cells.iter().enumerate() {
            let (r, q) = ((c / self.nx) as i64, (c % self.nx) as i64);
            let mut edges = 4.0;
            for (rr, qq) in [(r - 1, q), (r + 1, q), (r, q - 1), (r, q + 1)] {
                let inside = (0..self.ny as i64).contains(&rr) && (0..self.nx as i64).contains(&qq);
                if let Some(&m) = pos.get(&((rr * self.nx as i64 + qq) as usize)).filter(|_| inside) {
                    t.rows.push(vec![(k + 1) as f64, (m + 1) as f64, 100.0]);
                    edges -= 1.0;
                }
            }
            t.rows.push(vec![(k + 1) as f64, (k + 1) as f64, edges * 100.0]);
        }
        t
    }
}

/// The tables of landschaft's cases (null: no value).
fn tables(file: &str) -> BTreeMap<String, Table> {
    #[derive(serde::Deserialize)]
    struct Raw {
        columns: Vec<String>,
        rows: Vec<Vec<Option<f64>>>,
    }
    let path = root().join("tests/data/reference").join(file);
    let raw: BTreeMap<String, Raw> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    raw.into_iter()
        .map(|(k, r)| {
            let rows = r.rows.into_iter().map(|row| row.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect()).collect();
            (k, Table { columns: r.columns, rows, ..Default::default() })
        })
        .collect()
}

fn read_vector(name: &str) -> Value {
    let path = root().join(format!("tests/data/reference/vectors/{name}.gpkg"));
    run_op("vector.read", vec![], json!({"path": path.to_string_lossy()})).remove("vector").unwrap()
}

/// R's matrix(c(...), ncol = 3) of the zone cases is in the reference as features x zones.
struct Problem {
    inputs: BTreeMap<String, Value>,
    params: serde_json::Map<String, Json>,
}

/// The node's inputs and settings of a case: the reference's settings and what its kind of units adds.
fn problem_of(
    case: &Json,
    small: &Set,
    medium: &Set,
    zone_tables: &BTreeMap<String, Table>,
) -> Result<Problem, String> {
    let units = case["units"].as_str().unwrap();
    let locks = case["locks"].as_bool().unwrap_or(false);
    let set = if case["data"] == "small" { small } else { medium };
    let mut params = case["params"].as_object().unwrap().clone();
    params.insert("gap".into(), case["gap_asked"].clone());
    params.insert("time_limit".into(), case["time_limit"].clone());
    let mut inputs: BTreeMap<String, Value> = BTreeMap::new();
    let gdal = || terraflux_gdal::gdal().map(|_| ()).map_err(|e| format!("no GDAL: {e}"));
    match units {
        "raster" => {
            let grid = set.grid(false);
            inputs.insert("features".into(), set.stack(&grid));
            inputs.insert("cost".into(), Value::Map(Map::new(grid.clone(), set.cost.clone())));
            if let (true, Some((i, o))) = (locks, &set.locks) {
                inputs.insert("locked_in".into(), Value::Map(Map::new(grid.clone(), i.clone())));
                inputs.insert("locked_out".into(), Value::Map(Map::new(grid, o.clone())));
            }
        }
        "table" => {
            inputs.insert("pu".into(), Value::Table(set.table(locks)));
            params.insert("features".into(), json!(FEATURES));
            if params.contains_key("boundary_penalty") {
                inputs.insert("boundary_data".into(), Value::Table(set.boundary()));
            }
            if locks {
                params.insert("locked_in_column".into(), "locked_in".into());
                params.insert("locked_out_column".into(), "locked_out".into());
            }
        }
        "rij" => {
            let t = tables("optimize_tables.json");
            let pu = &t["pu_table"];
            let keep: Vec<usize> =
                ["id", "cost"].iter().map(|n| pu.columns.iter().position(|c| c == n).unwrap()).collect();
            let mut id_cost = Table::new(vec!["id".into(), "cost".into()]);
            id_cost.rows = pu.rows.iter().map(|r| keep.iter().map(|&k| r[k]).collect()).collect();
            inputs.insert("pu".into(), Value::Table(id_cost));
            inputs.insert("spec".into(), Value::Table(t["spec_table"].clone()));
            inputs.insert("rij".into(), Value::Table(t["rij_table"].clone()));
        }
        "polygons" => {
            gdal()?;
            let name = if case["data"] == "small" { "pu_polygons_partial" } else { "parity_polygons" };
            inputs.insert("features".into(), set.stack(&set.grid(false)));
            inputs.insert("polygons".into(), read_vector(name));
            params.insert("cost_column".into(), "cost".into());
        }
        h3 if h3.starts_with("h3_") => {
            gdal()?;
            // landschaft's hexagons as they are (the cases lie at x = 0 of UTM zone 23S, latitude -85, where the
            // transverse Mercator of PROJ 9.5.1 and 9.7.1 differ by 1e-8 m: the amounts of the cells a hexagon
            // only touches move, and the models with them); prioritize.h3_units is h3.rs's test
            let name = format!("parity_h3_{}_{}", case["data"].as_str().unwrap(), &h3[3..]);
            inputs.insert("features".into(), set.stack(&set.grid(false)));
            inputs.insert("polygons".into(), read_vector(&name));
            params.insert("cost_column".into(), "cost".into());
        }
        "zones_raster" => {
            let d = data("optimize_zones.npz");
            let grid = small.grid(false);
            let stack = |a: &Array, keys: Vec<String>| {
                Value::Stack(Stack {
                    grid: grid.clone(),
                    keys: keys.into_iter().map(Key::Name).collect(),
                    layers: a.data.chunks(small.ny * small.nx).map(|c| Arc::new(c.to_vec())).collect(),
                })
            };
            inputs.insert("zone_costs".into(), stack(&d["zone_cost"], ZONES.map(String::from).to_vec()));
            let keys = ZONES.iter().flat_map(|z| FEATURES.iter().map(move |f| format!("{z}_{f}"))).collect();
            inputs.insert("zone_features".into(), stack(&d["zone_features"], keys));
            params.insert("feature_names".into(), json!(FEATURES));
        }
        "zones_table" => {
            inputs.insert("pu".into(), Value::Table(zone_tables["zone_table"].clone()));
            let features: serde_json::Map<String, Json> =
                ZONES.iter().map(|z| (z.to_string(), json!(FEATURES.map(|f| format!("{}_{f}", &z[..1]))))).collect();
            params.insert("features".into(), features.into());
            params.insert("cost_column".into(), json!(ZONES.map(|z| format!("cost_{z}"))));
            params.insert("zones".into(), json!(ZONES));
            if locks {
                params.insert("locked_in_column".into(), json!(ZONES.map(|z| format!("locked_in_{z}"))));
                params.insert("locked_out_column".into(), json!(ZONES.map(|z| format!("locked_out_{z}"))));
            }
        }
        other => panic!("unknown units {other}"),
    }
    Ok(Problem { inputs, params })
}

/// The compiled model's size and sums, as the reference's.
fn check_model(p: &Problem, want: &Json) -> Result<(), String> {
    let args = Args { node: "n", inputs: p.inputs.clone(), params: &p.params };
    let problem = terraflux_ops::prioritize::problem_of(&args).map_err(|e| e.to_string())?;
    let (m, _) = terraflux_ops::milp::compile(&problem).map_err(|e| e.to_string())?;
    let (mut nnz, mut a_sum) = (0usize, 0.0);
    for i in 0..m.a.nrow {
        let (_, v) = m.a.row(i);
        nnz += v.iter().filter(|&&x| x != 0.0).count();
        a_sum += v.iter().sum::<f64>();
    }
    // the boundaries of slanted shared edges (hexagons) differ in the ninth digit between the two geometry codes
    let close = |a: f64, b: f64, tol: f64| (a - b).abs() <= tol * b.abs().max(1.0);
    let mut why = Vec::new();
    let (rows, cols) = (m.a.nrow as u64, m.obj.len() as u64);
    if rows != want["nrow"].as_u64().unwrap() || cols != want["ncol"].as_u64().unwrap() {
        why.push(format!("size {rows}x{cols} != {}x{}", want["nrow"], want["ncol"]));
    }
    if nnz as u64 != want["nnz"].as_u64().unwrap() {
        why.push(format!("non-zeros {nnz} != {}", want["nnz"]));
    }
    let rhs_sum: f64 = m.rhs.iter().sum();
    let obj_sum: f64 = m.obj.iter().sum();
    for (what, ours, key, tol) in
        [("rhs", rhs_sum, "rhs_sum", 1e-9), ("obj", obj_sum, "obj_sum", 1e-8), ("A", a_sum, "a_sum", 1e-8)]
    {
        let theirs = want[key].as_f64().unwrap();
        if !close(ours, theirs, tol) {
            why.push(format!("{what} sum {ours} != {theirs}"));
        }
    }
    let head: Vec<f64> = want["rhs_head"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    if let Some(k) = (0..head.len().min(m.rhs.len())).find(|&k| m.rhs[k].to_bits() != head[k].to_bits()) {
        why.push(format!("rhs[{k}] {} != {} (to the bit)", m.rhs[k], head[k]));
    }
    if why.is_empty() { Ok(()) } else { Err(why.join("; ")) }
}

struct Run {
    optimal: bool,
    objective: f64,
    selected: Vec<usize>,
    n: usize,
    seconds: f64,
}

/// prioritize.optimize `runs` times (the median of the seconds).
fn solve(p: &Problem, runs: usize) -> Result<Run, String> {
    let registry = Registry::standard();
    let op = registry.get("prioritize.optimize").unwrap();
    let mut seconds = Vec::new();
    let mut last = None;
    for _ in 0..runs {
        let args = Args { node: "n", inputs: p.inputs.clone(), params: &p.params };
        let mut ctx = Ctx::default();
        let t0 = Instant::now();
        let out = op.run(&mut ctx, &args).map_err(|e| format!("error: {e}"))?;
        seconds.push(t0.elapsed().as_secs_f64());
        last = Some(out);
    }
    seconds.sort_by(|a, b| a.total_cmp(b));
    let out = last.unwrap();
    let (Value::Table(s), Value::Table(sel)) = (&out["summary"], &out["selection"]) else { panic!("outputs") };
    let n = sel.rows.len();
    // the units selected: the solution's columns (one per zone) one after the other, as the reference's
    let mut selected = Vec::new();
    for (z, c) in (1..sel.columns.len()).enumerate() {
        selected.extend(sel.rows.iter().enumerate().filter(|(_, r)| r[c] > 0.5).map(|(i, _)| z * n + i));
    }
    Ok(Run {
        optimal: s.column("optimal").unwrap()[0] == 1.0,
        objective: s.column("objective").unwrap()[0],
        selected,
        n: n * (sel.columns.len() - 1),
        seconds: seconds[seconds.len() / 2],
    })
}

#[test]
#[ignore = "minutes; release build, one test at a time: see the module's doc"]
fn optimize_is_landschafts() {
    let reference: Json = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/data/reference/optimize_parity.json")).unwrap(),
    )
    .unwrap();
    let only: Vec<String> = std::env::var("PARITY_ONLY")
        .map(|s| s.split(',').filter(|p| !p.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    let (small, medium) = (Set::new("small"), Set::new("medium"));
    let zone_tables = tables("optimize_zone_tables.json");
    println!("landschaft {}, {} threads", reference["landschaft"], reference["threads"]);
    println!(
        "{:<40} {:>8} {:>8} {:>14} {:>14} {:>8} {:>8}  units",
        "case", "us", "them", "objective us", "objective them", "s us", "s them"
    );
    let (mut failures, mut ran, mut slower, mut total) = (Vec::new(), 0, 0, (0.0, 0.0));
    for case in reference["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if !only.is_empty() && !only.iter().any(|o| name.contains(o.as_str())) {
            continue;
        }
        let p = match problem_of(case, &small, &medium, &zone_tables) {
            Ok(p) => p,
            Err(why) => {
                println!("{name:<40} skipped: {why}");
                continue;
            }
        };
        let mut bad = Vec::new();
        if let Err(why) = check_model(&p, &case["model"]) {
            bad.push(format!("model: {why}"));
        }
        let want_optimal = case["status"] == "optimal";
        let runs = case["runs"].as_u64().unwrap() as usize;
        let r = match solve(&p, runs) {
            Ok(r) => r,
            Err(why) => {
                println!("{name:<40} {why}");
                failures.push(format!("{name}: {why}"));
                continue;
            }
        };
        ran += 1;
        let gap_asked = case["gap_asked"].as_f64().unwrap();
        let (want, secs) = (case["objective"].as_f64().unwrap(), case["seconds"].as_f64().unwrap());
        // the objective within the gap asked (and a rounding), when both proved it
        let allowed = (gap_asked + 1e-9) * want.abs().max(1.0);
        let same_objective = (r.objective - want).abs() <= allowed;
        if want_optimal && r.optimal && !same_objective {
            bad.push(format!("objective {} != {want} (gap {gap_asked})", r.objective));
        }
        if want_optimal != r.optimal {
            bad.push(format!("status: optimal {} here, {} there", r.optimal, case["status"]));
        }
        // the units: the same where the optimum is unique (proved by landschaft's cut), else told
        let theirs: Vec<usize> =
            case["selected"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect();
        assert_eq!(r.n, case["n"].as_u64().unwrap() as usize, "{name}: the units");
        let differ = r.selected.iter().filter(|i| !theirs.contains(i)).count()
            + theirs.iter().filter(|i| !r.selected.contains(i)).count();
        let unique = case["unique"].as_bool() == Some(true);
        if unique && gap_asked == 0.0 && r.optimal && differ > 0 {
            bad.push(format!("units: {differ} differ, the optimum being unique"));
        }
        if r.seconds > 1.5 * secs + 0.25 {
            bad.push(format!("time {:.3} s > 1.5 x {secs:.3} s + 0.25 s", r.seconds));
        }
        if r.seconds > secs {
            slower += 1;
        }
        total.0 += r.seconds;
        total.1 += secs;
        let units = match (differ, unique) {
            (0, _) => "same".to_string(),
            (d, true) => format!("{d} differ (unique)"),
            (d, false) => format!("{d} differ (not unique)"),
        };
        println!(
            "{name:<40} {:>8} {:>8} {:>14.6} {:>14.6} {:>8.3} {:>8.3}  {units}{}",
            if r.optimal { "optimal" } else { "no" },
            case["status"].as_str().unwrap(),
            r.objective,
            want,
            r.seconds,
            secs,
            if bad.is_empty() { String::new() } else { format!("  <-- {}", bad.join("; ")) }
        );
        failures.extend(bad.into_iter().map(|b| format!("{name}: {b}")));
    }
    println!("{ran} cases, {slower} slower than landschaft, {:.2} s here and {:.2} s there", total.0, total.1);
    assert!(failures.is_empty(), "{} divergences:\n{}", failures.len(), failures.join("\n"));
}
