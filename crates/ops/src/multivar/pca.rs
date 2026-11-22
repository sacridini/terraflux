//! Principal components of a few variables over many records (the cells of a stack, the rows of a table): the
//! eigenvectors of the covariance or the correlation (one pass over the records, `linalg::moments`; Jacobi's
//! rotations, `linalg::eigen`), the scores of any records from the fit's own means and scales.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::linalg::{eigen, moments, sign_of};

/// Records scored per piece on the threads.
const PIECE: usize = 8192;

/// How many components to keep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Keep {
    /// This many.
    Count(usize),
    /// As few as explain at least this share of the variance (0 to 1).
    Share(f64),
}

/// A fitted PCA: what it takes to score other records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pca {
    /// The variables, in order.
    pub predictors: Vec<String>,
    /// The mean of each variable in the fit.
    pub mean: Vec<f64>,
    /// What each variable is divided by after its mean is taken: its standard deviation for the correlation, 1 for
    /// the covariance.
    pub scale: Vec<f64>,
    /// The components kept, a row each (unit length, the largest loading positive), a loading per variable.
    pub components: Vec<Vec<f64>>,
    /// The variance of every component (not only those kept), from the largest.
    pub variance: Vec<f64>,
    /// The share of the total variance of every component.
    pub explained: Vec<f64>,
}

impl Pca {
    /// The components kept.
    pub fn k(&self) -> usize {
        self.components.len()
    }

    /// The scores of records, a slice of values per variable (all the same length): a vector of scores per component.
    /// A record with NaN in any variable has NaN scores. Each score is a sum in the variables' order, so the numbers
    /// are the same on any number of threads.
    pub fn scores(&self, columns: &[&[f64]]) -> Result<Vec<Vec<f64>>, String> {
        let d = self.predictors.len();
        if columns.len() != d {
            return Err(format!("the PCA takes {d} variables, not {}", columns.len()));
        }
        let n = columns.first().map_or(0, |c| c.len());
        if columns.iter().any(|c| c.len() != n) {
            return Err("the variables differ in length".into());
        }
        Ok(self
            .components
            .iter()
            .map(|comp| {
                let mut out = vec![f64::NAN; n];
                out.par_chunks_mut(PIECE).enumerate().for_each(|(p, piece)| {
                    for (r, o) in piece.iter_mut().enumerate() {
                        let i = p * PIECE + r;
                        let mut s = 0.0;
                        for j in 0..d {
                            s += comp[j] * ((columns[j][i] - self.mean[j]) / self.scale[j]);
                        }
                        // NaN in any variable makes the sum NaN
                        *o = s;
                    }
                });
                out
            })
            .collect())
    }
}

/// How many components `keep` asks for, given the share of the variance of each (all of them, from the largest).
pub fn kept(explained: &[f64], keep: Keep) -> Result<usize, String> {
    let d = explained.len();
    Ok(match keep {
        Keep::Count(k) if k == 0 || k > d => {
            return Err(format!("components {k}: 1 to {d} (the number of variables), or a share of the variance"));
        }
        Keep::Count(k) => k,
        Keep::Share(s) => {
            let mut cum = 0.0;
            let mut k = d;
            for (i, e) in explained.iter().enumerate() {
                cum += e;
                if cum >= s - 1e-12 {
                    k = i + 1;
                    break;
                }
            }
            k
        }
    })
}

/// The PCA of the records (a vector of values per variable, none NaN, all the same length): of the correlation
/// (`standardise`: each variable by its standard deviation) or of the covariance, keeping some components.
pub fn fit(names: Vec<String>, columns: &[Vec<f64>], standardise: bool, keep: Keep) -> Result<Pca, String> {
    fit_noting(names, columns, standardise, keep).map(|(p, _)| p)
}

/// [`fit`], and the constant variables too: standardising, as scikit-learn's StandardScaler does, one is centred and
/// not divided (scale 1) and weighs nothing (a zero loading in every component).
pub fn fit_noting(
    names: Vec<String>,
    columns: &[Vec<f64>],
    standardise: bool,
    keep: Keep,
) -> Result<(Pca, Vec<usize>), String> {
    let d = names.len();
    if d == 0 || columns.len() != d {
        return Err("no variables".into());
    }
    let slices: Vec<&[f64]> = columns.iter().map(Vec::as_slice).collect();
    let m = moments(&slices, 1).ok_or("a PCA needs two records with every variable at least")?;
    let (matrix, scale, constant) = if standardise {
        let (scale, corr, constant) = m.standardising();
        (corr, scale, constant)
    } else {
        (m.cov.clone(), vec![1.0; d], Vec::new())
    };
    let (values, vectors) = eigen(&matrix, d);
    // (rounding can leave a null eigenvalue a little below zero)
    let variance: Vec<f64> = values.iter().map(|v| v.max(0.0)).collect();
    let total: f64 = variance.iter().sum();
    if total.is_nan() || total <= 0.0 {
        return Err("the variables have no variance".into());
    }
    let explained: Vec<f64> = variance.iter().map(|v| v / total).collect();
    let k = kept(&explained, keep)?;
    let components = vectors[..k]
        .iter()
        .map(|v| {
            let sign = sign_of(v);
            v.iter().map(|x| x * sign).collect()
        })
        .collect();
    Ok((Pca { predictors: names, mean: m.mean, scale, components, variance, explained }, constant))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two correlated variables and a third of its own, 6 records.
    fn data() -> Vec<Vec<f64>> {
        vec![
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
            vec![2.1, 3.9, 6.2, 7.8, 10.1, 12.0],
            vec![5.0, 1.0, 4.0, 2.0, 6.0, 3.0],
        ]
    }

    fn names() -> Vec<String> {
        ["a", "b", "c"].map(String::from).to_vec()
    }

    #[test]
    fn scores_are_centred_and_the_variance_is_the_eigenvalue() {
        let cols = data();
        let p = fit(names(), &cols, true, Keep::Count(3)).unwrap();
        assert_eq!(p.k(), 3);
        assert!((p.explained.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        let slices: Vec<&[f64]> = cols.iter().map(Vec::as_slice).collect();
        let s = p.scores(&slices).unwrap();
        for (c, col) in s.iter().enumerate() {
            let mean = col.iter().sum::<f64>() / 6.0;
            let var = col.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / 5.0;
            assert!(mean.abs() < 1e-12, "{mean}");
            assert!((var - p.variance[c]).abs() < 1e-10, "{var} {}", p.variance[c]);
        }
        // each component's largest loading is positive
        for c in &p.components {
            assert!(sign_of(c) > 0.0);
        }
    }

    #[test]
    fn a_share_of_the_variance_picks_the_components() {
        let p = fit(names(), &data(), true, Keep::Share(0.5)).unwrap();
        assert_eq!(p.k(), 1);
        let all = fit(names(), &data(), true, Keep::Share(1.0)).unwrap();
        assert_eq!(all.k(), 3);
        let two = fit(names(), &data(), true, Keep::Share(p.explained[0] + 1e-6)).unwrap();
        assert_eq!(two.k(), 2);
    }

    #[test]
    fn bad_fits_are_told() {
        let mut cols = data();
        assert!(fit(names(), &cols, true, Keep::Count(4)).unwrap_err().contains("1 to 3"));
        assert!(fit(names(), &cols, true, Keep::Count(0)).is_err());
        cols[2] = vec![1.0; 6];
        // standardised, a constant variable is centred and not divided: scale 1, no weight
        let (p, constant) = fit_noting(names(), &cols, true, Keep::Count(2)).unwrap();
        assert_eq!((constant, p.scale[2]), (vec![2], 1.0));
        assert!(p.components.iter().all(|c| c[2] == 0.0));
        let two = fit(names()[..2].to_vec(), &cols[..2], true, Keep::Count(2)).unwrap();
        assert_eq!(p.variance[..2], two.variance[..]);
        assert_eq!(p.variance[2], 0.0);
        // by covariance a constant variable is only a null direction
        assert!(fit(names(), &cols, false, Keep::Share(0.9)).is_ok());
        assert!(fit(names(), &[vec![1.0], vec![2.0], vec![3.0]], false, Keep::Count(1)).is_err());
    }

    #[test]
    fn nan_scores_nan() {
        let p = fit(names(), &data(), true, Keep::Count(2)).unwrap();
        let (a, b, c) = ([1.0, f64::NAN], [2.0, 3.0], [3.0, 4.0]);
        let s = p.scores(&[&a, &b, &c]).unwrap();
        assert!(s.iter().all(|col| col[0].is_finite() && col[1].is_nan()));
        assert!(p.scores(&[&a, &b]).is_err());
    }
}
