//! Fits by reduction: what a fit needs from the whole map is summed over the tiles, read one at a time and in
//! parallel (here or on other machines). Markov chains and weights of evidence come out as the exact
//! engine's, to the bit; the patches observed are measured within tiles (patches crossing a tile's edge count
//! as two).

use std::collections::BTreeMap;

use terraflux_ls_sys::ffi;
use terraflux_ops::woe::{
    Extent, Settings, WoeModel, entry_from_counts, fit_buckets, layout_from, observed_transitions, variable_specs,
};
use terraflux_ops::{Markov, PatchParams, value::PatchTransition};

use crate::pass::{Part, Task};
use crate::tiles::{Source, Tile, Tiling};
use crate::{Error, Result};

fn check_grids(sources: &[&dyn Source], tiling: &Tiling) -> Result<()> {
    for s in sources {
        let g = s.grid();
        if g.width != tiling.width || g.height != tiling.height {
            return Err(Error::Msg(format!(
                "a map has {} x {} cells, the tiling {} x {}",
                g.width, g.height, tiling.width, tiling.height
            )));
        }
    }
    Ok(())
}

fn unexpected() -> Error {
    Error::Msg("a pass gave a part of another kind".into())
}

/// The fits' tasks on one tile.
pub(crate) fn on_tile(task: &Task, t: &Tile, src: &[&dyn Source]) -> Result<Part> {
    let read = |i: usize| src[i].read(t.window(0));
    Ok(match task {
        Task::Classes => {
            let mut v = Vec::new();
            for i in 0..src.len() {
                v.extend(ffi::map_classes(&read(i)?));
            }
            v.sort_by(f64::total_cmp);
            v.dedup();
            Part::Values(v)
        }
        Task::Crosstab { classes } => Part::Counts(ffi::crosstab(&read(0)?, &read(1)?, classes, classes)?),
        Task::Counts { values } => {
            let mut m = BTreeMap::new();
            for x in read(0)? {
                if values.contains(&x) {
                    *m.entry(x.to_bits()).or_insert(0) += 1;
                }
            }
            Part::ByValue(m)
        }
        Task::WoeExtents { froms, specs } => {
            let (a, b) = (read(0)?, read(1)?);
            let d: Vec<Vec<f64>> = (0..specs.len()).map(|k| read(2 + k)).collect::<Result<_>>()?;
            let mut out = Vec::with_capacity(froms.len() * specs.len());
            for &f in froms {
                let cells: Vec<bool> = a.iter().zip(&b).map(|(x, y)| !x.is_nan() && !y.is_nan() && *x == f).collect();
                for (k, v) in specs.iter().enumerate() {
                    out.push(Extent::of(&d[k], &cells, v.categorical));
                }
            }
            Part::Extents(out)
        }
        Task::WoeCounts { froms, specs, layouts, fin } => {
            let (a, b) = (read(0)?, read(1)?);
            let nv = specs.len();
            let d: Vec<Vec<f64>> = (0..nv).map(|k| read(2 + k)).collect::<Result<_>>()?;
            let mut out = Vec::with_capacity(layouts.len());
            for (fi, &f) in froms.iter().enumerate() {
                for k in 0..nv {
                    let lay = &layouts[fi * nv + k];
                    let buckets = fit_buckets(&d[k], lay, &specs[k])?;
                    out.push(ffi::woe_counts(&a, &b, &buckets, lay.n as i64, f, fin)?);
                }
            }
            Part::Buckets(out)
        }
        Task::Patches { transitions } => {
            let (a, b) = (read(0)?, read(1)?);
            let parts = transitions
                .iter()
                .map(|&(f, to)| {
                    let o = ffi::observed_patches(&a, &b, t.height as i64, t.width as i64, f, to)?;
                    // the cells in expansions: a whole number
                    let exp_cells = (o.expansion * o.cells as f64).round() as i64;
                    Ok((
                        o.cells,
                        exp_cells,
                        (o.expansion_patches, o.expansion_mean, o.expansion_variance),
                        (o.new_patches, o.new_mean, o.new_variance),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            Part::Patches(vec![(t.index, parts)])
        }
        _ => return Err(Error::Msg("not a fit's task".into())),
    })
}

/// The sorted distinct values of some maps.
pub fn classes(sources: &[&dyn Source], tiling: &Tiling) -> Result<Vec<f64>> {
    terraflux_ops::threads::ensure();
    check_grids(sources, tiling)?;
    match tiling.run(sources, &Task::Classes)? {
        Part::Values(v) => Ok(v),
        Part::Nothing => Ok(Vec::new()),
        _ => Err(unexpected()),
    }
}

/// The cells of each class of `a` holding each class of `b` (classes x classes).
pub fn crosstab(a: &dyn Source, b: &dyn Source, classes: &[f64], tiling: &Tiling) -> Result<Vec<i64>> {
    terraflux_ops::threads::ensure();
    match tiling.run(&[a, b], &Task::Crosstab { classes: classes.to_vec() })? {
        Part::Counts(v) => Ok(v),
        Part::Nothing => Ok(vec![0; classes.len() * classes.len()]),
        _ => Err(unexpected()),
    }
}

/// The Markov chain of the change from `t0` to `t1` (as `markov.fit`).
pub fn markov(t0: &dyn Source, t1: &dyn Source, tiling: &Tiling, steps: usize) -> Result<Markov> {
    terraflux_ops::threads::ensure();
    let classes = classes(&[t0, t1], tiling)?;
    let counts: Vec<f64> = crosstab(t0, t1, &classes, tiling)?.into_iter().map(|v| v as f64).collect();
    let mut kept = Vec::new();
    let single = ffi::markov_from_counts(&counts, classes.len() as i32, &mut kept)?;
    let kept: Vec<f64> = kept.iter().map(|&i| classes[i as usize]).collect();
    let multi = ffi::markov_root(&single, kept.len() as i32, steps as i32)?;
    Ok(Markov { classes: kept, single, multi, steps })
}

/// Weights of evidence (as `woe.fit`): the extent of each variable over each initial class and the counts of
/// its buckets are summed over the tiles.
pub fn woe(
    t0: &dyn Source,
    t1: &dyn Source,
    drivers: &[(String, &dyn Source)],
    tiling: &Tiling,
    s: &Settings,
    transitions: Option<Vec<(f64, f64)>>,
) -> Result<WoeModel> {
    terraflux_ops::threads::ensure();
    let mut all: Vec<&dyn Source> = vec![t0, t1];
    all.extend(drivers.iter().map(|(_, d)| *d));
    check_grids(&all, tiling)?;
    let mut order: Vec<usize> = (0..drivers.len()).collect();
    order.sort_by(|&a, &b| drivers[a].0.cmp(&drivers[b].0));
    let variables: Vec<String> = order.iter().map(|&i| drivers[i].0.clone()).collect();
    let specs = variable_specs(&variables, s)?;
    let transitions = match transitions.or_else(|| s.transitions.clone()) {
        Some(t) => t,
        None => {
            let cls = classes(&[t0, t1], tiling)?;
            observed_transitions(&cls, &crosstab(t0, t1, &cls, tiling)?)
        }
    };
    let fin = classes(&[t1], tiling)?;
    let mut froms: Vec<f64> = transitions.iter().map(|t| t.0).collect();
    froms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    froms.dedup();
    let nv = variables.len();
    // t0, t1 and the drivers in the order of the variables
    let mut sources: Vec<&dyn Source> = vec![t0, t1];
    sources.extend(order.iter().map(|&i| drivers[i].1));
    // the extent of each variable over the cells of each initial class
    let extents = match tiling.run(&sources, &Task::WoeExtents { froms: froms.clone(), specs: specs.clone() })? {
        Part::Extents(e) => e,
        Part::Nothing => vec![Extent::default(); froms.len() * nv],
        _ => return Err(unexpected()),
    };
    let layouts: Vec<terraflux_ops::woe::Layout> = (0..froms.len() * nv)
        .map(|i| layout_from(&specs[i % nv], &extents[i]))
        .collect::<terraflux_ops::Result<_>>()?;
    // the cells of each bucket by final class
    let task =
        Task::WoeCounts { froms: froms.clone(), specs: specs.clone(), layouts: layouts.clone(), fin: fin.clone() };
    let counts = match tiling.run(&sources, &task)? {
        Part::Buckets(c) => c,
        Part::Nothing => layouts.iter().map(|l| vec![0i64; l.n * fin.len()]).collect(),
        _ => return Err(unexpected()),
    };
    let mut entries = Vec::new();
    let mut rows = Vec::new();
    for &(f, t) in &transitions {
        let fi = froms.iter().position(|&x| x == f).unwrap();
        for k in 0..nv {
            let (e, r) = entry_from_counts(
                f,
                t,
                &variables[k],
                specs[k].clone(),
                layouts[fi * nv + k].clone(),
                &counts[fi * nv + k],
                &fin,
                s,
            )?;
            entries.push(e);
            rows.extend(r);
        }
    }
    Ok(WoeModel { transitions, entries, rows, fix_abnormal: s.fix_abnormal })
}

/// Size statistics of patches merged over tiles: count, sum and sum of squares of the sizes.
#[derive(Debug, Clone, Copy, Default)]
struct Moments {
    n: f64,
    sum: f64,
    sq: f64,
}

impl Moments {
    fn add_mean_var(&mut self, n: i64, mean: f64, var: f64) {
        if n == 0 {
            return;
        }
        let n = n as f64;
        self.n += n;
        self.sum += n * mean;
        self.sq += n * (var + mean * mean);
    }

    fn mean_var(&self) -> (f64, f64) {
        if self.n == 0.0 {
            return (1.0, 0.0);
        }
        let m = self.sum / self.n;
        (m, (self.sq / self.n - m * m).max(0.0))
    }
}

/// The Expander and Patcher settings measured on the observed change (as `patches.observe`), the patches
/// measured within each tile: the share of expansions exactly (whole cells), the sizes' mean and variance from
/// the tiles' (exactly the exact engine's when one tile holds them all).
pub fn patches(
    t0: &dyn Source,
    t1: &dyn Source,
    tiling: &Tiling,
    transitions: &[(f64, f64)],
    expansion: Option<f64>,
    isometry: f64,
) -> Result<PatchParams> {
    terraflux_ops::threads::ensure();
    check_grids(&[t0, t1], tiling)?;
    // the tiles' parts in the order of the tiles
    let per: Vec<Vec<crate::pass::PatchPart>> =
        match tiling.run(&[t0, t1], &Task::Patches { transitions: transitions.to_vec() })? {
            Part::Patches(mut p) => {
                p.sort_by_key(|x| x.0);
                p.into_iter().map(|x| x.1).collect()
            }
            Part::Nothing => Vec::new(),
            _ => return Err(unexpected()),
        };
    let mut out = Vec::new();
    // the mean and variance of the sizes over the tiles (a tile's own when it holds them all)
    let combine = |parts: &[(i64, f64, f64)]| -> (f64, f64) {
        let with: Vec<&(i64, f64, f64)> = parts.iter().filter(|p| p.0 > 0).collect();
        if with.len() == 1 {
            return (with[0].1, with[0].2);
        }
        let mut m = Moments::default();
        for p in with {
            m.add_mean_var(p.0, p.1, p.2);
        }
        m.mean_var()
    };
    for (k, &(from, to)) in transitions.iter().enumerate() {
        let cells: i64 = per.iter().map(|t| t[k].0).sum();
        let exp_cells: i64 = per.iter().map(|t| t[k].1).sum();
        let (em, ev) = combine(&per.iter().map(|t| t[k].2).collect::<Vec<_>>());
        let (nm, nv) = combine(&per.iter().map(|t| t[k].3).collect::<Vec<_>>());
        out.push(PatchTransition {
            from,
            to,
            expansion: expansion.unwrap_or(if cells > 0 { exp_cells as f64 / cells as f64 } else { 0.0 }),
            expander: [em, ev, isometry],
            patcher: [nm, nv, isometry],
        });
    }
    Ok(PatchParams { transitions: out })
}

/// The cells of each value of a map (by value).
pub fn counts(land: &dyn Source, tiling: &Tiling, values: &[f64]) -> Result<BTreeMap<u64, i64>> {
    terraflux_ops::threads::ensure();
    match tiling.run(&[land], &Task::Counts { values: values.to_vec() })? {
        Part::ByValue(m) => Ok(m),
        Part::Nothing => Ok(BTreeMap::new()),
        _ => Err(unexpected()),
    }
}
