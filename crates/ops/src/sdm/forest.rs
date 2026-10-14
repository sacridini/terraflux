//! scikit-learn's RandomForestClassifier (1.9) for presence and background: the trees grown as its Cython grows
//! them (the depth-first builder, the best splitter with its Fisher-Yates draw of the features by its rand_r,
//! its introsort of the values, the Gini criterion updated from the cheaper side), each from a bootstrap of the
//! records drawn by numpy's legacy RandomState (MT19937) with the seeds the forest draws; the presence's
//! probability averaged over the trees in their order. The same trees and probabilities as scikit-learn's.

use serde::{Deserialize, Serialize};

use super::extrapolation::Frame;

/// numpy's legacy MT19937 (RandomState), seeded with an integer as RandomState(seed) seeds it.
pub struct Mt19937 {
    mt: [u32; 624],
    pos: usize,
}

impl Mt19937 {
    pub fn new(seed: u32) -> Self {
        let mut mt = [0u32; 624];
        mt[0] = seed;
        for i in 1..624 {
            mt[i] = 1_812_433_253u32.wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_add(i as u32);
        }
        Mt19937 { mt, pos: 624 }
    }

    pub fn next_u32(&mut self) -> u32 {
        if self.pos >= 624 {
            const UPPER: u32 = 0x8000_0000;
            const LOWER: u32 = 0x7fff_ffff;
            const MATRIX: u32 = 0x9908_b0df;
            for k in 0..624 {
                let y = (self.mt[k] & UPPER) | (self.mt[(k + 1) % 624] & LOWER);
                self.mt[k] = self.mt[(k + 397) % 624] ^ (y >> 1) ^ if y & 1 == 1 { MATRIX } else { 0 };
            }
            self.pos = 0;
        }
        let mut y = self.mt[self.pos];
        self.pos += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// RandomState.random_sample: a double of 53 bits from two draws.
    pub fn random_sample(&mut self) -> f64 {
        let a = self.next_u32() >> 5;
        let b = self.next_u32() >> 6;
        (f64::from(a) * 67_108_864.0 + f64::from(b)) / 9_007_199_254_740_992.0
    }

    /// RandomState.randint(low, high): numpy's masked rejection of 32-bit draws (ranges below 2^32).
    pub fn randint(&mut self, low: i64, high: i64) -> i64 {
        let rng = (high - 1 - low) as u64;
        if rng == 0 {
            return low;
        }
        assert!(rng < 0xFFFF_FFFF, "randint: a range below 2^32");
        let mut mask = rng;
        for s in [1, 2, 4, 8, 16, 32] {
            mask |= mask >> s;
        }
        let (rng, mask) = (rng as u32, mask as u32);
        loop {
            let v = self.next_u32() & mask;
            if v <= rng {
                return low + i64::from(v);
            }
        }
    }
}

/// scikit-learn's rand_r (its xorshift, modulo 2^31).
fn our_rand_r(seed: &mut u32) -> u32 {
    if *seed == 0 {
        *seed = 1;
    }
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed % (RAND_R_MAX + 1)
}

const RAND_R_MAX: u32 = 2_147_483_647;
const FEATURE_THRESHOLD: f32 = 1e-7;
const EPSILON: f64 = f64::EPSILON;

fn rand_int(low: usize, high: usize, state: &mut u32) -> usize {
    low + (our_rand_r(state) as usize) % (high - low)
}

// ------------------------------------------------------------------ scikit-learn's introsort (3-way)

fn swap(v: &mut [f32], s: &mut [usize], i: usize, j: usize) {
    v.swap(i, j);
    s.swap(i, j);
}

fn median3(v: &[f32], n: usize) -> f32 {
    let (a, b, c) = (v[0], v[n / 2], v[n - 1]);
    if a < b {
        if b < c {
            b
        } else if a < c {
            c
        } else {
            a
        }
    } else if b < c {
        if a < c { a } else { c }
    } else {
        b
    }
}

fn insertion_sort(v: &mut [f32], s: &mut [usize]) {
    for i in 1..v.len() {
        let (tv, ti) = (v[i], s[i]);
        let mut j = i;
        while j > 0 && v[j - 1] > tv {
            v[j] = v[j - 1];
            s[j] = s[j - 1];
            j -= 1;
        }
        v[j] = tv;
        s[j] = ti;
    }
}

fn sift_down(v: &mut [f32], s: &mut [usize], start: usize, end: usize) {
    let mut root = start;
    loop {
        let child = root * 2 + 1;
        let mut maxind = root;
        if child < end && v[maxind] < v[child] {
            maxind = child;
        }
        if child + 1 < end && v[maxind] < v[child + 1] {
            maxind = child + 1;
        }
        if maxind == root {
            break;
        }
        swap(v, s, root, maxind);
        root = maxind;
    }
}

fn heapsort(v: &mut [f32], s: &mut [usize]) {
    let n = v.len();
    let mut start = (n as isize - 2) / 2;
    loop {
        sift_down(v, s, start.max(0) as usize, n);
        if start <= 0 {
            break;
        }
        start -= 1;
    }
    let mut end = n as isize - 1;
    while end > 0 {
        swap(v, s, 0, end as usize);
        sift_down(v, s, 0, end as usize);
        end -= 1;
    }
}

fn introsort_3way(mut v: &mut [f32], mut s: &mut [usize], mut maxd: isize) {
    while v.len() > 15 {
        if maxd <= 0 {
            heapsort(v, s);
            return;
        }
        maxd -= 1;
        let n = v.len();
        let pivot = median3(v, n);
        let (mut i, mut l, mut r) = (0, 0, n);
        while i < r {
            if v[i] < pivot {
                swap(v, s, i, l);
                i += 1;
                l += 1;
            } else if v[i] > pivot {
                r -= 1;
                swap(v, s, i, r);
            } else {
                i += 1;
            }
        }
        let (left_v, right_v) = v.split_at_mut(r);
        let (left_s, right_s) = s.split_at_mut(r);
        introsort_3way(&mut left_v[..l], &mut left_s[..l], maxd);
        v = right_v;
        s = right_s;
    }
    insertion_sort(v, s);
}

fn simultaneous_sort(v: &mut [f32], s: &mut [usize]) {
    let n = v.len();
    if n == 0 {
        return;
    }
    let maxd = 2 * ((n as f64).log2() as isize);
    introsort_3way(v, s, maxd);
}

// ------------------------------------------------------------------ the tree

/// A node: its children (none for a leaf), the feature and threshold of its split, the presence's probability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub left: i32,
    pub right: i32,
    pub feature: i32,
    pub threshold: f64,
    pub value: [f64; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree {
    pub nodes: Vec<Node>,
}

impl Tree {
    /// The classes' probabilities of a row (its values as floats, as scikit-learn's trees take them).
    fn leaf(&self, row: &[f32]) -> &[f64; 2] {
        let mut k = 0usize;
        loop {
            let n = &self.nodes[k];
            if n.left < 0 {
                return &n.value;
            }
            k = if f64::from(row[n.feature as usize]) <= n.threshold { n.left as usize } else { n.right as usize };
        }
    }
}

/// The Gini criterion of the samples of a node, its left part [start, pos) and right part [pos, end).
struct Criterion {
    sum_total: [f64; 2],
    sum_left: [f64; 2],
    sum_right: [f64; 2],
    weighted_n_samples: f64,
    weighted_n_node_samples: f64,
    weighted_n_left: f64,
    weighted_n_right: f64,
    start: usize,
    end: usize,
    pos: usize,
}

impl Criterion {
    fn init(&mut self, y: &[usize], w: &[f64], samples: &[usize], start: usize, end: usize) {
        self.start = start;
        self.end = end;
        self.weighted_n_node_samples = 0.0;
        self.sum_total = [0.0; 2];
        for &i in &samples[start..end] {
            self.sum_total[y[i]] += w[i];
            self.weighted_n_node_samples += w[i];
        }
        self.reset();
    }

    fn reset(&mut self) {
        self.pos = self.start;
        self.sum_left = [0.0; 2];
        self.sum_right = self.sum_total;
        self.weighted_n_left = 0.0;
        self.weighted_n_right = self.weighted_n_node_samples;
    }

    fn reverse_reset(&mut self) {
        self.pos = self.end;
        self.sum_right = [0.0; 2];
        self.sum_left = self.sum_total;
        self.weighted_n_right = 0.0;
        self.weighted_n_left = self.weighted_n_node_samples;
    }

    fn update(&mut self, y: &[usize], w: &[f64], samples: &[usize], new_pos: usize) {
        let pos = self.pos;
        if new_pos - pos <= self.end - new_pos {
            for &i in &samples[pos..new_pos] {
                self.sum_left[y[i]] += w[i];
                self.weighted_n_left += w[i];
            }
        } else {
            self.reverse_reset();
            for p in (new_pos..self.end).rev() {
                let i = samples[p];
                self.sum_left[y[i]] -= w[i];
                self.weighted_n_left -= w[i];
            }
        }
        self.weighted_n_right = self.weighted_n_node_samples - self.weighted_n_left;
        for c in 0..2 {
            self.sum_right[c] = self.sum_total[c] - self.sum_left[c];
        }
        self.pos = new_pos;
    }

    fn node_impurity(&self) -> f64 {
        let mut sq = 0.0;
        for c in self.sum_total {
            sq += c * c;
        }
        1.0 - sq / (self.weighted_n_node_samples * self.weighted_n_node_samples)
    }

    fn children_impurity(&self) -> (f64, f64) {
        let (mut sl, mut sr) = (0.0, 0.0);
        for c in 0..2 {
            sl += self.sum_left[c] * self.sum_left[c];
            sr += self.sum_right[c] * self.sum_right[c];
        }
        (
            1.0 - sl / (self.weighted_n_left * self.weighted_n_left),
            1.0 - sr / (self.weighted_n_right * self.weighted_n_right),
        )
    }

    fn proxy_impurity_improvement(&self) -> f64 {
        let (il, ir) = self.children_impurity();
        -self.weighted_n_right * ir - self.weighted_n_left * il
    }

    fn impurity_improvement(&self, parent: f64, il: f64, ir: f64) -> f64 {
        (self.weighted_n_node_samples / self.weighted_n_samples)
            * (parent
                - (self.weighted_n_right / self.weighted_n_node_samples * ir)
                - (self.weighted_n_left / self.weighted_n_node_samples * il))
    }

    fn node_value(&self) -> [f64; 2] {
        [self.sum_total[0] / self.weighted_n_node_samples, self.sum_total[1] / self.weighted_n_node_samples]
    }
}

struct Split {
    feature: usize,
    pos: usize,
    threshold: f64,
    improvement: f64,
    impurity_left: f64,
    impurity_right: f64,
}

/// The settings of a forest (scikit-learn's names and defaults, but the seed's).
#[derive(Debug, Clone)]
pub struct Settings {
    pub n_estimators: usize,
    pub max_depth: Option<usize>,
    pub min_samples_split: usize,
    pub min_samples_leaf: usize,
    /// "sqrt" (the default), "log2", "all", a number of features or a share of them.
    pub max_features: serde_json::Value,
    pub bootstrap: bool,
    pub random_state: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            n_estimators: 100,
            max_depth: None,
            min_samples_split: 2,
            min_samples_leaf: 1,
            max_features: serde_json::json!("sqrt"),
            bootstrap: true,
            random_state: 1,
        }
    }
}

struct Grower<'a> {
    x: &'a [Vec<f32>],
    y: &'a [usize],
    w: &'a [f64],
    max_features: usize,
    min_samples_leaf: usize,
    min_samples_split: usize,
    max_depth: usize,
    rand: u32,
    samples: Vec<usize>,
    features: Vec<usize>,
    constant: Vec<usize>,
    values: Vec<f32>,
    crit: Criterion,
}

impl Grower<'_> {
    /// node_split_best: the best split of samples[start, end), the features drawn by Fisher-Yates.
    fn split(&mut self, start: usize, end: usize, impurity: f64, n_constant: &mut usize) -> Split {
        let n_features = self.features.len();
        let mut best = Split {
            feature: 0,
            pos: end,
            threshold: 0.0,
            improvement: f64::NEG_INFINITY,
            impurity_left: f64::INFINITY,
            impurity_right: f64::INFINITY,
        };
        let mut best_proxy = f64::NEG_INFINITY;
        let mut f_i = n_features;
        let (mut n_visited, mut n_found, mut n_drawn) = (0usize, 0usize, 0usize);
        let n_known = *n_constant;
        let mut n_total = n_known;
        while f_i > n_total && (n_visited < self.max_features || n_visited <= n_found + n_drawn) {
            n_visited += 1;
            let mut f_j = rand_int(n_drawn, f_i - n_found, &mut self.rand);
            if f_j < n_known {
                self.features.swap(n_drawn, f_j);
                n_drawn += 1;
                continue;
            }
            f_j += n_found;
            let feature = self.features[f_j];
            let col = &self.x[feature];
            for p in start..end {
                self.values[p] = col[self.samples[p]];
            }
            simultaneous_sort(&mut self.values[start..end], &mut self.samples[start..end]);
            if self.values[end - 1] <= self.values[start] + FEATURE_THRESHOLD {
                self.features.swap(f_j, n_total);
                n_found += 1;
                n_total += 1;
                continue;
            }
            f_i -= 1;
            self.features.swap(f_i, f_j);
            self.crit.reset();
            let mut p = start;
            while p < end {
                // next_p
                p += 1;
                while p < end && self.values[p] <= self.values[p - 1] + FEATURE_THRESHOLD {
                    p += 1;
                }
                let p_prev = p - 1;
                if p == end {
                    continue;
                }
                let (n_left, n_right) = (p - start, end - p);
                if n_left < self.min_samples_leaf || n_right < self.min_samples_leaf {
                    continue;
                }
                self.crit.update(self.y, self.w, &self.samples, p);
                // min_weight_leaf is 0
                if self.crit.weighted_n_left < 0.0 || self.crit.weighted_n_right < 0.0 {
                    continue;
                }
                let proxy = self.crit.proxy_impurity_improvement();
                if proxy > best_proxy {
                    best_proxy = proxy;
                    best.feature = feature;
                    best.pos = p;
                    best.threshold = f64::from(self.values[p_prev]) / 2.0 + f64::from(self.values[p]) / 2.0;
                }
            }
        }
        if best.pos < end {
            // partition_samples_final
            let col = &self.x[best.feature];
            let (mut ps, mut pe) = (start, end);
            while ps < pe {
                if f64::from(col[self.samples[ps]]) <= best.threshold {
                    ps += 1;
                } else {
                    pe -= 1;
                    self.samples.swap(ps, pe);
                }
            }
            self.crit.reset();
            self.crit.update(self.y, self.w, &self.samples, best.pos);
            let (il, ir) = self.crit.children_impurity();
            best.impurity_left = il;
            best.impurity_right = ir;
            best.improvement = self.crit.impurity_improvement(impurity, il, ir);
        }
        // the known constant features keep their order; the new ones follow them
        self.features[..n_known].copy_from_slice(&self.constant[..n_known]);
        let found: Vec<usize> = self.features[n_known..n_known + n_found].to_vec();
        self.constant[n_known..n_known + n_found].copy_from_slice(&found);
        *n_constant = n_total;
        best
    }

    /// DepthFirstTreeBuilder.build.
    fn grow(mut self, weighted_n_samples: f64) -> Tree {
        self.crit.weighted_n_samples = weighted_n_samples;
        struct Rec {
            start: usize,
            end: usize,
            depth: usize,
            parent: Option<(usize, bool)>,
            impurity: f64,
            n_constant: usize,
        }
        let mut nodes: Vec<Node> = Vec::new();
        let mut stack = vec![Rec {
            start: 0,
            end: self.samples.len(),
            depth: 0,
            parent: None,
            impurity: f64::INFINITY,
            n_constant: 0,
        }];
        let mut first = true;
        while let Some(r) = stack.pop() {
            let n_node = r.end - r.start;
            self.crit.init(self.y, self.w, &self.samples, r.start, r.end);
            let mut is_leaf = r.depth >= self.max_depth
                || n_node < self.min_samples_split
                || n_node < 2 * self.min_samples_leaf
                || self.crit.weighted_n_node_samples < 0.0;
            let mut impurity = r.impurity;
            if first {
                impurity = self.crit.node_impurity();
                first = false;
            }
            is_leaf = is_leaf || impurity <= EPSILON;
            let mut n_constant = r.n_constant;
            let mut split = None;
            if !is_leaf {
                let s = self.split(r.start, r.end, impurity, &mut n_constant);
                is_leaf = s.pos >= r.end || s.improvement + EPSILON < 0.0;
                split = Some(s);
            }
            let id = nodes.len();
            if let Some((parent, is_left)) = r.parent {
                if is_left {
                    nodes[parent].left = id as i32;
                } else {
                    nodes[parent].right = id as i32;
                }
            }
            let value = self.crit.node_value();
            match split.filter(|_| !is_leaf) {
                Some(s) => {
                    nodes.push(Node { left: -1, right: -1, feature: s.feature as i32, threshold: s.threshold, value });
                    stack.push(Rec {
                        start: s.pos,
                        end: r.end,
                        depth: r.depth + 1,
                        parent: Some((id, false)),
                        impurity: s.impurity_right,
                        n_constant,
                    });
                    stack.push(Rec {
                        start: r.start,
                        end: s.pos,
                        depth: r.depth + 1,
                        parent: Some((id, true)),
                        impurity: s.impurity_left,
                        n_constant,
                    });
                }
                None => nodes.push(Node { left: -1, right: -1, feature: -2, threshold: -2.0, value }),
            }
        }
        Tree { nodes }
    }
}

/// A fitted forest: its trees and predictors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Forest {
    pub predictors: Vec<String>,
    pub trees: Vec<Tree>,
}

fn max_features_of(v: &serde_json::Value, n: usize) -> Result<usize, String> {
    Ok(match v {
        serde_json::Value::String(s) if s == "sqrt" => ((n as f64).sqrt() as usize).max(1),
        serde_json::Value::String(s) if s == "log2" => ((n as f64).log2() as usize).max(1),
        serde_json::Value::String(s) if s == "all" => n,
        serde_json::Value::Null => n,
        serde_json::Value::Number(x) if x.is_u64() => x.as_u64().unwrap() as usize,
        serde_json::Value::Number(x) => {
            let f = x.as_f64().unwrap();
            if f > 0.0 { ((f * n as f64) as usize).max(1) } else { 0 }
        }
        other => return Err(format!("max_features: sqrt, log2, all, a number of features or a share, not {other}")),
    })
}

/// RandomForestClassifier.fit on the records (presence 1, background 0), with their weights (scikit-learn's
/// sample_weight: the bootstraps drawn with them, or the trees weighted by them without bootstraps).
pub fn fit(x: &Frame, y: &[f64], weights: Option<&[f64]>, s: &Settings) -> Result<Forest, String> {
    let n = y.len();
    if n == 0 {
        return Err("no records".into());
    }
    if y.iter().any(|v| *v != 0.0 && *v != 1.0) || !y.contains(&0.0) || !y.contains(&1.0) {
        return Err("a forest needs presences (1) and background or absences (0)".into());
    }
    if s.n_estimators == 0 || s.min_samples_leaf == 0 || s.min_samples_split < 2 {
        return Err("n_estimators and min_samples_leaf: 1 or more; min_samples_split: 2 or more".into());
    }
    let cols: Vec<Vec<f32>> = x.columns.iter().map(|c| c.iter().map(|v| *v as f32).collect()).collect();
    if cols.iter().flatten().any(|v| !v.is_finite()) {
        return Err("the records' predictors have missing values".into());
    }
    let classes: Vec<usize> = y.iter().map(|v| *v as usize).collect();
    let nf = cols.len();
    let max_features = max_features_of(&s.max_features, nf)?;
    let max_depth = s.max_depth.unwrap_or(i32::MAX as usize);
    let min_samples_split = s.min_samples_split.max(2 * s.min_samples_leaf);
    if let Some(w) = weights
        && (w.len() != n || w.iter().any(|v| !v.is_finite() || *v < 0.0))
    {
        return Err("the weights: one per record, 0 or more".into());
    }
    // a weighted bootstrap: RandomState.choice(n, n, p = w / sum(w)), its cdf
    let cdf: Option<Vec<f64>> = weights.map(|w| {
        let total = crate::change::numpy_sum(w);
        let mut acc = 0.0;
        let mut c: Vec<f64> = w
            .iter()
            .map(|v| {
                acc += v / total;
                acc
            })
            .collect();
        let last = c[n - 1];
        c.iter_mut().for_each(|v| *v /= last);
        c
    });
    let mut forest_rng = Mt19937::new(s.random_state);
    let mut trees = Vec::with_capacity(s.n_estimators);
    for _ in 0..s.n_estimators {
        let seed = forest_rng.randint(0, i64::from(i32::MAX)) as u32;
        let w: Vec<f64> = if s.bootstrap {
            let mut r = Mt19937::new(seed);
            let mut counts = vec![0.0f64; n];
            for _ in 0..n {
                let k = match &cdf {
                    // searchsorted(side = right): the cells of the cdf at or below the draw
                    Some(c) => {
                        let u = r.random_sample();
                        c.partition_point(|v| *v <= u).min(n - 1)
                    }
                    None => r.randint(0, n as i64) as usize,
                };
                counts[k] += 1.0;
            }
            counts
        } else {
            weights.map_or_else(|| vec![1.0; n], <[f64]>::to_vec)
        };
        let rand = Mt19937::new(seed).randint(0, i64::from(RAND_R_MAX)) as u32;
        let samples: Vec<usize> = (0..n).filter(|&i| w[i] != 0.0).collect();
        let weighted: f64 = w.iter().fold(0.0, |a, b| a + b);
        let g = Grower {
            x: &cols,
            y: &classes,
            w: &w,
            max_features,
            min_samples_leaf: s.min_samples_leaf,
            min_samples_split,
            max_depth,
            rand,
            values: vec![0.0; n],
            samples,
            features: (0..nf).collect(),
            constant: vec![0; nf],
            crit: Criterion {
                sum_total: [0.0; 2],
                sum_left: [0.0; 2],
                sum_right: [0.0; 2],
                weighted_n_samples: 0.0,
                weighted_n_node_samples: 0.0,
                weighted_n_left: 0.0,
                weighted_n_right: 0.0,
                start: 0,
                end: 0,
                pos: 0,
            },
        };
        trees.push(g.grow(weighted));
    }
    Ok(Forest { predictors: x.names.clone(), trees })
}

impl Forest {
    /// predict_proba's presence column: the trees' probabilities added in their order, over their number.
    pub fn predict(&self, x: &Frame) -> Result<Vec<f64>, String> {
        let cols: Vec<Vec<f32>> = self
            .predictors
            .iter()
            .map(|p| {
                let j = x.names.iter().position(|q| q == p).ok_or_else(|| format!("no predictor {p:?}"))?;
                Ok(x.columns[j].iter().map(|v| *v as f32).collect())
            })
            .collect::<Result<_, String>>()?;
        let n = cols.first().map_or(0, Vec::len);
        let mut out = vec![0.0; n];
        let mut row = vec![0f32; cols.len()];
        for tree in &self.trees {
            for (i, o) in out.iter_mut().enumerate() {
                for (r, c) in row.iter_mut().zip(&cols) {
                    *r = c[i];
                }
                *o += tree.leaf(&row)[1];
            }
        }
        let k = self.trees.len() as f64;
        // no value where a predictor has none
        Ok(out
            .into_iter()
            .enumerate()
            .map(|(i, v)| if cols.iter().any(|c| c[i].is_nan()) { f64::NAN } else { v / k })
            .collect())
    }
}
