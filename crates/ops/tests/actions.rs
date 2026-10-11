//! prioritize.actions against prioriactions 0.5.0: the model its problem() gives (extern/landschaft/tests/data/
//! reference/optimize/actions) bit for bit, its optima (Gurobi's at gap 0), and the figures of prioriactions's own
//! solutions (getCost, getSolutionBenefit, getPotentialBenefit, getConnectivityPenalty, getActions); on the tables of
//! landschaft's cases (tests/data/reference/optimize_actions_tables.json, tools/make_optimize_inputs.py).

use std::collections::BTreeMap;
use std::path::Path;

use terraflux_ops::actions::{ActionsData, Settings, actions_data, actions_problem, compile, figures, potential};
use terraflux_ops::{Args, Table, Value};
use terraflux_raster::npy::Array;

struct Reference {
    arrays: BTreeMap<String, Array>,
    meta: serde_json::Value,
}

fn reference() -> Reference {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extern/landschaft/tests/data/reference/optimize");
    Reference {
        arrays: terraflux_raster::npy::read_npz(dir.join("actions.npz")).unwrap(),
        meta: serde_json::from_str(&std::fs::read_to_string(dir.join("actions.json")).unwrap()).unwrap(),
    }
}

impl Reference {
    fn floats(&self, key: &str) -> Vec<f64> {
        if let Some(a) = self.arrays.get(key) {
            return a.data.clone();
        }
        // numbers kept as text (prioriactions's data frames)
        self.texts(key).iter().map(|v| v.parse().unwrap_or_else(|_| panic!("{key}: {v}"))).collect()
    }
    fn texts(&self, key: &str) -> Vec<String> {
        self.meta["arrays"][key]["values"]
            .as_array()
            .unwrap_or_else(|| panic!("{key}"))
            .iter()
            .map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))
            .collect()
    }
}

fn tables() -> BTreeMap<String, Table> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/optimize_actions_tables.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A case: its name, tables (pa_* by port) and settings (model, budget, blm, curve, segments).
type Case = (&'static str, BTreeMap<String, Table>, (bool, f64, f64, i32, i32));

fn cases() -> Vec<Case> {
    let t = tables();
    let base = || -> BTreeMap<String, Table> {
        [
            ("pu", "pa_pu"),
            ("features", "pa_features"),
            ("dist_features", "pa_dist_features"),
            ("threats", "pa_threats"),
            ("dist_threats", "pa_dist_threats"),
            ("sensitivity", "pa_sensitivity"),
            ("boundary", "pa_boundary"),
        ]
        .into_iter()
        .map(|(port, name)| (port.to_string(), t[name].clone()))
        .collect()
    };
    let set = |mut b: BTreeMap<String, Table>, port: &str, column: &str, values: &[f64]| {
        let tb = b.get_mut(port).unwrap();
        let c = match tb.columns.iter().position(|c| c == column) {
            Some(c) => c,
            None => {
                tb.columns.push(column.into());
                tb.rows.iter_mut().for_each(|r| r.push(0.0));
                tb.columns.len() - 1
            }
        };
        for (r, v) in tb.rows.iter_mut().zip(values) {
            r[c] = *v;
        }
        b
    };
    vec![
        ("actions_mincost", base(), (false, 0.0, 0.0, 1, 3)),
        ("actions_mincost_blm", set(base(), "threats", "blm_actions", &[0.5, 0.25]), (false, 0.0, 1.0, 1, 3)),
        (
            "actions_mincost_conservation",
            set(base(), "features", "target_conservation", &[3.0, 2.0, 4.0, 1.0]),
            (false, 0.0, 0.0, 1, 3),
        ),
        ("actions_maxbenefit", base(), (true, 60.0, 0.0, 1, 3)),
        ("actions_maxbenefit_blm", base(), (true, 60.0, 0.5, 1, 3)),
        // prioriactions's sensitivity columns a-d are not its deltas: ignored
        ("actions_mincost_curve3", base(), (false, 0.0, 0.0, 3, 3)),
    ]
}

fn data(name: &str, t: &BTreeMap<String, Table>, s: (bool, f64, f64, i32, i32)) -> (ActionsData, Settings) {
    let mut d = actions_data(
        &t["pu"],
        &t["features"],
        &t["dist_features"],
        &t["threats"],
        &t["dist_threats"],
        Some(&t["sensitivity"]),
        Some(&t["boundary"]),
    )
    .unwrap_or_else(|e| panic!("{name}: {e}"));
    let st = actions_problem(&mut d, s.0, s.1, s.2, s.3, s.4).unwrap_or_else(|e| panic!("{name}: {e}"));
    (d, st)
}

/// prioriactions's model, bit for bit.
#[test]
fn compile_is_prioriactions() {
    let r = reference();
    for (name, t, s) in cases() {
        let (d, st) = data(name, &t, s);
        let m = compile(&d, &st).unwrap();
        let k = |s: &str| format!("{name}/model/{s}");
        let (nrow, ncol) = (r.floats(&k("n_row"))[0] as usize, r.floats(&k("n_col"))[0] as usize);
        assert_eq!((m.start.len() - 1, m.obj.len()), (nrow, ncol), "{name}: shape");
        let (ai, aj, ax) = (r.floats(&k("A_i")), r.floats(&k("A_j")), r.floats(&k("A_x")));
        // the reference's triplets: duplicates added up, zeros left out, by row then column
        let mut acc: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for e in 0..ax.len() {
            *acc.entry((ai[e] as usize, aj[e] as usize)).or_insert(0.0) += ax[e];
        }
        let want: Vec<(usize, usize, u64)> =
            acc.into_iter().filter(|(_, v)| *v != 0.0).map(|((i, j), v)| (i, j, v.to_bits())).collect();
        let mut ours = Vec::new();
        for i in 0..nrow {
            for e in m.start[i] as usize..m.start[i + 1] as usize {
                if m.value[e] != 0.0 {
                    ours.push((i, m.index[e] as usize, m.value[e].to_bits()));
                }
            }
        }
        ours.sort();
        assert_eq!(ours.len(), want.len(), "{name}: entries");
        assert!(ours == want, "{name}: the matrix");
        let bits = |what: &str, a: &[f64], b: &[f64]| {
            assert_eq!(a.len(), b.len(), "{name}: {what}");
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{name}: {what}[{i}] {x} != {y}");
            }
        };
        bits("obj", &m.obj, &r.floats(&k("obj")));
        bits("lb", &m.lb, &r.floats(&k("lb")));
        bits("ub", &m.ub, &r.floats(&k("ub")));
        bits("rhs", &m.rhs, &r.floats(&k("rhs")));
        let sense: Vec<&str> = m.sense.iter().map(|s| ["<=", ">=", "="][*s as usize]).collect();
        let want_sense: Vec<String> =
            r.texts(&k("sense")).into_iter().map(|s| if s == "==" { "=".into() } else { s }).collect();
        assert_eq!(sense, want_sense, "{name}: sense");
        let vtype: Vec<&str> = m.integer.iter().map(|&b| if b != 0 { "B" } else { "C" }).collect();
        assert_eq!(vtype, r.texts(&k("vtype")), "{name}: vtype");
        assert_eq!(r.texts(&k("modelsense"))[0], if st.maximize { "max" } else { "min" }, "{name}");
    }
}

/// The optima are prioriactions's (Gurobi's at gap 0; with curve 3 Gurobi's piecewise-linear power function as
/// ours), through the operator.
#[test]
fn optimum_is_prioriactions() {
    let r = reference();
    let registry = terraflux_ops::Registry::standard();
    for (name, t, s) in cases() {
        let params = serde_json::json!({
            "model": if s.0 { "maximizeBenefits" } else { "minimizeCosts" },
            "budget": s.1, "blm": s.2, "curve": s.3, "segments": s.4, "gap": 0.0
        });
        let inputs: BTreeMap<String, Value> = t.into_iter().map(|(k, v)| (k, Value::Table(v))).collect();
        let args = Args { node: "n", inputs, params: params.as_object().unwrap() };
        let out = registry
            .get("prioritize.actions")
            .unwrap()
            .run(&mut terraflux_ops::Ctx::default(), &args)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let Value::Table(res) = &out["result"] else { panic!() };
        let ours = res.column("objective").unwrap()[0];
        let want = r.floats(&format!("{name}/objective"))[0];
        assert_eq!(res.column("optimal").unwrap()[0], 1.0, "{name}");
        assert!((ours - want).abs() <= 1e-9 * want.abs().max(1.0), "{name}: {ours} != {want}");
    }
}

/// The figures of prioriactions's own solutions: getCost, getSolutionBenefit (total and local),
/// getPotentialBenefit, getConnectivityPenalty and getActions.
#[test]
fn figures_are_prioriactions() {
    let r = reference();
    for (name, t, s) in cases() {
        let (d, _) = data(name, &t, s);
        let x = r.floats(&format!("{name}/solution"));
        let f = figures(&d, &x).unwrap();
        let col = |t: &Table, c: &str| t.column(c).unwrap_or_else(|| panic!("{name}: {c}"));
        let same = |what: &str, a: &[f64], b: &[f64]| {
            assert_eq!(a.len(), b.len(), "{name}: {what}");
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                assert!((x - y).abs() <= 1e-9 * y.abs().max(1.0), "{name}: {what}[{i}] {x} != {y}");
            }
        };
        for c in &f.cost.columns {
            same(&format!("cost {c}"), &col(&f.cost, c), &r.floats(&format!("{name}/cost/{c}")));
        }
        for c in ["benefit.conservation", "benefit.recovery", "benefit.total"] {
            same(&format!("benefit {c}"), &col(&f.benefit, c), &r.floats(&format!("{name}/benefit/{c}")));
            same(&format!("local {c}"), &col(&f.local, c), &r.floats(&format!("{name}/local/{c}")));
        }
        same("local pu", &col(&f.local, "pu"), &r.floats(&format!("{name}/local/pu")));
        let p = potential(&d).unwrap();
        for c in
            ["dist", "dist_threatened", "maximum.conservation.benefit", "maximum.recovery.benefit", "maximum.benefit"]
        {
            same(&format!("potential {c}"), &col(&p, c), &r.floats(&format!("{name}/potential/{c}")));
        }
        let c = f.connectivity.as_ref().unwrap();
        for k in &c.columns {
            same(&format!("connectivity {k}"), &col(c, k), &r.floats(&format!("{name}/connectivity/{k}")));
        }
        for k in ["pu", "1", "2", "conservation", "connectivity"] {
            same(&format!("actions {k}"), &col(&f.summary, k), &r.floats(&format!("{name}/actions/{k}")));
        }
    }
}
