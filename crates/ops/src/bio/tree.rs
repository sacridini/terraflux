//! Phylogenetic trees (Newick, Nexus) read as Biodiverse reads them (landschaft's bio/_tree.py), and as the
//! index engine takes them (bio/_phylo.py: nodes in name order, Biodiverse's trimming to the labels).
//!
//! Unnamed nodes get `0___`, `1___`... in the order their definition closes, duplicated names `__dup1`...,
//! quoted names are unquoted, a Nexus Translate table is applied, trees whose branches are all of length 0 get
//! length 1 everywhere, and a forest gets a new root of length 0.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::basedata::BaseData;
use super::names::{dequote_element, join_element, split_element};
use crate::{Error, Result};

/// A rooted tree: nodes in the order the reader creates them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tree {
    pub name: String,
    pub names: Vec<String>,
    /// -1 for the root.
    pub parent: Vec<i64>,
    pub length: Vec<f64>,
    pub children: Vec<Vec<usize>>,
    pub root: usize,
}

impl Tree {
    fn new(
        name: String,
        names: Vec<String>,
        parent: Vec<i64>,
        length: Vec<f64>,
        children: Vec<Vec<usize>>,
    ) -> Result<Tree> {
        if names.iter().collect::<HashSet<_>>().len() != names.len() {
            return Err(Error::msg("node names must be unique"));
        }
        let roots: Vec<usize> = (0..names.len()).filter(|&i| parent[i] < 0).collect();
        if roots.len() != 1 {
            return Err(Error::msg(format!("a tree needs exactly one root, got {}", roots.len())));
        }
        Ok(Tree { name, names, parent, length, children, root: roots[0] })
    }

    pub fn n_nodes(&self) -> usize {
        self.names.len()
    }

    /// The nodes sorted by name.
    pub fn node_order(&self) -> Vec<usize> {
        let mut o: Vec<usize> = (0..self.n_nodes()).collect();
        o.sort_by(|&a, &b| self.names[a].cmp(&self.names[b]));
        o
    }

    /// The sum of all branch lengths, in name order.
    pub fn total_length(&self) -> f64 {
        let mut t = 0.0;
        for i in self.node_order() {
            t += self.length[i];
        }
        t
    }

    /// Nodes with every child before its parent.
    pub fn postorder(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = vec![(self.root, 0)];
        while let Some((node, k)) = stack.pop() {
            let ch = &self.children[node];
            if k < ch.len() {
                stack.push((node, k + 1));
                stack.push((ch[k], 0));
            } else {
                out.push(node);
            }
        }
        out
    }

    /// Edges from the root to each node.
    pub fn depth(&self) -> Vec<i64> {
        let mut d = vec![0; self.n_nodes()];
        for &node in self.postorder().iter().rev() {
            let p = self.parent[node];
            if p >= 0 {
                d[node] = d[p as usize] + 1;
            }
        }
        d
    }

    /// Newick text (names quoted when needed, lengths as Python writes them).
    pub fn to_newick(&self) -> String {
        fn fmt(name: &str) -> String {
            if name.chars().any(|c| c.is_whitespace() || "(),:;'[]".contains(c)) {
                format!("'{}'", name.replace('\'', "''"))
            } else {
                name.to_string()
            }
        }
        fn rec(t: &Tree, i: usize, out: &mut String) {
            if !t.children[i].is_empty() {
                out.push('(');
                for (k, &c) in t.children[i].iter().enumerate() {
                    if k > 0 {
                        out.push(',');
                    }
                    rec(t, c, out);
                }
                out.push(')');
            }
            out.push_str(&fmt(&t.names[i]));
            out.push(':');
            out.push_str(&super::names::py_float(t.length[i]));
        }
        let mut out = String::new();
        rec(self, self.root, &mut out);
        out.push(';');
        out
    }

    pub fn terminal_names(&self) -> Vec<String> {
        (0..self.n_nodes()).filter(|&i| self.children[i].is_empty()).map(|i| self.names[i].clone()).collect()
    }
}

/// A tree while it is read.
struct Builder {
    name: String,
    names: Vec<String>,
    index: HashMap<String, usize>,
    parent: Vec<i64>,
    length: Vec<f64>,
    children: Vec<Vec<usize>>,
    highest_internal: i64,
}

impl Builder {
    fn new(name: String) -> Builder {
        Builder {
            name,
            names: Vec::new(),
            index: HashMap::new(),
            parent: Vec::new(),
            length: Vec::new(),
            children: Vec::new(),
            highest_internal: -1,
        }
    }

    fn free_internal_name(&mut self, exclude: &HashMap<String, String>) -> String {
        let values: HashSet<&String> = exclude.values().collect();
        loop {
            self.highest_internal += 1;
            let name = format!("{}___", self.highest_internal);
            if !self.index.contains_key(&name) && !exclude.contains_key(&name) && !values.contains(&name) {
                return name;
            }
        }
    }

    fn unique_name(&self, prefix: &str, exclude: &HashMap<String, String>) -> String {
        let mut i = 1;
        loop {
            let name = format!("{prefix}__dup{i}");
            if !self.index.contains_key(&name) && !exclude.contains_key(&name) {
                return name;
            }
            i += 1;
        }
    }

    fn add(&mut self, name: String, length: Option<f64>, exclude: &HashMap<String, String>) -> usize {
        let name = if self.index.contains_key(&name) { self.unique_name(&name, exclude) } else { name };
        let i = self.names.len();
        self.index.insert(name.clone(), i);
        self.names.push(name);
        self.parent.push(-1);
        self.length.push(length.unwrap_or(0.0));
        self.children.push(Vec::new());
        i
    }

    fn add_children(&mut self, node: usize, kids: &[usize]) {
        let seen: HashSet<usize> = self.children[node].iter().copied().collect();
        let mut done = HashSet::new();
        for &k in kids {
            if !done.insert(k) || seen.contains(&k) {
                continue;
            }
            let old = self.parent[k];
            if old >= 0 && old as usize != node {
                self.children[old as usize].retain(|c| *c != k);
            }
            self.parent[k] = node as i64;
            self.children[node].push(k);
        }
    }

    fn finish(mut self) -> Result<Tree> {
        // all-zero trees get unit lengths
        if self.length.iter().all(|l| *l == 0.0) {
            self.length.iter_mut().for_each(|l| *l = 1.0);
        }
        let roots: Vec<usize> = (0..self.names.len()).filter(|&i| self.parent[i] < 0).collect();
        if roots.len() > 1 {
            let name = self.free_internal_name(&HashMap::new());
            let r = self.add(name, Some(0.0), &HashMap::new());
            self.add_children(r, &roots);
        }
        Tree::new(self.name, self.names, self.parent, self.length, self.children)
    }
}

static TEXT_IN_QUOTES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?s)\A(?:'(.+)'|"(.+)")\z"#).unwrap());

fn clean_name(name: &str, translate: &HashMap<String, String>) -> String {
    let name = translate.get(name).cloned().unwrap_or_else(|| name.to_string());
    if TEXT_IN_QUOTES.is_match(&name) {
        let parts = split_element(&name);
        let joined = if parts.len() == 1 { parts[0].clone() } else { join_element(&parts) };
        return dequote_element(&joined);
    }
    name
}

/// The index of the `)` matching the `(` at `i`.
fn matching_paren(s: &[char], i: usize) -> Result<usize> {
    let mut depth = 0;
    for (j, &c) in s.iter().enumerate().skip(i) {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                return Ok(j);
            }
        }
    }
    let count = |ch: char| s.iter().filter(|c| **c == ch).count();
    Err(Error::msg(format!(
        "Tree has unbalanced parentheses (left is {}, right is {}), unable to parse",
        count('('),
        count(')')
    )))
}

/// A number at `i` ([+-]?(?=\.?\d)\d*(\.\d*)?([Ee][+-]?\d+)?): its end.
fn number_end(s: &[char], i: usize) -> Option<usize> {
    let mut j = i;
    if j < s.len() && (s[j] == '+' || s[j] == '-') {
        j += 1;
    }
    let digit = |k: usize| k < s.len() && s[k].is_ascii_digit();
    if !(digit(j) || (j < s.len() && s[j] == '.' && digit(j + 1))) {
        return None;
    }
    while digit(j) {
        j += 1;
    }
    if j < s.len() && s[j] == '.' {
        j += 1;
        while digit(j) {
            j += 1;
        }
    }
    if j < s.len() && (s[j] == 'e' || s[j] == 'E') {
        let mut k = j + 1;
        if k < s.len() && (s[k] == '+' || s[k] == '-') {
            k += 1;
        }
        if digit(k) {
            while digit(k) {
                k += 1;
            }
            j = k;
        }
    }
    Some(j)
}

/// Biodiverse's parse_newick (one level; recursion on brackets): the nodes added at this level.
fn parse_newick(s: &[char], tree: &mut Builder, translate: &HashMap<String, String>) -> Result<Vec<usize>> {
    let mut added = Vec::new();
    let mut name: Option<String> = None;
    let mut length: Option<f64> = Some(0.0); // Biodiverse starts each level with length 0
    let mut kids: Vec<usize> = Vec::new();
    let mut pos = 0;
    let n = s.len();
    let make_node = |tree: &mut Builder, name: &Option<String>, length: Option<f64>, kids: &[usize]| -> usize {
        let nm = match name {
            Some(n) => n.clone(),
            None => tree.free_internal_name(translate),
        };
        let nm = clean_name(&nm, translate);
        let node = tree.add(nm, length, translate);
        if !kids.is_empty() {
            tree.add_children(node, kids);
        }
        node
    };
    while pos < n {
        let c = s[pos];
        if c == ',' {
            pos += 1;
            added.push(make_node(tree, &name, length, &kids));
            name = None;
            length = None;
            kids = Vec::new();
        } else if c == '(' {
            let end = matching_paren(s, pos)?;
            kids = parse_newick(&s[pos + 1..end], tree, translate)?;
            pos = end + 1;
        } else if c == '[' {
            match (pos + 1..n).find(|&k| s[k] == ']') {
                Some(end) => pos = end + 1,
                None => pos = n,
            }
        } else if c == '\'' {
            // '(?:[^']|'')*'
            let mut j = pos + 1;
            loop {
                if j >= n {
                    return Err(Error::msg(format!("unterminated quote at position {pos}")));
                }
                if s[j] == '\'' {
                    if j + 1 < n && s[j + 1] == '\'' {
                        j += 2;
                        continue;
                    }
                    break;
                }
                j += 1;
            }
            name = Some(s[pos..=j].iter().collect());
            pos = j + 1;
        } else if c == ':' {
            let Some(end) = number_end(s, pos + 1) else {
                let shown: String = s[pos + 1..(pos + 20).min(n)].iter().collect();
                return Err(Error::msg(format!("Length '{shown}' does not look like a number")));
            };
            let text: String = s[pos + 1..end].iter().collect();
            length =
                Some(text.parse().map_err(|_| Error::msg(format!("Length '{text}' does not look like a number")))?);
            pos = end;
        } else if c.is_whitespace() {
            while pos < n && s[pos].is_whitespace() {
                pos += 1;
            }
        } else {
            let end = (pos..n).find(|&k| "(),:'[]".contains(s[k])).unwrap_or(n);
            if end == pos {
                return Err(Error::msg(format!("Unexpected character '{c}' found at position {}", pos + 1)));
            }
            name = Some(s[pos..end].iter().collect());
            pos = end;
        }
    }
    added.push(make_node(tree, &name, length, &kids));
    Ok(added)
}

static BEGIN_TREES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s*begin trees;").unwrap());
static END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s*end(?:block)?;").unwrap());
static TRANSLATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s*Translate").unwrap());
static PAIR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*(\S+)\s+('(?:[^']|'')*'|\S+)").unwrap());
static TRAILING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[,;]\s*\z").unwrap());
static ENDS_SEMI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r";\s*\z").unwrap());
static TREE_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*tree").unwrap());
static TREE_DEF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)\s*tree\s+(.+?)\s*=\s*(\[..\])?\s*(.*)").unwrap());

/// The trees of a Nexus text: name, Newick and the Translate table.
/// A tree of a Nexus text: its name, its Newick and the Translate table.
type NexusTree = (String, String, HashMap<String, String>);

fn nexus_trees(text: &str) -> Result<Vec<NexusTree>> {
    let lines: Vec<&str> = text.split(['\r', '\n']).filter(|l| !l.is_empty()).collect();
    // re.split keeps a leading and a trailing empty string
    let lines: Vec<&str> = {
        let mut v = Vec::new();
        if text.starts_with(['\r', '\n']) {
            v.push("");
        }
        v.extend(lines);
        if text.ends_with(['\r', '\n']) {
            v.push("");
        }
        v
    };
    let mut translate = HashMap::new();
    let mut newicks: Vec<String> = Vec::new();
    let mut in_block = false;
    let mut i = 0;
    while i < lines.len() {
        let mut line = lines[i];
        i += 1;
        if !in_block && BEGIN_TREES.is_match(line) {
            in_block = true;
            if i >= lines.len() {
                break;
            }
            line = lines[i];
            i += 1;
        }
        if !in_block {
            continue;
        }
        if END.is_match(line) {
            break;
        }
        if line.trim_start().starts_with('[') {
            if line.contains(']') {
                continue;
            }
            while i < lines.len() {
                let c = lines[i];
                i += 1;
                if c.contains(']') {
                    break;
                }
            }
            continue;
        }
        if TRANSLATE.is_match(line) {
            while i < lines.len() {
                let t = lines[i];
                i += 1;
                if t.is_empty() {
                    break;
                }
                if let Some(m) = PAIR.captures(t) {
                    let nm = TRAILING.replace(&m[2], "").to_string();
                    translate.insert(m[1].to_string(), nm);
                }
                if ENDS_SEMI.is_match(t) {
                    break;
                }
            }
            continue;
        }
        if TREE_LINE.is_match(line) {
            let mut nwk = line.to_string();
            if !ENDS_SEMI.is_match(line) {
                while i < lines.len() {
                    let t = lines[i];
                    i += 1;
                    if t.is_empty() {
                        break;
                    }
                    nwk.push_str(t);
                    if ENDS_SEMI.is_match(t) {
                        break;
                    }
                }
            }
            newicks.push(nwk);
        }
    }
    if newicks.is_empty() {
        return Err(Error::msg("Data has no trees in it, or is not Nexus format"));
    }
    Ok(newicks
        .into_iter()
        .map(|nwk| {
            let nwk = nwk.strip_suffix(';').map(str::to_string).unwrap_or(nwk);
            let (name, rest) = match TREE_DEF.captures(&nwk) {
                Some(m) => (m[1].trim_end().to_string(), m.get(3).map_or(String::new(), |r| r.as_str().to_string())),
                None => (String::new(), String::new()),
            };
            (name, rest, translate.clone())
        })
        .collect())
}

/// Every tree of a Newick or Nexus text (the format guessed from a `#NEXUS` header when not given).
pub fn read_trees(text: &str, format: Option<&str>) -> Result<Vec<Tree>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let format = match format {
        Some(f) => f.to_lowercase(),
        None => {
            let head: Vec<&str> = text.lines().take(40).filter(|l| !l.trim().is_empty()).collect();
            if head.join("\n").trim_start().to_uppercase().starts_with("#NEXUS") {
                "nexus".into()
            } else {
                "newick".into()
            }
        }
    };
    let mut trees = Vec::new();
    match format.as_str() {
        "nexus" => {
            for (name, rest, translate) in nexus_trees(text)? {
                let mut b = Builder::new(name);
                let chars: Vec<char> = rest.chars().collect();
                parse_newick(&chars, &mut b, &translate)?;
                trees.push(b.finish()?);
            }
        }
        "newick" | "phylip" => {
            for (i, nwk) in text.split(';').filter(|p| !p.trim().is_empty()).enumerate() {
                let mut b = Builder::new(format!("anonymous_{i}"));
                let chars: Vec<char> = nwk.chars().collect();
                parse_newick(&chars, &mut b, &HashMap::new())?;
                trees.push(b.finish()?);
            }
        }
        f => return Err(Error::msg(format!("unknown tree format {f:?}"))),
    }
    Ok(trees)
}

fn is_internal_name(name: &str) -> bool {
    name.ends_with("___")
}

/// The nodes kept when Biodiverse trims a tree to the basedata's labels (Tree::trim).
pub fn trimmed_mask(tree: &Tree, keep_labels: &HashSet<&str>) -> Vec<bool> {
    let n = tree.n_nodes();
    let names = &tree.names;
    let children = &tree.children;
    if (0..n).filter(|&i| children[i].is_empty()).all(|i| keep_labels.contains(names[i].as_str())) {
        return vec![true; n];
    }
    let mut keep: HashSet<String> = names.iter().filter(|nm| keep_labels.contains(nm.as_str())).cloned().collect();
    let descendants = |i: usize| -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack: Vec<usize> = children[i].clone();
        while let Some(c) = stack.pop() {
            out.push(c);
            stack.extend(&children[c]);
        }
        out
    };
    let mut trim: Vec<usize> = Vec::new();
    for i in 0..n {
        let nm = &names[i];
        if keep.contains(nm) || is_internal_name(nm) || i == tree.root {
            continue;
        }
        if children[i].is_empty() {
            trim.push(i);
            continue;
        }
        if descendants(i).iter().any(|&d| keep.contains(&names[d])) {
            // keep this node and its named ancestors
            let mut a = i as i64;
            while a >= 0 {
                if !is_internal_name(&names[a as usize]) {
                    keep.insert(names[a as usize].clone());
                }
                a = tree.parent[a as usize];
            }
        } else {
            trim.push(i);
        }
    }
    let mut alive = vec![true; n];
    let mut deleted = false;
    for &i in &trim {
        if keep.contains(&names[i]) || !alive[i] {
            continue;
        }
        alive[i] = false;
        deleted = true;
        for d in descendants(i) {
            alive[d] = false;
        }
    }
    // unnamed internal nodes without named descendants (only after deletions)
    let mut changed = deleted;
    while changed {
        changed = false;
        for i in 0..n {
            if !alive[i] || i == tree.root || !is_internal_name(&names[i]) {
                continue;
            }
            let live: Vec<usize> = descendants(i).into_iter().filter(|&d| alive[d]).collect();
            if !live.iter().any(|&d| !is_internal_name(&names[d])) {
                alive[i] = false;
                for d in live {
                    alive[d] = false;
                }
                changed = true;
            }
        }
    }
    alive
}

/// The tree as the index engine takes it (nodes in name order), and the node names in that order.
pub fn tree_view(tree: &Tree, bd: &BaseData) -> (terraflux_ls_sys::ffi::BioTree, Vec<String>) {
    let order = tree.node_order();
    let n = tree.n_nodes();
    let mut new_of = vec![0usize; n];
    for (k, &i) in order.iter().enumerate() {
        new_of[i] = k;
    }
    let parent: Vec<i32> =
        order.iter().map(|&i| if tree.parent[i] >= 0 { new_of[tree.parent[i] as usize] as i32 } else { -1 }).collect();
    let length: Vec<f64> = order.iter().map(|&i| tree.length[i]).collect();
    let mut child_ptr = vec![0i64; n + 1];
    let mut child_idx: Vec<i32> = Vec::new();
    for (k, &i) in order.iter().enumerate() {
        child_idx.extend(tree.children[i].iter().map(|&c| new_of[c] as i32));
        child_ptr[k + 1] = child_idx.len() as i64;
    }
    let depth_full = tree.depth();
    let depth: Vec<i32> = order.iter().map(|&i| depth_full[i] as i32).collect();
    let terminal: Vec<u8> = order.iter().map(|&i| u8::from(tree.children[i].is_empty())).collect();
    let labels: HashSet<&str> = bd.labels.iter().map(String::as_str).collect();
    let alive_orig = trimmed_mask(tree, &labels);
    let alive: Vec<bool> = order.iter().map(|&i| alive_orig[i]).collect();
    let trimmed_children: Vec<i32> = (0..n)
        .map(|k| {
            if alive[k] {
                child_idx[child_ptr[k] as usize..child_ptr[k + 1] as usize]
                    .iter()
                    .filter(|&&c| alive[c as usize])
                    .count() as i32
            } else {
                0
            }
        })
        .collect();
    let names_sorted: Vec<String> = order.iter().map(|&i| tree.names[i].clone()).collect();
    let index: HashMap<&str, usize> = names_sorted.iter().enumerate().map(|(k, nm)| (nm.as_str(), k)).collect();
    let label_node: Vec<i32> = bd.labels.iter().map(|lb| index.get(lb.as_str()).map_or(-1, |&k| k as i32)).collect();
    let mut node_label = vec![-1i32; n];
    for (li, &k) in label_node.iter().enumerate() {
        if k >= 0 {
            node_label[k as usize] = li as i32;
        }
    }
    let mut total = 0.0;
    let mut trimmed_total = 0.0;
    let mut first_t = true;
    for k in 0..n {
        total += length[k];
        if alive[k] {
            if first_t {
                trimmed_total = length[k];
                first_t = false;
            } else {
                trimmed_total += length[k];
            }
        }
    }
    let _ = total;
    let nonzero_trimmed = (0..n).filter(|&k| alive[k] && length[k] != 0.0).count();
    let mean_trimmed = trimmed_total / (nonzero_trimmed.max(1) as f64);
    let (mut eq_trimmed_total, mut first_e, mut default_eq_len) = (0.0, true, 0.0);
    for k in 0..n {
        if !alive[k] {
            continue;
        }
        let v = if length[k] != 0.0 { mean_trimmed } else { 0.0 };
        if default_eq_len == 0.0 && v != 0.0 {
            default_eq_len = v;
        }
        if first_e {
            eq_trimmed_total = v;
            first_e = false;
        } else {
            eq_trimmed_total += v;
        }
    }
    let view = terraflux_ls_sys::ffi::BioTree {
        n: n as i32,
        parent,
        length: length.clone(),
        child_ptr,
        child_idx,
        depth,
        terminal: terminal.clone(),
        in_trimmed: alive.iter().map(|a| u8::from(*a)).collect(),
        trimmed_child_count: trimmed_children.clone(),
        total_length: tree.total_length(),
        trimmed_total_length: trimmed_total,
        root: new_of[tree.root] as i32,
        label_node,
        node_label,
        n_terminals: terminal.iter().map(|t| i32::from(*t)).sum(),
        n_trimmed_terminals: (0..n).filter(|&k| alive[k] && trimmed_children[k] == 0).count() as i32,
        zero_length_count: length.iter().filter(|l| **l == 0.0).count() as i32,
        eq_full_total: length.iter().filter(|l| **l != 0.0).count() as f64,
        eq_trimmed_total,
        eq_trimmed_length: mean_trimmed,
        default_eq_len,
        pc_steps: Vec::new(),
        pc_vpd: false,
        pc_seed: 0,
    };
    (view, names_sorted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newick_names_and_lengths_are_biodiverses() {
        let [t] = read_trees("((a:1,b:2):1,c:3);", None).unwrap().try_into().unwrap();
        assert_eq!(t.names, ["a", "b", "0___", "c", "1___"]);
        assert_eq!(t.total_length(), 7.0);
        let [t] = read_trees("((a,a),'b c');", None).unwrap().try_into().unwrap();
        assert_eq!(t.names, ["a", "a__dup1", "0___", "b c", "1___"]);
        assert!(t.length.iter().all(|l| *l == 1.0));
    }
}
