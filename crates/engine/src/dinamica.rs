//! Dinamica EGO's Expander and Patcher on tiles: the same maps whatever the tiling.
//!
//! A step changes, for each transition `i -> j` with a rate, `n = round(rate x cells of i)` cells, a share by
//! the Expander (patches grown next to existing patches of `j`) and the rest by the Patcher (new patches).
//! Where the sequential allocation draws seeds one by one from a ranking of the whole map, here:
//!
//! 1. **Weights.** Each cell of `i` not changed yet in the step weighs its probability `P` (the Expander: `P`
//!    where at least half of its 3 x 3 window holds `j`, else `P n_j / half`, as Dinamica EGO 8).
//! 2. **Pruning.** Only the cells of the largest weights count (`prune_factor x n` of them), the threshold
//!    found from histograms of the weights summed over the tiles.
//! 3. **Keys.** Each candidate draws a key `u^(1/w)` (Efraimidis and Spirakis' weighted sampling: the cells
//!    of the largest keys are a sample drawn in proportion to the weights); the Expander keeps a candidate with
//!    probability `w`, as Dinamica's Expander tries each seed once. `u` comes from a counter: (seed, step,
//!    phase, transition, cell). The largest keys are collected once per phase.
//! 4. **Patches.** The seeds are the candidates of the largest keys; each grows a patch of lognormal size,
//!    cell by cell into the neighbouring cells of `i` in proportion to `P x isometry^k` (`k` its cells around
//!    the candidate), within `radius` cells of its seed. A tile with a halo of twice the radius grows every
//!    patch that may reach it as any other tile would.
//! 5. **Conflicts.** A cell claimed by several patches (of any transitions) goes to the one of largest key.
//! 6. **Demand.** The number of seeds starts at `n / mean size` and is adjusted in rounds (a pass over the
//!    tiles each) until the cells changed reach `n`; the changes past it go (the cells grown last in the
//!    patches of the smallest keys). The Patcher also makes what the Expander could not.
//!
//! Every global quantity is a sum over tiles (histograms, counts, sets of keys), every draw a function of the
//! cell: the result is the same with any tile size, number of threads or machines.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use terraflux_ops::{Map, PatchParams, Stack};

use crate::pass::{Part, Task};
use crate::rng::{Stream, hash, unit};
use crate::tiles::{Layer, Source, Tile, Tiling};
use crate::{Error, Result};

/// How the tiled allocation runs.
#[derive(Debug, Clone)]
pub struct Settings {
    /// The largest distance (cells, Chebyshev) of a patch's cell from its seed.
    pub radius: usize,
    /// Cells ranked per change for the seeds (Dinamica's prune factor).
    pub prune: f64,
    /// Rounds of the seeds' thresholds at most, per phase.
    pub rounds: usize,
    /// The share of the demand a phase may pass by before its changes are cut.
    pub tolerance: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { radius: 32, prune: 10.0, rounds: 6, tolerance: 0.01 }
    }
}

/// A transition of a step: what it must change and how.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub from: f64,
    pub to: f64,
    pub rate: f64,
    pub expansion: f64,
    pub expander: [f64; 3],
    pub patcher: [f64; 3],
}

/// What a step did for a transition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Done {
    pub requested: i64,
    pub expanded: i64,
    pub patched: i64,
    pub expander_patches: i64,
    pub patcher_patches: i64,
    pub expander_rounds: usize,
    pub patcher_rounds: usize,
}

const BINS: usize = 1 << 16;

/// Counts of values in [lo, hi) by `BINS` bins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hist {
    lo: f64,
    hi: f64,
    bins: Vec<u64>,
}

impl Hist {
    fn new(lo: f64, hi: f64) -> Self {
        // a range asked for nothing holds no bins
        let n = if hi > lo { BINS } else { 0 };
        Hist { lo, hi, bins: vec![0; n] }
    }

    fn add(&mut self, x: f64) {
        if x < self.lo || x >= self.hi {
            return;
        }
        let i = (((x - self.lo) / (self.hi - self.lo)) * BINS as f64) as usize;
        self.bins[i.min(BINS - 1)] += 1;
    }

    pub(crate) fn merge(mut self, o: Hist) -> Hist {
        for (a, b) in self.bins.iter_mut().zip(o.bins) {
            *a += b;
        }
        self
    }

    fn edge(&self, i: usize) -> f64 {
        self.lo + (self.hi - self.lo) * i as f64 / BINS as f64
    }
}

/// Histograms of the values in a range per transition, summed over the tiles.
type Count<'a> = &'a dyn Fn(&[(f64, f64)]) -> Result<Vec<Hist>>;

/// For each `m[k]`, the value `x` such that at least `m[k]` of the values of `k` are >= `x` and as few more
/// as the histograms resolve (`None`: -inf, nothing wanted; `Some(0)`: +inf; 0 when there are fewer than
/// `m[k]` values): up to three levels of histograms, every `k` in the same passes `count(ranges)` over the
/// tiles. The values are in [0, 1].
fn top_thresholds(m: &[Option<u64>], count: Count) -> Result<Vec<f64>> {
    let n = m.len();
    let mut out: Vec<f64> = m
        .iter()
        .map(|x| match x {
            None => f64::NEG_INFINITY,
            Some(0) => f64::INFINITY,
            Some(_) => 0.0,
        })
        .collect();
    let mut open: Vec<bool> = m.iter().map(|x| x.is_some_and(|v| v > 0)).collect();
    let mut ranges = vec![(0.0f64, 1.0f64 + 1e-12); n];
    let mut above = vec![0u64; n];
    for _ in 0..3 {
        if !open.iter().any(|&o| o) {
            break;
        }
        // the closed ones count nothing
        let asked: Vec<(f64, f64)> = (0..n).map(|k| if open[k] { ranges[k] } else { (0.0, 0.0) }).collect();
        let hists = count(&asked)?;
        for k in 0..n {
            if !open[k] {
                continue;
            }
            let want = m[k].unwrap();
            let h = &hists[k];
            let mut acc = above[k];
            let mut found = None;
            for i in (0..BINS).rev() {
                acc += h.bins[i];
                if acc >= want {
                    found = Some((i, acc - h.bins[i]));
                    break;
                }
            }
            match found {
                None => {
                    out[k] = ranges[k].0;
                    open[k] = false;
                }
                Some((i, before)) => {
                    out[k] = h.edge(i);
                    // the bin holds about as many as wanted: take it whole
                    if h.bins[i] <= 1 || acc_ok(before + h.bins[i], want) {
                        open[k] = false;
                    } else {
                        ranges[k] = (h.edge(i), h.edge(i + 1));
                        above[k] = before;
                    }
                }
            }
        }
    }
    Ok(out)
}

fn acc_ok(total: u64, m: u64) -> bool {
    total as f64 <= m as f64 * 1.001 + 1.0
}

/// The cells of a tile's window: the land now, at the start of the step, and each transition's probability.
struct View {
    win_x0: i64,
    win_y0: i64,
    w: usize,
    h: usize,
    land: Vec<f64>,
    start: Vec<f64>,
    prob: Vec<Vec<f64>>,
}

impl View {
    fn read(tile: &Tile, halo: usize, land: &dyn Source, start: &dyn Source, prob: &[&dyn Source]) -> Result<View> {
        let win = tile.window(halo);
        Ok(View {
            win_x0: win.x0,
            win_y0: win.y0,
            w: win.width,
            h: win.height,
            land: land.read(win)?,
            start: start.read(win)?,
            prob: prob.iter().map(|p| p.read(win)).collect::<Result<_>>()?,
        })
    }

    /// The global index of a local cell.
    fn global(&self, i: usize, width: usize) -> u64 {
        let (x, y) = ((i % self.w) as i64 + self.win_x0, (i / self.w) as i64 + self.win_y0);
        (y as u64) * width as u64 + x as u64
    }

    fn free(&self, i: usize, from: f64) -> bool {
        self.land[i] == from && self.start[i] == from
    }

    /// Cells of `to` in the 3 x 3 window around a local cell (the centre left out).
    fn around(&self, i: usize, to: f64) -> i32 {
        let (x, y) = ((i % self.w) as i64, (i / self.w) as i64);
        let mut n = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (xx, yy) = (x + dx, y + dy);
                if xx >= 0
                    && yy >= 0
                    && (xx as usize) < self.w
                    && (yy as usize) < self.h
                    && self.land[yy as usize * self.w + xx as usize] == to
                {
                    n += 1;
                }
            }
        }
        n
    }
}

/// The half of the 3 x 3 window, its centre included: from it the Expander keeps the full probability.
const HALF: i32 = 5;

/// The weight of a free cell for a phase (0: not a candidate).
fn weight(v: &View, i: usize, t: &Transition, k: usize, expander: bool) -> f64 {
    let p = v.prob[k][i];
    if !(p > 0.0) || !v.free(i, t.from) {
        return 0.0;
    }
    if expander {
        let nj = v.around(i, t.to);
        if nj >= HALF { p } else { p * nj as f64 / HALF as f64 }
    } else {
        p
    }
}

fn phase_name(expander: bool) -> &'static str {
    if expander { "expander" } else { "patcher" }
}

fn phase_tag(expander: bool) -> u64 {
    if expander { 1 } else { 2 }
}

/// A candidate's key (u^(1/w)) and whether it is kept (the Expander tries a seed with probability w).
fn key(seed: u64, step: u64, expander: bool, k: usize, cell: u64, w: f64) -> Option<f64> {
    let tag = phase_tag(expander);
    if expander && unit(hash(&[seed, step, tag, k as u64, cell, 1])) >= w {
        return None;
    }
    let u = unit(hash(&[seed, step, tag, k as u64, cell, 2]));
    Some(u.powf(1.0 / w))
}

/// The lognormal patch size of a seed (cells; Dinamica's mean and variance).
fn patch_size(s: &mut Stream, mean: f64, var: f64) -> usize {
    if !(mean > 0.0) {
        return 1;
    }
    if !(var > 0.0) {
        return mean.round().max(1.0) as usize;
    }
    let s2 = (1.0 + var / (mean * mean)).ln();
    let mu = mean.ln() - s2 / 2.0;
    (mu + s2.sqrt() * s.normal()).exp().round().max(1.0) as usize
}

/// The global facts of one phase of a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase {
    expander: bool,
    transitions: Vec<Transition>,
    /// cells each transition must change in the phase
    demand: Vec<i64>,
    /// pruning threshold of the weights, per transition (infinite: inactive)
    prune: Vec<f64>,
    /// seeds' threshold of the keys, per transition
    kappa: Vec<f64>,
    seed: u64,
    step: u64,
    radius: usize,
    width: usize,
}

impl Phase {
    /// The transitions from each class: (class, the transitions' indices) for those `wanted`.
    fn by_class(&self, wanted: impl Fn(usize) -> bool) -> Vec<(f64, Vec<usize>)> {
        let mut out: Vec<(f64, Vec<usize>)> = Vec::new();
        for (k, t) in self.transitions.iter().enumerate() {
            if !wanted(k) {
                continue;
            }
            match out.iter_mut().find(|(c, _)| *c == t.from) {
                Some((_, ks)) => ks.push(k),
                None => out.push((t.from, vec![k])),
            }
        }
        out
    }

    /// A candidate's key for transition `k` (None: not a candidate).
    fn key_of(&self, v: &View, i: usize, k: usize) -> Option<f64> {
        let w = weight(v, i, &self.transitions[k], k, self.expander);
        if w <= 0.0 || w < self.prune[k] {
            return None;
        }
        key(self.seed, self.step, self.expander, k, v.global(i, self.width), w)
    }
}

/// The transitions that may apply to a cell of class `c` (none when it is of no transition's class).
fn of_class(classes: &[(f64, Vec<usize>)], c: f64) -> &[usize] {
    classes.iter().find(|(x, _)| *x == c).map(|(_, ks)| ks.as_slice()).unwrap_or(&[])
}

/// Grow the patches that may reach a tile and settle its cells' claims: the cells changed in the tile
/// (local interior index, transition, score), and the seeds in the tile per transition.
fn grow_tile(tile: &Tile, v: &View, ph: &Phase) -> (Vec<(usize, usize, f64)>, Vec<i64>) {
    let r = ph.radius as i64;
    let halo = (v.w - tile.width) / 2;
    let classes = ph.by_class(|k| ph.prune[k].is_finite());
    // the seeds: in the tile and up to `radius` around it
    let (sx0, sy0) = (halo as i64 - r, halo as i64 - r);
    let (sx1, sy1) = (halo as i64 + tile.width as i64 + r, halo as i64 + tile.height as i64 + r);
    let mut seeds: Vec<(f64, u64, usize, usize)> = Vec::new(); // key, global cell, local index, transition
    for y in sy0.max(0)..sy1.min(v.h as i64) {
        for x in sx0.max(0)..sx1.min(v.w as i64) {
            let i = y as usize * v.w + x as usize;
            for &k in of_class(&classes, v.land[i]) {
                if let Some(key) = ph.key_of(v, i, k)
                    && key >= ph.kappa[k]
                {
                    seeds.push((key, v.global(i, ph.width), i, k));
                }
            }
        }
    }
    let inside = |i: usize| {
        let (x, y) = (i % v.w, i / v.w);
        x >= halo && x < halo + tile.width && y >= halo && y < halo + tile.height
    };
    let mut seeded = vec![0i64; ph.transitions.len()];
    // claims of the tile's cells: (key, seed cell, transition, score)
    let mut claims: HashMap<usize, (f64, u64, usize, f64)> = HashMap::new();
    for &(key, cell, i0, k) in &seeds {
        if inside(i0) {
            seeded[k] += 1;
        }
        let t = &ph.transitions[k];
        let [mean, var, iso] = if ph.expander { t.expander } else { t.patcher };
        let mut s = Stream::new(&[ph.seed, ph.step, phase_tag(ph.expander), k as u64, cell, 3]);
        // as the sequential allocation, no patch passes what its transition must change
        let size = patch_size(&mut s, mean, var).min(ph.demand[k].max(1) as usize);
        let (cx, cy) = ((i0 % v.w) as i64, (i0 / v.w) as i64);
        let near = |i: usize| ((i % v.w) as i64 - cx).abs() <= r && ((i / v.w) as i64 - cy).abs() <= r;
        let eligible = |i: usize| v.free(i, t.from) && v.prob[k][i] > 0.0;
        let mut patch = vec![i0];
        let mut taken: std::collections::HashSet<usize> = [i0].into();
        let mut front: Vec<usize> = Vec::new();
        let mut count: HashMap<usize, i32> = HashMap::new();
        let touch = |c: usize,
                     front: &mut Vec<usize>,
                     count: &mut HashMap<usize, i32>,
                     taken: &std::collections::HashSet<usize>| {
            let (x, y) = ((c % v.w) as i64, (c / v.w) as i64);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let (xx, yy) = (x + dx, y + dy);
                    if xx < 0 || yy < 0 || xx as usize >= v.w || yy as usize >= v.h {
                        continue;
                    }
                    let nb = yy as usize * v.w + xx as usize;
                    if taken.contains(&nb) || !near(nb) || !eligible(nb) {
                        continue;
                    }
                    let e = count.entry(nb).or_insert(0);
                    if *e == 0 {
                        front.push(nb);
                    }
                    *e += 1;
                }
            }
        };
        touch(i0, &mut front, &mut count, &taken);
        while patch.len() < size && !front.is_empty() {
            let wts: Vec<f64> = front.iter().map(|c| v.prob[k][*c] * iso.powi(count[c])).collect();
            let total: f64 = wts.iter().sum();
            if !(total > 0.0) {
                break;
            }
            let mut u = s.uniform() * total;
            let mut pick = front.len() - 1;
            for (j, w) in wts.iter().enumerate() {
                if u < *w {
                    pick = j;
                    break;
                }
                u -= w;
            }
            let c = front.remove(pick);
            count.remove(&c);
            taken.insert(c);
            patch.push(c);
            touch(c, &mut front, &mut count, &taken);
        }
        for (g, &c) in patch.iter().enumerate() {
            if !inside(c) {
                continue;
            }
            // the cells grown last in the patches of the smallest keys go first when a phase passes its demand
            let claim = (key, cell, k, key.powi(1 + g as i32));
            match claims.get(&c) {
                Some(&(kk, cc, _, _)) if kk > key || (kk == key && cc < cell) => {}
                _ => {
                    claims.insert(c, claim);
                }
            }
        }
    }
    let mut changes: Vec<(usize, usize, f64)> = claims
        .into_iter()
        .map(|(i, (_, _, k, score))| {
            let (x, y) = (i % v.w - halo, i / v.w - halo);
            (y * tile.width + x, k, score)
        })
        .collect();
    changes.sort_unstable_by_key(|c| c.0);
    (changes, seeded)
}

/// What a pass over the tiles made: the cells changed and the seeds per transition, and (when asked) the
/// scores of the changes per transition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Made {
    made: Vec<i64>,
    seeded: Vec<i64>,
    scores: Vec<Vec<f64>>,
}

impl Made {
    fn empty(nt: usize) -> Made {
        Made { made: vec![0; nt], seeded: vec![0; nt], scores: vec![Vec::new(); nt] }
    }

    pub(crate) fn merge(mut self, o: Made) -> Made {
        for (a, b) in self.made.iter_mut().zip(&o.made) {
            *a += b;
        }
        for (a, b) in self.seeded.iter_mut().zip(&o.seeded) {
            *a += b;
        }
        for (a, b) in self.scores.iter_mut().zip(o.scores) {
            a.extend(b);
        }
        self
    }
}

/// The layer a pass writes.
fn layer<'a>(src: &[&'a dyn Source], i: usize) -> Result<&'a Layer> {
    src.get(i).and_then(|s| s.as_layer()).ok_or_else(|| Error::Msg("a pass writes only layers".into()))
}

/// The phases' tasks on one tile. Sources: the land, the land at the start of the step, a probability per
/// transition (then, for `Grow` with `write`, the layer written); for `Probabilities`, the land, the drivers and
/// the layers written.
pub(crate) fn on_tile(task: &Task, tile: &Tile, src: &[&dyn Source]) -> Result<Part> {
    match task {
        Task::Probabilities { model, names } => {
            let t0 = std::time::Instant::now();
            let land = src[0];
            let mut g = terraflux_raster::Grid::new(tile.width, tile.height);
            g.transform = land.grid().transform;
            let g = std::sync::Arc::new(g);
            let map = Map::new(g.clone(), land.read(tile.window(0))?);
            let mut keys = Vec::new();
            let mut data = Vec::new();
            for (k, name) in names.iter().enumerate() {
                keys.push(terraflux_ops::Key::Name(name.clone()));
                data.push(std::sync::Arc::new(src[1 + k].read(tile.window(0))?));
            }
            let stack = Stack { grid: g, keys, layers: data };
            let t1 = std::time::Instant::now();
            let p = terraflux_ops::woe::probability(model, &map, &stack)?;
            let t2 = std::time::Instant::now();
            for (k, l) in p.layers.iter().enumerate() {
                let out = layer(src, 1 + names.len() + k)?;
                out.put(tile, l.iter().map(|&x| if x.is_nan() { 0.0 } else { x as f32 }).collect())?;
            }
            crate::spent(&[t1 - t0, t2 - t1, t2.elapsed()]);
            Ok(Part::Nothing)
        }
        Task::Hists { phase, ranges, keys } => {
            let nt = phase.transitions.len();
            let v = View::read(tile, 1, src[0], src[1], &src[2..2 + nt])?;
            let mut h: Vec<Hist> = ranges.iter().map(|&(lo, hi)| Hist::new(lo, hi)).collect();
            let classes = phase.by_class(|k| ranges[k].1 > ranges[k].0);
            for y in 1..=tile.height {
                for x in 1..=tile.width {
                    let i = y * v.w + x;
                    for &k in of_class(&classes, v.land[i]) {
                        let val = if *keys {
                            phase.key_of(&v, i, k)
                        } else {
                            let w = weight(&v, i, &phase.transitions[k], k, phase.expander);
                            (w > 0.0).then_some(w)
                        };
                        if let Some(val) = val {
                            h[k].add(val);
                        }
                    }
                }
            }
            Ok(Part::Hists(h))
        }
        Task::Keys { phase, loose } => {
            let nt = phase.transitions.len();
            let v = View::read(tile, 1, src[0], src[1], &src[2..2 + nt])?;
            let mut out = vec![Vec::new(); nt];
            let classes = phase.by_class(|k| phase.demand[k] > 0);
            for y in 1..=tile.height {
                for x in 1..=tile.width {
                    let i = y * v.w + x;
                    for &k in of_class(&classes, v.land[i]) {
                        if let Some(key) = phase.key_of(&v, i, k)
                            && key >= loose[k]
                        {
                            out[k].push(key);
                        }
                    }
                }
            }
            Ok(Part::Keys(out))
        }
        Task::Grow { phase, trim, write, scores } => {
            let nt = phase.transitions.len();
            let halo = 2 * phase.radius + 2;
            let v = View::read(tile, halo, src[0], src[1], &src[2..2 + nt])?;
            let (changes, seeded) = grow_tile(tile, &v, phase);
            let mut m = Made::empty(nt);
            m.seeded = seeded;
            let kept: Vec<&(usize, usize, f64)> = changes.iter().filter(|c| c.2 >= trim[c.1]).collect();
            for &&(_, k, score) in &kept {
                m.made[k] += 1;
                if *scores {
                    m.scores[k].push(score);
                }
            }
            if *write {
                let out = layer(src, 2 + nt)?;
                let mut cells = src[0].read(tile.window(0))?.into_iter().map(|x| x as f32).collect::<Vec<f32>>();
                for &&(i, k, _) in &kept {
                    cells[i] = phase.transitions[k].to as f32;
                }
                out.put(tile, cells)?;
            }
            Ok(Part::Made(m))
        }
        _ => Err(Error::Msg("not a phase's task".into())),
    }
}

fn unexpected() -> Error {
    Error::Msg("a pass gave a part of another kind".into())
}

/// The `m`-th largest of some values (`m` >= 1; the smallest when there are fewer, +inf when none).
fn nth_largest(values: &mut [f64], m: usize) -> f64 {
    if values.is_empty() {
        return f64::INFINITY;
    }
    let m = m.clamp(1, values.len());
    let (_, x, _) = values.select_nth_unstable_by(m - 1, |a, b| b.total_cmp(a));
    *x
}

/// One phase of a step: thresholds, rounds, and the land written.
#[allow(clippy::too_many_arguments)]
fn phase(
    tiling: &Tiling,
    land: &dyn Source,
    start: &dyn Source,
    prob: &[&dyn Source],
    transitions: &[Transition],
    demand: Vec<i64>,
    expander: bool,
    seed: u64,
    step: u64,
    s: &Settings,
    out: &Layer,
) -> Result<(Vec<i64>, Vec<i64>, usize)> {
    let nt = transitions.len();
    let mut ph = Phase {
        expander,
        transitions: transitions.to_vec(),
        demand: demand.clone(),
        prune: vec![f64::INFINITY; nt],
        kappa: vec![f64::INFINITY; nt],
        seed,
        step,
        radius: s.radius,
        width: tiling.width,
    };
    let mut sources: Vec<&dyn Source> = vec![land, start];
    sources.extend(prob.iter().copied());
    let mut writing = sources.clone();
    writing.push(out);
    let grow = |ph: &Phase, trim: Vec<f64>, write: bool, scores: bool| -> Result<Made> {
        let task = Task::Grow { phase: ph.clone(), trim, write, scores };
        match tiling.run(if write { &writing } else { &sources }, &task)? {
            Part::Made(m) => Ok(m),
            Part::Nothing => Ok(Made::empty(nt)),
            _ => Err(unexpected()),
        }
    };
    let active: Vec<usize> = (0..nt).filter(|&k| demand[k] > 0).collect();
    let none = vec![f64::NEG_INFINITY; nt];
    if active.is_empty() {
        let m = grow(&ph, none, true, false)?;
        return Ok((m.made, m.seeded, 0));
    }
    // histograms over the tiles of the weights (or the keys) of each transition's cells
    let hists = |ph: &Phase, ranges: &[(f64, f64)], keys: bool| -> Result<Vec<Hist>> {
        let task = Task::Hists { phase: ph.clone(), ranges: ranges.to_vec(), keys };
        match tiling.run(&sources, &task)? {
            Part::Hists(h) => Ok(h),
            Part::Nothing => Ok(ranges.iter().map(|&(lo, hi)| Hist::new(lo, hi)).collect()),
            _ => Err(unexpected()),
        }
    };
    let only_active =
        |m: &dyn Fn(usize) -> u64| -> Vec<Option<u64>> { (0..nt).map(|k| active.contains(&k).then(|| m(k))).collect() };
    // pruning: the cells of the largest weights
    let prune_m = only_active(&|k| (s.prune * demand[k] as f64).ceil() as u64);
    let prune = top_thresholds(&prune_m, &|r| hists(&ph, r, false))?;
    for &k in &active {
        ph.prune[k] = prune[k].max(f64::MIN_POSITIVE);
    }
    crate::trace(|| format!("step {step} {}: pruning thresholds", phase_name(expander)));
    // the largest keys of each transition (at least `cap[k]` of them, all when `complete[k]`), collected once
    // so that the seeds' threshold of any round is a selection
    let collect = |ph: &Phase, cap: &[u64]| -> Result<(Vec<Vec<f64>>, Vec<bool>)> {
        let loose = top_thresholds(&only_active(&|k| cap[k]), &|r| hists(ph, r, true))?;
        let keys = match tiling.run(&sources, &Task::Keys { phase: ph.clone(), loose: loose.clone() })? {
            Part::Keys(k) => k,
            Part::Nothing => vec![Vec::new(); nt],
            _ => return Err(unexpected()),
        };
        let complete = loose.iter().map(|&l| l <= 0.0).collect();
        Ok((keys, complete))
    };
    let mut want: Vec<f64> = (0..nt)
        .map(|k| {
            let [mean, _, _] = if expander { transitions[k].expander } else { transitions[k].patcher };
            (demand[k] as f64 / mean.max(1.0)).max(1.0)
        })
        .collect();
    let mut cap: Vec<u64> = want.iter().map(|w| ((16.0 * w) as u64).max(4096)).collect();
    let (mut keys, mut complete) = collect(&ph, &cap)?;
    crate::trace(|| format!("step {step} {}: keys collected", phase_name(expander)));
    // a transition is settled once its changes reach its demand without passing it by more than the tolerance
    let settled = |made: i64, k: usize| {
        made >= demand[k] && (made - demand[k]) as f64 <= (s.tolerance * demand[k] as f64).max(1.0)
    };
    let wanted = |want: &[f64], k: usize| want[k].round().max(1.0) as usize;
    let mut done = vec![false; nt];
    let mut best: Option<(f64, Vec<f64>, Made)> = None;
    let mut rounds = 0;
    for _ in 0..s.rounds.max(1) {
        rounds += 1;
        // more keys when a round wants more seeds than were collected
        if active.iter().any(|&k| !complete[k] && wanted(&want, k) > keys[k].len()) {
            for &k in &active {
                cap[k] = cap[k].max(4 * wanted(&want, k) as u64);
            }
            (keys, complete) = collect(&ph, &cap)?;
        }
        for &k in &active {
            // all the candidates when there are no more than wanted
            let m = wanted(&want, k);
            ph.kappa[k] = if complete[k] && m >= keys[k].len() { 0.0 } else { nth_largest(&mut keys[k], m) };
        }
        let m = grow(&ph, none.clone(), false, true)?;
        crate::trace(|| {
            format!("step {step} {}: round {rounds}, made {:?} of {demand:?}", phase_name(expander), m.made)
        });
        // too many changes are cut afterwards: missing ones count more
        let err: f64 = active
            .iter()
            .map(|&k| {
                let d = (m.made[k] - demand[k]) as f64 / demand[k] as f64;
                if d >= 0.0 { d } else { -4.0 * d }
            })
            .sum();
        for &k in &active {
            if settled(m.made[k], k) {
                done[k] = true;
            }
            if done[k] {
                continue;
            }
            // aim a little past the demand: the excess is cut
            let target = demand[k] as f64 * (1.0 + s.tolerance / 2.0);
            let ratio = target / (m.made[k].max(1)) as f64;
            want[k] = (want[k] * ratio.clamp(0.25, 4.0)).max(1.0);
        }
        let enough = active.iter().all(|&k| m.made[k] >= demand[k]);
        if best.as_ref().is_none_or(|(e, _, _)| err < *e) {
            best = Some((err, ph.kappa.clone(), m));
        }
        if enough {
            break;
        }
    }
    let (_, kappa, mut made) = best.unwrap();
    ph.kappa = kappa;
    // the changes past the demand go: the cells grown last in the patches of the smallest keys
    let trim: Vec<f64> = (0..nt)
        .map(|k| {
            if made.made[k] > demand[k] {
                nth_largest(&mut made.scores[k], demand[k] as usize)
            } else {
                f64::NEG_INFINITY
            }
        })
        .collect();
    let m = grow(&ph, trim, true, false)?;
    crate::trace(|| format!("step {step} {}: written", phase_name(expander)));
    Ok((m.made, m.seeded, rounds))
}

/// One step of the Expander and Patcher on tiles: the land at the end of the step (in `dir` when given) and
/// what each transition did. `prob`: a layer per transition (its probability at every cell, 0 where none).
#[allow(clippy::too_many_arguments)]
pub fn step(
    tiling: &Tiling,
    land: &dyn Source,
    prob: &[&Layer],
    transitions: &[Transition],
    seed: u64,
    step: u64,
    s: &Settings,
    dir: Option<std::path::PathBuf>,
) -> Result<(Layer, Vec<Done>)> {
    terraflux_ops::threads::ensure();
    if prob.len() != transitions.len() {
        return Err(Error::Msg("a probability layer per transition".into()));
    }
    let prob: Vec<&dyn Source> = prob.iter().map(|l| *l as &dyn Source).collect();
    let grid = land.grid().clone();
    // the cells of each origin class
    let froms: Vec<f64> = transitions.iter().map(|t| t.from).collect();
    let counts = crate::fit::counts(land, tiling, &froms)?;
    let mut done: Vec<Done> = Vec::new();
    let mut expand = Vec::new();
    let mut patch = Vec::new();
    for t in transitions {
        let n = (t.rate * *counts.get(&t.from.to_bits()).unwrap_or(&0) as f64 + 0.5).floor() as i64;
        let ne = (n as f64 * t.expansion + 0.5).floor() as i64;
        expand.push(ne);
        patch.push(n - ne);
        done.push(Done { requested: n, ..Default::default() });
    }
    let sub = |name: &str| dir.as_ref().map(|d| d.join(name));
    let after_e = Layer::new(grid.clone(), tiling.clone(), sub("expander"))?;
    let (made_e, seeds_e, rounds_e) =
        phase(tiling, land, land, &prob, transitions, expand.clone(), true, seed, step, s, &after_e)?;
    // the Patcher makes the rest of each transition's changes
    for k in 0..transitions.len() {
        patch[k] = (done[k].requested - made_e[k]).max(0);
    }
    let after_p = Layer::new(grid, tiling.clone(), sub("patcher"))?;
    let (made_p, seeds_p, rounds_p) =
        phase(tiling, &after_e, land, &prob, transitions, patch, false, seed, step, s, &after_p)?;
    for k in 0..transitions.len() {
        done[k].expanded = made_e[k];
        done[k].patched = made_p[k];
        done[k].expander_patches = seeds_e[k];
        done[k].patcher_patches = seeds_p[k];
        done[k].expander_rounds = rounds_e;
        done[k].patcher_rounds = rounds_p;
    }
    Ok((after_p, done))
}

/// The probability of each transition on every tile (weights of evidence), from the land and the drivers:
/// a layer per transition of the model, 0 where it does not apply.
pub fn probabilities(
    tiling: &Tiling,
    model: &terraflux_ops::woe::WoeModel,
    land: &dyn Source,
    drivers: &[(String, &dyn Source)],
    dir: Option<std::path::PathBuf>,
) -> Result<Vec<Layer>> {
    terraflux_ops::threads::ensure();
    let grid = land.grid().clone();
    let layers: Vec<Layer> = (0..model.transitions.len())
        .map(|k| Layer::new(grid.clone(), tiling.clone(), dir.as_ref().map(|d| d.join(format!("p{k}")))))
        .collect::<Result<_>>()?;
    let mut sources: Vec<&dyn Source> = vec![land];
    sources.extend(drivers.iter().map(|(_, d)| *d));
    sources.extend(layers.iter().map(|l| l as &dyn Source));
    let names = drivers.iter().map(|(n, _)| n.clone()).collect();
    tiling.run(&sources, &Task::Probabilities { model: model.clone(), names })?;
    crate::trace(|| {
        let [r, c, w] = crate::spent_take();
        format!("probabilities: reading {r:.1} s, computing {c:.1} s, writing {w:.1} s (thread time, here)")
    });
    Ok(layers)
}

/// The transitions of a step from a Markov chain and the patch settings.
pub fn transitions(rates: &terraflux_ops::Markov, patches: &PatchParams) -> Result<Vec<Transition>> {
    terraflux_ops::threads::ensure();
    rates
        .transitions()
        .into_iter()
        .map(|(from, to)| {
            let p = patches.get(from, to).ok_or_else(|| Error::Msg(format!("no patch settings for {from} -> {to}")))?;
            Ok(Transition {
                from,
                to,
                rate: rates.rate(from, to).unwrap(),
                expansion: p.expansion,
                expander: p.expander,
                patcher: p.patcher,
            })
        })
        .collect()
}
