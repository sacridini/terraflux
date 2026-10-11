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
