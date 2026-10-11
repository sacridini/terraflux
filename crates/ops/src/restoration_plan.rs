//! Restoration planning (the restoration problem of plangea, Strassburg et al. 2020; as landschaft solves it): how
//! much of every converted cell to restore, for the most benefit within area targets and a budget.
//!
//! The decision is the share x_i of each cell restored, between 0 and the share that can be (its anthropic land,
//! times a limit). The benefits: the reduction of species' extinction risk (a species with habitat A of a
//! potential P has the risk 1 - (A / P)^z, the species-area relationship; restoring gives it habitat, A = A_0 +
//! sum_i g_si x_i), and linear benefits (carbon, per km² restored). The risk reduction is concave in x, so the
//! problem is solved to a proven tolerance by simplicial decomposition: plans that are vertices of the set of
//! plans (each a linear program over the cells with only the problem's rows), mixed by a small smooth master
//! problem; the bound is the Frank-Wolfe one. plangea's own procedure (the cells scored by their benefits per cost,
//! the target in steps with the benefits recomputed between them: a heuristic) is the method "steps".

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::milp::{self, Csr, Model};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

/// plangea's exponent of the species-area relationship.
pub const PLANGEA_Z: f64 = 0.25;
/// plangea's finite difference of the risk (in habitat area), method "steps".
pub const PLANGEA_DELTA: f64 = 1e-6;

pub fn register(r: &mut Registry) {
    r.add(Arc::new(PlanRestoration));
}

/// A risk benefit: species with habitat now, potential habitat and the habitat a unit share of every unit adds.
#[derive(Debug, Clone)]
pub struct Risk {
    pub weight: f64,
    /// the species' positions in the area of habitat's table
    pub species: Vec<usize>,
    pub current: Vec<f64>,
    pub potential: Vec<f64>,
    /// species x units: habitat per unit share
    pub gain: Csr,
    pub z: Vec<f64>,
    /// the habitat without the plan
    pub base: Vec<f64>,
}

/// A linear benefit: per unit share (`coef`) and plangea's per-km² layer.
#[derive(Debug, Clone)]
pub struct Linear {
    pub weight: f64,
    pub coef: Vec<f64>,
    pub layer: Vec<f64>,
}

/// A target row: its coefficients per unit share, right side and sense.
#[derive(Debug, Clone)]
pub struct Target {
    pub coef: Vec<f64>,
    pub rhs: f64,
    pub sense: String,
}

/// A restoration problem.
#[derive(Debug, Clone)]
pub struct Plan {
    pub shape: (usize, usize),
    /// the units' cells (row-major)
    pub index: Vec<usize>,
    /// the share of each unit's cell that can be restored
    pub upper: Vec<f64>,
    /// each unit's cell area (km²)
    pub area: Vec<f64>,
    pub risks: Vec<Risk>,
    pub linear: Vec<Linear>,
    pub targets: Vec<Target>,
    /// per unit share, and plangea's per-km² layer
    pub cost: Option<Vec<f64>>,
    pub cost_layer: Option<Vec<f64>>,
    pub budget: Option<f64>,
}

/// G x, every row in order.
fn mv(g: &Csr, x: &[f64]) -> Vec<f64> {
    (0..g.nrow)
        .map(|i| {
            let (idx, v) = g.row(i);
            idx.iter().zip(v).fold(0.0, |a, (&j, &w)| a + w * x[j])
        })
        .collect()
}

/// G' y.
fn rmv(g: &Csr, y: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; g.ncol];
    for (i, &yi) in y.iter().enumerate().take(g.nrow) {
        let (idx, v) = g.row(i);
        for (&j, &w) in idx.iter().zip(v) {
            out[j] += w * yi;
        }
    }
    out
}

/// 1 - risk: min(r, 1)^z.
fn h(r: f64, z: f64) -> f64 {
    r.clamp(0.0, 1.0).powf(z)
}

/// plangea's automatic scaling factor: 1 / 10^trunc(log10(max |layer|)).
fn scale(layer: &[f64]) -> f64 {
    let mx = layer.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    if !(mx > 0.0 && mx.is_finite()) {
        return 1.0;
    }
    1.0 / 10f64.powf(mx.log10().trunc())
}

impl Plan {
    /// The units: the cells with land that can be restored (`restorable` times `limit`, above 0), their area.
    pub fn new(
        restorable: &[f64],
        limit: &[f64],
        cell_area: &[f64],
        shape: (usize, usize),
    ) -> std::result::Result<Plan, String> {
        let n = shape.0 * shape.1;
        if restorable.len() != n || limit.len() != n || cell_area.len() != n {
            return Err("the restorable land, the limit and the cells' area must be on one grid".into());
        }
        let zero = |v: f64| if v.is_nan() { 0.0 } else { v };
        let u: Vec<f64> = (0..n).map(|c| zero(restorable[c]) * zero(limit[c])).collect();
        let index: Vec<usize> = (0..n).filter(|&c| u[c] > 0.0).collect();
        if index.is_empty() {
            return Err("no cell can be restored".into());
        }
        let area: Vec<f64> = index.iter().map(|&c| cell_area[c]).collect();
        if area.iter().any(|a| !(a.is_finite() && *a > 0.0)) {
            return Err("cell_area: positive on every cell of the planning units".into());
        }
        Ok(Plan {
            shape,
            upper: index.iter().map(|&c| u[c]).collect(),
            index,
            area,
            risks: Vec::new(),
            linear: Vec::new(),
            targets: Vec::new(),
            cost: None,
            cost_layer: None,
            budget: None,
        })
    }

    pub fn n(&self) -> usize {
        self.index.len()
    }

    /// The reduction of the species' extinction risk: their area of habitat now and potential, whether they
    /// count, the habitat they gain per unit of a cell restored (a layer per species on the grid), z per species,
    /// the share of the habitat value restored land does not deliver (per species).
    #[allow(clippy::too_many_arguments)]
    pub fn add_risk(
        &mut self,
        current: &[f64],
        potential: &[f64],
        valid: &[bool],
        gain: &[&[f64]],
        z: &[f64],
        discount: &[f64],
        weight: f64,
    ) -> std::result::Result<(), String> {
        if z.iter().any(|z| !(*z > 0.0 && *z < 1.0)) {
            return Err("z must be in (0, 1)".into());
        }
        if discount.iter().any(|d| !(0.0..=1.0).contains(d)) {
            return Err("restoration_discount must be in [0, 1]".into());
        }
        let keep: Vec<usize> = (0..current.len()).filter(|&s| valid[s] && potential[s] > 0.0).collect();
        let mut g = Csr { nrow: keep.len(), ncol: self.n(), start: vec![0], index: Vec::new(), value: Vec::new() };
        for &s in &keep {
            for (j, &c) in self.index.iter().enumerate() {
                let v = gain[s][c];
                if v != 0.0 && !v.is_nan() {
                    g.index.push(j);
                    g.value.push(if discount[s] != 0.0 { v * (1.0 - discount[s]) } else { v });
                }
            }
            g.start.push(g.index.len());
        }
        let cur: Vec<f64> = keep.iter().map(|&s| current[s]).collect();
        self.risks.push(Risk {
            weight,
            species: keep.clone(),
            base: cur.iter().map(|v| v.max(0.0)).collect(),
            current: cur,
            potential: keep.iter().map(|&s| potential[s]).collect(),
            gain: g,
            z: keep.iter().map(|&s| z[s]).collect(),
        });
        Ok(())
    }

    /// A benefit proportional to the area restored (values per km² on the grid).
    pub fn add_linear(&mut self, values: &[f64], weight: f64) -> std::result::Result<(), String> {
        let layer: Vec<f64> = self.index.iter().map(|&c| values[c]).collect();
        if layer.iter().any(|v| !v.is_finite()) {
            return Err("values: NaN or infinite on a planning unit".into());
        }
        let coef = layer.iter().zip(&self.area).map(|(v, a)| v * a).collect();
        self.linear.push(Linear { weight, coef, layer });
        Ok(())
    }

    /// The area restored (km²): in the landscape, or in every region (codes on the grid; a target per code);
    /// `relative`: shares of the area that can be restored.
    pub fn add_area_target(
        &mut self,
        targets: &[(Option<f64>, f64)],
        regions: Option<&[f64]>,
        sense: &str,
        relative: bool,
    ) -> std::result::Result<(), String> {
        if !matches!(sense, "<=" | ">=" | "=") {
            return Err("sense: '<=', '>=' or '='".into());
        }
        for &(code, target) in targets {
            let coef: Vec<f64> = match (code, regions) {
                (None, _) => self.area.clone(),
                (Some(k), Some(r)) => {
                    self.index.iter().zip(&self.area).map(|(&c, &a)| if r[c] == k { a } else { 0.0 }).collect()
                }
                (Some(_), None) => return Err("a target per region needs regions".into()),
            };
            let rhs = if relative {
                target * coef.iter().zip(&self.upper).map(|(a, u)| a.abs() * u).sum::<f64>()
            } else {
                target
            };
            if !rhs.is_finite() {
                return Err("targets must be finite".into());
            }
            self.targets.push(Target { coef, rhs, sense: sense.into() });
        }
        Ok(())
    }

    /// Spend at most `budget`, `cost` per km² restored (on the grid).
    pub fn add_budget(&mut self, budget: f64, cost: &[f64]) -> std::result::Result<(), String> {
        let layer: Vec<f64> = self.index.iter().map(|&c| cost[c]).collect();
        if layer.iter().any(|v| !v.is_finite()) || !budget.is_finite() {
            return Err("cost: NaN or infinite on a planning unit".into());
        }
        self.cost = Some(layer.iter().zip(&self.area).map(|(c, a)| c * a).collect());
        self.cost_layer = Some(layer);
        self.budget = Some(budget);
        Ok(())
    }

    /// Every benefit's value of a plan (the risks, then the linear ones).
    pub fn values(&self, x: &[f64]) -> Vec<f64> {
        let mut out: Vec<f64> = self
            .risks
            .iter()
            .map(|b| {
                let y = mv(&b.gain, x);
                (0..b.base.len())
                    .map(|s| h((b.base[s] + y[s]) / b.potential[s], b.z[s]) - h(b.base[s] / b.potential[s], b.z[s]))
                    .sum()
            })
            .collect();
        out.extend(self.linear.iter().map(|b| b.coef.iter().zip(x).map(|(c, x)| c * x).sum::<f64>()));
        out
    }

    /// The weighted benefit of a plan.
    pub fn objective(&self, x: &[f64]) -> f64 {
        let w = self.risks.iter().map(|b| b.weight).chain(self.linear.iter().map(|b| b.weight));
        self.values(x).iter().zip(w).map(|(v, w)| v * w).sum()
    }

    /// plangea's .multi_extinction_slope: |risk((A + delta) / P) - risk(A / P)| / delta, A the habitat without the
    /// plan plus what the plan so far adds.
    fn plangea_slopes(b: &Risk, extra: &[f64]) -> Vec<f64> {
        (0..b.base.len())
            .map(|s| {
                let a = b.base[s] + extra[s];
                let r1 = (1.0 - (a / b.potential[s]).powf(b.z[s])).max(0.0);
                let r2 = (1.0 - ((a + PLANGEA_DELTA) / b.potential[s]).powf(b.z[s])).max(0.0);
                (r2 - r1).abs() / PLANGEA_DELTA
            })
            .collect()
    }

    /// plangea's biodiversity layer at the plan so far (`cum`): the slopes times the habitat a unit of the cell's
    /// area gives, summed over the species.
    pub fn steps_layer(&self, b: &Risk, cum: &[f64]) -> Vec<f64> {
        let slopes = Self::plangea_slopes(b, &mv(&b.gain, cum));
        let mut out = vec![0.0; self.n()];
        for (s, &sl) in slopes.iter().enumerate() {
            let (idx, v) = b.gain.row(s);
            for (&j, &g) in idx.iter().zip(v) {
                out[j] += (g / self.area[j]) * sl;
            }
        }
        out
    }

    /// The rows of the plans: the targets and the budget.
    fn feasible_set(&self) -> Vec<Target> {
        let mut rows = self.targets.clone();
        if let (Some(c), Some(b)) = (&self.cost, self.budget) {
            rows.push(Target { coef: c.clone(), rhs: b, sense: "<=".into() });
        }
        rows
    }

    /// max obj x within the rows, lb <= x <= ub (a linear program on HiGHS); and whether it is optimal.
    fn lp(
        rows: &[Target],
        extra: Option<(&Csr, usize)>,
        obj: &[f64],
        lb: &[f64],
        ub: &[f64],
        time_limit: f64,
    ) -> std::result::Result<(Vec<f64>, bool), String> {
        let ncol = obj.len();
        if rows.is_empty() && extra.is_none() {
            return Ok((
                obj.iter().zip(lb.iter().zip(ub)).map(|(c, (l, u))| if *c > 0.0 { *u } else { *l }).collect(),
                true,
            ));
        }
        let (mut r, mut c, mut v) = (Vec::new(), Vec::new(), Vec::new());
        let (mut sense, mut rhs) = (Vec::new(), Vec::new());
        for (k, t) in rows.iter().enumerate() {
            for (j, &x) in t.coef.iter().enumerate() {
                if x != 0.0 {
                    r.push(k);
                    c.push(j);
                    v.push(x);
                }
            }
            sense.push(t.sense.clone());
            rhs.push(t.rhs);
        }
        // y - g x <= 0 for the lifted species (their columns after the units')
        if let Some((g, n)) = extra {
            for i in 0..g.nrow {
                let row = rows.len() + i;
                let (idx, val) = g.row(i);
                for (&j, &w) in idx.iter().zip(val) {
                    r.push(row);
                    c.push(j);
                    v.push(-w);
                }
                r.push(row);
                c.push(n + i);
                v.push(1.0);
                sense.push("<=".into());
                rhs.push(0.0);
            }
        }
        let nrow = sense.len();
        let m = Model {
            a: Csr::from_triplets(nrow, ncol, &r, &c, &v),
            obj: obj.to_vec(),
            lb: lb.to_vec(),
            ub: ub.to_vec(),
            rhs,
            sense,
            vtype: vec!['C'; ncol],
            maximize: true,
            row_ids: vec!["row".into(); nrow],
            col_ids: vec!["x".into(); ncol],
            connected: Vec::new(),
            zone_units: 0,
        };
        let s = milp::solve(&m, 0, 0.0, time_limit)?;
        if s.status == "infeasible" {
            return Err("the restoration problem is infeasible (targets or budget)".into());
        }
        if !s.has_solution {
            return Err(format!("no plan: {}", s.status));
        }
        Ok((s.x, s.status == "optimal"))
    }

    /// plangea's procedure: the targets met in `steps` parts, each a linear program on the weighted layers per
    /// cost (each layer scaled by 1 / 10^trunc(log10(max))), the layers recomputed from the plan so far between
    /// the parts.
    pub fn steps(&self, steps: usize) -> std::result::Result<Vec<f64>, String> {
        if steps < 1 {
            return Err("steps must be at least 1".into());
        }
        if self.budget.is_some() {
            return Err("the method steps takes area targets only (plangea's procedure): no budget".into());
        }
        if self.targets.is_empty() {
            return Err("the method steps needs an area target".into());
        }
        let n = self.n();
        let mut cum = vec![0.0; n];
        let mut ub = self.upper.clone();
        let mut risk_layers: Vec<Vec<f64>> = self.risks.iter().map(|b| self.steps_layer(b, &cum)).collect();
        let lin_layers: Vec<&Vec<f64>> = self.linear.iter().map(|b| &b.layer).collect();
        let sf_r: Vec<f64> = risk_layers.iter().map(|l| scale(l)).collect();
        let sf_l: Vec<f64> = lin_layers.iter().map(|l| scale(l)).collect();
        let denom: Vec<f64> = match &self.cost_layer {
            Some(c) => {
                let s = scale(c);
                c.iter().map(|v| v * s).collect()
            }
            None => vec![1.0; n],
        };
        for k in 0..steps {
            let rows: Vec<Target> = self
                .targets
                .iter()
                .map(|t| Target { coef: t.coef.clone(), rhs: t.rhs * 1.0 / steps as f64, sense: t.sense.clone() })
                .collect();
            let mut b_sum = vec![0.0; n];
            let mut any = false;
            for ((b, l), s) in self.risks.iter().zip(&risk_layers).zip(&sf_r) {
                any = true;
                for (o, v) in b_sum.iter_mut().zip(l) {
                    *o += b.weight * s * v;
                }
            }
            for ((b, l), s) in self.linear.iter().zip(&lin_layers).zip(&sf_l) {
                any = true;
                for (o, v) in b_sum.iter_mut().zip(l.iter()) {
                    *o += b.weight * s * v;
                }
            }
            let obj: Vec<f64> = (0..n).map(|i| if any { b_sum[i] / denom[i] } else { 1.0 / denom[i] }).collect();
            let (x, _) = Self::lp(&rows, None, &obj, &vec![0.0; n], &ub, f64::INFINITY)
                .map_err(|e| format!("step {}: {e}", k + 1))?;
            for i in 0..n {
                let xi = x[i].max(0.0);
                if xi > 0.0 {
                    ub[i] -= xi;
                }
                cum[i] += xi;
            }
            if steps > 1 && k + 1 < steps {
                risk_layers = self.risks.iter().map(|b| self.steps_layer(b, &cum)).collect();
            }
        }
        Ok(cum.iter().zip(&self.upper).map(|(c, u)| c.min(*u)).collect())
    }

    /// plangea's uniform scenario: every unit restored by the same share of what it can be, for a total `area`.
    pub fn uniform(&self, area: f64) -> Vec<f64> {
        let total: f64 = self.upper.iter().zip(&self.area).map(|(u, a)| u * a).sum();
        let k = if total > 0.0 { area / total } else { 0.0 };
        self.upper.iter().map(|u| u * k).collect()
    }

    /// The most weighted benefit by simplicial decomposition (landschaft's _solve_sd): the plan a mix of vertices
    /// of the set of plans, each a linear program with the benefits' gradient at the current plan; the mix by a
    /// small concave master problem (projected gradient ascent on the simplex: its accuracy only steers the
    /// search); the bound the Frank-Wolfe one, valid because the benefit is concave. A species that reaches its
    /// potential (a kink in the benefit) is lifted: its habitat gained a variable y <= g x, y <= P - A of its own.
    /// Returns the plan, the bound, the status (optimal within `tol`, time_limit, iteration_limit), the rounds.
    pub fn exact(&self, tol: f64, time_limit: f64) -> std::result::Result<(Vec<f64>, f64, String, usize), String> {
        let t0 = std::time::Instant::now();
        let left = || time_limit - t0.elapsed().as_secs_f64();
        let n = self.n();
        let risks = &self.risks;
        let mut lin = vec![0.0; n];
        for b in &self.linear {
            for (o, c) in lin.iter_mut().zip(&b.coef) {
                *o += b.weight * c;
            }
        }
        let rows = self.feasible_set();
        let lb_x = vec![0.0; n];
        let positive = |g: &Csr| -> Vec<f64> {
            (0..g.nrow)
                .map(|i| {
                    let (idx, v) = g.row(i);
                    idx.iter().zip(v).fold(0.0, |a, (&j, &w)| a + w.max(0.0) * self.upper[j])
                })
                .collect()
        };
        let floors: Vec<Vec<f64>> = risks
            .iter()
            .map(|b| {
                let pg = positive(&b.gain);
                (0..b.base.len()).map(|s| (1e-9 * (b.base[s] + pg[s]) / b.potential[s]).max(1e-15)).collect()
            })
            .collect();
        let caps: Vec<Vec<f64>> =
            risks.iter().map(|b| b.potential.iter().zip(&b.base).map(|(p, a)| (p - a).max(0.0)).collect()).collect();
        let mut lifted: Vec<Vec<bool>> = risks.iter().map(|b| vec![false; b.base.len()]).collect();
        let slopes = |i: usize, y: &[f64], lifted: &[Vec<bool>]| -> Vec<f64> {
            let b = &risks[i];
            (0..b.base.len())
                .map(|s| {
                    let r = (b.base[s] + y[s]) / b.potential[s];
                    let d = b.z[s] * r.min(1.0).max(floors[i][s]).powf(b.z[s] - 1.0) * (b.weight / b.potential[s]);
                    if r < 1.0 || lifted[i][s] { d } else { 0.0 }
                })
                .collect()
        };
        // the best vertex for cx on the units and dy on the lifted species; whether optimal
        let vertex =
            |cx: &[f64], dy: &[Vec<f64>], lifted: &[Vec<bool>]| -> std::result::Result<(Vec<f64>, bool), String> {
                let mut g_rows = Csr { nrow: 0, ncol: n, start: vec![0], index: Vec::new(), value: Vec::new() };
                let (mut obj, mut lb, mut ub) = (cx.to_vec(), lb_x.clone(), self.upper.clone());
                for (i, b) in risks.iter().enumerate() {
                    for s in (0..b.base.len()).filter(|&s| lifted[i][s]) {
                        let (idx, v) = b.gain.row(s);
                        g_rows.index.extend_from_slice(idx);
                        g_rows.value.extend_from_slice(v);
                        g_rows.start.push(g_rows.index.len());
                        g_rows.nrow += 1;
                        obj.push(dy[i][s]);
                        lb.push(0.0);
                        ub.push(caps[i][s]);
                    }
                }
                let extra = if g_rows.nrow > 0 { Some((&g_rows, n)) } else { None };
                let (x, ok) = Self::lp(&rows, extra, &obj, &lb, &ub, left().max(1e-3))?;
                Ok(((0..n).map(|j| x[j].clamp(lb_x[j], self.upper[j])).collect(), ok))
            };
        // the vertices, their gains per risk benefit (species x vertices) and linear benefit
        let mut verts: Vec<Vec<f64>> = Vec::new();
        let mut gv: Vec<Vec<Vec<f64>>> = risks.iter().map(|_| Vec::new()).collect(); // [risk][vertex][species]
        let mut lv: Vec<f64> = Vec::new();
        let add_vertex = |v: Vec<f64>,
                          verts: &mut Vec<Vec<f64>>,
                          gv: &mut Vec<Vec<Vec<f64>>>,
                          lv: &mut Vec<f64>,
                          lifted: &mut Vec<Vec<bool>>| {
            for (i, b) in risks.iter().enumerate() {
                let g = mv(&b.gain, &v);
                for s in 0..g.len() {
                    if caps[i][s] > 0.0 && g[s] >= caps[i][s] * (1.0 - 1e-12) {
                        lifted[i][s] = true;
                    }
                }
                gv[i].push(g);
            }
            lv.push(lin.iter().zip(&v).map(|(a, b)| a * b).sum());
            verts.push(v);
        };
        // a lifted species counts min(g v, P - A) of a vertex
        let effective = |i: usize, k: usize, gv: &[Vec<Vec<f64>>], lifted: &[Vec<bool>]| -> Vec<f64> {
            gv[i][k].iter().enumerate().map(|(s, &g)| if lifted[i][s] { g.min(caps[i][s]) } else { g }).collect()
        };
        let zeros: Vec<Vec<f64>> = risks.iter().map(|b| vec![0.0; b.base.len()]).collect();
        let mut c0 = lin.clone();
        let d0: Vec<Vec<f64>> = (0..risks.len()).map(|i| slopes(i, &zeros[i], &lifted)).collect();
        for (i, b) in risks.iter().enumerate() {
            for (o, v) in c0.iter_mut().zip(rmv(&b.gain, &d0[i])) {
                *o += v;
            }
        }
        let (v0, _) = vertex(&c0, &d0, &lifted)?;
        add_vertex(v0, &mut verts, &mut gv, &mut lv, &mut lifted);
        let (mut bound, mut best_x, mut best) = (f64::INFINITY, vec![0.0; n], f64::NEG_INFINITY);
        let mut status = "iteration_limit".to_string();
        let mut lam = vec![1.0];
        let mut it = 0;
        for round in 1..=5000 {
            it = round;
            let k = verts.len();
            let ev: Vec<Vec<Vec<f64>>> =
                (0..risks.len()).map(|i| (0..k).map(|j| effective(i, j, &gv, &lifted)).collect()).collect();
            let ys = |lam: &[f64], i: usize| -> Vec<f64> {
                let ns = risks[i].base.len();
                (0..ns).map(|s| (0..lam.len()).map(|j| ev[i][j][s] * lam[j]).sum()).collect()
            };
            // the master: the best mix of the vertices
            let f = |lam: &[f64]| -> f64 {
                let mut v: f64 = lv.iter().zip(lam).map(|(a, b)| a * b).sum();
                for (i, b) in risks.iter().enumerate() {
                    let y = ys(lam, i);
                    v += b.weight
                        * (0..b.base.len()).map(|s| h((b.base[s] + y[s]) / b.potential[s], b.z[s])).sum::<f64>();
                }
                v
            };
            let grad = |lam: &[f64]| -> Vec<f64> {
                let mut g = lv.clone();
                for (i, evi) in ev.iter().enumerate() {
                    let d = slopes(i, &ys(lam, i), &lifted);
                    for (gj, e) in g.iter_mut().zip(evi) {
                        *gj += e.iter().zip(&d).map(|(a, b)| a * b).sum::<f64>();
                    }
                }
                g
            };
            if k > 1 {
                lam = master(&lam, &f, &grad);
            }
            let mut x = vec![0.0; n];
            for (w, v) in lam.iter().zip(&verts) {
                if *w > 0.0 {
                    for (o, a) in x.iter_mut().zip(v) {
                        *o += w * a;
                    }
                }
            }
            for j in 0..n {
                x[j] = x[j].clamp(lb_x[j], self.upper[j]);
            }
            let fx = self.objective(&x);
            if fx > best {
                best = fx;
                best_x = x.clone();
            }
            let ysv: Vec<Vec<f64>> = (0..risks.len()).map(|i| ys(&lam, i)).collect();
            let f_lift: f64 = lv.iter().zip(&lam).map(|(a, b)| a * b).sum::<f64>()
                + risks
                    .iter()
                    .zip(&ysv)
                    .map(|(b, y)| {
                        b.weight
                            * (0..b.base.len())
                                .map(|s| {
                                    h((b.base[s] + y[s]) / b.potential[s], b.z[s])
                                        - h(b.base[s] / b.potential[s], b.z[s])
                                })
                                .sum::<f64>()
                    })
                    .sum::<f64>();
            let dy: Vec<Vec<f64>> = (0..risks.len()).map(|i| slopes(i, &ysv[i], &lifted)).collect();
            let mut cx = lin.clone();
            for (i, b) in risks.iter().enumerate() {
                let free: Vec<f64> = dy[i].iter().zip(&lifted[i]).map(|(d, l)| if *l { 0.0 } else { *d }).collect();
                if free.iter().any(|v| *v != 0.0) {
                    for (o, v) in cx.iter_mut().zip(rmv(&b.gain, &free)) {
                        *o += v;
                    }
                }
            }
            let lift_val: Vec<Vec<f64>> = dy
                .iter()
                .zip(&lifted)
                .map(|(d, l)| d.iter().zip(l).map(|(d, l)| if *l { *d } else { 0.0 }).collect())
                .collect();
            let (v, exact) = vertex(&cx, &lift_val, &lifted)?;
            if exact {
                let gain_v: f64 = risks
                    .iter()
                    .enumerate()
                    .map(|(i, b)| {
                        let g = mv(&b.gain, &v);
                        (0..g.len()).map(|s| lift_val[i][s] * (g[s].min(caps[i][s]) - ysv[i][s])).sum::<f64>()
                    })
                    .sum();
                let dir: f64 = cx.iter().zip(v.iter().zip(&x)).map(|(c, (a, b))| c * (a - b)).sum();
                bound = bound.min((f_lift + dir + gain_v).max(fx));
            }
            if bound - best <= tol * bound.abs().max(1.0) {
                status = "optimal".into();
                break;
            }
            if left() <= 0.0 {
                status = "time_limit".into();
                break;
            }
            // drop the vertices out of the mix
            let keep: Vec<usize> = (0..lam.len()).filter(|&j| lam[j] > 1e-12).collect();
            if keep.len() < lam.len() {
                verts = keep.iter().map(|&j| verts[j].clone()).collect();
                for g in gv.iter_mut() {
                    *g = keep.iter().map(|&j| g[j].clone()).collect();
                }
                lv = keep.iter().map(|&j| lv[j]).collect();
                let total: f64 = keep.iter().map(|&j| lam[j]).sum();
                lam = keep.iter().map(|&j| lam[j] / total).collect();
            }
            add_vertex(v, &mut verts, &mut gv, &mut lv, &mut lifted);
            lam.push(0.0);
        }
        Ok((best_x, bound, status, it))
    }
}

/// The Euclidean projection of v on the simplex.
fn project_simplex(v: &[f64]) -> Vec<f64> {
    let mut u = v.to_vec();
    u.sort_by(|a, b| b.total_cmp(a));
    let (mut css, mut theta) = (0.0, 0.0);
    for (k, &x) in u.iter().enumerate() {
        css += x;
        let t = (css - 1.0) / (k + 1) as f64;
        if x - t > 0.0 {
            theta = t;
        }
    }
    v.iter().map(|x| (x - theta).max(0.0)).collect()
}

/// The best mix of the vertices: the concave f's maximum on the simplex by projected gradient ascent with a
/// backtracking step, from lam0 (kept unless improved).
fn master(lam0: &[f64], f: &dyn Fn(&[f64]) -> f64, grad: &dyn Fn(&[f64]) -> Vec<f64>) -> Vec<f64> {
    let mut lam = lam0.to_vec();
    let mut fl = f(&lam);
    let mut step = 1.0;
    for _ in 0..1000 {
        let g = grad(&lam);
        let gn = g.iter().map(|v| v * v).sum::<f64>().sqrt();
        if gn.is_nan() || gn <= 0.0 {
            break;
        }
        let mut improved = false;
        let mut t = step / gn;
        for _ in 0..60 {
            let cand = project_simplex(&lam.iter().zip(&g).map(|(l, g)| l + t * g).collect::<Vec<_>>());
            let fc = f(&cand);
            if fc > fl {
                let gain = fc - fl;
                lam = cand;
                fl = fc;
                improved = gain > 1e-15 * fl.abs().max(1.0);
                step = (t * gn * 2.0).min(1e6);
                break;
            }
            t *= 0.5;
        }
        if !improved {
            break;
        }
    }
    lam
}

struct PlanRestoration;

impl Operator for PlanRestoration {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "restoration.plan",
            "Restoration planning as plangea (Strassburg et al. 2020): how much of every converted cell to \
             restore for the most benefit within area targets and a budget: the reduction of the species' \
             extinction risk (the species-area relationship on their area of habitat, sdm.area_of_habitat's \
             table and gain) and linear benefits (carbon per km²). exact: the optimum within a tolerance \
             (simplicial decomposition with a proven bound); steps: plangea's procedure (the target in steps, \
             the benefits recomputed between them); uniform: every cell by the same share.",
            Access::Global,
        )
        .input(PortSpec::new("restorable", Type::Map, "the share of every cell that can be restored (anthropic land)"))
        .input(PortSpec::optional("table", Type::Table, "sdm.area_of_habitat's table: current, potential, valid"))
        .input(PortSpec::optional(
            "gain",
            Type::Stack,
            "sdm.area_of_habitat's gain: habitat per unit of a cell restored",
        ))
        .input(PortSpec::optional("linear", Type::Stack, "linear benefits per km² restored (carbon...), a layer each"))
        .input(PortSpec::optional("cost", Type::Map, "the cost per km² restored (with budget)"))
        .input(PortSpec::optional("regions", Type::Map, "region codes, for targets per region"))
        .input(PortSpec::optional(
            "limit",
            Type::Map,
            "plangea's bound limit: a share per cell multiplying the restorable",
        ))
        .output(PortSpec::new("restored", Type::Map, "the share of every cell restored"))
        .output(PortSpec::new(
            "summary",
            Type::Table,
            "the weighted benefit, the area (km²), the cost, the bound (exact), optimal (1) and the rounds",
        ))
        .output(PortSpec::new(
            "benefits",
            Type::Table,
            "per benefit (the risks, then the linear ones): value, weight, weighted",
        ))
        .output(PortSpec::new(
            "species",
            Type::Table,
            "per species counted (its row in the area of habitat's table): the risk without and with the plan",
        ))
        .param(PortSpec::optional("method", Type::Text, "exact (default), steps or uniform"))
        .param(PortSpec::optional("steps", Type::Integer, "steps: the parts of the target (default 1)"))
        .param(PortSpec::optional(
            "target",
            Type::Any,
            "the area restored (km²): a number for the landscape, or {region code: area} with regions",
        ))
        .param(PortSpec::optional("sense", Type::Text, "the targets' sense: <= (default, at most), >=, ="))
        .param(PortSpec::optional("relative", Type::Bool, "the targets are shares of the area that can be restored"))
        .param(PortSpec::optional("budget", Type::Number, "the most the plan may cost (with cost)"))
        .param(PortSpec::optional("z", Type::Any, "the species-area exponent: a number (0.25) or one per species"))
        .param(PortSpec::optional("weight", Type::Number, "the risk benefit's weight (1)"))
        .param(PortSpec::optional("linear_weights", Type::Any, "the linear benefits' weights (1 each)"))
        .param(PortSpec::optional(
            "restoration_discount",
            Type::Any,
            "the share of the habitat value restored land does not deliver (0; one or one per species)",
        ))
        .param(PortSpec::optional("cell_area", Type::Number, "a cell's area in km² (default: the grid's)"))
        .param(PortSpec::optional("tolerance", Type::Number, "exact: the relative gap to the bound (1e-7)"))
        .param(PortSpec::optional("time_limit", Type::Number, "exact: seconds (default none)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let restorable = args.map("restorable")?;
        let grid = restorable.grid.clone();
        let ncell = grid.width * grid.height;
        let area: Vec<f64> = match args.opt_param::<f64>("cell_area")? {
            Some(a) => vec![a; ncell],
            None => crate::restoration::cell_area_km2(&grid)
                .ok_or_else(|| err("the grid has no coordinate system: give cell_area (km²)".into()))?,
        };
        let limit = match args.opt_map("limit")? {
            Some(m) => m.data.to_vec(),
            None => vec![1.0; ncell],
        };
        let mut p = Plan::new(&restorable.data, &limit, &area, (grid.height, grid.width)).map_err(err)?;
        let per_species = |name: &str, default: f64, k: usize| -> Result<Vec<f64>> {
            match args.params.get(name) {
                None | Some(serde_json::Value::Null) => Ok(vec![default; k]),
                Some(serde_json::Value::Number(x)) => Ok(vec![x.as_f64().unwrap(); k]),
                Some(v) => {
                    let l: Vec<f64> = serde_json::from_value(v.clone()).map_err(|e| err(format!("{name}: {e}")))?;
                    if l.len() != k {
                        return Err(err(format!("{name}: one value or one per species ({k})")));
                    }
                    Ok(l)
                }
            }
        };
        if let (Some(Value::Table(t)), Some(Value::Stack(g))) = (args.get("table"), args.get("gain")) {
            let col = |c: &str| t.column(c).ok_or_else(|| err(format!("the table needs a column {c}")));
            let (cur, pot, valid) = (col("current")?, col("potential")?, col("valid")?);
            let nspecies = cur.len();
            if g.layers.len() != nspecies || g.layers.iter().any(|l| l.len() != ncell) {
                return Err(err("the gain needs a layer per species of the table, on the restorable's grid".into()));
            }
            let layers: Vec<&[f64]> = g.layers.iter().map(|l| l.as_slice()).collect();
            p.add_risk(
                &cur,
                &pot,
                &valid.iter().map(|v| *v != 0.0).collect::<Vec<_>>(),
                &layers,
                &per_species("z", PLANGEA_Z, nspecies)?,
                &per_species("restoration_discount", 0.0, nspecies)?,
                args.param("weight", 1.0)?,
            )
            .map_err(err)?;
        }
        if let Some(Value::Stack(l)) = args.get("linear") {
            let w: Vec<f64> = args.param("linear_weights", vec![1.0; l.layers.len()])?;
            if w.len() != l.layers.len() {
                return Err(err("linear_weights: one per linear benefit".into()));
            }
            for (layer, w) in l.layers.iter().zip(w) {
                p.add_linear(layer, w).map_err(err)?;
            }
        }
        let sense: String = args.param("sense", "<=".to_string())?;
        let relative: bool = args.param("relative", false)?;
        match args.params.get("target") {
            None | Some(serde_json::Value::Null) => {}
            Some(serde_json::Value::Number(x)) => {
                p.add_area_target(&[(None, x.as_f64().unwrap())], None, &sense, relative).map_err(err)?
            }
            Some(serde_json::Value::Object(m)) => {
                let regions = args.map("regions")?;
                let mut targets = Vec::new();
                for (k, v) in m {
                    let code: f64 = k.parse().map_err(|_| err(format!("target: region {k:?} is no number")))?;
                    targets.push((Some(code), v.as_f64().ok_or_else(|| err("target: areas are numbers".into()))?));
                }
                p.add_area_target(&targets, Some(&regions.data), &sense, relative).map_err(err)?;
            }
            Some(_) => return Err(err("target: a number, or {region: area}".into())),
        }
        if let Some(b) = args.opt_param::<f64>("budget")? {
            let cost = args.map("cost")?;
            p.add_budget(b, &cost.data).map_err(err)?;
        }
        let method: String = args.param("method", "exact".to_string())?;
        let (x, bound, status, rounds) = match method.as_str() {
            "exact" => p
                .exact(args.param("tolerance", 1e-7)?, args.opt_param("time_limit")?.unwrap_or(f64::INFINITY))
                .map_err(err)?,
            "steps" => {
                let k: usize = args.param("steps", 1)?;
                (p.steps(k).map_err(err)?, f64::NAN, "heuristic".into(), k)
            }
            "uniform" => {
                let a = p.targets.first().map(|t| t.rhs).ok_or_else(|| err("uniform: give the target".into()))?;
                (p.uniform(a), f64::NAN, "uniform".into(), 0)
            }
            m => return Err(err(format!("method: exact, steps or uniform, not {m:?}"))),
        };
        let mut restored = vec![f64::NAN; ncell];
        for (i, &c) in p.index.iter().enumerate() {
            restored[c] = x[i];
        }
        let values = p.values(&x);
        let weights: Vec<f64> = p.risks.iter().map(|b| b.weight).chain(p.linear.iter().map(|b| b.weight)).collect();
        let mut benefits = Table::new(vec!["value".into(), "weight".into(), "weighted".into()]);
        benefits.rows = values.iter().zip(&weights).map(|(v, w)| vec![*v, *w, v * w]).collect();
        let total: f64 = values.iter().zip(&weights).map(|(v, w)| v * w).sum();
        let area_x: f64 = p.area.iter().zip(&x).map(|(a, x)| a * x).sum();
        let cost = p.cost.as_ref().map_or(f64::NAN, |c| c.iter().zip(&x).map(|(c, x)| c * x).sum());
        let mut summary =
            Table::new(["benefit", "area", "cost", "bound", "optimal", "rounds"].map(String::from).to_vec());
        summary.rows.push(vec![total, area_x, cost, bound, f64::from(u8::from(status == "optimal")), rounds as f64]);
        let mut species = Table::new(vec!["species".into(), "risk_before".into(), "risk_after".into()]);
        for b in &p.risks {
            let y = mv(&b.gain, &x);
            for (s, &row) in b.species.iter().enumerate() {
                species.rows.push(vec![
                    (row + 1) as f64,
                    1.0 - h(b.base[s] / b.potential[s], b.z[s]),
                    1.0 - h((b.base[s] + y[s]) / b.potential[s], b.z[s]),
                ]);
            }
        }
        Ok(outputs([
            ("restored", Value::Map(Map::new(grid, restored))),
            ("summary", Value::Table(summary)),
            ("benefits", Value::Table(benefits)),
            ("species", Value::Table(species)),
        ]))
    }
}
