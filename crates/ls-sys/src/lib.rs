//! Landschaft's C++ kernels of land change simulation, for Terraflux.
//!
//! [`ffi`] is the cxx bridge to the shim (`shim/shim.cc`) over landschaft's
//! `simcore`, `lsmcore`, `nlmcore` and `gtbcore` libraries; the functions of this crate
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

    struct SimilarityOut {
        first: Vec<f32>,
        second: Vec<f32>,
        first_mean: f64,
        second_mean: f64,
    }

    struct CarsOut {
        landscape: Vec<i32>,
        iterations: i64,
        converged: bool,
        decays: i64,
        /// the cells of each region x class after the last iteration
        last: Vec<i64>,
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

        fn set_threads(n: i32);
        fn max_threads() -> i32;
        fn rrng_new(seed: i32) -> UniquePtr<RRng>;
        fn rrng_unif_rand(rng: Pin<&mut RRng>) -> f64;
        fn rrng_runif(rng: Pin<&mut RRng>, n: usize) -> Vec<f64>;

        fn map_classes(values: &[f64]) -> Vec<f64>;
        fn crosstab(a: &[f64], b: &[f64], classes_a: &[f64], classes_b: &[f64]) -> Result<Vec<i64>>;
        fn r_sum(x: &[f64]) -> f64;
        fn focal_mean(x: &[f64], ny: i64, nx: i64, w: &[f64], wy: i32, wx: i32) -> Result<Vec<f64>>;
        fn distance_exact(origin: &[u8], ny: i64, nx: i64, res: f64) -> Result<Vec<f64>>;
        fn nlm_random_cluster(seed: i32, ny: i64, nx: i64, p: f64, ai: &[f64], neighbourhood: i32) -> Result<Vec<f64>>;
        fn nlm_gaussian_field(
            seed: i32,
            ny: i64,
            nx: i64,
            autocorr_range: f64,
            mag_var: f64,
            nug: f64,
            mean: f64,
        ) -> Result<Vec<f64>>;
        fn nlm_fbm(seed: i32, ny: i64, nx: i64, fract_dim: f64) -> Result<Vec<f64>>;
        fn mspa(input: &[u8], ny: i64, nx: i64, connectivity: i32, edge_width: i32, internal: bool) -> Result<Vec<u8>>;

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
        fn cars(
            ny: i64,
            nx: i64,
            k: i32,
            landscape: &[i32],
            fixed: &[u8],
            region: &[i32],
            nreg: i32,
            probability: &[f64],
            weights: &[f64],
            seeds: &[f64],
            allowed: &[u8],
            demand: &[i64],
            window: i32,
            decay: f64,
            step: f64,
            max_iter: i64,
            seed: u64,
        ) -> Result<CarsOut>;
        fn similarity(a: &[i32], b: &[i32], ny: i64, nx: i64, window: i32, exponential: bool) -> Result<SimilarityOut>;
        fn observed_patches(a: &[f64], b: &[f64], ny: i64, nx: i64, from: f64, to: f64) -> Result<ObservedPatches>;
    }
}

pub use ffi::{
    CarsOut, CluesOut, GlmFit, NbRule, ObservedPatches, OrderedOut, PatchOut, PatchSpec, SimilarityOut, WoeWeights,
};

/// The threads Terraflux uses: never every core of the machine (all but one by default).
pub mod threads {
    use std::sync::Once;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static INIT: Once = Once::new();
    static THREADS: AtomicUsize = AtomicUsize::new(0);

    /// The cores of the machine.
    pub fn cores() -> usize {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
    }

    /// The most threads Terraflux may use: all the cores but one (one on a machine of one).
    pub fn most() -> usize {
        cores().saturating_sub(1).max(1)
    }

    /// The threads by default: `TERRAFLUX_THREADS`, else all the cores but one; never more than [`most`].
    pub fn default_threads() -> usize {
        std::env::var("TERRAFLUX_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or_else(most).clamp(1, most())
    }

    /// Set the threads once for the process (later calls do nothing): the kernels' OpenMP threads and the
    /// pool of parallel tasks, whose threads run the kernels with one thread each (no more threads than
    /// asked in all). Returns the threads in use.
    pub fn init(n: Option<usize>) -> usize {
        INIT.call_once(|| {
            let n = n.unwrap_or_else(default_threads).clamp(1, most());
            // SAFETY: before any other thread of ours runs
            unsafe { std::env::set_var("OMP_NUM_THREADS", n.to_string()) };
            crate::ffi::set_threads(n as i32);
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .start_handler(|_| crate::ffi::set_threads(1))
                .build_global();
            THREADS.store(n, Ordering::Relaxed);
        });
        THREADS.load(Ordering::Relaxed)
    }

    /// The threads in use (setting them by default first).
    pub fn ensure() -> usize {
        init(None)
    }

    /// The kernels' threads for the calling thread (a worker sharing the machine with others).
    pub fn set_kernel_threads(n: usize) {
        crate::ffi::set_threads(n.clamp(1, most()) as i32);
    }
}

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
