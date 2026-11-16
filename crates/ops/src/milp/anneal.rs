//! The library's anneal solver (prioritize/_milp_anneal.py): Marxan's simulated annealing on a compiled minimum-set
//! model. The model is read as Marxan's problem, exactly: the units' objective coefficients, every pair column
//! `b = x_i x_j` with objective `q` a boundary of `-q / 2` between the two units and `q / 2` on each unit's cost
//! (`q x_i x_j = q / 2 (x_i + x_j) - q / 2 (x_i + x_j - 2 x_i x_j)`), rows `A x >= rhs` on the units Marxan's
//! features, the columns' bounds its locked units. Every run is repaired to meet every target and trimmed; the
//! solution is the run with the lowest objective, its bound the linear relaxation without the boundaries' cut terms.

use std::time::Instant;

use terraflux_ls_sys::ffi;

use super::{Csr, Model, engines, highs};

/// The anneal solver's settings (the library's add_anneal_solver).
#[derive(Debug, Clone, PartialEq)]
pub struct Anneal {
    pub num_reps: i64,
    /// Annealing iterations per run (None: 1000 per planning unit, at least a million).
    pub num_iters: Option<i64>,
    pub start_temp: f64,
    pub cool_fac: f64,
    pub num_temp: i64,
    /// Every feature's penalty factor during the annealing, one value per run in turn.
    pub spf: Vec<f64>,
    pub itimp_type: i32,
    pub seed: i64,
}

impl Default for Anneal {
    fn default() -> Anneal {
        Anneal {
            num_reps: 10,
            num_iters: None,
            start_temp: -1.0,
            cool_fac: 0.0,
            num_temp: 10_000,
            spf: vec![1.0, 2.0],
            itimp_type: 0,
            seed: 1,
        }
    }
}

impl Anneal {
    /// The settings' errors, as the library's add_anneal_solver.
    pub fn check(&self) -> Result<(), String> {
        if self.num_reps < 1 {
            return Err("num_reps must be at least 1".into());
        }
        if self.itimp_type != 0 && self.itimp_type != 3 {
            return Err("itimp_type must be 0 or 3".into());
        }
        Ok(())
    }
}

const SUPPORTED: &str = "the anneal solver solves minimum-set problems with binary decisions: targets (and other \
                         '>=' rows on the planning units with amounts >= 0), locked units, and penalties on units \
                         and on pairs of units (boundary, connectivity, linear)";

/// The compiled model as Marxan's problem (the library's marxan_model).
struct Marxan {
    cost: Vec<f64>,
    status: Vec<i32>,
    offset: Vec<i64>,
    spindex: Vec<i32>,
    amount: Vec<f64>,
    target: Vec<f64>,
    /// The kept feature rows over the units.
    f: Csr,
    pi: Vec<usize>,
    pj: Vec<usize>,
    bound: Vec<f64>,
}

fn ids(m: &Model, rows: &[usize]) -> String {
    let mut v: Vec<&str> = rows.iter().take(1000).map(|&r| m.row_ids[r].as_str()).collect();
    v.sort_unstable();
    v.dedup();
    v.join(", ")
}

fn marxan_model(m: &Model, n: usize) -> Result<Marxan, String> {
    let bad = |what: &str| Err(format!("{SUPPORTED}; this one {what}"));
    if m.maximize {
        return bad("maximises");
    }
    if !m.vtype[..n].iter().all(|&t| t == 'B') {
        return bad("has proportion or semi-continuous decisions");
    }
    let a = &m.a;
    let ncol = m.obj.len();
    let pair_row: Vec<bool> = (0..a.nrow).map(|r| a.row(r).0.iter().any(|&c| c >= n)).collect();
    // every row on a pair column: b - x_i <= 0 (two of them) or b - x_i - x_j >= -1
    let mut wrong = Vec::new();
    for r in (0..a.nrow).filter(|&r| pair_row[r]) {
        let nnz = a.row(r).0.len();
        let le = m.sense[r] == "<=" && nnz == 2 && m.rhs[r] == 0.0;
        let ge = m.sense[r] == ">=" && nnz == 3 && m.rhs[r] == -1.0;
        if !(le || ge) {
            wrong.push(r);
        }
    }
    if !wrong.is_empty() {
        return bad(&format!("has rows on other variables ({})", ids(m, &wrong)));
    }
    let mut le_rows = Vec::new();
    for r in (0..a.nrow).filter(|&r| pair_row[r]) {
        let (cols, vals) = a.row(r);
        if !cols.iter().zip(vals).all(|(&c, &v)| if c >= n { v == 1.0 } else { v == -1.0 }) {
            return bad("has rows on other variables");
        }
        if m.sense[r] == "<=" {
            le_rows.push(r);
        }
    }
    // the unit and the pair column of every b <= x_i row (the unit's column is the smaller)
    let mut rows: Vec<(usize, usize)> = Vec::with_capacity(le_rows.len());
    for &r in &le_rows {
        let (cols, _) = a.row(r);
        if cols[0] >= n || cols[1] < n {
            return bad("has rows on other variables");
        }
        rows.push((cols[1], cols[0]));
    }
    rows.sort_unstable();
    let np = ncol - n;
    let mut pi = Vec::with_capacity(np);
    let mut pj = Vec::with_capacity(np);
    let mut k = 0;
    let mut npair = 0;
    while k < rows.len() {
        let mut e = k;
        while e < rows.len() && rows[e].0 == rows[k].0 {
            e += 1;
        }
        if e - k != 2 || rows[k].1 == rows[k + 1].1 {
            return bad("has variables other than the planning units and their pairs");
        }
        pi.push(rows[k].1);
        pj.push(rows[k + 1].1);
        npair += 1;
        k = e;
    }
    if npair != np {
        return bad("has variables other than the planning units and their pairs");
    }
    let q: Vec<f64> = m.obj[n..].to_vec();
    if (n..ncol).any(|j| !m.lb[j].is_finite() || m.lb[j] > 0.0 || m.ub[j] < 1.0) {
        return bad("bounds a pair column");
    }
    // the features: the rows on the units alone, all '>='
    let feat: Vec<usize> = (0..a.nrow).filter(|&r| !pair_row[r]).collect();
    let not_ge: Vec<usize> = feat.iter().copied().filter(|&r| m.sense[r] != ">=").collect();
    if !not_ge.is_empty() {
        return bad(&format!("has '<=' or '=' rows ({})", ids(m, &not_ge)));
    }
    if feat.iter().any(|&r| a.row(r).1.iter().any(|&v| v < 0.0)) {
        return bad("has a '>=' row with negative amounts");
    }
    // the rows met by any selection (rhs <= 0) left out
    let kept: Vec<usize> = feat.iter().copied().filter(|&r| m.rhs[r] > 0.0).collect();
    let (mut tr, mut tc, mut tv) = (Vec::new(), Vec::new(), Vec::new());
    for (i, &r) in kept.iter().enumerate() {
        let (cols, vals) = a.row(r);
        for (&c, &v) in cols.iter().zip(vals) {
            if v != 0.0 {
                tr.push(i);
                tc.push(c);
                tv.push(v);
            }
        }
    }
    let f = Csr::from_triplets(kept.len(), n, &tr, &tc, &tv);
    let (offset, spindex, amount) = engines::by_unit(m, n, &kept);
    // (np.add.at: every pair's first unit, then every second)
    let mut cost = m.obj[..n].to_vec();
    for (&i, &q) in pi.iter().zip(&q) {
        cost[i] += q / 2.0;
    }
    for (&j, &q) in pj.iter().zip(&q) {
        cost[j] += q / 2.0;
    }
    let mut status = vec![0i32; n];
    for (i, st) in status.iter_mut().enumerate() {
        if m.lb[i] >= 0.5 {
            *st = 2;
        }
        if m.ub[i] <= 0.5 {
            *st = 3;
        }
        if m.lb[i] >= 0.5 && m.ub[i] <= 0.5 {
            return Err("a planning unit is locked in and out".into());
        }
    }
    Ok(Marxan {
        cost,
        status,
        offset,
        spindex,
        amount,
        target: kept.iter().map(|&r| m.rhs[r]).collect(),
        f,
        pi,
        pj,
        bound: q.iter().map(|q| -q / 2.0).collect(),
    })
}

/// Every unit's neighbours and boundaries (both directions; repeated pairs summed).
fn neighbours(n: usize, mm: &Marxan) -> (Vec<i64>, Vec<i32>, Vec<f64>) {
    let mut per: Vec<Vec<(i32, f64)>> = vec![Vec::new(); n];
    for k in 0..mm.pi.len() {
        per[mm.pi[k]].push((mm.pj[k] as i32, mm.bound[k]));
        per[mm.pj[k]].push((mm.pi[k] as i32, mm.bound[k]));
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
fn pair_values(ncol: usize, n: usize, x: &[f64], mm: &Marxan) -> Vec<f64> {
    let mut full = vec![0.0; ncol];
    full[..n].copy_from_slice(&x[..n]);
    for k in 0..mm.pi.len() {
        full[n + k] = x[mm.pi[k]] * x[mm.pj[k]];
    }
    full
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

fn row_dot(f: &Csr, r: usize, x: &[f64]) -> f64 {
    let (cols, vals) = f.row(r);
    cols.iter().zip(vals).map(|(&c, &v)| v * x[c]).sum()
}

fn failed(status: &str, bound: f64, nan_bound: bool) -> ffi::SolveOut {
    ffi::SolveOut {
        status: status.into(),
        has_solution: false,
        x: Vec::new(),
        objective: f64::NAN,
        bound: if nan_bound { f64::NAN } else { bound },
        gap: f64::NAN,
    }
}

/// A compiled minimum-set model on Marxan's annealing (the library's solve_anneal). `time_limit`: seconds for the
/// runs (infinite: none); `start`: a solution to start from (empty: none).
pub fn solve(m: &Model, n: usize, o: &Anneal, time_limit: f64, start: &[f64]) -> Result<ffi::SolveOut, String> {
    let t0 = Instant::now();
    let mm = marxan_model(m, n)?;
    let nf = mm.target.len();
    let mut status = mm.status.clone();
    // infeasible targets (the units not locked out cannot meet them)
    let avail: Vec<f64> = status.iter().map(|&s| f64::from(u8::from(s != 3))).collect();
    if (0..nf).any(|r| row_dot(&mm.f, r, &avail) < mm.target[r] - 1e-9 * mm.target[r].abs().max(1.0)) {
        return Ok(failed("infeasible", f64::NAN, true));
    }
    if !start.is_empty() {
        for i in 0..n {
            if status[i] == 0 && start[i] >= 0.5 {
                status[i] = 1;
            }
        }
    }
    let (nb_start, nb, nb_cost) = neighbours(n, &mm);
    let timed = time_limit.is_finite();
    let iterations = o.num_iters.unwrap_or_else(|| 1_000_000.max(1000 * n as i64));
    let data = ffi::MarxanData {
        cost: mm.cost.clone(),
        status,
        offset: mm.offset.clone(),
        spindex: mm.spindex.clone(),
        amount: mm.amount.clone(),
        target: mm.target.clone(),
        prop: vec![0.0; nf],
        spf: vec![1.0; nf],
        targetocc: vec![0; nf],
        fixedcost: vec![0.0; n],
        nb_start,
        nb,
        nb_cost,
    };
    let options = ffi::MarxanOptions {
        blm: 1.0,
        prop: 0.0,
        iterations,
        tinit: o.start_temp,
        tcool: o.cool_fac,
        titns: o.num_temp,
        repeats: o.num_reps,
        costthresh: 0.0,
        tpf1: 0.0,
        tpf2: 0.0,
        misslevel: 1.0,
        runmode: 1,
        heurtype: 1,
        itimptype: o.itimp_type,
        connectivity_in: false,
        seed: (o.seed & 0xFFFF_FFFF) as u32,
        time_limit: if timed { time_limit } else { -1.0 },
        fast_penalties: true,
        repair: true,
        time_schedule: timed,
        spf_scale: o.spf.clone(),
    };
    let out = ffi::opt_marxan(&data, &options).map_err(|e| e.what().to_string())?;
    let runs = out.score.len();
    let mut best: Option<Vec<f64>> = None;
    let mut best_obj = f64::INFINITY;
    for r in 0..runs {
        let x: Vec<f64> =
            out.solutions[r * n..(r + 1) * n].iter().map(|&s| f64::from(u8::from(s == 1 || s == 2))).collect();
        if (0..nf).any(|k| row_dot(&mm.f, k, &x) < mm.target[k] - 1e-9 * mm.target[k].abs().max(1.0)) {
            continue;
        }
        let obj = dot(&m.obj, &pair_values(m.obj.len(), n, &x, &mm));
        if obj < best_obj {
            best = Some(x);
            best_obj = obj;
        }
    }
    // the lower bound: the relaxation without the boundaries' cut terms
    let mut lb_cost = mm.cost.clone();
    for (&i, &b) in mm.pi.iter().zip(&mm.bound) {
        if b < 0.0 {
            lb_cost[i] += b;
        }
    }
    for (&j, &b) in mm.pj.iter().zip(&mm.bound) {
        if b < 0.0 {
            lb_cost[j] += b;
        }
    }
    let lower = Model {
        a: mm.f.clone(),
        obj: lb_cost,
        lb: mm_status_lb(&data.status),
        ub: data.status.iter().map(|&s| f64::from(u8::from(s != 3))).collect(),
        rhs: mm.target.clone(),
        sense: vec![">=".to_string(); nf],
        vtype: vec!['C'; n],
        maximize: false,
        row_ids: vec!["target".to_string(); nf],
        col_ids: vec!["pu".to_string(); n],
        connected: Vec::new(),
        corridors: Vec::new(),
        zone_units: 0,
        engine: m.engine,
    };
    let remaining = if timed { (time_limit - t0.elapsed().as_secs_f64()).max(1.0) } else { f64::INFINITY };
    let lr = highs(&lower, n, 0.0, remaining, true, &[])?;
    let bound = if lr.status != "infeasible" { lr.bound } else { f64::NAN };
    let timed_out = out.timed_out;
    let Some(x) = best else {
        let status = if timed_out { "time_limit" } else { "error: no run met every target" };
        return Ok(failed(status, bound, false));
    };
    let gap = if bound.is_finite() {
        let g = best_obj - bound;
        if g <= 0.0 { 0.0 } else { g / best_obj.abs().max(1e-10) }
    } else {
        f64::INFINITY
    };
    let status = if gap <= 1e-9 {
        "optimal"
    } else if timed_out {
        "time_limit"
    } else {
        "heuristic"
    };
    Ok(ffi::SolveOut {
        status: status.into(),
        has_solution: true,
        x: pair_values(m.obj.len(), n, &x, &mm),
        objective: best_obj,
        bound,
        gap,
    })
}

/// The units' lower bounds: 1 where locked in.
fn mm_status_lb(status: &[i32]) -> Vec<f64> {
    status.iter().map(|&s| f64::from(u8::from(s == 2))).collect()
}
