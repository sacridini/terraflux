//! `segment.spectral_difference`: eCognition's spectral difference segmentation. Neighbouring objects whose mean layer
//! values are close are merged, again and again with the means of what was merged, until no neighbours are closer than
//! the limit.
//!
//! The difference of two objects is the weighted mean, over the bands, of the absolute difference of the bands' means
//! (bands where either object has no value are left out). The pair of the least difference merges first, the lower id
//! on a tie, so the result does not depend on how the work is shared: a queue (a heap whose keys change in place) with
//! the best pair of each object. A merge costs O(neighbours of the merged object) key changes plus a new best for each
//! neighbour that had one of the two; the sums per object, the first bests, the table and the new labels run in
//! parallel.

use std::sync::Arc;

use rayon::prelude::*;
use smallvec::SmallVec;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::{Objects, find};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(SpectralDifference));
}

/// The neighbours of an object, by number (most have few).
type Near = SmallVec<[u32; 8]>;

/// No neighbour.
const NO: u32 = u32::MAX;

/// The objects as the merging sees them: the mean of each band, the neighbours and what is left.
struct State<'a> {
    nb: usize,
    /// Per band: its weight (0 or more).
    weights: &'a [f64],
    /// Per object, per band: the count and the sum of the values less the band's centre (the rounding error stays small).
    count: Vec<f64>,
    sum: Vec<f64>,
    centre: Vec<f64>,
    /// Per object, per band: the mean (NaN for none).
    mean: Vec<f64>,
    near: Vec<Near>,
    /// Per object: the neighbour it would merge with (the one of the least difference under the limit, the lowest
    /// number on a tie, `NO` for none) and that difference.
    best: Vec<(f64, u32)>,
    alive: Vec<bool>,
}

/// An object's key in the queue: the difference's bits (never negative, so they sort as the values), then the two objects
/// of its pair (the lower number first).
type Key = (u64, u32, u32);

/// A 4-ary min-heap of the objects with a pair to merge, by their keys, each object at most once: a key is changed or
/// taken out in place, so the heap never holds a stale pair.
struct Queue {
    heap: Vec<u32>,
    /// Per object: its place in the heap (`NO` when it is not in it).
    at: Vec<u32>,
    key: Vec<Key>,
}

impl Queue {
    fn new(n: usize) -> Queue {
        Queue { heap: Vec::new(), at: vec![NO; n], key: vec![(0, 0, 0); n] }
    }

    fn place(&mut self, i: usize, r: u32) {
        self.heap[i] = r;
        self.at[r as usize] = i as u32;
    }

    fn up(&mut self, mut i: usize) {
        let r = self.heap[i];
        let k = self.key[r as usize];
        while i > 0 {
            let p = (i - 1) / 4;
            let q = self.heap[p];
            if self.key[q as usize] <= k {
                break;
            }
            self.place(i, q);
            i = p;
        }
        self.place(i, r);
    }

    fn down(&mut self, mut i: usize) {
        let n = self.heap.len();
        let r = self.heap[i];
        let k = self.key[r as usize];
        loop {
            let first = 4 * i + 1;
            if first >= n {
                break;
            }
            let (mut at, mut least) = (first, self.key[self.heap[first] as usize]);
            for c in first + 1..(first + 4).min(n) {
                let kc = self.key[self.heap[c] as usize];
                if kc < least {
                    (at, least) = (c, kc);
                }
            }
            if least >= k {
                break;
            }
            let q = self.heap[at];
            self.place(i, q);
            i = at;
        }
        self.place(i, r);
    }

    /// The objects in the heap, set up in one pass (their keys are in `key`).
    fn build(&mut self, objects: Vec<u32>) {
        self.heap = objects;
        for (i, &r) in self.heap.iter().enumerate() {
            self.at[r as usize] = i as u32;
        }
        for i in (0..self.heap.len() / 4 + 1).rev() {
            if i < self.heap.len() {
                self.down(i);
            }
        }
    }

    /// Object `r` has the key `k`, or no pair (`None`).
    fn set(&mut self, r: usize, k: Option<Key>) {
        let at = self.at[r];
        match (k, at) {
            (Some(k), NO) => {
                self.key[r] = k;
                self.heap.push(r as u32);
                let i = self.heap.len() - 1;
                self.at[r] = i as u32;
                self.up(i);
            }
            (Some(k), i) => {
                let less = k < self.key[r];
                self.key[r] = k;
                if less { self.up(i as usize) } else { self.down(i as usize) }
            }
            (None, NO) => {}
            (None, i) => {
                let i = i as usize;
                self.at[r] = NO;
                let last = self.heap.pop().expect("a heap with the object");
                if i < self.heap.len() {
                    self.place(i, last);
                    self.up(i);
                    let j = self.at[last as usize] as usize;
                    self.down(j);
                }
            }
        }
    }

    /// The object of the least key, taken out.
    fn pop(&mut self) -> Option<u32> {
        let top = *self.heap.first()?;
        self.set(top as usize, None);
        Some(top)
    }
}

impl State<'_> {
    /// The difference of objects `a` and `b`: infinity when they share no band with a value.
    #[inline]
    fn difference(&self, a: usize, b: usize) -> f64 {
        let nb = self.nb;
        let (x, y) = (&self.mean[a * nb..(a + 1) * nb], &self.mean[b * nb..(b + 1) * nb]);
        let (mut s, mut w) = (0.0, 0.0);
        for j in 0..nb {
            let d = (x[j] - y[j]).abs();
            if !d.is_nan() {
                s += self.weights[j] * d;
                w += self.weights[j];
            }
        }
        if w > 0.0 { s / w } else { f64::INFINITY }
    }

    /// The neighbour of `r` that is the least different, if under `limit` (the lowest number on a tie).
    fn best_of(&self, r: usize, limit: f64) -> (f64, u32) {
        let mut best = (f64::INFINITY, NO);
        for &c in &self.near[r] {
            let d = self.difference(r, c as usize);
            if d < limit && d < best.0 {
                best = (d, c);
            }
        }
        best
    }

    /// Object `b` merged into `a` (the lower number): the sums, the means and the neighbours.
    fn merge(&mut self, a: usize, b: usize) {
        let nb = self.nb;
        for j in 0..nb {
            self.count[a * nb + j] += self.count[b * nb + j];
            self.sum[a * nb + j] += self.sum[b * nb + j];
            let c = self.count[a * nb + j];
            self.mean[a * nb + j] = if c > 0.0 { self.centre[j] + self.sum[a * nb + j] / c } else { f64::NAN };
        }
        let (a32, b32) = (a as u32, b as u32);
        let (la, lb) = (std::mem::take(&mut self.near[a]), std::mem::take(&mut self.near[b]));
        // the two lists (sorted) joined, without a and b
        let mut joined = Near::with_capacity(la.len() + lb.len());
        let (mut i, mut j) = (0, 0);
        while i < la.len() || j < lb.len() {
            let x = if j == lb.len() || (i < la.len() && la[i] <= lb[j]) {
                let v = la[i];
                if j < lb.len() && lb[j] == v {
                    j += 1;
                }
                i += 1;
                v
            } else {
                j += 1;
                lb[j - 1]
            };
            if x != a32 && x != b32 {
                joined.push(x);
            }
        }
        // every neighbour of the two now has a, and no b
        for &n in &joined {
            let l = &mut self.near[n as usize];
            if let Ok(i) = l.binary_search(&b32) {
                l.remove(i);
            }
            if let Err(i) = l.binary_search(&a32) {
                l.insert(i, a32);
            }
        }
        self.near[a] = joined;
        self.alive[b] = false;
    }
}

/// The key of object `r` with its best.
fn key_of(r: usize, best: (f64, u32)) -> Option<Key> {
    (best.1 != NO).then(|| (best.0.to_bits(), (r as u32).min(best.1), (r as u32).max(best.1)))
}

/// The objects merged: each object's root, the object it merged into (the lowest number of the group). Each object
/// is in the queue with its one best pair, so a merge changes what it must (the merged object's best, and that of each
/// neighbour whose best was one of the two, or is now the merged object), never a pair per neighbour.
fn merge_all(s: &mut State, limit: f64) -> Vec<u32> {
    let n = s.near.len();
    let sr = &*s;
    s.best = (0..n).into_par_iter().map(|r| sr.best_of(r, limit)).collect();
    let mut q = Queue::new(n);
    for (r, &b) in s.best.iter().enumerate() {
        if let Some(k) = key_of(r, b) {
            q.key[r] = k;
        }
    }
    q.build((0..n as u32).filter(|&r| s.best[r as usize].1 != NO).collect());
    let mut parent: Vec<u32> = (0..n as u32).collect();
    while let Some(r) = q.pop() {
        let (_, a, b) = q.key[r as usize];
        let (a, b) = (a as usize, b as usize);
        s.merge(a, b);
        parent[b] = a as u32;
        // the other of the pair is in the queue with a pair that is gone
        q.set(a + b - r as usize, None);
        s.best[b] = (f64::INFINITY, NO);
        s.best[a] = s.best_of(a, limit);
        q.set(a, key_of(a, s.best[a]));
        for i in 0..s.near[a].len() {
            let c = s.near[a][i] as usize;
            let (cd, co) = s.best[c];
            if co == a as u32 || co == b as u32 {
                s.best[c] = s.best_of(c, limit);
                q.set(c, key_of(c, s.best[c]));
            } else {
                let d = s.difference(a, c);
                if d < limit && (d < cd || (d == cd && (a as u32) < co)) {
                    s.best[c] = (d, a as u32);
                    q.set(c, key_of(c, s.best[c]));
                }
            }
        }
    }
    (0..n as u32).map(|r| find(&mut parent, r)).collect()
}

/// `segment.spectral_difference`.
struct SpectralDifference;

impl Operator for SpectralDifference {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.spectral_difference",
            "Objects merged by spectral difference (eCognition's spectral difference segmentation): neighbouring \
             objects whose mean values in the image differ by less than `max_difference` become one, and the means \
             of the new object count for the next merges, until no two neighbours are that close. The difference of \
             two objects is the weighted mean, over the bands, of the absolute difference of the bands' means (the \
             bands weighted by `weights`; a band where either has no value is left out), so with one band it is the \
             plain difference of the means, in the image's units. The pair of the least difference merges first (the \
             lower ids on a tie), so the result is always the same, on any number of threads; eCognition visits \
             the objects in an order of its own. Neighbours are objects sharing a cell edge; the objects are numbered \
             again in the order of their first cell. A cell without a value in a band counts in no mean of that \
             band. Use it to merge the over-segmentation of a chessboard, a \
             superpixel or a multiresolution level into areas of one tone.",
            Access::Global,
        )
        .input(PortSpec::new(
            "objects",
            Type::Objects,
            "the objects to merge (a segmenter's, on the grid of the image)",
        ))
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands, whose values the means are of"))
        .output(PortSpec::new("objects", Type::Objects, "the merged objects"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per object: id, cells, area (map units squared), and mean_<band> and sd_<band> for each band",
        ))
        .param(PortSpec::new(
            "max_difference",
            Type::Number,
            "above 0: neighbours merge while their difference is below this (in the image's units)",
        ))
        .param(PortSpec::optional("weights", Type::Any, "a list, the weight of each band (default 1 for all)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let limit = args
            .opt_param::<f64>("max_difference")?
            .ok_or_else(|| bad("the setting max_difference is needed".into()))?;
        if !limit.is_finite() || limit <= 0.0 {
            return Err(bad(format!("max_difference {limit}: above 0")));
        }
        let o = super::objects_in(args, "objects")?.clone();
        let l = crate::raster_stats::layers(args, "image")?;
        super::warnings::image(ctx, node, &l.grid, &l.names, &l.data, false);
        let (w, h) = (l.grid.width, l.grid.height);
        if (o.grid.width, o.grid.height) != (w, h) {
            return Err(bad("objects: objects of another grid than the image".into()));
        }
        let nb = l.data.len();
        let weights: Vec<f64> = match args.opt_param::<Vec<f64>>("weights")? {
            None => vec![1.0; nb],
            Some(v) => v,
        };
        if weights.len() != nb
            || weights.iter().any(|x| !x.is_finite() || *x < 0.0)
            || weights.iter().sum::<f64>() <= 0.0
        {
            return Err(bad(format!("weights: {nb} numbers, 0 or more, not all 0 (one for each band)")));
        }
        let count = o.count;
        // per band, in parallel (each band's sums stay in the order of the cells): the centre, then the count and the
        // sum of the values less it, of each object
        let per_band: Vec<(f64, Vec<f64>, Vec<f64>)> = l
            .data
            .par_iter()
            .map(|b| {
                let (mut t, mut c) = (0.0, 0usize);
                for &v in b.iter().filter(|v| !v.is_nan()) {
                    t += v;
                    c += 1;
                }
                let centre = if c > 0 { t / c as f64 } else { 0.0 };
                let (mut cnt, mut sum) = (vec![0.0f64; count], vec![0.0f64; count]);
                for (&id, &v) in o.labels.iter().zip(b.iter()) {
                    if id != 0 && !v.is_nan() {
                        let k = id as usize - 1;
                        cnt[k] += 1.0;
                        sum[k] += v - centre;
                    }
                }
                (centre, cnt, sum)
            })
            .collect();
        let mut state = State {
            nb,
            weights: &weights,
            count: vec![0.0; count * nb],
            sum: vec![0.0; count * nb],
            centre: per_band.iter().map(|b| b.0).collect(),
            mean: vec![f64::NAN; count * nb],
            near: o.adjacency.par_iter().map(|a| a.iter().map(|&(n, _)| n - 1).collect()).collect(),
            best: Vec::new(),
            alive: vec![true; count],
        };
        {
            let State { count: cn, sum, mean, centre, .. } = &mut state;
            cn.par_chunks_mut(nb.max(1))
                .zip(sum.par_chunks_mut(nb.max(1)))
                .zip(mean.par_chunks_mut(nb.max(1)))
                .enumerate()
                .for_each(|(k, ((c, s), m))| {
                    for j in 0..nb {
                        c[j] = per_band[j].1[k];
                        s[j] = per_band[j].2[k];
                        m[j] = if c[j] > 0.0 { centre[j] + s[j] / c[j] } else { f64::NAN };
                    }
                });
        }
        drop(per_band);
        let root = merge_all(&mut state, limit);
        // the new ids, in the order of the first cell of each group (a group is its lowest id's, but the ids of `objects`
        // need not follow the cells)
        let mut to_new = vec![0u32; count + 1];
        let (mut next, mut prev) = (0u32, 0u32);
        for &id in o.labels.iter() {
            if id == prev || id == 0 {
                continue;
            }
            prev = id;
            let r = root[id as usize - 1] as usize + 1;
            if to_new[r] == 0 {
                next += 1;
                to_new[r] = next;
            }
        }
        for id in 1..=count {
            to_new[id] = to_new[root[id - 1] as usize + 1];
        }
        let mut labels = vec![0u32; w * h];
        labels.par_chunks_mut(1 << 16).zip(o.labels[..].par_chunks(1 << 16)).for_each(|(out, src)| {
            for (x, &id) in out.iter_mut().zip(src) {
                *x = to_new[id as usize];
            }
        });
        let merged = Objects::from_labels(l.grid.clone(), labels, next as usize)?;
        let t = super::stats_table(&merged, &l.names, &l.data);
        let merged = merged.with_table(t.clone())?;
        Ok(outputs([("objects", Value::Objects(Arc::new(merged))), ("table", Value::Table(t))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The queue against a plain list under random sets, removals and pops.
    #[test]
    fn the_queue_keeps_the_least_key_first() {
        let n = 200;
        let mut q = Queue::new(n);
        let mut want: Vec<Option<Key>> = vec![None; n];
        let mut s = 99u64;
        let mut next = move || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 33) as usize
        };
        for step in 0..5000 {
            let r = next() % n;
            match next() % 4 {
                0 => {
                    q.set(r, None);
                    want[r] = None;
                }
                1 if step % 7 == 0 => {
                    let least = want.iter().flatten().min().copied();
                    let got = q.pop().map(|r| q.key[r as usize]);
                    assert_eq!(got, least);
                    if let Some(k) = least {
                        let at = want.iter().position(|w| *w == Some(k)).unwrap();
                        want[at] = None;
                    }
                }
                _ => {
                    // (unique keys: the pair holds the object)
                    let k = (next() as u64 % 50, r as u32, (next() % 1000) as u32);
                    q.set(r, Some(k));
                    want[r] = Some(k);
                }
            }
        }
        let mut rest: Vec<Key> = want.iter().flatten().copied().collect();
        rest.sort();
        let got: Vec<Key> = std::iter::from_fn(|| q.pop().map(|r| q.key[r as usize])).collect();
        assert_eq!(got, rest);
    }
}
