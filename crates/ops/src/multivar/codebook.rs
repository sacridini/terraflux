//! A fitted self-organizing map of a few variables over many records (the cells of a stack, the rows of a table): the
//! codebook (a weight vector per node, in the fit's own units: each variable minus its mean, divided by its scale),
//! the nodes in rows and columns (rectangular or hexagonal, MiniSom's), its training (the engine of `som`: batch or
//! online), the size Vesanto & Alhoniemi (2000) suggest, the groups of nodes (the second level of their two-level
//! approach), every record's best matching unit in parallel by pieces, the quantisation and topographic errors and the
//! U-matrix.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::clusters::order;
use super::kmeans::{Init, Settings, cut, kmeans_with, squared_distance, ward};
use super::linalg::{eigen, sign_of};
use super::som::{self, Grid, Schedule, Topology};
use crate::signatures::NumpyRng;

/// Records assigned per piece on the threads.
const PIECE: usize = 8192;

/// The most nodes a map has (the neighbourhoods of its winners take a square of them in memory).
pub const MAX_NODES: usize = 4096;

/// The most nodes the size by Vesanto's rule gives, for a map of very many records.
pub const MAX_AUTO_NODES: usize = 2500;

/// A fitted map: what it takes to give other records their node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Som {
    /// The variables, in order.
    pub predictors: Vec<String>,
    /// The rows of nodes.
    pub rows: usize,
    /// The nodes of a row.
    pub columns: usize,
    pub topology: Topology,
    /// The mean of each variable in the fit.
    pub mean: Vec<f64>,
    /// What each variable is divided by after its mean is taken: its standard deviation when standardised, else 1.
    pub scale: Vec<f64>,
    /// The weights of the nodes, a row each, a value per variable, in the fit's units, the nodes by row then column:
    /// node `r * columns + c` (from 0) is the row r, column c.
    pub weights: Vec<Vec<f64>>,
    /// The group (from 1) of each node, none when the nodes were not grouped.
    #[serde(default)]
    pub groups: Vec<usize>,
}

/// The node and group of each record, and how many records each node has.
pub struct Assigned {
    /// The node of each record, from 1 (`r * columns + c + 1`); NaN for a record left out.
    pub nodes: Vec<f64>,
    /// The group of each record's node, from 1; empty without groups, NaN for a record left out.
    pub groups: Vec<f64>,
    /// The records of each node.
    pub counts: Vec<usize>,
}

impl Som {
    /// The number of nodes.
    pub fn len(&self) -> usize {
        self.rows * self.columns
    }

    pub fn is_empty(&self) -> bool {
        self.weights.is_empty()
    }

    /// The groups the nodes are in (0 when not grouped).
    pub fn group_count(&self) -> usize {
        self.groups.iter().copied().max().unwrap_or(0)
    }

    /// A node's weights in the variables' own units.
    pub fn original(&self, k: usize) -> Vec<f64> {
        self.weights[k].iter().zip(&self.scale).zip(&self.mean).map(|((v, s), m)| v * s + m).collect()
    }

    /// The index MiniSom (and the engine) gives the node at `row`, `column`: `column * rows + row`.
    fn engine_index(&self, row: usize, column: usize) -> usize {
        column * self.rows + row
    }

    /// The coordinates of each node (`(x, y)`, the nodes by row then column): the columns apart by 1 along a row, the
    /// rows by 1 (rectangular) or by (3 / 2) / sqrt(3) with every other row shifted half a node (hexagonal).
    pub fn coordinates(&self) -> Vec<(f64, f64)> {
        let grid = Grid::minisom(self.columns, self.rows, self.topology);
        let (xx, yy) = grid.coordinates();
        let mut out = Vec::with_capacity(self.len());
        for r in 0..self.rows {
            for c in 0..self.columns {
                let e = self.engine_index(r, c);
                out.push((xx[e], yy[e]));
            }
        }
        out
    }

    /// The nodes next to each node: the 8 around it in a rectangular map; in a hexagonal map those at the distance 1
    /// (as MiniSom's topographic error counts them), up to 6.
    pub fn neighbours(&self) -> Vec<Vec<usize>> {
        let coords = self.coordinates();
        let (rows, columns) = (self.rows as i64, self.columns as i64);
        let mut out = vec![Vec::new(); self.len()];
        for r in 0..rows {
            for c in 0..columns {
                let k = (r * columns + c) as usize;
                for dr in -1..=1 {
                    for dc in -1..=1 {
                        let (rr, cc) = (r + dr, c + dc);
                        if (dr == 0 && dc == 0) || rr < 0 || cc < 0 || rr >= rows || cc >= columns {
                            continue;
                        }
                        let j = (rr * columns + cc) as usize;
                        let near = match self.topology {
                            Topology::Rectangular => true,
                            Topology::Hexagonal => {
                                let d =
                                    ((coords[k].0 - coords[j].0).powi(2) + (coords[k].1 - coords[j].1).powi(2)).sqrt();
                                // numpy's isclose(1, d)
                                (d - 1.0).abs() <= 1e-8 + 1e-5
                            }
                        };
                        if near {
                            out[k].push(j);
                        }
                    }
                }
            }
        }
        out
    }

    /// The U-matrix: each node's mean distance (in the fit's units) to the nodes next to it, large where the map is
    /// stretched across a gap in the records and small inside a group; NaN for a map of one node.
    pub fn umatrix(&self) -> Vec<f64> {
        self.neighbours()
            .iter()
            .enumerate()
            .map(|(k, near)| {
                if near.is_empty() {
                    return f64::NAN;
                }
                near.iter().map(|&j| squared_distance(&self.weights[k], &self.weights[j]).sqrt()).sum::<f64>()
                    / near.len() as f64
            })
            .collect()
    }

    /// The node (and group) of records, a slice of values per variable (all the same length), each to the nearest
    /// node (the first of equal distances). A record with NaN in any variable, or where `cells` (if given) is NaN or
    /// zero, is left out. The counts are sums over fixed pieces added in order: the same numbers on any number of
    /// threads.
    pub fn assign(&self, columns: &[&[f64]], cells: Option<&[f64]>) -> Result<Assigned, String> {
        let (d, k) = (self.predictors.len(), self.weights.len());
        if d == 0 || k == 0 {
            return Err("the map has no variables or no nodes".into());
        }
        if columns.len() != d {
            return Err(format!("the map takes {d} variables, not {}", columns.len()));
        }
        let n = columns[0].len();
        if columns.iter().any(|c| c.len() != n) || cells.is_some_and(|c| c.len() != n) {
            return Err("the variables differ in length".into());
        }
        let flat: Vec<f64> = self.weights.concat();
        let mut nodes = vec![f64::NAN; n];
        let parts: Vec<Vec<usize>> = nodes
            .par_chunks_mut(PIECE)
            .enumerate()
            .map(|(b, out)| {
                let lo = b * PIECE;
                // the records of the piece with every variable, standardised, a row each
                let (mut at, mut rows) = (Vec::new(), Vec::with_capacity(out.len() * d));
                for r in 0..out.len() {
                    let i = lo + r;
                    if cells.is_some_and(|c| c[i].is_nan() || c[i] == 0.0) || columns.iter().any(|c| c[i].is_nan()) {
                        continue;
                    }
                    at.push(r);
                    rows.extend((0..d).map(|j| (columns[j][i] - self.mean[j]) / self.scale[j]));
                }
                let mut best = vec![0usize; at.len()];
                let mut buf = vec![0.0; d];
                som::bmus_serial(&rows, d, &flat, k, &mut best, &mut buf);
                let mut counts = vec![0usize; k];
                for (&r, &b) in at.iter().zip(&best) {
                    out[r] = (b + 1) as f64;
                    counts[b] += 1;
                }
                counts
            })
            .collect();
        let mut counts = vec![0usize; k];
        for part in parts {
            for (c, p) in counts.iter_mut().zip(part) {
                *c += p;
            }
        }
        let groups = if self.groups.len() == k {
            nodes.iter().map(|&v| if v.is_nan() { f64::NAN } else { self.groups[v as usize - 1] as f64 }).collect()
        } else {
            Vec::new()
        };
        Ok(Assigned { nodes, groups, counts })
    }

    /// The quantisation error (the mean distance of the records to their nodes) and the topographic error (the share
    /// of records whose two nearest nodes are not next to each other: 0 when the map keeps the neighbourhoods) of the
    /// `n` records `x` (n x d, in the fit's units). NaN for a map of one node (its topographic error) or without
    /// records.
    pub fn quality(&self, x: &[f64]) -> (f64, f64) {
        let (d, k) = (self.predictors.len(), self.weights.len());
        let two = som::predict_two(x, d, &self.weights.concat(), k);
        if two.is_empty() {
            return (f64::NAN, f64::NAN);
        }
        let qe = two.iter().map(|t| t.2).sum::<f64>() / two.len() as f64;
        if k < 2 {
            return (qe, f64::NAN);
        }
        let near = self.neighbours();
        let bad = two.iter().filter(|t| !near[t.0].contains(&t.1)).count();
        (qe, bad as f64 / two.len() as f64)
    }
}

/// The rule of thumb of the number of nodes: 5 sqrt(n) for n records, at most `MAX_AUTO_NODES`.
pub fn wanted_nodes(n: usize) -> usize {
    ((5.0 * (n as f64).sqrt()).ceil() as usize).clamp(4, MAX_AUTO_NODES)
}

/// A map's size by Vesanto & Alhoniemi's rule (SOM Toolbox's `som_topol_struct`): about 5 sqrt(n) nodes, the sides
/// in the ratio of the square roots of the two largest eigenvalues of the covariance `cov` (d x d) of the records, so
/// the map is stretched as they are: rows is the longer side. At least 2 by 2.
pub fn vesanto(n: usize, cov: &[f64], d: usize) -> (usize, usize) {
    let units = wanted_nodes(n);
    let ratio = if d >= 2 {
        let (vals, _) = eigen(cov, d);
        if vals[1] > 1e-12 * vals[0] && vals[0].is_finite() { (vals[0] / vals[1]).sqrt() } else { f64::INFINITY }
    } else {
        1.0
    };
    let short = if ratio.is_finite() { ((units as f64 / ratio).sqrt().round() as usize).clamp(2, units) } else { 2 };
    let long = ((units as f64 / short as f64).round() as usize).max(short);
    (long, short)
}

/// How the neighbours of the winning node are drawn in with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Neighbourhood {
    Gaussian,
    Bubble,
    Triangle,
}

impl Neighbourhood {
    fn code(self) -> i32 {
        match self {
            Neighbourhood::Gaussian => 0,
            Neighbourhood::Bubble => 2,
            Neighbourhood::Triangle => 3,
        }
    }
}

/// How the learning rate and the sigma shrink: MiniSom's asymptotic decay (lr / (1 + t / (T / 2)), sigma / (1 + t
/// (sigma - 1) / T)), the inverse decays, or the linear ones (to 0 and to 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decay {
    Asymptotic,
    Inverse,
    Linear,
}

impl Decay {
    fn code(self) -> i32 {
        match self {
            Decay::Asymptotic => 0,
            Decay::Inverse => 1,
            Decay::Linear => 2,
        }
    }
}

/// Batch (Kohonen's: all the records, then the weights) or online (a record at a time, in random order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Training {
    Batch,
    Online,
}

/// The weights before the training: spread on the plane of the first two principal components (spanning two standard
/// deviations to each side; Kohonen's linear initialisation, with the longer side of the map on the first), or the
/// records of random draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    Pca,
    Random,
}

/// The settings of a training.
#[derive(Debug, Clone, Copy)]
pub struct Params {
    pub rows: usize,
    pub columns: usize,
    pub topology: Topology,
    pub neighbourhood: Neighbourhood,
    pub learning_rate: f64,
    pub sigma: f64,
    pub decay: Decay,
    pub training: Training,
    /// Batch: the iterations over all the records; online: the passes over them (a step for each record).
    pub iterations: usize,
    pub start: Start,
    pub seed: u64,
}

/// A position along `n` nodes from -1 to 1 (0 for one).
fn linspace(i: usize, n: usize) -> f64 {
    if n < 2 { 0.0 } else { -1.0 + 2.0 * i as f64 / (n - 1) as f64 }
}

/// The weights (row-major nodes, d values each) before the training from the records `x` (n x d) and their covariance
/// `cov`.
fn initial(x: &[f64], n: usize, d: usize, cov: &[f64], p: &Params, rng: &mut NumpyRng) -> Vec<f64> {
    let k = p.rows * p.columns;
    let mut w = vec![0.0; k * d];
    if p.start == Start::Pca && d >= 2 {
        let (vals, vecs) = eigen(cov, d);
        let (s1, s2) = (sign_of(&vecs[0]), sign_of(&vecs[1]));
        let (l1, l2) = (2.0 * vals[0].max(0.0).sqrt(), 2.0 * vals[1].max(0.0).sqrt());
        for r in 0..p.rows {
            for c in 0..p.columns {
                // the longer side of the map along the first component
                let (a, b) = if p.rows >= p.columns {
                    (linspace(r, p.rows), linspace(c, p.columns))
                } else {
                    (linspace(c, p.columns), linspace(r, p.rows))
                };
                let node = &mut w[(r * p.columns + c) * d..(r * p.columns + c + 1) * d];
                for j in 0..d {
                    node[j] = a * l1 * s1 * vecs[0][j] + b * l2 * s2 * vecs[1][j];
                }
            }
        }
    } else {
        for node in w.chunks_exact_mut(d) {
            let i = rng.integers(n as u64) as usize;
            node.copy_from_slice(&x[i * d..(i + 1) * d]);
        }
    }
    w
}

/// The weights (nodes by row then column, a row of d values each) of a map trained on the records `x` (n x d, in the
/// fit's units: centred, scaled; `cov` their covariance, d x d) with the settings `p`, on the rayon pool of the
/// caller: the same weights on any number of threads.
pub fn fit(x: &[f64], n: usize, d: usize, cov: &[f64], p: &Params) -> Vec<Vec<f64>> {
    assert!(x.len() == n * d && n > 0 && d > 0 && p.rows > 0 && p.columns > 0, "x must be n x d, the map not empty");
    let mut rng = NumpyRng::new(p.seed);
    let start = initial(x, n, d, cov, p, &mut rng);
    // the engine's order of the nodes: the column then the row
    let (rows, columns) = (p.rows, p.columns);
    let mut w = vec![0.0; start.len()];
    for r in 0..rows {
        for c in 0..columns {
            let (from, to) = ((r * columns + c) * d, (c * rows + r) * d);
            w[to..to + d].copy_from_slice(&start[from..from + d]);
        }
    }
    let grid = Grid::minisom(columns, rows, p.topology);
    let s = Schedule {
        learning_rate: p.learning_rate,
        sigma: p.sigma,
        lr_decay: p.decay.code(),
        sigma_decay: p.decay.code(),
        neighborhood: p.neighbourhood.code(),
    };
    match p.training {
        Training::Batch => som::train_batch(&mut w, x, d, p.iterations, &s, &grid),
        Training::Online => {
            let mut order = Vec::with_capacity(n * p.iterations);
            for _ in 0..p.iterations {
                let mut pass: Vec<usize> = (0..n).collect();
                rng.shuffle(&mut pass);
                order.extend(pass);
            }
            som::train_online(&mut w, x, d, &order, order.len(), false, &s, &grid);
        }
    }
    (0..rows * columns)
        .map(|k| {
            let (r, c) = (k / columns, k % columns);
            let at = (c * rows + r) * d;
            w[at..at + d].to_vec()
        })
        .collect()
}

/// How the nodes are grouped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grouping {
    Ward,
    KMeans,
}

/// The nodes (the `weights`) in `k` groups by Ward's method or by k-means (the greedy start, the best of 10 runs):
/// each node's group, from 1, numbered by the first principal component of the groups' centres as `ml.kmeans`' clusters
/// are (the same numbers from run to run).
pub fn group_nodes(weights: &[Vec<f64>], k: usize, how: Grouping, seed: u64) -> Vec<usize> {
    let (n, d) = (weights.len(), weights[0].len());
    let flat = weights.concat();
    let labels: Vec<usize> = match how {
        Grouping::Ward => cut(&ward(&flat, n, d), n, k),
        Grouping::KMeans => {
            kmeans_with(&flat, n, d, k, seed, Init::Greedy, Settings { starts: 10, ..Settings::default() }).labels
        }
    };
    // the centres of the groups that have nodes
    let mut ids: Vec<usize> = labels.clone();
    ids.sort_unstable();
    ids.dedup();
    let centres: Vec<Vec<f64>> = ids
        .iter()
        .map(|&g| {
            let mut sum = vec![0.0; d];
            let mut count = 0.0;
            for (w, _) in weights.iter().zip(&labels).filter(|(_, l)| **l == g) {
                sum.iter_mut().zip(w).for_each(|(s, v)| *s += v);
                count += 1.0;
            }
            sum.iter().map(|s| s / count).collect()
        })
        .collect();
    let mut number = vec![0usize; ids.iter().copied().max().map_or(0, |m| m + 1)];
    for (pos, &i) in order(&centres).iter().enumerate() {
        number[ids[i]] = pos + 1;
    }
    labels.iter().map(|&l| number[l]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(rows: usize, columns: usize, topology: Topology, weights: Vec<Vec<f64>>) -> Som {
        let d = weights[0].len();
        Som {
            predictors: (0..d).map(|j| format!("v{j}")).collect(),
            rows,
            columns,
            topology,
            mean: vec![0.0; d],
            scale: vec![1.0; d],
            weights,
            groups: vec![],
        }
    }

    #[test]
    fn the_umatrix_by_hand() {
        // a row of three nodes at 0, 1 and 3: the ends have one neighbour, the middle two
        let m = map(1, 3, Topology::Rectangular, vec![vec![0.0], vec![1.0], vec![3.0]]);
        assert_eq!(m.umatrix(), vec![1.0, 1.5, 2.0]);
        // one node has no neighbour
        assert!(map(1, 1, Topology::Rectangular, vec![vec![0.0]]).umatrix()[0].is_nan());
        // 2 x 2: every node touches the other three (the diagonal too): node (0,0) at (0,0), the others 3, 4 and 5
        // away along a line: distances 3, 4, 5, mean 4
        let m = map(2, 2, Topology::Rectangular, vec![vec![0.0, 0.0], vec![3.0, 0.0], vec![0.0, 4.0], vec![3.0, 4.0]]);
        assert!((m.umatrix()[0] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn the_neighbours_of_a_hexagonal_map_are_the_six_around() {
        let w = vec![vec![0.0]; 9];
        let m = map(3, 3, Topology::Hexagonal, w.clone());
        let near = m.neighbours();
        // the centre has six, a corner two: (row 0, column 0) touches the node beside it and the one above (rows 0 and 2
        // are the shifted ones)
        assert_eq!(near[4], vec![1, 2, 3, 5, 7, 8]);
        assert_eq!(near[0], vec![1, 3]);
        // a rectangular map has eight around the centre, three at a corner
        let r = map(3, 3, Topology::Rectangular, w);
        assert_eq!(r.neighbours()[4].len(), 8);
        assert_eq!(r.neighbours()[0], vec![1, 3, 4]);
    }

    /// By hand: four nodes in a row with the weights 0, 2, 1, 3; the records 0.2 (nodes 0 and 2: not next to each
    /// other), 1.4 (2 and 1: next), 1.6 (1 and 2: next) and 2.9 (3 and 1: not).
    #[test]
    fn the_errors_by_hand() {
        let m = map(1, 4, Topology::Rectangular, vec![vec![0.0], vec![2.0], vec![1.0], vec![3.0]]);
        let (qe, te) = m.quality(&[0.2, 1.4, 1.6, 2.9]);
        assert!((qe - (0.2 + 0.4 + 0.4 + 0.1) / 4.0).abs() < 1e-12, "{qe}");
        assert_eq!(te, 0.5);
        // one node: no topographic error
        let one = map(1, 1, Topology::Rectangular, vec![vec![0.0]]);
        let (qe, te) = one.quality(&[3.0, -1.0]);
        assert_eq!(qe, 2.0);
        assert!(te.is_nan());
    }

    #[test]
    fn assign_gives_the_nearest_node_and_leaves_out_nan_and_cells() {
        let mut m = map(1, 2, Topology::Rectangular, vec![vec![0.0, 0.0], vec![3.0, 0.0]]);
        m.mean = vec![1.0, 2.0];
        m.scale = vec![2.0, 4.0];
        m.groups = vec![2, 1];
        // in the original units a node is value * scale + mean
        assert_eq!(m.original(1), vec![7.0, 2.0]);
        let u = [1.0, 7.0, f64::NAN, 7.0, 1.0];
        let v = [2.0, 2.0, 2.0, 2.0, 2.0];
        let cells = [1.0, 1.0, 1.0, 0.0, f64::NAN];
        let a = m.assign(&[&u, &v], Some(&cells)).unwrap();
        assert_eq!(a.nodes[..2], [1.0, 2.0]);
        assert!(a.nodes[2..].iter().all(|l| l.is_nan()));
        assert_eq!(a.groups[..2], [2.0, 1.0]);
        assert!(a.groups[2..].iter().all(|l| l.is_nan()));
        assert_eq!(a.counts, vec![1, 1]);
        assert!(m.assign(&[&u], None).is_err());
        assert!(m.assign(&[&u, &v[..3]], None).is_err());
        assert!(m.assign(&[&u, &v], Some(&cells[..2])).is_err());
    }

    #[test]
    fn vesantos_size() {
        // 400 records of nearly equal spread: a square of about 5 * 20 = 100 nodes
        assert_eq!(wanted_nodes(400), 100);
        assert_eq!(vesanto(400, &[1.0, 0.0, 0.0, 1.0], 2), (10, 10));
        // a first eigenvalue 4 times the second: the sides 2 : 1, the longer the rows
        let (rows, columns) = vesanto(400, &[4.0, 0.0, 0.0, 1.0], 2);
        assert!(rows == 14 && columns == 7, "{rows} x {columns}");
        // one variable: a square; almost one direction only: 2 columns
        assert_eq!(vesanto(400, &[1.0], 1), (10, 10));
        assert_eq!(vesanto(400, &[1.0, 0.0, 0.0, 0.0], 2), (50, 2));
        // very many records: at most 2500 nodes
        assert_eq!(wanted_nodes(100_000_000), MAX_AUTO_NODES);
        assert_eq!(wanted_nodes(1), 5);
        assert_eq!(wanted_nodes(0), 4);
    }

    /// Two groups of 100 points in 2-D, 1 wide, at (-5, -5) and (5, 5) (the even records the first), interleaved.
    fn blobs() -> Vec<f64> {
        let mut rng = crate::multivar::kmeans::Rng::new(5);
        let mut x = vec![];
        for g in 0..200 {
            let c = if g % 2 == 0 { -5.0 } else { 5.0 };
            x.push(c + rng.uniform() - 0.5);
            x.push(c + rng.uniform() - 0.5);
        }
        x
    }

    /// The covariance of `blobs`, about.
    const BLOBS_COV: [f64; 4] = [25.1, 25.0, 25.0, 25.1];

    fn params(rows: usize, columns: usize, training: Training, start: Start) -> Params {
        Params {
            rows,
            columns,
            topology: Topology::Rectangular,
            neighbourhood: Neighbourhood::Gaussian,
            learning_rate: 0.5,
            sigma: 2.0,
            decay: Decay::Asymptotic,
            training,
            iterations: if training == Training::Batch { 30 } else { 3 },
            start,
            seed: 7,
        }
    }

    #[test]
    fn a_trained_map_separates_two_groups() {
        let x = blobs();
        let (u, v): (Vec<f64>, Vec<f64>) =
            (x.iter().step_by(2).copied().collect(), x.iter().skip(1).step_by(2).copied().collect());
        for (training, start) in
            [(Training::Batch, Start::Pca), (Training::Batch, Start::Random), (Training::Online, Start::Pca)]
        {
            let w = fit(&x, 200, 2, &BLOBS_COV, &params(3, 4, training, start));
            let m = map(3, 4, Topology::Rectangular, w);
            let a = m.assign(&[&u, &v], None).unwrap();
            let nodes_of = |first: usize| -> std::collections::BTreeSet<usize> {
                a.nodes.iter().skip(first).step_by(2).map(|v| *v as usize - 1).collect()
            };
            let (low, high) = (nodes_of(0), nodes_of(1));
            assert!(low.is_disjoint(&high), "{training:?} {start:?}: {low:?} {high:?}");
            // the groups of nodes, by Ward and by k-means: two, one for each blob, the blob at the origin's first (the
            // groups are numbered by the first component of their centres)
            for how in [Grouping::Ward, Grouping::KMeans] {
                let g = group_nodes(&m.weights, 2, how, 1);
                let of = |nodes: &std::collections::BTreeSet<usize>| -> Vec<usize> {
                    nodes.iter().map(|&k| g[k]).collect::<std::collections::BTreeSet<_>>().into_iter().collect()
                };
                assert_eq!((of(&low), of(&high)), (vec![1], vec![2]), "{training:?} {start:?} {how:?}");
            }
        }
    }

    #[test]
    fn a_training_is_the_same_on_any_threads_and_from_the_same_seed() {
        let x = blobs();
        let cov = BLOBS_COV;
        for training in [Training::Batch, Training::Online] {
            for topology in [Topology::Rectangular, Topology::Hexagonal] {
                let p = Params { topology, ..params(4, 3, training, Start::Random) };
                let on = |threads: usize| {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(threads)
                        .build()
                        .unwrap()
                        .install(|| fit(&x, 200, 2, &cov, &p))
                };
                let (a, b) = (on(1), on(8));
                assert!(a.concat().iter().zip(b.concat()).all(|(u, v)| u.to_bits() == v.to_bits()), "{training:?}");
                assert_eq!(a, on(2));
                let other = fit(&x, 200, 2, &cov, &Params { seed: 8, ..p });
                assert_ne!(a, other, "another seed, another map");
            }
        }
    }
}
