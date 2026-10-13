//! Agglomerative clustering of groups and the region grower (landschaft's bio/_cluster.py, Biodiverse::Cluster
//! and RegionGrower): the matrix and the joins in landschaft's C++, the tree finished here as Biodiverse's
//! TreeNode does it (lengths from the join values, zero-length branches raised, several roots joined, children
//! ladderised, terminals numbered).

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use terraflux_ls_sys::{ClusterRecalc, ffi};

use super::basedata::BaseData;
use super::conditions::{Condition, neighbours};
use super::properties::LabelMatrix;
use super::spatial::{DEPS, REGISTRY, RowLabels, SpatialResult, calc_name, calculation, run_sets};
use super::tree::Tree;
use crate::{Error, Result};

const DIRECT: [&str; 3] = ["SORENSON", "JACCARD", "S2"];
pub const LINKAGES: [&str; 5] =
    ["link_average", "link_average_unweighted", "link_minimum", "link_maximum", "link_recalculate"];
/// The node values of a cluster tree.
pub const VALUES: [&str; 6] =
    ["TOTAL_LENGTH", "JOIN_NUMBER", "MATRIX_ITER_USED", "TERMINAL_NODE_FIRST", "TERMINAL_NODE_LAST", "JOIN_NOT_METRIC"];

/// A JSON number, or Python's infinities and NaN (written as texts in the dependencies).
fn json_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) if s == "inf" => Some(f64::INFINITY),
        serde_json::Value::String(s) if s == "-inf" => Some(f64::NEG_INFINITY),
        serde_json::Value::String(s) if s == "nan" => Some(f64::NAN),
        _ => None,
    }
}

/// The calculation giving a cluster (or region grower) index, its lowest possible value and whether zeroes can
/// be lumped.
fn index_info(index: &str, region_grower: bool) -> Result<(String, f64, bool)> {
    if region_grower {
        return REGISTRY
            .iter()
            .find(|c| c.scalars.iter().any(|s| s == index))
            .map(|c| (c.name.clone(), 1e10, false))
            .ok_or_else(|| Error::msg(format!("{index:?} is not an index of the calculations")));
    }
    for (calc, meta) in DEPS.as_object().unwrap() {
        if let Some(info) = meta.get("cluster_indices").and_then(|c| c.get(index)) {
            let min_poss = info.get("bounds").and_then(|b| b.get(0)).and_then(json_f64).unwrap_or(1e10);
            let lump = info
                .get("can_lump_zeroes")
                .is_some_and(|v| v.as_bool().unwrap_or(false) || v.as_f64().is_some_and(|x| x != 0.0));
            return Ok((calc.clone(), min_poss, lump));
        }
    }
    let mut valid: Vec<&String> = DEPS
        .as_object()
        .unwrap()
        .values()
        .filter_map(|m| m.get("cluster_indices")?.as_object())
        .flat_map(|o| o.keys())
        .collect();
    valid.sort();
    Err(Error::msg(format!(
        "{index:?} is not a cluster index (valid: {})",
        valid.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
    )))
}

#[derive(Debug, Clone)]
struct Node {
    name: String,
    children: Vec<usize>,
    parent: Option<usize>,
    length: f64,
    values: HashMap<&'static str, Option<f64>>,
}

struct Arena {
    nodes: Vec<Node>,
    /// The nodes in the tree, in the order they were made (Python's dict).
    alive: Vec<usize>,
    by_name: HashMap<String, usize>,
}

impl Arena {
    fn add(&mut self, name: &str) -> usize {
        let i = self.nodes.len();
        self.nodes.push(Node {
            name: name.to_string(),
            children: Vec::new(),
            parent: None,
            length: 0.0,
            values: HashMap::new(),
        });
        self.alive.push(i);
        self.by_name.insert(name.to_string(), i);
        i
    }

    fn remove(&mut self, name: &str) {
        if let Some(i) = self.by_name.remove(name) {
            self.alive.retain(|&x| x != i);
        }
    }

    fn is_terminal(&self, n: usize) -> bool {
        self.nodes[n].children.is_empty()
    }

    fn length_below(&self, n: usize) -> f64 {
        let mut best = 0.0;
        for &c in &self.nodes[n].children {
            let lb = self.length_below(c);
            let lb = if lb == 0.0 { 0.0 } else { lb };
            if lb > best {
                best = lb;
            }
        }
        self.nodes[n].length + best
    }

    fn total_length(&self, n: usize) -> f64 {
        match self.nodes[n].values.get("TOTAL_LENGTH") {
            Some(Some(v)) => *v,
            _ => self.length_below(n),
        }
    }

    fn delete_child(&mut self, parent: usize, child: usize) {
        if let Some(i) = self.nodes[parent].children.iter().position(|&c| c == child) {
            self.nodes[parent].children.remove(i);
        }
    }

    fn add_children(&mut self, node: usize, children: &[usize]) {
        for &child in children {
            if self.nodes[node].children.contains(&child) {
                continue;
            }
            if let Some(p) = self.nodes[child].parent {
                self.delete_child(p, child);
            }
            self.nodes[node].children.push(child);
            self.nodes[child].parent = Some(node);
        }
    }

    fn descendants(&self, n: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = self.nodes[n].children.clone();
        while let Some(c) = stack.pop() {
            out.push(c);
            stack.extend(&self.nodes[c].children);
        }
        out
    }

    /// TreeNode::raise_zerolength_children.
    fn raise_zero_length(&mut self, node: usize) -> (usize, Vec<String>) {
        let (mut raised, mut empty) = (0, Vec::new());
        if self.is_terminal(node) {
            return (raised, empty);
        }
        let children = self.nodes[node].children.clone();
        let count = children.len();
        let parent = self.nodes[node].parent;
        if let Some(p) = parent {
            for &child in &children {
                if self.total_length(child) == self.total_length(node) {
                    self.add_children(p, &[child]);
                    raised += 1;
                }
            }
        }
        if raised == count {
            if let Some(p) = parent {
                self.delete_child(p, node);
            }
            empty.push(self.nodes[node].name.clone());
        } else if let Some(p) = parent
            && raised == count - 1
        {
            let child = self.nodes[node].children[0];
            self.add_children(p, &[child]);
            self.nodes[child].length += self.nodes[node].length;
            self.delete_child(p, node);
            raised += 1;
            empty.push(self.nodes[node].name.clone());
        }
        for child in children {
            let (r, e) = self.raise_zero_length(child);
            raised += r;
            empty.extend(e);
        }
        (raised, empty)
    }

    fn number_terminals(&mut self, node: usize, count: f64) -> f64 {
        let (left, mut right) = (count, count);
        for c in self.nodes[node].children.clone() {
            if self.is_terminal(c) {
                right += 1.0;
                self.nodes[c].values.insert("TERMINAL_NODE_FIRST", Some(right));
                self.nodes[c].values.insert("TERMINAL_NODE_LAST", Some(right));
            } else {
                right = self.number_terminals(c, right);
            }
        }
        self.nodes[node].values.insert("TERMINAL_NODE_FIRST", Some(left + 1.0));
        self.nodes[node].values.insert("TERMINAL_NODE_LAST", Some(right));
        right
    }
}

/// A node of a cluster tree: its parent, length, children (names) and Biodiverse's values (see VALUES).
#[derive(Debug, Clone)]
pub struct ClusterNode {
    pub name: String,
    pub parent: Option<String>,
    pub length: f64,
    pub children: Vec<String>,
    pub values: [Option<f64>; 6],
}

/// The result of a clustering: the dendrogram, its nodes in name order and the calculations of each node's
/// groups.
#[derive(Debug, Clone)]
pub struct ClusterResult {
    pub tree: Tree,
    pub nodes: Vec<ClusterNode>,
    pub table: Option<SpatialResult>,
}

impl ClusterResult {
    /// The dendrogram cut into `k` clusters (the highest joins undone first): each group's cluster, 1 to k in
    /// the dendrogram's terminal order.
    pub fn cut(&self, k: usize) -> Result<Vec<(String, usize)>> {
        let index: HashMap<&str, usize> = self.nodes.iter().enumerate().map(|(i, n)| (n.name.as_str(), i)).collect();
        let n_groups = self.nodes.iter().filter(|n| n.children.is_empty()).count();
        if !(1..=n_groups).contains(&k) {
            return Err(Error::msg(format!("k must be between 1 and {n_groups}")));
        }
        let height = |i: usize| self.nodes[i].values[0].unwrap_or(f64::NAN) - self.nodes[i].length;
        let first = |i: usize| self.nodes[i].values[3].unwrap_or(f64::NAN);
        let mut clusters: Vec<usize> = (0..self.nodes.len()).filter(|&i| self.nodes[i].parent.is_none()).collect();
        while clusters.len() < k {
            // the highest join still in place (Python's max: the first of the greatest keys)
            let mut best: Option<usize> = None;
            for &c in clusters.iter().filter(|&&c| !self.nodes[c].children.is_empty()) {
                let greater = |a: usize, b: usize| {
                    let (ha, hb) = (height(a), height(b));
                    if ha != hb { ha > hb } else { -first(a) > -first(b) }
                };
                if best.is_none_or(|b| greater(c, b)) {
                    best = Some(c);
                }
            }
            let split = best.ok_or_else(|| Error::msg("nothing left to split"))?;
            clusters.retain(|&c| c != split);
            clusters.extend(self.nodes[split].children.iter().map(|c| index[c.as_str()]));
        }
        clusters.sort_by(|&a, &b| first(a).total_cmp(&first(b)));
        let mut out = Vec::new();
        for (i, &c) in clusters.iter().enumerate() {
            let mut stack = vec![c];
            while let Some(nd) = stack.pop() {
                if self.nodes[nd].children.is_empty() {
                    out.push((self.nodes[nd].name.clone(), i + 1));
                } else {
                    stack.extend(self.nodes[nd].children.iter().map(|c| index[c.as_str()]));
                }
            }
        }
        out.sort();
        Ok(out)
    }
}

/// How a clustering runs.
#[derive(Debug, Clone)]
pub struct ClusterOptions {
    pub index: String,
    pub linkage: String,
    pub conditions: Vec<String>,
    pub definition_query: Option<String>,
    pub calculations: Vec<String>,
    pub seed: u64,
    /// Biodiverse's cluster_tie_breaker pairs ("random" and "maximise"/"minimise", or "none").
    pub tie_breaker: Vec<String>,
    pub clear_singletons: bool,
    pub region_grower: bool,
}

/// Each node's labels, group counts and samples as Biodiverse's hierarchical mode merges them: a node takes
/// its first child's labels; later children add theirs with a group count of 1 and their samples added.
fn hierarchical_labels(bd: &BaseData, a: &Arena, order: &[String]) -> RowLabels {
    let depth = |mut n: usize| {
        let mut d = 0;
        while let Some(p) = a.nodes[n].parent {
            n = p;
            d += 1;
        }
        d
    };
    let mut by_depth: Vec<&String> = order.iter().collect();
    by_depth.sort_by(|x, y| depth(a.by_name[*y]).cmp(&depth(a.by_name[*x])).then(x.cmp(y)));
    type LabelMaps = (HashMap<i32, f64>, HashMap<i32, f64>);
    let mut cache: HashMap<&str, LabelMaps> = HashMap::new();
    for nm in by_depth {
        let nd = a.by_name[nm.as_str()];
        if a.is_terminal(nd) {
            let g = bd.group_index(nm).unwrap();
            let r = bd.indptr[g] as usize..bd.indptr[g + 1] as usize;
            let counts = bd.indices[r.clone()].iter().map(|&l| (l, 1.0)).collect();
            let samples = bd.indices[r.clone()].iter().copied().zip(bd.counts[r].iter().copied()).collect();
            cache.insert(nm, (counts, samples));
            continue;
        }
        let (mut counts, mut samples): LabelMaps = (HashMap::new(), HashMap::new());
        for &child in &a.nodes[nd].children {
            let (cc, cs) = &cache[a.nodes[child].name.as_str()];
            if counts.is_empty() {
                counts = cc.clone();
            } else {
                for &lb in cc.keys() {
                    counts.insert(lb, 1.0);
                }
            }
            if samples.is_empty() {
                samples = cs.clone();
            } else {
                for (&lb, &v) in cs {
                    *samples.entry(lb).or_insert(0.0) += v;
                }
            }
        }
        cache.insert(nm, (counts, samples));
    }
    let (mut indptr, mut labels, mut cnts, mut smps) = (vec![0i64], Vec::new(), Vec::new(), Vec::new());
    for nm in order {
        let (cc, cs) = &cache[nm.as_str()];
        let all: BTreeSet<i32> = cc.keys().chain(cs.keys()).copied().collect();
        for lb in all {
            labels.push(lb);
            cnts.push(cc.get(&lb).copied().unwrap_or(0.0));
            smps.push(cs.get(&lb).copied().unwrap_or(0.0));
        }
        indptr.push(labels.len() as i64);
    }
    (indptr, labels, cnts, smps)
}

/// Biodiverse's cluster analysis (or its region grower): groups joined pair by pair by an index of their
/// labels.
pub fn cluster(
    bd: &BaseData,
    tree: Option<&Tree>,
    matrix: Option<&LabelMatrix>,
    o: &ClusterOptions,
    parallel: bool,
) -> Result<ClusterResult> {
    if !LINKAGES.contains(&o.linkage.as_str()) {
        return Err(Error::msg(format!("linkage must be one of {}", LINKAGES.join(", "))));
    }
    let (calc, min_poss, can_lump) = index_info(&o.index, o.region_grower)?;
    let mut conditions: Vec<String> =
        if o.conditions.is_empty() { vec!["sp_select_all()".into()] } else { o.conditions.clone() };
    while conditions.last().is_some_and(|c| c.trim().is_empty()) {
        conditions.pop();
    }
    if conditions.is_empty() {
        conditions.push("1".into());
    }
    let conds: Vec<Condition> = conditions.iter().map(|c| Condition::new(c)).collect::<Result<_>>()?;
    let query = o.definition_query.as_deref().filter(|q| !q.trim().is_empty()).map(Condition::new).transpose()?;
    let nb = neighbours(bd, &conds, query.as_ref(), parallel, true, None)?;
    let procs: Vec<i32> = nb.groups.clone();
    let n = procs.len();
    if n == 0 {
        return Err(Error::msg("no groups to cluster (check the conditions and definition query)"));
    }
    let mut elem_of = vec![-1i64; bd.n_groups()];
    for (e, &g) in procs.iter().enumerate() {
        elem_of[g as usize] = e as i64;
    }
    // neighbours processed before are excluded (exclude_processed_elements)
    let nbr_lists: Vec<Vec<Vec<i32>>> = nb
        .stored
        .iter()
        .map(|(ip, ix)| {
            (0..n)
                .map(|r| {
                    ix[ip[r] as usize..ip[r + 1] as usize]
                        .iter()
                        .map(|&g| elem_of[g as usize])
                        .filter(|&e| e > r as i64)
                        .map(|e| e as i32)
                        .collect()
                })
                .collect()
        })
        .collect();
    let trim = conds.len() == 1 && conds[0].result_type() == "always_true";
    let names: Vec<String> = procs.iter().map(|&g| bd.groups[g as usize].clone()).collect();
    let direct = if DIRECT.contains(&o.index.as_str()) && !o.region_grower { o.index.clone() } else { String::new() };
    let (mut label_ptr, mut label_idx) = (vec![0i64], Vec::new());
    let (mut pair_keys, mut pair_values) = (Vec::new(), Vec::new());
    let calcs = vec![calc.clone()];
    let value_of = |r: &SpatialResult| -> Vec<f64> {
        let j = r.names.iter().position(|x| *x == o.index);
        r.rows.iter().map(|row| j.map_or(f64::NAN, |j| row[j])).collect()
    };
    if !direct.is_empty() {
        for &g in &procs {
            let g = g as usize;
            label_idx.extend_from_slice(&bd.indices[bd.indptr[g] as usize..bd.indptr[g + 1] as usize]);
            label_ptr.push(label_idx.len() as i64);
        }
    } else {
        let pairs: BTreeSet<(i32, i32)> = nbr_lists
            .iter()
            .flat_map(|per| per.iter().enumerate().flat_map(|(e1, l)| l.iter().map(move |&e2| (e1 as i32, e2))))
            .collect();
        if !pairs.is_empty() {
            let p1: Vec<i32> = pairs.iter().map(|&(a, _)| procs[a as usize]).collect();
            let p2: Vec<i32> = pairs.iter().map(|&(_, b)| procs[b as usize]).collect();
            let ip: Vec<i64> = (0..=pairs.len() as i64).collect();
            let r =
                run_sets(bd, p1.clone(), (ip.clone(), p1), Some((ip, p2)), &calcs, tree, matrix, true, None, parallel)?;
            for ((a, b), v) in pairs.iter().zip(value_of(&r)) {
                if !v.is_nan() {
                    pair_keys.push(i64::from(*a) * n as i64 + i64::from(*b));
                    pair_values.push(v);
                }
            }
        }
    }
    let (mut keys, mut minmax) = (Vec::new(), Vec::new());
    for k in (0..o.tie_breaker.len()).step_by(2) {
        let key = &o.tie_breaker[k];
        if key == "none" {
            continue;
        }
        if key != "random" {
            return Err(Error::msg("only the 'random' and 'none' tie breakers are supported so far"));
        }
        keys.push(key.clone());
        minmax.push(o.tie_breaker.get(k + 1).cloned().unwrap_or_else(|| "minimise".into()));
    }
    let mut rng = ffi::bio_rng_new(o.seed);
    // link_recalculate: the index of the merged groups against each other node
    let terminal_groups: Rc<RefCell<HashMap<String, Vec<i32>>>> =
        Rc::new(RefCell::new(names.iter().cloned().zip(procs.iter().map(|&g| vec![g])).collect()));
    let failure: Rc<RefCell<Option<Error>>> = Rc::new(RefCell::new(None));
    let mut recalc = {
        let (groups, failure, calcs, tree) = (terminal_groups.clone(), failure.clone(), calcs.clone(), tree.cloned());
        let matrix = matrix.cloned();
        let bd = bd.clone();
        let index = o.index.clone();
        ClusterRecalc(Box::new(move |n1: &str, n2: &str, new_name: &str, checks: &[String]| -> Vec<f64> {
            let mut g = groups.borrow_mut();
            let merged: Vec<i32> = g[n1].iter().chain(&g[n2]).copied().collect::<BTreeSet<_>>().into_iter().collect();
            g.insert(new_name.to_string(), merged.clone());
            if checks.is_empty() {
                return Vec::new();
            }
            let m = checks.len();
            let ip1: Vec<i64> = (0..=m as i64).map(|i| i * merged.len() as i64).collect();
            let ix1: Vec<i32> = (0..m).flat_map(|_| merged.iter().copied()).collect();
            let rows2: Vec<&Vec<i32>> = checks.iter().map(|c| &g[c.as_str()]).collect();
            let mut ip2 = vec![0i64];
            for r in &rows2 {
                ip2.push(ip2.last().unwrap() + r.len() as i64);
            }
            let ix2: Vec<i32> = rows2.iter().flat_map(|r| r.iter().copied()).collect();
            match run_sets(
                &bd,
                vec![merged[0]; m],
                (ip1, ix1),
                Some((ip2, ix2)),
                &calcs,
                tree.as_ref(),
                matrix.as_ref(),
                false,
                None,
                parallel,
            ) {
                Ok(r) => {
                    let j = r.names.iter().position(|x| *x == index);
                    r.rows.iter().map(|row| j.map_or(f64::NAN, |j| row[j])).collect()
                }
                Err(e) => {
                    *failure.borrow_mut() = Some(e);
                    vec![f64::NAN; m]
                }
            }
        }))
    };
    let mut max_poss = f64::NAN;
    if o.region_grower && conds.len() == 1 && conds[0].result_type() == "always_true" {
        let r = run_sets(
            bd,
            procs[..1].to_vec(),
            (vec![0, n as i64], procs.clone()),
            None,
            &calcs,
            tree,
            matrix,
            false,
            None,
            parallel,
        )?;
        max_poss = value_of(&r)[0];
    }
    let n_sets = nbr_lists.len();
    let (mut nbr_ptr, mut nbr_idx) = (vec![0i64], Vec::new());
    for per in &nbr_lists {
        for l in per {
            nbr_idx.extend_from_slice(l);
            nbr_ptr.push(nbr_idx.len() as i64);
        }
    }
    let out = ffi::bio_cluster(
        &names,
        n_sets as i32,
        &nbr_ptr,
        &nbr_idx,
        trim,
        &direct,
        &label_ptr,
        &label_idx,
        &pair_keys,
        &pair_values,
        &o.linkage,
        can_lump,
        min_poss,
        !o.tie_breaker.is_empty(),
        &keys,
        &minmax,
        o.region_grower,
        max_poss,
        rng.pin_mut(),
        o.linkage == "link_recalculate",
        &mut recalc,
    )?;
    if let Some(e) = failure.borrow_mut().take() {
        return Err(e);
    }

    // the tree
    let mut a = Arena { nodes: Vec::new(), alive: Vec::new(), by_name: HashMap::new() };
    for t in &out.terminals {
        a.add(t);
    }
    for j in &out.joins {
        let node = a.add(&j.name);
        let (c1, c2) = (a.by_name[j.child1.as_str()], a.by_name[j.child2.as_str()]);
        a.add_children(node, &[c1, c2]);
        a.nodes[node].values.insert("MATRIX_ITER_USED", Some(f64::from(j.matrix_iter)));
        a.nodes[node].values.insert("JOIN_NUMBER", Some(f64::from(j.join_number)));
        for child in a.nodes[node].children.clone() {
            a.nodes[child].values.insert("TOTAL_LENGTH", Some(j.value));
            a.nodes[child].length =
                if a.is_terminal(child) { j.value } else { j.value - a.total_length(a.nodes[child].children[0]) };
        }
        for child in a.nodes[node].children.clone() {
            if a.is_terminal(child) {
                a.nodes[child].values.insert("MATRIX_ITER_USED", Some(f64::from(j.matrix_iter)));
                a.nodes[child].values.insert("JOIN_NUMBER", Some(f64::from(j.join_number)));
            }
        }
    }
    let roots: Vec<usize> = a.alive.iter().copied().filter(|&i| a.nodes[i].parent.is_none()).collect();
    for r in roots {
        if a.is_terminal(r) {
            continue;
        }
        let mut count = 1;
        let mut empty = Vec::new();
        while count > 0 {
            let (c, e) = a.raise_zero_length(r);
            count = c;
            empty.extend(e);
        }
        for name in empty {
            a.remove(&name);
        }
    }
    let roots: Vec<usize> = a.alive.iter().copied().filter(|&i| a.nodes[i].parent.is_none()).collect();
    if roots.len() > 1 {
        let (mut to_add, mut singletons) = (Vec::new(), Vec::new());
        let mut target: Option<f64> = None;
        for &r in &roots {
            if o.clear_singletons && a.is_terminal(r) {
                singletons.push(a.nodes[r].name.clone());
                continue;
            }
            let lb = a.length_below(r);
            target = Some(match target {
                None => lb,
                Some(t) => {
                    if lb > t {
                        lb
                    } else {
                        t
                    }
                }
            });
            to_add.push(r);
        }
        let mut k = 0;
        while a.by_name.contains_key(&format!("{k}___")) {
            k += 1;
        }
        let root = a.add(&format!("{k}___"));
        a.nodes[root].values.insert("JOIN_NOT_METRIC", Some(1.0));
        for &r in &to_add {
            a.nodes[r].length = target.unwrap() - a.length_below(r);
            a.nodes[r].values.insert("JOIN_NOT_METRIC", Some(1.0));
        }
        a.add_children(root, &to_add);
        for s in singletons {
            a.remove(&s);
        }
    }
    let root = *a.alive.iter().find(|&&i| a.nodes[i].parent.is_none()).unwrap();
    a.nodes[root].length = 0.0;
    a.nodes[root].values.insert("TOTAL_LENGTH", Some(out.min_value));
    a.nodes[root].values.insert("JOIN_NUMBER", Some(f64::from(out.join_number)));
    a.nodes[root].values.entry("MATRIX_ITER_USED").or_insert(None);
    // ladderise: more descendants first, then by name
    let mut all = vec![root];
    all.extend(a.descendants(root));
    for nd in all {
        let mut ch = a.nodes[nd].children.clone();
        ch.sort_by(|&x, &y| {
            a.descendants(y).len().cmp(&a.descendants(x).len()).then(a.nodes[x].name.cmp(&a.nodes[y].name))
        });
        a.nodes[nd].children = ch;
    }
    a.number_terminals(root, 0.0);

    let mut order: Vec<String> = a.alive.iter().map(|&i| a.nodes[i].name.clone()).collect();
    order.sort();
    let idx: HashMap<&str, usize> = order.iter().enumerate().map(|(i, n)| (n.as_str(), i)).collect();
    let node_of = |nm: &str| a.by_name[nm];
    let parent: Vec<i64> = order
        .iter()
        .map(|nm| a.nodes[node_of(nm)].parent.map_or(-1, |p| idx[a.nodes[p].name.as_str()] as i64))
        .collect();
    let lengths: Vec<f64> = order.iter().map(|nm| a.nodes[node_of(nm)].length).collect();
    let children: Vec<Vec<usize>> = order
        .iter()
        .map(|nm| a.nodes[node_of(nm)].children.iter().map(|&c| idx[a.nodes[c].name.as_str()]).collect())
        .collect();
    let tree_out =
        Tree { name: "cluster".into(), names: order.clone(), parent, length: lengths.clone(), children, root: 0 };
    let tree_out = Tree { root: tree_out.parent.iter().position(|p| *p < 0).unwrap(), ..tree_out };
    let nodes: Vec<ClusterNode> = order
        .iter()
        .map(|nm| {
            let nd = &a.nodes[node_of(nm)];
            ClusterNode {
                name: nm.clone(),
                parent: nd.parent.map(|p| a.nodes[p].name.clone()),
                length: nd.length,
                children: nd.children.iter().map(|&c| a.nodes[c].name.clone()).collect(),
                values: VALUES.map(|v| nd.values.get(v).copied().flatten()),
            }
        })
        .collect();
    let table = if o.calculations.is_empty() {
        None
    } else {
        let mut calcs: Vec<String> = Vec::new();
        for c in &o.calculations {
            let cn = calc_name(c);
            let info = calculation(&cn).ok_or_else(|| Error::msg(format!("unknown calculation {c:?}")))?;
            if info.uses_nbr_lists <= 1 && !calcs.contains(&cn) {
                calcs.push(cn);
            }
        }
        let rows: Vec<Vec<i32>> = order
            .iter()
            .map(|nm| {
                let nd = node_of(nm);
                let terms: Vec<usize> = if a.is_terminal(nd) {
                    vec![nd]
                } else {
                    a.descendants(nd).into_iter().filter(|&x| a.is_terminal(x)).collect()
                };
                let mut g: Vec<i32> = terms.iter().map(|&t| bd.group_index(&a.nodes[t].name).unwrap() as i32).collect();
                g.sort_unstable();
                g
            })
            .collect();
        let mut ip = vec![0i64];
        for r in &rows {
            ip.push(ip.last().unwrap() + r.len() as i64);
        }
        let ix: Vec<i32> = rows.iter().flatten().copied().collect();
        let procs_rows: Vec<i32> = rows.iter().map(|r| r[0]).collect();
        let row_labels = hierarchical_labels(bd, &a, &order);
        Some(run_sets(bd, procs_rows, (ip, ix), None, &calcs, tree, matrix, false, Some(row_labels), parallel)?)
    };
    Ok(ClusterResult { tree: tree_out, nodes, table })
}
