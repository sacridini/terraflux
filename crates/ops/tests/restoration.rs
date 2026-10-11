//! Restoration planning's inputs against plangea's R: the outputs landschaft keeps
//! (extern/landschaft/tests/data/reference/plangea) of the maps its tests draw (tests/data/reference/
//! plangea_inputs.npz, tools/make_plangea_inputs.py), compared as landschaft's test_plangea_reference.py does.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Map, Registry, Stack, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;

struct Data {
    inputs: BTreeMap<String, Array>,
    meta: serde_json::Value,
    reference: BTreeMap<String, Array>,
    texts: serde_json::Value,
}

fn data() -> Data {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let refdir = root.join("extern/landschaft/tests/data/reference/plangea");
    let all: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(refdir.join("reference.json")).unwrap()).unwrap();
    Data {
        inputs: terraflux_raster::npy::read_npz(root.join("tests/data/reference/plangea_inputs.npz")).unwrap(),
        meta: serde_json::from_str(
            &std::fs::read_to_string(root.join("tests/data/reference/plangea_inputs.json")).unwrap(),
        )
        .unwrap(),
        reference: terraflux_raster::npy::read_npz(refdir.join("reference.npz")).unwrap(),
        texts: all["texts"].clone(),
    }
}

fn grid(ny: usize, nx: usize) -> Arc<Grid> {
    Arc::new(Grid::new(nx, ny))
}

/// A stack of the layers of an array (k, y, x), named.
fn stack(a: &Array, names: &[String]) -> Value {
    let (ny, nx) = (a.shape[1], a.shape[2]);
    Value::Stack(Stack {
        grid: grid(ny, nx),
        keys: names.iter().map(|n| Key::Name(n.clone())).collect(),
        layers: a.data.chunks(ny * nx).map(|c| Arc::new(c.to_vec())).collect(),
    })
}

fn map(a: &Array) -> Value {
    Value::Map(Map::new(grid(a.shape[0], a.shape[1]), a.data.clone()))
}

fn strings(v: &serde_json::Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

/// R's 1-based cells as indices.
fn cells(a: &Array) -> Vec<usize> {
    a.data.iter().map(|&c| c as usize - 1).collect()
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

/// Beyer et al.'s intactness is plangea's (multi_calc_it) to the bit: with restoration, other parameters and cells
/// of other sizes; inside the windows that the map's edges do not clip, plangea's original function's too; and
/// its gradient the derivative of plangea's total.
#[test]
fn intactness_is_plangeas() {
    let d = data();
    for (name, spec) in d.meta["it_cases"].as_object().unwrap() {
        let h = &d.inputs[&format!("{name}/footprint")];
        let (ny, nx) = (h.shape[0], h.shape[1]);
        let mut inputs = vec![("footprint", map(h))];
        if let Some(a) = d.inputs.get(&format!("{name}/reduction")) {
            inputs.push(("reduction", map(a)));
            let x = match d.inputs.get(&format!("{name}/restored")) {
                Some(x) => map(x),
                None => Value::Map(Map::new(grid(ny, nx), vec![0.0; ny * nx])),
            };
            inputs.push(("restored", x));
        }
        let params = serde_json::json!({
            "radius": spec["radius"], "gamma": spec["gamma"], "beta": spec["beta"], "z": spec["z"],
            "cell_size": spec["res"].as_f64().unwrap() / 1000.0,
        });
        let out = run("restoration.intactness", inputs, params.clone());
        let Value::Map(q) = &out["map"] else { panic!() };
        let at = cells(&d.reference[&format!("{name}/master_index")]);
        let fixed = &d.reference[&format!("{name}/fixed")].data;
        for (i, &c) in at.iter().enumerate() {
            assert!(same(q.data[c], fixed[i]), "{name}: cell {c}: {} != {}", q.data[c], fixed[i]);
        }
        // plangea's original function (no clipped windows) where the windows are whole
        let r = spec["radius"].as_u64().unwrap() as usize;
        let original = &d.reference[&format!("{name}/original")].data;
        let mut checked = 0;
        for (i, &c) in at.iter().enumerate() {
            let (y, x) = (c / nx, c % nx);
            if y >= r && y + r + 1 < ny && x >= r && x + r + 1 < nx && !h.data[c].is_nan() {
                assert!(same(q.data[c], original[i]), "{name}: inner cell {c}");
                checked += 1;
            }
        }
        assert!(checked > 0, "{name}");
        if name == "it_gradient" {
            let a = &d.inputs["it_gradient/reduction"];
            let out =
                run("restoration.intactness_gradient", vec![("footprint", map(h)), ("reduction", map(a))], params);
            let Value::Map(g) = &out["map"] else { panic!() };
            let want = &d.reference["it_gradient/gradient"].data;
            for (i, &c) in at.iter().enumerate() {
                assert!(
                    (g.data[c] - want[i]).abs() <= 1e-6 * want[i].abs(),
                    "gradient at {c}: {} != {}",
                    g.data[c],
                    want[i]
                );
            }
        }
    }
}

/// The original vegetation of plangea's harmonize_oa, to the bit, for every case kept.
fn vegetation(d: &Data, spec: &serde_json::Value) -> terraflux_ops::Outputs {
    let natural = strings(&d.meta["natural"]);
    let mut inputs = vec![("land_cover", stack(&d.inputs["land_cover"], &strings(&d.meta["classes"])))];
    if spec["past"].as_bool().unwrap() {
        inputs.push(("past", stack(&d.inputs["past"], &natural)));
    }
    if spec["er"].as_bool().unwrap() {
        inputs.push(("ecoregions", stack(&d.inputs["ecoregions"], &natural)));
    }
    let current = spec["problem"].as_array().unwrap().iter().any(|p| p == "D");
    run(
        "restoration.original_vegetation",
        inputs,
        serde_json::json!({
            "natural": d.meta["natural"], "anthropic": d.meta["anthropic"], "failsafe": spec["failsafe"],
            "restore_to": if current { "current" } else { "original" },
        }),
    )
}

#[test]
fn original_vegetation_is_plangeas() {
    let d = data();
    let natural = strings(&d.meta["natural"]);
    for (name, spec) in d.meta["oa_cases"].as_object().unwrap() {
        let out = vegetation(&d, spec);
        let at = cells(&d.reference[&format!("{name}/master_index")]);
        for port in ["original", "restore"] {
            let Value::Stack(s) = &out[port] else { panic!() };
            let key = if port == "original" { "oa" } else { "restore" };
            for (k, c) in natural.iter().enumerate() {
                let want = &d.reference[&format!("{name}/{key}/{c}")].data;
                for (i, &cell) in at.iter().enumerate() {
                    assert!(
                        same(s.layers[k][cell], want[i]),
                        "{name} {port} {c} at {cell}: {} != {}",
                        s.layers[k][cell],
                        want[i]
                    );
                }
            }
        }
    }
    // where it came from, and the failsafe shares adding up to 1
    let out = vegetation(&d, &d.meta["oa_cases"]["oa_past_er"]);
    let Value::Map(source) = &out["source"] else { panic!() };
    let nx = 11;
    assert!(source.data[0].is_nan());
    assert_eq!((source.data[nx + 1], source.data[2 * nx + 2], source.data[8 * nx + 10]), (1.0, 1.0, 2.0));
    let Value::Table(f) = &out["failsafe"] else { panic!() };
    assert_eq!(f.rows.len(), 2);
    assert!((f.rows.iter().map(|r| r[1]).sum::<f64>() - 1.0).abs() < 1e-12);
}

/// The area of habitat of plangea's harmonize_bd, to the bit: the species that count, their areas now and
/// potential, and the habitat they gain per cell restored.
#[test]
fn area_of_habitat_is_plangeas() {
    let d = data();
    let species = strings(&d.meta["species"]);
    for (name, spec) in d.meta["bd_cases"].as_object().unwrap() {
        let veg = vegetation(&d, &d.meta["oa_cases"][spec["oa"].as_str().unwrap()]);
        let mut inputs = vec![
            ("ranges", stack(&d.inputs["ranges"], &species)),
            ("land_cover", stack(&d.inputs["land_cover"], &strings(&d.meta["classes"]))),
            ("original", veg["original"].clone()),
            ("restore", veg["restore"].clone()),
            ("anthropic", veg["anthropic"].clone()),
        ];
        let mut params = serde_json::json!({
            "natural": d.meta["natural"], "suitability": d.meta["suitability"], "cell_area": 1.0,
            "include_unlisted": spec["include_absent"],
        });
        if spec["elevation"].as_bool().unwrap() {
            inputs.push(("elevation", map(&d.inputs["elevation"])));
            params["elevation_limits"] = d.meta["elevation_limits"].clone();
        }
        let out = run("sdm.area_of_habitat", inputs, params);
        let Value::Table(t) = &out["table"] else { panic!() };
        let valid: Vec<String> =
            species.iter().enumerate().filter(|(i, _)| t.rows[*i][3] == 1.0).map(|(_, s)| s.clone()).collect();
        let mut want_valid = strings(&d.texts[format!("{name}/species")]);
        want_valid.sort();
        let mut got = valid.clone();
        got.sort();
        assert_eq!(got, want_valid, "{name}: the species that count");
        let (now, pot) = (&d.reference[&format!("{name}/hab_now")].data, &d.reference[&format!("{name}/hab_pot")].data);
        let order = strings(&d.texts[format!("{name}/species")]);
        let Value::Stack(gain) = &out["gain"] else { panic!() };
        for (i, s) in order.iter().enumerate() {
            let row = species.iter().position(|x| x == s).unwrap();
            assert!(same(t.rows[row][0], now[i]) && same(t.rows[row][1], pot[i]), "{name} {s}: areas");
            let mut expected = vec![0.0; gain.layers[row].len()];
            let idx = &d.reference[&format!("{name}/gain_index/{s}")];
            let g = &d.reference[&format!("{name}/gain/{s}")];
            for (c, v) in cells(idx).into_iter().zip(&g.data) {
                expected[c] = *v;
            }
            assert!(gain.layers[row].iter().zip(&expected).all(|(a, b)| same(*a, *b)), "{name} {s}: gain");
        }
    }
}

/// bd_absent's restoration problem: the vegetation (oa_past_er), the area of habitat and its gain, cells of 1 km².
fn plan_inputs(d: &Data) -> (Vec<(&'static str, Value)>, terraflux_ops::restoration_plan::Plan) {
    let spec = &d.meta["bd_cases"]["bd_absent"];
    let veg = vegetation(d, &d.meta["oa_cases"][spec["oa"].as_str().unwrap()]);
    let species = strings(&d.meta["species"]);
    let aoh = run(
        "sdm.area_of_habitat",
        vec![
            ("ranges", stack(&d.inputs["ranges"], &species)),
            ("land_cover", stack(&d.inputs["land_cover"], &strings(&d.meta["classes"]))),
            ("original", veg["original"].clone()),
            ("restore", veg["restore"].clone()),
            ("anthropic", veg["anthropic"].clone()),
        ],
        serde_json::json!({"natural": d.meta["natural"], "suitability": d.meta["suitability"], "cell_area": 1.0,
                           "include_unlisted": spec["include_absent"]}),
    );
    let (Value::Map(anthropic), Value::Table(t), Value::Stack(gain)) = (&veg["anthropic"], &aoh["table"], &aoh["gain"])
    else {
        panic!()
    };
    let n = anthropic.data.len();
    let mut p = terraflux_ops::restoration_plan::Plan::new(
        &anthropic.data,
        &vec![1.0; n],
        &vec![1.0; n],
        (anthropic.grid.height, anthropic.grid.width),
    )
    .unwrap();
    let layers: Vec<&[f64]> = gain.layers.iter().map(|l| l.as_slice()).collect();
    let k = t.rows.len();
    p.add_risk(
        &t.column("current").unwrap(),
        &t.column("potential").unwrap(),
        &t.column("valid").unwrap().iter().map(|v| *v != 0.0).collect::<Vec<_>>(),
        &layers,
        &vec![0.25; k],
        &vec![0.0; k],
        1.0,
    )
    .unwrap();
    let inputs =
        vec![("restorable", veg["anthropic"].clone()), ("table", aoh["table"].clone()), ("gain", aoh["gain"].clone())];
    (inputs, p)
}

/// plangea's restoration problem: its units and bounds, its biodiversity layer (the finite-difference slopes of
/// the species' risk times the habitat a unit of a cell restored gives; to the last bits but one), and its
/// procedure (scen: the target in steps, the layer refreshed between them, each step's linear program; HiGHS
/// here, Gurobi there: their optimum is unique), as landschaft's test_prioritize_restoration.
#[test]
fn plan_steps_is_plangeas() {
    let d = data();
    let (inputs, p) = plan_inputs(&d);
    let master = cells(&d.reference["bd_absent/master_index"]);
    assert_eq!(p.index, master, "the units");
    assert_eq!(p.upper, d.reference["scen_1/ub"].data, "the bounds");
    let close = |what: &str, a: &[f64], b: &[f64], rtol: f64, atol: f64| {
        assert_eq!(a.len(), b.len(), "{what}");
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            assert!((x - y).abs() <= atol + rtol * y.abs(), "{what}[{i}]: {x} != {y}");
        }
    };
    close("bd", &p.steps_layer(&p.risks[0], &vec![0.0; p.n()]), &d.reference["bd_absent/bd"].data, 1e-15, 0.0);
    for (name, steps) in [("scen_1", 1usize), ("scen_3", 3), ("scen_4_60", 4)] {
        let target = d.reference[&format!("{name}/target")].data[0];
        let out = run(
            "restoration.plan",
            inputs.clone(),
            serde_json::json!({"method": "steps", "steps": steps, "target": target, "cell_area": 1.0}),
        );
        let Value::Map(restored) = &out["restored"] else { panic!() };
        let x: Vec<f64> = p.index.iter().map(|&c| restored.data[c]).collect();
        close(name, &x, &d.reference[&format!("{name}/x")].data, 0.0, 1e-12);
        for k in 1..steps {
            let cum = &d.reference[&format!("{name}/cum_{k}")].data;
            close(
                &format!("{name} bd_{k}"),
                &p.steps_layer(&p.risks[0], cum),
                &d.reference[&format!("{name}/bd_{k}")].data,
                1e-7,
                0.0,
            );
        }
    }
}

/// The exact plan: optimal within the tolerance (the Frank-Wolfe bound), no worse than plangea's procedure and the
/// uniform plan for the same area, the area at the target; the uniform plan every unit by the same share.
#[test]
fn plan_exact_is_the_optimum() {
    let d = data();
    let (inputs, mut p) = plan_inputs(&d);
    let target = d.reference["scen_1/target"].data[0];
    p.add_area_target(&[(None, target)], None, "<=", false).unwrap();
    let (x, bound, status, _) = p.exact(1e-9, f64::INFINITY).unwrap();
    assert_eq!(status, "optimal");
    let f = p.objective(&x);
    assert!(bound >= f && bound - f <= 1e-9 * bound.abs().max(1.0), "{f} {bound}");
    let area: f64 = p.area.iter().zip(&x).map(|(a, x)| a * x).sum();
    assert!(area <= target * (1.0 + 1e-9));
    for steps in [1, 3] {
        let s = p.steps(steps).unwrap();
        assert!(f >= p.objective(&s) - 1e-12, "steps {steps}: {} > {f}", p.objective(&s));
    }
    let u = p.uniform(target);
    assert!(f >= p.objective(&u));
    let total: f64 = p.upper.iter().zip(&p.area).map(|(u, a)| u * a).sum();
    for (a, b) in u.iter().zip(&p.upper) {
        assert!((a - b * (target / total)).abs() <= 1e-15 * b.max(1e-300));
    }
    // the operator's
    let out =
        run("restoration.plan", inputs, serde_json::json!({"target": target, "cell_area": 1.0, "tolerance": 1e-9}));
    let Value::Table(s) = &out["summary"] else { panic!() };
    assert_eq!(s.column("optimal").unwrap()[0], 1.0);
    assert!((s.column("benefit").unwrap()[0] - f).abs() <= 1e-9 * f.abs().max(1.0));
}
