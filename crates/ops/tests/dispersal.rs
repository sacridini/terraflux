//! Species dispersal against MigClim 1.6.2 (tests/data/reference/dispersal.npz,
//! tools/make_dispersal_reference.py: MigClim's C built with a seed): three simulations of 3 periods on 40 x 50
//! cells (weak barriers and long-distance dispersal; strong barriers, a threshold and propagule production that
//! grows with age; no barriers and long-distance dispersal every step), every step's state, the final state and
//! the statistics; to the bit.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::dispersal::{BarrierType, NO_DATA, Settings, migrate};
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Stack, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;

fn reference() -> std::collections::BTreeMap<String, Array> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    terraflux_raster::npy::read_npz(dir.join("dispersal.npz")).unwrap()
}

fn ints(v: &[f64]) -> Vec<i32> {
    v.iter().map(|x| *x as i32).collect()
}

/// The three simulations of the reference: name, seed, barrier, threshold, steps, kernel, maturity, propagules,
/// long-distance dispersal.
#[allow(clippy::type_complexity)]
fn cases() -> Vec<(&'static str, Settings, bool)> {
    let s = |seed, barrier_type, threshold, steps, kernel: &[f64], ini, full, propagules: &[f64], ldd, lmin, lmax| {
        Settings {
            steps,
            kernel: kernel.iter().map(|p| *p as f32 as f64).collect(),
            ini_maturity: ini,
            full_maturity: full,
            propagules: propagules.iter().map(|p| *p as f32 as f64).collect(),
            threshold,
            ldd_frequency: ldd as f32 as f64,
            ldd_min: lmin,
            ldd_max: lmax,
            barrier_type,
            seed,
        }
    };
    vec![
        ("weak", s(7, BarrierType::Weak, 0, 5, &[1.0, 0.4, 0.16, 0.06], 1, 1, &[1.0], 0.1, 6, 15), true),
        ("strong", s(11, BarrierType::Strong, 250, 4, &[0.9, 0.6, 0.3], 1, 4, &[0.3, 0.6, 0.9], 0.0, 10, 15), true),
        ("open", s(3, BarrierType::Strong, 0, 3, &[0.9, 0.5], 2, 3, &[0.6], 1.0, 3, 10), false),
    ]
}

#[test]
fn migclims_runs() {
    let z = reference();
    let (ny, nx) = (40, 50);
    let ini = ints(&z["land/ini"].data);
    let hs: Vec<Vec<i32>> = (1..=3).map(|k| ints(&z[format!("land/hs{k}").as_str()].data)).collect();
    let bar = ints(&z["land/barrier"].data);
    for (name, s, barriers) in cases() {
        let r = migrate((ny, nx), &ini, &hs, barriers.then_some(&bar[..]), &s);
        let steps = &z[format!("{name}/steps").as_str()];
        assert_eq!(r.steps.len(), steps.shape[0], "{name}: steps");
        for (k, (id, m)) in r.steps.iter().enumerate() {
            let want = ints(&steps.data[k * ny * nx..(k + 1) * ny * nx]);
            if let Some(c) = m.iter().zip(&want).position(|(a, b)| a != b) {
                panic!("{name}: step {id}: cell {c}: {} {}", m[c], want[c]);
            }
        }
        assert_eq!(r.fin, ints(&z[format!("{name}/final").as_str()].data), "{name}: final");
        let stats: Vec<i32> = r.stats.iter().flatten().copied().collect();
        assert_eq!(stats, ints(&z[format!("{name}/stats").as_str()].data), "{name}: statistics");
    }
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

#[test]
fn the_operator_on_maps() {
    let z = reference();
    let grid = Arc::new(Grid::new(50, 40));
    let map = |k: &str| {
        let d: Vec<f64> = z[k].data.iter().map(|v| if *v as i32 == NO_DATA { f64::NAN } else { *v }).collect();
        Map::new(grid.clone(), d)
    };
    let suit = Value::Stack(Stack {
        grid: grid.clone(),
        keys: (1..=3).map(|k| terraflux_ops::Key::Name(format!("p{k}"))).collect(),
        layers: (1..=3).map(|k| map(&format!("land/hs{k}")).data).collect(),
    });
    let o = run(
        "sdm.migrate",
        vec![
            ("initial", Value::Map(map("land/ini"))),
            ("suitability", suit),
            ("barriers", Value::Map(map("land/barrier"))),
        ],
        serde_json::json!({"steps": 5, "kernel": [1.0, 0.4, 0.16, 0.06], "ldd_frequency": 0.1, "ldd_min": 6,
                           "ldd_max": 15, "barrier_type": "weak", "seed": 7}),
    );
    let Value::Map(m) = &o["map"] else { panic!() };
    let want = &z["weak/final"].data;
    for (k, (a, b)) in m.data.iter().zip(want).enumerate() {
        assert!(a == b || (a.is_nan() && *b as i32 == NO_DATA), "cell {k}: {a} {b}");
    }
    let Value::Stack(s) = &o["steps"] else { panic!() };
    assert_eq!(s.layers.len(), 15);
    let Value::Table(t) = &o["table"] else { panic!() };
    assert_eq!(t.rows.len(), 16);
}
