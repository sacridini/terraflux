//! The MILP prioritization of prioritizr 8.0.6 (Hanson et al.), as landschaft compiles it: the planning units and
//! the features' amounts in them, a problem (objective, targets, weights, decisions, locked units) and its model,
//! the model prioritizr's `compile()` gives bit for bit, solved by HiGHS (through landschaft's C++).
//!
//! Layout (one zone): the columns are the planning units, then the columns the objective adds; the rows are the
//! targets (the amounts of their feature in the units), then the rows the objective adds.

use terraflux_ls_sys::ffi;

/// The planning units: cells of a raster with a cost, and the features' amounts in them.
#[derive(Debug, Clone)]
pub struct Units {
    /// per unit
    pub cost: Vec<f64>,
    /// features x units, compressed rows without zeros
    pub rij: Csr,
    /// per feature: what relative targets are shares of
    pub totals: Vec<f64>,
    pub names: Vec<String>,
    /// raster units: the cell of each unit
    pub cells: Vec<usize>,
    /// the raster's rows and columns and its geotransform
    pub ny: usize,
    pub nx: usize,
    pub transform: [f64; 6],
}

/// A sparse matrix in compressed rows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Csr {
    pub nrow: usize,
    pub ncol: usize,
    pub start: Vec<usize>,
    pub index: Vec<usize>,
    pub value: Vec<f64>,
}

impl Csr {
    pub fn row(&self, i: usize) -> (&[usize], &[f64]) {
        (&self.index[self.start[i]..self.start[i + 1]], &self.value[self.start[i]..self.start[i + 1]])
    }

    pub fn nnz(&self) -> usize {
        self.value.len()
    }

    /// From triplets: duplicates added up in their order, zeros left out, columns in order (scipy's csr_matrix
    /// of a COO, then sort_indices).
    pub fn from_triplets(nrow: usize, ncol: usize, rows: &[usize], cols: &[usize], vals: &[f64]) -> Csr {
        let mut by_row: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nrow];
        for k in 0..vals.len() {
            by_row[rows[k]].push((cols[k], vals[k]));
        }
        let mut out = Csr { nrow, ncol, start: vec![0], index: Vec::new(), value: Vec::new() };
        for mut r in by_row {
            // a stable sort keeps the duplicates' order, which their sum follows
            r.sort_by_key(|&(c, _)| c);
            let mut i = 0;
            while i < r.len() {
                let c = r[i].0;
                let mut v = r[i].1;
                i += 1;
                while i < r.len() && r[i].0 == c {
                    v += r[i].1;
                    i += 1;
                }
                out.index.push(c);
                out.value.push(v);
            }
            out.start.push(out.index.len());
        }
        out
    }
}

/// A sum in order, in double (Rcpp's sum(), Matrix's rowSums over a row).
fn seq_sum(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |a, b| a + b)
}

impl Units {
    /// Raster units: the cells with a finite cost, the features' amounts there (no value 0) and their totals
    /// over the whole raster (terra's), through landschaft's FeatureMatrix.
    pub fn raster(
        cost: &[f64],
        features: &[&[f64]],
        names: Vec<String>,
        (ny, nx): (usize, usize),
        transform: [f64; 6],
    ) -> Result<Units, String> {
        if features.is_empty() {
            return Err("no features".into());
        }
        let n = cost.len();
        if features.iter().any(|f| f.len() != n) {
            return Err("the features and the cost must be on the same grid".into());
        }
        let cells: Vec<usize> = (0..n).filter(|&c| cost[c].is_finite()).collect();
        let mut unit = vec![-1i64; n];
        for (i, &c) in cells.iter().enumerate() {
            unit[c] = i as i64;
        }
        let values: Vec<f64> = features.iter().flat_map(|f| f.iter().copied()).collect();
        let m = ffi::opt_features(&values, features.len() as i64, &unit).map_err(|e| e.what().to_string())?;
        Ok(Units {
            cost: cells.iter().map(|&c| cost[c]).collect(),
            rij: Csr {
                nrow: features.len(),
                ncol: cells.len(),
                start: m.start.iter().map(|&s| s as usize).collect(),
                index: m.index.iter().map(|&s| s as usize).collect(),
                value: m.value,
            },
            totals: m.totals,
            names,
            cells,
            ny,
            nx,
            transform,
        })
    }

    pub fn n(&self) -> usize {
        self.cost.len()
    }
}

/// prioritizr's objectives (and their budgets).
#[derive(Debug, Clone, PartialEq)]
pub enum Objective {
    MinSet,
    MinShortfall(f64),
    MinLargestShortfall(f64),
    MaxUtility(f64),
    MaxFeatures(f64),
    MaxCover(f64),
    MinPenalties(f64),
}

impl Objective {
    pub fn name(&self) -> &'static str {
        match self {
            Objective::MinSet => "min_set",
            Objective::MinShortfall(_) => "min_shortfall",
            Objective::MinLargestShortfall(_) => "min_largest_shortfall",
            Objective::MaxUtility(_) => "max_utility",
            Objective::MaxFeatures(_) => "max_features",
            Objective::MaxCover(_) => "max_cover",
            Objective::MinPenalties(_) => "min_penalties",
        }
    }

    fn budget(&self) -> Option<f64> {
        match self {
            Objective::MinSet => None,
            Objective::MinShortfall(b)
            | Objective::MinLargestShortfall(b)
            | Objective::MaxUtility(b)
            | Objective::MaxFeatures(b)
            | Objective::MaxCover(b)
            | Objective::MinPenalties(b) => Some(*b),
        }
    }

    /// The objectives whose model has no targets.
    pub fn no_targets(&self) -> bool {
        matches!(self, Objective::MaxUtility(_) | Objective::MaxCover(_))
    }

    /// The objectives that ignore the feature weights.
    pub fn no_weights(&self) -> bool {
        matches!(self, Objective::MinSet | Objective::MinLargestShortfall(_))
    }
}

/// A target given by hand: its feature, whether relative or absolute, its sense and amount.
#[derive(Debug, Clone, PartialEq)]
pub struct ManualTarget {
    pub feature: usize,
    pub relative: bool,
    pub sense: String,
    pub target: f64,
}

/// prioritizr's targets.
#[derive(Debug, Clone, PartialEq)]
pub enum TargetSpec {
    Relative(Vec<f64>),
    Absolute(Vec<f64>),
    /// lower bound amount and target, upper bound amount and target, the cap (amount, target)
    Loglinear {
        lo_amount: f64,
        lo_target: f64,
        hi_amount: f64,
        hi_target: f64,
        cap: Option<(f64, f64)>,
    },
    Manual(Vec<ManualTarget>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decisions {
    Binary,
    Proportion,
    Semicontinuous(f64),
}

/// A problem: its units, objective, targets, weights, decisions and locked units.
#[derive(Debug, Clone)]
pub struct Problem {
    pub units: Units,
    pub objective: Objective,
    pub targets: Option<TargetSpec>,
    pub weights: Option<Vec<f64>>,
    pub decisions: Decisions,
    pub locked_in: Vec<bool>,
    pub locked_out: Vec<bool>,
    /// penalties and constraints, in the order added (prioritizr applies the penalties, then the constraints)
    pub spatial: Vec<Spatial>,
}

/// The targets as the model's rows: the feature, sense and amount of each, and the total relative targets are
/// shares of.
#[derive(Debug, Clone, PartialEq)]
pub struct Targets {
    pub feature: Vec<usize>,
    pub sense: Vec<String>,
    pub value: Vec<f64>,
    pub total: Vec<f64>,
}

/// prioritizr's loglinear_interpolation: y1 up to x1, y2 from x2, between them linear in log10(x) (R's approx()).
fn loglinear(x: &[f64], x1: f64, y1: f64, x2: f64, y2: f64) -> Result<Vec<f64>, String> {
    if x1.partial_cmp(&x2) != Some(std::cmp::Ordering::Less) {
        return Err("the lower bound amount must be below the upper bound amount".into());
    }
    let (u1, u2) = (x1.log10(), x2.log10());
    Ok(x.iter()
        .map(|&v| {
            if v <= x1 {
                y1
            } else if v >= x2 {
                y2
            } else {
                let l = v.log10();
                if l == u1 {
                    y1
                } else if l == u2 {
                    y2
                } else {
                    y1 + (y2 - y1) * ((l - u1) / (u2 - u1))
                }
            }
        })
        .collect())
}

/// The rows of a target specification.
pub fn resolve_targets(spec: &TargetSpec, totals: &[f64]) -> Result<Targets, String> {
    let nf = totals.len();
    let ge = |n: usize| vec![">=".to_string(); n];
    Ok(match spec {
        TargetSpec::Relative(r) => Targets {
            feature: (0..nf).collect(),
            sense: ge(nf),
            value: totals.iter().zip(r).map(|(t, r)| t * r).collect(),
            total: totals.to_vec(),
        },
        TargetSpec::Absolute(a) => {
            Targets { feature: (0..nf).collect(), sense: ge(nf), value: a.clone(), total: totals.to_vec() }
        }
        TargetSpec::Loglinear { lo_amount, lo_target, hi_amount, hi_target, cap } => {
            let share = loglinear(totals, *lo_amount, *lo_target, *hi_amount, *hi_target)?;
            let mut value: Vec<f64> = totals.iter().zip(&share).map(|(a, s)| a * s).collect();
            if let Some((ca, ct)) = cap {
                for (v, a) in value.iter_mut().zip(totals) {
                    if a >= ca {
                        *v = *ct;
                    }
                }
            }
            Targets { feature: (0..nf).collect(), sense: ge(nf), value, total: totals.to_vec() }
        }
        TargetSpec::Manual(t) => {
            for m in t {
                if m.feature >= nf {
                    return Err(format!("no feature {} in the targets", m.feature + 1));
                }
                if !matches!(m.sense.as_str(), ">=" | "<=" | "=") {
                    return Err("target senses are '>=', '<=' or '='".into());
                }
            }
            Targets {
                feature: t.iter().map(|m| m.feature).collect(),
                sense: t.iter().map(|m| m.sense.clone()).collect(),
                value: t.iter().map(|m| if m.relative { totals[m.feature] * m.target } else { m.target }).collect(),
                total: t.iter().map(|m| totals[m.feature]).collect(),
            }
        }
    })
}

/// A compiled model, prioritizr's compile(): min or max obj x subject to A x (sense) rhs, lb <= x <= ub, x of type
/// vtype (B binary, C continuous).
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub a: Csr,
    pub obj: Vec<f64>,
    pub lb: Vec<f64>,
    pub ub: Vec<f64>,
    pub rhs: Vec<f64>,
    pub sense: Vec<String>,
    pub vtype: Vec<char>,
    pub maximize: bool,
    pub row_ids: Vec<String>,
    pub col_ids: Vec<String>,
}

/// The model being compiled: triplets and vectors that penalties and constraints extend.
pub struct Builder {
    pub nrow: usize,
    pub ncol: usize,
    pub rows: Vec<usize>,
    pub cols: Vec<usize>,
    pub vals: Vec<f64>,
    pub obj: Vec<f64>,
    pub lb: Vec<f64>,
    pub ub: Vec<f64>,
    pub vtype: Vec<char>,
    pub sense: Vec<String>,
    pub rhs: Vec<f64>,
    pub row_ids: Vec<String>,
    pub col_ids: Vec<String>,
    pub maximize: bool,
    pub n: usize,
}

impl Builder {
    /// Entries added to the matrix.
    pub fn push(
        &mut self,
        r: impl IntoIterator<Item = usize>,
        c: impl IntoIterator<Item = usize>,
        v: impl IntoIterator<Item = f64>,
    ) {
        self.rows.extend(r);
        self.cols.extend(c);
        self.vals.extend(v);
    }

    /// The model: the matrix without the zeros prioritizr stores.
    pub fn model(self) -> Model {
        let keep: Vec<usize> = (0..self.vals.len()).filter(|&k| self.vals[k] != 0.0).collect();
        let r: Vec<usize> = keep.iter().map(|&k| self.rows[k]).collect();
        let c: Vec<usize> = keep.iter().map(|&k| self.cols[k]).collect();
        let v: Vec<f64> = keep.iter().map(|&k| self.vals[k]).collect();
        Model {
            a: Csr::from_triplets(self.nrow, self.ncol, &r, &c, &v),
            obj: self.obj,
            lb: self.lb,
            ub: self.ub,
            rhs: self.rhs,
            sense: self.sense,
            vtype: self.vtype,
            maximize: self.maximize,
            row_ids: self.row_ids,
            col_ids: self.col_ids,
        }
    }
}

/// The model prioritizr's compile() gives for a problem, and its targets.
pub fn compile(p: &Problem) -> Result<(Model, Targets), String> {
    let u = &p.units;
    let nf = u.names.len();
    let n = u.n();
    let kind = &p.objective;
    let t = if kind.no_targets() {
        // prioritizr's "fake" targets: one per feature
        Targets {
            feature: (0..nf).collect(),
            sense: vec!["?".into(); nf],
            value: vec![0.0; nf],
            total: u.totals.clone(),
        }
    } else {
        let spec = p.targets.as_ref().ok_or_else(|| {
            format!("the {} objective needs targets (relative, absolute, loglinear or manual)", kind.name())
        })?;
        resolve_targets(spec, &u.totals)?
    };
    let nt = t.feature.len();
    let cost = &u.cost;
    let mut lb = vec![0.0; n];
    let (mut ub, mut vtype) = match p.decisions {
        Decisions::Binary => (vec![1.0; n], vec!['B'; n]),
        Decisions::Proportion => (vec![1.0; n], vec!['C'; n]),
        Decisions::Semicontinuous(limit) => (vec![limit; n], vec!['C'; n]),
    };
    // the targets' rows: their features' amounts in the units
    let (mut rows, mut cols, mut vals) = (Vec::new(), Vec::new(), Vec::new());
    for (i, &f) in t.feature.iter().enumerate() {
        let (idx, v) = u.rij.row(f);
        rows.extend(std::iter::repeat_n(i, idx.len()));
        cols.extend_from_slice(idx);
        vals.extend_from_slice(v);
    }
    let mut obj = vec![0.0; n];
    let mut col_ids = vec!["pu".to_string(); n];
    let row_ids: Vec<String>;
    let sense: Vec<String>;
    let rhs: Vec<f64>;
    let budget = kind.budget();
    let (mut nrow_extra, mut ncol_extra, mut weight_cols) = (0usize, 0usize, None);
    let bud_sense = || if budget.is_some() { vec!["<=".to_string()] } else { Vec::new() };
    let bud_ids = || if budget.is_some() { vec!["budget".to_string()] } else { Vec::new() };
    let budget_row = |rows: &mut Vec<usize>, cols: &mut Vec<usize>, vals: &mut Vec<f64>, row: usize| {
        rows.extend(std::iter::repeat_n(row, n));
        cols.extend(0..n);
        vals.extend_from_slice(cost);
    };
    let inv_target = |v: f64| if v > 1.0e-5 { 1.0 / v } else { 0.0 };
    match kind {
        Objective::MinSet => {
            obj = cost.clone();
            sense = t.sense.clone();
            rhs = t.value.clone();
            row_ids = vec!["spp_target".into(); nt];
        }
        Objective::MinShortfall(b) => {
            sense = [t.sense.clone(), bud_sense()].concat();
            rhs = [t.value.clone(), vec![*b]].concat();
            obj.extend(t.value.iter().map(|&v| inv_target(v)));
            lb.extend(vec![0.0; nt]);
            ub.extend(vec![f64::INFINITY; nt]);
            vtype.extend(vec!['C'; nt]);
            rows.extend(0..nt);
            cols.extend(n..n + nt);
            vals.extend(vec![1.0; nt]);
            budget_row(&mut rows, &mut cols, &mut vals, nt);
            col_ids.extend(vec!["spp_met".to_string(); nt]);
            row_ids = [vec!["spp_target".to_string(); nt], bud_ids()].concat();
            nrow_extra = 1;
            ncol_extra = nt;
            weight_cols = Some(n);
        }
        Objective::MinLargestShortfall(b) => {
            sense = [t.sense.clone(), vec![">=".to_string(); nt], bud_sense()].concat();
            rhs = [t.value.clone(), vec![0.0; nt], vec![*b]].concat();
            obj.extend(vec![0.0; nt]);
            obj.push(1.0);
            lb.extend(vec![0.0; nt + 1]);
            ub.extend(vec![f64::INFINITY; nt + 1]);
            vtype.extend(vec!['C'; nt + 1]);
            rows.extend(0..nt);
            cols.extend(n..n + nt);
            vals.extend(vec![1.0; nt]);
            rows.extend(nt..2 * nt);
            cols.extend(n..n + nt);
            vals.extend(t.value.iter().map(|&v| if v > 1.0e-5 { -1.0 / v } else { 0.0 }));
            rows.extend(nt..2 * nt);
            cols.extend(std::iter::repeat_n(n + nt, nt));
            vals.extend(vec![1.0; nt]);
            budget_row(&mut rows, &mut cols, &mut vals, 2 * nt);
            col_ids.extend(vec!["spp_met".to_string(); nt]);
            col_ids.push("max_shortfall".into());
            row_ids = [vec!["spp_target".to_string(); nt], vec!["max_shortfall".to_string(); nt], bud_ids()].concat();
            nrow_extra = nt + 1;
            ncol_extra = nt + 1;
        }
        Objective::MaxUtility(b) | Objective::MaxCover(b) | Objective::MaxFeatures(b) => {
            let scale = -0.01 / seq_sum(cost);
            obj = cost.iter().map(|c| c * scale).collect();
            obj.extend(vec![1.0; nt]);
            match kind {
                Objective::MaxUtility(_) => {
                    // the positive amounts of every feature (Matrix's rowSums, in unit order)
                    let abund: Vec<f64> = (0..nf)
                        .map(|f| seq_sum(&u.rij.row(f).1.iter().map(|v| v.max(0.0)).collect::<Vec<_>>()))
                        .collect();
                    let negative = u.rij.nnz() > 0 && u.rij.value.iter().cloned().fold(f64::INFINITY, f64::min) < 0.0;
                    lb.extend(vec![if negative { f64::NEG_INFINITY } else { 0.0 }; nt]);
                    ub.extend(abund.iter().map(|a| a.max(0.0)));
                    vtype.extend(vec!['C'; nt]);
                    rows.extend(0..nt);
                    cols.extend(n..n + nt);
                    vals.extend(vec![-1.0; nt]);
                    sense = [vec!["=".to_string(); nt], bud_sense()].concat();
                    rhs = [vec![0.0; nt], vec![*b]].concat();
                    col_ids.extend(vec!["amount".to_string(); nt]);
                    row_ids = [vec!["spp_amount".to_string(); nt], bud_ids()].concat();
                }
                Objective::MaxCover(_) => {
                    lb.extend(vec![0.0; nt]);
                    ub.extend(vec![1.0; nt]);
                    vtype.extend(vec!['B'; nt]);
                    rows.extend(0..nt);
                    cols.extend(n..n + nt);
                    vals.extend(vec![-1.0; nt]);
                    sense = [vec![">=".to_string(); nt], bud_sense()].concat();
                    rhs = [vec![0.0; nt], vec![*b]].concat();
                    col_ids.extend(vec!["present".to_string(); nt]);
                    row_ids = [vec!["spp_present".to_string(); nt], bud_ids()].concat();
                }
                _ => {
                    lb.extend(vec![0.0; nt]);
                    ub.extend(vec![1.0; nt]);
                    vtype.extend(vec!['B'; nt]);
                    rows.extend(0..nt);
                    cols.extend(n..n + nt);
                    vals.extend(t.value.iter().map(|v| -v));
                    sense = [t.sense.clone(), bud_sense()].concat();
                    rhs = [vec![0.0; nt], vec![*b]].concat();
                    col_ids.extend(vec!["spp_met".to_string(); nt]);
                    row_ids = [vec!["spp_target".to_string(); nt], bud_ids()].concat();
                }
            }
            budget_row(&mut rows, &mut cols, &mut vals, nt);
            nrow_extra = 1;
            ncol_extra = nt;
            weight_cols = Some(n);
        }
        Objective::MinPenalties(b) => {
            sense = [t.sense.clone(), bud_sense()].concat();
            rhs = [t.value.clone(), vec![*b]].concat();
            budget_row(&mut rows, &mut cols, &mut vals, nt);
            row_ids = [vec!["spp_target".to_string(); nt], bud_ids()].concat();
            nrow_extra = 1;
        }
    }
    let nrow = nt + nrow_extra;
    let ncol = n + ncol_extra;
    if let Some(w) = &p.weights
        && !kind.no_weights()
    {
        if w.len() != nt {
            return Err(format!("the weights need one value per target ({nt})"));
        }
        let mut w = w.clone();
        if matches!(kind, Objective::MinShortfall(_)) {
            // its default weights: 1 / target (R: 1 / 0 = Inf -> 0)
            for (x, v) in w.iter_mut().zip(&t.value) {
                let d = 1.0 / v;
                *x *= if d.is_finite() { d } else { 0.0 };
            }
        }
        let at = weight_cols.unwrap_or(0);
        obj[at..at + w.len()].copy_from_slice(&w);
    }
    let mut b = Builder {
        nrow,
        ncol,
        rows,
        cols,
        vals,
        obj,
        lb,
        ub,
        vtype,
        sense,
        rhs,
        row_ids,
        col_ids,
        maximize: kind.name().starts_with("max"),
        n,
    };
    for sp in p.spatial.iter().filter(|s| s.is_penalty()) {
        apply_spatial(&mut b, u, sp)?;
    }
    for sp in p.spatial.iter().filter(|s| !s.is_penalty()) {
        apply_spatial(&mut b, u, sp)?;
    }
    // locked units (bounds only)
    if p.locked_in.len() == n && p.locked_out.len() == n {
        if let Some(i) = (0..n).find(|&i| p.locked_in[i] && p.locked_out[i]) {
            return Err(format!("the planning unit {} is both locked in and locked out", i + 1));
        }
        for i in 0..n {
            if p.locked_in[i] {
                b.lb[i] = 1.0;
            }
            if p.locked_out[i] {
                b.ub[i] = 0.0;
            }
        }
    }
    Ok((b.model(), t))
}

/// A model's rows' bounds (-inf / inf where open).
pub fn row_bounds(m: &Model) -> (Vec<f64>, Vec<f64>) {
    let lo =
        m.sense.iter().zip(&m.rhs).map(|(s, r)| if s == ">=" || s == "=" { *r } else { f64::NEG_INFINITY }).collect();
    let hi = m.sense.iter().zip(&m.rhs).map(|(s, r)| if s == "<=" || s == "=" { *r } else { f64::INFINITY }).collect();
    (lo, hi)
}

/// landschaft's choice of HiGHS's engine: covering (a minimisation with only >= rows and finite bounds), wide
/// (the units with finite bounds and few other columns) or HiGHS itself.
pub fn method(m: &Model, n_units: usize) -> &'static str {
    let finite = |v: &[f64]| v.iter().all(|x| x.is_finite());
    if !m.maximize && m.sense.iter().all(|s| s == ">=") && finite(&m.lb) && finite(&m.ub) {
        "covering"
    } else if n_units > 0 && m.obj.len() - n_units <= 20000 && finite(&m.lb[..n_units]) && finite(&m.ub[..n_units]) {
        "wide"
    } else {
        "highs"
    }
}

/// A model solved: its status, the solution, its objective and gap.
pub fn solve(m: &Model, n_units: usize, gap: f64, time_limit: f64) -> Result<ffi::SolveOut, String> {
    let (lo, hi) = row_bounds(m);
    let model = ffi::MilpModel {
        start: m.a.start.iter().map(|&s| s as i64).collect(),
        index: m.a.index.iter().map(|&s| s as i64).collect(),
        value: m.a.value.clone(),
        obj: m.obj.clone(),
        lb: m.lb.clone(),
        ub: m.ub.clone(),
        row_lo: lo,
        row_hi: hi,
        integer: m.vtype.iter().map(|&t| u8::from(t == 'B' || t == 'I')).collect(),
        maximize: m.maximize,
        offset: 0.0,
    };
    let settings =
        ffi::SolveSettings { gap, time_limit, presolve: "choose".into(), relaxation: false, start: Vec::new() };
    ffi::opt_solve(&model, &settings, method(m, n_units), n_units as i64).map_err(|e| e.what().to_string())
}

/// prioritizr's spatial penalties and constraints (on raster units).
#[derive(Debug, Clone, PartialEq)]
pub enum Spatial {
    /// add_boundary_penalties(penalty, edge_factor) on the units' boundary matrix
    Boundary { penalty: f64, edge_factor: f64 },
    /// add_linear_penalties(penalty, data): a value per cell
    LinearPenalty { penalty: f64, data: Vec<f64> },
    /// add_linear_constraints(threshold, sense, data): a value per cell
    LinearConstraint { threshold: f64, sense: String, data: Vec<f64> },
    /// add_neighbors_constraints(k, clamp) on the units' 4-adjacency
    Neighbors { k: f64, clamp: bool },
}

impl Spatial {
    fn is_penalty(&self) -> bool {
        matches!(self, Spatial::Boundary { .. } | Spatial::LinearPenalty { .. })
    }
}

/// A sparse matrix in compressed columns (Matrix's dgCMatrix, armadillo's sp_mat).
#[derive(Debug, Clone, Default)]
struct Csc {
    n: usize,
    start: Vec<usize>,
    index: Vec<usize>,
    value: Vec<f64>,
}

impl Csc {
    /// From triplets of an n x n matrix: duplicates added up (scipy's sum_duplicates), rows in order.
    fn from_triplets(n: usize, rows: &[usize], cols: &[usize], vals: &[f64]) -> Csc {
        let t = Csr::from_triplets(n, n, cols, rows, vals);
        Csc { n, start: t.start, index: t.index, value: t.value }
    }

    /// The units' sub-matrix (rows and columns of `cells`, in their order), zeros dropped.
    fn cut(&self, cells: &[usize]) -> Csc {
        let mut pos = vec![usize::MAX; self.n];
        for (k, &c) in cells.iter().enumerate() {
            pos[c] = k;
        }
        let mut out = Csc { n: cells.len(), start: vec![0], index: Vec::new(), value: Vec::new() };
        for &c in cells {
            let mut col: Vec<(usize, f64)> = (self.start[c]..self.start[c + 1])
                .filter(|&e| pos[self.index[e]] != usize::MAX && self.value[e] != 0.0)
                .map(|e| (pos[self.index[e]], self.value[e]))
                .collect();
            col.sort_by_key(|&(r, _)| r);
            for (r, v) in col {
                out.index.push(r);
                out.value.push(v);
            }
            out.start.push(out.index.len());
        }
        out
    }

    fn diagonal(&self) -> Vec<f64> {
        (0..self.n)
            .map(|j| (self.start[j]..self.start[j + 1]).find(|&e| self.index[e] == j).map_or(0.0, |e| self.value[e]))
            .collect()
    }

    /// Matrix::rowSums: every row's sum in double, over the columns in order.
    fn row_sums(&self) -> Vec<f64> {
        let mut by_row: Vec<Vec<(usize, f64)>> = vec![Vec::new(); self.n];
        for j in 0..self.n {
            for e in self.start[j]..self.start[j + 1] {
                by_row[self.index[e]].push((j, self.value[e]));
            }
        }
        by_row.iter().map(|r| r.iter().fold(0.0, |a, (_, v)| a + v)).collect()
    }
}

/// The cells of a raster whose cost is finite, and their 4-neighbour pairs: vertical (upper, lower) then
/// horizontal (left, right), each in the raster's order.
fn pairs_4(include: &[usize], ny: usize, nx: usize) -> [(Vec<usize>, Vec<usize>); 2] {
    let mut mask = vec![false; ny * nx];
    for &c in include {
        mask[c] = true;
    }
    let (mut u, mut d) = (Vec::new(), Vec::new());
    for y in 0..ny.saturating_sub(1) {
        for x in 0..nx {
            let (a, b) = (y * nx + x, (y + 1) * nx + x);
            if mask[a] && mask[b] {
                u.push(a);
                d.push(b);
            }
        }
    }
    let (mut l, mut r) = (Vec::new(), Vec::new());
    for y in 0..ny {
        for x in 0..nx.saturating_sub(1) {
            let (a, b) = (y * nx + x, y * nx + x + 1);
            if mask[a] && mask[b] {
                l.push(a);
                r.push(b);
            }
        }
    }
    [(u, d), (l, r)]
}

/// A symmetric matrix from the pairs (i, j) of value v.
fn symmetric(i: &[usize], j: &[usize], v: &[f64]) -> (Vec<usize>, Vec<usize>, Vec<f64>) {
    ([i, j].concat(), [j, i].concat(), [v, v].concat())
}

impl Units {
    /// prioritizr's boundary_matrix of the raster, cut to the units: the shared edges of 4-neighbours, every
    /// cell's perimeter on the diagonal.
    fn boundary_matrix(&self) -> Csc {
        let (ny, nx) = (self.ny, self.nx);
        let n = ny * nx;
        let t = self.transform;
        // as landschaft's _res, operation for operation
        let xres = (t[0] + nx as f64 * t[1] - t[0]) / nx as f64;
        let yres = (t[3] - (t[3] + ny as f64 * t[5])) / ny as f64;
        let [(u, d), (l, r)] = pairs_4(&self.cells, ny, nx);
        let v: Vec<f64> = [vec![xres; u.len()], vec![yres; l.len()]].concat();
        let (mut rows, mut cols, mut vals) = symmetric(&[u, l].concat(), &[d, r].concat(), &v);
        // + the perimeters on the diagonal (added after the shared edges)
        let perimeter = (xres + yres) * 2.0;
        let sym = Csc::from_triplets(n, &rows, &cols, &vals);
        rows.clear();
        cols.clear();
        vals.clear();
        for j in 0..n {
            for e in sym.start[j]..sym.start[j + 1] {
                rows.push(sym.index[e]);
                cols.push(j);
                vals.push(sym.value[e]);
            }
        }
        for &c in &self.cells {
            rows.push(c);
            cols.push(c);
            vals.push(perimeter);
        }
        Csc::from_triplets(n, &rows, &cols, &vals).cut(&self.cells)
    }

    /// prioritizr's adjacency_matrix (4 neighbours) of the raster, cut to the units.
    fn adjacency_matrix(&self) -> Csc {
        let n = self.ny * self.nx;
        let [(u, d), (l, r)] = pairs_4(&self.cells, self.ny, self.nx);
        let i = [u, l].concat();
        let j = [d, r].concat();
        let (rows, cols, vals) = symmetric(&i, &j, &vec![1.0; i.len()]);
        let mut m = Csc::from_triplets(n, &rows, &cols, &vals);
        m.value.iter_mut().for_each(|v| *v = 1.0);
        m.cut(&self.cells)
    }
}

impl Builder {
    fn add_columns(&mut self, obj: &[f64], lb: f64, ub: f64, vtype: char, id: &str) -> usize {
        let first = self.ncol;
        self.obj.extend_from_slice(obj);
        self.lb.extend(std::iter::repeat_n(lb, obj.len()));
        self.ub.extend(std::iter::repeat_n(ub, obj.len()));
        self.vtype.extend(std::iter::repeat_n(vtype, obj.len()));
        self.col_ids.extend(std::iter::repeat_n(id.to_string(), obj.len()));
        self.ncol += obj.len();
        first
    }

    /// Rows numbered from 0 (r), appended after the current ones.
    fn add_rows(&mut self, r: &[usize], c: &[usize], v: &[f64], sense: &[String], rhs: &[f64], ids: &[String]) {
        let at = self.nrow;
        self.push(r.iter().map(|x| x + at), c.iter().copied(), v.iter().copied());
        self.sense.extend_from_slice(sense);
        self.rhs.extend_from_slice(rhs);
        self.row_ids.extend_from_slice(ids);
        self.nrow += sense.len();
    }

    /// One column per pair (x_i x_j, linearised) and its rows, as rcpp_apply_*_penalties.
    fn pair_columns(&mut self, pi: &[usize], pj: &[usize], coef: &[f64], id: &str) {
        if pi.is_empty() {
            return;
        }
        let vt = self.vtype[0];
        let first = self.add_columns(coef, 0.0, 1.0, vt, id);
        let (mut r, mut c, mut v, mut sense, mut rhs, mut ids) = (vec![], vec![], vec![], vec![], vec![], vec![]);
        let mut row = 0;
        for k in 0..pi.len() {
            let var = first + k;
            for (other, suffix) in [(pi[k], "1"), (pj[k], "2")] {
                r.extend([row, row]);
                c.extend([var, other]);
                v.extend([1.0, -1.0]);
                sense.push("<=".to_string());
                rhs.push(0.0);
                ids.push(format!("{id}{suffix}"));
                row += 1;
            }
            let third = if self.maximize { coef[k] < 0.0 } else { coef[k] > 0.0 };
            if third {
                r.extend([row, row, row]);
                c.extend([var, pi[k], pj[k]]);
                v.extend([1.0, -1.0, -1.0]);
                sense.push(">=".to_string());
                rhs.push(-1.0);
                ids.push(format!("{id}3"));
                row += 1;
            }
        }
        self.add_rows(&r, &c, &v, &sense, &rhs, &ids);
    }
}

/// A spatial penalty or constraint applied to the model being compiled.
fn apply_spatial(b: &mut Builder, u: &Units, s: &Spatial) -> Result<(), String> {
    let n = b.n;
    match s {
        Spatial::Boundary { penalty, edge_factor } => {
            if penalty.abs().partial_cmp(&1e-50) != Some(std::cmp::Ordering::Greater) {
                return Ok(());
            }
            let bm = u.boundary_matrix();
            let total = bm.diagonal();
            let rs = bm.row_sums();
            let p = if b.maximize { -penalty } else { *penalty };
            for i in 0..n {
                let exposed = total[i] - (rs[i] - total[i]);
                b.obj[i] += p * 1.0 * ((total[i] - exposed) + (exposed * edge_factor));
            }
            // the strictly lower triangle, by column then row
            let (mut pi, mut pj, mut coef) = (vec![], vec![], vec![]);
            for j in 0..bm.n {
                for e in bm.start[j]..bm.start[j + 1] {
                    let i = bm.index[e];
                    if i > j {
                        let cv = bm.value[e] * 1.0 * p;
                        if cv.abs() > 1.0e-15 {
                            pi.push(i);
                            pj.push(j);
                            coef.push(cv * -2.0);
                        }
                    }
                }
            }
            b.pair_columns(&pi, &pj, &coef, "b");
        }
        Spatial::LinearPenalty { penalty, data } => {
            if penalty.abs().partial_cmp(&1e-50) != Some(std::cmp::Ordering::Greater) {
                return Ok(());
            }
            let p = if b.maximize { -penalty } else { *penalty };
            for (i, &c) in u.cells.iter().enumerate() {
                let v = if data[c].is_nan() { 0.0 } else { data[c] };
                let cv = v * p;
                b.obj[i] += if v != 0.0 && cv.abs() > 1.0e-15 { cv } else { 0.0 };
            }
        }
        Spatial::LinearConstraint { threshold, sense, data } => {
            let (mut c, mut v) = (vec![], vec![]);
            for (i, &cell) in u.cells.iter().enumerate() {
                let x = if data[cell].is_nan() { 0.0 } else { data[cell] };
                if x != 0.0 {
                    c.push(i);
                    v.push(x);
                }
            }
            b.add_rows(&vec![0; c.len()], &c, &v, std::slice::from_ref(sense), &[*threshold], &["lc".to_string()]);
        }
        Spatial::Neighbors { k, clamp } => {
            let d = u.adjacency_matrix();
            let mut kk = vec![*k; n];
            if *clamp {
                for (k, rs) in kk.iter_mut().zip(d.row_sums()) {
                    *k = k.min(rs);
                }
            }
            if kk.iter().cloned().fold(0.0, f64::max).partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
                return Ok(());
            }
            let (mut r, mut c, mut v): (Vec<usize>, Vec<usize>, Vec<f64>) =
                ((0..n).collect(), (0..n).collect(), kk.iter().map(|k| k * -1.0).collect());
            for j in 0..d.n {
                for e in d.start[j]..d.start[j + 1] {
                    if d.value[e] > 1.0e-15 && d.index[e] != j {
                        r.push(d.index[e]);
                        c.push(j);
                        v.push(1.0);
                    }
                }
            }
            b.add_rows(&r, &c, &v, &vec![">=".to_string(); n], &vec![0.0; n], &vec!["n".to_string(); n]);
        }
    }
    Ok(())
}
