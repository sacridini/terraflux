//! landschaft's subset search (prioritize/_milp_subset.py and `_wide_then_subset` of _milp_solve.py) for the
//! budgeted objectives that reward meeting targets (maximum features, maximum phylogenetic diversity and
//! endemism): the wide engine first (its bound, a first solution) for a share of the time limit, then a local
//! search over the set of targets met. A set of targets is priced by a minimum set over their rows on the covering
//! engine; the search moves by additions and swaps that raise the value, the most valuable first, each accepted
//! when the cheapest selection meeting the set fits the budget.

use std::time::Instant;

use terraflux_ls_sys::ffi;

use super::{Csr, Model, STOPPED, current_watch, row_bounds};

const ORACLE_GAP: f64 = 0.005; // the covering engine's gap when it prices a set of targets
const ORACLE_SECONDS: f64 = 20.0; // and its time for one set at most
const MAX_SWAPS: usize = 20000; // swaps weighed per step
const SUBSET_ENGINE_SHARE: f64 = 0.4; // the wide engine's share of the time limit before the search

/// A solve of a model on HiGHS: the model, its units (0: none), the gap, the time limit and a start.
pub(super) type Run<'a> = dyn Fn(&Model, usize, f64, f64, &[f64]) -> Result<ffi::SolveOut, String> + 'a;

/// The structure of a target-rewarding budgeted model.
struct Subset {
    n: usize,
    /// The targets' columns (target met), in target order.
    flags: Vec<usize>,
    /// The units' coefficients in the budget row, and the budget.
    cost: Vec<f64>,
    budget: f64,
    /// The targets' rows over the units, the amount each needs, each target's own reward.
    amounts: Csr,
    target: Vec<f64>,
    reward: Vec<f64>,
    /// The columns rewarded when a target of theirs is met (a phylogeny's branches), their reward and, for each,
    /// the targets (positions) below it.
    branch_cols: Vec<usize>,
    branch_reward: Vec<f64>,
    branch_targets: Vec<Vec<usize>>,
}

/// The rows `rows` of `a`, the columns before `ncol` only.
fn pick_rows(a: &Csr, rows: &[usize], ncol: usize) -> Csr {
    let mut out = Csr { nrow: rows.len(), ncol, start: vec![0], index: Vec::new(), value: Vec::new() };
    for &r in rows {
        let (idx, val) = a.row(r);
        for (&c, &v) in idx.iter().zip(val) {
            if c < ncol {
                out.index.push(c);
                out.value.push(v);
            }
        }
        out.start.push(out.index.len());
    }
    out
}

/// The structure of a target-rewarding budgeted model, or None when the model is not one.
fn subset_model(m: &Model, n: usize) -> Option<Subset> {
    if n == 0 || !m.maximize || !m.col_ids.iter().any(|c| c == "spp_met") {
        return None;
    }
    let ncol = m.obj.len();
    let at = |ids: &[String], what: &str| (0..ids.len()).filter(|&k| ids[k] == what).collect::<Vec<usize>>();
    let flags = at(&m.col_ids, "spp_met");
    let trows = at(&m.row_ids, "spp_target");
    let brows = at(&m.row_ids, "budget");
    if flags.len() != trows.len()
        || brows.len() != 1
        || !trows.iter().all(|&r| m.sense[r] == ">=")
        || m.sense[brows[0]] != "<="
    {
        return None;
    }
    let mut flag_at: Vec<Option<usize>> = vec![None; ncol];
    for (k, &c) in flags.iter().enumerate() {
        flag_at[c] = Some(k);
    }
    let other: Vec<usize> = (n..ncol).filter(|&c| flag_at[c].is_none()).collect();
    let rows = at(&m.row_ids, "branch_target");
    if trows.len() + 1 + rows.len() != m.rhs.len() || other.iter().any(|&c| m.col_ids[c] != "branch_met") {
        return None;
    }
    if !flags.iter().chain(&other).all(|&c| m.vtype[c] == 'B') {
        return None;
    }
    let a = &m.a;
    // a target's row: its amounts in the units, minus the target times its flag (and nothing else)
    let mut target = vec![0.0; trows.len()];
    for (i, &r) in trows.iter().enumerate() {
        let (idx, val) = a.row(r);
        for (&c, &v) in idx.iter().zip(val) {
            match flag_at[c] {
                Some(k) if k == i => {
                    if v > 0.0 {
                        return None;
                    }
                    target[i] = -v;
                }
                Some(_) => {
                    if v.abs() > 1e-8 {
                        return None;
                    }
                }
                None if c >= n => return None,
                None => {}
            }
        }
    }
    let (bidx, bval) = a.row(brows[0]);
    if bidx.iter().any(|&c| c >= n) {
        return None;
    }
    let mut cost = vec![0.0; n];
    for (&c, &v) in bidx.iter().zip(bval) {
        cost[c] = v;
    }
    let mut branch_targets = Vec::new();
    if !other.is_empty() {
        if rows.len() != other.len() {
            return None;
        }
        // each branch row: the targets below the branch (+1) minus the branch (-1)
        for (e, &r) in rows.iter().enumerate() {
            if m.sense[r] != ">=" || m.rhs[r] != 0.0 {
                return None;
            }
            let mut below = Vec::new();
            let mut own = false;
            for (&c, &v) in a.row(r).0.iter().zip(a.row(r).1) {
                if c < n {
                    return None;
                }
                match flag_at[c] {
                    Some(k) if v == 1.0 => below.push(k),
                    Some(_) => return None,
                    None if c == other[e] && v == -1.0 => own = true,
                    None => return None,
                }
            }
            if !own {
                return None;
            }
            branch_targets.push(below);
        }
    }
    Some(Subset {
        n,
        amounts: pick_rows(a, &trows, n),
        target,
        reward: flags.iter().map(|&c| m.obj[c]).collect(),
        branch_reward: other.iter().map(|&c| m.obj[c]).collect(),
        flags,
        cost,
        budget: m.rhs[brows[0]],
        branch_cols: other,
        branch_targets,
    })
}

/// The reward of a set of targets met (a boolean per target).
fn value(sm: &Subset, met: &[bool]) -> f64 {
    let mut v = 0.0;
    for (r, &on) in sm.reward.iter().zip(met) {
        if on {
            v += r;
        }
    }
    let mut b = 0.0;
    for (e, below) in sm.branch_targets.iter().enumerate() {
        if sm.branch_reward[e] > 0.0 && below.iter().any(|&k| met[k]) {
            b += sm.branch_reward[e];
        }
    }
    v + b
}

fn dot(a: &Csr, i: usize, x: &[f64]) -> f64 {
    let (idx, val) = a.row(i);
    idx.iter().zip(val).fold(0.0, |s, (&c, &v)| s + v * x[c])
}

/// Every column from the units' values: the targets they meet, the branches those allow; the objective, or None
/// when a row is broken.
fn complete(m: &Model, sm: &Subset, xu: &[f64]) -> Option<(Vec<f64>, f64)> {
    let mut z = vec![0.0; m.obj.len()];
    z[..sm.n].copy_from_slice(xu);
    let met: Vec<bool> = (0..sm.flags.len())
        .map(|i| dot(&sm.amounts, i, xu) >= sm.target[i] - 1e-9 * sm.target[i].abs().max(1.0))
        .collect();
    for (i, &c) in sm.flags.iter().enumerate() {
        z[c] = f64::from(met[i]);
    }
    for (e, below) in sm.branch_targets.iter().enumerate() {
        z[sm.branch_cols[e]] = f64::from(below.iter().any(|&k| met[k]) && sm.branch_reward[e] > 0.0);
    }
    let (lo, hi) = row_bounds(m);
    for r in 0..m.a.nrow {
        let act = dot(&m.a, r, &z);
        let tol = 1e-9 * m.rhs[r].abs().max(1.0);
        if act < lo[r] - tol || act > hi[r] + tol {
            return None;
        }
    }
    let obj = m.obj.iter().zip(&z).fold(0.0, |s, (c, v)| s + c * v);
    Some((z, obj))
}

/// From a solution x, the local search; the best solution found and its objective (None if none better than x).
/// `deadline`: None until no move helps. The watch is asked between moves; `bound` and `t0` only tell its progress.
fn improve(
    m: &Model,
    sm: &Subset,
    x: &[f64],
    deadline: Option<Instant>,
    bound: f64,
    t0: Instant,
    run: &Run,
) -> Result<Option<(Vec<f64>, f64)>, String> {
    let n = sm.n;
    let (lb, ub) = (&m.lb[..n], &m.ub[..n]);
    let left = || deadline.map_or(f64::INFINITY, |d| d.saturating_duration_since(Instant::now()).as_secs_f64());
    let mut watch = current_watch();
    let room = sm.budget + 1e-9 * sm.budget.abs().max(1.0);

    // the cheapest selection meeting the targets s: its units' values, or None (over the budget)
    let price = |s: &[bool]| -> Result<Option<Vec<f64>>, String> {
        let idx: Vec<usize> = (0..s.len()).filter(|&k| s[k]).collect();
        let xu = if idx.is_empty() {
            lb.to_vec()
        } else {
            let cm = Model {
                a: pick_rows(&sm.amounts, &idx, n),
                obj: sm.cost.clone(),
                lb: lb.to_vec(),
                ub: ub.to_vec(),
                rhs: idx.iter().map(|&k| sm.target[k]).collect(),
                sense: vec![">=".to_string(); idx.len()],
                vtype: m.vtype[..n].to_vec(),
                maximize: false,
                row_ids: vec!["spp_target".to_string(); idx.len()],
                col_ids: vec!["pu".to_string(); n],
                connected: Vec::new(),
                zone_units: 0,
                engine: m.engine,
            };
            let mut r = run(&cm, 0, ORACLE_GAP, ORACLE_SECONDS.min(left()).max(0.0), &[])?;
            if !r.has_solution {
                return Ok(None);
            }
            // over the budget within the gap, but the bound fits: the cheapest selection may fit, priced exactly
            if r.objective > room && r.bound <= room {
                let r2 = run(&cm, 0, 0.0, ORACLE_SECONDS.min(left()).max(0.0), &r.x)?;
                if r2.has_solution && r2.objective < r.objective {
                    r = r2;
                }
            }
            r.x.iter()
                .zip(&m.vtype)
                .map(|(&v, &t)| if t == 'B' || t == 'I' { v.round_ties_even() } else { v })
                .collect()
        };
        let spent = sm.cost.iter().zip(&xu).fold(0.0, |s, (c, v)| s + c * v);
        Ok((spent <= room).then_some(xu))
    };

    let mut best_z = x.to_vec();
    let mut best = m.obj.iter().zip(&best_z).fold(0.0, |s, (c, v)| s + c * v);
    let mut met: Vec<bool> = sm.flags.iter().map(|&c| best_z[c] > 0.5).collect();
    let mut cur = value(sm, &met);
    let mut improved_any = false;
    let mut moved = true;
    while moved && left() > 0.0 {
        moved = false;
        let inside: Vec<usize> = (0..met.len()).filter(|&k| met[k]).collect();
        let mut outside: Vec<usize> = (0..met.len()).filter(|&k| !met[k]).collect();
        // (value, kind: 0 an addition, 1 a swap, the set)
        let mut moves: Vec<(f64, u8, Vec<bool>)> = Vec::new();
        for &f in &outside {
            let mut s = met.clone();
            s[f] = true;
            moves.push((value(sm, &s), 0, s));
        }
        if inside.len() * outside.len() > MAX_SWAPS {
            // many targets: swaps in of the best additions only
            let keep = (MAX_SWAPS / inside.len().max(1)).max(1);
            let mut order: Vec<usize> = (0..moves.len()).collect();
            order.sort_by(|&a, &b| moves[b].0.partial_cmp(&moves[a].0).unwrap_or(std::cmp::Ordering::Equal));
            outside = order.into_iter().take(keep).map(|k| outside[k]).collect();
        }
        for &r in &inside {
            for &f in &outside {
                let mut s = met.clone();
                s[r] = false;
                s[f] = true;
                moves.push((value(sm, &s), 1, s));
            }
        }
        // the most valuable first, additions before swaps of the same value
        let floor = cur + 1e-9 * cur.abs().max(1.0);
        moves.retain(|mv| mv.0 > floor);
        moves.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
        for (_, _, s) in moves {
            if left() <= 0.0 {
                break;
            }
            if let Some(w) = watch.as_mut()
                && !w(f64::NAN, f64::NAN, f64::NAN, t0.elapsed().as_secs_f64())
            {
                return Err(STOPPED.into());
            }
            let Some(xu) = price(&s)? else { continue };
            let Some((z, obj)) = complete(m, sm, &xu) else { continue };
            if obj <= best + 1e-12 * best.abs().max(1.0) {
                continue;
            }
            best_z = z;
            best = obj;
            met = sm.flags.iter().map(|&c| best_z[c] > 0.5).collect();
            cur = value(sm, &met);
            improved_any = true;
            moved = true;
            if let Some(w) = watch.as_mut() {
                let gap = ((bound - best) / best.abs().max(1e-10)).max(0.0);
                if !w(best, bound, gap, t0.elapsed().as_secs_f64()) {
                    return Err(STOPPED.into());
                }
            }
            break;
        }
    }
    Ok(improved_any.then_some((best_z, best)))
}

/// Budgeted objectives that reward the targets met: the wide engine (its bound, a first solution), then a local
/// search over the set of targets met for the rest of the time (landschaft's `_wide_then_subset`). None when the
/// model is not of that form (a maximisation of target rewards under one budget).
pub(super) fn wide_then_subset(
    m: &Model,
    n: usize,
    gap: f64,
    time_limit: f64,
    start: &[f64],
    run: &Run,
) -> Option<Result<ffi::SolveOut, String>> {
    let sm = subset_model(m, n)?;
    let t0 = Instant::now();
    let share = if time_limit.is_finite() { time_limit * SUBSET_ENGINE_SHARE } else { time_limit };
    let mut r = match run(m, n, gap, share, start) {
        Ok(r) => r,
        Err(e) => return Some(Err(e)),
    };
    if r.status == "optimal" || r.status == "infeasible" || !r.has_solution {
        return Some(Ok(r));
    }
    let deadline = time_limit.is_finite().then(|| t0 + std::time::Duration::from_secs_f64(time_limit.max(0.0)));
    match improve(m, &sm, &r.x, deadline, r.bound, t0, run) {
        Ok(Some((x, objective))) => {
            r.x = x;
            r.objective = objective;
            let g = r.bound - r.objective; // a maximisation
            r.gap = if g <= 0.0 { 0.0 } else { g / r.objective.abs().max(1e-10) };
            if r.gap <= gap {
                r.status = "optimal".into();
            }
        }
        Ok(None) => {}
        Err(e) => return Some(Err(e)),
    }
    Some(Ok(r))
}

/// landschaft's `improve_subset` from a selection of the units (`units`: their values), until no move helps: the
/// best solution found and its objective, None if none is better than the selection's own; an error when the model
/// is not of the subset search's form.
pub(super) fn improve_from(m: &Model, n: usize, units: &[f64], run: &Run) -> Result<Option<(Vec<f64>, f64)>, String> {
    let sm = subset_model(m, n).ok_or("the model is not a maximisation of the targets met under a budget")?;
    let (z, _) = complete(m, &sm, units).ok_or("the selection breaks a row")?;
    improve(m, &sm, &z, None, f64::NAN, Instant::now(), run)
}
