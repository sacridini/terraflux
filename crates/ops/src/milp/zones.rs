//! Management zones (prioritizr 8.0.6's zones, as landschaft compiles them): planning units that go to at most one
//! of several zones. Every zone has its own cost and its own amount of every feature (prioritizr's `zones()`, or
//! the base features times every zone's contribution). The planning units are the cells (rows) with a cost in any
//! zone; no cost in a zone keeps the unit out of it. The model has a column per unit and zone, zone after zone
//! (column z * units + i).

use std::collections::HashMap;

use super::*;

impl Units {
    /// Whether the units have management zones.
    pub fn zoned(&self) -> bool {
        !self.zones.is_empty()
    }

    /// The number of zones (1 without zones).
    pub fn nz(&self) -> usize {
        self.zones.len().max(1)
    }

    /// The number of planning units (the columns are units x zones).
    pub fn units(&self) -> usize {
        self.cells.len()
    }

    /// Zone z's features x units block of the amounts.
    pub fn zone_rij(&self, z: usize) -> Csr {
        let nu = self.units();
        let (lo, hi) = (z * nu, (z + 1) * nu);
        let mut out = Csr { nrow: self.rij.nrow, ncol: nu, start: vec![0], index: Vec::new(), value: Vec::new() };
        for f in 0..self.rij.nrow {
            let (idx, v) = self.rij.row(f);
            for (&j, &x) in idx.iter().zip(v) {
                if j >= lo && j < hi {
                    out.index.push(j - lo);
                    out.value.push(x);
                }
            }
            out.start.push(out.index.len());
        }
        out
    }

    /// The zones' blocks (features x units each) side by side, and the totals: every feature in every zone.
    fn with_zones(
        mut self,
        zones: Vec<String>,
        blocks: Vec<Csr>,
        zone_totals: Vec<Vec<f64>>,
        base_totals: Option<Vec<f64>>,
    ) -> Units {
        let nu = self.cells.len();
        let nf = blocks[0].nrow;
        let mut rij = Csr { nrow: nf, ncol: nu * blocks.len(), start: vec![0], index: Vec::new(), value: Vec::new() };
        for f in 0..nf {
            for (z, b) in blocks.iter().enumerate() {
                let (idx, v) = b.row(f);
                rij.index.extend(idx.iter().map(|j| z * nu + j));
                rij.value.extend_from_slice(v);
            }
            rij.start.push(rij.index.len());
        }
        self.rij = rij;
        // R's rowSums of the zones' totals (max_phylo_end's)
        self.totals = zone_totals.iter().map(|t| ffi::r_sum(t)).collect();
        self.zone_totals = zone_totals;
        self.base_totals = base_totals;
        self.zones = zones;
        self
    }

    /// Raster units with zones: a cost raster per zone, and the features of every zone (zone after zone: layer
    /// z * features + f), or the base features and every zone's contribution (a factor per feature, or a raster).
    /// The cells with a cost in any zone; the totals over the whole raster (terra's), through landschaft's
    /// FeatureMatrix (a product formed in double, as terra multiplies two rasters).
    pub fn raster_zones(
        costs: &[&[f64]],
        features: ZoneFeatures,
        zones: Vec<String>,
        names: Vec<String>,
        (ny, nx): (usize, usize),
        transform: [f64; 6],
    ) -> Result<Units, String> {
        let nz = zones.len();
        if costs.len() != nz || nz == 0 {
            return Err(format!("a cost raster per zone ({nz})"));
        }
        let size = ny * nx;
        if costs.iter().any(|c| c.len() != size) {
            return Err("the zones' costs must be on one grid".into());
        }
        let cells: Vec<usize> = (0..size).filter(|&c| costs.iter().any(|z| z[c].is_finite())).collect();
        if cells.is_empty() {
            return Err("no cell has a cost in any zone".into());
        }
        let mut unit = vec![-1i64; size];
        for (i, &c) in cells.iter().enumerate() {
            unit[c] = i as i64;
        }
        let (values, base_totals, nf) = match features {
            ZoneFeatures::General(layers) => {
                if layers.is_empty() || layers.len() % nz != 0 {
                    return Err(format!("every zone needs the same features (layers: a multiple of {nz})"));
                }
                (layers.iter().flat_map(|f| f.iter().copied()).collect::<Vec<f64>>(), None, layers.len() / nz)
            }
            ZoneFeatures::Contribution { base, factors } => {
                if factors.len() != nz {
                    return Err(format!("a contribution per zone ({nz})"));
                }
                let nf = base.len();
                let mut values = Vec::with_capacity(nz * nf * size);
                for c in &factors {
                    for (f, b) in base.iter().enumerate() {
                        match c {
                            Contribution::Factors(k) => values.extend(b.iter().map(|v| v * k[f])),
                            Contribution::Map(m) => values.extend(b.iter().zip(m.iter()).map(|(v, s)| v * s)),
                        }
                    }
                }
                let all: Vec<f64> = base.iter().flat_map(|f| f.iter().copied()).collect();
                let none = vec![-1i64; size];
                let b = ffi::opt_features(&all, nf as i64, &none).map_err(|e| e.what().to_string())?;
                (values, Some(b.totals), nf)
            }
        };
        if nf == 0 {
            return Err("no features".into());
        }
        if values.len() != nz * nf * size {
            return Err("the features must be on the costs' grid".into());
        }
        let m = ffi::opt_features(&values, (nz * nf) as i64, &unit).map_err(|e| e.what().to_string())?;
        let full = Csr {
            nrow: nz * nf,
            ncol: cells.len(),
            start: m.start.iter().map(|&s| s as usize).collect(),
            index: m.index.iter().map(|&s| s as usize).collect(),
            value: m.value,
        };
        let blocks: Vec<Csr> = (0..nz).map(|z| full.rows(z * nf..(z + 1) * nf)).collect();
        let zone_totals = (0..nf).map(|f| (0..nz).map(|z| m.totals[z * nf + f]).collect()).collect();
        let cost = (0..nz).flat_map(|z| cells.iter().map(move |&c| costs[z][c])).collect();
        let u = Units {
            cost,
            rij: Csr::default(),
            totals: Vec::new(),
            names,
            cells,
            ny,
            nx,
            transform,
            raster: true,
            size,
            zones: Vec::new(),
            zone_totals: Vec::new(),
            base_totals: None,
        };
        Ok(u.with_zones(zones, blocks, zone_totals, base_totals))
    }

    /// Table units with zones: a cost column per zone; every zone's amounts (zone x feature x row; no value 0),
    /// and the base features' with contributions. The rows with a cost in any zone; the totals R's colSums.
    pub fn table_zones(
        costs: &[&[f64]],
        amounts: &[Vec<Vec<f64>>],
        base: Option<&[Vec<f64>]>,
        zones: Vec<String>,
        names: Vec<String>,
    ) -> Result<Units, String> {
        let nz = zones.len();
        if costs.len() != nz || amounts.len() != nz || nz == 0 {
            return Err(format!("a cost column per zone ({nz})"));
        }
        let size = costs[0].len();
        let rows: Vec<usize> = (0..size).filter(|&r| costs.iter().any(|c| c[r].is_finite())).collect();
        if rows.is_empty() {
            return Err("no row has a cost in any zone".into());
        }
        let nf = names.len();
        let zero = |v: f64| if v.is_nan() { 0.0 } else { v };
        let zone_totals = (0..nf)
            .map(|f| (0..nz).map(|z| ffi::r_sum(&amounts[z][f].iter().map(|&v| zero(v)).collect::<Vec<_>>())).collect())
            .collect();
        let base_totals =
            base.map(|b| b.iter().map(|f| ffi::r_sum(&f.iter().map(|&v| zero(v)).collect::<Vec<_>>())).collect());
        let blocks = amounts
            .iter()
            .map(|a| {
                let mut m = Csr { nrow: nf, ncol: rows.len(), start: vec![0], index: Vec::new(), value: Vec::new() };
                for f in a {
                    for (k, &r) in rows.iter().enumerate() {
                        let v = zero(f[r]);
                        if v != 0.0 {
                            m.index.push(k);
                            m.value.push(v);
                        }
                    }
                    m.start.push(m.index.len());
                }
                m
            })
            .collect();
        let cost = (0..nz).flat_map(|z| rows.iter().map(move |&r| costs[z][r])).collect();
        let u = Units {
            cost,
            rij: Csr::default(),
            totals: Vec::new(),
            names,
            cells: rows,
            ny: 0,
            nx: 0,
            transform: [0.0; 6],
            raster: false,
            size,
            zones: Vec::new(),
            zone_totals: Vec::new(),
            base_totals: None,
        };
        Ok(u.with_zones(zones, blocks, zone_totals, base_totals))
    }

    /// Marxan-style table units with zones (prioritizr's problem(x, features, rij, zones)): a cost column per zone,
    /// rij's amounts with a zone column (the zones' ids); the totals added in the table's order.
    pub fn rij_zones(
        ids: &[f64],
        costs: &[&[f64]],
        feature_ids: &[f64],
        names: Vec<String>,
        (pu, species, zone, amount): (&[f64], &[f64], &[f64], &[f64]),
        zone_ids: &[f64],
        zones: Vec<String>,
    ) -> Result<Units, String> {
        let nz = zones.len();
        if costs.len() != nz || zone_ids.len() != nz || nz == 0 {
            return Err(format!("a cost column and an id per zone ({nz})"));
        }
        let size = ids.len();
        let nf = feature_ids.len();
        let key = |v: &[f64]| v.iter().enumerate().map(|(k, x)| (x.to_bits(), k)).collect::<HashMap<u64, usize>>();
        let (unit_of, feat_of, zone_of) = (key(ids), key(feature_ids), key(zone_ids));
        let rows: Vec<usize> = (0..size).filter(|&r| costs.iter().any(|c| c[r].is_finite())).collect();
        let mut pos = vec![usize::MAX; size];
        for (k, &r) in rows.iter().enumerate() {
            pos[r] = k;
        }
        let mut zone_totals = vec![vec![0.0; nz]; nf];
        let mut trip: Vec<(Vec<usize>, Vec<usize>, Vec<f64>)> = vec![(Vec::new(), Vec::new(), Vec::new()); nz];
        for k in 0..amount.len() {
            let (Some(&r), Some(&f), Some(&z)) =
                (unit_of.get(&pu[k].to_bits()), feat_of.get(&species[k].to_bits()), zone_of.get(&zone[k].to_bits()))
            else {
                return Err("rij refers to planning units, features or zones that are not in the tables".into());
            };
            zone_totals[f][z] += amount[k];
            if pos[r] != usize::MAX {
                trip[z].0.push(f);
                trip[z].1.push(pos[r]);
                trip[z].2.push(amount[k]);
            }
        }
        let blocks = trip
            .iter()
            .map(|(f, r, a)| {
                let m = Csr::from_triplets(nf, rows.len(), f, r, a);
                let keep: Vec<usize> = (0..m.value.len()).filter(|&e| m.value[e] != 0.0).collect();
                let mut out = Csr { nrow: nf, ncol: rows.len(), start: vec![0], index: Vec::new(), value: Vec::new() };
                for i in 0..nf {
                    for e in m.start[i]..m.start[i + 1] {
                        if keep.binary_search(&e).is_ok() {
                            out.index.push(m.index[e]);
                            out.value.push(m.value[e]);
                        }
                    }
                    out.start.push(out.index.len());
                }
                out
            })
            .collect();
        let cost = (0..nz).flat_map(|z| rows.iter().map(move |&r| costs[z][r])).collect();
        let u = Units {
            cost,
            rij: Csr::default(),
            totals: Vec::new(),
            names,
            cells: rows,
            ny: 0,
            nx: 0,
            transform: [0.0; 6],
            raster: false,
            size,
            zones: Vec::new(),
            zone_totals: Vec::new(),
            base_totals: None,
        };
        Ok(u.with_zones(zones, blocks, zone_totals, None))
    }
}

impl Csr {
    /// Some rows.
    fn rows(&self, r: std::ops::Range<usize>) -> Csr {
        let (a, b) = (self.start[r.start], self.start[r.end]);
        Csr {
            nrow: r.len(),
            ncol: self.ncol,
            start: self.start[r.start..=r.end].iter().map(|s| s - a).collect(),
            index: self.index[a..b].to_vec(),
            value: self.value[a..b].to_vec(),
        }
    }
}

/// The features of raster units with zones.
pub enum ZoneFeatures<'a> {
    /// every zone's own features (prioritizr's zones()): zone after zone, layer z * features + f
    General(Vec<&'a [f64]>),
    /// the base features and every zone's contribution
    Contribution { base: Vec<&'a [f64]>, factors: Vec<Contribution<'a>> },
}

/// A zone's contribution to the features: a factor per feature, or a raster.
pub enum Contribution<'a> {
    Factors(Vec<f64>),
    Map(&'a [f64]),
}

/// The zones' labels ("protect + use").
fn zone_label(u: &Units, zs: &[usize]) -> String {
    zs.iter().map(|&z| u.zones[z].as_str()).collect::<Vec<_>>().join(" + ")
}

/// The target rows of a problem with zones: the targets of every zone (summed over them: relative targets are
/// shares of the base features' totals, or of R's sum of the zones' totals), a matrix of every zone's own, or
/// manual ones with their zones.
pub fn resolve_zone_targets(u: &Units, spec: &TargetSpec) -> Result<Targets, String> {
    let (nf, nz) = (u.names.len(), u.nz());
    let every: Vec<usize> = (0..nz).collect();
    let mut t = Targets::default();
    let denominator = |f: usize, zs: &[usize]| -> f64 {
        if zs.len() == 1 {
            u.zone_totals[f][zs[0]]
        } else if let Some(b) = &u.base_totals {
            b[f]
        } else {
            ffi::r_sum(&zs.iter().map(|&z| u.zone_totals[f][z]).collect::<Vec<_>>())
        }
    };
    let mut add = |f: usize, zs: Vec<usize>, relative: bool, v: f64, s: &str| {
        let d = denominator(f, &zs);
        t.feature.push(f);
        t.sense.push(s.into());
        t.total.push(d);
        t.value.push(if relative { d * v } else { v });
        t.zone.push(zs);
    };
    match spec {
        TargetSpec::Relative(r) | TargetSpec::Absolute(r) => {
            let relative = matches!(spec, TargetSpec::Relative(_));
            if r.len() != nf {
                return Err(format!("a target per feature ({nf})"));
            }
            for (f, &v) in r.iter().enumerate() {
                add(f, every.clone(), relative, v, ">=");
            }
        }
        TargetSpec::RelativeZones(m) | TargetSpec::AbsoluteZones(m) => {
            let relative = matches!(spec, TargetSpec::RelativeZones(_));
            if m.len() != nf || m.iter().any(|r| r.len() != nz) {
                return Err(format!("a matrix of targets must be features x zones ({nf} x {nz})"));
            }
            // prioritizr's matrix: feature first, zone after zone
            for z in 0..nz {
                for (f, row) in m.iter().enumerate() {
                    add(f, vec![z], relative, row[z], ">=");
                }
            }
        }
        TargetSpec::Manual(ts) => {
            for m in ts {
                if m.feature >= nf {
                    return Err(format!("no feature {} in the targets", m.feature + 1));
                }
                if !matches!(m.sense.as_str(), ">=" | "<=" | "=") {
                    return Err("target senses are '>=', '<=' or '='".into());
                }
                let zs = m.zones.clone().unwrap_or_else(|| every.clone());
                if zs.is_empty() || zs.iter().any(|&z| z >= nz) {
                    return Err(format!("a target's zones are 1 to {nz}"));
                }
                add(m.feature, zs, m.relative, m.target, &m.sense);
            }
        }
        TargetSpec::Loglinear { .. } => return Err("loglinear targets are not supported with zones".into()),
    }
    if t.feature.is_empty() {
        return Err("no targets".into());
    }
    t.labels = t.zone.iter().map(|zs| zone_label(u, zs)).collect();
    Ok(t)
}

/// The targets prioritizr's compile() makes for objectives without targets: every feature in every zone (feature
/// first, zone after zone).
pub fn fake_zone_targets(u: &Units) -> Targets {
    let (nf, nz) = (u.names.len(), u.nz());
    let zone: Vec<Vec<usize>> = (0..nz).flat_map(|z| std::iter::repeat_n(vec![z], nf)).collect();
    Targets {
        feature: (0..nz).flat_map(|_| 0..nf).collect(),
        sense: vec!["?".into(); nf * nz],
        value: vec![0.0; nf * nz],
        total: (0..nz).flat_map(|z| (0..nf).map(move |f| u.zone_totals[f][z])).collect(),
        labels: zone.iter().map(|zs| zone_label(u, zs)).collect(),
        zone,
    }
}

/// The targets' rows over the unit columns: every target's feature in each of its zones (triplets).
pub fn zone_target_entries(u: &Units, t: &Targets) -> (Vec<usize>, Vec<usize>, Vec<f64>) {
    let (nu, nz) = (u.units(), u.nz());
    let mut entries: Vec<(usize, usize, f64)> = Vec::new();
    for z in 0..nz {
        let block = u.zone_rij(z);
        for (i, (f, zs)) in t.feature.iter().zip(&t.zone).enumerate() {
            if zs.contains(&z) {
                let (idx, v) = block.row(*f);
                entries.extend(idx.iter().zip(v).map(|(&j, &x)| (i, z * nu + j, x)));
            }
        }
    }
    entries.sort_by_key(|e| (e.0, e.1));
    let r = entries.iter().map(|e| e.0).collect();
    let c = entries.iter().map(|e| e.1).collect();
    let v = entries.iter().map(|e| e.2).collect();
    (r, c, v)
}

/// Every target's positive amount in the units that have a cost in its zones (prioritizr's
/// feature_positive_abundances_in_planning_units: a sum in unit order per zone, then the zones in turn).
pub fn positive_abundances(u: &Units, t: &Targets) -> Vec<f64> {
    let nu = u.units();
    let blocks: Vec<Csr> = (0..u.nz()).map(|z| u.zone_rij(z)).collect();
    t.feature
        .iter()
        .zip(&t.zone)
        .map(|(&f, zs)| {
            let mut acc = 0.0;
            for (k, &z) in zs.iter().enumerate() {
                let (idx, v) = blocks[z].row(f);
                let pos: Vec<f64> = idx
                    .iter()
                    .zip(v)
                    .filter(|(j, _)| u.cost[z * nu + **j].is_finite())
                    .map(|(_, x)| x.max(0.0))
                    .collect();
                let s = seq_sum(&pos);
                acc = if k == 0 { s } else { acc + s };
            }
            acc
        })
        .collect()
}

/// prioritizr's spatial penalties and constraints with zones. A zones matrix (zones x zones, rows the first zone)
/// says how much a pair of neighbouring (or connected) units counts when they are in two given zones: the diagonal
/// for units in the same zone (prioritizr's `zones` argument; by default the identity).
#[derive(Debug, Clone, PartialEq)]
pub enum ZoneSpatial {
    /// add_boundary_penalties(penalty, edge_factor (one per zone), data, zones)
    Boundary { penalty: f64, edge_factor: Vec<f64>, data: Option<Matrix>, zones: Option<Vec<Vec<f64>>> },
    /// add_connectivity_penalties(penalty, zones, data): a symmetric matrix of every unit
    Connectivity { penalty: f64, data: Matrix, zones: Option<Vec<Vec<f64>>> },
    /// add_asym_connectivity_penalties(penalty, zones, data)
    AsymConnectivity { penalty: f64, data: Matrix, zones: Option<Vec<Vec<f64>>> },
    /// add_linear_penalties(penalty per zone, data): a value per zone and cell (or row); zones with penalty 0 add
    /// nothing (prioritizr then leaves out every zone's, a bug)
    LinearPenalty { penalty: Vec<f64>, data: Vec<Vec<f64>> },
    /// add_linear_constraints(threshold, sense, data): the sum over every zone of the units' values there
    LinearConstraint { threshold: f64, sense: String, data: Vec<Vec<f64>> },
    /// add_neighbor_constraints(k per zone, clamp, zones, data): a unit in zone z1 has at least k[z1] neighbours
    /// in the zones z2 with zones[z1][z2] = 1
    Neighbors { k: Vec<f64>, clamp: bool, data: Option<Matrix>, zones: Option<Vec<Vec<f64>>> },
    /// add_contiguity_constraints(zones, data): the units of every cluster of connected zones form one patch
    /// (landschaft's exact formulation)
    Contiguity { data: Option<Matrix>, zones: Option<Vec<Vec<f64>>> },
}

impl ZoneSpatial {
    pub(super) fn is_penalty(&self) -> bool {
        matches!(
            self,
            ZoneSpatial::Boundary { .. }
                | ZoneSpatial::Connectivity { .. }
                | ZoneSpatial::AsymConnectivity { .. }
                | ZoneSpatial::LinearPenalty { .. }
        )
    }
}

/// A zones x zones matrix (none: the identity), checked as prioritizr does.
fn zone_matrix(
    nz: usize,
    zones: &Option<Vec<Vec<f64>>>,
    what: &str,
    binary: bool,
    symmetric: bool,
) -> Result<Vec<Vec<f64>>, String> {
    let Some(z) = zones else {
        return Ok((0..nz).map(|i| (0..nz).map(|j| f64::from(u8::from(i == j))).collect()).collect());
    };
    if z.len() != nz || z.iter().any(|r| r.len() != nz) {
        return Err(format!("{what}: the zones matrix must be {nz} x {nz}"));
    }
    if z.iter().flatten().any(|v| !v.is_finite() || *v < -1.0 || *v > 1.0) {
        return Err(format!("{what}: the zones matrix must hold values from -1 to 1"));
    }
    if symmetric && (0..nz).any(|i| (0..nz).any(|j| z[i][j] != z[j][i])) {
        return Err(format!("{what}: the zones matrix must be symmetric"));
    }
    if binary && z.iter().flatten().any(|v| *v != 0.0 && *v != 1.0) {
        return Err(format!("{what}: the zones matrix must hold 0 and 1"));
    }
    Ok(z.clone())
}

/// One value per zone (one for all, or one each).
fn per_zone(nz: usize, v: &[f64], what: &str) -> Result<Vec<f64>, String> {
    match v.len() {
        1 => Ok(vec![v[0]; nz]),
        k if k == nz => Ok(v.to_vec()),
        _ => Err(format!("{what}: one value or one per zone ({nz})")),
    }
}

/// The entries of every zone pair's matrix (m x zones[z1][z2], the lower triangle with its diagonal when
/// `lower`) times `scale`, by zone pair, then column, then row, the diagonal of the pairs of different zones left
/// out: (z1, z2, row, col, value).
fn zone_pairs(m: &Csc, zm: &[Vec<f64>], scale: f64, lower: bool) -> Vec<(usize, usize, usize, usize, f64)> {
    let nz = zm.len();
    let mut out = Vec::new();
    for (z1, row_z) in zm.iter().enumerate().take(nz) {
        for (z2, &w) in row_z.iter().enumerate() {
            for j in 0..m.n {
                for e in m.start[j]..m.start[j + 1] {
                    let i = m.index[e];
                    if lower && i < j {
                        continue;
                    }
                    let v = m.value[e] * w * scale;
                    if v.abs() > 1.0e-15 && !(z1 != z2 && i == j) {
                        out.push((z1, z2, i, j, v));
                    }
                }
            }
        }
    }
    out
}

/// A spatial penalty or constraint with zones applied to the model being compiled.
pub(super) fn apply_zone_spatial(b: &mut Builder, u: &Units, s: &ZoneSpatial) -> Result<(), String> {
    let (n, nz) = (u.units(), u.nz());
    match s {
        ZoneSpatial::Boundary { penalty, edge_factor, data, zones } => {
            let bm = match data {
                None if u.raster => u.boundary_matrix(),
                None => return Err("boundary penalties of tables need data: a boundary matrix of every unit".into()),
                Some(m) => {
                    let mut bm = m.units(u, "boundary data")?;
                    bm.value.iter_mut().for_each(|v| *v = r_round(*v, 6));
                    bm = bm.dropped_zeros();
                    if !bm.symmetric() {
                        return Err("the boundary data must be symmetric".into());
                    }
                    bm
                }
            };
            let zm = zone_matrix(nz, zones, "boundary penalties", false, true)?;
            let ef = per_zone(nz, edge_factor, "edge_factor")?;
            if ef.iter().any(|e| !(0.0..=1.0).contains(e)) {
                return Err("edge_factor must be from 0 to 1".into());
            }
            if penalty.abs().partial_cmp(&1e-50) != Some(std::cmp::Ordering::Greater) {
                return Ok(());
            }
            let total = bm.diagonal();
            let rs = bm.row_sums();
            let exposed: Vec<f64> = (0..n).map(|i| total[i] - (rs[i] - total[i])).collect();
            let p = if b.maximize { -penalty } else { *penalty };
            // the strictly lower triangle, by column then row
            let mut low = Vec::new();
            for j in 0..bm.n {
                for e in bm.start[j]..bm.start[j + 1] {
                    if bm.index[e] > j && bm.value[e] != 0.0 {
                        low.push((bm.index[e], j, bm.value[e]));
                    }
                }
            }
            let mut pen = vec![0.0; nz * n];
            let (mut pi, mut pj, mut pb) = (vec![], vec![], vec![]);
            for z1 in 0..nz {
                for (z2, &w) in zm[z1].iter().enumerate().skip(z1) {
                    if z1 == z2 {
                        for i in 0..n {
                            pen[z1 * n + i] += p * w * ((total[i] - exposed[i]) + (exposed[i] * ef[z1]));
                        }
                    }
                    for &(i, j, x) in &low {
                        let cv = x * w * p;
                        if cv.abs() <= 1.0e-15 {
                            continue;
                        }
                        let v = cv * -2.0;
                        if z1 == z2 {
                            pi.push(z1 * n + i);
                            pj.push(z1 * n + j);
                            pb.push(v);
                        } else {
                            // both ways: (z1 i, z2 j) then (z2 i, z1 j)
                            pi.extend([z1 * n + i, z2 * n + i]);
                            pj.extend([z2 * n + j, z1 * n + j]);
                            pb.extend([v, v]);
                        }
                    }
                }
            }
            for (o, x) in b.obj[..nz * n].iter_mut().zip(&pen) {
                *o += x;
            }
            b.pair_columns(&pi, &pj, &pb, "b");
        }
        ZoneSpatial::Connectivity { penalty, data, zones } => {
            let m = data.units(u, "connectivity data")?;
            if !m.symmetric() {
                return Err("the connectivity data must be symmetric (use asymmetric connectivity penalties)".into());
            }
            let zm = zone_matrix(nz, zones, "connectivity penalties", false, false)?;
            let mult = if b.maximize { 1.0 } else { -1.0 };
            let mut pen = vec![0.0; nz * n];
            let (mut pi, mut pj, mut pb) = (vec![], vec![], vec![]);
            for (z1, z2, row, col, v) in zone_pairs(&m, &zm, penalty * mult, true) {
                if row == col && z1 == z2 {
                    pen[z1 * n + row] += v;
                } else {
                    pi.push(z1 * n + row);
                    pj.push(z2 * n + col);
                    pb.push(v);
                }
            }
            for (o, x) in b.obj[..nz * n].iter_mut().zip(&pen) {
                *o += x;
            }
            b.pair_columns(&pi, &pj, &pb, "c");
        }
        ZoneSpatial::AsymConnectivity { penalty, data, zones } => {
            let m = data.units(u, "connectivity data")?;
            let zm = zone_matrix(nz, zones, "asymmetric connectivity penalties", false, false)?;
            let (mult1, mult2) = if b.maximize { (-1.0, 1.0) } else { (1.0, -1.0) };
            let mut pen = vec![0.0; nz * n];
            let (mut pi, mut pj, mut pb) = (vec![], vec![], vec![]);
            for (z1, z2, row, col, v) in zone_pairs(&m, &zm, *penalty, false) {
                if row == col && z1 == z2 {
                    pen[z1 * n + row] += mult2 * v;
                } else {
                    pen[z1 * n + row] += mult1 * v;
                    pi.push(z1 * n + row);
                    pj.push(z2 * n + col);
                    pb.push(mult2 * v);
                }
            }
            for (o, x) in b.obj[..nz * n].iter_mut().zip(&pen) {
                *o += x;
            }
            b.pair_columns(&pi, &pj, &pb, "ac");
        }
        ZoneSpatial::LinearPenalty { penalty, data } => {
            let mut p = per_zone(nz, penalty, "linear penalties")?;
            if p.iter().map(|v| v.abs()).fold(0.0, f64::max).partial_cmp(&1e-50) != Some(std::cmp::Ordering::Greater) {
                return Ok(());
            }
            if b.maximize {
                p.iter_mut().for_each(|v| *v = -*v);
            }
            let d = zone_data(u, data, "linear penalty data")?;
            for (z, dz) in d.iter().enumerate() {
                for (i, &x) in dz.iter().enumerate() {
                    let cv = x * p[z];
                    b.obj[z * n + i] += if x != 0.0 && cv.abs() > 1.0e-15 { cv } else { 0.0 };
                }
            }
        }
        ZoneSpatial::LinearConstraint { threshold, sense, data } => {
            let d = zone_data(u, data, "linear constraint data")?;
            let (mut c, mut v) = (vec![], vec![]);
            for (z, dz) in d.iter().enumerate() {
                for (i, &x) in dz.iter().enumerate() {
                    if x != 0.0 {
                        c.push(z * n + i);
                        v.push(x);
                    }
                }
            }
            b.add_rows(&vec![0; c.len()], &c, &v, std::slice::from_ref(sense), &[*threshold], &["lc".to_string()]);
        }
        ZoneSpatial::Neighbors { k, clamp, data, zones } => {
            let zm = zone_matrix(nz, zones, "neighbor constraints", true, false)?;
            let kk = per_zone(nz, k, "k")?;
            if kk.iter().any(|v| *v < 0.0 || v.fract() != 0.0) {
                return Err("k must be whole numbers >= 0".into());
            }
            let d = adjacency(u, data.as_ref())?;
            // k of every unit in every zone (clamped to the neighbours it has in the zones that count)
            let mut kmat = vec![kk.clone(); n];
            if *clamp {
                for (z1, row_z) in zm.iter().enumerate() {
                    let mut mx = d.clone();
                    for (e, v) in mx.value.iter_mut().enumerate() {
                        *v = row_z.iter().map(|w| d.value[e] * w).fold(f64::NEG_INFINITY, f64::max);
                    }
                    for (row, rs) in kmat.iter_mut().zip(mx.row_sums()) {
                        row[z1] = kk[z1].min(rs);
                    }
                }
            }
            if kmat.iter().flatten().cloned().fold(0.0, f64::max).partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
            {
                return Ok(());
            }
            let (mut r, mut c, mut v): (Vec<usize>, Vec<usize>, Vec<f64>) =
                ((0..nz * n).collect(), (0..nz * n).collect(), Vec::new());
            for z in 0..nz {
                v.extend(kmat.iter().map(|row| -row[z]));
            }
            for (z1, row_z) in zm.iter().enumerate() {
                for (z2, &w) in row_z.iter().enumerate() {
                    for j in 0..d.n {
                        for e in d.start[j]..d.start[j + 1] {
                            if d.value[e] * w > 1.0e-15 && d.index[e] != j {
                                r.push(z1 * n + d.index[e]);
                                c.push(z2 * n + j);
                                v.push(1.0);
                            }
                        }
                    }
                }
            }
            b.add_rows(&r, &c, &v, &vec![">=".to_string(); nz * n], &vec![0.0; nz * n], &vec!["n".to_string(); nz * n]);
        }
        ZoneSpatial::Contiguity { data, zones } => {
            let zm = zone_matrix(nz, zones, "contiguity constraints", true, true)?;
            let d = adjacency(u, data.as_ref())?;
            // the clusters: the components of the zones matrix's graph (labelled from the lowest zone)
            let mut label = vec![usize::MAX; nz];
            let mut next = 0;
            for z in 0..nz {
                if label[z] != usize::MAX {
                    continue;
                }
                let mut stack = vec![z];
                label[z] = next;
                while let Some(a) = stack.pop() {
                    for c in 0..nz {
                        if c != a && zm[a][c] != 0.0 && label[c] == usize::MAX {
                            label[c] = next;
                            stack.push(c);
                        }
                    }
                }
                next += 1;
            }
            for c in 0..next {
                let members: Vec<usize> = (0..nz).filter(|&z| label[z] == c && zm[z][z] > 0.0).collect();
                if members.is_empty() {
                    continue;
                }
                let nodes: Vec<usize> = if members.len() == 1 {
                    (members[0] * n..(members[0] + 1) * n).collect()
                } else {
                    // a unit is in the cluster when it is in any of its zones: y_i = sum_z x_zi
                    let vt = b.vtype[0];
                    let first = b.add_columns(&vec![0.0; n], 0.0, 1.0, vt, "cluster");
                    let (mut r, mut cc, mut v) = (vec![], vec![], vec![]);
                    for i in 0..n {
                        r.push(i);
                        cc.push(first + i);
                        v.push(1.0);
                        for &z in &members {
                            r.push(i);
                            cc.push(z * n + i);
                            v.push(-1.0);
                        }
                    }
                    b.add_rows(&r, &cc, &v, &vec!["=".to_string(); n], &vec![0.0; n], &vec!["cluster".to_string(); n]);
                    (first..first + n).collect()
                };
                add_connected(b, &nodes, &d, &format!("c{}", c + 1));
            }
        }
    }
    Ok(())
}

/// Values of every unit in every zone (zones x units; no value 0): one dataset per zone (a value per cell or row).
fn zone_data(u: &Units, data: &[Vec<f64>], what: &str) -> Result<Vec<Vec<f64>>, String> {
    let nz = u.nz();
    let per: Vec<&Vec<f64>> = match data.len() {
        1 => vec![&data[0]; nz],
        k if k == nz => data.iter().collect(),
        _ => return Err(format!("{what}: one dataset, or one per zone ({nz})")),
    };
    per.iter()
        .map(|d| {
            if d.len() != u.size {
                return Err(format!("{what}: a value per cell or row ({})", u.size));
            }
            Ok(u.cells.iter().map(|&c| if d[c].is_nan() { 0.0 } else { d[c] }).collect())
        })
        .collect()
}

/// The summaries of a solution with zones (s: every unit in every zone, zone after zone), prioritizr's eval_*
/// with zones: the overall value, then every zone's.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneEvaluation {
    /// eval_n_summary and eval_cost_summary: R's sums
    pub n: Vec<f64>,
    pub cost: Vec<f64>,
    /// eval_boundary_summary (None: table units without a boundary matrix)
    pub boundary: Option<Vec<f64>>,
    /// eval_feature_representation_summary: every feature's total and amount held, overall then per zone (zone
    /// after zone)
    pub total: Vec<f64>,
    pub held: Vec<f64>,
}

/// A slice's values as i64 (the bridges' indices).
fn i64s(v: &[usize]) -> Vec<i64> {
    v.iter().map(|&x| x as i64).collect()
}

/// The units' boundary matrix rounded as prioritizr's eval does, its strictly lower triangle (zeros left out),
/// every unit's exposed and total boundary.
fn boundary_parts(mut bm: Csc) -> (Csc, Vec<f64>, Vec<f64>) {
    bm.value.iter_mut().for_each(|v| *v = r_round(*v, 6));
    let total = bm.diagonal();
    let exposed: Vec<f64> = total.iter().zip(bm.row_sums()).map(|(t, r)| t - (r - t)).collect();
    let mut lower = Csc { n: bm.n, start: vec![0], index: Vec::new(), value: Vec::new() };
    for j in 0..bm.n {
        for e in bm.start[j]..bm.start[j + 1] {
            if bm.index[e] > j && bm.value[e] != 0.0 {
                lower.index.push(bm.index[e]);
                lower.value.push(bm.value[e]);
            }
        }
        lower.start.push(lower.index.len());
    }
    (lower, exposed, total)
}

/// The summaries of a solution with zones: edge factors one or one per zone, a zones matrix for the boundary
/// (none: the identity), a boundary matrix of every unit (none: the raster's).
pub fn evaluate_zones(
    u: &Units,
    s: &[f64],
    edge_factor: &[f64],
    zones: &Option<Vec<Vec<f64>>>,
    data: Option<&Matrix>,
) -> Result<ZoneEvaluation, String> {
    let (nu, nz, nf) = (u.units(), u.nz(), u.names.len());
    let ef = per_zone(nz, edge_factor, "edge_factor")?;
    if ef.iter().any(|e| !(0.0..=1.0).contains(e)) {
        return Err("edge_factor must be between 0 and 1".into());
    }
    let zm = zone_matrix(nz, zones, "the boundary summary", false, true)?;
    let sz = |z: usize| &s[z * nu..(z + 1) * nu];
    let mut n = vec![ffi::r_sum(s)];
    n.extend((0..nz).map(|z| ffi::r_sum(sz(z))));
    // colSums(cost * status, na.rm = TRUE)
    let costs: Vec<f64> = (0..nz)
        .map(|z| {
            let prod: Vec<f64> =
                (0..nu).map(|i| u.cost[z * nu + i] * s[z * nu + i]).filter(|v| v.is_finite()).collect();
            ffi::r_sum(&prod)
        })
        .collect();
    let mut cost = vec![ffi::r_sum(&costs)];
    cost.extend(costs);
    let bm = match (data, u.raster) {
        (Some(d), _) => Some(d.units(u, "the boundary matrix")?),
        (None, true) => Some(u.boundary_matrix()),
        (None, false) => None,
    };
    let boundary = match bm {
        None => None,
        Some(bm) => {
            let (lower, exposed, total) = boundary_parts(bm);
            let (st, ix) = (i64s(&lower.start), i64s(&lower.index));
            let flat: Vec<f64> = zm.iter().flatten().copied().collect();
            let mut out = vec![
                ffi::eval_boundary_zones(&ef, &flat, &st, &ix, &lower.value, &exposed, &total, s)
                    .map_err(|e| e.what().to_string())?,
            ];
            for (z, &e) in ef.iter().enumerate() {
                out.push(
                    ffi::eval_boundary(e, &st, &ix, &lower.value, &exposed, &total, sz(z))
                        .map_err(|e| e.what().to_string())?,
                );
            }
            Some(out)
        }
    };
    // every feature's amount held in every zone (Matrix's rij %*% s: the units in order, zeros of s left out)
    let held_z: Vec<Vec<f64>> = (0..nz)
        .map(|z| {
            let b = u.zone_rij(z);
            let s = sz(z);
            (0..nf)
                .map(|f| {
                    let (idx, v) = b.row(f);
                    idx.iter().zip(v).filter(|(i, _)| s[**i] != 0.0).fold(0.0, |a, (i, x)| a + x * s[*i])
                })
                .collect()
        })
        .collect();
    let mut total: Vec<f64> = (0..nf).map(|f| ffi::r_sum(&u.zone_totals[f])).collect();
    let mut held: Vec<f64> = (0..nf).map(|f| ffi::r_sum(&(0..nz).map(|z| held_z[z][f]).collect::<Vec<_>>())).collect();
    for (z, hz) in held_z.iter().enumerate() {
        total.extend((0..nf).map(|f| u.zone_totals[f][z]));
        held.extend_from_slice(hz);
    }
    Ok(ZoneEvaluation { n, cost, boundary, total, held })
}

/// eval_connectivity_summary with zones (eval_asym_connectivity_summary with asym): the overall connectivity (a
/// zones matrix, none: the identity), then every zone's; and whether the matrix is symmetric.
pub fn connectivity_zones(
    u: &Units,
    data: &Matrix,
    s: &[f64],
    zones: &Option<Vec<Vec<f64>>>,
    asym: bool,
) -> Result<(Vec<f64>, bool), String> {
    let (nu, nz) = (u.units(), u.nz());
    let zm = zone_matrix(nz, zones, "the connectivity summary", false, false)?;
    let m = data.units(u, "the connectivity matrix")?;
    let (st, ix) = (i64s(&m.start), i64s(&m.index));
    let flat: Vec<f64> = zm.iter().flatten().copied().collect();
    let mut out =
        vec![ffi::eval_connectivity_zones(&st, &ix, &m.value, &flat, s, asym).map_err(|e| e.what().to_string())?];
    for z in 0..nz {
        out.push(
            ffi::eval_connectivity(&st, &ix, &m.value, &s[z * nu..(z + 1) * nu], asym)
                .map_err(|e| e.what().to_string())?,
        );
    }
    Ok((out, m.symmetric()))
}

/// eval_target_coverage_summary's amounts held with zones: every zone of a target in turn, Armadillo's sum of
/// its non-zero products.
pub fn zone_targets_held(u: &Units, t: &Targets, s: &[f64]) -> Vec<f64> {
    let nu = u.units();
    let blocks: Vec<Csr> = (0..u.nz()).map(|z| u.zone_rij(z)).collect();
    t.feature
        .iter()
        .zip(&t.zone)
        .map(|(&f, zs)| {
            let mut held = 0.0;
            for &z in zs {
                let (idx, v) = blocks[z].row(f);
                let prod: Vec<f64> =
                    idx.iter().zip(v).map(|(&i, &x)| x * s[z * nu + i]).filter(|p| *p != 0.0).collect();
                held += ffi::eval_accumulate(&prod);
            }
            held
        })
        .collect()
}
