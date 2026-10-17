//! The analysis of spatial signatures (pylandstats' SpatialSignatureAnalysis in landschaft's lsm/analysis.py):
//! landscapes or zones described by their metrics (a table of metrics.signatures, or any table of metrics), their
//! principal components (`metrics.pca`), non-negative factors (`metrics.nmf`), clusters (`metrics.cluster`:
//! k-means with numpy's generator, so the same starts as landschaft; scipy's linkages and cuts) and clustergrams
//! (`metrics.clustergram`).
//!
//! Rows with NA values are left out and constant metrics dropped, as landschaft. The clusters are landschaft's (the
//! same labels, numbered 0.. in order of first appearance); the components, factors and means its numbers up to
//! the rounding of LAPACK's SVD and BLAS' products, which numpy uses and this module does not.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::change::numpy_sum;
use crate::sampling::KEYS;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Pca));
    r.add(Arc::new(Nmf));
    r.add(Arc::new(Cluster));
    r.add(Arc::new(Clustergram));
}

// -- numpy's generator --------------------------------------------------------------------------------------------

/// numpy's default generator (`np.random.default_rng(seed)`): PCG64 (XSL-RR 128/64) seeded by a SeedSequence, with
/// its draws of `integers`, `random` and `choice`.
pub struct NumpyRng {
    state: u128,
    inc: u128,
    /// The upper half of a 64-bit draw kept for the next 32-bit one.
    buffered: Option<u32>,
}

const PCG_MULT: u128 = (2549297995355413924u128 << 64) | 4865540595714422341u128;

/// numpy's SeedSequence(entropy).generate_state(4, uint64), as 8 words of 32 bits.
fn seed_sequence(entropy: u64) -> [u32; 8] {
    const INIT_A: u32 = 0x43b0d7e5;
    const MULT_A: u32 = 0x931e8875;
    const INIT_B: u32 = 0x8b51f9dd;
    const MULT_B: u32 = 0x58f38ded;
    const MIX_MULT_L: u32 = 0xca01f9dd;
    const MIX_MULT_R: u32 = 0x4973f715;
    let mut words = Vec::new();
    let mut e = entropy;
    while e > 0 {
        words.push(e as u32);
        e >>= 32;
    }
    if words.is_empty() {
        words.push(0);
    }
    let mut hash_const = INIT_A;
    let hashmix = |value: u32, hc: &mut u32| {
        let mut v = value ^ *hc;
        *hc = hc.wrapping_mul(MULT_A);
        v = v.wrapping_mul(*hc);
        v ^ (v >> 16)
    };
    let mix = |x: u32, y: u32| {
        let r = MIX_MULT_L.wrapping_mul(x).wrapping_sub(MIX_MULT_R.wrapping_mul(y));
        r ^ (r >> 16)
    };
    let mut pool = [0u32; 4];
    for (i, p) in pool.iter_mut().enumerate() {
        *p = hashmix(words.get(i).copied().unwrap_or(0), &mut hash_const);
    }
    for src in 0..4 {
        for dst in 0..4 {
            if src != dst {
                let h = hashmix(pool[src], &mut hash_const);
                pool[dst] = mix(pool[dst], h);
            }
        }
    }
    for &w in words.iter().skip(4) {
        for p in pool.iter_mut() {
            let h = hashmix(w, &mut hash_const);
            *p = mix(*p, h);
        }
    }
    let mut out = [0u32; 8];
    let mut hc = INIT_B;
    for (i, o) in out.iter_mut().enumerate() {
        let mut v = pool[i % 4] ^ hc;
        hc = hc.wrapping_mul(MULT_B);
        v = v.wrapping_mul(hc);
        *o = v ^ (v >> 16);
    }
    out
}

impl NumpyRng {
    pub fn new(seed: u64) -> Self {
        let w = seed_sequence(seed);
        let word = |i: usize| (u64::from(w[2 * i + 1]) << 32) | u64::from(w[2 * i]);
        let initstate = (u128::from(word(0)) << 64) | u128::from(word(1));
        let initseq = (u128::from(word(2)) << 64) | u128::from(word(3));
        let mut r = NumpyRng { state: 0, inc: (initseq << 1) | 1, buffered: None };
        r.step();
        r.state = r.state.wrapping_add(initstate);
        r.step();
        r
    }

    fn step(&mut self) {
        self.state = self.state.wrapping_mul(PCG_MULT).wrapping_add(self.inc);
    }

    /// A raw draw of 64 bits (`bit_generator.random_raw`).
    pub fn next_u64(&mut self) -> u64 {
        self.step();
        let (hi, lo) = ((self.state >> 64) as u64, self.state as u64);
        (hi ^ lo).rotate_right((hi >> 58) as u32)
    }

    fn next_u32(&mut self) -> u32 {
        if let Some(u) = self.buffered.take() {
            return u;
        }
        let next = self.next_u64();
        self.buffered = Some((next >> 32) as u32);
        next as u32
    }

    /// `integers(n)`: a whole number in [0, n), by Lemire's method as numpy.
    pub fn integers(&mut self, n: u64) -> u64 {
        assert!(n > 0);
        let rng = n - 1;
        if rng == 0 {
            0
        } else if rng <= 0xFFFF_FFFF {
            if rng == 0xFFFF_FFFF {
                return u64::from(self.next_u32());
            }
            let excl = rng as u32 + 1;
            let mut m = u64::from(self.next_u32()) * u64::from(excl);
            let mut leftover = m as u32;
            if leftover < excl {
                let threshold = (u32::MAX - rng as u32) % excl;
                while leftover < threshold {
                    m = u64::from(self.next_u32()) * u64::from(excl);
                    leftover = m as u32;
                }
            }
            m >> 32
        } else if rng == u64::MAX {
            self.next_u64()
        } else {
            let excl = rng + 1;
            let mut m = u128::from(self.next_u64()) * u128::from(excl);
            let mut leftover = m as u64;
            if leftover < excl {
                let threshold = (u64::MAX - rng) % excl;
                while leftover < threshold {
                    m = u128::from(self.next_u64()) * u128::from(excl);
                    leftover = m as u64;
                }
            }
            (m >> 64) as u64
        }
    }

    /// numpy's random_interval: a whole number in [0, max] by masked rejection.
    pub fn interval(&mut self, max: u64) -> u64 {
        if max == 0 {
            return 0;
        }
        let mut mask = max;
        for s in [1, 2, 4, 8, 16, 32] {
            mask |= mask >> s;
        }
        loop {
            let v = if max <= 0xFFFF_FFFF { u64::from(self.next_u32()) & mask } else { self.next_u64() & mask };
            if v <= max {
                return v;
            }
        }
    }

    /// `shuffle` of a 1-D array (and `permutation` of a copy): Fisher-Yates from the end.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.interval(i as u64) as usize;
            v.swap(i, j);
        }
    }

    /// `random()`: a double in [0, 1).
    pub fn random(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9007199254740992.0)
    }

    /// `choice(pop, size, replace=False)`: `size` distinct indices below `pop` in numpy's order: a tail shuffle of
    /// the whole range when the sample is a large share of a large population, else Floyd's algorithm over a hash
    /// set, then shuffled.
    pub fn choice_distinct(&mut self, pop: usize, size: usize) -> Vec<usize> {
        assert!(size <= pop, "choice: a sample larger than the population");
        let (pop, size) = (pop as u64, size as u64);
        let mut idx: Vec<u64>;
        if pop > 10_000 && size > pop / 50 {
            idx = (0..pop).collect();
            let first = (pop - size).max(1);
            for i in (first..pop).rev() {
                let j = self.integers(i + 1);
                idx.swap(i as usize, j as usize);
            }
            idx.drain(..(pop - size) as usize);
        } else {
            idx = vec![0; size as usize];
            let mut mask = (1.2 * size as f64) as u64;
            for s in [1, 2, 4, 8, 16, 32] {
                mask |= mask >> s;
            }
            let mut set = vec![u64::MAX; (mask + 1) as usize];
            for j in pop - size..pop {
                let val = self.integers(j + 1);
                let mut loc = (val & mask) as usize;
                while set[loc] != u64::MAX && set[loc] != val {
                    loc = (loc + 1) & mask as usize;
                }
                let k = (j + size - pop) as usize;
                if set[loc] == u64::MAX {
                    set[loc] = val;
                    idx[k] = val;
                } else {
                    loc = (j & mask) as usize;
                    while set[loc] != u64::MAX {
                        loc = (loc + 1) & mask as usize;
                    }
                    set[loc] = j;
                    idx[k] = j;
                }
            }
            for i in (1..size).rev() {
                let j = self.integers(i + 1);
                idx.swap(i as usize, j as usize);
            }
        }
        idx.into_iter().map(|v| v as usize).collect()
    }

    /// `choice(len(p), p=p)`: an index drawn by its probability (the cumulative sum, searched for a uniform).
    pub fn choice(&mut self, p: &[f64]) -> usize {
        let mut cdf = Vec::with_capacity(p.len());
        let mut s = 0.0;
        for &v in p {
            s += v;
            cdf.push(s);
        }
        let last = cdf[cdf.len() - 1];
        for c in &mut cdf {
            *c /= last;
        }
        let u = self.random();
        cdf.partition_point(|&c| c <= u)
    }
}

// -- linear algebra -----------------------------------------------------------------------------------------------

/// A matrix of rows.
type Matrix = Vec<Vec<f64>>;

/// The thin singular value decomposition of an n × m matrix by one-sided Jacobi rotations: U (n × k), the singular
/// values decreasing (k) and Vᵀ (k × m), k = min(n, m).
pub fn svd(a: &Matrix) -> (Matrix, Vec<f64>, Matrix) {
    let n = a.len();
    let m = a.first().map_or(0, Vec::len);
    if n < m {
        // A = (Aᵀ)ᵀ = (U' S V'ᵀ)ᵀ = V' S U'ᵀ
        let t: Matrix = (0..m).map(|j| (0..n).map(|i| a[i][j]).collect()).collect();
        let (u, s, vt) = svd(&t);
        let u2: Matrix = (0..n).map(|i| (0..s.len()).map(|c| vt[c][i]).collect()).collect();
        let vt2: Matrix = (0..s.len()).map(|c| (0..m).map(|j| u[j][c]).collect()).collect();
        return (u2, s, vt2);
    }
    // columns of A, rotated until orthogonal; V accumulates the rotations
    let mut cols: Matrix = (0..m).map(|j| (0..n).map(|i| a[i][j]).collect()).collect();
    let mut v: Matrix = (0..m).map(|j| (0..m).map(|i| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    let dot = |x: &[f64], y: &[f64]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f64>();
    for _sweep in 0..100 {
        let mut rotated = false;
        for p in 0..m {
            for q in p + 1..m {
                let alpha = dot(&cols[p], &cols[p]);
                let beta = dot(&cols[q], &cols[q]);
                let gamma = dot(&cols[p], &cols[q]);
                if gamma == 0.0 || gamma.abs() <= f64::EPSILON * (alpha * beta).sqrt() {
                    continue;
                }
                rotated = true;
                let zeta = (beta - alpha) / (2.0 * gamma);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for x in [&mut cols, &mut v] {
                    let (lo, hi) = x.split_at_mut(q);
                    for (a, b) in lo[p].iter_mut().zip(hi[0].iter_mut()) {
                        let (ap, bq) = (*a, *b);
                        *a = c * ap - s * bq;
                        *b = s * ap + c * bq;
                    }
                }
            }
        }
        if !rotated {
            break;
        }
    }
    let norms: Vec<f64> = cols.iter().map(|c| dot(c, c).sqrt()).collect();
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&i, &j| norms[j].total_cmp(&norms[i]));
    let s: Vec<f64> = order.iter().map(|&j| norms[j]).collect();
    let u: Matrix = (0..n)
        .map(|i| order.iter().map(|&j| if norms[j] > 0.0 { cols[j][i] / norms[j] } else { 0.0 }).collect())
        .collect();
    let vt: Matrix = order.iter().map(|&j| v[j].clone()).collect();
    (u, s, vt)
}

/// The sign that makes a vector's largest absolute value (the first of them) positive, as landschaft (and
/// scikit-learn) give components.
fn sign_of(v: &[f64]) -> f64 {
    let mut best = 0;
    for (i, x) in v.iter().enumerate() {
        if x.abs() > v[best].abs() {
            best = i;
        }
    }
    if v.get(best).copied().unwrap_or(0.0) < 0.0 { -1.0 } else { 1.0 }
}

// -- the signatures -----------------------------------------------------------------------------------------------

/// A table's metrics: the rows without NA (the others left out), their keys, the metrics' names and values.
struct Signatures {
    key_names: Vec<String>,
    keys: Matrix,
    names: Vec<String>,
    x: Matrix,
}

fn signatures_of(args: &Args) -> Result<Signatures> {
    let Some(Value::Table(t)) = args.get("table") else {
        return Err(Error::msg(format!("node {:?}: connect a table of metrics (metrics.signatures)", args.node)));
    };
    let metrics: Vec<usize> = match args.opt_param::<Vec<String>>("columns")? {
        Some(names) => names
            .iter()
            .map(|n| {
                t.columns
                    .iter()
                    .position(|c| c == n)
                    .ok_or_else(|| Error::msg(format!("node {:?}: columns: the table has no {n:?}", args.node)))
            })
            .collect::<Result<_>>()?,
        None => (0..t.columns.len()).filter(|&j| !KEYS.contains(&t.columns[j].as_str())).collect(),
    };
    if metrics.is_empty() {
        return Err(Error::msg(format!("node {:?}: the table has no metrics", args.node)));
    }
    let keys: Vec<usize> = (0..t.columns.len()).filter(|j| KEYS.contains(&t.columns[*j].as_str())).collect();
    let rows: Vec<&Vec<f64>> = t.rows.iter().filter(|r| metrics.iter().all(|&j| !r[j].is_nan())).collect();
    if rows.is_empty() {
        return Err(Error::msg(format!("node {:?}: no row without NA values", args.node)));
    }
    Ok(Signatures {
        key_names: keys.iter().map(|&j| t.columns[j].clone()).collect(),
        keys: rows.iter().map(|r| keys.iter().map(|&j| r[j]).collect()).collect(),
        names: metrics.iter().map(|&j| t.columns[j].clone()).collect(),
        x: rows.iter().map(|r| metrics.iter().map(|&j| r[j]).collect()).collect(),
    })
}

/// A column of a matrix.
fn column(x: &Matrix, j: usize) -> Vec<f64> {
    x.iter().map(|r| r[j]).collect()
}

impl Signatures {
    /// The metrics as z-scores (pandas' mean and population deviation: numpy's pairwise sums), constant ones left
    /// out: their names and values.
    fn standardised(&self) -> (Vec<String>, Matrix) {
        let n = self.x.len() as f64;
        let mut names = Vec::new();
        let mut stats = Vec::new();
        for (j, name) in self.names.iter().enumerate() {
            let c = column(&self.x, j);
            let mean = numpy_sum(&c) / n;
            let sq: Vec<f64> = c.iter().map(|v| (mean - v) * (mean - v)).collect();
            let sd = (numpy_sum(&sq) / n).sqrt();
            let scale = c.iter().fold(0.0f64, |a, v| a.max(v.abs()));
            if sd <= 1e-12 * scale || sd == 0.0 {
                continue;
            }
            names.push(name.clone());
            stats.push((j, mean, sd));
        }
        let z = self.x.iter().map(|r| stats.iter().map(|&(j, mean, sd)| (r[j] - mean) / sd).collect()).collect();
        (names, z)
    }

    /// The metrics scaled to [0, 1], constant ones left out.
    fn minmax(&self) -> (Vec<String>, Matrix) {
        let mut names = Vec::new();
        let mut stats = Vec::new();
        for (j, name) in self.names.iter().enumerate() {
            let c = column(&self.x, j);
            let lo = c.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = c.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let span = hi - lo;
            if span <= 1e-12 * hi.abs().max(lo.abs()) {
                continue;
            }
            names.push(name.clone());
            stats.push((j, lo, span));
        }
        let x = self.x.iter().map(|r| stats.iter().map(|&(j, lo, span)| (r[j] - lo) / span).collect()).collect();
        (names, x)
    }

    /// The principal components of the z-scores (all, or the first `k`): scores, loadings (a row per component)
    /// and the shares of the variance.
    fn pca(&self, k: Option<usize>) -> Result<(Vec<String>, Matrix, Matrix, Vec<f64>)> {
        let (names, z) = self.standardised();
        if names.is_empty() {
            return Err(Error::msg("every metric is constant"));
        }
        let (u, s, vt) = svd(&z);
        let r = s.len();
        let k = k.map_or(r, |k| k.min(r));
        let signs: Vec<f64> = vt.iter().map(|row| sign_of(row)).collect();
        let scores = u.iter().map(|row| (0..k).map(|c| row[c] * signs[c] * s[c]).collect()).collect();
        let loadings = (0..k).map(|c| vt[c].iter().map(|v| v * signs[c]).collect()).collect();
        let sq: Vec<f64> = s.iter().map(|v| v * v).collect();
        let total = numpy_sum(&sq);
        Ok((names, scores, loadings, sq[..k].iter().map(|v| v / total).collect()))
    }

    /// What the clusters are made of: the first `components` principal components' scores, or the z-scores; and
    /// whether numpy holds them by columns (pandas' z-scores) or by rows (the scores).
    fn cluster_data(&self, components: Option<usize>) -> Result<(Matrix, bool)> {
        match components {
            Some(k) => Ok((self.pca(Some(k))?.1, false)),
            None => {
                let (names, z) = self.standardised();
                if names.is_empty() {
                    return Err(Error::msg("every metric is constant"));
                }
                Ok((z, true))
            }
        }
    }
}

// -- non-negative factors -----------------------------------------------------------------------------------------

/// The Frobenius norm of x − W H.
fn residual(x: &Matrix, w: &Matrix, h: &Matrix) -> f64 {
    let mut s = 0.0;
    for (i, row) in x.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            let p: f64 = (0..h.len()).map(|c| w[i][c] * h[c][j]).sum();
            s += (v - p) * (v - p);
        }
    }
    s.sqrt()
}

/// W, H ≥ 0 minimising ‖x − W H‖ (landschaft's _nmf, scikit-learn's NMF(init="nndsvda", solver="cd")): the NNDSVDa
/// start, HALS sweeps until the loss improves by less than `tol` (relative). W, H, the error and the sweeps.
fn nmf(x: &Matrix, k: usize, max_iter: usize, tol: f64) -> (Matrix, Matrix, f64, usize) {
    let (n, m) = (x.len(), x[0].len());
    let (u, s, vt) = svd(x);
    let norm = |v: &[f64]| v.iter().map(|a| a * a).sum::<f64>().sqrt();
    let mut w = vec![vec![0.0; k]; n];
    let mut h = vec![vec![0.0; m]; k];
    for i in 0..n {
        w[i][0] = s[0].sqrt() * u[i][0].abs();
    }
    for j in 0..m {
        h[0][j] = s[0].sqrt() * vt[0][j].abs();
    }
    for c in 1..k {
        let a = column(&u, c);
        let b = &vt[c];
        let pos = |v: &[f64]| v.iter().map(|x| x.max(0.0)).collect::<Vec<_>>();
        let neg = |v: &[f64]| v.iter().map(|x| (-x).max(0.0)).collect::<Vec<_>>();
        let (ap, an, bp, bn) = (pos(&a), neg(&a), pos(b), neg(b));
        let p = norm(&ap) * norm(&bp);
        let q = norm(&an) * norm(&bn);
        let unit = |v: &[f64]| {
            let l = norm(v);
            let l = if l == 0.0 { 1.0 } else { l };
            v.iter().map(|x| x / l).collect::<Vec<_>>()
        };
        let (uu, vv, sigma) = if p >= q { (unit(&ap), unit(&bp), p) } else { (unit(&an), unit(&bn), q) };
        let f = (s[c] * sigma).sqrt();
        for i in 0..n {
            w[i][c] = f * uu[i];
        }
        for j in 0..m {
            h[c][j] = f * vv[j];
        }
    }
    // numpy's mean of the whole (column-major) array
    let flat: Vec<f64> = (0..m).flat_map(|j| x.iter().map(move |r| r[j])).collect();
    let mean = numpy_sum(&flat) / (n * m) as f64;
    for v in w.iter_mut().flatten().chain(h.iter_mut().flatten()) {
        if *v == 0.0 {
            *v = mean;
        }
    }
    const EPS: f64 = 1e-16;
    let mut previous = residual(x, &w, &h);
    let mut it = 0;
    for sweep in 1..=max_iter {
        it = sweep;
        // H's rows, then W's columns: each the exact minimiser with the others fixed
        let wtw: Matrix = (0..k).map(|a| (0..k).map(|b| (0..n).map(|i| w[i][a] * w[i][b]).sum()).collect()).collect();
        let wtx: Matrix = (0..k).map(|a| (0..m).map(|j| (0..n).map(|i| w[i][a] * x[i][j]).sum()).collect()).collect();
        for c in 0..k {
            let d = wtw[c][c].max(EPS);
            for j in 0..m {
                let wh: f64 = (0..k).map(|b| wtw[c][b] * h[b][j]).sum();
                h[c][j] = (h[c][j] + (wtx[c][j] - wh) / d).max(EPS);
            }
        }
        let hht: Matrix = (0..k).map(|a| (0..k).map(|b| (0..m).map(|j| h[a][j] * h[b][j]).sum()).collect()).collect();
        let xht: Matrix = (0..n).map(|i| (0..k).map(|b| (0..m).map(|j| x[i][j] * h[b][j]).sum()).collect()).collect();
        for c in 0..k {
            let d = hht[c][c].max(EPS);
            let whh: Vec<f64> = (0..n).map(|i| (0..k).map(|b| w[i][b] * hht[b][c]).sum()).collect();
            for i in 0..n {
                w[i][c] = (w[i][c] + (xht[i][c] - whh[i]) / d).max(EPS);
            }
        }
        let err = residual(x, &w, &h);
        if previous - err <= tol * previous.max(EPS) {
            break;
        }
        previous = err;
    }
    let err = residual(x, &w, &h);
    (w, h, err, it)
}

// -- clusters -----------------------------------------------------------------------------------------------------

/// Labels numbered 0.. in order of first appearance.
fn first_appearance(labels: &[usize]) -> Vec<usize> {
    let mut order: Vec<usize> = Vec::new();
    labels
        .iter()
        .map(|l| match order.iter().position(|o| o == l) {
            Some(i) => i,
            None => {
                order.push(*l);
                order.len() - 1
            }
        })
        .collect()
}

/// The squared distance between two rows, summed in order (numpy's sum along an outer axis).
fn dist2(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |s, (p, q)| s + (p - q) * (p - q))
}

/// The mean of rows (numpy's mean along the first axis: the rows added in order).
fn mean_of(rows: &[&Vec<f64>]) -> Vec<f64> {
    let mut s = rows[0].clone();
    for r in &rows[1..] {
        for (a, b) in s.iter_mut().zip(r.iter()) {
            *a += b;
        }
    }
    let n = rows.len() as f64;
    s.iter().map(|v| v / n).collect()
}

/// landschaft's k-means: k-means++ starts from numpy's generator, Lloyd's iterations (at most 300), the best of
/// `n_init` starts by inertia. `by_columns`: how numpy holds `x` (the order of the inertia's sum).
fn kmeans(x: &Matrix, k: usize, n_init: usize, seed: u64, by_columns: bool) -> Vec<usize> {
    let n = x.len();
    let mut rng = NumpyRng::new(seed);
    let mut best: Option<Vec<usize>> = None;
    let mut best_inertia = f64::INFINITY;
    for _ in 0..n_init.max(1) {
        let mut centres: Matrix = vec![x[rng.integers(n as u64) as usize].clone()];
        for _ in 1..k {
            let d2: Vec<f64> =
                x.iter().map(|r| centres.iter().map(|c| dist2(r, c)).fold(f64::INFINITY, f64::min)).collect();
            let total = numpy_sum(&d2);
            let i = if total > 0.0 {
                let p: Vec<f64> = d2.iter().map(|v| v / total).collect();
                rng.choice(&p)
            } else {
                rng.integers(n as u64) as usize
            };
            centres.push(x[i].clone());
        }
        let mut labels = vec![0usize; n];
        for _ in 0..300 {
            let new: Vec<usize> = x
                .iter()
                .map(|r| {
                    let mut best = 0;
                    let mut d = dist2(r, &centres[0]);
                    for (j, c) in centres.iter().enumerate().skip(1) {
                        let e = dist2(r, c);
                        if e < d {
                            (best, d) = (j, e);
                        }
                    }
                    best
                })
                .collect();
            let moved: Matrix = (0..k)
                .map(|j| {
                    let members: Vec<&Vec<f64>> =
                        x.iter().zip(&new).filter(|(_, l)| **l == j).map(|(r, _)| r).collect();
                    if members.is_empty() { centres[j].clone() } else { mean_of(&members) }
                })
                .collect();
            let close = moved
                .iter()
                .flatten()
                .zip(centres.iter().flatten())
                .all(|(a, b)| (a - b).abs() <= 1e-8 + 1e-5 * b.abs());
            if new == labels && close {
                break;
            }
            labels = new;
            centres = moved;
        }
        let sq: Vec<f64> = if by_columns {
            let d = x[0].len();
            (0..d)
                .flat_map(|j| (0..n).map(move |i| (j, i)))
                .map(|(j, i)| (x[i][j] - centres[labels[i]][j]).powi(2))
                .collect()
        } else {
            (0..n)
                .flat_map(|i| x[i].iter().zip(&centres[labels[i]]).map(|(a, b)| (a - b).powi(2)).collect::<Vec<_>>())
                .collect()
        };
        let inertia = numpy_sum(&sq);
        if inertia < best_inertia - 1e-12 {
            best = Some(labels);
            best_inertia = inertia;
        }
    }
    first_appearance(&best.unwrap_or_else(|| vec![0; n]))
}

/// scipy's linkages, in its order of methods.
const LINKAGES: [&str; 7] = ["single", "complete", "average", "centroid", "median", "ward", "weighted"];

/// scipy's distance from cluster i to the merge of x and y (_hierarchy_distance_update.pxi).
fn new_dist(method: usize, d_xi: f64, d_yi: f64, d_xy: f64, sx: i32, sy: i32, si: i32) -> f64 {
    let (fx, fy, fi) = (f64::from(sx), f64::from(sy), f64::from(si));
    match method {
        0 => {
            if d_yi < d_xi {
                d_yi
            } else {
                d_xi
            }
        }
        1 => {
            if d_yi > d_xi {
                d_yi
            } else {
                d_xi
            }
        }
        2 => (fx * d_xi + fy * d_yi) / f64::from(sx + sy),
        3 => ((((fx * d_xi * d_xi) + (fy * d_yi * d_yi)) - (f64::from(sx * sy) * d_xy * d_xy) / f64::from(sx + sy))
            / f64::from(sx + sy))
        .sqrt(),
        4 => (0.5 * (d_xi * d_xi + d_yi * d_yi) - 0.25 * d_xy * d_xy).sqrt(),
        5 => {
            let t = 1.0 / f64::from(sx + sy + si);
            (f64::from(si + sx) * t * d_xi * d_xi + f64::from(si + sy) * t * d_yi * d_yi - fi * t * d_xy * d_xy).sqrt()
        }
        _ => 0.5 * (d_xi + d_yi),
    }
}

/// The index of (i, j) in a condensed distance matrix of n points.
fn condensed(n: usize, i: usize, j: usize) -> usize {
    let (i, j) = if i < j { (i, j) } else { (j, i) };
    n * i - (i * (i + 1) / 2) + (j - i - 1)
}

/// scipy's binary heap of fast_linkage (keys 0..n by value).
struct Heap {
    index_by_key: Vec<usize>,
    key_by_index: Vec<usize>,
    values: Vec<f64>,
    size: usize,
}

impl Heap {
    fn new(values: &[f64]) -> Self {
        let size = values.len();
        let mut h = Heap {
            index_by_key: (0..size).collect(),
            key_by_index: (0..size).collect(),
            values: values.to_vec(),
            size,
        };
        for i in (0..size / 2).rev() {
            h.sift_down(i);
        }
        h
    }

    fn get_min(&self) -> (usize, f64) {
        (self.key_by_index[0], self.values[0])
    }

    fn remove_min(&mut self) {
        self.swap(0, self.size - 1);
        self.size -= 1;
        self.sift_down(0);
    }

    fn change_value(&mut self, key: usize, value: f64) {
        let index = self.index_by_key[key];
        let old = self.values[index];
        self.values[index] = value;
        if value < old {
            self.sift_up(index);
        } else {
            self.sift_down(index);
        }
    }

    fn sift_up(&mut self, mut index: usize) {
        while index > 0 {
            let parent = (index - 1) >> 1;
            if self.values[parent] > self.values[index] {
                self.swap(index, parent);
                index = parent;
            } else {
                break;
            }
        }
    }

    fn sift_down(&mut self, mut index: usize) {
        let mut child = (index << 1) + 1;
        while child < self.size {
            if child + 1 < self.size && self.values[child + 1] < self.values[child] {
                child += 1;
            }
            if self.values[index] > self.values[child] {
                self.swap(index, child);
                index = child;
                child = (index << 1) + 1;
            } else {
                break;
            }
        }
    }

    fn swap(&mut self, i: usize, j: usize) {
        self.values.swap(i, j);
        let (ki, kj) = (self.key_by_index[i], self.key_by_index[j]);
        self.key_by_index[i] = kj;
        self.key_by_index[j] = ki;
        self.index_by_key[ki] = j;
        self.index_by_key[kj] = i;
    }
}

/// A row of a linkage matrix: the clusters merged, their distance and the size of the merge.
type Link = [f64; 4];

/// The nearest cluster after x among those alive (fast_linkage's find_min_dist).
fn find_min_dist(n: usize, d: &[f64], size: &[i32], x: usize) -> (usize, f64) {
    let mut current = f64::INFINITY;
    let mut y = usize::MAX;
    for i in x + 1..n {
        if size[i] == 0 {
            continue;
        }
        let dist = d[condensed(n, x, i)];
        if dist < current {
            current = dist;
            y = i;
        }
    }
    (y, current)
}

/// scipy's generic clustering (fast_linkage: centroid and median).
fn fast_linkage(dists: &[f64], n: usize, method: usize) -> Vec<Link> {
    let mut z = vec![[0.0; 4]; n - 1];
    let mut d = dists.to_vec();
    let mut size = vec![1i32; n];
    let mut cluster_id: Vec<usize> = (0..n).collect();
    let mut neighbor = vec![0usize; n - 1];
    let mut min_dist = vec![0.0; n - 1];
    for x in 0..n - 1 {
        let (y, v) = find_min_dist(n, &d, &size, x);
        neighbor[x] = y;
        min_dist[x] = v;
    }
    let mut heap = Heap::new(&min_dist);
    for (k, link) in z.iter_mut().enumerate() {
        let (mut x, mut dist) = (0, 0.0);
        let mut y = 0;
        for _ in 0..n - k {
            (x, dist) = heap.get_min();
            y = neighbor[x];
            if dist == d[condensed(n, x, y)] {
                break;
            }
            (y, dist) = find_min_dist(n, &d, &size, x);
            neighbor[x] = y;
            min_dist[x] = dist;
            heap.change_value(x, dist);
        }
        heap.remove_min();
        let (mut id_x, mut id_y) = (cluster_id[x], cluster_id[y]);
        let (nx, ny) = (size[x], size[y]);
        if id_x > id_y {
            std::mem::swap(&mut id_x, &mut id_y);
        }
        *link = [id_x as f64, id_y as f64, dist, f64::from(nx + ny)];
        size[x] = 0;
        size[y] = nx + ny;
        cluster_id[y] = n + k;
        for i in 0..n {
            let ni = size[i];
            if ni == 0 || i == y {
                continue;
            }
            d[condensed(n, i, y)] = new_dist(method, d[condensed(n, i, x)], d[condensed(n, i, y)], dist, nx, ny, ni);
        }
        for i in 0..x {
            if size[i] > 0 && neighbor[i] == x {
                neighbor[i] = y;
            }
        }
        for i in 0..y {
            if size[i] == 0 {
                continue;
            }
            let v = d[condensed(n, i, y)];
            if v < min_dist[i] {
                neighbor[i] = y;
                min_dist[i] = v;
                heap.change_value(i, v);
            }
        }
        if y < n - 1 {
            let (i, v) = find_min_dist(n, &d, &size, y);
            if i != usize::MAX {
                neighbor[y] = i;
                min_dist[y] = v;
                heap.change_value(y, v);
            }
        }
    }
    z
}

/// scipy's nearest-neighbour chain (complete, average, weighted, ward), sorted and labelled.
fn nn_chain(dists: &[f64], n: usize, method: usize) -> Vec<Link> {
    let mut z = vec![[0.0; 4]; n - 1];
    let mut d = dists.to_vec();
    let mut size = vec![1i32; n];
    let mut chain: Vec<usize> = Vec::with_capacity(n);
    for link in z.iter_mut() {
        if chain.is_empty() {
            chain.push((0..n).find(|&i| size[i] > 0).unwrap());
        }
        let (mut x, mut y, mut current);
        loop {
            x = chain[chain.len() - 1];
            if chain.len() > 1 {
                y = chain[chain.len() - 2];
                current = d[condensed(n, x, y)];
            } else {
                y = 0;
                current = f64::INFINITY;
            }
            for i in 0..n {
                if size[i] == 0 || x == i {
                    continue;
                }
                let dist = d[condensed(n, x, i)];
                if dist < current {
                    current = dist;
                    y = i;
                }
            }
            if chain.len() > 1 && y == chain[chain.len() - 2] {
                break;
            }
            chain.push(y);
        }
        chain.truncate(chain.len() - 2);
        if x > y {
            std::mem::swap(&mut x, &mut y);
        }
        let (nx, ny) = (size[x], size[y]);
        *link = [x as f64, y as f64, current, f64::from(nx + ny)];
        size[x] = 0;
        size[y] = nx + ny;
        for i in 0..n {
            let ni = size[i];
            if ni == 0 || i == y {
                continue;
            }
            d[condensed(n, i, y)] = new_dist(method, d[condensed(n, i, x)], d[condensed(n, i, y)], current, nx, ny, ni);
        }
    }
    sort_and_label(z, n)
}

/// scipy's minimum spanning tree (single linkage), sorted and labelled.
fn mst_single_linkage(dists: &[f64], n: usize) -> Vec<Link> {
    let mut z = vec![[0.0; 4]; n - 1];
    let mut merged = vec![false; n];
    let mut d = vec![f64::INFINITY; n];
    let (mut x, mut y) = (0, 0);
    for row in z.iter_mut() {
        let mut current = f64::INFINITY;
        merged[x] = true;
        for i in 0..n {
            if merged[i] {
                continue;
            }
            let dist = dists[condensed(n, x, i)];
            if d[i] > dist {
                d[i] = dist;
            }
            if d[i] < current {
                y = i;
                current = d[i];
            }
        }
        *row = [x as f64, y as f64, current, 0.0];
        x = y;
    }
    sort_and_label(z, n)
}

/// The rows sorted by distance (stable) and the clusters named as merged (scipy's label).
fn sort_and_label(z: Vec<Link>, n: usize) -> Vec<Link> {
    let mut order: Vec<usize> = (0..z.len()).collect();
    order.sort_by(|&a, &b| z[a][2].total_cmp(&z[b][2]));
    let mut z: Vec<Link> = order.iter().map(|&i| z[i]).collect();
    let mut parent: Vec<usize> = (0..2 * n - 1).collect();
    let mut sizes = vec![1usize; 2 * n - 1];
    let mut next = n;
    let find = |parent: &mut Vec<usize>, mut x: usize| {
        let mut p = x;
        while parent[x] != x {
            x = parent[x];
        }
        while parent[p] != x {
            let up = parent[p];
            parent[p] = x;
            p = up;
        }
        x
    };
    for row in z.iter_mut() {
        let (x, y) = (row[0] as usize, row[1] as usize);
        let (xr, yr) = (find(&mut parent, x), find(&mut parent, y));
        (row[0], row[1]) = if xr < yr { (xr as f64, yr as f64) } else { (yr as f64, xr as f64) };
        parent[xr] = next;
        parent[yr] = next;
        sizes[next] = sizes[xr] + sizes[yr];
        row[3] = sizes[next] as f64;
        next += 1;
    }
    z
}

/// The greatest distance within every cluster of a tree (scipy's get_max_dist_for_each_cluster).
fn max_dists(z: &[Link], n: usize) -> Vec<f64> {
    let mut md = vec![0.0; n - 1];
    let mut visited = vec![false; 2 * n - 1];
    let mut stack = vec![2 * n - 2];
    while let Some(&node) = stack.last() {
        let root = node - n;
        let (lc, rc) = (z[root][0] as usize, z[root][1] as usize);
        if lc >= n && !visited[lc] {
            visited[lc] = true;
            stack.push(lc);
            continue;
        }
        if rc >= n && !visited[rc] {
            visited[rc] = true;
            stack.push(rc);
            continue;
        }
        let mut m = z[root][2];
        if lc >= n && md[lc - n] > m {
            m = md[lc - n];
        }
        if rc >= n && md[rc - n] > m {
            m = md[rc - n];
        }
        md[root] = m;
        stack.pop();
    }
    md
}

/// The clusters of a tree cut at a criterion (scipy's cluster_monocrit): each point's cluster, 1...
fn cluster_monocrit(z: &[Link], mc: &[f64], cutoff: f64, n: usize) -> Vec<usize> {
    let mut t = vec![0usize; n];
    let mut visited = vec![false; 2 * n - 1];
    let mut stack = vec![2 * n - 2];
    let (mut n_cluster, mut leader) = (0usize, usize::MAX);
    while let Some(&node) = stack.last() {
        let root = node - n;
        let (lc, rc) = (z[root][0] as usize, z[root][1] as usize);
        if leader == usize::MAX && mc[root] <= cutoff {
            leader = root;
            n_cluster += 1;
        }
        if lc >= n && !visited[lc] {
            visited[lc] = true;
            stack.push(lc);
            continue;
        }
        if rc >= n && !visited[rc] {
            visited[rc] = true;
            stack.push(rc);
            continue;
        }
        for c in [lc, rc] {
            if c < n {
                if leader == usize::MAX {
                    n_cluster += 1;
                }
                t[c] = n_cluster;
            }
        }
        if leader == root {
            leader = usize::MAX;
        }
        stack.pop();
    }
    t
}

/// At most `max_nc` clusters of a tree (scipy's fcluster(criterion="maxclust")): the lowest cut of the greatest
/// distances within clusters that gives no more.
fn maxclust(z: &[Link], n: usize, max_nc: usize) -> Vec<usize> {
    if max_nc >= n {
        return (1..=n).collect();
    }
    let mc = max_dists(z, n);
    let (mut lower, mut upper) = (-1i64, n as i64 - 1);
    while upper - lower > 1 {
        let i = ((lower + upper) >> 1) as usize;
        let thresh = mc[i];
        let mut visited = vec![false; 2 * n - 1];
        let mut nc = 0;
        let mut stack = vec![2 * n - 2];
        'walk: while let Some(&node) = stack.last() {
            let root = node - n;
            let (lc, rc) = (z[root][0] as usize, z[root][1] as usize);
            if mc[root] <= thresh {
                nc += 1;
                if nc > max_nc {
                    break;
                }
                stack.pop();
                visited[lc] = true;
                visited[rc] = true;
                continue;
            }
            for c in [lc, rc] {
                if !visited[c] {
                    visited[c] = true;
                    if c >= n {
                        stack.push(c);
                        continue 'walk;
                    }
                    nc += 1;
                    if nc > max_nc {
                        break 'walk;
                    }
                }
            }
            stack.pop();
        }
        if nc > max_nc {
            lower = i as i64;
        } else {
            upper = i as i64;
        }
    }
    cluster_monocrit(z, &mc, mc[upper as usize], n)
}

/// scipy's linkage tree of the rows (Euclidean distances), cut into k clusters for each k.
fn hierarchical(x: &Matrix, ks: &[usize], method: &str) -> Result<Vec<Vec<usize>>> {
    let code = LINKAGES
        .iter()
        .position(|m| *m == method)
        .ok_or_else(|| Error::msg(format!("method is kmeans or one of {}, not {method:?}", LINKAGES.join(", "))))?;
    let n = x.len();
    if n == 1 {
        return Ok(ks.iter().map(|_| vec![0]).collect());
    }
    let mut dists = Vec::with_capacity(n * (n - 1) / 2);
    for i in 0..n {
        for j in i + 1..n {
            dists.push(dist2(&x[i], &x[j]).sqrt());
        }
    }
    let z = match method {
        "single" => mst_single_linkage(&dists, n),
        "complete" | "average" | "weighted" | "ward" => nn_chain(&dists, n, code),
        _ => fast_linkage(&dists, n, code),
    };
    Ok(ks.iter().map(|&k| first_appearance(&maxclust(&z, n, k))).collect())
}

/// The clusters of the rows for each k, by k-means or a linkage.
fn clusters(
    x: &Matrix,
    ks: &[usize],
    method: &str,
    n_init: usize,
    seed: u64,
    by_columns: bool,
) -> Result<Vec<Vec<usize>>> {
    if method == "kmeans" {
        Ok(ks.iter().map(|&k| kmeans(x, k, n_init, seed, by_columns)).collect())
    } else {
        hierarchical(x, ks, method)
    }
}

// -- operators ----------------------------------------------------------------------------------------------------

/// The settings that say which columns are metrics.
fn columns_param(sig: OpSignature) -> OpSignature {
    sig.param(PortSpec::optional(
        "columns",
        Type::Any,
        "the metrics (default every column but the keys: zone, landscape, class, date...)",
    ))
}

/// The keys of the rows kept, and more columns.
fn keyed(s: &Signatures, names: impl IntoIterator<Item = String>, values: &Matrix) -> Table {
    let mut columns = s.key_names.clone();
    columns.extend(names);
    let rows = s.keys.iter().zip(values).map(|(k, v)| k.iter().chain(v).copied().collect()).collect();
    Table { columns, rows }
}

fn positive(args: &Args, name: &str, default: Option<usize>) -> Result<Option<usize>> {
    match args.opt_param::<f64>(name)? {
        None => Ok(default),
        Some(v) if v >= 1.0 && v.fract() == 0.0 => Ok(Some(v as usize)),
        Some(v) => Err(Error::msg(format!("node {:?}: {name}: a whole number ≥ 1, not {v}", args.node))),
    }
}

fn method_param(sig: OpSignature) -> OpSignature {
    sig.param(
        PortSpec::optional(
            "method",
            Type::Text,
            "kmeans (default; k-means++ starts, the best of n_init) or a linkage: ward, average, complete, single, \
             centroid, median, weighted",
        )
        .choices(["kmeans", "ward", "average", "complete", "single", "centroid", "median", "weighted"]),
    )
    .param(PortSpec::optional("components", Type::Integer, "cluster the first principal components, not the metrics"))
    .param(PortSpec::optional("n_init", Type::Integer, "k-means: the starts (default 10)"))
    .param(PortSpec::optional("seed", Type::Integer, "k-means: numpy's seed (default 0)"))
}

struct Pca;

impl Operator for Pca {
    fn signature(&self) -> OpSignature {
        columns_param(
            OpSignature::new(
                "metrics.pca",
                "The principal components of spatial signatures (landschaft's SpatialSignatureAnalysis.pca): the \
                 metrics as z-scores (constant ones left out, rows with NA too), their singular value \
                 decomposition, each component's largest loading positive.",
                Access::None,
            )
            .input(PortSpec::new("table", Type::Table, "the signatures (metrics.signatures) or any metrics"))
            .output(PortSpec::new("scores", Type::Table, "per row: its keys and its scores (PC1, PC2...)"))
            .output(PortSpec::new(
                "loadings",
                Type::Table,
                "per component: its share of the variance (explained) and the loadings of the metrics",
            )),
        )
        .param(PortSpec::optional("components", Type::Integer, "the components kept (default all)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = signatures_of(args)?;
        let k = positive(args, "components", None)?;
        let (names, scores, loadings, explained) =
            s.pca(k).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let pcs: Vec<String> = (1..=explained.len()).map(|i| format!("PC{i}")).collect();
        let mut columns = vec!["component".to_string(), "explained".to_string()];
        columns.extend(names);
        let rows = loadings
            .iter()
            .enumerate()
            .map(|(c, l)| [(c + 1) as f64, explained[c]].into_iter().chain(l.iter().copied()).collect())
            .collect();
        Ok(outputs([
            ("scores", Value::Table(keyed(&s, pcs, &scores))),
            ("loadings", Value::Table(Table { columns, rows })),
        ]))
    }
}

struct Nmf;

impl Operator for Nmf {
    fn signature(&self) -> OpSignature {
        columns_param(
            OpSignature::new(
                "metrics.nmf",
                "Non-negative factors of spatial signatures (landschaft's SpatialSignatureAnalysis.nmf): each metric \
                 scaled to [0, 1] (constant ones left out), approximated by scores times components, both \
                 non-negative (NNDSVDa start, HALS sweeps; scikit-learn's NMF(init=\"nndsvda\", solver=\"cd\")).",
                Access::None,
            )
            .input(PortSpec::new("table", Type::Table, "the signatures (metrics.signatures) or any metrics"))
            .output(PortSpec::new("scores", Type::Table, "per row: its keys and its factors (NMF1, NMF2...)"))
            .output(PortSpec::new("components", Type::Table, "per factor: the weights of the metrics"))
            .output(PortSpec::new("error", Type::Number, "the Frobenius norm of the residual"))
            .output(PortSpec::new("iterations", Type::Integer, "the sweeps made")),
        )
        .param(PortSpec::new("factors", Type::Integer, "the number of factors"))
        .param(PortSpec::optional("max_iter", Type::Integer, "the most sweeps (default 1000)"))
        .param(PortSpec::optional("tol", Type::Number, "stop when the loss improves by less (relative; 1e-6)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = signatures_of(args)?;
        let (names, x) = s.minmax();
        let most = x.len().min(names.len());
        let k = positive(args, "factors", None)?
            .ok_or_else(|| Error::msg(format!("node {:?}: factors: how many", args.node)))?;
        if names.is_empty() || k > most {
            return Err(Error::msg(format!("node {:?}: factors between 1 and {most}", args.node)));
        }
        let max_iter = positive(args, "max_iter", Some(1000))?.unwrap();
        let tol: f64 = args.param("tol", 1e-6)?;
        let (w, h, err, it) = nmf(&x, k, max_iter, tol);
        let factors: Vec<String> = (1..=k).map(|i| format!("NMF{i}")).collect();
        let mut columns = vec!["factor".to_string()];
        columns.extend(names);
        let rows = h
            .iter()
            .enumerate()
            .map(|(c, r)| std::iter::once((c + 1) as f64).chain(r.iter().copied()).collect())
            .collect();
        Ok(outputs([
            ("scores", Value::Table(keyed(&s, factors, &w))),
            ("components", Value::Table(Table { columns, rows })),
            ("error", Value::Number(err)),
            ("iterations", Value::Integer(it as i64)),
        ]))
    }
}

/// The method, components, starts and seed of a clustering node.
fn cluster_settings(args: &Args) -> Result<(String, Option<usize>, usize, u64)> {
    let method: String = args.param("method", "kmeans".to_string())?;
    if method != "kmeans" && !LINKAGES.contains(&method.as_str()) {
        return Err(Error::msg(format!(
            "node {:?}: method is kmeans or one of {}, not {method:?}",
            args.node,
            LINKAGES.join(", ")
        )));
    }
    let components = positive(args, "components", None)?;
    let n_init = positive(args, "n_init", Some(10))?.unwrap();
    let seed: f64 = args.param("seed", 0.0)?;
    if seed < 0.0 || seed.fract() != 0.0 {
        return Err(Error::msg(format!("node {:?}: seed: a whole number ≥ 0", args.node)));
    }
    Ok((method, components, n_init, seed as u64))
}

struct Cluster;

impl Operator for Cluster {
    fn signature(&self) -> OpSignature {
        method_param(columns_param(
            OpSignature::new(
                "metrics.cluster",
                "Groups of landscapes or zones with similar metrics (landschaft's SpatialSignatureAnalysis.cluster): \
                 k-means of the z-scores (or of the first principal components) from numpy's generator, so the \
                 same groups as landschaft, or scipy's hierarchical linkages cut into k clusters. The groups are \
                 numbered 0.. in order of first appearance.",
                Access::None,
            )
            .input(PortSpec::new("table", Type::Table, "the signatures (metrics.signatures) or any metrics"))
            .output(PortSpec::new("table", Type::Table, "the rows kept (without NA) and their cluster")),
        ))
        .param(PortSpec::new("k", Type::Integer, "the number of clusters"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = signatures_of(args)?;
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let (method, components, n_init, seed) = cluster_settings(args)?;
        let (x, by_columns) = s.cluster_data(components).map_err(fail)?;
        let k = positive(args, "k", None)?.ok_or_else(|| Error::msg(format!("node {:?}: k: how many", args.node)))?;
        if k > x.len() {
            return Err(Error::msg(format!("node {:?}: k between 1 and the rows ({})", args.node, x.len())));
        }
        let labels = clusters(&x, &[k], &method, n_init, seed, by_columns).map_err(fail)?.remove(0);
        let values: Matrix =
            s.x.iter().zip(&labels).map(|(r, &l)| r.iter().copied().chain([l as f64]).collect()).collect();
        let names = s.names.iter().cloned().chain(["cluster".to_string()]);
        Ok(outputs([("table", Value::Table(keyed(&s, names, &values)))]))
    }
}

struct Clustergram;

impl Operator for Clustergram {
    fn signature(&self) -> OpSignature {
        method_param(columns_param(
            OpSignature::new(
                "metrics.clustergram",
                "Clusters for a range of k (Schonlau's clustergram, landschaft's \
                 SpatialSignatureAnalysis.clustergram): for every k each row's cluster and the mean of its cluster, \
                 the cluster's centre projected on the first principal axis (pca_weighted) or averaged over the \
                 metrics.",
                Access::None,
            )
            .input(PortSpec::new("table", Type::Table, "the signatures (metrics.signatures) or any metrics"))
            .output(PortSpec::new("labels", Type::Table, "per row: its keys and its cluster at each k (k1, k2...)"))
            .output(PortSpec::new(
                "means",
                Type::Table,
                "per row: its keys and its cluster's mean at each k",
            )),
        ))
        .param(PortSpec::optional("k_min", Type::Integer, "the fewest clusters (default 1)"))
        .param(PortSpec::optional("k_max", Type::Integer, "the most clusters (default 8)"))
        .param(PortSpec::optional(
            "pca_weighted",
            Type::Bool,
            "the means on the first principal axis (default true)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = signatures_of(args)?;
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let (method, components, n_init, seed) = cluster_settings(args)?;
        let (x, by_columns) = s.cluster_data(components).map_err(fail)?;
        let (lo, hi) = (positive(args, "k_min", Some(1))?.unwrap(), positive(args, "k_max", Some(8))?.unwrap());
        if lo > hi || hi > x.len() {
            return Err(Error::msg(format!("node {:?}: k_min ≤ k_max ≤ the rows ({})", args.node, x.len())));
        }
        let ks: Vec<usize> = (lo..=hi).collect();
        let all = clusters(&x, &ks, &method, n_init, seed, by_columns).map_err(fail)?;
        let weighted: bool = args.param("pca_weighted", true)?;
        let n = x.len();
        let d = x[0].len();
        let axis: Vec<f64> = if weighted {
            let means: Vec<f64> = (0..d).map(|j| numpy_sum(&column(&x, j)) / n as f64).collect();
            let centred: Matrix = x.iter().map(|r| r.iter().zip(&means).map(|(v, m)| v - m).collect()).collect();
            let (_, _, vt) = svd(&centred);
            let sign = sign_of(&vt[0]);
            vt[0].iter().map(|v| v * sign).collect()
        } else {
            Vec::new()
        };
        let mut labels = vec![Vec::with_capacity(ks.len()); n];
        let mut means = vec![Vec::with_capacity(ks.len()); n];
        for lab in &all {
            let k = lab.iter().max().map_or(0, |m| m + 1);
            let value: Vec<f64> = (0..k)
                .map(|j| {
                    let members: Vec<&Vec<f64>> = x.iter().zip(lab).filter(|(_, l)| **l == j).map(|(r, _)| r).collect();
                    let centre = mean_of(&members);
                    if weighted {
                        centre.iter().zip(&axis).map(|(a, b)| a * b).sum()
                    } else {
                        numpy_sum(&centre) / d as f64
                    }
                })
                .collect();
            for (i, &l) in lab.iter().enumerate() {
                labels[i].push(l as f64);
                means[i].push(value[l]);
            }
        }
        let names = || ks.iter().map(|k| format!("k{k}"));
        Ok(outputs([
            ("labels", Value::Table(keyed(&s, names(), &labels))),
            ("means", Value::Table(keyed(&s, names(), &means))),
        ]))
    }
}
