//! The accuracy and area of a map from a stratified sample against numpy's sampling and R's mapaccuracy
//! (tests/data/reference/accuracy.json, tools/make_accuracy_reference.py): the points drawn as numpy draws them, to
//! the bit; olofsson and stehman2014 to 1e-12 (R sums in long double); the published example of Olofsson et al.
//! (2014) and its sample size.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::accuracy::{Allocation, allocate, qnorm, sample_size};
use terraflux_ops::signatures::NumpyRng;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn reference() -> serde_json::Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/accuracy.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn nums(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect()
}

fn close(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        let same = (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-12 * b.abs().max(1e-3);
        assert!(same, "{what}: at {k}: {a} {b}");
    }
}

fn table(v: &Value) -> &Table {
    let Value::Table(t) = v else { panic!("{v:?}") };
    t
}

fn col(t: &Table, name: &str) -> Vec<f64> {
    t.column(name).unwrap_or_else(|| panic!("no column {name}"))
}

/// The table of accuracy.assess against an estimator of mapaccuracy.
fn same_as_r(out: &Outputs, r: &serde_json::Value) {
    let t = table(&out["table"]);
    close("classes", &col(t, "class"), &nums(&r["classes"]));
    for (ours, theirs) in [
        ("user_accuracy", "UA"),
        ("user_se", "SEua"),
        ("producer_accuracy", "PA"),
        ("producer_se", "SEpa"),
        ("area_share", "area"),
        ("area_share_se", "SEa"),
    ] {
        close(ours, &col(t, ours), &nums(&r[theirs]));
    }
    let o = table(&out["overall"]);
    close("overall", &col(o, "overall_accuracy"), &nums(&r["OA"]));
    close("overall se", &col(o, "overall_se"), &nums(&r["SEoa"]));
    let m: Vec<f64> = table(&out["matrix"]).rows.iter().flat_map(|row| row[1..].to_vec()).collect();
    close("matrix", &m, &nums(&r["matrix"]));
}

fn grid(r: &serde_json::Value) -> Arc<Grid> {
    let (ny, nx) = (r["shape"][0].as_u64().unwrap() as usize, r["shape"][1].as_u64().unwrap() as usize);
    let mut g = Grid::new(nx, ny);
    g.transform = serde_json::from_value(r["transform"].clone()).unwrap();
    Arc::new(g)
}

#[test]
fn numpy_draws_without_replacement_as_numpy() {
    // numpy 2.5: default_rng(seed).choice(pop, size, replace=False), its first and last draws and their sum, then
    // integers(1000): both branches (Floyd's and the tail shuffle) and the generator's state after them
    for (seed, pop, size, head, tail, sum, next) in [
        (1u64, 10usize, 3usize, vec![4, 3, 7], vec![4, 3, 7], 14usize, 144u64),
        (7, 100, 40, vec![10, 98, 93, 80, 61, 77, 86, 23], vec![47, 3, 9], 2040, 154),
        (3, 20000, 50, vec![2784, 1537, 9563, 15990, 17834, 2270, 13311, 3580], vec![8646, 3453, 5679], 477171, 393),
        (
            5,
            20000,
            1000,
            vec![10974, 12442, 12953, 16040, 7964, 16773, 4571, 17762],
            vec![453, 16099, 13415],
            9948058,
            89,
        ),
        (
            9,
            12000,
            12000,
            vec![3635, 7094, 10050, 4410, 6705, 2978, 1022, 8194],
            vec![11530, 10442, 5058],
            71994000,
            619,
        ),
        (2, 5, 5, vec![4, 0, 1, 3, 2], vec![1, 3, 2], 10, 334),
        (4, 1, 1, vec![0], vec![0], 0, 726),
    ] {
        let mut g = NumpyRng::new(seed);
        let a = g.choice_distinct(pop, size);
        assert_eq!(a[..head.len()], head[..], "seed {seed}");
        assert_eq!(a[a.len() - tail.len()..], tail[..], "seed {seed}");
        assert_eq!(a.iter().sum::<usize>(), sum, "seed {seed}");
        assert_eq!(g.integers(1000), next, "seed {seed}");
    }
}

#[test]
fn the_sample_is_numpys() {
    let r = reference();
    let g = grid(&r);
    let map = Value::Map(Map::new(g.clone(), nums(&r["map"])));
    let out = run("accuracy.sample", vec![("map", map)], serde_json::json!({"n": r["sizes"], "seed": r["seed"]}));
    let Value::Vector(v) = &out["points"] else { panic!() };
    let cells: Vec<f64> = r["cells"].as_array().unwrap().iter().map(|c| c.as_f64().unwrap()).collect();
    let rows = v.column("row").unwrap().numbers();
    let cols = v.column("col").unwrap().numbers();
    let got: Vec<f64> = rows.iter().zip(&cols).map(|(r, c)| r * g.width as f64 + c).collect();
    assert_eq!(got, cells);
    assert_eq!(v.column("stratum").unwrap().numbers(), nums(&r["stratum_of"]));
    let Geometry::Point(p) = v.geometries[0] else { panic!() };
    assert_eq!(p, [500000.0 + 30.0 * (cols[0] + 0.5), 8000000.0 - 30.0 * (rows[0] + 0.5)]);
    let Value::Map(m) = &out["cells"] else { panic!() };
    assert_eq!(m.data.iter().filter(|v| !v.is_nan()).count(), cells.len());

    // the sizes from accuracy.sample_size's table draw the same
    let mut sizes = Table::new(vec!["stratum".into(), "n".into()]);
    for (k, n) in r["sizes"].as_object().unwrap() {
        sizes.rows.push(vec![k.parse().unwrap(), n.as_f64().unwrap()]);
    }
    let again = run(
        "accuracy.sample",
        vec![("map", Value::Map(Map::new(g, nums(&r["map"])))), ("sizes", Value::Table(sizes))],
        serde_json::json!({"seed": r["seed"]}),
    );
    let Value::Vector(w) = &again["points"] else { panic!() };
    assert_eq!(w.geometries, v.geometries);
}

/// The sample's points labelled with their reference class.
fn labelled(r: &serde_json::Value, g: &Grid) -> Value {
    let cells = r["cells"].as_array().unwrap();
    let geometries = cells
        .iter()
        .map(|c| {
            let i = c.as_u64().unwrap() as usize;
            let (row, col) = ((i / g.width) as f64, (i % g.width) as f64);
            Geometry::Point([g.transform[0] + 30.0 * (col + 0.5), g.transform[3] - 30.0 * (row + 0.5)])
        })
        .collect();
    let v =
        Vector { geometries, attributes: vec![("truth".into(), Column::Numbers(nums(&r["reference"])))], crs: None };
    Value::Vector(Arc::new(v))
}

#[test]
fn olofsson_and_stehman_are_mapaccuracys() {
    let r = reference();
    let g = grid(&r);
    let map = || Value::Map(Map::new(g.clone(), nums(&r["map"])));
    let out = run(
        "accuracy.assess",
        vec![("map", map()), ("points", labelled(&r, &g))],
        serde_json::json!({"reference": "truth"}),
    );
    same_as_r(&out, &r["r"]["olofsson"]);
    let t = table(&out["table"]);
    // the area in cells and hectares (cells of 30 m), and the samples of each class of the map
    let total = table(&out["overall"]).rows[0][4];
    close("area cells", &col(t, "area_cells"), &col(t, "area_share").iter().map(|a| a * total).collect::<Vec<_>>());
    close("area ha", &col(t, "area_ha"), &col(t, "area_cells").iter().map(|a| a * 0.09).collect::<Vec<_>>());
    assert_eq!(col(t, "samples"), vec![60.0, 50.0, 40.0, 30.0, 0.0]);
    let z = qnorm(0.975);
    close("ci", &col(t, "user_ci"), &col(t, "user_se").iter().map(|s| z * s).collect::<Vec<_>>());

    let out = run(
        "accuracy.assess",
        vec![("map", map()), ("points", labelled(&r, &g))],
        serde_json::json!({"reference": "truth", "estimator": "stehman"}),
    );
    same_as_r(&out, &r["r"]["stehman_map"]);

    let strata = Value::Map(Map::new(g.clone(), nums(&r["strata"])));
    let out = run(
        "accuracy.assess",
        vec![("map", map()), ("points", labelled(&r, &g)), ("strata", strata)],
        serde_json::json!({"reference": "truth"}),
    );
    same_as_r(&out, &r["r"]["stehman_strata"]);
}

/// Olofsson et al. (2014)'s example from its table of sample counts: the areas of section 5 (21 158 ± 6 158 ha of
/// deforestation in cells of 30 m), and its sample size of 641.
#[test]
fn olofssons_example() {
    let r = reference();
    let p = &r["paper"];
    let cells: Vec<f64> = nums(&p["cells"]);
    // a map with the example's cells by class (only their numbers matter): a row of each class's cells, a 50th of
    // them, each 50 cells of 30 m
    let width = 6_450_000 / 50;
    let mut data = vec![f64::NAN; 4 * width];
    for (k, &n) in cells.iter().enumerate() {
        data[k * width..k * width + n as usize / 50].fill((k + 1) as f64);
    }
    let mut g = Grid::new(width, 4);
    g.transform = [0.0, 30.0, 0.0, 0.0, 0.0, -30.0];
    let mut samples = Table::new(vec!["map".into(), "reference".into()]);
    for (m, rf) in nums(&p["map"]).iter().zip(nums(&p["reference"])) {
        samples.rows.push(vec![*m, rf]);
    }
    let out = run(
        "accuracy.assess",
        vec![("map", Value::Map(Map::new(Arc::new(g), data))), ("samples", Value::Table(samples))],
        serde_json::json!({"cell_area": 900.0 * 50.0}),
    );
    same_as_r(&out, &r["r"]["paper"]);
    let t = table(&out["table"]);
    assert_eq!(col(t, "area_ha")[0].round(), 21158.0);
    assert_eq!(col(t, "area_ha_ci")[0].round(), 6158.0);
    assert!((table(&out["overall"]).rows[0][0] - 0.95).abs() < 0.005);

    // the size: user's accuracies of 0.7, 0.6, 0.9 and 0.95, a standard error of 0.01; 75 for each change class
    let w = [0.02, 0.015, 0.32, 0.645];
    let sd: Vec<f64> = [0.7f64, 0.6, 0.9, 0.95].iter().map(|u| (u * (1.0 - u)).sqrt()).collect();
    let n = sample_size(&w, &sd, 0.01);
    assert_eq!(n, 641);
    let (sizes, over) = allocate(n, &w, &sd, &[200_000, 150_000, 3_200_000, 6_450_000], Allocation::Minimum(75));
    assert_eq!((sizes, over), (vec![75, 75, 163, 328], false));
    let (sizes, _) = allocate(n, &w, &sd, &[200_000, 150_000, 3_200_000, 6_450_000], Allocation::Proportional);
    assert_eq!(sizes, vec![13, 10, 205, 413]);
}

#[test]
fn the_sample_size_operator() {
    // 2000 cells in the example's shares
    let mut data = Vec::new();
    for (c, n) in [(1.0, 40), (2.0, 30), (3.0, 640), (4.0, 1290)] {
        data.extend(std::iter::repeat_n(c, n));
    }
    let out = run(
        "accuracy.sample_size",
        vec![("map", Value::Map(Map::new(Arc::new(Grid::new(2000, 1)), data)))],
        serde_json::json!({"user_accuracy": {"1": 0.7, "2": 0.6, "3": 0.9, "4": 0.95}, "minimum": 30}),
    );
    let t = table(&out["table"]);
    // 30 for the two rare classes, the 581 left in proportion to 640 and 1290 cells
    assert_eq!(col(t, "n"), vec![30.0, 30.0, 193.0, 388.0]);
    assert!(matches!(out["n"], Value::Integer(641)));
    assert!((qnorm(0.975) - 1.959963984540054).abs() < 1e-15);
    assert!((qnorm(0.995) - 2.5758293035489004).abs() < 1e-15);
    assert!((qnorm(0.05) + 1.6448536269514729).abs() < 1e-15);
}
