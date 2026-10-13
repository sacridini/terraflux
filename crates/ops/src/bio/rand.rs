//! Randomisations of a spatial analysis (landschaft's bio/_randomise.py, Biodiverse::Randomise): each iteration
//! randomises the basedata, runs the analysis again with the same neighbourhoods and counts, per group and
//! index, how often the observed value exceeds (C), ties (T) and is compared with (Q) the randomised one, with
//! the sums for z-scores; from them the p-ranks, z-scores and CANAPE categories, as Biodiverse gives them.

use std::collections::{HashMap, HashSet};

use cxx::UniquePtr;
use terraflux_ls_sys::ffi;

use super::basedata::BaseData;
use super::conditions::{Condition, definition_query, neighbours};
use super::names::{numify, perl_number, split_element};
use super::properties::Properties;
use super::spatial::{Run, SpatialResult, evaluate, label_ranges};
use super::tree::Tree;
use crate::{Error, Result};

/// Biodiverse's DEFAULT_PRECISION_SMALL.
const EPS: f64 = 1e-10;

/// The randomisation functions.
pub const FUNCTIONS: [&str; 9] = [
    "rand_nochange",
    "rand_csr_by_group",
    "rand_structured",
    "rand_spatially_structured",
    "rand_diffusion",
    "rand_random_walk",
    "rand_independent_swaps",
    "rand_independent_swaps_modified",
    "rand_curveball",
];

/// The arguments of a randomisation (None: the function's default).
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub richness_multiplier: Option<f64>,
    pub richness_addition: Option<f64>,
    pub spatial_conditions_for_label_allocation: Option<String>,
    pub spatial_allocation_order: Option<String>,
    pub label_allocation_backtracking: Option<String>,
    pub spatial_allocation_reseed_prob: Option<f64>,
    pub spatial_conditions_for_seed_location: Option<String>,
    pub swap_count: Option<i64>,
    pub max_swap_attempts: Option<i64>,
    pub stop_on_all_swapped: bool,
    pub use_max_swap: bool,
    pub spatial_conditions_for_subset: Option<String>,
    pub definition_query: Option<String>,
    pub labels_not_to_randomise: Vec<String>,
    pub randomise_trees_by: Option<String>,
    /// no_change (default), by_set or by_item.
    pub randomise_group_props_by: Option<String>,
}

/// Whether a condition has more than whitespace and comments.
fn has_text(c: Option<&str>) -> bool {
    c.is_some_and(|t| t.lines().any(|l| !l.split('#').next().unwrap_or("").trim().is_empty()))
}

fn with_counts(bd: &BaseData, c: ffi::BioCounts) -> BaseData {
    let mut out = bd.clone();
    out.indptr = c.indptr;
    out.indices = c.indices;
    out.counts = c.counts;
    out
}

/// A basedata of some groups (and labels), as Biodiverse builds subsets.
fn subset(
    bd: &BaseData,
    groups: &[usize],
    labels: Option<&[usize]>,
    group_props: bool,
) -> (BaseData, Vec<usize>, Vec<usize>) {
    let labels: Vec<usize> = match labels {
        Some(l) => l.to_vec(),
        None => {
            let mut l: Vec<usize> = groups
                .iter()
                .flat_map(|&g| bd.indices[bd.indptr[g] as usize..bd.indptr[g + 1] as usize].iter().map(|&x| x as usize))
                .collect();
            l.sort_unstable();
            l.dedup();
            l
        }
    };
    let mut new_label = vec![-1i64; bd.n_labels()];
    for (k, &l) in labels.iter().enumerate() {
        new_label[l] = k as i64;
    }
    let n_axes = bd.n_axes();
    let (mut indptr, mut indices, mut counts) = (vec![0i64], Vec::new(), Vec::new());
    for &g in groups {
        for e in bd.indptr[g] as usize..bd.indptr[g + 1] as usize {
            let nl = new_label[bd.indices[e] as usize];
            if nl >= 0 {
                indices.push(nl as i32);
                counts.push(bd.counts[e]);
            }
        }
        indptr.push(indices.len() as i64);
    }
    let sub = BaseData {
        groups: groups.iter().map(|&g| bd.groups[g].clone()).collect(),
        labels: labels.iter().map(|&l| bd.labels[l].clone()).collect(),
        indptr,
        indices,
        counts,
        coords: groups.iter().flat_map(|&g| bd.coords[g * n_axes..(g + 1) * n_axes].to_vec()).collect(),
        cell_sizes: bd.cell_sizes.clone(),
        cell_origins: bd.cell_origins.clone(),
        crs: bd.crs.clone(),
        group_axes: if bd.group_axes.is_empty() {
            Vec::new()
        } else {
            groups.iter().map(|&g| bd.group_axes[g].clone()).collect()
        },
        label_props: Vec::new(),
        group_props: if group_props { rows_of(&bd.group_props, groups) } else { Vec::new() },
    };
    (sub, groups.to_vec(), labels)
}

/// Merged randomised parts (basedata, group indices, label indices) as the full basedata's counts.
fn place(bd: &BaseData, parts: &[(&BaseData, &[usize], &[usize])]) -> BaseData {
    let mut triplets: Vec<(usize, usize, f64)> = Vec::new();
    for (sbd, g_idx, l_idx) in parts {
        for g in 0..sbd.n_groups() {
            for e in sbd.indptr[g] as usize..sbd.indptr[g + 1] as usize {
                triplets.push((g_idx[g], l_idx[sbd.indices[e] as usize], sbd.counts[e]));
            }
        }
    }
    triplets.sort_by_key(|t| (t.0, t.1));
    let mut merged: Vec<(usize, usize, f64)> = Vec::new();
    for t in triplets {
        match merged.last_mut() {
            Some(m) if m.0 == t.0 && m.1 == t.1 => m.2 += t.2,
            _ => merged.push(t),
        }
    }
    let mut indptr = vec![0i64; bd.n_groups() + 1];
    for t in &merged {
        indptr[t.0 + 1] += 1;
    }
    for g in 0..bd.n_groups() {
        indptr[g + 1] += indptr[g];
    }
    let mut out = bd.clone();
    out.indptr = indptr;
    out.indices = merged.iter().map(|t| t.1 as i32).collect();
    out.counts = merged.iter().map(|t| t.2).collect();
    out
}

/// What a randomisation keeps across iterations.
#[derive(Default)]
struct State {
    /// The spatial allocation of rand_structured (built once; None without allocation conditions).
    alloc: Option<UniquePtr<ffi::BioAlloc>>,
    subsets: Option<Subsets>,
    constant: Option<Box<Constant>>,
}

struct Subsets {
    stored: HashMap<usize, Vec<usize>>,
    failed: Vec<usize>,
    cache: HashMap<usize, (BaseData, Vec<usize>, Vec<usize>)>,
    failed_part: Option<(BaseData, Vec<usize>, Vec<usize>)>,
    /// The failed groups' properties as last randomised (by_set starts from them).
    failed_props: Option<Properties>,
    leftovers: Option<(BaseData, Vec<usize>, Vec<usize>)>,
}

struct Constant {
    nbd: BaseData,
    g_idx: Vec<usize>,
    l_idx: Vec<usize>,
    /// The counts of the labels kept in place (every group, those labels).
    kept: (BaseData, Vec<usize>, Vec<usize>),
    state: State,
}

/// The allocation state of rand_structured (landschaft's _spatial_alloc).
fn spatial_alloc(bd: &BaseData, o: &Options, order: &str, backtracking: &str) -> Result<UniquePtr<ffi::BioAlloc>> {
    let conds = o.spatial_conditions_for_label_allocation.as_deref();
    if !has_text(conds) {
        return Ok(ffi::bio_alloc_none());
    }
    let conds = vec![Condition::new(conds.unwrap())?];
    let nb = neighbours(bd, &conds, None, true, true, None)?;
    let n = bd.n_groups();
    let rows: HashMap<i32, usize> = nb.groups.iter().enumerate().map(|(r, &g)| (g, r)).collect();
    let n_sets = nb.stored.len();
    let (mut ptr, mut idx) = (vec![0i64], Vec::new());
    for g in 0..n {
        for (ip, ix) in &nb.stored {
            if let Some(&r) = rows.get(&(g as i32)) {
                let mut l = ix[ip[r] as usize..ip[r + 1] as usize].to_vec();
                l.sort_unstable();
                idx.extend(l);
            }
            ptr.push(idx.len() as i64);
        }
    }
    let same = conds.iter().all(|c| {
        let t = c.result_type();
        t.starts_with("always_same") || t.starts_with("always_true")
    });
    let ndim = bd.n_axes();
    let coords: Vec<f64> = bd
        .groups
        .iter()
        .flat_map(|name| {
            let mut v: Vec<f64> = split_element(name)
                .iter()
                .map(|s| {
                    let x = numify(s);
                    if x.is_nan() && !s.trim().eq_ignore_ascii_case("nan") { 0.0 } else { x }
                })
                .collect();
            v.resize(ndim, 0.0);
            v
        })
        .collect();
    let mut have_seeds = false;
    let mut seeds: Vec<i32> = Vec::new();
    if let Some(sc) = o.spatial_conditions_for_seed_location.as_deref().filter(|s| has_text(Some(s))) {
        let sc = Condition::new(sc)?;
        if sc.result_type() != "always_false" {
            have_seeds = true;
            seeds = definition_query(bd, &sc, true)?
                .iter()
                .enumerate()
                .filter(|(_, p)| **p)
                .map(|(g, _)| g as i32)
                .collect();
        }
    }
    Ok(ffi::bio_alloc_new(
        order,
        backtracking,
        o.spatial_allocation_reseed_prob.unwrap_or(0.0),
        n as i64,
        n_sets as i32,
        &ptr,
        &idx,
        same,
        &coords,
        ndim as i32,
        have_seeds,
        &seeds,
    )?)
}

/// One randomisation function on a basedata.
fn randomise_once(
    bd: &BaseData,
    rng: &mut UniquePtr<ffi::BioRng>,
    function: &str,
    o: &Options,
    state: &mut State,
) -> Result<BaseData> {
    let rng = rng.pin_mut();
    match function {
        "rand_nochange" => Ok(bd.clone()),
        "rand_csr_by_group" => {
            // group i's labels go to group perm[i]
            let perm = ffi::bio_rng_shuffle(rng, bd.n_groups() as i64);
            let mut inverse = vec![0usize; perm.len()];
            for (i, &p) in perm.iter().enumerate() {
                inverse[p as usize] = i;
            }
            let (mut indptr, mut indices, mut counts) = (vec![0i64], Vec::new(), Vec::new());
            for &src in &inverse {
                let r = bd.indptr[src] as usize..bd.indptr[src + 1] as usize;
                indices.extend_from_slice(&bd.indices[r.clone()]);
                counts.extend_from_slice(&bd.counts[r]);
                indptr.push(indices.len() as i64);
            }
            Ok(with_counts(bd, ffi::BioCounts { indptr, indices, counts }))
        }
        "rand_structured" | "rand_spatially_structured" | "rand_diffusion" | "rand_random_walk" => {
            let mut o = o.clone();
            let default = Some("sp_select_all()".to_string());
            let (order, backtracking) = match function {
                "rand_structured" => (
                    o.spatial_allocation_order.clone().unwrap_or_default(),
                    o.label_allocation_backtracking.clone().unwrap_or_default(),
                ),
                "rand_spatially_structured" => {
                    o.spatial_conditions_for_label_allocation = o.spatial_conditions_for_label_allocation.or(default);
                    (
                        o.spatial_allocation_order.clone().unwrap_or_else(|| "diffusion".into()),
                        o.label_allocation_backtracking.clone().unwrap_or_else(|| "from_end".into()),
                    )
                }
                "rand_diffusion" => {
                    o.spatial_conditions_for_label_allocation = o.spatial_conditions_for_label_allocation.or(default);
                    ("diffusion".into(), o.label_allocation_backtracking.clone().unwrap_or_default())
                }
                _ => {
                    o.spatial_conditions_for_label_allocation = o.spatial_conditions_for_label_allocation.or(default);
                    ("random_walk".into(), o.label_allocation_backtracking.clone().unwrap_or_else(|| "from_end".into()))
                }
            };
            if state.alloc.is_none() {
                state.alloc = Some(spatial_alloc(bd, &o, &order, &backtracking)?);
            }
            let alloc = state.alloc.as_mut().unwrap();
            let c = ffi::bio_rand_structured(
                &bd.indptr,
                &bd.indices,
                &bd.counts,
                bd.n_groups() as i64,
                bd.n_labels() as i64,
                rng,
                o.richness_multiplier.unwrap_or(1.0),
                o.richness_addition.unwrap_or(0.0),
                alloc.pin_mut(),
            )?;
            Ok(with_counts(bd, c))
        }
        "rand_independent_swaps" | "rand_independent_swaps_modified" | "rand_curveball" => {
            let range = label_ranges(bd).0;
            let c = ffi::bio_rand_swaps(
                &bd.indptr,
                &bd.indices,
                &bd.counts,
                bd.n_groups() as i64,
                bd.n_labels() as i64,
                rng,
                &range,
                function.trim_start_matches("rand_"),
                o.swap_count.unwrap_or(0),
                o.max_swap_attempts.unwrap_or(0),
                o.stop_on_all_swapped,
                o.use_max_swap,
            )?;
            Ok(with_counts(bd, c))
        }
        f => Err(Error::msg(format!("unknown randomisation {f:?} (available: {})", FUNCTIONS.join(", ")))),
    }
}

/// get_rand_structured_subset: each subset of groups randomised on its own, then merged.
fn randomise_subsets(
    bd: &BaseData,
    rng: &mut UniquePtr<ffi::BioRng>,
    function: &str,
    o: &Options,
    state: &mut State,
) -> Result<BaseData> {
    if state.subsets.is_none() {
        let cond = o
            .spatial_conditions_for_subset
            .clone()
            .filter(|c| has_text(Some(c)))
            .unwrap_or_else(|| "sp_select_all()".into());
        let dq = o.definition_query.as_deref().filter(|q| has_text(Some(q))).map(Condition::new).transpose()?;
        let nb = neighbours(bd, &[Condition::new(&cond)?], dq.as_ref(), true, true, None)?;
        let (ip, ix) = &nb.stored[0];
        let stored: HashMap<usize, Vec<usize>> = nb
            .groups
            .iter()
            .enumerate()
            .map(|(r, &g)| (g as usize, ix[ip[r] as usize..ip[r + 1] as usize].iter().map(|&x| x as usize).collect()))
            .collect();
        let failed: Vec<usize> =
            if dq.is_some() { (0..bd.n_groups()).filter(|g| !stored.contains_key(g)).collect() } else { Vec::new() };
        state.subsets = Some(Subsets {
            stored,
            failed,
            cache: HashMap::new(),
            failed_part: None,
            failed_props: None,
            leftovers: None,
        });
    }
    let s = state.subsets.as_mut().unwrap();
    let n = bd.n_groups();
    let mut done = vec![false; n];
    let mut parts: Vec<(BaseData, Vec<usize>, Vec<usize>)> = Vec::new();
    let how = o.randomise_group_props_by.as_deref();
    let mut props: Vec<(Vec<usize>, Properties)> = Vec::new();
    if !s.failed.is_empty() {
        if s.failed_part.is_none() {
            s.failed_part = Some(subset(bd, &s.failed, None, true));
        }
        parts.push(s.failed_part.clone().unwrap()); // not randomised
        let fp = shuffle_props(&bd.group_props, &s.failed, rng, how, s.failed_props.as_ref())?;
        props.push((s.failed.clone(), fp.clone()));
        s.failed_props = Some(fp);
        for &g in &s.failed {
            done[g] = true;
        }
    }
    let failed: HashSet<usize> = s.failed.iter().copied().collect();
    for g in 0..n {
        if done.iter().all(|d| *d) {
            break;
        }
        if failed.contains(&g) {
            continue;
        }
        if !s.cache.contains_key(&g) {
            let mut nbrs: Vec<usize> =
                s.stored.get(&g).map_or(Vec::new(), |l| l.iter().copied().filter(|&x| !done[x]).collect());
            if nbrs.is_empty() {
                continue;
            }
            nbrs.sort_unstable();
            s.cache.insert(g, subset(bd, &nbrs, None, true));
        }
        let (sbd, g_idx, l_idx) = &s.cache[&g];
        let rbd = randomise_once(sbd, rng, function, o, &mut State::default())?;
        let all: Vec<usize> = (0..sbd.n_groups()).collect();
        props.push((g_idx.clone(), shuffle_props(&sbd.group_props, &all, rng, how, None)?));
        for &x in g_idx {
            done[x] = true;
        }
        parts.push((rbd, g_idx.clone(), l_idx.clone()));
    }
    if !done.iter().all(|d| *d) {
        if s.leftovers.is_none() {
            let left: Vec<usize> = (0..n).filter(|&g| !done[g]).collect();
            s.leftovers = Some(subset(bd, &left, None, true));
        }
        let (sbd, g_idx, l_idx) = s.leftovers.as_ref().unwrap();
        let rbd = randomise_once(sbd, rng, function, o, &mut State::default())?;
        let all: Vec<usize> = (0..sbd.n_groups()).collect();
        props.push((g_idx.clone(), shuffle_props(&sbd.group_props, &all, rng, how, None)?));
        parts.push((rbd, g_idx.clone(), l_idx.clone()));
    }
    let refs: Vec<(&BaseData, &[usize], &[usize])> =
        parts.iter().map(|(b, g, l)| (b, g.as_slice(), l.as_slice())).collect();
    let mut out = place(bd, &refs);
    if !bd.group_props.is_empty() {
        let mut gp: Properties = bd.group_props.iter().map(|(n, _)| (n.clone(), vec![None; n_groups(bd)])).collect();
        for (g_idx, frame) in &props {
            for (k, (_, col)) in frame.iter().enumerate() {
                for (i, &g) in g_idx.iter().enumerate() {
                    gp[k].1[g] = col[i].clone();
                }
            }
        }
        out.group_props = gp;
    }
    Ok(out)
}

fn n_groups(bd: &BaseData) -> usize {
    bd.n_groups()
}

/// The properties of some elements (rows), in their order.
fn rows_of(p: &Properties, rows: &[usize]) -> Properties {
    p.iter().map(|(n, col)| (n.clone(), rows.iter().map(|&g| col[g].clone()).collect())).collect()
}

/// process_group_props: the group properties of a randomised basedata. no_change keeps them; by_set moves each
/// group's properties together to a random group; by_item moves each property's values independently; `current`
/// holds what the receiving groups had.
fn shuffle_props(
    orig: &Properties,
    rows: &[usize],
    rng: &mut UniquePtr<ffi::BioRng>,
    how: Option<&str>,
    current: Option<&Properties>,
) -> Result<Properties> {
    let how = how.unwrap_or("no_change");
    let how = how.strip_prefix("process_group_props_").unwrap_or(how);
    let mut keys: Vec<&String> =
        orig.iter().filter(|(_, col)| col.iter().any(Option::is_some)).map(|(n, _)| n).collect();
    keys.sort();
    let sub = rows_of(orig, rows);
    if how == "no_change" || keys.is_empty() {
        return Ok(sub);
    }
    let n = rows.len();
    match how {
        "by_set" => {
            let mut out: Properties = match current {
                Some(c) => c.clone(),
                None => orig.iter().map(|(name, _)| (name.clone(), vec![None; n])).collect(),
            };
            let perm = ffi::bio_rng_shuffle(rng.pin_mut(), n as i64);
            for i in 0..n {
                if sub.iter().all(|(_, col)| col[i].is_none()) {
                    continue; // no properties
                }
                for (k, (_, col)) in sub.iter().enumerate() {
                    out[k].1[perm[i] as usize] = col[i].clone();
                }
            }
            Ok(out)
        }
        "by_item" => {
            let mut out: Properties = orig.iter().map(|(name, _)| (name.clone(), vec![None; n])).collect();
            for key in keys {
                let perm = ffi::bio_rng_shuffle(rng.pin_mut(), n as i64);
                let k = sub.iter().position(|(name, _)| name == key).unwrap();
                for (i, &to) in perm.iter().enumerate() {
                    if let Some(v) = &sub[k].1[i] {
                        out[k].1[to as usize] = Some(v.clone());
                    }
                }
            }
            Ok(out)
        }
        h => Err(Error::msg(format!("unknown randomise_group_props_by {h:?} (no_change, by_set or by_item)"))),
    }
}

/// get_randomised_basedata: the labels kept in place, the subsets, then the function.
fn randomised_basedata(
    bd: &BaseData,
    rng: &mut UniquePtr<ffi::BioRng>,
    function: &str,
    o: &Options,
    state: &mut State,
) -> Result<BaseData> {
    let use_subsets = has_text(o.spatial_conditions_for_subset.as_deref()) || has_text(o.definition_query.as_deref());
    let core = |b: &BaseData, rng: &mut UniquePtr<ffi::BioRng>, st: &mut State| -> Result<BaseData> {
        if use_subsets {
            return randomise_subsets(b, rng, function, o, st);
        }
        let mut out = randomise_once(b, rng, function, o, st)?;
        if !b.group_props.is_empty() {
            let all: Vec<usize> = (0..b.n_groups()).collect();
            out.group_props = shuffle_props(&b.group_props, &all, rng, o.randomise_group_props_by.as_deref(), None)?;
        }
        Ok(out)
    };
    let constant: HashSet<usize> =
        o.labels_not_to_randomise.iter().filter_map(|l| bd.labels.binary_search(l).ok()).collect();
    if o.labels_not_to_randomise.is_empty() {
        return core(bd, rng, state);
    }
    if state.constant.is_none() {
        // the other labels, in the groups that have any of them
        let mut range = vec![0usize; bd.n_labels()];
        for &l in &bd.indices {
            range[l as usize] += 1;
        }
        let g_idx: Vec<usize> = (0..bd.n_groups())
            .filter(|&g| {
                bd.indices[bd.indptr[g] as usize..bd.indptr[g + 1] as usize]
                    .iter()
                    .any(|l| !constant.contains(&(*l as usize)))
            })
            .collect();
        let l_idx: Vec<usize> = (0..bd.n_labels()).filter(|l| !constant.contains(l) && range[*l] > 0).collect();
        let (nbd, g_idx, l_idx) = subset(bd, &g_idx, Some(&l_idx), false);
        let const_l: Vec<usize> = (0..bd.n_labels()).filter(|l| constant.contains(l)).collect();
        let all: Vec<usize> = (0..bd.n_groups()).collect();
        let kept = subset(bd, &all, Some(&const_l), false);
        state.constant = Some(Box::new(Constant { nbd, g_idx, l_idx, kept, state: State::default() }));
    }
    let c = state.constant.as_mut().unwrap();
    let rbd = core(&c.nbd, rng, &mut c.state)?;
    let (kept, all, const_l) = &c.kept;
    let mut out = place(bd, &[(&rbd, &c.g_idx, &c.l_idx), (kept, all, const_l)]);
    // Biodiverse's randomised basedata has no properties here
    out.label_props = Vec::new();
    out.group_props = Vec::new();
    Ok(out)
}

/// Comparison counters of one list (or the scalar indices), aligned to the observed values.
#[derive(Debug, Clone)]
pub struct Counts {
    pub observed: Vec<f64>,
    pub c: Vec<f64>,
    pub q: Vec<f64>,
    pub t: Vec<f64>,
    pub sumx: Vec<f64>,
    pub sumxx: Vec<f64>,
}

impl Counts {
    fn new(observed: Vec<f64>) -> Counts {
        let nan = vec![f64::NAN; observed.len()];
        Counts { observed, c: nan.clone(), q: nan.clone(), t: nan.clone(), sumx: nan.clone(), sumxx: nan }
    }

    fn add(&mut self, rand: &[f64]) {
        let z = |v: f64| if v.is_nan() { 0.0 } else { v };
        for (i, &r) in rand.iter().enumerate().take(self.observed.len()) {
            let obs = self.observed[i];
            if obs.is_nan() || r.is_nan() {
                continue;
            }
            let diff = obs - r;
            self.c[i] = z(self.c[i]) + if diff > EPS { 1.0 } else { 0.0 };
            self.q[i] = z(self.q[i]) + 1.0;
            self.sumx[i] = z(self.sumx[i]) + r;
            self.sumxx[i] = z(self.sumxx[i]) + r * r;
            if diff.abs() <= EPS {
                self.t[i] = z(self.t[i]) + 1.0;
            }
        }
    }

    pub fn p(&self) -> Vec<f64> {
        self.c.iter().zip(&self.q).map(|(c, q)| c / q).collect()
    }

    /// The rank of the observed values among the randomised ones: P above 0.5, else the share not below.
    pub fn p_rank(&self) -> Vec<f64> {
        let p = self.p();
        (0..self.c.len())
            .map(|i| {
                if self.c[i].is_nan() {
                    return f64::NAN;
                }
                if p[i] > 0.5 {
                    p[i]
                } else {
                    (self.c[i] + if self.t[i].is_nan() { 0.0 } else { self.t[i] }) / self.q[i]
                }
            })
            .collect()
    }

    /// The standardised difference of the observed values from the randomised ones.
    pub fn z_scores(&self) -> Vec<f64> {
        let n = self.q.len();
        let var: Vec<f64> =
            (0..n).map(|i| (self.sumxx[i] - (self.sumx[i] * self.sumx[i]) / self.q[i]) / self.q[i]).collect();
        let sd = ffi::bio_perl_sqrt(&var.iter().map(|v| if *v > 0.0 { *v } else { 1.0 }).collect::<Vec<_>>());
        (0..n)
            .map(|i| {
                let q = self.q[i];
                if q.is_nan() || q == 0.0 {
                    return f64::NAN;
                }
                if var[i] > 0.0 { (self.observed[i] - self.sumx[i] / q) / sd[i] } else { 0.0 }
            })
            .collect()
    }
}

/// The key of a list entry as a text (Biodiverse's).
fn key_text(kind: i32, k: f64, bd: &BaseData, r: &SpatialResult) -> String {
    let s = |v: &Vec<String>| v.get(k as usize).cloned().unwrap_or_default();
    match kind {
        0 => s(&bd.labels),
        1 | 4 => s(&bd.groups),
        2 => s(&r.texts),
        5 => s(&r.nodes),
        _ => perl_number(k),
    }
}

/// The (group, key) entries and values of a list.
fn list_entries(l: &super::spatial::ListIndex, bd: &BaseData, r: &SpatialResult) -> Vec<((String, String), f64)> {
    r.groups
        .iter()
        .zip(&l.rows)
        .flat_map(|(&g, row)| {
            row.iter().map(move |e| ((bd.groups[g as usize].clone(), key_text(l.kind, e.0, bd, r)), e.1))
        })
        .collect()
}

/// The counters of a hash list: its name, its (group, key) entries and their counts.
pub type ListCounts = (String, Vec<(String, String)>, Counts);

/// Randomisations of a spatial analysis.
pub struct Randomisation {
    pub result: SpatialResult,
    pub function: String,
    pub iterations: usize,
    pub scalars: Counts,
    /// Per hash list: its (group, key) entries and counters.
    pub lists: Vec<ListCounts>,
    bd: BaseData,
    run: Run,
    options: Options,
    rng: UniquePtr<ffi::BioRng>,
    state: State,
}

impl Randomisation {
    /// A randomisation of an analysis (`seed`: Biodiverse's Mersenne Twister's).
    pub fn new(bd: &BaseData, run: Run, function: &str, seed: u64, options: Options) -> Result<Randomisation> {
        if !FUNCTIONS.contains(&function) {
            return Err(Error::msg(format!(
                "unknown randomisation {function:?} (available: {})",
                FUNCTIONS.join(", ")
            )));
        }
        let result = evaluate(bd, &run, None)?;
        let scalars = Counts::new(result.rows.concat());
        let lists = result
            .lists
            .iter()
            .filter(|l| l.kind != 4 && l.kind != 6) // Biodiverse compares hash lists only
            .map(|l| {
                let (keys, values): (Vec<_>, Vec<_>) = list_entries(l, bd, &result).into_iter().unzip();
                (l.name.clone(), keys, Counts::new(values))
            })
            .collect();
        Ok(Randomisation {
            result,
            function: function.to_string(),
            iterations: 0,
            scalars,
            lists,
            bd: bd.clone(),
            run,
            options,
            rng: ffi::bio_rng_new(seed),
            state: State::default(),
        })
    }

    /// The analysis tree for an iteration (`randomise_trees_by`).
    fn shuffled_tree(&mut self) -> Result<Option<Tree>> {
        let how = self.options.randomise_trees_by.clone().unwrap_or_default();
        let Some(tree) = &self.run.tree else { return Ok(None) };
        let how = how.trim_start_matches("shuffle_");
        if how.is_empty() || how == "no_change" {
            return Ok(None);
        }
        if how != "terminal_names" {
            return Err(Error::msg(format!(
                "unknown randomise_trees_by {:?} (shuffle_no_change, shuffle_terminal_names)",
                self.options.randomise_trees_by
            )));
        }
        let mut terminals = tree.terminal_names();
        terminals.sort();
        let perm = ffi::bio_rng_shuffle(self.rng.pin_mut(), terminals.len() as i64);
        let new_of: HashMap<&String, &String> =
            terminals.iter().zip(perm.iter().map(|&p| &terminals[p as usize])).collect();
        let mut t = tree.clone();
        t.names = tree.names.iter().map(|n| new_of.get(n).map_or_else(|| n.clone(), |m| (*m).clone())).collect();
        Ok(Some(t))
    }

    /// More iterations (the random stream continues).
    pub fn run(&mut self, iterations: usize) -> Result<()> {
        for _ in 0..iterations {
            let rbd = randomised_basedata(&self.bd, &mut self.rng, &self.function, &self.options, &mut self.state)?;
            let tree = self.shuffled_tree()?;
            let r = evaluate(&rbd, &self.run, tree.as_ref())?;
            // the randomised table aligned to the observed columns
            let cols: Vec<Option<usize>> =
                self.result.names.iter().map(|n| r.names.iter().position(|m| m == n)).collect();
            let rand: Vec<f64> =
                r.rows.iter().flat_map(|row| cols.iter().map(|c| c.map_or(f64::NAN, |j| row[j]))).collect();
            self.scalars.add(&rand);
            for (name, keys, counts) in &mut self.lists {
                let values: HashMap<(String, String), f64> = r
                    .lists
                    .iter()
                    .find(|l| &l.name == name)
                    .map_or(HashMap::new(), |l| list_entries(l, &rbd, &r).into_iter().collect());
                let rand: Vec<f64> = keys.iter().map(|k| values.get(k).copied().unwrap_or(f64::NAN)).collect();
                counts.add(&rand);
            }
            self.iterations += 1;
        }
        Ok(())
    }

    /// CANAPE categories (Mishler et al. 2014) from the p-ranks of PE and RPE: 0 not significant, 1 neo,
    /// 2 palaeo, 3 mixed, 4 super; None when the analysis lacks pe and phylo_rpe2 (pe_central and
    /// phylo_rpe_central with `central`).
    pub fn canape(&self, central: bool, diff: bool) -> Option<Vec<f64>> {
        let calcs = &self.result.calculations;
        let need = if central { ["pe_central", "phylo_rpe_central"] } else { ["pe", "phylo_rpe2"] };
        if !need.iter().all(|c| calcs.iter().any(|x| x == c)) {
            return None;
        }
        let pe_obs = if central { "PEC_WE" } else { "PE_WE" };
        let pe_alt = match (central, diff) {
            (true, true) => "PHYLO_RPE_DIFFC",
            (false, true) => "PHYLO_RPE_DIFF2",
            (true, false) => "PHYLO_RPE_NULLC",
            (false, false) => "PHYLO_RPE_NULL2",
        };
        let rpe = if central { "PHYLO_RPEC" } else { "PHYLO_RPE2" };
        let k = self.result.names.len();
        let col = |n: &str| self.result.names.iter().position(|x| x == n);
        let (o, a, r) = (col(pe_obs)?, col(pe_alt)?, col(rpe)?);
        let pr = self.scalars.p_rank();
        let fill = |v: f64| if v.is_nan() { 0.5 } else { v };
        Some(
            (0..self.result.groups.len())
                .map(|g| {
                    if self.result.rows[g][o].is_nan() {
                        return f64::NAN;
                    }
                    let (obs, alt, rp) = (fill(pr[g * k + o]), fill(pr[g * k + a]), fill(pr[g * k + r]));
                    if obs <= 0.95 && alt <= 0.95 {
                        0.0
                    } else if rp < 0.025 {
                        1.0
                    } else if rp > 0.975 {
                        2.0
                    } else if obs > 0.99 && alt > 0.99 {
                        4.0
                    } else {
                        3.0
                    }
                })
                .collect(),
        )
    }
}
