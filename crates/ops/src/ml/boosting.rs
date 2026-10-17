//! scikit-learn's histogram gradient boosting (1.9), HistGradientBoostingClassifier and Regressor without early
//! stopping: the predictors binned by their quantiles (numpy's averaged inverted cdf, at most 255 bins and the one
//! of missing values), then at every iteration the gradients and hessians of the loss (half squared error, half
//! binomial or half multinomial, as float32) and a tree per class grown best leaf first from histograms of their
//! sums (a node's built for its smaller child, the larger's by subtraction), Python's heap breaking the ties of
//! gain; the leaves shrunk by the learning rate and added to the raw predictions. The same trees and predictions as
//! scikit-learn's.

use serde::{Deserialize, Serialize};

use super::forest::Mt19937;
use crate::change::numpy_sum;
use crate::sdm::extrapolation::Frame;
use crate::signatures::NumpyRng;

/// The settings (scikit-learn's names and defaults, but the seed's and early stopping, always off).
#[derive(Debug, Clone)]
pub struct Settings {
    pub learning_rate: f64,
    pub max_iter: usize,
    pub max_leaf_nodes: Option<usize>,
    pub max_depth: Option<usize>,
    pub min_samples_leaf: usize,
    pub l2_regularization: f64,
    /// The share of the predictors tried at each split.
    pub max_features: f64,
    pub max_bins: usize,
    pub random_state: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            learning_rate: 0.1,
            max_iter: 100,
            max_leaf_nodes: Some(31),
            max_depth: None,
            min_samples_leaf: 20,
            l2_regularization: 0.0,
            max_features: 1.0,
            max_bins: 255,
            random_state: 0,
        }
    }
}

const MIN_HESSIAN_TO_SPLIT: f64 = 1e-3;
const ALMOST_INF: f64 = 1e300;
/// The bins of a predictor: those of its values and the one of missing values.
const N_BINS: usize = 256;

// -- binning --------------------------------------------------------------------------------------------------------

/// numpy's percentile (method averaged_inverted_cdf) of sorted values at `q` (0-1).
fn averaged_inverted_cdf(sorted: &[f64], q: f64) -> f64 {
    let n = sorted.len();
    let virtual_index = n as f64 * q - 1.0;
    let (mut prev, mut next) = (virtual_index.floor(), virtual_index.floor() + 1.0);
    if virtual_index >= (n - 1) as f64 {
        prev = -1.0;
        next = -1.0;
    }
    if virtual_index < 0.0 {
        prev = 0.0;
        next = 0.0;
    }
    let gamma = if virtual_index - prev == 0.0 { 0.5 } else { 1.0 };
    let at = |i: f64| if i < 0.0 { sorted[n - 1] } else { sorted[i as usize] };
    let (a, b) = (at(prev), at(next));
    let diff = b - a;
    if gamma >= 0.5 { b - diff * (1.0 - gamma) } else { a + diff * gamma }
}

/// _find_binning_thresholds: the values separating the bins of a predictor (its distinct values' midpoints, or its
/// quantiles when it has more than `max_bins`).
fn binning_thresholds(col: &[f64], max_bins: usize) -> Vec<f64> {
    let mut sorted: Vec<f64> = col.iter().copied().filter(|v| !v.is_nan()).collect();
    sorted.sort_by(f64::total_cmp);
    let mut distinct = sorted.clone();
    distinct.dedup();
    if distinct.len() <= 1 {
        return Vec::new();
    }
    let mut t: Vec<f64> = if distinct.len() <= max_bins {
        distinct.windows(2).map(|w| (w[0] + w[1]) / 2.0).collect()
    } else {
        let step = 100.0 / max_bins as f64;
        (1..max_bins).map(|i| averaged_inverted_cdf(&sorted, (i as f64 * step) / 100.0)).collect()
    };
    let mut unique = t.clone();
    unique.sort_by(f64::total_cmp);
    unique.dedup();
    if unique.len() != t.len() {
        t = unique;
    }
    t.iter_mut().for_each(|v| *v = v.min(ALMOST_INF));
    t
}

/// The bin of a value: i where thresholds[i - 1] < x <= thresholds[i] (the last bin for a missing value).
fn bin_of(x: f64, t: &[f64]) -> u8 {
    if x.is_nan() {
        return (N_BINS - 1) as u8;
    }
    let (mut left, mut right) = (0usize, t.len());
    while left < right {
        let middle = left + (right - left - 1) / 2;
        if x <= t[middle] {
            right = middle;
        } else {
            left = middle + 1;
        }
    }
    left as u8
}

// -- the trees ------------------------------------------------------------------------------------------------------

/// A node of a fitted tree: a leaf's value, or the predictor and threshold of a split.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub value: f64,
    /// The predictor of the split; -1 for a leaf.
    pub feature: i32,
    pub threshold: f64,
    pub missing_left: bool,
    pub left: u32,
    pub right: u32,
    /// The split's gain (0 for a leaf).
    pub gain: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree {
    pub nodes: Vec<Node>,
}

impl Tree {
    fn predict(&self, row: &[f64]) -> f64 {
        let mut k = 0usize;
        loop {
            let n = &self.nodes[k];
            if n.feature < 0 {
                return n.value;
            }
            let x = row[n.feature as usize];
            let left = if x.is_nan() { n.missing_left } else { x <= n.threshold };
            k = if left { n.left as usize } else { n.right as usize };
        }
    }
}

/// A bin of a histogram: the sums of the gradients and hessians of its samples, and their count.
#[derive(Debug, Clone, Copy, Default)]
struct Bin {
    g: f64,
    h: f64,
    count: u32,
}

#[derive(Debug, Clone, Copy)]
struct Split {
    gain: f64,
    feature: usize,
    bin: usize,
    missing_left: bool,
    g_left: f64,
    g_right: f64,
    h_left: f64,
    h_right: f64,
    value_left: f64,
    value_right: f64,
}

/// A node of the tree being grown: its samples (a range of the partition) and sums.
struct GrowNode {
    depth: usize,
    start: usize,
    stop: usize,
    g: f64,
    h: f64,
    value: f64,
    leaf: bool,
    split: Option<Split>,
    children: Option<(usize, usize)>,
    hist: Option<Vec<Bin>>,
}

fn node_value(g: f64, h: f64, l2: f64) -> f64 {
    -g / (h + l2 + 1e-15)
}

struct Grower<'a> {
    binned: &'a [Vec<u8>],
    gradients: &'a [f32],
    /// None: the hessians are constant (1).
    hessians: Option<&'a [f32]>,
    n_bins_non_missing: &'a [usize],
    s: &'a Settings,
    rng: &'a mut NumpyRng,
    partition: Vec<u32>,
    nodes: Vec<GrowNode>,
    heap: Vec<usize>,
    leaves: Vec<usize>,
}

impl Grower<'_> {
    fn gain(&self, n: usize) -> f64 {
        self.nodes[n].split.map_or(-1.0, |s| s.gain)
    }

    /// TreeNode.__lt__: the node of the larger gain first.
    fn less(&self, a: usize, b: usize) -> bool {
        self.gain(a) > self.gain(b)
    }

    // Python's heapq
    fn sift_down(&mut self, start: usize, mut pos: usize) {
        let item = self.heap[pos];
        while pos > start {
            let parent = (pos - 1) >> 1;
            if self.less(item, self.heap[parent]) {
                self.heap[pos] = self.heap[parent];
                pos = parent;
                continue;
            }
            break;
        }
        self.heap[pos] = item;
    }

    fn sift_up(&mut self, mut pos: usize) {
        let end = self.heap.len();
        let start = pos;
        let item = self.heap[pos];
        let mut child = 2 * pos + 1;
        while child < end {
            let right = child + 1;
            if right < end && !self.less(self.heap[child], self.heap[right]) {
                child = right;
            }
            self.heap[pos] = self.heap[child];
            pos = child;
            child = 2 * pos + 1;
        }
        self.heap[pos] = item;
        self.sift_down(start, pos);
    }

    fn push(&mut self, n: usize) {
        self.heap.push(n);
        let last = self.heap.len() - 1;
        self.sift_down(0, last);
    }

    fn pop(&mut self) -> usize {
        let last = self.heap.pop().unwrap();
        if self.heap.is_empty() {
            return last;
        }
        let top = self.heap[0];
        self.heap[0] = last;
        self.sift_up(0);
        top
    }

    /// The histograms of the samples of a range of the partition, a predictor after the other.
    fn histograms(&self, start: usize, stop: usize) -> Vec<Bin> {
        use rayon::prelude::*;
        let samples = &self.partition[start..stop];
        let mut hist = vec![Bin::default(); self.binned.len() * N_BINS];
        hist.par_chunks_mut(N_BINS).enumerate().for_each(|(f, out)| {
            let col = &self.binned[f];
            for &i in samples {
                let b = &mut out[col[i as usize] as usize];
                b.g += f64::from(self.gradients[i as usize]);
                if let Some(h) = self.hessians {
                    b.h += f64::from(h[i as usize]);
                }
                b.count += 1;
            }
        });
        hist
    }

    /// _find_best_bin_to_split_left_to_right of a predictor (no missing values).
    fn best_bin(&self, f: usize, hist: &[Bin], n: u32, g: f64, h: f64, value: f64) -> Option<Split> {
        let s = self.s;
        let end = self.n_bins_non_missing[f] - 1;
        let (mut gl, mut hl, mut nl) = (0.0f64, 0.0f64, 0u32);
        let loss_current = g * value;
        let mut best: Option<Split> = None;
        let mut best_gain = -1.0;
        for bin in 0..end {
            let b = hist[f * N_BINS + bin];
            nl += b.count;
            let nr = n - nl;
            if self.hessians.is_some() {
                hl += b.h;
            } else {
                hl += f64::from(b.count);
            }
            let hr = h - hl;
            gl += b.g;
            let gr = g - gl;
            if (nl as usize) < s.min_samples_leaf {
                continue;
            }
            if (nr as usize) < s.min_samples_leaf {
                break;
            }
            if hl < MIN_HESSIAN_TO_SPLIT {
                continue;
            }
            if hr < MIN_HESSIAN_TO_SPLIT {
                break;
            }
            let vl = node_value(gl, hl, s.l2_regularization);
            let vr = node_value(gr, hr, s.l2_regularization);
            let mut gain = loss_current;
            gain -= gl * vl;
            gain -= gr * vr;
            if gain > best_gain && gain > 0.0 {
                best_gain = gain;
                best = Some(Split {
                    gain,
                    feature: f,
                    bin,
                    missing_left: false,
                    g_left: gl,
                    g_right: 0.0,
                    h_left: hl,
                    h_right: 0.0,
                    value_left: 0.0,
                    value_right: 0.0,
                });
            }
        }
        best.map(|mut b| {
            b.g_right = g - b.g_left;
            b.h_right = h - b.h_left;
            b.value_left = node_value(b.g_left, b.h_left, s.l2_regularization);
            b.value_right = node_value(b.g_right, b.h_right, s.l2_regularization);
            b
        })
    }

    /// Splitter.find_node_split: the best split over the predictors (a share of them drawn when max_features < 1),
    /// the first of the largest gain.
    fn find_split(&mut self, node: usize) -> Option<Split> {
        let nf = self.binned.len();
        let mask: Option<Vec<bool>> = (self.s.max_features < 1.0).then(|| {
            let k = ((self.s.max_features * nf as f64).ceil() as usize).max(1);
            let mut m: Vec<bool> = (0..nf).map(|i| i < k).collect();
            self.rng.shuffle(&mut m);
            m
        });
        let nd = &self.nodes[node];
        let hist = nd.hist.as_ref().unwrap();
        let n = (nd.stop - nd.start) as u32;
        let mut best: Option<Split> = None;
        for f in 0..nf {
            if mask.as_ref().is_some_and(|m| !m[f]) {
                continue;
            }
            if let Some(s) = self.best_bin(f, hist, n, nd.g, nd.h, nd.value)
                && best.is_none_or(|b| s.gain > b.gain)
            {
                best = Some(s);
            }
        }
        best
    }

    fn finalize(&mut self, n: usize) {
        self.nodes[n].leaf = true;
        self.leaves.push(n);
    }

    fn compute_and_push(&mut self, n: usize) {
        let split = self.find_split(n);
        self.nodes[n].split = split;
        if split.is_none_or(|s| s.gain <= 0.0) {
            self.finalize(n);
        } else {
            self.push(n);
        }
    }

    fn new_node(&mut self, depth: usize, start: usize, stop: usize, g: f64, h: f64, value: f64) -> usize {
        self.nodes.push(GrowNode {
            depth,
            start,
            stop,
            g,
            h,
            value,
            leaf: false,
            split: None,
            children: None,
            hist: None,
        });
        self.nodes.len() - 1
    }

    fn grow(&mut self) {
        let n = self.partition.len();
        let hist = self.histograms(0, n);
        let g = numpy_sum(&hist[..N_BINS].iter().map(|b| b.g).collect::<Vec<_>>());
        let h = match self.hessians {
            None => n as f64,
            Some(_) => numpy_sum(&hist[..N_BINS].iter().map(|b| b.h).collect::<Vec<_>>()),
        };
        let root = self.new_node(0, 0, n, g, h, 0.0);
        let msl = self.s.min_samples_leaf;
        if n < 2 * msl || h < MIN_HESSIAN_TO_SPLIT {
            self.finalize(root);
        } else {
            self.nodes[root].hist = Some(hist);
            self.compute_and_push(root);
        }
        while !self.heap.is_empty() {
            self.split_next();
        }
        let shrinkage = self.s.learning_rate;
        for &l in &self.leaves {
            self.nodes[l].value *= shrinkage;
        }
    }

    fn split_next(&mut self) {
        let node = self.pop();
        let split = self.nodes[node].split.unwrap();
        let (start, stop, depth) = (self.nodes[node].start, self.nodes[node].stop, self.nodes[node].depth + 1);
        // a stable partition of the node's samples: those to the left first
        let col = &self.binned[split.feature];
        let (left, right): (Vec<u32>, Vec<u32>) =
            self.partition[start..stop].iter().partition(|&&i| usize::from(col[i as usize]) <= split.bin);
        let mid = start + left.len();
        self.partition[start..mid].copy_from_slice(&left);
        self.partition[mid..stop].copy_from_slice(&right);
        let n_leaf_nodes = self.leaves.len() + self.heap.len() + 2;
        let l = self.new_node(depth, start, mid, split.g_left, split.h_left, split.value_left);
        let r = self.new_node(depth, mid, stop, split.g_right, split.h_right, split.value_right);
        self.nodes[node].children = Some((l, r));
        // no missing values: they would go to the child of more samples
        if let Some(s) = self.nodes[node].split.as_mut() {
            s.missing_left = left.len() > right.len();
        }
        let s = self.s;
        if s.max_leaf_nodes == Some(n_leaf_nodes) {
            self.finalize(l);
            self.finalize(r);
            while let Some(k) = self.heap.pop() {
                self.finalize(k);
            }
            return;
        }
        if s.max_depth == Some(depth) {
            self.finalize(l);
            self.finalize(r);
            return;
        }
        let (nl, nr) = (mid - start, stop - mid);
        if nl < 2 * s.min_samples_leaf {
            self.finalize(l);
        }
        if nr < 2 * s.min_samples_leaf {
            self.finalize(r);
        }
        let (split_l, split_r) = (!self.nodes[l].leaf, !self.nodes[r].leaf);
        if split_l || split_r {
            let (small, large) = if nl < nr { (l, r) } else { (r, l) };
            let sh = self.histograms(self.nodes[small].start, self.nodes[small].stop);
            let mut lh = self.nodes[node].hist.take().unwrap();
            for (a, b) in lh.iter_mut().zip(&sh) {
                a.g -= b.g;
                a.h -= b.h;
                a.count -= b.count;
            }
            self.nodes[small].hist = Some(sh);
            self.nodes[large].hist = Some(lh);
            if split_l {
                self.compute_and_push(l);
            }
            if split_r {
                self.compute_and_push(r);
            }
            for c in [l, r] {
                if self.nodes[c].leaf {
                    self.nodes[c].hist = None;
                }
            }
        }
        self.nodes[node].hist = None;
    }

    /// The fitted tree (_fill_predictor_arrays: a node, then its left subtree, then its right).
    fn tree(&self, thresholds: &[Vec<f64>]) -> Tree {
        fn fill(g: &Grower, k: usize, thresholds: &[Vec<f64>], out: &mut Vec<Node>) {
            let n = &g.nodes[k];
            let id = out.len();
            out.push(Node {
                value: n.value,
                feature: -1,
                threshold: 0.0,
                missing_left: false,
                left: 0,
                right: 0,
                gain: 0.0,
            });
            if n.leaf {
                return;
            }
            let s = n.split.unwrap();
            let (l, r) = n.children.unwrap();
            out[id].feature = s.feature as i32;
            out[id].missing_left = s.missing_left;
            out[id].gain = s.gain;
            out[id].threshold =
                if s.bin == g.n_bins_non_missing[s.feature] - 1 { f64::INFINITY } else { thresholds[s.feature][s.bin] };
            out[id].left = out.len() as u32;
            fill(g, l, thresholds, out);
            out[id].right = out.len() as u32;
            fill(g, r, thresholds, out);
        }
        let mut nodes = Vec::new();
        fill(self, 0, thresholds, &mut nodes);
        Tree { nodes }
    }
}

// -- the model ------------------------------------------------------------------------------------------------------

/// A fitted model: its predictors, its classes (none for a regression), the raw prediction it starts from (one per
/// class of a multinomial model, else one) and its trees (per iteration, one per class of a multinomial model).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Boosting {
    pub predictors: Vec<String>,
    pub classes: Vec<f64>,
    pub baseline: Vec<f64>,
    pub trees: Vec<Vec<Tree>>,
}

/// scipy's logit (xsf's: log1p's form between 0.3 and 0.65).
fn logit(x: f64) -> f64 {
    if !(0.3..=0.65).contains(&x) {
        (x / (1.0 - x)).ln()
    } else {
        let s = 2.0 * (x - 0.5);
        s.ln_1p() - (-s).ln_1p()
    }
}

/// HistGradientBoostingClassifier.fit: the classes are the distinct values of `y`, increasing.
pub fn classifier(x: &Frame, y: &[f64], s: &Settings) -> Result<Boosting, String> {
    if y.iter().any(|v| !v.is_finite()) {
        return Err("the classes have missing values".into());
    }
    let mut classes = y.to_vec();
    classes.sort_by(f64::total_cmp);
    classes.dedup();
    if classes.len() < 2 {
        return Err("a classifier needs two classes or more".into());
    }
    let codes: Vec<f64> = y.iter().map(|v| classes.partition_point(|c| c < v) as f64).collect();
    fit(x, &codes, classes, s)
}

/// HistGradientBoostingRegressor.fit (the half squared error).
pub fn regressor(x: &Frame, y: &[f64], s: &Settings) -> Result<Boosting, String> {
    if y.iter().any(|v| !v.is_finite()) {
        return Err("the values have missing values".into());
    }
    fit(x, y, Vec::new(), s)
}

fn fit(x: &Frame, y: &[f64], classes: Vec<f64>, s: &Settings) -> Result<Boosting, String> {
    use rayon::prelude::*;
    let n = y.len();
    if n == 0 {
        return Err("no records".into());
    }
    if x.columns.iter().flatten().any(|v| v.is_nan()) {
        return Err("the records' predictors have missing values".into());
    }
    if !(2..=255).contains(&s.max_bins) || s.max_iter == 0 || s.min_samples_leaf == 0 {
        return Err("max_bins: 2 to 255; max_iter and min_samples_leaf: 1 or more".into());
    }
    if s.max_leaf_nodes.is_some_and(|m| m < 2) || s.max_depth.is_some_and(|d| d < 1) {
        return Err("max_leaf_nodes: 2 or more; max_depth: 1 or more".into());
    }
    if !(s.max_features > 0.0 && s.max_features <= 1.0) {
        return Err("max_features: a share of the predictors, above 0 and at most 1".into());
    }
    // the seeds of the binning's subsample and of the predictors drawn at each split
    let mut rng = Mt19937::new(s.random_state);
    let seed = rng.randint(0, i64::from(u32::MAX)) as u32;
    let feature_seed = rng.randint(0, i64::from(u32::MAX)) as u64;
    let mut feature_rng = NumpyRng::new(feature_seed);
    // the bins, their thresholds from a subsample of 200,000 records drawn with replacement
    let thresholds: Vec<Vec<f64>> = if n > 200_000 {
        let mut r = Mt19937::new(seed);
        let pick: Vec<usize> = (0..200_000).map(|_| r.randint(0, n as i64) as usize).collect();
        x.columns
            .par_iter()
            .map(|c| binning_thresholds(&pick.iter().map(|&i| c[i]).collect::<Vec<_>>(), s.max_bins))
            .collect()
    } else {
        x.columns.par_iter().map(|c| binning_thresholds(c, s.max_bins)).collect()
    };
    let binned: Vec<Vec<u8>> =
        x.columns.par_iter().zip(&thresholds).map(|(c, t)| c.iter().map(|&v| bin_of(v, t)).collect()).collect();
    let n_bins_non_missing: Vec<usize> = thresholds.iter().map(|t| t.len() + 1).collect();
    // the raw prediction of a model of the intercept alone
    let k_trees = if classes.len() > 2 { classes.len() } else { 1 };
    let baseline: Vec<f64> = if classes.is_empty() {
        vec![numpy_sum(y) / n as f64]
    } else if classes.len() == 2 {
        let eps = 10.0 * f64::EPSILON;
        vec![logit((numpy_sum(y) / n as f64).clamp(eps, 1.0 - eps))]
    } else {
        let p: Vec<f64> = (0..k_trees)
            .map(|k| {
                (y.iter().filter(|&&v| v == k as f64).count() as f64 / n as f64).clamp(f64::EPSILON, 1.0 - f64::EPSILON)
            })
            .collect();
        let logs: Vec<f64> = p.iter().map(|v| v.ln()).collect();
        let gm = (numpy_sum(&logs) / k_trees as f64).exp();
        p.iter().map(|v| (v / gm).ln()).collect()
    };
    let mut raw: Vec<Vec<f64>> = baseline.iter().map(|&b| vec![b; n]).collect();
    let mut trees = Vec::with_capacity(s.max_iter);
    let mut gradients = vec![vec![0f32; n]; k_trees];
    let mut hessians = vec![vec![0f32; n]; k_trees];
    for _ in 0..s.max_iter {
        // the gradients and hessians of the loss at the raw predictions
        if classes.is_empty() {
            gradients[0].par_iter_mut().enumerate().for_each(|(i, g)| *g = (raw[0][i] - y[i]) as f32);
        } else if classes.len() == 2 {
            let (g0, h0) = (&mut gradients[0], &mut hessians[0]);
            g0.par_iter_mut().zip(h0.par_iter_mut()).enumerate().for_each(|(i, (g, h))| {
                let (r, t) = (raw[0][i], y[i]);
                if r > -37.0 {
                    let e = (-r).exp();
                    *g = (((1.0 - t) - t * e) / (1.0 + e)) as f32;
                    *h = (e / (1.0 + e).powf(2.0)) as f32;
                } else {
                    let e = r.exp();
                    *g = (e - t) as f32;
                    *h = e as f32;
                }
            });
        } else {
            let gh: Vec<(Vec<f32>, Vec<f32>)> = (0..n)
                .into_par_iter()
                .map(|i| {
                    let mut max = raw[0][i];
                    for r in raw.iter().skip(1) {
                        if max < r[i] {
                            max = r[i];
                        }
                    }
                    let mut p: Vec<f64> = raw.iter().map(|r| (r[i] - max).exp()).collect();
                    let mut sum = 0.0;
                    for v in &p {
                        sum += v;
                    }
                    p.iter_mut().for_each(|v| *v /= sum);
                    let g = p.iter().enumerate().map(|(k, v)| (v - f64::from(u8::from(y[i] == k as f64))) as f32);
                    (g.collect(), p.iter().map(|v| (v * (1.0 - v)) as f32).collect())
                })
                .collect();
            for (i, (g, h)) in gh.into_iter().enumerate() {
                for k in 0..k_trees {
                    gradients[k][i] = g[k];
                    hessians[k][i] = h[k];
                }
            }
        }
        let mut iteration = Vec::with_capacity(k_trees);
        for k in 0..k_trees {
            let mut grower = Grower {
                binned: &binned,
                gradients: &gradients[k],
                hessians: (!classes.is_empty()).then_some(&hessians[k][..]),
                n_bins_non_missing: &n_bins_non_missing,
                s,
                rng: &mut feature_rng,
                partition: (0..n as u32).collect(),
                nodes: Vec::new(),
                heap: Vec::new(),
                leaves: Vec::new(),
            };
            grower.grow();
            for &l in &grower.leaves {
                let nd = &grower.nodes[l];
                for &i in &grower.partition[nd.start..nd.stop] {
                    raw[k][i as usize] += nd.value;
                }
            }
            iteration.push(grower.tree(&thresholds));
        }
        trees.push(iteration);
    }
    Ok(Boosting { predictors: x.names.clone(), classes, baseline, trees })
}

impl Boosting {
    /// The raw predictions of the rows (a column per tree of an iteration), the trees added in their order.
    fn raw(&self, x: &Frame) -> Result<Vec<Vec<f64>>, String> {
        use rayon::prelude::*;
        let cols: Vec<&Vec<f64>> = self
            .predictors
            .iter()
            .map(|p| {
                let j = x.names.iter().position(|q| q == p).ok_or_else(|| format!("no predictor {p:?}"))?;
                Ok(&x.columns[j])
            })
            .collect::<Result<_, String>>()?;
        let n = cols.first().map_or(0, |c| c.len());
        let k = self.baseline.len();
        let rows: Vec<Vec<f64>> = (0..n)
            .into_par_iter()
            .map(|i| {
                let row: Vec<f64> = cols.iter().map(|c| c[i]).collect();
                if row.iter().any(|v| v.is_nan()) {
                    return vec![f64::NAN; k];
                }
                let mut out = vec![0.0; k];
                for (o, b) in out.iter_mut().zip(&self.baseline) {
                    *o += b;
                }
                for it in &self.trees {
                    for (o, t) in out.iter_mut().zip(it) {
                        *o += t.predict(&row);
                    }
                }
                out
            })
            .collect();
        Ok((0..k).map(|c| rows.iter().map(|r| r[c]).collect()).collect())
    }

    /// predict_proba (a column per class), or predict for a regression (one column); NaN where a predictor has none.
    pub fn outputs(&self, x: &Frame) -> Result<Vec<Vec<f64>>, String> {
        let raw = self.raw(x)?;
        if self.classes.is_empty() {
            return Ok(raw);
        }
        if self.classes.len() == 2 {
            let p1: Vec<f64> = raw[0].iter().map(|r| 1.0 / (1.0 + (-r).exp())).collect();
            return Ok(vec![p1.iter().map(|p| 1.0 - p).collect(), p1]);
        }
        // softmax: the largest subtracted, the exponentials over their sum
        let k = raw.len();
        let n = raw[0].len();
        let mut out = vec![vec![0.0; n]; k];
        for i in 0..n {
            let mut max = raw[0][i];
            for r in &raw[1..] {
                max = max.max(r[i]);
            }
            let e: Vec<f64> = raw.iter().map(|r| (r[i] - max).exp()).collect();
            let mut sum = e[0];
            for v in &e[1..] {
                sum += v;
            }
            for c in 0..k {
                out[c][i] = e[c] / sum;
            }
        }
        Ok(out)
    }

    /// Each predictor's share of the gain of the splits on it, over every tree (scikit-learn gives none).
    pub fn importances(&self) -> Vec<f64> {
        let mut imp = vec![0.0; self.predictors.len()];
        for t in self.trees.iter().flatten() {
            for n in &t.nodes {
                if n.feature >= 0 {
                    imp[n.feature as usize] += n.gain;
                }
            }
        }
        let total: f64 = imp.iter().sum();
        if total > 0.0 {
            imp.iter_mut().for_each(|v| *v /= total);
        }
        imp
    }
}
