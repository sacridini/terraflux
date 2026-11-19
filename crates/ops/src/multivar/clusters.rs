//! A fitted k-means of a few variables over many records (the cells of a stack, the rows of a table): the centres in
//! the fit's own units (each variable minus its mean, divided by its scale), numbered by a fixed rule so that a
//! cluster keeps its number from one run to the next; every record's nearest centre, in parallel by pieces; and the
//! measures that choose k (the silhouette and the Calinski-Harabasz index).

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::kmeans::{nearest, squared_distance};
use super::linalg::{eigen, sign_of};

/// Records assigned per piece on the threads.
const PIECE: usize = 8192;

/// A fitted k-means: what it takes to give other records their cluster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clusters {
    /// The variables, in order.
    pub predictors: Vec<String>,
    /// The mean of each variable in the fit.
    pub mean: Vec<f64>,
    /// What each variable is divided by after its mean is taken: its standard deviation when standardised, else 1.
    pub scale: Vec<f64>,
    /// The centres, a row each, a value per variable, in the fit's units (the mean taken, the scale divided), in the
    /// order of the clusters: row 0 is cluster 1.
    pub centres: Vec<Vec<f64>>,
}

/// The cluster of each record, and how many records and how much squared distance each cluster has.
pub struct Assigned {
    /// The cluster of each record, numbered from 1; NaN for a record left out.
    pub labels: Vec<f64>,
    /// The records of each cluster.
    pub sizes: Vec<usize>,
    /// The sum of squared distances to the centre (in the fit's units) of the records of each cluster.
    pub inertia: Vec<f64>,
}

impl Clusters {
    /// The number of clusters.
    pub fn k(&self) -> usize {
        self.centres.len()
    }

    /// A centre in the variables' own units.
    pub fn original(&self, c: usize) -> Vec<f64> {
        self.centres[c].iter().zip(&self.scale).zip(&self.mean).map(|((v, s), m)| v * s + m).collect()
    }

    /// The cluster of records, a slice of values per variable (all the same length), each to the nearest centre (the
    /// first of equal distances). A record with NaN in any variable, or where `cells` (if given) is NaN or zero, is
    /// left out. The sizes and the inertia are sums over fixed pieces added in order: the same numbers on any number
    /// of threads.
    pub fn assign(&self, columns: &[&[f64]], cells: Option<&[f64]>) -> Result<Assigned, String> {
        let (d, k) = (self.predictors.len(), self.centres.len());
        if d == 0 || k == 0 {
            return Err("the k-means has no variables or no centres".into());
        }
        if columns.len() != d {
            return Err(format!("the k-means takes {d} variables, not {}", columns.len()));
        }
        let n = columns[0].len();
        if columns.iter().any(|c| c.len() != n) || cells.is_some_and(|c| c.len() != n) {
            return Err("the variables differ in length".into());
        }
        let flat: Vec<f64> = self.centres.concat();
        let mut labels = vec![f64::NAN; n];
        let parts: Vec<(Vec<usize>, Vec<f64>)> = labels
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
                let (mut sizes, mut inertia) = (vec![0usize; k], vec![0.0; k]);
                for (&r, &(c, dist)) in at.iter().zip(&nearest(&rows, d, &flat)) {
                    out[r] = (c + 1) as f64;
                    sizes[c] += 1;
                    inertia[c] += dist;
                }
                (sizes, inertia)
            })
            .collect();
        let (mut sizes, mut inertia) = (vec![0usize; k], vec![0.0; k]);
        for (s, e) in parts {
            for c in 0..k {
                sizes[c] += s[c];
                inertia[c] += e[c];
            }
        }
        Ok(Assigned { labels, sizes, inertia })
    }
}

/// The order the clusters are numbered in: by their score on the first principal component of the centres (the
/// largest loading positive, as `ml.pca`'s), the least score first, a tie by the centre's first position. The same
/// clusters get the same numbers whichever way a run's own numbering went. `centres`: a row each.
pub fn order(centres: &[Vec<f64>]) -> Vec<usize> {
    let (k, d) = (centres.len(), centres.first().map_or(0, Vec::len));
    let mut order: Vec<usize> = (0..k).collect();
    if k < 2 || d == 0 {
        return order;
    }
    let mean: Vec<f64> = (0..d).map(|j| centres.iter().map(|c| c[j]).sum::<f64>() / k as f64).collect();
    let mut cov = vec![0.0; d * d];
    for c in centres {
        for i in 0..d {
            for j in 0..d {
                cov[i * d + j] += (c[i] - mean[i]) * (c[j] - mean[j]);
            }
        }
    }
    let (_, vectors) = eigen(&cov, d);
    let sign = sign_of(&vectors[0]);
    let score: Vec<f64> =
        centres.iter().map(|c| (0..d).map(|j| (c[j] - mean[j]) * vectors[0][j] * sign).sum::<f64>()).collect();
    order.sort_by(|&a, &b| score[a].total_cmp(&score[b]).then(a.cmp(&b)));
    order
}

/// The mean silhouette (Rousseeuw 1987, Euclidean) of the points `sample` (indices into the n x p points `x`) of
/// the clustering `labels` (0..k, of all the points): each point's (b - a) / max(a, b) with a its mean distance to
/// the others of its cluster and b the least mean distance to another cluster's points, 0 for a cluster of one.
/// Only the sampled points count, among themselves. NaN with fewer than two clusters in the sample. Parallel by
/// point, the mean added in order: the same number on any threads.
pub fn silhouette(x: &[f64], p: usize, labels: &[usize], k: usize, sample: &[usize]) -> f64 {
    let m = sample.len();
    let xs: Vec<f64> = sample.iter().flat_map(|&i| x[i * p..(i + 1) * p].iter().copied()).collect();
    let ls: Vec<usize> = sample.iter().map(|&i| labels[i]).collect();
    let mut count = vec![0usize; k];
    for &l in &ls {
        count[l] += 1;
    }
    if count.iter().filter(|&&c| c > 0).count() < 2 {
        return f64::NAN;
    }
    let s: Vec<f64> = (0..m)
        .into_par_iter()
        .map(|i| {
            let xi = &xs[i * p..(i + 1) * p];
            let mut sum = vec![0.0; k];
            for j in 0..m {
                if j != i {
                    sum[ls[j]] += squared_distance(xi, &xs[j * p..(j + 1) * p]).sqrt();
                }
            }
            let li = ls[i];
            if count[li] < 2 {
                return 0.0;
            }
            let a = sum[li] / (count[li] - 1) as f64;
            let b = (0..k)
                .filter(|&c| c != li && count[c] > 0)
                .map(|c| sum[c] / count[c] as f64)
                .fold(f64::INFINITY, f64::min);
            let top = a.max(b);
            if top > 0.0 { (b - a) / top } else { 0.0 }
        })
        .collect();
    s.iter().sum::<f64>() / m as f64
}

/// The Calinski-Harabasz index (the variance ratio criterion) of the clustering `labels` (0..k) of the n x p points
/// `x`: the between-cluster dispersion over the within, each over its degrees of freedom, so higher is better. NaN
/// with fewer than two clusters or as many as points; 1 when the clusters have no spread (as scikit-learn's).
pub fn calinski_harabasz(x: &[f64], p: usize, labels: &[usize], k: usize) -> f64 {
    let n = labels.len();
    let (mut size, mut mean, mut all) = (vec![0usize; k], vec![0.0; k * p], vec![0.0; p]);
    for (i, &l) in labels.iter().enumerate() {
        size[l] += 1;
        for j in 0..p {
            mean[l * p + j] += x[i * p + j];
            all[j] += x[i * p + j];
        }
    }
    let used = size.iter().filter(|&&c| c > 0).count();
    if used < 2 || n <= used {
        return f64::NAN;
    }
    all.iter_mut().for_each(|v| *v /= n as f64);
    for c in 0..k {
        mean[c * p..(c + 1) * p].iter_mut().for_each(|v| *v /= size[c].max(1) as f64);
    }
    let between: f64 = (0..k).map(|c| size[c] as f64 * squared_distance(&mean[c * p..(c + 1) * p], &all)).sum();
    let within: f64 = labels
        .iter()
        .enumerate()
        .map(|(i, &l)| squared_distance(&x[i * p..(i + 1) * p], &mean[l * p..(l + 1) * p]))
        .sum();
    if within == 0.0 {
        return 1.0;
    }
    between * (n - used) as f64 / (within * (used - 1) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multivar::kmeans::{Rng, kmeans};

    /// Three groups of 40 points in 2-D, 0-1 wide, at (0, 0), (10, 0) and (0, 10).
    fn groups() -> Vec<f64> {
        let mut rng = Rng::new(3);
        let mut x = vec![];
        for (cx, cy) in [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)] {
            for _ in 0..40 {
                x.push(cx + rng.uniform());
                x.push(cy + rng.uniform());
            }
        }
        x
    }

    fn model(centres: Vec<Vec<f64>>) -> Clusters {
        Clusters {
            predictors: ["u", "v"].map(String::from).to_vec(),
            mean: vec![1.0, 2.0],
            scale: vec![2.0, 4.0],
            centres,
        }
    }

    #[test]
    fn the_order_does_not_depend_on_the_numbering() {
        let c = vec![vec![5.0, 0.0], vec![-5.0, 0.1], vec![0.0, 0.0]];
        assert_eq!(order(&c), vec![1, 2, 0]);
        // the same centres numbered otherwise are put in the same order
        let shuffled = vec![c[2].clone(), c[0].clone(), c[1].clone()];
        let o: Vec<Vec<f64>> = order(&shuffled).iter().map(|&i| shuffled[i].clone()).collect();
        let o0: Vec<Vec<f64>> = order(&c).iter().map(|&i| c[i].clone()).collect();
        assert_eq!(o, o0);
        assert_eq!(order(&c[..1]), vec![0]);
    }

    #[test]
    fn assign_gives_the_nearest_centre_and_leaves_out_nan_and_cells() {
        let m = model(vec![vec![0.0, 0.0], vec![3.0, 0.0]]);
        // in the original units a centre is value * scale + mean
        assert_eq!(m.original(1), vec![7.0, 2.0]);
        let u = [1.0, 7.0, f64::NAN, 7.0, 1.0];
        let v = [2.0, 2.0, 2.0, 2.0, 2.0];
        let cells = [1.0, 1.0, 1.0, 0.0, f64::NAN];
        let a = m.assign(&[&u, &v], Some(&cells)).unwrap();
        assert_eq!(a.labels[..2], [1.0, 2.0]);
        assert!(a.labels[2..].iter().all(|l| l.is_nan()));
        assert_eq!(a.sizes, vec![1, 1]);
        assert_eq!(a.inertia, vec![0.0, 0.0]);
        assert!(m.assign(&[&u], None).is_err());
        assert!(m.assign(&[&u, &v[..3]], None).is_err());
        assert!(m.assign(&[&u, &v], Some(&cells[..2])).is_err());
    }

    #[test]
    fn the_measures_of_well_separated_groups() {
        let x = groups();
        let km = kmeans(&x, 120, 2, 3, 3, 1);
        let sample: Vec<usize> = (0..120).collect();
        let s = silhouette(&x, 2, &km.labels, 3, &sample);
        assert!(s > 0.85 && s <= 1.0, "{s}");
        // a worse k has a worse silhouette, and a smaller index
        let two = kmeans(&x, 120, 2, 2, 3, 1);
        assert!(silhouette(&x, 2, &two.labels, 2, &sample) < s);
        assert!(calinski_harabasz(&x, 2, &two.labels, 2) < calinski_harabasz(&x, 2, &km.labels, 3));
        assert!(silhouette(&x, 2, &vec![0; 120], 1, &sample).is_nan());
        assert!(calinski_harabasz(&x, 2, &vec![0; 120], 1).is_nan());
        // a half of the points: still the same clusters, a silhouette about the same
        let half: Vec<usize> = (0..120).step_by(2).collect();
        assert!((silhouette(&x, 2, &km.labels, 3, &half) - s).abs() < 0.05);
    }

    /// By hand: points 0, 1 and 10, 11 on a line, two clusters. For the point 0: a = 1, b = (10 + 11) / 2 = 10.5,
    /// s = 9.5 / 10.5; the point 1: a = 1, b = 9.5, s = 8.5 / 9.5; the others by symmetry.
    #[test]
    fn the_silhouette_by_hand() {
        let x = [0.0, 1.0, 10.0, 11.0];
        let s = silhouette(&x, 1, &[0, 0, 1, 1], 2, &[0, 1, 2, 3]);
        let want = (9.5 / 10.5 + 8.5 / 9.5) / 2.0;
        assert!((s - want).abs() < 1e-12, "{s} {want}");
        // the index: between = 4 * 5^2 = 100, within = 4 * 0.25 = 1: 100 * (4 - 2) / (1 * 1)
        assert!((calinski_harabasz(&x, 1, &[0, 0, 1, 1], 2) - 200.0).abs() < 1e-9);
        // a cluster of one scores 0
        let s = silhouette(&x, 1, &[0, 1, 1, 1], 2, &[0, 1, 2, 3]);
        assert!(s.is_finite());
    }
}
