//! Metapopulations against R's MetaLandSim 2.0 (tests/data/reference/metapopulation.npz,
//! tools/make_metapopulation_reference.py): spom iterated 40 steps from set.seed on 60 patches, four combinations of
//! the kernel, the connectivity, the colonisation and the extinction; the occupancy after every step.

use std::path::Path;

use terraflux_ls_sys::RRng;
use terraflux_ops::metapopulation::{Colonisation, Connectivity, Extinction, Ifm, Kernel};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};
use terraflux_raster::npy::Array;

fn reference() -> std::collections::BTreeMap<String, Array> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    terraflux_raster::npy::read_npz(dir.join("metapopulation.npz")).unwrap()
}

/// A case's model from its settings: alpha, x, y, e, beta1, b, c1, c2, z, R, seed.
fn model(name: &str, s: &[f64]) -> Ifm {
    let (kernel, connectivity, colonisation, extinction) = match name {
        "hanski" | "dense" => (Kernel::Exponential, Connectivity::Plain, Colonisation::Logistic, Extinction::Area),
        "rescue" => (
            Kernel::Exponential,
            Connectivity::Area { c1: s[6] },
            Colonisation::Exponential,
            Extinction::Rescue { r: s[9] },
        ),
        "power" => (
            Kernel::Power { beta: s[4] },
            Connectivity::Plain,
            Colonisation::Power { c2: s[7], z: s[8] },
            Extinction::Exponential,
        ),
        _ => unreachable!(),
    };
    Ifm { kernel, connectivity, colonisation, extinction, alpha: s[0], x: s[1], y: s[2], e: s[3], b: s[5] }
}

#[test]
fn metalandsims_runs() {
    let z = reference();
    let p = &z["patches"].data;
    let xy: Vec<[f64; 2]> = p.chunks(4).map(|r| [r[0], r[1]]).collect();
    let area: Vec<f64> = p.chunks(4).map(|r| r[2]).collect();
    let start: Vec<bool> = p.chunks(4).map(|r| r[3] != 0.0).collect();
    for name in ["hanski", "rescue", "power", "dense"] {
        let s = &z[format!("{name}/settings").as_str()].data;
        let m = model(name, s);
        let mut rng = RRng::new(s[10] as i32);
        let want = &z[format!("{name}/occupancy").as_str()];
        let mut occ = start.clone();
        for t in 0..want.shape[0] {
            occ = m.step(&xy, &area, &occ, &mut rng);
            let row: Vec<bool> = want.data[t * 60..(t + 1) * 60].iter().map(|v| *v != 0.0).collect();
            assert_eq!(occ, row, "{name}: step {}", t + 1);
        }
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
fn the_operator_on_a_table() {
    let z = reference();
    let patches = Table {
        columns: ["x", "y", "area", "occupied"].map(String::from).to_vec(),
        rows: z["patches"].data.chunks(4).map(|r| r.to_vec()).collect(),
        ..Default::default()
    };
    let o = run(
        "conn.metapopulation",
        vec![("patches", Value::Table(patches))],
        serde_json::json!({"steps": 40, "alpha": 0.08, "x": 0.5, "y": 8.0, "e": 0.4, "seed": 3}),
    );
    let Value::Table(t) = &o["table"] else { panic!() };
    let want = &z["hanski/occupancy"];
    for step in 1..=40 {
        let n: f64 = want.data[(step - 1) * 60..step * 60].iter().sum();
        assert_eq!(t.rows[step][1], n, "step {step}");
    }
    let Value::Table(p) = &o["patches"] else { panic!() };
    let last: Vec<f64> = want.data[39 * 60..].to_vec();
    assert_eq!(p.rows.iter().map(|r| r[4]).collect::<Vec<_>>(), last);
    assert!(p.rows.iter().all(|r| (0.0..=1.0).contains(&r[5])));
}
