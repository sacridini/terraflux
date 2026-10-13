//! Biodiverse's spatial analysis (landschaft's bio/_spatial.py): its indices over the neighbourhoods of groups,
//! computed by landschaft's C++ index engine.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use terraflux_ls_sys::ffi;

use super::basedata::BaseData;
use super::conditions::{Condition, Csr, Neighbourhoods, neighbours};
use super::names::{dequote_element, join_element, looks_like_number, numify, split_element};
use super::properties::{LabelMatrix, any_set, numbers, tables};
use super::tree::{Tree, tree_view};
use crate::{Error, Result};

/// Biodiverse's pre_calc dependencies of every calculation (dumped from Biodiverse by landschaft).
/// (Python's JSON: its infinities and NaN read as the texts "inf", "-inf" and "nan".)
pub static DEPS: LazyLock<serde_json::Value> = LazyLock::new(|| {
    let text = include_str!("biodiverse_dependencies.json")
        .replace("-Infinity", "\"-inf\"")
        .replace("Infinity", "\"inf\"")
        .replace("NaN", "\"nan\"");
    serde_json::from_str(&text).unwrap()
});

/// The calculations of the index engine.
pub static REGISTRY: LazyLock<Vec<ffi::BioCalcInfo>> = LazyLock::new(ffi::bio_calculations);

pub fn calculation(name: &str) -> Option<&'static ffi::BioCalcInfo> {
    REGISTRY.iter().find(|c| c.name == name)
}

fn pre_calc(c: &str) -> Vec<String> {
    DEPS.get(c)
        .and_then(|d| d.get("pre_calc"))
        .and_then(|p| p.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// The order Biodiverse runs pre_calcs in (parse_dependencies_for_calc and the aggregation).
pub fn precalc_order(calcs: &[String]) -> Vec<String> {
    let mut sorted = calcs.to_vec();
    sorted.sort();
    let mut agg: Vec<String> = Vec::new();
    for calc in sorted {
        let mut lst = vec![calc];
        let mut i = 0;
        while i < lst.len() {
            let more = pre_calc(&lst[i]);
            lst.extend(more);
            i += 1;
        }
        let mut seen = HashSet::new();
        for c in lst.into_iter().rev() {
            if seen.insert(c.clone()) {
                agg.push(c);
            }
        }
    }
    let mut seen = HashSet::new();
    let mut u: Vec<String> = agg.into_iter().filter(|c| seen.insert(c.clone())).collect();
    if !u.is_empty() {
        let to_front = |u: &mut Vec<String>, i: usize| {
            let c = u.remove(i);
            u.insert(0, c);
        };
        if !u.iter().any(|c| c.starts_with("calc_abc"))
            && let Some(i) = u.iter().position(|c| c == "_calc_abc_any")
            && i > 0
        {
            to_front(&mut u, i);
        }
        let i23 = u.iter().position(|c| c.starts_with("calc_abc2") || c.starts_with("calc_abc3"));
        match i23 {
            Some(i) if i > 0 => to_front(&mut u, i),
            None => {
                if let Some(i) = u.iter().position(|c| c == "calc_abc")
                    && i > 0
                {
                    to_front(&mut u, i);
                }
            }
            _ => {}
        }
    }
    u
}

/// The label counts `_calc_abc_any` passes on: 1 presence, 2 groups, 3 samples.
fn abc_any_variant(calcs: &[String]) -> i32 {
    let order = precalc_order(calcs);
    let Some(at) = order.iter().position(|c| c == "_calc_abc_any") else { return 1 };
    let before = &order[..at];
    for (name, v) in [("calc_abc", 1), ("calc_abc2", 2), ("calc_abc3", 3)] {
        if before.iter().any(|c| c == name) {
            return v;
        }
    }
    1
}

/// The prefixes of the label axes (Biodiverse's labels as a tree), numbered in name order.
fn label_hierarchy(bd: &BaseData) -> (i32, Vec<i32>, Vec<String>) {
    if bd.labels.is_empty() {
        return (1, Vec::new(), Vec::new());
    }
    let levels = split_element(&bd.labels[0]).len();
    let names_per: Vec<Vec<String>> = bd
        .labels
        .iter()
        .map(|lb| {
            let c = split_element(lb);
            (0..levels).map(|i| dequote_element(&join_element(if i < c.len() { &c[..=i] } else { &c[..] }))).collect()
        })
        .collect();
    let mut uniq: Vec<String> = names_per.iter().flatten().cloned().collect();
    uniq.sort();
    uniq.dedup();
    let prefix = names_per.iter().flatten().map(|n| uniq.binary_search(n).unwrap() as i32).collect();
    (levels as i32, prefix, uniq)
}

/// The basedata as the index engine reads it.
/// Each label's range (Biodiverse's get_range: the RANGE property where larger than the groups it is in) and
/// abundance (the ABUNDANCE property where larger than its samples), and its sample count.
pub fn label_ranges(bd: &BaseData) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n_l = bd.n_labels();
    let mut variety = vec![0.0; n_l];
    let mut sample = vec![0.0; n_l];
    for (k, &l) in bd.indices.iter().enumerate() {
        variety[l as usize] += 1.0;
        sample[l as usize] += bd.counts[k];
    }
    let over = |base: &[f64], name: &str| -> Vec<f64> {
        match numbers(&bd.label_props, name) {
            Some(r) => base.iter().zip(r).map(|(&v, r)| if r.is_nan() { v } else { r.max(v) }).collect(),
            None => base.to_vec(),
        }
    };
    (over(&variety, "RANGE"), over(&sample, "ABUNDANCE"), sample)
}

fn view(bd: &BaseData, valid: &[String], matrix: Option<&LabelMatrix>) -> ffi::BioView {
    let n_l = bd.n_labels();
    let (range, abundance, sample) = label_ranges(bd);
    let (levels, prefix, prefix_names) = label_hierarchy(bd);
    let numeric = bd.labels.iter().all(|l| looks_like_number(l));
    let order = precalc_order(valid);
    let position: HashMap<&String, usize> = order.iter().enumerate().map(|(i, c)| (c, i)).collect();
    let mut exec: Vec<i32> = (0..valid.len() as i32).collect();
    exec.sort_by_key(|&i| position.get(&valid[i as usize]).copied().unwrap_or(order.len()));
    let (lp, gp) = (tables(&bd.label_props), tables(&bd.group_props));
    // the matrix's block of the labels in the basedata
    let mx = matrix.map(|m| {
        let index: HashMap<&String, usize> = m.elements.iter().enumerate().map(|(i, e)| (e, i)).collect();
        let label_index: Vec<i64> = bd.labels.iter().map(|l| index.get(l).map_or(-1, |&i| i as i64)).collect();
        let mut present: Vec<usize> = label_index.iter().filter(|&&i| i >= 0).map(|&i| i as usize).collect();
        present.sort_unstable();
        present.dedup();
        let mut remap = vec![-1i32; m.elements.len()];
        for (k, &p) in present.iter().enumerate() {
            remap[p] = k as i32;
        }
        let n = m.elements.len();
        let values: Vec<f64> =
            present.iter().flat_map(|&i| present.iter().map(move |&j| m.values[i * n + j])).collect();
        let lmx: Vec<i32> = label_index.iter().map(|&i| if i >= 0 { remap[i as usize] } else { -1 }).collect();
        (lmx, present.len() as i32, values)
    });
    ffi::BioView {
        n_groups: bd.n_groups() as i64,
        n_labels: n_l as i64,
        indptr: bd.indptr.clone(),
        indices: bd.indices.clone(),
        counts: bd.counts.clone(),
        label_range: range,
        label_abundance: abundance,
        label_sample_count: sample,
        abc_any_variant: abc_any_variant(valid),
        n_label_levels: levels,
        label_prefix: prefix,
        prefix_names,
        labels_numeric: numeric,
        label_values: bd.labels.iter().map(|l| if numeric { numify(l) } else { f64::NAN }).collect(),
        exec_order: exec,
        pairwise_mode: false,
        has_row_labels: false,
        row_indptr: Vec::new(),
        row_labels: Vec::new(),
        row_counts: Vec::new(),
        row_samples: Vec::new(),
        label_prop_names: lp.0,
        label_prop_key_off: lp.1,
        label_prop_keys: lp.2,
        label_prop_values: lp.3,
        label_prop_elem: lp.4,
        group_prop_names: gp.0,
        group_prop_key_off: gp.1,
        group_prop_keys: gp.2,
        group_prop_values: gp.3,
        group_prop_elem: gp.4,
        has_matrix: mx.is_some(),
        label_mx: mx.as_ref().map_or(Vec::new(), |m| m.0.clone()),
        mx_n: mx.as_ref().map_or(0, |m| m.1),
        mx_values: mx.as_ref().map_or(Vec::new(), |m| m.2.clone()),
        mx_self_similarity: matrix.and_then(|m| m.self_similarity).unwrap_or(f64::NAN),
    }
}

/// The calculation's Biodiverse name.
pub fn calc_name(c: &str) -> String {
    if c.starts_with("calc_") { c.to_string() } else { format!("calc_{c}") }
}

/// A list index of the processing groups: its kind (landschaft's KeyKind) and per group its (key, value) entries.
#[derive(Debug, Clone)]
pub struct ListIndex {
    pub name: String,
    pub kind: i32,
    pub rows: Vec<Vec<(f64, f64)>>,
}

/// The result of a spatial analysis.
#[derive(Debug, Clone)]
pub struct SpatialResult {
    /// The processing groups (indices into the basedata's groups).
    pub groups: Vec<i32>,
    /// Scalar indices: names, and a row per processing group.
    pub names: Vec<String>,
    pub rows: Vec<Vec<f64>>,
    pub lists: Vec<ListIndex>,
    /// The text keys of the lists.
    pub texts: Vec<String>,
    /// The calculations run (without `calc_`).
    pub calculations: Vec<String>,
    /// The tree's node names in name order (the keys of node lists).
    pub nodes: Vec<String>,
}

fn no_tree() -> ffi::BioTree {
    ffi::BioTree {
        n: 0,
        parent: Vec::new(),
        length: Vec::new(),
        child_ptr: vec![0],
        child_idx: Vec::new(),
        depth: Vec::new(),
        terminal: Vec::new(),
        in_trimmed: Vec::new(),
        trimmed_child_count: Vec::new(),
        total_length: 0.0,
        trimmed_total_length: 0.0,
        root: -1,
        label_node: Vec::new(),
        node_label: Vec::new(),
        n_terminals: 0,
        n_trimmed_terminals: 0,
        zero_length_count: 0,
        eq_full_total: 0.0,
        eq_trimmed_total: 0.0,
        eq_trimmed_length: 0.0,
        default_eq_len: 0.0,
        pc_steps: Vec::new(),
        pc_vpd: false,
        pc_seed: 0,
    }
}

fn take_rows(c: &Csr, rows: &[usize]) -> Csr {
    let mut indptr = vec![0i64];
    let mut indices = Vec::new();
    for &r in rows {
        indices.extend_from_slice(&c.1[c.0[r] as usize..c.0[r + 1] as usize]);
        indptr.push(indices.len() as i64);
    }
    (indptr, indices)
}

/// What an analysis needs to run again on other counts (the randomisations): the calculations, the
/// neighbourhoods and the rows the engine computes.
#[derive(Debug, Clone)]
pub struct Run {
    pub valid: Vec<String>,
    pub nbrs: Neighbourhoods,
    pub set1: Csr,
    pub set2: Option<Csr>,
    /// The groups the engine computes (representatives of identical neighbourhoods).
    pub procs: Vec<i32>,
    /// Each processing group's row among `procs`.
    pub expand: Option<Vec<usize>>,
    pub tree: Option<Tree>,
    pub matrix: Option<LabelMatrix>,
    pub pc_steps: Vec<i32>,
    pub pc_vpd: bool,
    pub seed: u64,
    pub parallel: bool,
}

/// Biodiverse's spatial analysis: indices over the neighbourhood of every group (landschaft's
/// spatial_analysis, without matrices and properties). `seed`: Biodiverse's prng_seed, of the random tips of the
/// expected NTI on trees that are not ultrametric and of the net VPD.
#[allow(clippy::too_many_arguments)]
pub fn spatial_analysis(
    bd: &BaseData,
    conditions: &[Condition],
    calculations: &[String],
    query: Option<&Condition>,
    tree: Option<&Tree>,
    matrix: Option<&LabelMatrix>,
    seed: u64,
    parallel: bool,
) -> Result<SpatialResult> {
    let run = prepare(bd, conditions, calculations, query, tree, matrix, seed, parallel)?;
    evaluate(bd, &run, None)
}

/// The calculations that can run, the neighbourhoods and the rows to compute.
#[allow(clippy::too_many_arguments)]
pub fn prepare(
    bd: &BaseData,
    conditions: &[Condition],
    calculations: &[String],
    query: Option<&Condition>,
    tree: Option<&Tree>,
    matrix: Option<&LabelMatrix>,
    seed: u64,
    parallel: bool,
) -> Result<Run> {
    let mut calcs: Vec<String> = Vec::new();
    for c in calculations {
        let name = calc_name(c);
        if calculation(&name).is_none() {
            return Err(Error::msg(format!("unknown calculation {c:?}")));
        }
        if !calcs.contains(&name) {
            calcs.push(name);
        }
    }
    let n_sets = conditions.len();
    let mut valid: Vec<String> =
        calcs.into_iter().filter(|c| calculation(c).unwrap().uses_nbr_lists as usize <= n_sets).collect();
    let needs: Vec<&String> = valid.iter().filter(|c| calculation(c).unwrap().uses_tree).collect();
    if !needs.is_empty() && tree.is_none() {
        return Err(Error::msg(format!(
            "these calculations need a tree: {}",
            needs.iter().map(|n| n.trim_start_matches("calc_")).collect::<Vec<_>>().join(", ")
        )));
    }
    let matrix_calcs = ["calc_matrix_stats", "calc_compare_dissim_matrix_values", "calc_mx_rao_qe"];
    let needs: Vec<&String> = valid.iter().filter(|c| matrix_calcs.contains(&c.as_str())).collect();
    if !needs.is_empty() && matrix.is_none() {
        return Err(Error::msg(format!(
            "these calculations need a matrix: {}",
            needs.iter().map(|n| n.trim_start_matches("calc_")).collect::<Vec<_>>().join(", ")
        )));
    }
    // Biodiverse's pre-conditions: calculations for data the basedata lacks are not run
    if !bd.labels.iter().all(|l| looks_like_number(l)) {
        valid.retain(|c| {
            !(c.starts_with("calc_numeric_label") || c.starts_with("calc_num_labels") || c.starts_with("calc_iei"))
        });
    }
    if !any_set(&bd.label_props) {
        valid.retain(|c| !c.starts_with("calc_lbprop"));
    }
    if !any_set(&bd.group_props) {
        valid.retain(|c| !c.starts_with("calc_gpprop"));
    }
    if valid.is_empty() {
        return Err(Error::msg("none of the calculations can run with this many neighbour sets"));
    }
    let uses_proc = valid.iter().any(|c| calculation(c).unwrap().uses_processing_element);
    let nbrs: Neighbourhoods =
        neighbours(bd, conditions, query, parallel, true, if uses_proc { Some(false) } else { None })?;
    let (mut set1, mut set2) = (nbrs.sets[0].clone(), nbrs.sets.get(1).cloned());
    let mut procs = nbrs.groups.clone();
    // PhyloCom: expected values drawn from one random stream, and (ultrametric trees) path lengths cached as
    // groups are processed
    let order = precalc_order(&valid);
    let pc_steps: Vec<i32> = order
        .iter()
        .filter(|c| *c == "_calc_nri_nti_expected_values" || c.starts_with("calc_phylo_mpd_mntd"))
        .map(|c| if c == "_calc_nri_nti_expected_values" { 0 } else { c[c.len() - 1..].parse().unwrap_or(0) })
        .collect();
    let pc_vpd = valid.iter().any(|c| c == "calc_net_vpd" || c == "calc_vpd_expected_values");
    let mut order_sensitive = false;
    if let Some(t) = tree
        && !pc_steps.is_empty()
    {
        let u = ffi::bio_ultrametric(&tree_view(t, bd).0)?;
        order_sensitive = pc_vpd || (u[0] != 0 && u[1] != 0);
    }
    // groups with the same neighbour sets get the same results: compute each once (not where results depend
    // on the groups processed before, unless Biodiverse copies them too)
    let mut expand: Option<Vec<usize>> = None;
    if !uses_proc && procs.len() > 1 && (!order_sensitive || nbrs.results_recyclable) {
        let mut seen: HashMap<(Vec<i32>, Vec<i32>), usize> = HashMap::new();
        let mut rep = Vec::new();
        let inverse: Vec<usize> = (0..procs.len())
            .map(|r| {
                let key = (
                    set1.1[set1.0[r] as usize..set1.0[r + 1] as usize].to_vec(),
                    set2.as_ref().map_or(Vec::new(), |s| s.1[s.0[r] as usize..s.0[r + 1] as usize].to_vec()),
                );
                *seen.entry(key).or_insert_with(|| {
                    rep.push(r);
                    rep.len() - 1
                })
            })
            .collect();
        if rep.len() < procs.len() {
            set1 = take_rows(&set1, &rep);
            set2 = set2.map(|s| take_rows(&s, &rep));
            procs = rep.iter().map(|&r| procs[r]).collect();
            expand = Some(inverse);
        }
    }
    Ok(Run {
        valid,
        nbrs,
        set1,
        set2,
        procs,
        expand,
        tree: tree.cloned(),
        matrix: matrix.cloned(),
        pc_steps,
        pc_vpd,
        seed,
        parallel,
    })
}

/// The calculations of an analysis on a basedata with the same groups (its neighbourhoods), with its tree or
/// another (landschaft's _evaluate).
pub fn evaluate(bd: &BaseData, run: &Run, tree: Option<&Tree>) -> Result<SpatialResult> {
    evaluate_with(bd, run, tree, false, None)
}

/// Per-row labels of neighbour set 1 (indptr, labels, group counts, samples) used instead of the groups'.
pub type RowLabels = (Vec<i64>, Vec<i32>, Vec<f64>, Vec<f64>);

/// Calculations on given neighbour sets (rows of group indices), one row per set (landschaft's _run_sets):
/// `pairwise` the cluster matrices' mode, `row_labels` the hierarchical mode of cluster nodes.
#[allow(clippy::too_many_arguments)]
pub fn run_sets(
    bd: &BaseData,
    procs: Vec<i32>,
    set1: Csr,
    set2: Option<Csr>,
    calcs: &[String],
    tree: Option<&Tree>,
    matrix: Option<&LabelMatrix>,
    pairwise: bool,
    row_labels: Option<RowLabels>,
    parallel: bool,
) -> Result<SpatialResult> {
    let nbrs = Neighbourhoods {
        groups: procs.clone(),
        sets: vec![set1.clone()],
        results_recyclable: false,
        stored: Vec::new(),
    };
    let run = Run {
        valid: calcs.iter().map(|c| calc_name(c)).collect(),
        nbrs,
        set1,
        set2,
        procs,
        expand: None,
        tree: tree.cloned(),
        matrix: matrix.cloned(),
        pc_steps: Vec::new(),
        pc_vpd: false,
        seed: 0,
        parallel,
    };
    evaluate_with(bd, &run, None, pairwise, row_labels)
}

fn evaluate_with(
    bd: &BaseData,
    run: &Run,
    tree: Option<&Tree>,
    pairwise: bool,
    row_labels: Option<RowLabels>,
) -> Result<SpatialResult> {
    let tree = tree.or(run.tree.as_ref());
    let valid = &run.valid;
    let procs = &run.procs;
    let mut v = view(bd, valid, run.matrix.as_ref());
    v.pairwise_mode = pairwise;
    if let Some((ip, labels, counts, samples)) = row_labels {
        v.has_row_labels = true;
        v.row_indptr = ip;
        v.row_labels = labels;
        v.row_counts = counts;
        v.row_samples = samples;
    }
    let csr = |c: &Csr| ffi::BioCsr { indptr: c.0.clone(), indices: c.1.clone() };
    let empty = ffi::BioCsr { indptr: vec![0; procs.len() + 1], indices: Vec::new() };
    let s2 = run.set2.as_ref().map(csr);
    let (tview, nodes) = match tree {
        Some(t) if valid.iter().any(|c| calculation(c).unwrap().uses_tree) => {
            let (mut tv, names) = tree_view(t, bd);
            if !run.pc_steps.is_empty() {
                tv.pc_steps = run.pc_steps.clone();
                tv.pc_vpd = run.pc_vpd;
                tv.pc_seed = run.seed;
            }
            (Some(tv), names)
        }
        _ => (None, Vec::new()),
    };
    let none = no_tree();
    let out = ffi::bio_run_indices(
        &v,
        procs,
        &csr(&run.set1),
        s2.is_some(),
        s2.as_ref().unwrap_or(&empty),
        valid,
        run.parallel,
        tview.is_some(),
        tview.as_ref().unwrap_or(&none),
    )?;
    let k = out.scalar_names.len();
    let mut rows: Vec<Vec<f64>> = out.scalars.chunks(k.max(1)).take(procs.len()).map(<[f64]>::to_vec).collect();
    if k == 0 {
        rows = vec![Vec::new(); procs.len()];
    }
    let mut lists: Vec<ListIndex> = (0..out.list_names.len())
        .map(|i| {
            let ip = &out.list_indptr[out.list_indptr_off[i] as usize..out.list_indptr_off[i + 1] as usize];
            let base = out.list_entry_off[i] as usize;
            ListIndex {
                name: out.list_names[i].clone(),
                kind: out.list_kinds[i],
                rows: ip
                    .windows(2)
                    .map(|w| {
                        (w[0] as usize..w[1] as usize)
                            .map(|e| (out.list_keys[base + e], out.list_values[base + e]))
                            .collect()
                    })
                    .collect(),
            }
        })
        .collect();
    if let Some(inv) = &run.expand {
        rows = inv.iter().map(|&r| rows[r].clone()).collect();
        for l in &mut lists {
            l.rows = inv.iter().map(|&r| l.rows[r].clone()).collect();
        }
    }
    // indices needing more neighbour sets than given are not reported (Biodiverse's indices_to_clear)
    let n_sets = if run.set2.is_none() { 1 } else { 2 };
    let clear: HashSet<String> = valid
        .iter()
        .filter_map(|c| DEPS.get(c)?.get("index_uses_nbr_lists")?.as_object())
        .flat_map(|o| o.iter().filter(|(_, n)| n.as_i64().unwrap_or(0) > n_sets).map(|(i, _)| i.clone()))
        .collect();
    let keep: Vec<usize> = (0..k).filter(|&j| !clear.contains(&out.scalar_names[j])).collect();
    Ok(SpatialResult {
        groups: run.nbrs.groups.clone(),
        names: keep.iter().map(|&j| out.scalar_names[j].clone()).collect(),
        rows: rows.iter().map(|r| keep.iter().map(|&j| r[j]).collect()).collect(),
        lists: lists.into_iter().filter(|l| !clear.contains(&l.name)).collect(),
        texts: out.texts.clone(),
        calculations: valid.iter().map(|c| c.trim_start_matches("calc_").to_string()).collect(),
        nodes,
    })
}
