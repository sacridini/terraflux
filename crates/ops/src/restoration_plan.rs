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

use terraflux_ls_sys::ffi;

use crate::milp::{self, Csr, Model};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

/// plangea's exponent of the species-area relationship.
pub const PLANGEA_Z: f64 = 0.25;
/// plangea's finite difference of the risk (in habitat area), method "steps".
pub const PLANGEA_DELTA: f64 = 1e-6;

pub fn register(r: &mut Registry) {
    r.add(Arc::new(PlanRestoration));
    r.add(Arc::new(EcosystemAreasOp));
    r.add(Arc::new(EcosystemTargetsOp));
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

/// A linear benefit: per unit share (`coef`) and plangea's per-km² layer. Intactness is one too, linearised at
/// the plan of nothing (`exact` false) or its true value (`exact`: the total intactness gained, `t0` the total
/// without the plan).
#[derive(Debug, Clone)]
pub struct Linear {
    pub weight: f64,
    pub coef: Vec<f64>,
    pub layer: Vec<f64>,
    pub intactness: Option<Footprint>,
    pub exact: bool,
    pub t0: f64,
}

/// What a plan does to the human footprint, for the intactness benefit.
#[derive(Debug, Clone)]
pub struct Footprint {
    /// the footprint without the plan (the grid's cells; conservation's baseline loss added)
    pub h0: Vec<f64>,
    /// per unit: the footprint's change per unit share (negative lowers it)
    pub change: Vec<f64>,
    pub radius: i32,
    pub gamma: f64,
    pub beta: f64,
    pub z: f64,
    /// a cell's width and height in km
    pub cell: (f64, f64),
}

/// Benefits linear in x plus terms coef_e min(A_e x, cap_e) (concave), for the exact solver: the terms are
/// lifted, y_e <= A_e x and lo_e <= y_e <= cap_e in its linear programs.
#[derive(Debug, Clone)]
pub struct Capped {
    pub lin: Vec<f64>,
    pub a: Csr,
    pub cap: Vec<f64>,
    pub lo: Vec<f64>,
    pub coef: Vec<f64>,
}

impl Capped {
    fn empty(n: usize) -> Capped {
        Capped {
            lin: vec![0.0; n],
            a: Csr { nrow: 0, ncol: n, start: vec![0], index: Vec::new(), value: Vec::new() },
            cap: Vec::new(),
            lo: Vec::new(),
            coef: Vec::new(),
        }
    }

    fn push(&mut self, idx: &[usize], v: &[f64], cap: f64, lo: f64, coef: f64) {
        self.a.index.extend_from_slice(idx);
        self.a.value.extend_from_slice(v);
        self.a.start.push(self.a.index.len());
        self.a.nrow += 1;
        self.cap.push(cap);
        self.lo.push(lo);
        self.coef.push(coef);
    }

    /// min(A x, cap) of every term.
    fn y(&self, x: &[f64]) -> Vec<f64> {
        mv(&self.a, x).iter().zip(&self.cap).map(|(a, c)| a.min(*c)).collect()
    }

    fn value(&self, x: &[f64]) -> f64 {
        dot(&self.lin, x) + dot(&self.coef, &self.y(x))
    }

    /// The terms of several, one after another.
    fn combine(parts: Vec<Capped>, n: usize) -> Capped {
        let mut out = Capped::empty(n);
        for c in parts {
            for (o, v) in out.lin.iter_mut().zip(&c.lin) {
                *o += v;
            }
            for e in 0..c.a.nrow {
                let (idx, v) = c.a.row(e);
                out.push(idx, v, c.cap[e], c.lo[e], c.coef[e]);
            }
        }
        out
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
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
    /// per unit: the least share (none: 0), for nested trade-offs
    pub lower: Option<Vec<f64>>,
    /// per unit: its action (0 restore, 1 conserve, 2 convert; block after block), and for conservation the
    /// natural share lost without it and the part of it lost (the threat)
    pub action: Vec<u8>,
    pub natural: Vec<f64>,
    pub threat: Vec<f64>,
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
        let n_units = index.len();
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
            lower: None,
            action: vec![0; n_units],
            natural: vec![0.0; n_units],
            threat: vec![1.0; n_units],
        })
    }

    /// The units of every action (block after block: restore, conserve, convert): the cells with land the action
    /// can take (its share times `limit`, above 0). Conservation keeps natural land that would be lost without it
    /// (`threat` of it; plangea's C); conversion takes natural land (plangea's D; with restoration, R + D). Not
    /// conservation and conversion together.
    pub fn with_actions(
        restorable: Option<&[f64]>,
        conservable: Option<&[f64]>,
        convertible: Option<&[f64]>,
        limit: &[f64],
        threat: &[f64],
        cell_area: &[f64],
        shape: (usize, usize),
    ) -> std::result::Result<Plan, String> {
        if conservable.is_some() && convertible.is_some() {
            return Err("conservable and convertible: the same natural land is either conserved or converted".into());
        }
        let n = shape.0 * shape.1;
        let zero = |v: f64| if v.is_nan() { 0.0 } else { v };
        let (mut index, mut upper, mut action, mut natural, mut thr) = (vec![], vec![], vec![], vec![], vec![]);
        for (code, share) in [restorable, conservable, convertible].into_iter().enumerate() {
            let Some(share) = share else { continue };
            if share.len() != n || limit.len() != n || cell_area.len() != n || threat.len() != n {
                return Err("the shares, the limit, the threat and the cells' area must be on one grid".into());
            }
            for c in 0..n {
                let u = zero(share[c]) * zero(limit[c]);
                if u > 0.0 {
                    index.push(c);
                    upper.push(u);
                    action.push(code as u8);
                    natural.push(if code == 1 { zero(share[c]) } else { 0.0 });
                    thr.push(if code == 1 { zero(threat[c]) } else { 1.0 });
                }
            }
        }
        if index.is_empty() {
            return Err("no cell can be acted on".into());
        }
        if thr.iter().any(|t| !(0.0..=1.0).contains(t)) {
            return Err("threat must be in [0, 1]".into());
        }
        let area: Vec<f64> = index.iter().map(|&c| cell_area[c]).collect();
        if area.iter().any(|a| !(a.is_finite() && *a > 0.0)) {
            return Err("cell_area: positive on every cell of the planning units".into());
        }
        Ok(Plan {
            shape,
            index,
            upper,
            area,
            risks: Vec::new(),
            linear: Vec::new(),
            targets: Vec::new(),
            cost: None,
            cost_layer: None,
            budget: None,
            lower: None,
            action,
            natural,
            threat: thr,
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
        self.add_risk_loss(current, potential, valid, gain, &[], z, discount, weight)
    }

    /// As add_risk with the habitat every species loses per unit of a cell's natural land (sdm.area_of_habitat's
    /// loss): conserving keeps the habitat the baseline would lose (by the threat), converting takes it away.
    #[allow(clippy::too_many_arguments)]
    pub fn add_risk_loss(
        &mut self,
        current: &[f64],
        potential: &[f64],
        valid: &[bool],
        gain: &[&[f64]],
        loss: &[&[f64]],
        z: &[f64],
        discount: &[f64],
        weight: f64,
    ) -> std::result::Result<(), String> {
        if self.action.contains(&0) && gain.is_empty() {
            return Err("restoration needs the gain (area_of_habitat given the original vegetation)".into());
        }
        if self.action.iter().any(|a| *a != 0) && loss.is_empty() {
            return Err("conservation and conversion need the loss (area_of_habitat with loss)".into());
        }
        if z.iter().any(|z| !(*z > 0.0 && *z < 1.0)) {
            return Err("z must be in (0, 1)".into());
        }
        if discount.iter().any(|d| !(0.0..=1.0).contains(d)) {
            return Err("restoration_discount must be in [0, 1]".into());
        }
        let keep: Vec<usize> = (0..current.len()).filter(|&s| valid[s] && potential[s] > 0.0).collect();
        let mut g = Csr { nrow: keep.len(), ncol: self.n(), start: vec![0], index: Vec::new(), value: Vec::new() };
        let cur: Vec<f64> = keep.iter().map(|&s| current[s]).collect();
        let mut base = cur.clone();
        for (k, &s) in keep.iter().enumerate() {
            // without the plan the conservable land is lost (by the threat): the baseline loses its habitat
            let mut lost = 0.0;
            for (j, &c) in self.index.iter().enumerate() {
                let v = match self.action[j] {
                    0 => gain[s][c],
                    _ => loss[s][c],
                };
                if v == 0.0 || v.is_nan() {
                    continue;
                }
                let w = match self.action[j] {
                    0 if discount[s] != 0.0 => v * (1.0 - discount[s]),
                    0 => v,
                    1 => {
                        lost += v * (self.threat[j] * self.natural[j]);
                        v * self.threat[j]
                    }
                    _ => -v,
                };
                g.index.push(j);
                g.value.push(w);
            }
            g.start.push(g.index.len());
            base[k] -= lost;
        }
        self.risks.push(Risk {
            weight,
            species: keep.clone(),
            base: base.iter().map(|v| v.max(0.0)).collect(),
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
        self.linear.push(Linear { weight, coef, layer, intactness: None, exact: false, t0: f64::NAN });
        Ok(())
    }

    /// Ecological intactness (Beyer et al. 2020): restoring a share x of a cell lowers its human footprint by
    /// `reduction` x, converting it raises the footprint by `increase` x (and natural land lost for want of
    /// conservation by `increase` times the threat, which conserving spares); the benefit is the total intactness
    /// gained. `hfi`, `reduction` and `increase` on the grid; the other parameters as restoration.intactness.
    /// Linearised at the plan of nothing (restoration.intactness_gradient; with the method steps recomputed at
    /// every step, as plangea does), or `exact`: the total intactness itself, which is not concave (the exact
    /// method improves the plan by successive concave minorants to a local optimum and bounds the best plan by
    /// the secants of the pairs).
    #[allow(clippy::too_many_arguments)]
    pub fn add_intactness(
        &mut self,
        hfi: &[f64],
        reduction: Option<&[f64]>,
        increase: Option<&[f64]>,
        radius: i32,
        weight: f64,
        (gamma, beta, z): (f64, f64, f64),
        cell: (f64, f64),
        exact: bool,
    ) -> std::result::Result<(), String> {
        let ncell = self.shape.0 * self.shape.1;
        if hfi.len() != ncell {
            return Err("hfi: not on the problem's grid".into());
        }
        if exact && weight < 0.0 {
            return Err("the exact intactness needs a weight of at least 0".into());
        }
        let zero = |v: f64| if v.is_nan() { 0.0 } else { v };
        let mut change = vec![0.0; self.n()];
        let mut h0 = hfi.to_vec();
        if self.action.contains(&0) {
            let r = reduction.ok_or("restoration needs the reduction of the footprint per unit restored")?;
            if r.len() != ncell {
                return Err("reduction: not on the problem's grid".into());
            }
            for (j, &c) in self.index.iter().enumerate() {
                if self.action[j] == 0 {
                    change[j] = -zero(r[c]);
                }
            }
        }
        if self.action.iter().any(|a| *a != 0) {
            let inc =
                increase.ok_or("conservation and conversion need the increase of the footprint per unit converted")?;
            if inc.len() != ncell {
                return Err("increase: not on the problem's grid".into());
            }
            for (j, &c) in self.index.iter().enumerate() {
                match self.action[j] {
                    2 => change[j] = zero(inc[c]),
                    1 => {
                        change[j] = -zero(inc[c]) * self.threat[j];
                        h0[c] += zero(inc[c]) * self.threat[j] * self.natural[j];
                    }
                    _ => {}
                }
            }
        }
        let fp = Footprint { h0, change, radius, gamma, beta, z, cell };
        let zeros = vec![0.0; self.n()];
        let coef = self.intactness_coef(&fp, &zeros)?;
        let t0 = if exact { self.intactness_total(&fp, &zeros)? } else { f64::NAN };
        self.linear.push(Linear { weight, layer: coef.clone(), coef, intactness: Some(fp), exact, t0 });
        Ok(())
    }

    /// The human footprint (the grid's cells) under the plan x: without it, plus the units' changes, at least 0.
    fn footprint_at(&self, fp: &Footprint, x: &[f64]) -> Vec<f64> {
        let mut h = fp.h0.clone();
        if x.iter().any(|v| *v != 0.0) {
            let mut d = vec![0.0; h.len()];
            for (j, &c) in self.index.iter().enumerate() {
                d[c] += fp.change[j] * x[j];
            }
            for (h, d) in h.iter_mut().zip(d) {
                if !h.is_nan() {
                    let v = *h + d;
                    *h = if v < 0.0 { 0.0 } else { v };
                }
            }
        }
        h
    }

    /// The total intactness under the plan x.
    fn intactness_total(&self, fp: &Footprint, x: &[f64]) -> std::result::Result<f64, String> {
        let h = self.footprint_at(fp, x);
        let (ny, nx) = (self.shape.0 as i64, self.shape.1 as i64);
        let q = ffi::rest_intactness(&h, ny, nx, fp.gamma, fp.beta, fp.z, fp.radius, fp.cell.0, fp.cell.1)
            .map_err(|e| e.to_string())?;
        Ok(q.iter().filter(|v| !v.is_nan()).sum())
    }

    /// The derivative of the total intactness at the footprint `h` when every cell's footprint falls by `a` per
    /// unit (NaN as 0).
    fn intactness_gradient(&self, fp: &Footprint, h: &[f64], a: &[f64]) -> std::result::Result<Vec<f64>, String> {
        let (ny, nx) = (self.shape.0 as i64, self.shape.1 as i64);
        let g = ffi::rest_intactness_gradient(h, a, ny, nx, fp.gamma, fp.beta, fp.z, fp.radius, fp.cell.0, fp.cell.1)
            .map_err(|e| e.to_string())?;
        Ok(g.into_iter().map(|v| if v.is_nan() { 0.0 } else { v }).collect())
    }

    /// The derivative of the total intactness in every unit's share, at the plan x (an action at a time: a cell
    /// can be a unit of two).
    fn intactness_coef(&self, fp: &Footprint, x: &[f64]) -> std::result::Result<Vec<f64>, String> {
        let h = self.footprint_at(fp, x);
        let mut out = vec![0.0; self.n()];
        for code in 0..3u8 {
            if !self.action.contains(&code) {
                continue;
            }
            let mut a = vec![0.0; h.len()];
            for (j, &c) in self.index.iter().enumerate() {
                if self.action[j] == code {
                    a[c] = -fp.change[j];
                }
            }
            let g = self.intactness_gradient(fp, &h, &a)?;
            for (j, &c) in self.index.iter().enumerate() {
                if self.action[j] == code {
                    out[j] = g[c];
                }
            }
        }
        Ok(out)
    }

    /// The footprint's reduction t_i(x) = min(a_i x, h0_i) of every cell with units: the cells, a (cells x units)
    /// and the bounds of a x over the plans' box.
    fn reduction_terms(&self, fp: &Footprint) -> (Vec<usize>, Csr, Vec<f64>, Vec<f64>) {
        let mut cells = self.index.clone();
        cells.sort_unstable();
        cells.dedup();
        let rows: Vec<usize> = self.index.iter().map(|c| cells.binary_search(c).unwrap()).collect();
        let cols: Vec<usize> = (0..self.n()).collect();
        let vals: Vec<f64> = fp.change.iter().map(|c| -c).collect();
        let a = Csr::from_triplets(cells.len(), self.n(), &rows, &cols, &vals);
        let lb = self.lower.clone().unwrap_or_else(|| vec![0.0; self.n()]);
        let (mut lo, mut hi) = (vec![0.0; cells.len()], vec![0.0; cells.len()]);
        for e in 0..cells.len() {
            let (idx, v) = a.row(e);
            for (&j, &w) in idx.iter().zip(v) {
                let (at_hi, at_lo) = if w > 0.0 { (self.upper[j], lb[j]) } else { (lb[j], self.upper[j]) };
                hi[e] += w * at_hi;
                lo[e] += w * at_lo;
            }
        }
        (cells, a, lo, hi)
    }

    /// coef_i t_i(x) over the cells (`coef_cell` per cell of the grid): linear where the footprint cannot reach 0,
    /// lifted where it can.
    fn capped_terms(&self, fp: &Footprint, coef_cell: &[f64]) -> Capped {
        let (cells, a, lo, hi) = self.reduction_terms(fp);
        let mut out = Capped::empty(self.n());
        for (e, &c) in cells.iter().enumerate() {
            let (h0, k) = (fp.h0[c], if coef_cell[c].is_nan() { 0.0 } else { coef_cell[c] });
            if !h0.is_finite() || k == 0.0 {
                continue;
            }
            let (idx, v) = a.row(e);
            if hi[e] <= h0 {
                for (&j, &w) in idx.iter().zip(v) {
                    out.lin[j] += w * k;
                }
            } else {
                out.push(idx, v, h0, lo[e].min(h0), k);
            }
        }
        out
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
            // the area of every action, or with restoration and conversion the net change of natural land
            let net = self.action.contains(&0) && self.action.contains(&2);
            let area: Vec<f64> = self
                .area
                .iter()
                .zip(&self.action)
                .map(|(a, act)| {
                    if net && *act == 2 {
                        -a
                    } else if net && *act == 1 {
                        0.0
                    } else {
                        *a
                    }
                })
                .collect();
            let coef: Vec<f64> = match (code, regions) {
                (None, _) => area,
                (Some(k), Some(r)) => {
                    self.index.iter().zip(&area).map(|(&c, &a)| if r[c] == k { a } else { 0.0 }).collect()
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
        if !budget.is_finite() {
            return Err("budget must be finite".into());
        }
        self.add_cost(cost)?;
        self.budget = Some(budget);
        Ok(())
    }

    /// The cost per km² acted on (on the grid), without a budget (for the minimum cost).
    pub fn add_cost(&mut self, cost: &[f64]) -> std::result::Result<(), String> {
        let layer: Vec<f64> = self.index.iter().map(|&c| cost[c]).collect();
        if layer.iter().any(|v| !v.is_finite()) {
            return Err("cost: NaN or infinite on a planning unit".into());
        }
        self.cost = Some(layer.iter().zip(&self.area).map(|(c, a)| c * a).collect());
        self.cost_layer = Some(layer);
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
        out.extend(self.linear.iter().map(|b| match &b.intactness {
            Some(fp) if b.exact => self.intactness_total(fp, x).map_or(f64::NAN, |t| t - b.t0),
            _ => dot(&b.coef, x),
        }));
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
        let all = vec![true; self.targets.len()];
        Ok(self.steps_levels(steps, &[1.0], &all)?.pop().unwrap())
    }

    /// plangea's procedure over the `levels` of a trade-off curve (shares of the `scaled` target rows, plangea's
    /// gradient steps): the parts follow one another and the plan of every level is kept; the other rows hold for
    /// the plan so far (every part asks for its share of what is left of the row, plangea's scen_check_sr_targets).
    pub fn steps_levels(
        &self,
        steps: usize,
        levels: &[f64],
        scaled: &[bool],
    ) -> std::result::Result<Vec<Vec<f64>>, String> {
        if steps < 1 {
            return Err("steps must be at least 1".into());
        }
        if self.budget.is_some() {
            return Err("the method steps takes area targets only (plangea's procedure): no budget".into());
        }
        if self.lower.as_ref().is_some_and(|l| l.iter().any(|v| *v != 0.0)) {
            return Err("the method steps takes no least shares".into());
        }
        if self.targets.is_empty() {
            return Err("the method steps needs an area target".into());
        }
        let n = self.n();
        let mut cum = vec![0.0; n];
        let mut ub = self.upper.clone();
        let mut risk_layers: Vec<Vec<f64>> = self.risks.iter().map(|b| self.steps_layer(b, &cum)).collect();
        let mut lin_layers: Vec<Vec<f64>> = self.linear.iter().map(|b| b.layer.clone()).collect();
        let sf_r: Vec<f64> = risk_layers.iter().map(|l| scale(l)).collect();
        let sf_l: Vec<f64> = lin_layers.iter().map(|l| scale(l)).collect();
        let denom: Vec<f64> = match &self.cost_layer {
            Some(c) => {
                let s = scale(c);
                c.iter().map(|v| v * s).collect()
            }
            None => vec![1.0; n],
        };
        let (mut out, mut prev) = (Vec::new(), 0.0);
        for (li, &level) in levels.iter().enumerate() {
            let inc = level - prev;
            prev = level;
            for k in 0..steps {
                let rows: Vec<Target> = self
                    .targets
                    .iter()
                    .zip(scaled)
                    .map(|(t, &sc)| {
                        let rhs = if sc {
                            t.rhs * inc / steps as f64
                        } else {
                            let done: f64 = t.coef.iter().zip(&cum).map(|(a, x)| a * x).sum();
                            (t.rhs / steps as f64).min(t.rhs - done).max(0.0)
                        };
                        Target { coef: t.coef.clone(), rhs, sense: t.sense.clone() }
                    })
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
                if steps > 1 && !(li + 1 == levels.len() && k + 1 == steps) {
                    risk_layers = self.risks.iter().map(|b| self.steps_layer(b, &cum)).collect();
                    for (l, b) in lin_layers.iter_mut().zip(&self.linear) {
                        if let Some(fp) = &b.intactness
                            && cum.iter().any(|v| *v != 0.0)
                        {
                            *l = self.intactness_coef(fp, &cum)?;
                        }
                    }
                }
            }
            out.push(cum.iter().zip(&self.upper).map(|(c, u)| c.min(*u)).collect());
        }
        Ok(out)
    }

    /// The plans for a growing target (plangea's gradient steps): the landscape targets (`scaled`) at every level;
    /// exact: nested (every plan contains the previous, the best given it) or the frontier (each the optimum);
    /// steps: plangea's procedure. With the gradient: per unit the levels whose plan acts on it (above `threshold`;
    /// for conversion the levels plus one less that).
    #[allow(clippy::too_many_arguments)]
    pub fn tradeoff(
        &self,
        levels: &[f64],
        scaled: &[bool],
        exact: bool,
        nested: bool,
        steps: usize,
        tol: f64,
        threshold: f64,
    ) -> std::result::Result<(Vec<Vec<f64>>, Vec<f64>), String> {
        if levels.is_empty()
            || levels.iter().any(|l| !(*l > 0.0 && *l <= 1.0))
            || levels.windows(2).any(|w| w[1] <= w[0])
        {
            return Err("levels: increasing shares in (0, 1]".into());
        }
        if !scaled.iter().any(|s| *s) {
            return Err("the trade-off needs a target to grow (a landscape area target)".into());
        }
        let plans = if exact {
            let mut out: Vec<Vec<f64>> = Vec::new();
            for &level in levels {
                let mut q = self.clone();
                for (t, &sc) in q.targets.iter_mut().zip(scaled) {
                    if sc {
                        t.rhs *= level;
                    }
                }
                if nested && let Some(prev) = out.last() {
                    let lb = self.lower.clone().unwrap_or_else(|| vec![0.0; self.n()]);
                    q.lower = Some(prev.iter().zip(&lb).map(|(a, b)| a.max(*b)).collect());
                }
                out.push(q.exact(tol, f64::INFINITY)?.0);
            }
            out
        } else {
            self.steps_levels(steps, levels, scaled)?
        };
        let gradient = (0..self.n())
            .map(|i| {
                let count = plans.iter().filter(|x| x[i] > threshold).count() as f64;
                if self.action[i] == 2 { levels.len() as f64 + 1.0 - count } else { count }
            })
            .collect();
        Ok((plans, gradient))
    }

    /// Projected changes of natural land per region (plangea's econometric targets): positive, land spared for
    /// restoration; negative, a demand for land to convert. A plan that restores and converts: the net change, and
    /// with `restrict` every region acts in the projected direction only. A plan that only restores: with
    /// `compensate` (plangea's harmonize_sr_sparable) the land demanded elsewhere is taken from the restoration.
    /// A plan that only converts converts what the regions demand.
    pub fn add_projected_change(
        &mut self,
        change: &[(f64, f64)],
        regions: &[f64],
        sense: &str,
        restrict: bool,
        compensate: bool,
    ) -> std::result::Result<(), String> {
        let (restore, conserve, convert) =
            (self.action.contains(&0), self.action.contains(&1), self.action.contains(&2));
        if conserve {
            return Err("projected changes of natural land are for plans that restore or convert".into());
        }
        if change.iter().any(|(_, c)| !c.is_finite()) {
            return Err("change: NaN or infinite".into());
        }
        let code_of: Vec<f64> = self.index.iter().map(|&c| regions[c]).collect();
        let targets: Vec<(Option<f64>, f64)> = if restore && convert {
            if restrict {
                for &(code, c) in change {
                    for (i, &k) in code_of.iter().enumerate() {
                        let shut = if c > 0.0 {
                            self.action[i] == 2
                        } else if c < 0.0 {
                            self.action[i] == 0
                        } else {
                            true
                        };
                        if k == code && shut {
                            self.upper[i] = 0.0;
                        }
                    }
                }
            }
            change.iter().map(|&(k, c)| (Some(k), c)).collect()
        } else if restore {
            let spared: f64 = change.iter().filter(|(_, c)| *c > 0.0).map(|(_, c)| c).sum();
            let net = change.iter().map(|(_, c)| c).sum::<f64>().max(0.0);
            change
                .iter()
                .map(|&(k, c)| {
                    let t = if c > 0.0 {
                        if compensate { c * if spared > 0.0 { net / spared } else { 0.0 } } else { c }
                    } else {
                        0.0
                    };
                    (Some(k), t)
                })
                .collect()
        } else {
            change.iter().map(|&(k, c)| (Some(k), if c < 0.0 { -c } else { 0.0 })).collect()
        };
        self.add_area_target(&targets, Some(regions), sense, false)
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
    /// With the exact intactness: successive concave minorants (`mm`).
    pub fn exact(&self, tol: f64, time_limit: f64) -> std::result::Result<(Vec<f64>, f64, String, usize), String> {
        if self.linear.iter().any(|b| b.exact) { self.mm(tol, time_limit) } else { self.sd(tol, time_limit, None) }
    }

    /// Simplicial decomposition (`exact`), with `extra` benefits: linear and capped-linear terms (lifted from the
    /// start) added to the problem's own (the exact intactness's minorants and bounds).
    fn sd(
        &self,
        tol: f64,
        time_limit: f64,
        extra: Option<&Capped>,
    ) -> std::result::Result<(Vec<f64>, f64, String, usize), String> {
        let t0 = std::time::Instant::now();
        let left = || time_limit - t0.elapsed().as_secs_f64();
        let n = self.n();
        let risks = &self.risks;
        let mut lin = vec![0.0; n];
        for b in self.linear.iter().filter(|b| !b.exact) {
            for (o, c) in lin.iter_mut().zip(&b.coef) {
                *o += b.weight * c;
            }
        }
        if let Some(e) = extra {
            for (o, c) in lin.iter_mut().zip(&e.lin) {
                *o += c;
            }
        }
        let m_c = extra.map_or(0, |e| e.a.nrow);
        let rows = self.feasible_set();
        let lb_x = self.lower.clone().unwrap_or_else(|| vec![0.0; n]);
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
        // the least habitat gained: below 0 for species that conversion takes habitat from
        let floor_y: Vec<Vec<f64>> = risks
            .iter()
            .map(|b| {
                (0..b.base.len())
                    .map(|s| if b.gain.row(s).1.iter().any(|v| *v < 0.0) { -b.base[s] } else { 0.0 })
                    .collect()
            })
            .collect();
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
                        lb.push(floor_y[i][s]);
                        ub.push(caps[i][s]);
                    }
                }
                // y_e - A_e x <= 0 for the capped terms, lo_e <= y_e <= cap_e
                if let Some(e) = extra {
                    for k in 0..e.a.nrow {
                        let (idx, v) = e.a.row(k);
                        g_rows.index.extend_from_slice(idx);
                        g_rows.value.extend_from_slice(v);
                        g_rows.start.push(g_rows.index.len());
                        g_rows.nrow += 1;
                    }
                    obj.extend_from_slice(&e.coef);
                    lb.extend_from_slice(&e.lo);
                    ub.extend_from_slice(&e.cap);
                }
                let extra = if g_rows.nrow > 0 { Some((&g_rows, n)) } else { None };
                let (x, ok) = Self::lp(&rows, extra, &obj, &lb, &ub, left().max(1e-3))?;
                Ok(((0..n).map(|j| x[j].clamp(lb_x[j], self.upper[j])).collect(), ok))
            };
        // the vertices, their gains per risk benefit (species x vertices) and linear benefit
        let mut verts: Vec<Vec<f64>> = Vec::new();
        let mut gv: Vec<Vec<Vec<f64>>> = risks.iter().map(|_| Vec::new()).collect(); // [risk][vertex][species]
        let mut lv: Vec<f64> = Vec::new();
        let mut yc: Vec<Vec<f64>> = Vec::new(); // the capped terms' y of every vertex: min(A v, cap)
        let add_vertex = |v: Vec<f64>,
                          verts: &mut Vec<Vec<f64>>,
                          gv: &mut Vec<Vec<Vec<f64>>>,
                          lv: &mut Vec<f64>,
                          yc: &mut Vec<Vec<f64>>,
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
            let y = extra.map_or(Vec::new(), |e| e.y(&v));
            lv.push(dot(&lin, &v) + extra.map_or(0.0, |e| dot(&e.coef, &y)));
            yc.push(y);
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
        add_vertex(v0, &mut verts, &mut gv, &mut lv, &mut yc, &mut lifted);
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
            let fx = self.objective(&x) + extra.map_or(0.0, |e| e.value(&x));
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
                    .sum::<f64>()
                    + extra.map_or(0.0, |e| {
                        let y = e.y(&v);
                        (0..m_c)
                            .map(|t| e.coef[t] * (y[t] - yc.iter().zip(&lam).map(|(y, l)| y[t] * l).sum::<f64>()))
                            .sum()
                    });
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
                yc = keep.iter().map(|&j| yc[j].clone()).collect();
                let total: f64 = keep.iter().map(|&j| lam[j]).sum();
                lam = keep.iter().map(|&j| lam[j] / total).collect();
            }
            add_vertex(v, &mut verts, &mut gv, &mut lv, &mut yc, &mut lifted);
            lam.push(0.0);
        }
        Ok((best_x, bound, status, it))
    }

    /// The exact intactness (landschaft's _solve_mm). T(x), the sum over the windows' pairs of c_jk
    /// exp(gamma z (t_j + t_k)), t the footprint's reduction, is convex in t, and t_i(x) = min(a_i x, h0_i) concave
    /// in x. At a plan x0 the tangent of the exponentials gives a concave minorant that touches T at x0; each step
    /// maximises the other benefits plus it exactly (simplicial decomposition), so the benefit never falls; the
    /// steps stop when it no longer rises. The bound: the secant of every pair over the reductions the plans can
    /// reach is a concave majorant, and the most of the other benefits plus it bounds the best plan. Optimal when the
    /// two meet within `tol`, else local.
    fn mm(&self, tol: f64, time_limit: f64) -> std::result::Result<(Vec<f64>, f64, String, usize), String> {
        let t0 = std::time::Instant::now();
        let left = || (time_limit - t0.elapsed().as_secs_f64()).max(1e-3);
        let n = self.n();
        let exact: Vec<(&Linear, &Footprint)> =
            self.linear.iter().filter(|b| b.exact).filter_map(|b| b.intactness.as_ref().map(|fp| (b, fp))).collect();
        let mut q = self.clone();
        q.linear.retain(|b| !b.exact);
        // the derivative of the total intactness in every cell's reduction
        let t_gradient = |fp: &Footprint, x: &[f64]| -> std::result::Result<Vec<f64>, String> {
            let h = self.footprint_at(fp, x);
            let g = self.intactness_gradient(fp, &h, &vec![-1.0; h.len()])?;
            Ok(g.into_iter().map(|v| -v).collect())
        };
        // the first minorant at the plan of nothing (a point of tangency need not be a feasible plan)
        let lb = self.lower.clone().unwrap_or_else(|| vec![0.0; n]);
        let mut best_x: Vec<f64> = lb.iter().zip(&self.upper).map(|(l, u)| 0f64.max(*l).min(*u)).collect();
        let mut best = f64::NEG_INFINITY;
        let (mut status, mut it) = ("local".to_string(), 0);
        for round in 1..=100 {
            it = round;
            let mut parts = Vec::new();
            for (b, fp) in &exact {
                let g: Vec<f64> = t_gradient(fp, &best_x)?.iter().map(|v| b.weight * v).collect();
                parts.push(self.capped_terms(fp, &g));
            }
            let (x1, _, st, _) = q.sd(tol, left(), Some(&Capped::combine(parts, n)))?;
            let f1 = self.objective(&x1);
            if st == "time_limit" {
                if f1 > best {
                    (best_x, best) = (x1, f1);
                }
                status = "time_limit".into();
                break;
            }
            if f1 <= best + tol * best.abs().max(1.0) {
                if f1 > best {
                    (best_x, best) = (x1, f1);
                }
                break;
            }
            (best_x, best) = (x1, f1);
        }
        // the bound: the other benefits plus the secants
        let (ny, nx) = self.shape;
        let mut parts = Vec::new();
        let mut constant = 0.0;
        for (b, fp) in &exact {
            let (cells, _, lo, hi) = self.reduction_terms(fp);
            let (mut tlo, mut thi) = (vec![0.0; ny * nx], vec![0.0; ny * nx]);
            let at_most = |v: f64, h: f64| if h.is_nan() { f64::NAN } else { v.min(h) };
            for (e, &c) in cells.iter().enumerate() {
                tlo[c] = at_most(lo[e], fp.h0[c]);
                thi[c] = at_most(hi[e], fp.h0[c]);
            }
            let s = ffi::rest_intactness_secant(
                &fp.h0, &tlo, &thi, ny as i64, nx as i64, fp.gamma, fp.beta, fp.z, fp.radius, fp.cell.0, fp.cell.1,
            )
            .map_err(|e| e.to_string())?;
            let sigma: Vec<f64> = s.sigma.iter().map(|v| b.weight * v).collect();
            parts.push(self.capped_terms(fp, &sigma));
            constant += b.weight * (s.u0 - b.t0);
        }
        let (_, ub, ub_status, _) = q.sd(tol, left(), Some(&Capped::combine(parts, n)))?;
        let bound = if ub_status == "optimal" { ub + constant } else { f64::INFINITY };
        if status != "time_limit" && bound - best <= tol * bound.abs().max(1.0) {
            status = "optimal".into();
        }
        Ok((best_x, bound.max(best), status, it))
    }

    /// The cheapest plan whose weighted benefit reaches `target` (landschaft's _solve_min_cost), its cost the
    /// problem's (add_budget's, or `cost` per unit share): the most benefit for a budget, F(B), is concave and
    /// increasing in the budget, so a search on the budget, each step the most benefit for it, finds the least
    /// budget that reaches the benefit; a budget whose bound falls short proves the cost above it (the bound
    /// returned). The budgets tried: regula falsi between the last budget short of the benefit and the cheapest
    /// that reaches it, with Illinois' modification, kept off the ends, and the middle when two steps did not halve
    /// the bracket.
    pub fn min_cost(
        &self,
        target: f64,
        tol: f64,
        time_limit: f64,
    ) -> std::result::Result<(Vec<f64>, f64, String, usize), String> {
        let cost = self.cost.clone().ok_or("the minimum cost needs a cost")?;
        let t0 = std::time::Instant::now();
        let left = || time_limit - t0.elapsed().as_secs_f64();
        let mut q = self.clone();
        let mut most = |budget: Option<f64>| -> std::result::Result<Option<(Vec<f64>, f64)>, String> {
            q.budget = budget;
            match q.exact(tol, left().max(1e-3)) {
                Ok((x, bound, _, _)) => Ok(Some((x, bound))),
                Err(e) if e.contains("infeasible") => Ok(None),
                Err(e) => Err(e),
            }
        };
        // the most benefit at all (within the problem's own budget)
        let top = most(self.budget)?
            .filter(|t| t.1 >= target)
            .ok_or("the restoration problem is infeasible: the benefit asked for cannot be reached")?;
        let (mut hi, mut lo, mut proven) = (dot(&cost, &top.0), 0.0, 0.0);
        let f_hi = self.objective(&top.0);
        // the best plan found falls short within the tolerance
        let mut best_x = if f_hi < target { None } else { Some(top.0) };
        // regula falsi on g(B) = F(B) - benefit with Illinois' modification: g at the end kept twice in a row is
        // halved, so the steps close in on the root from both sides
        let (mut g_lo, mut g_hi): (Option<f64>, f64) = (None, f_hi - target);
        let mut widths: Vec<f64> = Vec::new();
        let (mut status, mut it, mut kept) = ("time_limit", 1, 0i32);
        while best_x.is_some() && left() > 0.0 && it < 200 {
            let width = hi - lo;
            if width <= tol * hi.abs().max(1.0) {
                status = "optimal";
                break;
            }
            it += 1;
            let margin = 0.01 * width;
            let cand = match g_lo {
                Some(gl) if g_hi > gl && !(widths.len() >= 2 && width > 0.5 * widths[widths.len() - 2]) => {
                    (lo - gl * width / (g_hi - gl)).max(lo + margin).min(hi - margin)
                }
                _ => 0.5 * (lo + hi),
            };
            widths.push(width);
            let r = most(Some(cand))?;
            let f = r.as_ref().map_or(f64::NEG_INFINITY, |r| self.objective(&r.0));
            match r {
                Some((x, _)) if f >= target => {
                    hi = cand.min(dot(&cost, &x));
                    best_x = Some(x);
                    g_hi = f - target;
                    kept = if kept < 0 { kept - 1 } else { -1 };
                    if kept <= -2
                        && let Some(g) = g_lo.as_mut()
                    {
                        *g *= 0.5;
                    }
                }
                _ => {
                    lo = cand;
                    g_lo = r.as_ref().map(|_| f - target);
                    kept = if kept > 0 { kept + 1 } else { 1 };
                    if kept >= 2 {
                        g_hi *= 0.5;
                    }
                    // proven: no plan within cand reaches the benefit
                    if r.as_ref().is_none_or(|r| r.1 < target) {
                        proven = cand;
                    }
                }
            }
        }
        if status != "optimal" && left() > 0.0 {
            status = "iteration_limit";
        }
        let best_x = best_x.ok_or_else(|| format!("no plan reaches the benefit within the tolerance ({status})"))?;
        Ok((best_x, proven, status.into(), it))
    }

    /// The benefits' weights, the risks then the linear ones.
    pub fn weights(&self) -> Vec<f64> {
        self.risks.iter().map(|b| b.weight).chain(self.linear.iter().map(|b| b.weight)).collect()
    }

    /// The problem with these weights of the benefits (the risks then the linear ones).
    pub fn with_weights(&self, w: &[f64]) -> Plan {
        let mut q = self.clone();
        let nr = q.risks.len();
        for (b, w) in q.risks.iter_mut().zip(w) {
            b.weight = *w;
        }
        for (b, w) in q.linear.iter_mut().zip(&w[nr.min(w.len())..]) {
            b.weight = *w;
        }
        q
    }
}

/// plangea's choice of weights (post_choose_weights, landschaft's mean_sd): every criterion (a column of the rows)
/// rescaled to 0-1 over the alternatives (a constant one to 1), those smaller-is-better (`minus`) inverted, and
/// every alternative scored by the mean of its criteria divided by their standard deviation (good on all of them
/// at once). Returns the mean, the deviation, the score (larger is better) and the rank (1 the best, ties
/// averaged).
pub fn mean_sd(rows: &[Vec<f64>], minus: &[bool]) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let nc = minus.len();
    let col = |c: usize| rows.iter().map(move |r| r[c]);
    let lo: Vec<f64> = (0..nc).map(|c| col(c).fold(f64::INFINITY, f64::min)).collect();
    let span: Vec<f64> = (0..nc).map(|c| col(c).fold(f64::NEG_INFINITY, f64::max) - lo[c]).collect();
    let (mut mean, mut sd, mut score) = (Vec::new(), Vec::new(), Vec::new());
    for r in rows {
        let v: Vec<f64> = (0..nc)
            .map(|c| {
                let v = if span[c] > 0.0 { (r[c] - lo[c]) / span[c] } else { 1.0 };
                if minus[c] { 1.0 - v } else { v }
            })
            .collect();
        let m = v.iter().sum::<f64>() / nc as f64;
        let s = if nc > 1 { (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (nc - 1) as f64).sqrt() } else { 0.0 };
        mean.push(m);
        sd.push(s);
        score.push(if s > 0.0 {
            m / s
        } else if m > 0.0 {
            f64::INFINITY
        } else {
            0.0
        });
    }
    // the rank of -score, ties averaged (pandas' rank(method="average"))
    let rank = score
        .iter()
        .map(|s| {
            let above = score.iter().filter(|o| *o > s).count() as f64;
            let same = score.iter().filter(|o| *o == s).count() as f64;
            above + (same + 1.0) / 2.0
        })
        .collect();
    (mean, sd, score, rank)
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
        .input(PortSpec::optional("restorable", Type::Map, "the share of every cell that can be restored (anthropic land)"))
        .input(PortSpec::optional(
            "conservable",
            Type::Map,
            "the share of every cell's natural land conservation can keep (plangea's C): without the plan, threat of it is lost",
        ))
        .input(PortSpec::optional(
            "convertible",
            Type::Map,
            "the share of every cell's natural land that can be converted (plangea's D; with restorable, R + D)",
        ))
        .input(PortSpec::optional("threat", Type::Map, "conservation: the share of the conservable land lost without it (1)"))
        .input(PortSpec::optional("loss", Type::Stack, "sdm.area_of_habitat's loss (conservation, conversion)"))
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
        .input(PortSpec::optional("hfi", Type::Map, "the human footprint, for the intactness benefit"))
        .input(PortSpec::optional(
            "reduction",
            Type::Map,
            "intactness: the footprint a cell loses per unit restored",
        ))
        .input(PortSpec::optional(
            "increase",
            Type::Map,
            "intactness: the footprint a cell gains per unit of natural land converted (or lost unconserved)",
        ))
        .output(PortSpec::optional("restored", Type::Map, "the share of every cell restored"))
        .output(PortSpec::optional("conserved", Type::Map, "the share of every cell's natural land conserved"))
        .output(PortSpec::optional("converted", Type::Map, "the share of every cell's natural land converted"))
        .output(PortSpec::new(
            "summary",
            Type::Table,
            "the weighted benefit, the area (km²), the cost, the bound (exact), optimal (1), the rounds, the extinction risk's reduction and its standard deviation",
        ))
        .output(PortSpec::optional(
            "tradeoff",
            Type::Table,
            "with levels: per level its share, the landscape target, the area and the weighted benefit",
        ))
        .output(PortSpec::optional(
            "gradient",
            Type::Map,
            "with levels: the levels whose plan acts on the cell (conversion: the levels plus one less that)",
        ))
        .output(PortSpec::new(
            "benefits",
            Type::Table,
            "per benefit (the risk, the linear ones, intactness): value, weight, weighted",
        ))
        .output(PortSpec::optional(
            "weights",
            Type::Table,
            "with weights: per set the weights (w.<benefit>), the benefits, the cost, and plangea's mean, sd, \
             score and rank",
        ))
        .output(PortSpec::new(
            "species",
            Type::Table,
            "per species counted (its row in the area of habitat's table): the habitat gained, the risk without and with the plan, the reduction",
        ))
        .param(PortSpec::optional("method", Type::Text, "exact (default), steps or uniform"))
        .param(PortSpec::optional(
            "levels",
            Type::Any,
            "a trade-off curve: the landscape target's shares at every level (increasing, in (0, 1])",
        ))
        .param(PortSpec::optional("nested", Type::Bool, "exact trade-offs: every level contains the previous (true)"))
        .param(PortSpec::optional("threshold", Type::Number, "the gradient: a cell counts when its share is above (0)"))
        .param(PortSpec::optional(
            "projected_change",
            Type::Any,
            "projected changes of natural land per region ({code: km²}; + spared for restoration, - demanded)",
        ))
        .param(PortSpec::optional("change_sense", Type::Text, "the projected changes' rows (= default)"))
        .param(PortSpec::optional("restrict", Type::Bool, "restore and convert: every region in its direction only (true)"))
        .param(PortSpec::optional("compensate", Type::Bool, "restore only: the land demanded elsewhere taken from it (true)"))
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
        .param(PortSpec::optional(
            "z_sd",
            Type::Any,
            "the standard deviation of z (plangea: 0.1; one or one per species counted): the risk reduction's",
        ))
        .param(PortSpec::optional(
            "uncertainty",
            Type::Text,
            "how the species' sensitivities to z add up: correlated (default), independent or linear (plangea's)",
        ))
        .param(PortSpec::optional(
            "intactness_weight",
            Type::Number,
            "the intactness benefit's weight (1; with hfi)",
        ))
        .param(PortSpec::optional(
            "intactness_form",
            Type::Text,
            "linear (default: its gradient at the plan of nothing, recomputed between steps) or exact (the total \
             intactness)",
        ))
        .param(PortSpec::optional("radius", Type::Integer, "intactness: the neighbourhood's radius in cells (3)"))
        .param(PortSpec::optional("gamma", Type::Number, "intactness: the footprint's decay of quality (-ln(0.2)/4)"))
        .param(PortSpec::optional("beta", Type::Number, "intactness: the distance's decay per km (-0.2)"))
        .param(PortSpec::optional("intactness_z", Type::Number, "intactness: the exponent of the pairs' quality (0.5)"))
        .param(PortSpec::optional(
            "cell_size",
            Type::Any,
            "intactness: a cell's size in km, or [width, height] (default: the grid's, projected)",
        ))
        .param(PortSpec::optional(
            "min_benefit",
            Type::Number,
            "the cheapest plan whose weighted benefit reaches this (with cost; exact)",
        ))
        .param(PortSpec::optional(
            "weights",
            Type::Any,
            "sets of weights to compare ([{benefit: weight}]; the benefits extinction_risk, the linear layers' \
             names, intactness): the plan of the best ranked",
        ))
        .param(PortSpec::optional("tolerance", Type::Number, "exact: the relative gap to the bound (1e-7)"))
        .param(PortSpec::optional("time_limit", Type::Number, "exact: seconds (default none)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let restorable = args.opt_map("restorable")?;
        let conservable = args.opt_map("conservable")?;
        let convertible = args.opt_map("convertible")?;
        let first = restorable
            .or(conservable)
            .or(convertible)
            .ok_or_else(|| err("give the land that can be restored (restorable), conserved or converted".into()))?;
        let grid = first.grid.clone();
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
        let threat = match args.opt_map("threat")? {
            Some(m) => m.data.to_vec(),
            None => vec![1.0; ncell],
        };
        let data = |m: Option<&Map>| -> Result<Option<Vec<f64>>> {
            match m {
                Some(m) if m.data.len() != ncell => Err(err("the shares must be on one grid".into())),
                Some(m) => Ok(Some(m.data.to_vec())),
                None => Ok(None),
            }
        };
        let (r, c, v) = (data(restorable)?, data(conservable)?, data(convertible)?);
        let mut p = Plan::with_actions(
            r.as_deref(),
            c.as_deref(),
            v.as_deref(),
            &limit,
            &threat,
            &area,
            (grid.height, grid.width),
        )
        .map_err(err)?;
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
        if let Some(Value::Table(t)) = args.get("table") {
            let col = |c: &str| t.column(c).ok_or_else(|| err(format!("the table needs a column {c}")));
            let (cur, pot, valid) = (col("current")?, col("potential")?, col("valid")?);
            let nspecies = cur.len();
            let layers_of = |port: &str| -> Result<Vec<&[f64]>> {
                match args.get(port) {
                    Some(Value::Stack(g)) => {
                        if g.layers.len() != nspecies || g.layers.iter().any(|l| l.len() != ncell) {
                            return Err(err(format!("the {port} needs a layer per species of the table, on the grid")));
                        }
                        Ok(g.layers.iter().map(|l| l.as_slice()).collect())
                    }
                    _ => Ok(Vec::new()),
                }
            };
            let (layers, loss) = (layers_of("gain")?, layers_of("loss")?);
            p.add_risk_loss(
                &cur,
                &pot,
                &valid.iter().map(|v| *v != 0.0).collect::<Vec<_>>(),
                &layers,
                &loss,
                &per_species("z", PLANGEA_Z, nspecies)?,
                &per_species("restoration_discount", 0.0, nspecies)?,
                args.param("weight", 1.0)?,
            )
            .map_err(err)?;
        }
        // the benefits' names, for the sets of weights
        let mut names: Vec<String> = Vec::new();
        if !p.risks.is_empty() {
            names.push("extinction_risk".into());
        }
        if let Some(Value::Stack(l)) = args.get("linear") {
            let w: Vec<f64> = args.param("linear_weights", vec![1.0; l.layers.len()])?;
            if w.len() != l.layers.len() {
                return Err(err("linear_weights: one per linear benefit".into()));
            }
            for (k, (layer, w)) in l.layers.iter().zip(w).enumerate() {
                p.add_linear(layer, w).map_err(err)?;
                names.push(match l.keys.get(k) {
                    Some(crate::Key::Name(n)) if !names.contains(n) => n.clone(),
                    _ => format!("linear_{}", k + 1),
                });
            }
        }
        if let Some(hfi) = args.opt_map("hfi")? {
            if hfi.data.len() != ncell {
                return Err(err("hfi: not on the grid of the shares".into()));
            }
            let form: String = args.param("intactness_form", "linear".to_string())?;
            if !matches!(form.as_str(), "linear" | "exact") {
                return Err(err(format!("intactness_form: linear or exact, not {form:?}")));
            }
            let radius: i64 = args.param("radius", 3)?;
            let radius = i32::try_from(radius)
                .ok()
                .filter(|r| *r >= 0)
                .ok_or_else(|| err("radius: a number of cells, 0 or more".into()))?;
            let reduction = args.opt_map("reduction")?.map(|m| m.data.to_vec());
            let increase = args.opt_map("increase")?.map(|m| m.data.to_vec());
            p.add_intactness(
                &hfi.data,
                reduction.as_deref(),
                increase.as_deref(),
                radius,
                args.param("intactness_weight", 1.0)?,
                (
                    args.param("gamma", crate::restoration::intactness_gamma())?,
                    args.param("beta", -0.2)?,
                    args.param("intactness_z", 0.5)?,
                ),
                crate::restoration::cell_km(args, &hfi.grid)?,
                form == "exact",
            )
            .map_err(err)?;
            names.push("intactness".into());
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
        let min_benefit: Option<f64> = args.opt_param("min_benefit")?;
        if let Some(b) = args.opt_param::<f64>("budget")? {
            let cost = args.map("cost")?;
            p.add_budget(b, &cost.data).map_err(err)?;
        } else if min_benefit.is_some() {
            let cost = args.opt_map("cost")?.ok_or_else(|| err("the minimum cost needs a cost".into()))?;
            p.add_cost(&cost.data).map_err(err)?;
        }
        // the landscape's targets grow along a trade-off curve; the regions' hold
        let landscape: Vec<bool> =
            p.targets.iter().map(|t| t.coef.iter().zip(&p.area).all(|(c, a)| c.abs() == *a)).collect();
        if let Some(serde_json::Value::Object(m)) = args.params.get("projected_change") {
            let regions = args.map("regions")?;
            let mut change = Vec::new();
            for (k, v) in m {
                let code: f64 = k.parse().map_err(|_| err(format!("projected_change: region {k:?} is no number")))?;
                change.push((code, v.as_f64().ok_or_else(|| err("projected_change: changes are numbers".into()))?));
            }
            let sense: String = args.param("change_sense", "=".to_string())?;
            p.add_projected_change(
                &change,
                &regions.data,
                &sense,
                args.param("restrict", true)?,
                args.param("compensate", true)?,
            )
            .map_err(err)?;
        }
        let scaled: Vec<bool> =
            landscape.iter().copied().chain(std::iter::repeat(false)).take(p.targets.len()).collect();
        let method: String = args.param("method", "exact".to_string())?;
        let levels: Option<Vec<f64>> = args.opt_param("levels")?;
        let sets: Option<Vec<serde_json::Map<String, serde_json::Value>>> = args.opt_param("weights")?;
        if min_benefit.is_some() && (method != "exact" || levels.is_some()) {
            return Err(err("min_benefit: the minimum cost is solved by the method exact, without levels".into()));
        }
        if sets.is_some() && levels.is_some() {
            return Err(err("weights and levels: one or the other".into()));
        }
        let tolerance: f64 = args.param("tolerance", 1e-7)?;
        let time_limit: f64 = args.opt_param("time_limit")?.unwrap_or(f64::INFINITY);
        let steps: usize = args.param("steps", 1)?;
        let solve = |q: &Plan| -> Result<(Vec<f64>, f64, String, usize)> {
            match method.as_str() {
                "exact" => match min_benefit {
                    Some(b) => q.min_cost(b, tolerance, time_limit).map_err(err),
                    None => q.exact(tolerance, time_limit).map_err(err),
                },
                "steps" => Ok((q.steps(steps).map_err(err)?, f64::NAN, "heuristic".into(), steps)),
                "uniform" => {
                    let a = q.targets.first().map(|t| t.rhs).ok_or_else(|| err("uniform: give the target".into()))?;
                    Ok((q.uniform(a), f64::NAN, "uniform".into(), 0))
                }
                m => Err(err(format!("method: exact, steps or uniform, not {m:?}"))),
            }
        };
        let mut curve = None;
        if let Some(levels) = &levels {
            let (plans, gradient) = p
                .tradeoff(
                    levels,
                    &scaled,
                    method != "steps",
                    args.param("nested", true)?,
                    steps,
                    tolerance,
                    args.param("threshold", 0.0)?,
                )
                .map_err(err)?;
            curve = Some((plans, gradient));
        }
        // the sets of weights (plangea's post_choose_weights): a plan each, the best ranked kept
        let mut compared = None;
        if let Some(sets) = &sets {
            if sets.is_empty() {
                return Err(err("weights: no sets".into()));
            }
            let own = p.weights();
            let (mut rows, mut plans) = (Vec::new(), Vec::new());
            for set in sets {
                let mut w = own.clone();
                for (name, v) in set {
                    let k = names
                        .iter()
                        .position(|n| n == name)
                        .ok_or_else(|| err(format!("weights: no benefit {name:?} (of {})", names.join(", "))))?;
                    w[k] = v.as_f64().ok_or_else(|| err(format!("weights: the weight of {name} is no number")))?;
                }
                let q = p.with_weights(&w);
                let r = solve(&q)?;
                let mut row = w.clone();
                row.extend(q.values(&r.0));
                row.push(p.cost.as_ref().map_or(f64::NAN, |c| dot(c, &r.0)));
                rows.push(row);
                plans.push((w, r));
            }
            let nb = names.len();
            let crit: Vec<Vec<f64>> =
                rows.iter().map(|r| r[nb..2 * nb + usize::from(p.cost.is_some())].to_vec()).collect();
            let minus: Vec<bool> = (0..crit[0].len()).map(|c| c >= nb).collect();
            let (mean, sd, score, rank) = mean_sd(&crit, &minus);
            let mut cols: Vec<String> = names.iter().map(|n| format!("w.{n}")).collect();
            cols.extend(names.iter().cloned());
            cols.extend(["cost", "mean", "sd", "score", "rank"].map(String::from));
            let mut t = Table::new(cols);
            for (k, mut r) in rows.into_iter().enumerate() {
                r.extend([mean[k], sd[k], score[k], rank[k]]);
                t.rows.push(r);
            }
            let best = (0..rank.len()).min_by(|a, b| rank[*a].total_cmp(&rank[*b])).unwrap();
            compared = Some((t, plans.swap_remove(best)));
        }
        let (x, bound, status, rounds) = match (&curve, compared.as_mut()) {
            (Some((plans, _)), _) => (plans.last().unwrap().clone(), f64::NAN, "tradeoff".into(), plans.len()),
            (None, Some((_, (w, r)))) => {
                p = p.with_weights(w);
                std::mem::take(r)
            }
            (None, None) => solve(&p)?,
        };
        // a map per action
        let maps: Vec<Option<Vec<f64>>> = (0..3u8)
            .map(|a| {
                if !p.action.contains(&a) {
                    return None;
                }
                let mut m = vec![f64::NAN; ncell];
                for (i, &c) in p.index.iter().enumerate() {
                    if p.action[i] == a {
                        m[c] = x[i];
                    }
                }
                Some(m)
            })
            .collect();
        let values = p.values(&x);
        let weights = p.weights();
        let mut benefits = Table::new(vec!["value".into(), "weight".into(), "weighted".into()]);
        benefits.rows = values.iter().zip(&weights).map(|(v, w)| vec![*v, *w, v * w]).collect();
        let total: f64 = values.iter().zip(&weights).map(|(v, w)| v * w).sum();
        let area_x: f64 = p.area.iter().zip(&x).map(|(a, x)| a * x).sum();
        let cost = p.cost.as_ref().map_or(f64::NAN, |c| c.iter().zip(&x).map(|(c, x)| c * x).sum());
        // the risk reduction (plangea's multi_exrisk_aggregate) and its deviation from z's
        let (mut reduction, mut sd) = (f64::NAN, f64::NAN);
        let mut species =
            Table::new(["species", "gained", "risk_before", "risk_after", "reduction"].map(String::from).to_vec());
        if !p.risks.is_empty() {
            let k = p.risks[0].base.len();
            let z_sd: Vec<f64> = match args.params.get("z_sd") {
                None | Some(serde_json::Value::Null) => Vec::new(),
                Some(serde_json::Value::Number(v)) => vec![v.as_f64().unwrap(); k],
                Some(v) => serde_json::from_value(v.clone()).map_err(|e| err(format!("z_sd: {e}")))?,
            };
            let how: String = args.param("uncertainty", "correlated".to_string())?;
            let rr = p.risk_reduction(0, &x, &z_sd, &how, 1e-6).map_err(err)?;
            (reduction, sd) = (rr.total, rr.sd);
            for (s, &row) in p.risks[0].species.iter().enumerate() {
                species.rows.push(vec![(row + 1) as f64, rr.gained[s], rr.before[s], rr.after[s], rr.reduction[s]]);
            }
        }
        let mut summary = Table::new(
            ["benefit", "area", "cost", "bound", "optimal", "rounds", "risk_reduction", "sd"]
                .map(String::from)
                .to_vec(),
        );
        summary.rows.push(vec![
            total,
            area_x,
            cost,
            bound,
            f64::from(u8::from(status == "optimal")),
            rounds as f64,
            reduction,
            sd,
        ]);
        let mut out = outputs([
            ("summary", Value::Table(summary)),
            ("benefits", Value::Table(benefits)),
            ("species", Value::Table(species)),
        ]);
        for (name, m) in ["restored", "conserved", "converted"].iter().zip(maps) {
            if let Some(m) = m {
                out.insert(name.to_string(), Value::Map(Map::new(grid.clone(), m)));
            }
        }
        if let Some((t, _)) = compared {
            out.insert("weights".into(), Value::Table(t));
        }
        if let (Some((plans, gradient)), Some(levels)) = (curve, levels) {
            let land = p.targets.iter().zip(&scaled).find(|(_, s)| **s).map(|(t, _)| t.rhs);
            let mut t = Table::new(["level", "target", "area", "benefit"].map(String::from).to_vec());
            t.rows = levels
                .iter()
                .zip(&plans)
                .map(|(l, x)| {
                    let area: f64 = p.area.iter().zip(x).map(|(a, x)| a * x).sum();
                    vec![*l, land.map_or(f64::NAN, |r| r * l), area, p.objective(x)]
                })
                .collect();
            let mut g = vec![f64::NAN; ncell];
            for (i, &c) in p.index.iter().enumerate() {
                g[c] = gradient[i];
            }
            out.insert("tradeoff".into(), Value::Table(t));
            out.insert("gradient".into(), Value::Map(Map::new(grid.clone(), g)));
        }
        Ok(out)
    }
}

/// plangea's multi_exrisk_aggregate: an updated habitat this close to 0 is 0.
pub const EXRISK_EPS: f64 = 1e-8;

/// The reduction of the extinction risk a plan brings (plangea's multi_exrisk_aggregate): per species its habitat
/// gained, its risk before and after; the total reduction and, with z's standard deviations, the total's.
pub struct RiskReduction {
    pub gained: Vec<f64>,
    pub before: Vec<f64>,
    pub after: Vec<f64>,
    pub reduction: Vec<f64>,
    pub total: f64,
    pub sd: f64,
}

impl Plan {
    /// The risk reduction of a plan for the risk benefit `b`; `z_sd` per species (empty: no uncertainty), how the
    /// species' sensitivities to z add up (correlated: |sum|; independent: in quadrature; linear: sum of |.|,
    /// plangea's two forms), z's forward difference.
    pub fn risk_reduction(
        &self,
        b: usize,
        x: &[f64],
        z_sd: &[f64],
        method: &str,
        step: f64,
    ) -> std::result::Result<RiskReduction, String> {
        let b = self.risks.get(b).ok_or("the problem has no such extinction-risk benefit")?;
        let gained = mv(&b.gain, &x.iter().map(|v| if v.is_nan() { 0.0 } else { *v }).collect::<Vec<_>>());
        let after: Vec<f64> = b
            .base
            .iter()
            .zip(&gained)
            .map(|(a, g)| {
                let v = a + g;
                if v.abs() < EXRISK_EPS { 0.0 } else { v }
            })
            .collect();
        let risk = |a: f64, s: usize, z: f64| (1.0 - (a / b.potential[s]).powf(z)).max(0.0);
        let reduction = |dz: f64| -> Vec<f64> {
            (0..b.base.len())
                .map(|s| {
                    let v = risk(b.base[s], s, b.z[s] + dz) - risk(after[s], s, b.z[s] + dz);
                    if v.is_nan() { 0.0 } else { v }
                })
                .collect()
        };
        let red = reduction(0.0);
        let total = red.iter().sum();
        let sd = if z_sd.is_empty() {
            f64::NAN
        } else {
            if z_sd.len() != red.len() {
                return Err(format!("z_sd: one per species counted ({})", red.len()));
            }
            let up = reduction(step);
            let d: Vec<f64> = (0..red.len()).map(|s| (up[s] - red[s]) / step * z_sd[s]).collect();
            match method {
                "correlated" => d.iter().sum::<f64>().abs(),
                "independent" => d.iter().map(|v| v * v).sum::<f64>().sqrt(),
                "linear" => d.iter().map(|v| v.abs()).sum(),
                m => return Err(format!("method: correlated, independent or linear, not {m:?}")),
            }
        };
        Ok(RiskReduction {
            before: (0..b.base.len()).map(|s| risk(b.base[s], s, b.z[s])).collect(),
            after: (0..b.base.len()).map(|s| risk(after[s], s, b.z[s])).collect(),
            gained,
            reduction: red,
            total,
            sd,
        })
    }
}

/// The ecosystems' (ecoregions') natural area now and before conversion, and what restoring a cell gives them
/// (plangea's ec): per ecoregion code its natural area (current), natural plus anthropic (potential) and cells;
/// and per ecoregion a layer of the cell's area where it has anthropic land. Every cell of the ecoregion counts
/// (plangea sums over its problem's cells only, which overstates the loss).
pub struct EcosystemAreas {
    pub codes: Vec<f64>,
    pub current: Vec<f64>,
    pub potential: Vec<f64>,
    pub cells: Vec<f64>,
    pub gain: Vec<Vec<f64>>,
}

pub fn ecosystem_areas(natural: &[f64], anthropic: &[f64], ecoregions: &[f64], area: &[f64]) -> EcosystemAreas {
    let mut codes: Vec<f64> = ecoregions.iter().copied().filter(|v| v.is_finite()).collect();
    codes.sort_by(f64::total_cmp);
    codes.dedup();
    let mut out = EcosystemAreas {
        codes: codes.clone(),
        current: Vec::new(),
        potential: Vec::new(),
        cells: Vec::new(),
        gain: Vec::new(),
    };
    for &code in &codes {
        let cells: Vec<usize> = (0..ecoregions.len()).filter(|&c| ecoregions[c] == code).collect();
        out.current.push(cells.iter().map(|&c| area[c] * natural[c]).sum());
        out.potential.push(cells.iter().map(|&c| area[c] * (natural[c] + anthropic[c])).sum());
        out.cells.push(cells.len() as f64);
        let mut g = vec![0.0; ecoregions.len()];
        for &c in &cells {
            if anthropic[c] > 0.0 {
                g[c] = area[c];
            }
        }
        out.gain.push(g);
    }
    out
}

/// Share a restoration target among ecosystems for the least risk of collapse (plangea's harmonize_sr_ecosys):
/// every ecosystem with natural area n of a potential t gets a share between its least (`minimum` of an equal
/// split, up to its converted area) and t - n, for the most total of ((n + a) / t)^z, each marginal value damped by
/// exp(half_life (n + a) / t). exact: the marginal values equal among the ecosystems off their bounds (bisection);
/// steps: plangea's procedure (the target in steps, each a linear program on the marginal values).
#[allow(clippy::too_many_arguments)]
pub fn ecosystem_targets(
    n_e: &[f64],
    t_e: &[f64],
    target: f64,
    minimum: f64,
    z: f64,
    half_life: f64,
    exact: bool,
    steps: usize,
) -> std::result::Result<Vec<f64>, String> {
    if !(0.0..=1.0).contains(&minimum) {
        return Err("minimum must be in [0, 1]".into());
    }
    if !(z > 0.0 && z < 1.0) {
        return Err("z must be in (0, 1)".into());
    }
    if half_life >= 1.0 - z {
        return Err("half_life must be below 1 - z (the marginal value must fall with the area)".into());
    }
    let k = n_e.len();
    let up: Vec<f64> = (0..k).map(|e| (t_e[e] - n_e[e]).max(0.0)).collect();
    if !exact {
        if steps < 1 {
            return Err("steps must be at least 1".into());
        }
        let values = |nat: &[f64]| -> Vec<f64> {
            (0..k)
                .map(|e| {
                    let r1 = (1.0 - (nat[e] / t_e[e]).powf(z)).max(0.0);
                    let r2 = (1.0 - ((nat[e] + PLANGEA_DELTA) / t_e[e]).powf(z)).max(0.0);
                    (r2 - r1).abs() / PLANGEA_DELTA * (half_life * nat[e] / t_e[e]).exp()
                })
                .collect()
        };
        let mut obj = values(n_e);
        let mut ub = up.clone();
        let step = target / steps as f64;
        let mut least: Vec<f64> = (0..k).map(|e| (step / k as f64 * minimum).min(ub[e])).collect();
        let mut total = vec![0.0; k];
        for _ in 0..steps {
            let mut x = least.clone();
            let mut left = step - x.iter().sum::<f64>();
            let mut order: Vec<usize> = (0..k).collect();
            order.sort_by(|&a, &b| obj[b].total_cmp(&obj[a]));
            for e in order {
                if left <= 0.0 {
                    break;
                }
                let add = (ub[e] - x[e]).min(left);
                if add > 0.0 {
                    x[e] += add;
                    left -= add;
                }
            }
            for e in 0..k {
                ub[e] = (ub[e] - x[e]).max(0.0);
                least[e] = least[e].min(ub[e]);
                total[e] += x[e];
            }
            obj = values(&n_e.iter().zip(&total).map(|(n, t)| n + t).collect::<Vec<_>>());
        }
        return Ok(total);
    }
    let least: Vec<f64> = (0..k).map(|e| (target / k as f64 * minimum).min(up[e])).collect();
    let marginal = |e: usize, a: f64| {
        let r = (n_e[e] + a) / t_e[e];
        z * r.powf(z - 1.0) * (half_life * r).exp() / t_e[e]
    };
    let share = |lam: f64| -> Vec<f64> {
        (0..k)
            .map(|e| {
                if marginal(e, up[e]) >= lam {
                    return up[e];
                }
                if marginal(e, least[e]) <= lam {
                    return least[e];
                }
                let (mut lo, mut hi) = (least[e], up[e]);
                for _ in 0..200 {
                    let mid = 0.5 * (lo + hi);
                    if marginal(e, mid) > lam {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                0.5 * (lo + hi)
            })
            .collect()
    };
    if up.iter().sum::<f64>() <= target {
        return Ok(up);
    }
    let m_least: Vec<f64> = (0..k).map(|e| marginal(e, least[e])).collect();
    let mut lam_hi = m_least.iter().map(|m| if m.is_finite() { *m } else { 0.0 }).fold(0.0, f64::max) + 1.0;
    if m_least.iter().any(|m| !m.is_finite()) {
        lam_hi = (0..k).map(|e| marginal(e, (least[e] + 1e-12 * t_e[e].max(1.0)).min(up[e]))).fold(0.0, f64::max) * 2.0
            + 1.0;
    }
    let mut lam_lo = 0.0;
    for _ in 0..200 {
        let lam = 0.5 * (lam_lo + lam_hi);
        if share(lam).iter().sum::<f64>() > target {
            lam_lo = lam;
        } else {
            lam_hi = lam;
        }
    }
    let mut a = share(lam_hi);
    // the last bit of the target to the ecosystems at the marginal value
    let free: Vec<usize> = (0..k).filter(|&e| a[e] > least[e] && a[e] < up[e]).collect();
    let rest = target - a.iter().sum::<f64>();
    if rest > 0.0 && !free.is_empty() {
        let room: f64 = free.iter().map(|&e| up[e] - a[e]).sum();
        let f = (rest / room).min(1.0);
        for &e in &free {
            a[e] += (up[e] - a[e]) * f;
        }
    }
    Ok(a)
}

struct EcosystemAreasOp;

impl Operator for EcosystemAreasOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "restoration.ecosystem_areas",
            "The ecosystems' (ecoregions') natural area now and before conversion and what restoring a cell gives \
             them (plangea's ec): restoration.plan's table and gain for the risk of ecosystem collapse. Every cell \
             of an ecoregion counts (plangea counts its problem's cells only, overstating the loss).",
            Access::Global,
        )
        .input(PortSpec::new("land_cover", Type::Stack, "the share of each class, by name"))
        .input(PortSpec::new("ecoregions", Type::Map, "ecoregion codes (no value: none)"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "per ecoregion: its code, natural area (current), natural and anthropic (potential), cells, valid",
        ))
        .output(PortSpec::new("gain", Type::Stack, "per ecoregion: the area a unit of a cell restored gives it"))
        .param(PortSpec::new("natural", Type::Any, "the natural classes"))
        .param(PortSpec::new("anthropic", Type::Any, "the anthropic classes"))
        .param(PortSpec::optional("cell_area", Type::Number, "a cell's area in km² (default: the grid's)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let cover = args.stack("land_cover")?;
        let eco = args.map("ecoregions")?;
        let n = cover.grid.width * cover.grid.height;
        if eco.data.len() != n {
            return Err(err("the ecoregions are not on the land cover's grid".into()));
        }
        let sum = |param: &str| -> Result<Vec<f64>> {
            let names: Vec<String> = args.param(param, Vec::new())?;
            let mut out = vec![0.0; n];
            for c in &names {
                let k = cover
                    .keys
                    .iter()
                    .position(|key| &key.to_string() == c)
                    .ok_or_else(|| err(format!("the land cover has no layer {c:?}")))?;
                for (o, v) in out.iter_mut().zip(cover.layers[k].iter()) {
                    *o += if v.is_nan() { 0.0 } else { *v };
                }
            }
            Ok(out)
        };
        let (nat, ant) = (sum("natural")?, sum("anthropic")?);
        let area: Vec<f64> = match args.opt_param::<f64>("cell_area")? {
            Some(a) => vec![a; n],
            None => crate::restoration::cell_area_km2(&cover.grid)
                .ok_or_else(|| err("the grid has no coordinate system: give cell_area (km²)".into()))?,
        };
        let e = ecosystem_areas(&nat, &ant, &eco.data, &area);
        let mut table = Table::new(["code", "current", "potential", "range_cells", "valid"].map(String::from).to_vec());
        table.rows = (0..e.codes.len())
            .map(|k| {
                vec![e.codes[k], e.current[k], e.potential[k], e.cells[k], f64::from(u8::from(e.potential[k] > 0.0))]
            })
            .collect();
        let keys = e.codes.iter().map(|c| crate::Key::Name(crate::value::class_label(*c))).collect();
        let layers = e.gain.into_iter().map(Arc::new).collect();
        Ok(outputs([
            ("table", Value::Table(table)),
            ("gain", Value::Stack(crate::Stack { grid: cover.grid.clone(), keys, layers })),
        ]))
    }
}

struct EcosystemTargetsOp;

impl Operator for EcosystemTargetsOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "restoration.ecosystem_targets",
            "Share a restoration target among ecosystems for the least risk of collapse (plangea's ecosystemic \
             sub-region targets): every ecosystem between its least share and its converted area, for the most \
             total of ((natural + share) / potential)^z, damped by exp(half_life x the ratio). exact: the optimum; \
             steps: plangea's procedure. The targets per region for restoration.plan (target: {code: area}).",
            Access::Global,
        )
        .input(PortSpec::new("table", Type::Table, "restoration.ecosystem_areas's table"))
        .output(PortSpec::new("targets", Type::Table, "per ecosystem: its code (region) and its share (target)"))
        .param(PortSpec::new("target", Type::Number, "the area to share (km²)"))
        .param(PortSpec::optional("minimum", Type::Number, "every ecosystem's least share, of an equal split (0)"))
        .param(PortSpec::optional("z", Type::Number, "the species-area exponent (0.25)"))
        .param(PortSpec::optional("half_life", Type::Number, "the damping exponent (0: none; below 1 - z)"))
        .param(PortSpec::optional("method", Type::Text, "exact (default) or steps"))
        .param(PortSpec::optional("steps", Type::Integer, "steps: the parts of the target (10)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let Some(Value::Table(t)) = args.get("table") else { return Err(err("the table is needed".into())) };
        let col = |c: &str| t.column(c).ok_or_else(|| err(format!("the table needs a column {c}")));
        let (codes, cur, pot, valid) = (col("code")?, col("current")?, col("potential")?, col("valid")?);
        let keep: Vec<usize> = (0..codes.len()).filter(|&k| valid[k] != 0.0 && pot[k] > 0.0).collect();
        let pick = |v: &[f64]| keep.iter().map(|&k| v[k]).collect::<Vec<f64>>();
        let method: String = args.param("method", "exact".to_string())?;
        let a = ecosystem_targets(
            &pick(&cur),
            &pick(&pot),
            args.param("target", 0.0)?,
            args.param("minimum", 0.0)?,
            args.param("z", PLANGEA_Z)?,
            args.param("half_life", 0.0)?,
            method != "steps",
            args.param("steps", 10)?,
        )
        .map_err(err)?;
        let mut out = Table::new(vec!["region".into(), "target".into()]);
        out.rows = keep.iter().zip(&a).map(|(&k, a)| vec![codes[k], *a]).collect();
        Ok(outputs([("targets", Value::Table(out))]))
    }
}
