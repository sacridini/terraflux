//! Multi-action planning (prioriactions 0.5.0, Salgado-Rojas et al. 2020), through landschaft's C++: which
//! planning units to monitor and which actions against which threats to take in them so that the features recover
//! (or are kept) to their targets at the least cost (minimizeCosts), or recover the most within a budget
//! (maximizeBenefits). The inputs are checked and completed as prioriactions's inputData(), the model is
//! problem()'s bit for bit, the solution's figures solve()'s, getCost()'s, getSolutionBenefit()'s,
//! getPotentialBenefit()'s, getConnectivityPenalty()'s and getActions()'s.
//!
//! As landschaft, only the correct forms of prioriactions's slips: with no status in dist_threats the actions are
//! free (prioriactions resets the units' status there); the maximum conservation benefit is the feature's amount
//! (not the units); the lock-in budget check indexes the units by position.

use std::collections::HashMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
pub use terraflux_ls_sys::ffi::{ActionsModel, ActionsSettings as Settings};

use crate::milp::r_round;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Actions));
}

/// A multi-action problem's inputs, checked and completed (inputData()): the tables' ids and the C++ inputs.
pub struct ActionsData {
    pub input: ffi::ActionsInput,
    /// the units', features' and threats' ids (in the model's order)
    pub unit_ids: Vec<f64>,
    pub feature_ids: Vec<f64>,
    pub threat_ids: Vec<f64>,
    /// every action's unit and threat id (dist_threats's rows)
    pub action_unit: Vec<f64>,
    pub action_threat: Vec<f64>,
    pub warnings: Vec<String>,
}

fn round3(v: f64) -> f64 {
    r_round(v, 3)
}

/// A column, or an error naming the table.
fn col(t: &Table, c: &str, what: &str) -> std::result::Result<Vec<f64>, String> {
    let v = t.column(c).ok_or_else(|| format!("{what} needs the column {c}"))?;
    if v.iter().any(|x| x.is_nan()) {
        return Err(format!("{what}: {c} has missing values"));
    }
    Ok(v)
}

/// An optional column (a default where missing).
fn opt_col(t: &Table, c: &str, default: f64) -> Vec<f64> {
    t.column(c).unwrap_or_else(|| vec![default; t.rows.len()])
}

fn ids_text(v: &[f64]) -> String {
    v.iter().map(|x| crate::value::class_label(*x)).collect::<Vec<_>>().join(" ")
}

/// inputData(): pu (id, monitoring_cost, status 0/2/3), features (id, target_recovery, target_conservation),
/// dist_features (pu, feature, amount), threats (id, blm_actions), dist_threats (pu, threat, amount, action_cost,
/// status), sensitivity (feature, threat, delta1-4; default every feature to every threat), boundary (id1, id2,
/// boundary).
pub fn actions_data(
    pu: &Table,
    features: &Table,
    dist_features: &Table,
    threats: &Table,
    dist_threats: &Table,
    sensitivity: Option<&Table>,
    boundary: Option<&Table>,
) -> std::result::Result<ActionsData, String> {
    let mut warnings = Vec::new();
    // planning units, by id
    let (id, mc) = (col(pu, "id", "pu")?, col(pu, "monitoring_cost", "pu")?);
    let status = opt_col(pu, "status", 0.0);
    if status.iter().any(|s| ![0.0, 2.0, 3.0].contains(s)) {
        return Err("pu status must be 0, 2 or 3".into());
    }
    let mut order: Vec<usize> = (0..id.len()).collect();
    order.sort_by(|&a, &b| id[a].total_cmp(&id[b]));
    if order.windows(2).any(|w| id[w[0]] == id[w[1]]) || id.is_empty() {
        return Err("pu needs unique ids".into());
    }
    if mc.iter().any(|c| *c < 0.0) {
        return Err("argument to pu has negative monitoring cost data".into());
    }
    let unit_ids: Vec<f64> = order.iter().map(|&k| id[k]).collect();
    let unit_status: Vec<f64> = order.iter().map(|&k| status[k]).collect();
    let monitoring: Vec<f64> = order.iter().map(|&k| round3(mc[k])).collect();
    // features
    let mut f_id = col(features, "id", "features")?;
    let mut f_rec = col(features, "target_recovery", "features")?;
    let mut f_cons = opt_col(features, "target_conservation", 0.0);
    // dist_features
    let (df_pu, df_f, df_a) = (
        col(dist_features, "pu", "dist_features")?,
        col(dist_features, "feature", "dist_features")?,
        col(dist_features, "amount", "dist_features")?,
    );
    if df_a.is_empty() || df_a.iter().any(|a| *a < 0.0) {
        return Err("dist_features needs rows with amounts >= 0".into());
    }
    if df_pu.iter().any(|p| !unit_ids.contains(p)) || df_f.iter().any(|f| !f_id.contains(f)) {
        return Err("dist_features refers to planning units or features that do not exist".into());
    }
    let df_rows: Vec<usize> = (0..df_a.len()).filter(|&k| df_a[k] != 0.0).collect();
    let mut seen = std::collections::HashSet::new();
    for &k in &df_rows {
        if !seen.insert((df_pu[k].to_bits(), df_f[k].to_bits())) {
            return Err("There are duplicate values of the pu and feature pair in dist_features".into());
        }
    }
    // threats
    let mut t_id = col(threats, "id", "threats")?;
    let mut t_blm = opt_col(threats, "blm_actions", 0.0);
    if t_blm.iter().any(|b| *b < 0.0) {
        return Err("blm_actions must be >= 0".into());
    }
    // dist_threats: the actions in locked-out units locked out
    let (dt_pu, dt_t, dt_cost, dt_a) = (
        col(dist_threats, "pu", "dist_threats")?,
        col(dist_threats, "threat", "dist_threats")?,
        col(dist_threats, "action_cost", "dist_threats")?,
        col(dist_threats, "amount", "dist_threats")?,
    );
    if dt_a.is_empty() || dt_a.iter().any(|a| *a < 0.0) {
        return Err("dist_threats needs rows with amounts >= 0".into());
    }
    if dt_pu.iter().any(|p| !unit_ids.contains(p)) {
        return Err("dist_threats refers to planning units that do not exist".into());
    }
    if dt_t.iter().any(|t| !t_id.contains(t)) {
        return Err("dist_threats refers to threats that do not exist".into());
    }
    let mut dt_status = opt_col(dist_threats, "status", 0.0);
    if dt_status.iter().any(|s| ![0.0, 2.0, 3.0].contains(s)) {
        return Err("dist_threats status must be 0, 2 or 3".into());
    }
    for (k, &uid) in unit_ids.iter().enumerate() {
        if unit_status[k] != 3.0 {
            continue;
        }
        let rows: Vec<usize> = (0..dt_pu.len()).filter(|&r| dt_pu[r] == uid).collect();
        if rows.iter().any(|&r| dt_status[r] == 2.0) {
            warnings.push(format!(
                "The pu {} was set as locked out so it is not possible to set actions on it (lock in). Therefore, \
                 all actions in that unit will be set as locked out",
                crate::value::class_label(uid)
            ));
        }
        for r in rows {
            dt_status[r] = 3.0;
        }
    }
    let mut dt_rows: Vec<usize> = (0..dt_a.len()).filter(|&k| dt_a[k] != 0.0).collect();
    let mut seen = std::collections::HashSet::new();
    for &k in &dt_rows {
        if !seen.insert((dt_pu[k].to_bits(), dt_t[k].to_bits())) {
            return Err("There are duplicate values of the pu and threat pair in dist_threats".into());
        }
    }
    // sensitivity: given, or every feature to every threat (features fastest); delta2 by default the threat's
    // highest amount
    let (mut s_f, mut s_t, mut d1, d2g, mut d3, mut d4) = match sensitivity {
        Some(s) => (
            col(s, "feature", "sensitivity")?,
            col(s, "threat", "sensitivity")?,
            opt_col(s, "delta1", 0.0),
            opt_col(s, "delta2", f64::NAN),
            opt_col(s, "delta3", 0.0),
            opt_col(s, "delta4", 1.0),
        ),
        None => {
            let n = f_id.len() * t_id.len();
            let f: Vec<f64> = t_id.iter().flat_map(|_| f_id.iter().copied()).collect();
            let t: Vec<f64> = t_id.iter().flat_map(|&t| std::iter::repeat_n(t, f_id.len())).collect();
            (f, t, vec![0.0; n], vec![f64::NAN; n], vec![0.0; n], vec![1.0; n])
        }
    };
    let fill = |v: &mut Vec<f64>, x: f64| v.iter_mut().filter(|d| d.is_nan()).for_each(|d| *d = x);
    fill(&mut d1, 0.0);
    fill(&mut d3, 0.0);
    fill(&mut d4, 1.0);
    let top = |t: f64| dt_rows.iter().filter(|&&k| dt_t[k] == t).map(|&k| dt_a[k]).fold(f64::NAN, f64::max);
    let mut d2: Vec<f64> = d2g.iter().zip(&s_t).map(|(&d, &t)| if d.is_nan() { top(t) } else { d }).collect();
    if d1.iter().zip(&d2).any(|(a, b)| !b.is_nan() && b <= a) {
        return Err("Every value of delta1 parameter must be less than every value of delta2 parameter".into());
    }
    if d3.iter().zip(&d4).any(|(a, b)| b <= a) {
        return Err("Every value of delta3 parameter must be less than every value of delta4 parameter".into());
    }
    // boundary
    let mut bnd: Option<(Vec<f64>, Vec<f64>, Vec<f64>)> = match boundary {
        Some(b) => Some((
            col(b, "id1", "boundary")?,
            col(b, "id2", "boundary")?,
            col(b, "boundary", "boundary")?.into_iter().map(round3).collect(),
        )),
        None => None,
    };
    // what has no data: dropped
    let empty: Vec<f64> = unit_ids.iter().copied().filter(|u| !df_rows.iter().any(|&k| df_pu[k] == *u)).collect();
    if !empty.is_empty() {
        warnings.push(format!("The following pu's do not contain features: {}", ids_text(&empty)));
    }
    let keep =
        |v: &[f64], mask: &[bool]| v.iter().zip(mask).filter(|(_, m)| **m).map(|(x, _)| *x).collect::<Vec<f64>>();
    let missing_f: Vec<f64> = f_id.iter().copied().filter(|f| !df_rows.iter().any(|&k| df_f[k] == *f)).collect();
    if !missing_f.is_empty() {
        warnings.push(format!(
            "The following features are not represented (it'll not be considered in the model): {}",
            ids_text(&missing_f)
        ));
        let m: Vec<bool> = f_id.iter().map(|f| !missing_f.contains(f)).collect();
        (f_rec, f_cons) = (keep(&f_rec, &m), keep(&f_cons, &m));
        f_id = keep(&f_id, &m);
        let ms: Vec<bool> = s_f.iter().map(|f| !missing_f.contains(f)).collect();
        (s_t, d1, d2, d3, d4) = (keep(&s_t, &ms), keep(&d1, &ms), keep(&d2, &ms), keep(&d3, &ms), keep(&d4, &ms));
        s_f = keep(&s_f, &ms);
    }
    let not_threatened: Vec<f64> = f_id.iter().copied().filter(|f| !s_f.contains(f)).collect();
    if !not_threatened.is_empty() {
        warnings.push(format!("The following features are not threatened: {}", ids_text(&not_threatened)));
    }
    let missing_t: Vec<f64> = t_id.iter().copied().filter(|t| !dt_rows.iter().any(|&k| dt_t[k] == *t)).collect();
    if !missing_t.is_empty() {
        warnings.push(format!(
            "The following threats are not represented (it'll not be considered in the model): {}",
            ids_text(&missing_t)
        ));
        let m: Vec<bool> = t_id.iter().map(|t| !missing_t.contains(t)).collect();
        t_blm = keep(&t_blm, &m);
        t_id = keep(&t_id, &m);
        let ms: Vec<bool> = s_t.iter().map(|t| !missing_t.contains(t)).collect();
        (s_f, d1, d2, d3, d4) = (keep(&s_f, &ms), keep(&d1, &ms), keep(&d2, &ms), keep(&d3, &ms), keep(&d4, &ms));
        s_t = keep(&s_t, &ms);
    }
    let harmless: Vec<f64> = t_id.iter().copied().filter(|t| !s_t.contains(t)).collect();
    if !harmless.is_empty() {
        warnings.push(format!(
            "The following threats are not dangerous to any features (it'll not be considered in the model): {}",
            ids_text(&harmless)
        ));
        let m: Vec<bool> = t_id.iter().map(|t| !harmless.contains(t)).collect();
        t_blm = keep(&t_blm, &m);
        t_id = keep(&t_id, &m);
        dt_rows.retain(|&k| !harmless.contains(&dt_t[k]));
    }
    if let Some((a, b, v)) = &mut bnd {
        let m: Vec<bool> = a.iter().zip(b.iter()).map(|(x, y)| unit_ids.contains(x) && unit_ids.contains(y)).collect();
        if m.iter().any(|k| !k) {
            warnings.push(
                "The boundary data contain pu which ids does not exist in pu data (it'll not be considered in the model)"
                    .into(),
            );
            (*a, *b, *v) = (keep(a, &m), keep(b, &m), keep(v, &m));
        }
    }
    // internal ids
    let index =
        |ids: &[f64]| ids.iter().enumerate().map(|(k, v)| (v.to_bits(), k as i64)).collect::<HashMap<u64, i64>>();
    let (pu_ix, f_ix, t_ix) = (index(&unit_ids), index(&f_id), index(&t_id));
    let sens: Vec<usize> = (0..s_f.len())
        .filter(|&k| f_ix.contains_key(&s_f[k].to_bits()) && t_ix.contains_key(&s_t[k].to_bits()))
        .collect();
    let pick = |v: &[f64], rows: &[usize]| rows.iter().map(|&k| v[k]).collect::<Vec<f64>>();
    let mut sens_delta = Vec::with_capacity(4 * sens.len());
    for d in [&d1, &d2, &d3, &d4] {
        sens_delta.extend(sens.iter().map(|&k| round3(d[k])));
    }
    let input = ffi::ActionsInput {
        n_units: unit_ids.len() as i32,
        n_features: f_id.len() as i32,
        n_threats: t_id.len() as i32,
        monitoring_cost: monitoring,
        unit_status,
        target_recovery: f_rec.iter().map(|v| round3(*v)).collect(),
        target_conservation: f_cons.iter().map(|v| round3(*v)).collect(),
        df_unit: df_rows.iter().map(|&k| pu_ix[&df_pu[k].to_bits()]).collect(),
        df_feature: df_rows.iter().map(|&k| f_ix[&df_f[k].to_bits()]).collect(),
        df_amount: df_rows.iter().map(|&k| round3(df_a[k])).collect(),
        blm_actions: t_blm.iter().map(|v| round3(*v)).collect(),
        act_unit: dt_rows.iter().map(|&k| pu_ix[&dt_pu[k].to_bits()]).collect(),
        act_threat: dt_rows.iter().map(|&k| t_ix[&dt_t[k].to_bits()]).collect(),
        act_amount: dt_rows.iter().map(|&k| round3(dt_a[k])).collect(),
        act_cost: dt_rows.iter().map(|&k| round3(dt_cost[k])).collect(),
        act_status: pick(&dt_status, &dt_rows),
        sens_feature: sens.iter().map(|&k| f_ix[&s_f[k].to_bits()]).collect(),
        sens_threat: sens.iter().map(|&k| t_ix[&s_t[k].to_bits()]).collect(),
        sens_delta,
        has_boundary: bnd.is_some(),
        bnd_id1: bnd.as_ref().map(|b| b.0.iter().map(|x| pu_ix[&x.to_bits()]).collect()).unwrap_or_default(),
        bnd_id2: bnd.as_ref().map(|b| b.1.iter().map(|x| pu_ix[&x.to_bits()]).collect()).unwrap_or_default(),
        bnd_value: bnd.map(|b| b.2).unwrap_or_default(),
    };
    Ok(ActionsData {
        input,
        unit_ids,
        feature_ids: f_id,
        threat_ids: t_id,
        action_unit: pick(&dt_pu, &dt_rows),
        action_threat: pick(&dt_t, &dt_rows),
        warnings,
    })
}

/// prioriactions's problem(): its settings, the targets or the budget presolved; and its warnings.
pub fn actions_problem(
    d: &mut ActionsData,
    maximize: bool,
    budget: f64,
    blm: f64,
    curve: i32,
    segments: i32,
) -> std::result::Result<ffi::ActionsSettings, String> {
    let budget = round3(budget);
    let blm_actions = &d.input.blm_actions;
    if blm.abs() <= 1e-10 && d.input.has_boundary {
        d.warnings.push("The blm argument was set to 0, so the boundary data has no effect".into());
    }
    if blm.abs() > 1e-50 && !d.input.has_boundary {
        d.warnings.push("No boundary data supplied so the blm argument has no effect".into());
    }
    if blm_actions.iter().all(|b| *b <= 1e-10) && d.input.has_boundary {
        d.warnings
            .push("Some blm_actions argument were set to 0, so the boundary data has no effect for these cases".into());
    }
    if blm_actions.iter().any(|b| *b > 1e-10) && !d.input.has_boundary {
        d.warnings.push("No boundary data supplied so the blm_actions arguments has no effect".into());
    }
    if !(1..=3).contains(&curve) {
        return Err("invalid curve type (1, 2 or 3)".into());
    }
    let mut curve = curve;
    let mut segments = if curve == 1 { 1 } else { segments };
    if curve != 1 && d.input.act_amount.iter().any(|a| *a < 1.0) {
        curve = 1;
        segments = 1;
        d.warnings
            .push("Curve set to 1 because there are non-binary values in threat amount for a recovery target".into());
    }
    if !(1..=5).contains(&segments) {
        return Err("invalid number of segments for linearization (1 to 5)".into());
    }
    let mut budget = budget;
    if maximize {
        // the lock-in budget check: the locked units and actions' cost
        let mut units: Vec<f64> = d.input.unit_status.iter().map(|s| f64::from(u8::from(*s == 2.0))).collect();
        let actions: Vec<f64> = d.input.act_status.iter().map(|s| f64::from(u8::from(*s == 2.0))).collect();
        for (k, a) in actions.iter().enumerate() {
            if *a == 1.0 {
                units[d.input.act_unit[k] as usize] = 1.0;
            }
        }
        let c = ffi::actions_costs(&d.input, &units, &actions).map_err(|e| e.what().to_string())?;
        let need = c[1..].iter().sum::<f64>() + c[0];
        if budget < need {
            d.warnings.push(format!(
                "Infeasible model. There is not enough budget to achieve the actions required (lock-in). The budget \
                 will be set as the sum of the minimum costs to achieve the required actions: {need}"
            ));
            budget = need + 0.1;
        }
    } else {
        // targets above the most benefit: the most benefit less 0.1
        let b = ffi::actions_benefit(&d.input, &[]).map_err(|e| e.what().to_string())?;
        for (targets, top, what) in [
            (&mut d.input.target_recovery, &b.recovery, "recovery"),
            (&mut d.input.target_conservation, &b.conservation, "conservation"),
        ] {
            let top: Vec<f64> = top.iter().map(|v| r_round(*v, 4)).collect();
            let over: Vec<usize> = (0..targets.len()).filter(|&k| targets[k] > top[k]).collect();
            if !over.is_empty() {
                d.warnings.push(format!(
                    "Infeasible model. There is not enough representativeness to achieve the targets of {what} \
                     required of following features: {}. The targets of {what} for these features will be set to the \
                     maximum {what} benefit values",
                    ids_text(&over.iter().map(|&k| d.feature_ids[k]).collect::<Vec<_>>())
                ));
                for k in over {
                    targets[k] = top[k] - 0.1;
                }
            }
        }
    }
    Ok(ffi::ActionsSettings { maximize, blm, curve, segments, step: r_round(1.0 / f64::from(segments), 1), budget })
}

/// prioriactions's model (problem()'s matrices) bit for bit.
pub fn compile(d: &ActionsData, s: &ffi::ActionsSettings) -> std::result::Result<ffi::ActionsModel, String> {
    ffi::actions_compile(&d.input, s).map_err(|e| e.what().to_string())
}

/// A solution's figures: getCost() (monitoring, every threat's actions; 3 decimals), getSolutionBenefit() (every
/// feature's conservation, recovery and total; 4 decimals), getConnectivityPenalty() (the units', every threat's
/// actions'; 3 decimals), getActions() (every unit's actions by threat, conservation and connectivity).
pub struct Figures {
    pub cost: Table,
    pub benefit: Table,
    pub local: Table,
    pub connectivity: Option<Table>,
    pub summary: Table,
}

/// The figures of a solution: x the units, the actions, then (when solved) the pairs' benefits.
pub fn figures(d: &ActionsData, x: &[f64]) -> std::result::Result<Figures, String> {
    let (n, na) = (d.unit_ids.len(), d.action_unit.len());
    let units: Vec<f64> = x[..n].iter().map(|v| v.round()).collect();
    let actions: Vec<f64> = x[n..n + na].iter().map(|v| v.round()).collect();
    let threat_cols = |first: &str| {
        std::iter::once(first.to_string())
            .chain(d.threat_ids.iter().map(|t| format!("threat_{}", crate::value::class_label(*t))))
            .collect::<Vec<_>>()
    };
    let c = ffi::actions_costs(&d.input, &units, &actions).map_err(|e| e.what().to_string())?;
    let mut cost = Table::new(threat_cols("monitoring"));
    cost.rows.push(c.iter().map(|v| round3(*v)).collect());
    let sol: Vec<f64> = units.iter().chain(&actions).copied().collect();
    let b = ffi::actions_benefit(&d.input, &sol).map_err(|e| e.what().to_string())?;
    let mut benefit =
        Table::new(["feature", "benefit.conservation", "benefit.recovery", "benefit.total"].map(String::from).to_vec());
    benefit.rows = (0..d.feature_ids.len())
        .map(|f| {
            vec![
                d.feature_ids[f],
                r_round(b.conservation[f], 4),
                r_round(b.recovery[f], 4),
                r_round(b.recovery[f] + b.conservation[f], 4),
            ]
        })
        .collect();
    // the local benefit: every (unit, feature) pair's, by unit (stable)
    let p = ffi::actions_pairs(&d.input).map_err(|e| e.what().to_string())?;
    let local_x = if x.len() >= n + na + p.feature.len() { &x[n + na..n + na + p.feature.len()] } else { &[][..] };
    let mut rows: Vec<Vec<f64>> = (0..p.feature.len())
        .map(|k| {
            let v = local_x.get(k).copied().unwrap_or(f64::NAN);
            let (cons, rec) = if p.threatened[k] != 0 { (0.0, v) } else { (v, 0.0) };
            vec![d.unit_ids[p.unit[k] as usize], d.feature_ids[p.feature[k] as usize], cons, rec, rec + cons]
        })
        .collect();
    rows.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let mut local = Table::new(
        ["pu", "feature", "benefit.conservation", "benefit.recovery", "benefit.total"].map(String::from).to_vec(),
    );
    local.rows = rows;
    let connectivity = if d.input.has_boundary && !d.input.bnd_id1.is_empty() {
        let c = ffi::actions_connectivity(&d.input, &units, &actions).map_err(|e| e.what().to_string())?;
        let mut t = Table::new(threat_cols("units"));
        t.rows.push(c.iter().map(|v| round3(*v)).collect());
        Some(t)
    } else {
        None
    };
    // getActions(): every unit's actions (by threat), conservation (a feature kept without actions) and
    // connectivity (monitored for neither)
    let cons_units: Vec<f64> = local.rows.iter().filter(|r| r[2] > 0.0).map(|r| r[0]).collect();
    let mut columns = vec!["pu".to_string()];
    let mut threats: Vec<f64> = d.threat_ids.clone();
    threats.sort_by(f64::total_cmp);
    columns.extend(threats.iter().map(|t| crate::value::class_label(*t)));
    columns.extend(["conservation", "connectivity"].map(String::from));
    let mut summary = Table::new(columns);
    for (i, &uid) in d.unit_ids.iter().enumerate() {
        let mut row = vec![uid];
        let mut acted = 0.0;
        for t in &threats {
            let v = (0..na).find(|&k| d.action_unit[k] == uid && d.action_threat[k] == *t).map_or(0.0, |k| actions[k]);
            if v > 0.0 {
                acted = 1.0;
            }
            row.push(r_round(v, 1));
        }
        let conservation = f64::from(u8::from(cons_units.contains(&uid)));
        let both = f64::from(u8::from(conservation + acted >= 1.0));
        row.push(r_round(conservation, 1));
        row.push(r_round(units[i] - both, 1));
        summary.rows.push(row);
    }
    Ok(Figures { cost, benefit, local, connectivity, summary })
}

/// getPotentialBenefit(): every feature's distribution and the most benefit the actions can give it.
pub fn potential(d: &ActionsData) -> std::result::Result<Table, String> {
    let b = ffi::actions_benefit(&d.input, &[]).map_err(|e| e.what().to_string())?;
    let mut t = Table::new(
        [
            "feature",
            "dist",
            "dist_threatened",
            "maximum.conservation.benefit",
            "maximum.recovery.benefit",
            "maximum.benefit",
        ]
        .map(String::from)
        .to_vec(),
    );
    t.rows = (0..d.feature_ids.len())
        .map(|f| {
            vec![
                d.feature_ids[f],
                b.dist[f],
                b.dist_threatened[f],
                r_round(b.conservation[f], 4),
                r_round(b.recovery[f], 4),
                r_round(b.recovery[f] + b.conservation[f], 4),
            ]
        })
        .collect();
    Ok(t)
}

/// The data of a node's inputs.
pub fn data_of(args: &Args) -> Result<ActionsData> {
    let node = args.node;
    let table = |port: &str| -> Option<&Table> {
        match args.get(port) {
            Some(Value::Table(t)) => Some(t),
            _ => None,
        }
    };
    let need = |port: &str| table(port).ok_or_else(|| Error::msg(format!("node {node:?}: the table {port} is needed")));
    actions_data(
        need("pu")?,
        need("features")?,
        need("dist_features")?,
        need("threats")?,
        need("dist_threats")?,
        table("sensitivity"),
        table("boundary"),
    )
    .map_err(|e| Error::msg(format!("node {node:?}: {e}")))
}

/// The settings of a node: model, budget, blm, curve, segments.
pub fn settings_of(args: &Args, d: &mut ActionsData) -> Result<ffi::ActionsSettings> {
    let node = args.node;
    let model: String = args.param("model", "minimizeCosts".to_string())?;
    let maximize = match model.as_str() {
        "minimizeCosts" | "min_costs" => false,
        "maximizeBenefits" | "max_benefits" => true,
        m => return Err(Error::msg(format!("node {node:?}: unknown model {m:?} (minimizeCosts or maximizeBenefits)"))),
    };
    actions_problem(
        d,
        maximize,
        args.param("budget", 0.0)?,
        args.param("blm", 0.0)?,
        args.param("curve", 1)?,
        args.param("segments", 3)?,
    )
    .map_err(|e| Error::msg(format!("node {node:?}: {e}")))
}

struct Actions;

impl Operator for Actions {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.actions",
            "Multi-action planning as prioriactions 0.5.0 (Salgado-Rojas et al.): which planning units to monitor \
             and which actions against which threats to take in them, so that the features recover (or are kept) \
             to their targets at the least cost (minimizeCosts), or recover the most within a budget \
             (maximizeBenefits); the boundary of the units and of every threat's actions weighed; linear, \
             quadratic or cubic responses. prioriactions's model bit for bit (its inputData() checks and \
             defaults), solved by landschaft's closure engine and HiGHS.",
            Access::Global,
        )
        .input(PortSpec::new("pu", Type::Table, "planning units: id, monitoring_cost, status (0, 2 locked in, 3 out)"))
        .input(PortSpec::new("features", Type::Table, "id, target_recovery, target_conservation (0)"))
        .input(PortSpec::new("dist_features", Type::Table, "the features' amounts: pu, feature, amount"))
        .input(PortSpec::new("threats", Type::Table, "id, blm_actions (the weight of its actions' boundary; 0)"))
        .input(PortSpec::new(
            "dist_threats",
            Type::Table,
            "the threats in the units and the actions against them: pu, threat, amount, action_cost, status",
        ))
        .input(PortSpec::optional(
            "sensitivity",
            Type::Table,
            "which features each threat harms: feature, threat, delta1-delta4 (default every feature to every threat)",
        ))
        .input(PortSpec::optional("boundary", Type::Table, "id1, id2, boundary (shared lengths)"))
        .output(PortSpec::new("monitoring", Type::Table, "id and solution of every planning unit"))
        .output(PortSpec::new("actions", Type::Table, "pu, threat and solution of every action"))
        .output(PortSpec::new(
            "summary",
            Type::Table,
            "per unit: its actions by threat, conservation (a feature kept without actions), connectivity \
             (monitored for neither): getActions()",
        ))
        .output(PortSpec::new("result", Type::Table, "the objective, the solver's gap and status (1 optimal)"))
        .output(PortSpec::new("cost", Type::Table, "the monitoring cost and every threat's actions' cost"))
        .output(PortSpec::new("benefit", Type::Table, "every feature's benefit: conservation, recovery, total"))
        .output(PortSpec::new("local", Type::Table, "every (unit, feature)'s benefit"))
        .output(PortSpec::new("potential", Type::Table, "every feature's distribution and the most benefit"))
        .output(PortSpec::optional(
            "connectivity",
            Type::Table,
            "with boundary data: the boundary of the monitored units and of every threat's actions",
        ))
        .param(
            PortSpec::optional("model", Type::Text, "minimizeCosts (default) or maximizeBenefits")
                .choices(["minimizeCosts", "maximizeBenefits"]),
        )
        .param(PortSpec::optional("budget", Type::Number, "maximizeBenefits: the most the units and actions may cost"))
        .param(PortSpec::optional("blm", Type::Number, "the weight of the monitored units' boundary (0)"))
        .param(
            PortSpec::optional("curve", Type::Integer, "the features' response: 1 linear (default), 2, 3")
                .choices([1, 2, 3]),
        )
        .param(PortSpec::optional("segments", Type::Integer, "curve 2, 3: pieces of its linear approximation (3)"))
        .param(PortSpec::optional("gap", Type::Number, "the relative optimality gap (default 0)"))
        .param(PortSpec::optional("time_limit", Type::Number, "seconds (default none)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let mut d = data_of(args)?;
        let s = settings_of(args, &mut d)?;
        for w in d.warnings.drain(..) {
            ctx.warn(w);
        }
        let gap: f64 = args.param("gap", 0.0)?;
        let time_limit: f64 = args.opt_param("time_limit")?.unwrap_or(f64::INFINITY);
        let r = ffi::actions_solve(&d.input, &s, gap, time_limit, &[]).map_err(|e| err(e.what().to_string()))?;
        if !r.has_solution {
            return Err(err(format!("no solution: {}", r.status)));
        }
        let f = figures(&d, &r.x).map_err(err)?;
        let (n, na) = (d.unit_ids.len(), d.action_unit.len());
        let mut monitoring = Table::new(vec!["id".into(), "solution".into()]);
        monitoring.rows = (0..n).map(|i| vec![d.unit_ids[i], r.x[i].round()]).collect();
        let mut actions = Table::new(vec!["pu".into(), "threat".into(), "solution".into()]);
        actions.rows = (0..na).map(|k| vec![d.action_unit[k], d.action_threat[k], r.x[n + k].round()]).collect();
        let mut result = Table::new(vec!["objective".into(), "gap".into(), "optimal".into()]);
        result.rows.push(vec![r.objective, r.gap, f64::from(u8::from(r.status == "optimal"))]);
        let mut out = outputs([
            ("monitoring", Value::Table(monitoring)),
            ("actions", Value::Table(actions)),
            ("summary", Value::Table(f.summary)),
            ("result", Value::Table(result)),
            ("cost", Value::Table(f.cost)),
            ("benefit", Value::Table(f.benefit)),
            ("local", Value::Table(f.local)),
            ("potential", Value::Table(potential(&d).map_err(err)?)),
        ]);
        if let Some(c) = f.connectivity {
            out.insert("connectivity".into(), Value::Table(c));
        }
        Ok(out)
    }
}
