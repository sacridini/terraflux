//! Landschaft's C++ kernels of land change simulation, for Terraflux.
//!
//! [`ffi`] is the cxx bridge to the shim (`shim/shim.cc`) over landschaft's
//! `simcore`, `lsmcore` and `nlmcore` libraries; the functions of this crate
//! wrap it with Rust types. Maps are row-major `f64` cells with NaN for
//! missing ones; matrices of cells x classes are row-major (cell `i`, class
//! `j` at `i * ncode + j`).
//!
//! The kernels are the same code that landschaft's Python package runs, so
//! a model run here gives the same bits as `ls.simulate` with the same seed.

#[allow(clippy::too_many_arguments)] // the kernels take their settings one by one
#[cxx::bridge(namespace = "tfx")]
pub mod ffi {
    struct CluesOut {
        allocation: Vec<f64>,
        iterations: i32,
        converged: bool,
        max_diff: f64,
        mean_diff: f64,
    }

    struct OrderedOut {
        allocation: Vec<f64>,
        iterations: i32,
        converged: bool,
        short_cells: i64,
    }

    struct NbRule {
        cls: f64,
        threshold: f64,
        wy: i32,
        wx: i32,
        weights: Vec<f64>,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct GlmFit {
        coefficients: Vec<f64>,
        pivot: Vec<i32>,
        rank: i32,
        iterations: i32,
        converged: bool,
        boundary: bool,
        deviance: f64,
        null_deviance: f64,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct WoeWeights {
        plus: f64,
        contrast: f64,
        significant: bool,
    }

    struct PatchSpec {
        from: i32,
        to: i32,
        probability: Vec<f64>,
        expand: i64,
        patch: i64,
        e_mean: f64,
        e_var: f64,
        e_iso: f64,
        p_mean: f64,
        p_var: f64,
        p_iso: f64,
    }

    struct PatchOut {
        landscape: Vec<i32>,
        expanded: Vec<i64>,
        patched: Vec<i64>,
        patches: Vec<i64>,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct ObservedPatches {
        cells: i64,
        expansion: f64,
        expansion_patches: i64,
        expansion_mean: f64,
        expansion_variance: f64,
        new_patches: i64,
        new_mean: f64,
        new_variance: f64,
    }

    unsafe extern "C++" {
        include!("terraflux-ls-sys/shim/shim.h");

        /// R's random number generator (Mersenne-Twister, set.seed), as lulcc and landschaft draw from it.
        #[namespace = "nlm"]
        type RRng;

        fn rrng_new(seed: i32) -> UniquePtr<RRng>;
        fn rrng_unif_rand(rng: Pin<&mut RRng>) -> f64;
        fn rrng_runif(rng: Pin<&mut RRng>, n: usize) -> Vec<f64>;

        fn map_classes(values: &[f64]) -> Vec<f64>;
        fn crosstab(a: &[f64], b: &[f64], classes_a: &[f64], classes_b: &[f64]) -> Result<Vec<i64>>;
        fn r_sum(x: &[f64]) -> f64;
        fn focal_mean(x: &[f64], ny: i64, nx: i64, w: &[f64], wy: i32, wx: i32) -> Result<Vec<f64>>;
        fn distance_exact(origin: &[u8], ny: i64, nx: i64, res: f64) -> Result<Vec<f64>>;

        fn allow_matrix(x: &[f64], hist: &[i64], classes: &[f64], cd: &[f64], rules: &[i64]) -> Result<Vec<f64>>;
        fn auto_convert(x: &[f64], prob: &[f64], classes: &[f64], mask: &[i64]) -> Result<Vec<f64>>;
        fn neighbourhood_allow(
            landscape: &[f64],
            ny: i64,
            nx: i64,
            classes: &[f64],
            rules: &[NbRule],
        ) -> Result<Vec<f64>>;

        fn clues(
            tprob: &[f64],
            landuse: &[f64],
            demand: &[f64],
            codes: &[f64],
            jitter: f64,
            scale: f64,
            max_iter: f64,
            max_diff: f64,
            mean_diff: f64,
            rng: Pin<&mut RRng>,
        ) -> Result<CluesOut>;
        fn ordered(
            tprob: &[f64],
            x: &[f64],
            demand: &[f64],
            classes: &[f64],
            order: &[f64],
            stochastic: bool,
            rng: Pin<&mut RRng>,
        ) -> Result<OrderedOut>;

        fn approx_extrap(x: &[f64], y: &[f64], xout: &[f64]) -> Result<Vec<f64>>;
        fn round_sum(x: &[f64], nrow: i64, ncol: i64, ncell: i64) -> Result<Vec<f64>>;
        fn markov_from_counts(counts: &[f64], k: i32, kept: &mut Vec<i32>) -> Result<Vec<f64>>;
        fn markov_root(p: &[f64], k: i32, n: i32) -> Result<Vec<f64>>;
        fn markov_project(areas: &[f64], p: &[f64], k: i32, steps: i32) -> Result<Vec<f64>>;

        fn woe_buckets(
            var: &[f64],
            categorical: bool,
            classes: &[f64],
            start: f64,
            inc: f64,
            n_buckets: i64,
        ) -> Result<Vec<i32>>;
        fn woe_counts(
            t0: &[f64],
            t1: &[f64],
            buckets: &[i32],
            n_buckets: i64,
            from: f64,
            final_classes: &[f64],
        ) -> Result<Vec<i64>>;
        fn woe_probability(
            landscape: &[f64],
            from: f64,
            buckets: &[i32],
            weights: &[f64],
            offsets: &[i64],
        ) -> Result<Vec<f64>>;
        fn woe_weights(nb: f64, b: f64, n: f64, a: f64, fix: bool) -> WoeWeights;
        fn woe_generalise(
            possible: &[i64],
            executed: &[i64],
            min_delta: f64,
            max_delta: f64,
            tolerance: f64,
            fix: bool,
        ) -> Result<Vec<i32>>;
        fn woe_normalise(sums: &mut [f64], k: i32, n: i64) -> Result<()>;

        fn glm_logit(x: &[f64], y: &[f64], n: i32, p: i32, maxit: i32, epsilon: f64) -> Result<GlmFit>;
        fn glm_predict(x: &[f64], n: i64, p: i32, fit: &GlmFit) -> Result<Vec<f64>>;

        fn allocate_patches(
            ny: i64,
            nx: i64,
            landscape: &[i32],
            specs: &[PatchSpec],
            prune_factor: f64,
            win_rows: i32,
            win_cols: i32,
            seed: u64,
        ) -> Result<PatchOut>;
        fn observed_patches(a: &[f64], b: &[f64], ny: i64, nx: i64, from: f64, to: f64) -> Result<ObservedPatches>;
    }
}

pub use ffi::{CluesOut, GlmFit, NbRule, ObservedPatches, OrderedOut, PatchOut, PatchSpec, WoeWeights};

/// R's generator after `set.seed(seed)`.
pub struct RRng(cxx::UniquePtr<ffi::RRng>);

impl RRng {
    pub fn new(seed: i32) -> Self {
        RRng(ffi::rrng_new(seed))
    }

    /// `unif_rand()`: a number in (0, 1).
    pub fn unif_rand(&mut self) -> f64 {
        ffi::rrng_unif_rand(self.0.pin_mut())
    }

    /// `runif(n)`.
    pub fn runif(&mut self, n: usize) -> Vec<f64> {
        ffi::rrng_runif(self.0.pin_mut(), n)
    }

    pub fn pin_mut(&mut self) -> std::pin::Pin<&mut ffi::RRng> {
        self.0.pin_mut()
    }
}

// the generator is used by one thread at a time
unsafe impl Send for RRng {}

/// An error raised by a kernel (its message).
pub type Error = cxx::Exception;
