//! Self-Organizing Maps: the engine of Zeit (rust/zeit-core/src/som.rs, GPL-3.0, the same author), itself an
//! operation-by-operation port of MiniSom 2.3.x (G. Vettigli, MIT): `train_online` is MiniSom.train(), `train_batch`
//! MiniSom.train_batch_offline(). NumPy's pairwise summation for the distances, argmin over the square-rooted
//! distance, the same neighbourhood, decay and update expressions, so the same bits as MiniSom. Changes from Zeit's:
//! the parallel batch runs on the rayon pool of the caller (the sums by fixed blocks of neurons, each in sample order,
//! so the same bits on any number of threads) instead of Zeit's own team of threads, the hexagonal topology (MiniSom's
//! coordinates; Zeit's engine takes them from the caller), the second best matching unit (the topographic error).

// The kernels index neurons and features as the C++ does.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// exp from the C runtime, one value at a time (as NumPy's).
#[inline(never)]
fn scalar_exp(v: f64) -> f64 {
    v.exp()
}

/// pow(v, 2.0): NumPy squares (v * v), the correctly rounded value pow gives.
#[inline]
fn square(v: f64) -> f64 {
    v * v
}

/// NumPy's DOUBLE_pairwise_sum.
fn pairwise_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0;
        for &v in a {
            res += v;
        }
        return res;
    }
    if n <= 128 {
        let mut r: [f64; 8] = a[..8].try_into().expect("8 values");
        let mut i = 8;
        while i < n - n % 8 {
            for j in 0..8 {
                r[j] += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        return res;
    }
    let mut n2 = n / 2;
    n2 -= n2 % 8;
    pairwise_sum(&a[..n2]) + pairwise_sum(&a[n2..])
}

/// The squared distance as NumPy rounds it: add.reduce((x - w) * (x - w)).
#[inline]
fn squared_distance(x: &[f64], w: &[f64], buf: &mut [f64]) -> f64 {
    let d_n = x.len();
    if d_n < 8 {
        let mut res = 0.0;
        for d in 0..d_n {
            let diff = x[d] - w[d];
            res += diff * diff;
        }
        return res;
    }
    if d_n <= 128 {
        let mut r = [0.0; 8];
        for j in 0..8 {
            let diff = x[j] - w[j];
            r[j] = diff * diff;
        }
        let mut d = 8;
        while d < d_n - d_n % 8 {
            for j in 0..8 {
                let diff = x[d + j] - w[d + j];
                r[j] += diff * diff;
            }
            d += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while d < d_n {
            let diff = x[d] - w[d];
            res += diff * diff;
            d += 1;
        }
        return res;
    }
    for d in 0..d_n {
        let diff = x[d] - w[d];
        buf[d] = diff * diff;
    }
    pairwise_sum(&buf[..d_n])
}

/// argmin over sqrt(squared distance), the first index on ties (np.argmin); the square root only when the squared
/// distance improves.
fn find_bmu(x: &[f64], w: &[f64], k_n: usize, buf: &mut [f64]) -> usize {
    let d_n = x.len();
    let (mut bmu, mut best_sq, mut best) = (0, f64::INFINITY, f64::INFINITY);
    for k in 0..k_n {
        let sq = squared_distance(x, &w[k * d_n..(k + 1) * d_n], buf);
        if sq < best_sq {
            let dist = sq.sqrt();
            if dist < best {
                best = dist;
                best_sq = sq;
                bmu = k;
            }
        }
    }
    bmu
}

/// The two nearest neurons of `x` (the first by index on ties) and the distance to the first: what the quantisation
/// and topographic errors need.
fn find_two(x: &[f64], w: &[f64], k_n: usize, buf: &mut [f64]) -> (usize, usize, f64) {
    let d_n = x.len();
    let (mut first, mut second) = ((0, f64::INFINITY), (0, f64::INFINITY));
    for k in 0..k_n {
        let sq = squared_distance(x, &w[k * d_n..(k + 1) * d_n], buf);
        if sq < first.1 {
            second = first;
            first = (k, sq);
        } else if sq < second.1 {
            second = (k, sq);
        }
    }
    (first.0, if k_n > 1 { second.0 } else { first.0 }, first.1.sqrt())
}

fn decay_learning_rate(code: i32, lr: f64, t: f64, max_iter: f64) -> f64 {
    match code {
        1 => {
            let c = max_iter / 100.0;
            lr * c / (c + t)
        }
        2 => lr * (1.0 - t / max_iter),
        _ => lr / (1.0 + t / (max_iter / 2.0)),
    }
}

fn decay_sigma(code: i32, sigma: f64, t: f64, max_iter: f64) -> f64 {
    match code {
        1 => {
            let c = (sigma - 1.0) / max_iter;
            sigma / (1.0 + (t * c))
        }
        2 => sigma + (t * (1.0 - sigma) / max_iter),
        _ => sigma / (1.0 + t / (max_iter / 2.0)),
    }
}

/// How the nodes lie: in rows and columns, or as a hexagonal grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topology {
    Rectangular,
    Hexagonal,
}

/// MiniSom's rows of a hexagonal map: counted from the last, every other one is shifted half a node (with an even
/// number of rows, the odd rows). `row` from 0.
pub fn shifted(row: usize, rows: usize) -> bool {
    (rows - 1 - row).is_multiple_of(2)
}

/// MiniSom's y spacing of a hexagonal map's rows, (3 / 2) / sqrt(3).
fn hex_factor() -> f64 {
    (3.0 / 2.0) / 3.0_f64.sqrt()
}

/// The map: MiniSom's euclidean neuron coordinates (flattened in (i, j) order), and their distinct values for the
/// separable gaussian.
pub struct Grid {
    x: usize,
    y: usize,
    k: usize,
    xx: Vec<f64>,
    yy: Vec<f64>,
    ux: Vec<f64>,
    uy: Vec<f64>,
    ix: Vec<usize>,
    iy: Vec<usize>,
}

impl Grid {
    pub fn new(x: usize, y: usize, xx: &[f64], yy: &[f64]) -> Result<Grid, &'static str> {
        if xx.len() != x * y || yy.len() != x * y {
            return Err("xx/yy must contain one coordinate per neuron");
        }
        let unique = |v: &[f64]| {
            let mut u = v.to_vec();
            u.sort_by(f64::total_cmp);
            u.dedup();
            let idx = v.iter().map(|&a| u.partition_point(|&b| b < a)).collect::<Vec<_>>();
            (u, idx)
        };
        let (ux, ix) = unique(xx);
        let (uy, iy) = unique(yy);
        Ok(Grid { x, y, k: x * y, xx: xx.to_vec(), yy: yy.to_vec(), ux, uy, ix, iy })
    }

    /// MiniSom's map of `x` by `y` neurons (the neuron (i, j) is `i * y + j`): its coordinates, i along a row and j the
    /// row. The hexagonal topology shifts some rows half a neuron (`shifted`) and spaces the rows by `hex_factor`.
    pub fn minisom(x: usize, y: usize, topology: Topology) -> Grid {
        let hex = topology == Topology::Hexagonal;
        let mut xx = Vec::with_capacity(x * y);
        let mut yy = Vec::with_capacity(x * y);
        for i in 0..x {
            for j in 0..y {
                let mut cx = i as f64;
                let mut cy = j as f64;
                if hex {
                    if shifted(j, y) {
                        cx -= 0.5;
                    }
                    cy *= hex_factor();
                }
                xx.push(cx);
                yy.push(cy);
            }
        }
        Grid::new(x, y, &xx, &yy).expect("one coordinate per neuron")
    }

    /// The neurons.
    pub fn len(&self) -> usize {
        self.k
    }

    pub fn is_empty(&self) -> bool {
        self.k == 0
    }

    /// The coordinates of every neuron, in (i, j) order.
    pub fn coordinates(&self) -> (&[f64], &[f64]) {
        (&self.xx, &self.yy)
    }

    /// The neighborhood of winner `c` with spread `sigma`, into h (K values).
    fn neighborhood(&self, code: i32, c: usize, sigma: f64, h: &mut [f64], ax: &mut [f64], ay: &mut [f64]) {
        let (ci, cj) = ((c / self.y) as i64, (c % self.y) as i64);
        match code {
            1 => {
                // mexican_hat
                let d = 2.0 * sigma * sigma;
                for k in 0..self.k {
                    let p = square(self.xx[k] - self.xx[c]) + square(self.yy[k] - self.yy[c]);
                    h[k] = scalar_exp(-p / d) * (1.0 - 2.0 / d * p);
                }
            }
            2 => {
                // bubble
                for i in 0..self.x {
                    let fi = i as f64;
                    ax[i] = if fi > ci as f64 - sigma && fi < ci as f64 + sigma { 1.0 } else { 0.0 };
                }
                for j in 0..self.y {
                    let fj = j as f64;
                    ay[j] = if fj > cj as f64 - sigma && fj < cj as f64 + sigma { 1.0 } else { 0.0 };
                }
                for i in 0..self.x {
                    for j in 0..self.y {
                        h[i * self.y + j] = ax[i] * ay[j];
                    }
                }
            }
            3 => {
                // triangle
                for i in 0..self.x {
                    let v = (-(ci - i as i64).abs()) as f64 + sigma;
                    ax[i] = if v < 0.0 { 0.0 } else { v };
                }
                for j in 0..self.y {
                    let v = (-(cj - j as i64).abs()) as f64 + sigma;
                    ay[j] = if v < 0.0 { 0.0 } else { v };
                }
                for i in 0..self.x {
                    for j in 0..self.y {
                        h[i * self.y + j] = ax[i] * ay[j];
                    }
                }
            }
            _ => {
                // gaussian, separable
                let d = 2.0 * sigma * sigma;
                let (xc, yc) = (self.xx[c], self.yy[c]);
                for m in 0..self.ux.len() {
                    ax[m] = scalar_exp(-square(self.ux[m] - xc) / d);
                }
                for m in 0..self.uy.len() {
                    ay[m] = scalar_exp(-square(self.uy[m] - yc) / d);
                }
                for k in 0..self.k {
                    h[k] = ax[self.ix[k]] * ay[self.iy[k]];
                }
            }
        }
    }

    fn scratch(&self) -> (Vec<f64>, Vec<f64>) {
        (vec![0.0; self.ux.len().max(self.x)], vec![0.0; self.uy.len().max(self.y)])
    }
}

/// The decays and neighborhood of a training. The learning rate decays by `lr_decay`: 0 asymptotic, 1 inverse to
/// zero, 2 linear to zero; the sigma by `sigma_decay`: 0 asymptotic, 1 inverse to one, 2 linear to one; the
/// neighborhood: 0 gaussian, 1 mexican hat, 2 bubble, 3 triangle.
#[derive(Clone, Copy, Debug)]
pub struct Schedule {
    pub learning_rate: f64,
    pub sigma: f64,
    pub lr_decay: i32,
    pub sigma_decay: i32,
    pub neighborhood: i32,
}

/// MiniSom.train(): `weights` (K x D) updated in place; `order` the sample of every step (or one epoch's order, with
/// `use_epochs`).
pub fn train_online(
    weights: &mut [f64],
    data: &[f64],
    d_n: usize,
    order: &[usize],
    num_iteration: usize,
    use_epochs: bool,
    s: &Schedule,
    grid: &Grid,
) {
    let k_n = grid.k;
    let epochs = if use_epochs { num_iteration } else { 1 };
    let max_iter = num_iteration as f64;
    let mut buf = vec![0.0; d_n];
    let mut h = vec![0.0; k_n];
    let (mut ax, mut ay) = grid.scratch();
    for e in 0..epochs {
        for (step, &sample) in order.iter().enumerate() {
            let xs = &data[sample * d_n..(sample + 1) * d_n];
            let bmu = find_bmu(xs, weights, k_n, &mut buf);
            // MiniSom's "t": the step, or the epoch with use_epochs
            let t = if use_epochs { e } else { step } as f64;
            let eta = decay_learning_rate(s.lr_decay, s.learning_rate, t, max_iter);
            let sig = decay_sigma(s.sigma_decay, s.sigma, t, max_iter);
            grid.neighborhood(s.neighborhood, bmu, sig, &mut h, &mut ax, &mut ay);
            for (wk, &hk) in weights.chunks_exact_mut(d_n).zip(&h[..k_n]) {
                let gk = hk * eta;
                if gk == 0.0 {
                    continue; // w += 0 * (x - w) leaves w unchanged
                }
                for (w, &x) in wk.iter_mut().zip(xs) {
                    *w += gk * (x - *w);
                }
            }
        }
    }
}

/// Neurons accumulated together by one task in the batch update.
const NEURON_BLOCK: usize = 8;

/// Samples whose winners one task looks for.
const WINNER_CHUNK: usize = 512;

/// MiniSom.train_batch_offline(): `weights` (K x D) updated in place. Each neuron's sums run in sample order, so the
/// result does not depend on the number of threads of the rayon pool it runs on.
pub fn train_batch(weights: &mut [f64], data: &[f64], d_n: usize, num_iteration: usize, s: &Schedule, grid: &Grid) {
    let k_n = grid.k;
    let n = data.len() / d_n;
    let mut bmu = vec![0usize; n];
    let mut gt = vec![0.0; k_n * k_n]; // gt[k * K + c]: the neighborhood at k of winner c
    let mut num = vec![0.0; k_n * d_n];
    let mut den = vec![0.0; k_n];
    let state = BatchState { data, d_n, k_n, n, num_iteration, s, grid };
    let (mut h, mut is_winner) = (vec![0.0; k_n], vec![false; k_n]);
    for it in 0..num_iteration {
        {
            // the winners, in chunks of samples
            let w: &[f64] = weights;
            bmu.par_chunks_mut(WINNER_CHUNK).enumerate().for_each_init(
                || vec![0.0; d_n],
                |buf, (c, out)| {
                    let start = c * WINNER_CHUNK;
                    for (i, b) in (start..).zip(out.iter_mut()) {
                        *b = find_bmu(&data[i * d_n..(i + 1) * d_n], w, k_n, buf);
                    }
                },
            );
        }
        state.neighborhoods(it, &bmu, &mut gt, &mut h, &mut is_winner);
        // the sums, by blocks of neurons
        num.par_chunks_mut(NEURON_BLOCK * d_n).zip(den.par_chunks_mut(NEURON_BLOCK)).enumerate().for_each(
            |(blk, (num_b, den_b))| {
                state.accumulate(blk, &bmu, &gt, num_b, den_b);
            },
        );
        state.update(it, weights, &num, &den);
    }
}

/// The fixed inputs of a batch training, and its phases.
struct BatchState<'a> {
    data: &'a [f64],
    d_n: usize,
    k_n: usize,
    n: usize,
    num_iteration: usize,
    s: &'a Schedule,
    grid: &'a Grid,
}

impl BatchState<'_> {
    fn decays(&self, it: usize) -> (f64, f64) {
        let (t, max_iter) = (it as f64, self.num_iteration as f64);
        (
            decay_learning_rate(self.s.lr_decay, self.s.learning_rate, t, max_iter),
            decay_sigma(self.s.sigma_decay, self.s.sigma, t, max_iter),
        )
    }

    /// The neighborhood of every winning neuron, into gt[k * K + c].
    fn neighborhoods(&self, it: usize, bmu: &[usize], gt: &mut [f64], h: &mut [f64], is_winner: &mut [bool]) {
        let (_, sig) = self.decays(it);
        let k_n = self.k_n;
        is_winner.fill(false);
        for &b in bmu {
            is_winner[b] = true;
        }
        let (mut ax, mut ay) = self.grid.scratch();
        for c in 0..k_n {
            if !is_winner[c] {
                continue;
            }
            self.grid.neighborhood(self.s.neighborhood, c, sig, h, &mut ax, &mut ay);
            for k in 0..k_n {
                gt[k * k_n + c] = h[k];
            }
        }
    }

    /// numerator += g * sample; denominator += g, in sample order, for the neurons of block `blk`, summed in buffers of
    /// its own then copied.
    fn accumulate(&self, blk: usize, bmu: &[usize], gt: &[f64], num_b: &mut [f64], den_b: &mut [f64]) {
        let (d_n, k_n) = (self.d_n, self.k_n);
        let k0 = blk * NEURON_BLOCK;
        let nk = den_b.len();
        let mut acc = vec![0.0; nk * d_n];
        let mut acc_den = [0.0; NEURON_BLOCK];
        // the block's rows of gt, so that a sample reads them at its winner
        let gt_b = &gt[k0 * k_n..(k0 + nk) * k_n];
        for (xs, &c) in self.data.chunks_exact(d_n).zip(&bmu[..self.n]) {
            for ((a, den), g_row) in acc.chunks_exact_mut(d_n).zip(&mut acc_den[..nk]).zip(gt_b.chunks_exact(k_n)) {
                let g = g_row[c];
                if g == 0.0 {
                    continue; // adds exact zeros
                }
                for (a, &x) in a.iter_mut().zip(xs) {
                    *a += g * x;
                }
                *den += g;
            }
        }
        num_b.copy_from_slice(&acc);
        den_b.copy_from_slice(&acc_den[..nk]);
    }

    /// The new weights: (1 - lr) w + lr num / den.
    fn update(&self, it: usize, weights: &mut [f64], num: &[f64], den: &[f64]) {
        let (lr, _) = self.decays(it);
        let d_n = self.d_n;
        for ((wk, nk), &dk) in weights.chunks_exact_mut(d_n).zip(num.chunks_exact(d_n)).zip(&den[..self.k_n]) {
            if dk.is_nan() || dk <= 0.0 {
                continue;
            }
            for (w, &nv) in wk.iter_mut().zip(nk) {
                let new_w = nv / dk;
                *w = (1.0 - lr) * *w + lr * new_w;
            }
        }
    }
}

/// The index (i * y + j) of the best matching unit of every sample, on the rayon pool of the caller.
pub fn predict_bmus(data: &[f64], d_n: usize, weights: &[f64], k_n: usize) -> Vec<usize> {
    let n = data.len().checked_div(d_n).unwrap_or(0);
    let mut out = vec![0usize; n];
    bmus_into(data, d_n, weights, k_n, &mut out);
    out
}

/// `predict_bmus` into `out` (one per sample).
pub fn bmus_into(data: &[f64], d_n: usize, weights: &[f64], k_n: usize, out: &mut [usize]) {
    out.par_chunks_mut(WINNER_CHUNK).enumerate().for_each_init(
        || vec![0.0; d_n],
        |buf, (c, o)| bmus_serial(&data[c * WINNER_CHUNK * d_n..], d_n, weights, k_n, o, buf),
    );
}

/// The best matching unit of the first `out.len()` samples of `data`, on this thread; `buf` (d_n values) is scratch.
pub fn bmus_serial(data: &[f64], d_n: usize, weights: &[f64], k_n: usize, out: &mut [usize], buf: &mut [f64]) {
    for (xs, o) in data.chunks_exact(d_n).zip(out.iter_mut()) {
        *o = find_bmu(xs, weights, k_n, buf);
    }
}

/// For each sample its best and second best matching unit (indices) and the distance to the first, on the rayon pool
/// of the caller.
pub fn predict_two(data: &[f64], d_n: usize, weights: &[f64], k_n: usize) -> Vec<(usize, usize, f64)> {
    let n = data.len().checked_div(d_n).unwrap_or(0);
    let mut out = vec![(0, 0, 0.0); n];
    out.par_chunks_mut(WINNER_CHUNK).enumerate().for_each_init(
        || vec![0.0; d_n],
        |buf, (c, o)| {
            for (i, oi) in (c * WINNER_CHUNK..).zip(o.iter_mut()) {
                *oi = find_two(&data[i * d_n..(i + 1) * d_n], weights, k_n, buf);
            }
        },
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rayon pool of `threads` threads.
    fn pool(threads: usize) -> rayon::ThreadPool {
        rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap()
    }

    #[test]
    fn pairwise_sum_matches_a_plain_sum_on_integers() {
        let a: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        assert_eq!(pairwise_sum(&a), 499500.0);
    }

    #[test]
    fn batch_training_separates_two_clusters() {
        let data: Vec<f64> = (0..200).flat_map(|i| if i % 2 == 0 { [0.0, 0.0] } else { [10.0, 10.0] }).collect();
        let grid = Grid::new(1, 2, &[0.0, 1.0], &[0.0, 0.0]).unwrap();
        let mut w = vec![1.0, 1.0, 9.0, 9.0];
        let s = Schedule { learning_rate: 0.5, sigma: 0.3, lr_decay: 0, sigma_decay: 0, neighborhood: 0 };
        for threads in [1, 3] {
            let mut ww = w.clone();
            pool(threads).install(|| train_batch(&mut ww, &data, 2, 20, &s, &grid));
            assert!(ww[0] < 1.0 && ww[2] > 9.0, "{ww:?}");
            w = ww;
        }
        assert_eq!(predict_bmus(&[0.5, 0.5, 9.5, 9.5], 2, &w, 2), vec![0, 1]);
    }

    /// MiniSom's coordinates of a hexagonal map of 3 by 4 (i along the row, 4 rows): the rows counted from the last,
    /// every other shifted, the rows (3 / 2) / sqrt(3) apart.
    #[test]
    fn the_hexagonal_coordinates_are_minisoms() {
        let g = Grid::minisom(3, 4, Topology::Hexagonal);
        let (xx, yy) = g.coordinates();
        let f = (3.0 / 2.0) / 3.0_f64.sqrt();
        for i in 0..3 {
            for j in 0..4 {
                let k = i * 4 + j;
                // 4 rows: rows 3 and 1 are shifted
                assert_eq!(xx[k], i as f64 - if j % 2 == 1 { 0.5 } else { 0.0 }, "x at {i} {j}");
                assert_eq!(yy[k], j as f64 * f, "y at {i} {j}");
            }
        }
        // 3 rows: rows 2 and 0
        let g = Grid::minisom(2, 3, Topology::Hexagonal);
        let (xx, _) = g.coordinates();
        assert_eq!(xx, &[-0.5, 0.0, -0.5, 0.5, 1.0, 0.5]);
        // a rectangular map is the plain indices
        let g = Grid::minisom(2, 3, Topology::Rectangular);
        let (xx, yy) = g.coordinates();
        assert_eq!(xx, &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        assert_eq!(yy, &[0.0, 1.0, 2.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn the_two_nearest_neurons_and_the_distance() {
        // neurons at 0, 10, 3 on a line
        let w = [0.0, 10.0, 3.0];
        let mut buf = [0.0];
        assert_eq!(find_two(&[2.0], &w, 3, &mut buf), (2, 0, 1.0));
        assert_eq!(find_two(&[9.0], &w, 3, &mut buf), (1, 2, 1.0));
        // a tie goes to the first index
        assert_eq!(find_two(&[5.0], &[0.0, 10.0], 2, &mut buf), (0, 1, 5.0));
        // one neuron: itself twice
        assert_eq!(find_two(&[5.0], &[0.0], 1, &mut buf), (0, 0, 5.0));
        let two = predict_two(&[2.0, 9.0], 1, &w, 3);
        assert_eq!(two, vec![(2, 0, 1.0), (1, 2, 1.0)]);
    }

    #[test]
    fn the_batch_is_the_same_on_any_number_of_threads() {
        // more samples than a chunk of winners, more neurons than a block
        let n = 1300;
        let data: Vec<f64> = (0..n * 3).map(|i| ((i * 7919) % 1013) as f64 / 101.3).collect();
        let grid = Grid::minisom(5, 4, Topology::Hexagonal);
        let s = Schedule { learning_rate: 0.7, sigma: 2.0, lr_decay: 2, sigma_decay: 2, neighborhood: 0 };
        let start: Vec<f64> = (0..20 * 3).map(|i| ((i * 37) % 11) as f64).collect();
        let train = |threads: usize| {
            let mut w = start.clone();
            pool(threads).install(|| train_batch(&mut w, &data, 3, 4, &s, &grid));
            w
        };
        let one = train(1);
        assert_ne!(one, start);
        for threads in [2, 8] {
            let w = train(threads);
            assert!(one.iter().zip(&w).all(|(a, b)| a.to_bits() == b.to_bits()), "{threads} threads");
        }
        let bmus = |threads: usize| pool(threads).install(|| predict_bmus(&data, 3, &one, 20));
        assert_eq!(bmus(1), bmus(8));
    }
}
