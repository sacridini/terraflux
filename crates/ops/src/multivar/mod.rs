//! The multivariate methods shared by the operators: the linear algebra (the singular value decomposition, the
//! covariance of a stack in one pass, the eigenvectors of a few bands) and k-means with Ward's clustering. The
//! tables of `metrics.signatures` (`metrics.pca`, `metrics.cluster`) and the operators on maps and stacks call them.

pub mod kmeans;
pub mod linalg;
