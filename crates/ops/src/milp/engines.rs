//! landschaft's engines for the models with pair terms, in front of HiGHS as landschaft's `_highs_or_cut`
//! (prioritize/_milp_solve.py) puts them: a minimum set with boundary penalties or symmetric connectivity rewards
//! on the cut engine (a Lagrangian bound by minimum cuts, repaired selections), a budgeted minimisation with pair
//! penalties on the closure engine, then HiGHS's MIP from their solution when the gap is not reached and the
//! model is small enough; so a model solves here as fast as in landschaft, to the same plan.

use terraflux_ls_sys::ffi;

use super::{Model, current_watch};

/// Models up to this many columns go on to HiGHS's MIP when the cut engine stops short of the gap.
const CUT_HIGHS_COLUMNS: usize = 50_000;

/// The pair columns of a model in landschaft's form: every column past the units a pair `b = x_i x_j`,
/// linearised by prioritizr's rows `b <= x_i`, `b <= x_j` (and `b >= x_i + x_j - 1`); the rows on the units alone
/// apart. `pi`, `pj`: the pairs' units (the smaller first), `q`: their objective.
struct Pairs {
    pi: Vec<usize>,
    pj: Vec<usize>,
    q: Vec<f64>,
    /// The rows on the units alone.
    side: Vec<usize>,
}

/// `m`'s pair columns and side rows (landschaft's marxan_model and closure_model, their common part), or None.
fn pairs(m: &Model, n: usize) -> Option<Pairs> {
    if m.maximize || !m.vtype[..n].iter().all(|&t| t == 'B') {
        return None;
    }
    let a = &m.a;
    let ncol = m.obj.len();
    let mut side = Vec::new();
    // the units of every pair column's `b <= x` rows
    let mut units: Vec<Vec<usize>> = vec![Vec::new(); ncol - n];
    for r in 0..a.nrow {
        let (cols, vals) = a.row(r);
        if !cols.iter().any(|&c| c >= n) {
            side.push(r);
            continue;
        }
        let le = m.sense[r] == "<=" && cols.len() == 2 && m.rhs[r] == 0.0;
        let ge = m.sense[r] == ">=" && cols.len() == 3 && m.rhs[r] == -1.0;
        if !(le || ge) {
            return None;
        }
        if !cols.iter().zip(vals).all(|(&c, &v)| if c >= n { v == 1.0 } else { v == -1.0 }) {
            return None;
        }
        if le {
            // (columns in order: the unit, then the pair)
            let (unit, pcol) = (cols[0], cols[1]);
            if unit >= n || pcol < n {
                return None;
            }
            units[pcol - n].push(unit);
        }
    }
    let (mut pi, mut pj) = (Vec::with_capacity(ncol - n), Vec::with_capacity(ncol - n));
    for u in &mut units {
        u.sort_unstable();
        if u.len() != 2 || u[0] == u[1] {
            return None;
        }
        pi.push(u[0]);
        pj.push(u[1]);
    }
    let q = m.obj[n..].to_vec();
    if (n..ncol).any(|j| !m.lb[j].is_finite() || m.lb[j] > 0.0 || m.ub[j] < 1.0) {
        return None;
    }
    Some(Pairs { pi, pj, q, side })
}

/// Rows of the units as columns: every unit's rows (in order) and amounts.
fn by_unit(m: &Model, n: usize, rows: &[usize]) -> (Vec<i64>, Vec<i32>, Vec<f64>) {
    let mut per: Vec<Vec<(i32, f64)>> = vec![Vec::new(); n];
    for (k, &r) in rows.iter().enumerate() {
        let (cols, vals) = m.a.row(r);
        for (&c, &v) in cols.iter().zip(vals) {
            if c < n && v != 0.0 {
                per[c].push((k as i32, v));
            }
        }
    }
    let mut start = vec![0i64];
    let (mut index, mut value) = (Vec::new(), Vec::new());
    for p in per {
        for (k, v) in p {
            index.push(k);
            value.push(v);
        }
        start.push(index.len() as i64);
    }
    (start, index, value)
}

/// A minimum set in Marxan's form (BLM 1) for the cut engine: the units' costs (with half of every pair's
/// objective), their status (2 locked in, 3 locked out), the targets by unit and the boundaries `-q / 2`; None
/// when the cut engine does not take the model (landschaft's cut_applies).
struct Marxan {
    cost: Vec<f64>,
    status: Vec<i32>,
    offset: Vec<i64>,
    spindex: Vec<i32>,
    amount: Vec<f64>,
    target: Vec<f64>,
    pairs: Pairs,
    bound: Vec<f64>,
}

fn cut_applies(m: &Model, n: usize) -> Option<Marxan> {
    if n == 0 || m.obj.len() <= n || m.maximize {
        return None;
    }
    let p = pairs(m, n)?;
    // the targets: the rows on the units alone, all >=, amounts >= 0; those any selection meets left out
    let mut rows = Vec::new();
    for &r in &p.side {
        if m.sense[r] != ">=" {
            return None;
        }
        if m.a.row(r).1.iter().zip(m.a.row(r).0).any(|(&v, &c)| c < n && v < 0.0) {
            return None;
        }
        if m.rhs[r] > 0.0 {
            rows.push(r);
        }
    }
    let bound: Vec<f64> = p.q.iter().map(|q| -q / 2.0).collect();
    if bound.iter().any(|&b| b < 0.0) {
        return None;
    }
    let mut cost = m.obj[..n].to_vec();
    for (k, q) in p.q.iter().enumerate() {
        cost[p.pi[k]] += q / 2.0;
        cost[p.pj[k]] += q / 2.0;
    }
    // (a unit locked in and out: not the engine's)
    let status = m.lb[..n]
        .iter()
        .zip(&m.ub[..n])
        .map(|(&lb, &ub)| match (lb >= 0.5, ub <= 0.5) {
            (true, true) => None,
            (true, false) => Some(2),
            (false, true) => Some(3),
            (false, false) => Some(0),
        })
        .collect::<Option<Vec<i32>>>()?;
    let (offset, spindex, amount) = by_unit(m, n, &rows);
    let target = rows.iter().map(|&r| m.rhs[r]).collect();
    Some(Marxan { cost, status, offset, spindex, amount, target, pairs: p, bound })
}

/// Every unit's neighbours and boundaries, both directions, a pair given twice summed (landschaft's _neighbours).
fn neighbours(n: usize, p: &Pairs, bound: &[f64]) -> (Vec<i64>, Vec<i32>, Vec<f64>) {
    let mut per: Vec<Vec<(i32, f64)>> = vec![Vec::new(); n];
    for k in 0..p.pi.len() {
        per[p.pi[k]].push((p.pj[k] as i32, bound[k]));
        per[p.pj[k]].push((p.pi[k] as i32, bound[k]));
    }
    let mut start = vec![0i64];
    let (mut nb, mut cost) = (Vec::new(), Vec::new());
    for mut v in per {
        v.sort_by_key(|&(j, _)| j);
        let mut k = 0;
        while k < v.len() {
            let (j, mut c) = v[k];
            k += 1;
            while k < v.len() && v[k].0 == j {
                c += v[k].1;
                k += 1;
            }
            nb.push(j);
            cost.push(c);
        }
        start.push(nb.len() as i64);
    }
    (start, nb, cost)
}

/// The full solution of a selection of units: the pairs' columns `x_i x_j`.
fn pair_values(m: &Model, n: usize, x: &[f64], p: &Pairs) -> Vec<f64> {
    let mut full = vec![0.0; m.obj.len()];
    full[..n].copy_from_slice(&x[..n]);
    for k in 0..p.pi.len() {
        full[n + k] = x[p.pi[k]] * x[p.pj[k]];
    }
    full
}

/// A watch for an engine: this thread's, else one that never stops it.
fn watch() -> terraflux_ls_sys::SolveWatch {
    terraflux_ls_sys::SolveWatch(current_watch().unwrap_or_else(|| Box::new(|_, _, _, _| true)))
}

/// An engine's result as a solve's: the plan's full solution, its objective, the gap to the bound, optimal when
/// within the gap asked.
fn result(m: &Model, n: usize, p: &Pairs, out: ffi::EngineOut, gap: f64) -> ffi::SolveOut {
    if out.status == "infeasible" || out.status == "no_solution" || out.status == "cancelled" {
        let status = if out.status == "no_solution" { "time_limit".to_string() } else { out.status };
        return ffi::SolveOut {
            status,
            has_solution: false,
            x: Vec::new(),
            objective: f64::NAN,
            bound: out.bound,
            gap: f64::NAN,
        };
    }
    let full = pair_values(m, n, &out.x, p);
    let objective: f64 = m.obj.iter().zip(&full).map(|(c, x)| c * x).sum();
    let g = objective - out.bound;
    let gp = if g <= 0.0 { 0.0 } else { g / objective.abs().max(1e-10) };
    let status = if gp <= gap { "optimal".to_string() } else { out.status };
    ffi::SolveOut { status, has_solution: true, x: full, objective, bound: out.bound, gap: gp }
}

fn solve_cut(
    m: &Model,
    n: usize,
    mm: &Marxan,
    gap: f64,
    time_limit: f64,
    start: &[f64],
) -> Result<ffi::SolveOut, String> {
    let (nb_start, nb, nb_cost) = neighbours(n, &mm.pairs, &mm.bound);
    let init: Vec<i32> =
        if start.is_empty() { Vec::new() } else { start[..n].iter().map(|&s| i32::from(s >= 0.5)).collect() };
    let out = ffi::opt_solve_cut(
        &mm.cost,
        &mm.status,
        &mm.offset,
        &mm.spindex,
        &mm.amount,
        &mm.target,
        &nb_start,
        &nb,
        &nb_cost,
        gap,
        time_limit,
        &init,
        ffi::max_threads(),
        &mut watch(),
    )
    .map_err(|e| e.what().to_string())?;
    Ok(result(m, n, &mm.pairs, out, gap))
}

/// A budgeted minimisation with pair penalties for the closure engine (landschaft's closure_model): binary
/// units, pair terms that are penalties (q <= 0: w = -q / 2 both ways, -w on both units' costs), rows on the
/// units alone with amounts >= 0, >= (targets) or <= (budgets).
fn solve_closure(
    m: &Model,
    n: usize,
    gap: f64,
    time_limit: f64,
    start: &[f64],
) -> Option<Result<ffi::SolveOut, String>> {
    let p = pairs(m, n)?;
    if p.q.iter().any(|&q| q > 0.0) {
        return None;
    }
    let mut rows = Vec::new();
    for &r in &p.side {
        let s = m.sense[r].as_str();
        if s != ">=" && s != "<=" {
            return None;
        }
        if m.a.row(r).1.iter().zip(m.a.row(r).0).any(|(&v, &c)| c < n && v < 0.0) {
            return None;
        }
        // (>= rows with rhs <= 0 are always met)
        if s == "<=" || m.rhs[r] > 0.0 {
            rows.push(r);
        }
    }
    let w: Vec<f64> = p.q.iter().map(|q| -q / 2.0).collect();
    let mut cost = m.obj[..n].to_vec();
    for (k, wk) in w.iter().enumerate() {
        cost[p.pi[k]] -= wk;
        cost[p.pj[k]] -= wk;
    }
    let fixed: Vec<i8> = (0..n)
        .map(|i| {
            if m.lb[i] >= 0.5 {
                1
            } else if m.ub[i] <= 0.5 {
                0
            } else {
                -1
            }
        })
        .collect();
    let to32 = |v: &[usize]| v.iter().map(|&i| i as i32).collect::<Vec<i32>>();
    let pen_i = [to32(&p.pi), to32(&p.pj)].concat();
    let pen_j = [to32(&p.pj), to32(&p.pi)].concat();
    let pen_w = [w.clone(), w].concat();
    let rhs: Vec<f64> = rows.iter().map(|&r| m.rhs[r]).collect();
    let geq: Vec<u8> = rows.iter().map(|&r| u8::from(m.sense[r] == ">=")).collect();
    let (col_start, col_row, col_val) = by_unit(m, n, &rows);
    let init: Vec<u8> =
        if start.is_empty() { Vec::new() } else { start[..n].iter().map(|&s| u8::from(s > 0.5)).collect() };
    let out = ffi::opt_solve_closure(
        &cost,
        &fixed,
        &pen_i,
        &pen_j,
        &pen_w,
        &rhs,
        &geq,
        &col_start,
        &col_row,
        &col_val,
        gap,
        time_limit,
        ffi::max_threads(),
        &init,
        &mut watch(),
    );
    Some(out.map_err(|e| e.what().to_string()).map(|o| result(m, n, &p, o, gap)))
}

/// A model with pair terms on the cut or closure engine, then HiGHS's MIP from their solution when the gap is
/// not reached and the model is small enough (landschaft's _highs_or_cut); None when neither engine takes it (the
/// model goes to HiGHS behind the covering and wide engines).
pub(super) fn highs_or_cut(
    m: &Model,
    n: usize,
    gap: f64,
    time_limit: f64,
    start: &[f64],
    highs: impl Fn(f64, &[f64]) -> Result<ffi::SolveOut, String>,
) -> Option<Result<ffi::SolveOut, String>> {
    let t0 = std::time::Instant::now();
    let left = || time_limit - t0.elapsed().as_secs_f64();
    let r = match cut_applies(m, n) {
        Some(mm) => solve_cut(m, n, &mm, gap, time_limit, start),
        None => {
            if m.obj.len() <= n {
                return None;
            }
            let r = solve_closure(m, n, gap, time_limit, start)?;
            match r {
                // no selection repaired: HiGHS alone
                Ok(r) if !r.has_solution && r.status != "infeasible" && r.status != "cancelled" => {
                    if left() <= 0.0 {
                        return Some(Ok(r));
                    }
                    return Some(highs(left(), start));
                }
                r => r,
            }
        }
    };
    let mut r = match r {
        Ok(r) => r,
        Err(e) => return Some(Err(e)),
    };
    if r.status == "cancelled" {
        return Some(Ok(r));
    }
    if r.status == "optimal" || r.status == "infeasible" || m.obj.len() > CUT_HIGHS_COLUMNS || !r.has_solution {
        if !["optimal", "infeasible", "time_limit"].contains(&r.status.as_str()) {
            r.status = "heuristic".into();
        }
        return Some(Ok(r));
    }
    if left() <= 0.0 {
        r.status = "time_limit".into();
        return Some(Ok(r));
    }
    let h = match highs(left(), &r.x) {
        Ok(h) => h,
        Err(e) => return Some(Err(e)),
    };
    // the better plan, the larger bound
    let bound = if h.bound.is_finite() { r.bound.max(h.bound) } else { r.bound };
    let mut out = h;
    if !out.has_solution || out.objective > r.objective {
        out.has_solution = r.has_solution;
        out.x = r.x;
        out.objective = r.objective;
    }
    out.bound = bound;
    let g = out.objective - out.bound;
    out.gap = if g <= 0.0 { 0.0 } else { g / out.objective.abs().max(1e-10) };
    if out.gap <= gap {
        out.status = "optimal".into();
    }
    Some(Ok(out))
}
