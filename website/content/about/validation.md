---
title: Validation and benchmarks
short: Validation
description: How the results of Terraflux's methods are checked against the tools they reproduce, how many tests there are, how much faster they run, and how to repeat it all.
order: 30
---

Terraflux makes two promises: **the same results as the established tools**, and **much less waiting for them**. This
page shows how both are checked, with the numbers.

!!! note "Whose numbers these are"
    The comparisons, the test counts and the timings below are those of **the Python library**
    ([terraflux-py](https://github.com/sacridini/terraflux-py), `import terraflux as tf`, version {{pyversion}}),
    which holds the methods and their test suite; the machines and the versions of every original tool are stated
    with each number, and none was measured again for this page. The application's own checks are in
    [Terraflux, the application](#terraflux-the-application).

- **Same results.** The tools are compared with the programs they reproduce: pixel for pixel, value for value, or to
  a stated tolerance where the original itself works in floating point. The exceptions are listed under
  [known differences](#known-differences).
- **Faster.** C++ with OpenMP (and Eigen for the circuit solver): from a few times to hundreds of times faster than
  the originals.
- **Tested on every change.** The comparisons live in the test suite: stored outputs of the originals, and live runs
  when the original is installed.

## The test suite

As of terraflux {{pyversion}}, the library's test suite has **171 test files with 1,665 test functions**, which pytest
collects as **4,678 tests** once the parametrised cases are counted one by one; 81 of them carry the
`live_reference` marker (they run the original tool, so they need it installed). They were counted with:

```bash
git grep -h "^def test_\|^    def test_" -- tests | wc -l          # test functions: 1665
pytest -p no:xdist --collect-only -q tests | tail -1              # collected tests: 4678 tests collected
pytest -p no:xdist --collect-only -q tests -m live_reference | tail -1   # 81 of them need an original tool
```

!!! note "Older figures"
    The library's old guide said "324 test cases". That figure is out of date; the figures above replace it. The numbers of cases inside the tables below (for example "53 reference
    cases" for gdistance) are the cases of one comparison, not of the whole suite.

## Speed

![Speed-up over the original tools](/img/about/bench_speedup.png "How much faster the library is than the tool it reproduces, on the cases of the tables below, logarithmic scale.")

How much faster Terraflux is than the tool it reproduces, on the cases in the
tables below (logarithmic scale). The original's time is its own run,
including its input and output where noted; Terraflux is timed in memory.

!!! info "How these were measured"
    The MSPA table was measured with [`tools/benchmark.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark.py) (version 0.53), and
    the spatial prioritization table with [`tools/benchmark_prioritize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_prioritize.py), on a
    desktop Intel Core i5-13600K (14 cores, 20 threads, 64 GB, Windows 11),
    the fastest of 10 alternating runs, with both programs kept on the
    performance cores. The other timings were recorded in the
    [changelog](/python/changelog/) when each tool was written, with the
    version given ("Since"). Times depend on the computer, the map and the options: treat them as
    orders of magnitude.

### Habitat patterns (MSPA)

Against GuidosToolbox's `mspa` executable (v2.3, whose time includes reading
and writing its TIFF files) on its sample map and on the CORINE forest map;
Terraflux with 1 thread and with the default 19. Every output was compared
pixel by pixel. Terraflux is 10 to 17 times faster.

| Map | Connectivity, edge width | GuidosToolbox | 1 thread | 19 threads | Identical |
|---|---|---|---|---|---|
| Sample, 1000² | 8, 1 | 0.46 s | 0.040 s | 0.029 s | yes |
| Sample, 1000² | 8, 5 | 0.56 s | 0.065 s | 0.054 s | yes |
| Sample, 1000² | 4, 20 | 0.68 s | 0.079 s | 0.068 s | yes |
| CORINE, 2000² | 8, 1 | 1.30 s | 0.095 s | 0.078 s | yes |
| CORINE, 2000² | 8, 5 | 1.53 s | 0.151 s | 0.137 s | yes |
| CORINE, 2000² | 4, 20 | 1.70 s | 0.213 s | 0.174 s | yes |

At scale, the CORINE map repeated 3 × 3 (6000 × 6000, 36 million pixels)
takes GuidosToolbox 20 to 26 s and Terraflux 1.4 to 2.3 s, again with
identical outputs.

MSPA's longest steps (geodesic distances, thinning, watershed) are ordered
propagations that must run in sequence to stay bit-exact, so one map takes
only 10 to 30 % less time with more threads. Many maps gain fully: run them side by side with
[`mspa_batch`](/python/api/batch/#terraflux.mspa_batch) or
[`batch`](/python/api/batch/#terraflux.batch).

!!! note "Hybrid processors"
    On processors with performance and efficiency cores (Intel 12th
    generation and later), Windows may move a busy process to the slower
    efficiency cores, especially when it runs in the background; any program
    then takes up to twice as long. Keep the terminal in the foreground for
    timings, and compare runs made side by side.

### Landscape metrics

Against the R package landscapemetrics 2.2.2.

| Task | landscapemetrics (R) | Terraflux | Since |
|---|---|---|---|
| All 133 metrics of `augusta_nlcd` (440 × 678) | 15.6 s | 0.03 s | 0.14 |
| `sample_lsm`, 6 plots of `augusta_nlcd` | 3.7 s | 0.02 s | 0.15 |
| Nearest-neighbour distances (`enn`) of `augusta_nlcd` | 2.4 s | 0.014 s | 0.14 |
| `get_nearestneighbour`, 1000 × 1000 | 1.8 s | 0.05 s | 0.18 |

At scale: a 10,000 × 10,000 map with 7.7 million patches gets all 133 metrics
(92 million values) in about 10 s, and maps larger than memory are read in
blocks with the same results.

### Habitat networks

Against the R package Makurhini (and, through it, gdistance).

| Task | Makurhini (R) | Terraflux | Since |
|---|---|---|---|
| IIC and node importances, 404 patches (`MK_dPCIIC`) | 1.4 s | 0.005 s | 0.30 |
| PC and node importances, 404 patches (`MK_dPCIIC`) | 10.6 s | 0.017 s | 0.30 |
| Least-cost distances, 398 patches over 222 k cells | 10.2 s | 0.5 s | 0.32 |
| Commute-time distances | | 13 × faster | 0.39 |

At scale: the 35 million centroid distances between 8,395 patches of a
4000 × 4000 map take 0.9 s, and all 4.3 million links of a 5,000-patch PC
graph are rated in 22 s.

### Circuit connectivity

Against Omniscape 0.7 and Circuitscape 6.1 (Julia), both with 20 threads.

| Task | Original | Terraflux | Since |
|---|---|---|---|
| Omniscape, 800 × 800, radius 10 | 29.4 s | 0.64 s | 0.43 |
| Omniscape, 700 × 700, radius 25 | 46.7 s | 1.36 s | 0.43 |
| Omniscape, 1000 × 1000, radius 100 | 275 s | 12.4 s | 0.43 |
| Circuitscape pairwise, 1000 × 1000, 20 focal points | 8.7 s | 2.7 s | 0.48 |
| … with the cumulative current map | 76.5 s | 9.5 s | 0.48 |
| Circuitscape one-to-all with maps, same map | 21.8 s | 4.6 s | 0.48 |
| Circuitscape network pairwise, 1 M nodes, 20 focal nodes | 27.7 s (CHOLMOD), 182 s (default cg+amg) | 5.0 s | 0.51 |

Every window and every circuit is solved exactly (a sparse Cholesky
factorisation), and the result does not depend on the number of threads.

### Grid distances

Against the R package gdistance 1.6.5 on a 300 × 300 grid of random
resistances (8 directions; best of 3 runs in R, of 5 in Terraflux).

| Task | gdistance (R) | Terraflux | Since |
|---|---|---|---|
| Transition (`1 / mean`) and geographic correction | 2.0 s | 0.005 s | 0.75 |
| Cost surface from one point (`accCost`) | 0.10 s | 0.011 s | 0.76 |
| Least-cost distances between 20 points | 0.31 s | 0.026 s | 0.76 |
| Least-cost paths to 5 goals | 0.05 s | 0.012 s | 0.76 |
| Commute distances between 10 points | 0.29 s | 0.084 s | 0.77 |
| Random-walk passages | 0.36 s | 0.066 s | 0.77 |
| Randomised-shortest-path passages (`theta = 0.5`) | 5.98 s | 0.078 s | 0.80 |
| Randomised-shortest-path distances, 3 × 3 points | fails (a 60 GiB dense matrix) | 0.15 s | 0.80 |
| … the same on 60 × 60 | 0.23 s | 0.0043 s | 0.80 |
| Route overlap from a common origin, 10 locations | 5.97 s | 0.16 s | 0.79 |

At scale: on 2000 × 2000 (4 million cells) a transition takes 0.15 s, a cost
surface 0.6 s, a random-walk passage map 4.5 s, a randomised-shortest-path
passage map 4.6 s (7.8 s for an asymmetric transition).

### Neutral landscape models

The same landscapes as NLMR (same seeds, identical values), one model call each
(best of 5, default threads).

| Task | NLMR (R) | Terraflux | Since |
|---|---|---|---|
| `nlm_random`, 2000 × 2000 | 0.25 s | 0.044 s | 0.54 |
| `nlm_percolation`, 1000 × 1000 | 1.54 s | 0.014 s | 0.54 |
| `nlm_planargradient`, 2000 × 2000 | 0.25 s | 0.017 s | 0.54 |
| `nlm_edgegradient`, 2000 × 2000 | 0.31 s | 0.020 s | 0.54 |
| `nlm_distancegradient`, 2000 × 2000 | 0.25 s | 0.10 s | 0.54 |
| `nlm_distancegradient`, 360 × 90 (great-circle distances) | 0.06 s | 0.011 s | 0.54 |
| `landscape_classify` by proportions, 1000 × 1000 | 0.05 s | 0.019 s | 0.54 |
| `nlm_mpd`, 1025 × 1025 | 0.13 s | 0.043 s | 0.55 |
| `nlm_neigh`, 500 × 500 | 0.04 s | 0.023 s | 0.55 |
| `nlm_randomrectangularcluster`, 300 × 300, sides 5–20 | 0.02 s | 0.003 s | 0.55 |
| `nlm_randomcluster`, 200 × 200 | 19 s | 0.003 s | 0.56 |
| `nlm_curds`, 512 × 512 (three steps) | 0.03 s | 0.003 s | 0.56 |
| Gaussian field (`simulate_gaussian_field`), 500 × 500 | 0.36 s | 0.043 s | 0.57 |
| Fractional Brownian motion (`simulate_fbm`), 1000 × 1000 | 0.40 s | 0.050 s | 0.57 |
| `nlm_mosaicfield`, 200 × 200, 20 mosaics | 9.95 s | 0.010 s | 0.58 |
| `nlm_mosaicfield`, 100 × 100, 20 mosaics and the limiting field | 9.0 s | 0.008 s | 0.58 |
| `nlm_mosaictess`, 500 × 500, 1000 germs | 0.25 s | 0.010 s | 0.59 |
| `nlm_mosaicgibbs`, 500 × 500, 500 germs | 0.50 s | 0.007 s | 0.59 |

### Surface metrics

Against geodiv 1.1.1 and spatialEco 2.0-5 (R 4.4.2, terra 1.9-50) on geodiv's
example rasters (371 × 371 cells: `normforest`, `orforest` for the trend,
`orelevation` for terrain), measured with [`tools/benchmark_surface.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_surface.py): the
best of 3 runs in R (fast tasks repeated to 0.5 s), of 5 in Terraflux with the
default threads. Where Terraflux corrects geodiv or spatialEco the work is the
same.

| Task | R | Terraflux | Since |
|---|---|---|---|
| Amplitude (sa, sq, ssk, sku) | 29 ms | 3.9 ms | 0.85 |
| Best trend surface of order 0-3 | 0.15 s | 25 ms | 0.85 |
| Slopes and developed area (sdq, sdq6, sdr) | 2.16 s | 4.8 ms | 0.85 |
| Peaks and valleys (sds, ssc, s10z) | 1.84 s | 1.7 ms | 0.85 |
| Bearing area parameters (sbi, sci, svi, sk, spk, svk) | 0.88 s | 11 ms | 0.85 |
| Correlation lengths and aspect ratio (scl, stxr) | 0.13 s | 28 ms | 0.85 |
| Texture direction and wavelength (std, srw) | 0.61 s | 28 ms | 0.85 |
| Fractal dimension (sfd) | 6.3 ms | 2.2 ms | 0.85 |
| Texture image, Sa in 5 × 5 windows, 100 × 100 (`focal_metrics`) | 0.17 s | 1.0 ms | 0.85 |
| Texture image, Sk in 5 × 5 windows, 30 × 30 (`focal_metrics`) | 134 s | 22 ms | 0.85 |
| TPI, 5 × 5 | 8.5 ms | 2.5 ms | 0.88 |
| TRI | 0.97 s | 1.9 ms | 0.88 |
| VRM | 80 ms | 15 ms | 0.86 |
| Curvature (total) | 1.58 s | 1.6 ms | 0.88 |
| Hierarchical slope position, scales 3-27 | 0.11 s | 14 ms | 0.88 |
| Spherical standard deviation | 73 ms | 12 ms | 0.86 |

The gains are largest where the R packages compute cell by cell in R (texture
images, peaks, curvature, TRI) and smallest where terra's focal functions do
the work in C++ with running sums (TPI, HSP, VRM). Terraflux's focal means,
sums and standard deviations use compensated prefix sums along the rows
(within about 1e-13 of summing each window), so a window costs one step per
row whatever its size.

### Terrain hydrology

Against WhiteboxTools 2.4 on a rough terrain of 3000 × 3000 cells, both on 4
cores ([`tools/benchmark_hydro.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_hydro.py), best of 3); WhiteboxTools' times include
reading and writing its GeoTIFFs, so Terraflux's include writing its result.

| Task | WhiteboxTools | Terraflux | Since |
|---|---|---|---|
| Filling depressions | 3.16 s | 2.07 s | 1.48 |
| Least-cost breaching (channels up to 20 cells) | 3.86 s | 2.35 s | 1.48 |
| D8 flow accumulation | 1.95 s | 0.62 s | 1.48 |
| D-infinity flow accumulation | 1.84 s | 1.22 s | 1.48 |
| FD8 flow accumulation | 2.00 s | 1.65 s | 1.48 |

### Spatial prioritization

Against Zonation 5 v2.4 (`z5.exe`, and `z5_16bit.exe` for 16 bits) on
synthetic species maps: seeded Gaussian random fields above a threshold, each
feature on 60 % of the cells (10 % in the 200-feature case), written as
GeoTIFFs. Measured with [`tools/benchmark_prioritize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_prioritize.py) (version 0.95) on the
performance cores, one run of z5 and the fastest of 3 of Terraflux; both
times include reading the GeoTIFFs, and z5's writing its outputs. Every rank
map is the same, bit for bit, as z5's on one thread.

| Map | Features | Rule | Iterations | z5, 1 thread | z5, 12 threads | Terraflux, 1 thread | Terraflux, 12 threads | Since |
|---|---|---|---|---|---|---|---|---|
| 1000 × 1000 | 30 | CAZ2 | 60 | 20.6 s | 12.4 s | 9.5 s | 1.86 s | 0.95 |
| 2000 × 2000 | 30 | CAZ2 | 60 | 102.8 s | 48.0 s | 45.6 s | 7.29 s | 0.95 |
| 2000 × 2000 | 30 | ABF | 20 | 53.6 s | 23.4 s | 36.5 s | 5.90 s | 0.95 |
| 2000 × 2000 | 30 | CAZ2, 16 bits | 60 | 104.4 s | 49.7 s | 46.2 s | 8.71 s | 0.95 |
| 1000 × 1000 | 200 sparse | CAZ2 | 59 | 105.8 s | 33.6 s | 15.0 s | 3.16 s | 0.95 |

On one thread Terraflux is 1.5 to 7 times faster than z5, most with many
sparse features: each feature is walked along its own occurrences, where z5
visits every feature in every cell. With 12 threads the gap grows to 4 to 11
times: z5 gains 1.7 to 3 times from its 12 threads, Terraflux 4.7 to 6.3. Memory: about 130 bytes per
cell with data during the ranking, plus 1 to 2 bytes per value that is not
zero; `storage=` puts the large arrays in memory-mapped files.

#### Optimized plans

Against prioritizr 8.0.6's models solved by HiGHS 1.15 alone (the solver
prioritizr calls) and by Gurobi 12, on synthetic problems of
[`tools/benchmark_optimize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_optimize.py): 30 species-like features with targets of 30 %
and costs from 1 to 10 (multi-action: 10 features, 3 threats), a gap of 1 %,
12 threads on the performance cores. "No answer" means no plan within the
time limit (120 s unless stated). The same models and the same optima as
prioritizr's; Terraflux's engines start HiGHS from near-optimal plans, fix
the units whose choice is proven, and solve the problems general solvers
find hard on their own.

| Problem | Units | HiGHS alone | Gurobi | Terraflux | Since |
|---|---|---|---|---|---|
| minimum set | 10⁵ | 7 s | 2 s | 0.44 s | 1.21 |
| minimum set | 10⁶ | no answer (its best plan 2.1 x the optimum) | 20 s | 1.7 s (gap 0.002 %) | 1.21 |
| minimum set | 10⁷ (5 features) | | | 60 s, proven optimal | 1.26 |
| boundary penalty | 10⁶ | no answer in 16 min | 289 s | 27 s (gap 0.77 %) | 1.21 |
| minimum penalties with boundary, gap 10 % | 2400 | more than 150 s | | 12 s | 1.27 |
| multi-action, least cost | 10⁵ | 57 s | 3 s | 0.7 s | 1.22 |
| multi-action, least cost, gap 0 | 10⁵ | no answer in 300 s | 13 s | 6 s | 1.22 |
| multi-action with boundaries | 10⁴ | | 7.5 s | 0.18 s | 1.22 |
| multi-action with river dependencies (soft / hard) | 10⁵ | | 6 s / 8 s | 1 s / 5.5 s | 1.22 |

Contiguity constraints are the exception. prioritizr's contiguity model is a
restriction (it misses some patches, see the
[migration guide](/tutorials/spatial-prioritization/from-prioritizr-in-python/#what-to-expect-when-you-compare-the-results));
Terraflux solves the exact model: a patch heuristic, a first patch of the
restricted model, directed cuts at the root, then HiGHS's branch and bound with
the same cuts separated at its nodes (HiGHS is patched for it). Measured with
[`tools/benchmark_connect.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_connect.py) (version 1.52; 120 s, gap 1 %, 4 threads, on a
Ryzen 7 7730U laptop), against Gurobi 13 on the same exact model, alone and
with Terraflux's cuts added at its nodes by a callback (best patch / proven
gap):

| Problem | Terraflux | Gurobi | Gurobi + the same cuts |
|---|---|---|---|
| prioritizr's contiguity test, raster (166 units) | 1642.83 / 2.4 % | 1646.49 / 2.1 % | 1643.33 / 0.9 % in 15 s |
| prioritizr's contiguity test, table (166 units) | 1496.98 / 1.7 % | 1496.98 / 1.5 % | 1499.86 / 0.9 % in 10 s |
| feature contiguity, raster (166 units) | 509.27 / 18.8 % | 509.27, optimal in 76 s | optimal in 62 s |
| 13 × 13 grid, 10 features | 111.30 / 8.7 % | 111.30 / 8.5 % | 111.30 / 5.0 % |
| Serra Verde at 3 km (543 units, 8 features) | 30 141 / 13.0 % | 30 679 / 13.1 % | 30 706 / 12.4 % |

Terraflux's patches are as cheap as or cheaper than Gurobi's, and its proven
gaps match Gurobi's own except with feature contiguity; Gurobi's branch and
bound does more with the same cuts. Before version 1.52, the same runs gave
Serra Verde a network of 33 981 with a gap of 24 % (in 60 s).

### Species distributions

Against the R packages each step reproduces (R 4.4.2, terra 1.9-50):
CoordinateCleaner 3.0.1, spThin 0.2.0, usdm 2.1-7, dismo 1.3-16, blockCV
3.2-0, predicts 0.1-19, ecospat 4.1.3 and maxnet 0.1.4 with glmnet 4.1-10.
The inputs are the example climate (9 bioclimatic layers, 192 × 186 cells of
0.5°), the *Bradypus* records with 10 000 background points, 5 000 synthetic
records over the same area and 2 000 clustered ones. Measured with
[`tools/benchmark_sdm.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_sdm.py) (version 1.0) on the performance cores: the best of
3 runs in R (fast tasks repeated to 0.5 s), of 5 in Terraflux with the
default threads. Every result is the same as R's, the random steps included
(same seed), except where the notes on accuracy below say otherwise.

| Task | R | Terraflux | Since |
|---|---|---|---|
| Clean 5 000 records: capitals, centroids, equal, GBIF, institutions, outliers, zeros (CoordinateCleaner) | 1.99 s | 0.16 s | 1.0 |
| Thin 2 000 records at 10 km, 10 repetitions (spThin) | 0.91 s | 1.4 ms | 1.0 |
| VIF selection, 9 layers (usdm `vifstep`) | 0.17 s | 16 ms | 1.0 |
| 10 000 background points (dismo `randomPoints`) | 0.43 s | 12 ms | 1.0 |
| Hexagonal blocks of 500 km, 10 116 points, 100 iterations (blockCV `cv_spatial`) | 1.77 s | 53 ms | 1.0 |
| k-means folds, 10 116 points, 25 starts (blockCV `cv_cluster`) | 93 ms | 14 ms | 1.0 |
| Autocorrelation range, 9 layers, 5 000 cells each (blockCV `cv_spatial_autocor`) | 17.5 s | 0.73 s | 1.0 |
| Evaluate 1 000 presences against 10 000 background points (predicts `pa_evaluate`) | 0.19 s | 8.3 ms | 1.0 |
| Boyce index, the same predictions (ecospat) | 9.8 ms | 1.6 ms | 1.0 |
| MESS, 9 layers, 10 116 reference points (predicts) | 0.75 s | 19 ms | 1.0 |
| MaxNet, 116 presences and 10 000 background points, 9 predictors (maxnet) | 3.25 s | 0.30 s | 1.0 |

The largest gains are where R works on whole matrices: spThin computes the
distance between every pair of records and rescans them after each removal
(Terraflux keeps only the neighbours, in a grid over the sphere, and the
counts in a tree: a million records take seconds, where R runs out of
memory), and blockCV's blocks and predicts' evaluation go through data frames
row by row. MaxNet keeps glmnet's arithmetic and order of operations, so the
gain comes from screening (most gradient checks are settled by a bound,
without the exact dot product) and from parallel work around the coordinate
descent, which itself stays serial; with 20 predictors and hinge features
(10 000 background points) it fits in 0.47 s against R's 6.4 s. Weighted
sampling without replacement, which R does in time proportional to the cells
times the draws, runs in closed form over runs of equal weights (1 million
cells and 30 000 draws: 0.24 s against 16-20 s with R's algorithm).

### Land change simulation

Against the land change tools `tf.simulate` reproduces, on the cases of its
reference tests. Measured with [`tools/benchmark_simulate.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_simulate.py) (version 1.20):
lulcc 1.0.4 in R 4.4.2 (with the corrections of [`tools/lulcc_fixed.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/lulcc_fixed.R), so
that both sides make the same allocation; only the timed call is measured),
MOLUSCE 5.3.0's algorithms in Python, Dinamica EGO 8.13's console (the time
of a script that only loads the maps is subtracted); the best of 3 runs, the
default threads. CLUMondo and FUTURES were timed when they were written.

| Task | Original | Terraflux | Since |
|---|---|---|---|
| CLUE-S allocation, Plum Island, 14 steps (lulcc `allocate`) | 36.4 s | 4.12 s | 1.8 |
| Ordered allocation, Plum Island, 14 steps (lulcc `allocate`) | 2.9 s | 0.40 s | 1.8 |
| Land systems, 400 × 500 cells, 8 years (CLUMondo) | 16.2 s | 1.7 s | 1.9 |
| Urban growth, 600 × 800 cells, 5 steps (`r.futures.simulation`) | 0.35 s | 0.20 s | 1.10 |
| Weights of evidence and 3 simulator iterations, 180 × 240 (MOLUSCE) | 1.99 s | 53 ms | 1.15 |
| Three-map comparison, agreement budget and figure of merit, Plum Island, 6 resolutions (lulcc) | 7.21 s | 0.17 s | 1.17 |
| Fuzzy similarity of the differences, windows 1-11, 480 × 640 (Dinamica EGO) | 0.67 s | 49 ms | 1.18 |
| Gravity potential, 1000 × 1000, 1000 sources (Dinamica EGO `CalcInteractionPotentialMap`) | 0.42 s | 0.13 s | 1.46 |
| Stock taken by patches, 3000 × 3000, 50 000 cells (Dinamica EGO `PatcherContinuousState`) | 3.8 s | 0.59 s | 1.46 |
| Cost surface to a road network, 3000 × 3000 (Dinamica EGO `CalcCostMap`, passes until exact) | 769 s | 1.4 s | 1.46 |

The Dinamica EGO functors of version 1.46 were timed with
[`tools/benchmark_dinamica.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_dinamica.py) on 4 cores on both sides (Dinamica with
`-processors 4`), the time of the functor in Dinamica's log. Its cost surface sweeps the map in four directions until no cost
changes (fewer passes give an approximation); Terraflux's runs Dijkstra's
algorithm once, with the same sums.

CLUE-S draws a random number per cell and class at each of its thousands of
passes, from R's generator, one after the other: Terraflux draws the same
numbers, so the gain is in the rest of the pass. The three-map comparison
runs lulcc's equations per block in one pass of C++ (R builds a raster per
class and equation), and MOLUSCE loops over the pixels in Python.

### Zonal statistics and extraction

Not a reproduction of another tool: the functions that read a raster in zones
and at points (`zonal_stats`, `extract`), against the Python packages commonly
used for it, rasterstats 0.21 and exactextract 0.3 (both single-threaded). A
5000 × 5000 float32 raster in memory; the statistics mean, min, max, standard
deviation and median; random polygons of 16 vertices. Measured with
[`tools/benchmark_zonal.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_zonal.py) (version 0.98) on the performance cores, the
fastest of 3 alternating runs.

| Case | rasterstats | exactextract | Terraflux, 1 thread | Terraflux, 12 threads | Since |
|---|---|---|---|---|---|
| zonal statistics, 2000 small polygons (16 million cells) | 2.34 s | 3.59 s | 0.37 s | 0.16 s | 0.98 |
| zonal statistics, 20 large polygons (22 million cells) | 1.14 s | 8.37 s | 0.37 s | 0.18 s | 0.98 |
| values at 1 million points | ~119 s | – | 0.028 s | 0.014 s | 0.98 |

The cells of every polygon are those GDAL burns (the centre rule, or
`all_touched`), as rasterstats takes them: the statistics agree with its own,
which adds up in single precision, to 1e-7, and with NumPy in double precision
to 1e-15. exactextract weights every cell by the share of it the polygon
covers, so its means differ slightly. The median and the percentiles take a
linear-time selection, not a sort; the result does not depend on the number of
threads.

### Plotting

`tf.plot`, from the data to the page a notebook or a window shows (the map's
HTML, images and data included), measured with [`tools/benchmark_plot.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_plot.py)
(version 1.32.0) on the performance cores, the fastest of 3 runs. A first plot
in a session also imports folium and compiles its templates, about 0.7 s once.
A page holds everything it needs (Leaflet, about 0.16 MB; plotly, 4.3 MB, in
a page with charts), so it opens without a network connection.

| Case | Time | Page |
|---|---|---|
| float32 raster, 10 000 × 10 000, in memory | 0.25 s | 4.7 MB |
| class raster, 10 000 × 10 000, in memory | 0.14 s | 1.6 MB |
| float32 raster, 10 000 × 10 000, read lazily from a file | 0.61 s | 4.7 MB |
| float32 raster, 2000 × 2000 | 0.23 s | 5.1 MB |
| stack of 47 bands of 1000 × 1000 (a band per thread, 16 million pixels in all) | 0.85 s | 14 MB |
| 10 000 polygons coloured by a column (GeoJSON) | 0.14 s | 2.7 MB |
| 50 000 polygons coloured by a column (WebGL) | 0.36 s | 1.5 MB |
| 200 000 polygons coloured by a column (WebGL, with their columns) | 0.99 s | 5.3 MB |
| 2 million points coloured by a column (WebGL) | 0.70 s | 13 MB |
| 50 000 polygons, static figure (`static=True`, drawn as a PNG) | 0.57 s | 0.7 MB |
| 200 000 polygons, static figure (burnt into an image) | 0.80 s | 0.6 MB |
| a table of landscape metrics (a chart) | 0.03 s | 4.3 MB |

The time hardly grows with the size of a raster: it is reduced to the
resolution of the screen first (block means in C++, or the cells of a lazy
raster read in strips), and only that is reprojected and encoded.
## Accuracy

### How results are compared

- **Stored references.** The outputs of the original tools on the example
  maps, on synthetic maps built to hit edge cases, and on the originals' own
  test cases are stored in `tests/data/reference`. Every run of the test
  suite compares with them.
- **Live runs.** When the original is installed, the tests also run it on
  freshly generated random maps and compare on the spot
  (`pytest -m live_reference`).
- **Original code, not a re-reading of it.** Where GuidosToolbox computes in
  IDL, its processing statements are copied verbatim from `guidostoolbox.pro`
  and run with [GDL](https://github.com/gnudatalanguage/gdl), the free IDL
  implementation, with GuidosToolbox's own `spatcon`, `grayspatcon`, `recode`
  and `ggeo` programs where it calls them.
- **Bits, then tolerances.** Integer maps and counts must be identical.
  Floating-point results are compared bit for bit where the original's
  arithmetic can be reproduced (landscapemetrics: R's 80-bit sums and x87
  `log`/`sqrt`), and otherwise to the tolerance given below, typically the
  precision of the original itself.

### Results by tool

| Tools | Compared with | Agreement |
|---|---|---|
| MSPA | GuidosToolbox `mspa` v2.3 executable: sample map with 7 parameter sets, 10 synthetic scenes, live random scenes; during development 1,200 random cases and CORINE with edge widths up to 100 | identical, 0 differing pixels |
| MSPA Tiling | GuidosToolbox's IDL code with its GDAL tools and `mspa`, in GDL | identical |
| SPA, Accounting | GuidosToolbox's IDL code in GDL | identical maps and statistics |
| FAD, FAD-MS, FOS, landscape mosaic, contagion and clustering | GuidosToolbox's IDL code and `spatcon` / `grayspatcon` / `recode` | identical maps; single-precision means and standard deviations to 1e-4 (Terraflux computes in double) |
| FAD-MS change, landscape mosaic heatmap and heatmap change | GuidosToolbox's IDL code in GDL | identical |
| Isochrone maps A and AB, least-cost path | GuidosToolbox's `ggeo` and IDL post-processing | identical |
| 133 landscape metrics and the utilities | landscapemetrics 2.2.2 (R): 14 maps (its example data, NA cells, non-square cells, degenerate layouts) × 5 option sets, live random maps | identical bits; `gyrate` and rare `enn` ties to a relative 1e-10; `circle` is randomised in landscapemetrics |
| FRAGSTATS extras, `convention="fragstats"`, spatial signatures | pylandstats, scikit-learn, FRAGSTATS' definitions | same values |
| Habitat graphs: indices, node and link importances, centrality | Makurhini (`MK_dPCIIC`, `MK_dPCIIC_links`, `MK_RMCentrality`) and Conefor Sensinode 2.2 / 2.7.1 | Makurhini: unrounded values to 1e-12, its rounded output exactly; Conefor: to its single precision (6 digits) |
| Nodes and distances (centroid, edge, least-cost, commute-time) | Makurhini's `nodesfile` / `distancefile` (gdistance) | identical bits; commute-time to 1e-11 |
| ProtConn, grids, ECA change, focal nodes | Makurhini (`MK_ProtConn`, `MK_Connect_grid`, `MK_dECA`, `MK_dECA_grid`, `MK_Focal_nodes`) | its rounded tables exactly; ECA change to 1e-13 |
| Settlement probabilities (SAMC) | samc and Makurhini's `MK_dPC_SAMC` | 1e-14 |
| Omniscape | Omniscape 0.7 with the CHOLMOD solver: its test configurations and synthetic maps | 1.7e-11 |
| Circuitscape raster modes | Circuitscape 6.1 with CHOLMOD: its own test cases (advanced, 17 pairwise, 25 one-to-all / all-to-one) | advanced: currents to 1.2e-13 of the largest; pairwise: resistances to 1e-12, maps to 3e-13; one-to-all: 5e-14 |
| Circuitscape network mode | Circuitscape's network test cases and six synthetic ones | 1.1e-14 |
| Grid distances: transitions (4/8/16 directions, symmetric or not, R's arithmetic, Mahalanobis), geographic correction (projected, longitude/latitude, global grids), normalisation, arithmetic, cost surfaces, least-cost distances and paths | gdistance 1.6.5 (R 4.4.2, raster 3.6-31, igraph 2.1.3): 53 reference cases, among them uniform grids where many paths cost the same | identical bits and the same paths |
| Grid distances: commute distances, random-walk and randomised-shortest-path passages and distances, route overlap, barrier and area transitions | the same gdistance: 15 reference cases | 1e-14 typical, 1e-10 worst (different sparse factorisations than R's Matrix package); barrier and area transitions identical |
| Surface metrics: amplitude parameters (`sa`, `sq`, `ssk`, `sku`, `sv`, `sph`, `smean`), trend surfaces of order 0-3 and the choice of the best one, slopes and developed area (`sdq`, `sdq6`, `sdr`), peaks, valleys, `sds`, `ssc`, `s10z`, the bearing area curve and its parameters (`sdc`, `sbi`, `svi`, `sci`, `sk`, `spk`, `svk`), autocorrelation, correlation lengths, texture direction and wavelength, fractal dimension, texture images | geodiv 1.1.1 (R 4.4.2, terra 1.9.50, spatial 7.3-17), and where geodiv errs geodiv with its errors corrected ([`tools/surface_fixed.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/surface_fixed.R)): 70 reference cases, projected and longitude/latitude, among them geodiv's example rasters, NA cells, non-square cells and ties | identical bits |
| Terrain indices: slope, aspect, tpi, tri, vrm, srr, dissection, curvature, spherical standard deviation, hsp, trasp, hli | spatialEco 2.0-5 and terra 1.9-50 (R 4.4.2), and where spatialEco errs spatialEco with its errors corrected ([`tools/surface_fixed.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/surface_fixed.R)): 3 DEMs (square and rectangular cells, missing cells, longitude/latitude) | identical bits; focal means (tpi, hsp, srr, vrm, spherical standard deviation, Bolstad's curvature) within 1e-12 of summing each window (compensated prefix sums), 1e-11 of terra's running sums; on longitude/latitude slope and aspect within 1e-12 |
| Terrain hydrology: least-cost breaching, D8 and D-infinity directions, D8, D-infinity and FD8 accumulations (cells, specific contributing area), HAND, wetness, stream power and sediment transport indices | WhiteboxTools 2.4: 4 DEMs (SRTM, missing cells, rectangular cells, terraced elevations with many flats and ties) | identical bits (the accumulations and indices to the float32 WhiteboxTools writes); filling and breaching identical to `FillDepressions` and `BreachDepressionsLeastCost` (on one thread) run with the map's edge as an outlet (WhiteboxTools raises depressions that touch the edge or missing cells, and leaves channels to the edge uncut) |
| Terrain hydrology: multiple flow routing (`"mfd"`), distance to channel, stream tracing, its D8 routing, pit filling | pygeoprocessing 2.4.11 (the routing of InVEST 3.20): the same 4 DEMs, plain and weighted | identical bits |
| Spatial prioritization: the ranking (8 rules, weights and weight groups, output groups, thresholds, zero modes, value extraction, costs, hierarchic masks, analysis areas, smoothing, 8 and 16 bits), condition, retention, pre-processing maps, transformed layers, connectivity transforms, post-processing, impact assessment | Zonation 5 v2.4 (`z5.exe`, `z5_16bit.exe`, on one thread): 64 reference projects, and the timing cases below | identical rank maps and iterations; curve, statistics, post-processing and impact files digit for digit |
| Optimized prioritization: every objective, target, penalty, constraint, decision and portfolio of prioritizr; raster, table and polygon planning units | prioritizr 8.0.6 (R 4.4.2; its errors corrected: phylogenetic tip order, portfolio cuts): `compile()` of 55 reference problems; the optimum with Gurobi | identical models, row by row and bit for bit; the same optima |
| Evaluation: summaries, representation, target coverage, Ferrier, rarity-weighted richness, replacement and rank importance; calibration | prioritizr 8.0.6 and its calibration vignette | identical bits (R's `pnorm` ported) |
| Multi-action planning | prioriactions 0.5.0 (its errors corrected): `getModelInfo` of 6 reference problems, costs, benefits, actions | identical models and figures; the same optima as Gurobi; dependencies and portfolios tested against exhaustive enumeration |
| Polygon planning units | exactextractr 0.10.0 (`exact_extract(fun = "sum")`) | identical cell fractions and sums |
| Marxan | Marxan 4.0.6 (its errors corrected), the same seeds | identical runs: solutions, scores, summaries |
| Species distributions: occurrence cleaning (all 13 tests, records on circle vertices, the antimeridian and poles), thinning, one record per cell | CoordinateCleaner 3.0.1 (its errors corrected: [`tools/cc_fixed.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/cc_fixed.R)), spThin 0.2.0, terra 1.9-50: 14 reference cases | identical flags and kept records; the circles' vertices identical (GeographicLib 2.7 and 1.52 with mingw's x87 maths) |
| Species distributions: predictor selection, background points, spatial blocks and clusters, ENMeval partitions | usdm 2.1-7, dismo 1.3-16, blockCV 3.2-0 (its own k-means: R's `kmeans`, 48 cases), ENMeval 2.0.5.2: 53 reference cases with R's generator | identical bits, random steps included |
| Species distributions: evaluation, Boyce index, omission rates, MESS, MOP | predicts 0.1-19, ecospat 4.1.3 (its errors corrected: [`tools/ecospat_fixed.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/ecospat_fixed.R)), ENMeval, mop 0.1.4: 16 reference cases | identical bits; the p-value of the correlation to 1e-9 (R's `pt`) |
| Species distributions: Mahalanobis distances (MOP, ExDet), autocorrelation range | mop, dsmextra 1.1.6 (its covariate index corrected), blockCV `cv_spatial_autocor` (automap, gstat 2.1-6) | 1e-9 (R inverts the covariance with LAPACK); variograms identical, fitted ranges to 1e-6 |
| Animal movement: Movebank files, steps (distances, time lags, speeds, azimuths, turn angles), duplicates, intervals, interpolation, track lines, local projection, Brownian tracks | move2 0.5.0 with sf, s2 and lwgeom (R 4.4.2; its datum code corrected: [`tools/move2_fixed.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/move2_fixed.R)): random tracks from millimetre to continental steps, across the antimeridian and near the poles, and move2's fishers | identical bits on Windows (R's libm reproduced); elsewhere within 1e-14 |
| Animal movement: resampling and bursts, steps, track summaries, time of day, fitted step-length and turn-angle distributions, random steps, covariates | amt 0.3.1 with fitdistrplus and circular (its errors corrected: [`tools/amt_fixed.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/amt_fixed.R)) | identical bits, R's random numbers included; elsewhere within 1e-14 |
| MaxNet | maxnet 0.1.4 with glmnet 4.1-10: 13 fits (Bradypus and synthetic, every feature class, regularisation multipliers, factors) | identical bits: coefficients, lambda, entropy and the four output types, on Windows and Linux |
| Neutral landscape models: random, percolation, gradients, midpoint displacement, fractional Brownian motion, Gaussian fields, Poisson line and Voronoi mosaics, random, neighbourhood and rectangular clusters, curds, classification | NLMR 1.4.0 (R 4.4.2) with the same seeds; R's generator and the libstdc++ distributions of NLMR's C++ code against R and the Rtools compiler | identical bits |
| Land change analysis: cross tables, intensity analysis, components of difference, multiple resolutions, Markov chains | lulcc 1.0.4, OpenLand 1.0.5, diffeR 0.0-8 (R 4.4.2; their errors corrected, [`tools/differ_fixed.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/differ_fixed.R)): 11 cases; Dinamica EGO 8 transition matrices: 6 cases | identical bits; Dinamica's matrices to 2e-11 (it writes 15 digits; its root of the matrix is less exact) |
| Land change: weights of evidence (ranges, weights, correlations, probabilities) | Dinamica EGO 8.13 (its code is closed: inferred from its outputs): 9 cases, 84 range probes | ranges and counts identical, weights to the 15 digits it writes |
| Land change: logistic regression, CLUE-S and ordered allocation, neighbourhoods, rules | R's `glm` and lulcc 1.0.4 (its errors corrected: [`tools/lulcc_fixed.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/lulcc_fixed.R)): 9 cases | identical bits, R's random numbers included |
| Land change: land systems (CLUMondo) | CLUMondo (GitHub a5c0c1f, built Win32 with MSVC; its age error corrected: [`tools/clumondo_fixed.patch`](https://github.com/sacridini/terraflux-py/blob/main/tools/clumondo_fixed.patch)): 6 cases; the May 2017 executable for its later options | identical maps and iteration logs; 97-100 % of the cells for the 2017 options (no source) |
| Land change: urban growth (FUTURES): simulation, flooding, demand, pressure, patch library and calibration | `r.futures` addons in GRASS 8.3.2 (their errors corrected: [`tools/futures_fixed.patch`](https://github.com/sacridini/terraflux-py/blob/main/tools/futures_fixed.patch)): 16 cases; the potential's mixed model against lme4 2.0.1 and MuMIn 1.48 | identical maps and tables; mixed-model coefficients to 2.6e-6 (an optimiser: lme4 stops at its tolerance) |
| Land change: MOLUSCE's weights of evidence, simulator, multi-criteria evaluation, kappa and error budget | MOLUSCE 5.3.0 imported without QGIS (its confidence corrected: [`tools/molusce_fixed.patch`](https://github.com/sacridini/terraflux-py/blob/main/tools/molusce_fixed.patch)): 6 cases | identical bits, NumPy's random numbers included |
| Land change validation: three maps at several resolutions, agreement budget, figure of merit, `r.futures.validation` | lulcc 1.0.4 (raster 3.6-31): 2 cases; GRASS's addon: 3 cases | identical bits |
| Land change validation: fuzzy similarity and similarity of differences | Dinamica EGO 8.13 (measured): 36 maps, windows 1-11, exponential and constant decay | identical maps; means to the 15 digits it writes |
| Land change: gravity potential (`surface.gravity`) | Dinamica EGO 8.13 `CalcInteractionPotentialMap` (measured): 10 maps, five exponents, with and without a mask | within 3e-6 (Dinamica adds float32 terms in float32; Terraflux sums in float64) |
| Land change: roads (`friction_cost`, `pathways`) | Dinamica EGO 8.13 `CalcCostMap` and `CalcPathwayMap` (measured): real and integer frictions, with and without diagonal costs, relative frictions, impassable and unreachable cells; paths down the costs and along the directions | identical costs and paths; directions identical except between equally cheap predecessors (Dinamica's follow its sweeps) |
| Watershed model (`hydro.swat.run`): weather and generator, HRUs (snow, ET, curve number runoff, MUSLE, percolation, lateral flow, nutrients, plant growth, scheduled management, decision tables, crop rotations, urban land), routing units, aquifers, SWAT-DEG channels, every printed table | SWAT+ 62.0.1 built with flang 22 against the UCRT (equal to the official Windows executable in every printed digit), dumping its state each day and its printed records: the official Hubbard Brook and Ames examples, Muskingum routing, two projects made by `from_rasters` (one with crop rotations and urban land) and the plants' minimum C (`c_factor="plant"`) | identical bits (every field of every day) |
| Watershed division and project files (`hydro.swat.watershed`, `from_tables`) | QSWAT+ 3 (its sources) and the SWAT+ Editor 4.0.2 run on the same tables: a synthetic watershed and a 282 km² DEM (QSWAT+'s test data) | the Editor's 64 files, token for token |

The default solver of Omniscape and Circuitscape (conjugate gradients with an
algebraic multigrid) is itself only about 2e-7 away from the exact solution;
Terraflux's solves are exact, so it is compared with their exact (CHOLMOD)
runs.

### GuidosToolbox: the reference programs and the cases

The methods of GuidosToolbox are compared with its own programs and code, as the table above says. The cases behind
each comparison, as the library's older guide listed them:

- **MSPA:** the `mspa` executable (`mspa_win64.exe`) on 1,200+ random configurations and on full CORINE datasets: 0
  differing pixels.
- **FAD and contagion:** the original JRC `spatcon64` program: the maps are bit-exact.
- **Landscape mosaic:** the JRC `recode64` program: bit-exact.
- **FAD-MS, FAD multiscale change, the landscape mosaic heatmap and its change:** GuidosToolbox's own IDL code run
  in GDL together with its `spatcon`: FAD-MS maps and 103-class maps identical, change tables identical, heatmap
  vectors identical bit for bit, CSV reports identical character for character.
- **MSPA Tiling:** GuidosToolbox's `mspatile` IDL code run in GDL with its gdal tools and `mspa`: 10 stored cases
  covering all the tiling layouts and tile shortcuts, plus live random cases; the maps are identical. GDL needs two
  shims for GuidosToolbox's file handling (`tests/gtb_idl/read_tiff.pro` and `updategeotagsinimage.pro`).
- **FOS** (FAD, FED and FAC, 5 and 6 classes, binary and grayscale): GuidosToolbox's batch FOS IDL code run in GDL
  with its `spatcon` and `grayspatcon`: 13 stored cases plus live random scenes; the maps are identical and the
  statistics are identical to IDL's float or double precision.
- **Accounting** (IDL-only in GuidosToolbox): its own IDL code (`tests/gtb_idl`, the statements copied verbatim from
  `guidostoolbox.pro`) run in GDL: maps and integer statistics identical. The stored digests
  (`tests/data/reference/idl_reference.json`) cover 7 maps with 5 parameter sets each, and random scenes run live
  when GDL is available.
- **GraySpatCon:** the two metrics that FOS uses (the mean cover and the clustering) are compared with the
  `grayspatcon` program, to float32 precision. The guide said that every metric was compared with
  `grayspatcon64.exe`; the tests in the repository check the others on constructed cases (see
  [checked in other ways](#checked-in-other-ways)), so that is what this page says.

### Checked in other ways

These tools follow GuidosToolbox's method, but are not compared with a run of
its code:

| Tools | How they are checked |
|---|---|
| Network components and importance of MSPA cores and bridges | an independent step-by-step reproduction of GuidosToolbox's IDL routine |
| Spatial entropy | an independent reproduction of GuidosToolbox's `entropy_mspainp.pro` |
| Restoration status, parcellation | GuidosToolbox's values for its sample map |
| Cost distance, least-cost path, restoration corridor, contortion, simple and morphological change, FOS change, GraySpatCon metrics other than those of FOS | unit tests on constructed cases |
| Zonal statistics, extraction at points, clip, rasterize | cells against GDAL's rasterizer, statistics against NumPy and rasterstats |
| Land change: Dinamica's Expander and Patcher | Dinamica EGO 8.13's functors crash for patches of more than one cell, so their seed selection was measured with one-cell patches (acceptance by neighbours, draw by probability) and the patch growth follows the publications; statistical comparison |
| Land change: Dinamica's continuous Patcher (`ContinuousPatcher`) and road destinations (`destinations`) | Dinamica EGO 8.13's continuous Patcher crashes for patches of more than one cell, as its Patcher does: the candidates, the pruning, the quota and the draw by probability were measured with one-cell patches (exact on the cases that do not depend on the draw, statistical on 32 draws), the growth is the Patcher's; destinations compared statistically on 100 draws (Dinamica's draw errors corrected, see below) |
| Hilltop protection areas (`surface.hilltop`) | written from the Forest Code (Lei 12.651/2012): Dinamica EGO's `CalcHilltop` follows the older CONAMA 303/2002 rule on a DEM cut into elevation bands, with its own definitions (its slope criterion measures to the map's edge or to other features: 5° on an isolated cone of 21.8°), so only the heights are compared (to 0.05 m on cones); checked against an independent computation of the landforms by connected components on random DEMs, and on cones, ranges and lakes of known geometry |
| Land change: PLUS's CA with multi-type random patch seeds (CARS) | PLUS is distributed without source code or licence: written from its paper, checked by its properties (demand met, growth from patches and seeds, the threshold, the rules, the same maps with any number of threads) |
| Land change: dynamic variables, regional demands, presets, projections, scenarios, calibration | unit tests: the pieces they assemble are compared above |

### Details by module

Where the old guide kept a verification section and a timing table per module, they are kept here. The wording is the
library's; the scripts and tests it names are in the [library's repository](https://github.com/sacridini/terraflux-py).

#### Landscape metrics in detail

[`tools/make_reference_lsm.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_lsm.py) runs landscapemetrics ([`tools/lsm_reference.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/lsm_reference.R)) on 14
maps (its `landscape`, crops of `augusta_nlcd` and `podlasie_ccilc`, synthetic maps
with NA cells, negative and large class values, non-square cells, salt-and-pepper
patterns, few distant patches with a non-integer cell size, a checkerboard, single
rows and columns, a single class) with 5 option sets, and stores every value in `tests/data/reference/lsm`. `tests/test_lsm_reference.py`
compares all of them; with `Rscript` available (`TERRAFLUX_RSCRIPT`,
`TERRAFLUX_R_LIBS` for a specific landscapemetrics library) it also runs
landscapemetrics live on random maps with other option combinations.
`make_reference_lsm.py sample window` also stores `sample_lsm` (square, circle and
polygon plots, with and without `all_classes`, a CRS and boundary options) and
`window_lsm` (square and circular windows) results, which `tests/test_lsm_sampling.py`
compares, plus live runs on random maps; sample plots and windows are also checked
against `Landscape` objects built on the same cells. `tests/test_lsm_tiled.py` checks
that out-of-core results (`block_rows`) are identical to the in-memory ones on every
stored reference map and on files with many blocks, patches across every seam, deep
`edge_depth` context, several data types and nodata values.
All comparisons with landscapemetrics require the same bits, except for the three
families above (relative 1e-10); `tests/test_lsm_rmath.py` checks the arithmetic itself
against R on 600 stored vectors and the `lm()` slope on 400 regressions, 166 of them
collinear (`make_reference_lsm.py rmath`).
`tests/test_lsm_extras.py` checks the extras against numpy and pylandstats, the names
and the threads. `make_reference_lsm.py utils` stores the results of the utilities
([`tools/lsm_utils_reference.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/lsm_utils_reference.R)) on 8 maps and option sets, compared by
`tests/test_lsm_utils.py`, which also runs them live on random maps. `tests/test_lsm.py` checks values computed by hand, the labels against SciPy, the
parallel seams, input handling and metric selection; `enn` against SciPy's exact
Euclidean distance transform and `circle` against an exhaustive search over the
circles through two or three hull points.

**Timings of the landscape metrics** (the first rows repeat the table of [Landscape metrics](#landscape-metrics)
above; the rest are at scale):

| map | landscapemetrics 2.2.2 | terraflux |
|---|---|---|
| `augusta_nlcd`, 440×678, the first 91 metrics | 8.6 s | 0.1 s |
| `augusta_nlcd`, all 133 metrics | 15.6 s | 0.03 s |
| `augusta_nlcd`, `enn` / `circle` | 2.4 s / 0.14 s | 0.014 s / 0.008 s |
| 10,000×10,000: preparing the map (class values, NA, per-class counts) | – | 0.07 s |
| 10,000×10,000, 10 classes, 7.7 M patches: labelling | – | 1.4 s |
| same, all class and landscape metrics | – | 2.2 s more |
| same, core areas (distance, core cells, core labelling) | – | 1.0 s more |
| same, `enn` / `circle` | – | 2.0 s / 0.5 s more |
| `augusta_nlcd`, 6 circular plots, all metrics (`sample_lsm`) | 3.7 s | 0.02 s |
| 4000×4000, 10,000 plots, all metrics (10 M values), squares / circles | – | 2.2 s / 2.2 s |
| 9×11 map, 5×5 window, 10 metrics (`window_lsm`) | ~4 s | 0.003 s |
| 1000×1000, 21×21 window, all 66 landscape metrics | – | 8.7 s |
| 20,000×20,000 file (400 M cells, 334 k patches), class and landscape metrics without core/gyrate/circle/enn: in memory / `block_rows="auto"` | – | 3.4 s, 3.7 GB / 3.9 s, 0.8 GB |
| same, all 133 metrics: in memory / `block_rows="auto"` | – | 8.6 s, 8.7 GB / 13.1 s, 3.3 GB |

#### Habitat graphs in detail

* [`tools/make_reference_conn.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_conn.py) runs Makurhini's `MK_dPCIIC` (Makurhini 3.0) on its
  example `habitat_nodes` (404 patches, centroid distances) and three synthetic
  scenes (random points, non-metric costs, separate clusters) with IIC and PC,
  several distances, probabilities, `pij_min` and landscape areas
  (`tests/data/reference/conn/makurhini.csv.gz`). `tests/test_conn_reference.py`
  compares the unrounded values of its internal steps to 1e-12 and reproduces its
  rounded output exactly.
* The same script builds Conefor Sensinode 2.2's own index code (`landscape.cpp`,
  with [`tools/conefor/conefor_harness.cpp`](https://github.com/sacridini/terraflux-py/blob/main/tools/conefor/conefor_harness.cpp) replaying its node-removal loop) with
  g++ and stores every index and node importance of the synthetic scenes
  (`conefor.csv.gz`). Conefor computes in single precision: the values agree to
  its precision (1e-5 relative for the indices).
* `make_reference_conn.py spatial` stores `nodesfile`, `distancefile` and `MK_dPCIIC`
  on Makurhini's example raster (`habitat_nodes_raster`, 398 patches) and on 53
  synthetic polygons (multipart, with holes, U-shaped, touching and overlapping) in
  `tests/data/reference/conn/spatial`. `tests/test_conn_spatial.py` finds the same
  bits for the node attributes and the centroid distances (rasters and polygons) and
  for the polygon edge distances; raster edge distances agree to the last bit
  (GEOS measures between terra's polygon vertices: 1 in 80,000 values differs by 1
  ulp). It also checks raster edge distances against brute force (non-square
  cells too) and GEOS on the cell polygons.
* Least-cost distances (`resistance_matrix`, 8 and 4 directions, `1/mean(x)`; the
  polygons over a resistance with NA walls, whose unreachable pairs take the centroid
  distance) are gdistance's bits, and `MK_dPCIIC` with them is reproduced exactly;
  `tests/test_conn_spatial.py` also checks them against SciPy's Dijkstra on the
  graph built from gdistance's formulas.
* `make_reference_conn.py commute` stores `distancefile(type = "commute-time")` on the
  polygons (8 and 4 directions, both cost functions) and on the example raster over
  `resistance_matrix`; `tests/test_conn_spatial.py` matches them to 1e-11 (the same
  pairs) and checks commute times against the Laplacian's pseudo-inverse.
* `make_reference_conn.py links` runs `MK_dPCIIC_links` (removal, and changes of 40
  pairs brought closer or moved apart) and `MK_dPCIIC` with `restoration` on the three
  synthetic scenes (`makurhini_links.csv.gz`): the same rounded values, restoration
  gains to 1e-12. The Conefor driver also replays its "adding nodes" mode (the last
  five nodes of each scene), matched to its precision for every index.
* `make_reference_conn.py deca` runs `MK_dECA` on three times of the polygons (PC and IIC,
  two distances) and of the example raster (`deca_steps.csv`, `makurhini_deca.csv.gz`):
  `dA` has the same bits and the other values agree to 1e-13.
* `make_reference_conn.py protconn` runs `MK_ProtConn` on 36 synthetic protected
  areas and two regions (without and with transboundary zones of both types, and
  ProtConn[Bound]) with its inner steps: fed Makurhini's own nodes and distances,
  `protconn_indicators` gives `get_protconn_grid`'s bits; terraflux's own nodes have
  the same areas (1e-14) and its rounded ProtConn table is Makurhini's, value for value.
* `make_reference_conn.py grids` stores `make_grid` (hexagons and squares: the same
  coordinates), `MK_Connect_grid` (ProtConn with its default and with `pij_min = 0`, and
  PC), `MK_ProtConn_raster` (three transboundary settings) and `MK_dECA_grid`; all are
  reproduced after Makurhini's rounding (the dECA grid to 1e-12).
* `make_reference_conn.py centrality` runs `MK_RMCentrality` (binary and weighted, with
  and without a probability) on the polygons and the example raster
  (`makurhini_centrality.csv.gz`): degree, BWC and cluster are the same, the other
  values agree to 1e-12 (eigenvectors: ARPACK against LAPACK).
* `make_reference_conn.py betweenness` runs the command line of Conefor 2.7.1
  (`CONEFOR_EXE`) for BC, BC(IIC) and BC(PC) on the synthetic scenes
  (`conefor_betweenness.csv.gz`), matched to the 6 digits it prints. Conefor keeps the
  probabilities in single precision: where a path is shorter than the direct link by
  less than float precision (near-collinear points, 1e-10 to 8e-8 relative) it counts
  the path; on the Euclidean scenes BC(PC) is 0 in exact arithmetic, and the tests
  check that every difference is such a tie.
* `make_reference_conn.py focal` runs `MK_Focal_nodes` (IIC and PC, one and two
  distances and search buffers, `pij_min`) on the polygons and the ProtConn areas
  (`makurhini_focal.csv.gz`): every neighbourhood is the same, the indices agree to
  1e-12 and the importances after Makurhini's 7-digit rounding. (Makurhini fails with a
  `threshold` there.)
* `make_reference_conn.py samc` runs `MK_dPC_SAMC` on the polygons over their resistance
  (with an absorption raster written from them), 4 and 8 directions, sum and mean,
  10 or 3 vertices or a fifth of them, and stores samc's settlement probabilities: they
  agree to 1e-14, and the PC importances are Makurhini's after its rounding.
* `tests/test_conn.py` checks every index and importance against a brute-force
  recomputation of the graph without each node (and with its attribute set to 0),
  under both conventions, and the input kinds, pair tables, Conefor files and
  thread independence.

**Timings of the habitat graphs:**

| graph | Makurhini 3.0 | terraflux |
|---|---|---|
| `habitat_nodes` (404 patches), 10 km, IIC / PC with node importance | 1.4 s / 10.6 s | 0.005 s / 0.017 s |
| 2,000 random patches, 9 km (47 k links; PC: 1.2 M pairs above `pij_min`), IIC / PC | – | 0.09 s / 1.7 s |
| 5,000 random patches, 6 km (134 k links; PC: 4.3 M pairs), IIC / PC | – | 0.8 s / 19 s |
| 4000×4000 raster, 8,395 patches: centroid distances, all 35 M pairs / up to 5 km | – | 0.9 s / 0.1 s |
| same, edge distances up to 5 km / 20 km / all pairs | – | 0.24 s / 1.1 s / 12.5 s |
| `habitat_nodes_raster` over `resistance_matrix` (222 k cells): least-cost distances | 10.2 s | 0.5 s |
| same: commute-time distances | 50 s | 3.7 s |
| 3000×3000 resistance, 300 nodes: least-cost distances, all pairs / up to 500 k cost units | – | 46 s / 1.5 s |
| 2,000 random patches: importance of every link, IIC (47 k links) / PC (1.2 M edges) | – | 0.1 s / 1.9 s |
| 5,000 random patches: importance of every link, IIC (134 k) / PC (4.3 M) | – | 0.9 s / 22 s |
| `habitat_nodes_raster` (398 patches), 20 km: centrality, binary / weighted (79 k edges) | 0.53 s / 1.1 s | 0.07 s / 0.14 s |

#### Circuit connectivity in detail

`python tools/make_reference_omniscape.py advanced` (Julia with Circuitscape 6.1, see the
script) runs `compute_omniscape_current` with the `cholmod` and the default `cg+amg`
solvers on synthetic rasters: NoData islands, finite and direct grounds, components
without a source or a ground, 4 and 8 neighbours, an Omniscape window and conductances
over six orders of magnitude (`tests/data/reference/circuit`). The same cells carry
current and the values agree with CHOLMOD to 1.2e-13 of the largest current; the
iterative `cg+amg` itself is 2e-7 away from both.

### Known differences

!!! warning "Where the results differ"
    - **Euclidean distance, distance to resistance, influence zones and
      proximity** depend on IDL's `MORPH_DISTANCE`, which does not always pick
      the nearest object near the boundaries between the zones of different
      objects (about 1 % of the pixels of the test cases). Terraflux's
      distances are exact Euclidean distances, so these maps can differ from
      GuidosToolbox's in detail; influence zones and proximity are marked
      experimental.
    - **MSPA Tiling** reproduces GuidosToolbox's tiling exactly, including its
      approximation near the tile seams when objects are larger than the
      overlap. A map that fits in one tile gets the MSPA of the whole map
      (GuidosToolbox drops the first `overlap / 2` columns).
    - **Zonation 5's curves** (performance curves, AUC, `features_info`,
      histograms and flexibility maps) come from the order of its
      second-to-last sort, while its rank map comes from the last one.
      Terraflux's describe the rank map it returns; they are compared with z5
      evaluating that rank map (`LOAD` mode). Flexibility maps are identical
      where the two orders give the same curves, and otherwise compared with a
      direct reproduction of Zonation's algorithm.
    - **Zonation 5 on several threads** adds up the layers per thread, and its
      results can differ from its run on one thread; Terraflux's do not
      depend on the threads and equal z5's on one thread.
    - **Species distributions** keep the R packages' results except where
      they err (each case listed in [IDEAS.md](https://github.com/sacridini/terraflux-py/blob/main/IDEAS.md) of the library and in the functions' notes).
      Three of the corrections change values people compare with R: the
      continuous Boyce index drops ecospat's "+1" on the last window (which
      made the index depend on the scale of the predictions), country
      checks use Natural Earth's `iso_a3_eh` (with `iso_a3`, France and
      Norway are "-99"), and MOP's Mahalanobis distance uses the
      reference's covariance (mop used that of the cells tested).
    - **Dinamica EGO's destinations** (`destinations`, of its road
      constructor): Dinamica gives each quadrant `n // q` rows or columns,
      so the last `n % q` never get a destination, and rounds its draw to an
      integer of the summed attraction, so the first cell of a quadrant gets
      half its share and a draw past the end takes the quadrant's last cell,
      even of no attraction. Terraflux's quadrants cover the map and its
      draw follows the attraction exactly.
    - **Off Windows**, the arctangent of the slopes of `surface.terrain` is
      the C library's, as terra's is on Windows: R takes it from Windows'
      UCRT, whose code is not public, and other C libraries (glibc) differ
      from it in the last bit. The slopes then differ from R's by one unit in
      the last place (two in degrees). The SWAT+ port calls the C library's
      exp, log and powers as SWAT+ does, and carries their last bits through
      years of daily state: off Windows its results are within 1e-4 of each
      quantity's scale of SWAT+'s on Windows (2.4e-5 at most in the test
      cases). Everything else gives the same bits on Windows and Linux: Terraflux carries its own versions of the
      functions whose bits matter (log, sqrt, sin, cos, atan2, exp and powers
      as mingw-w64 computes them on the x87 unit, and the UCRT's hypot).

## Terraflux, the application

The application and the library agree because they share their kernels and because the application is checked against
the library:

- For land change simulation, landscape metrics, neutral landscape models and the GuidosToolbox analyses, the
  application runs the library's own C++ code (its `terraflux-ls-sys` crate is built from the library's sources), so a
  model run in the application gives the same bits as the library with the same seed.
- The application's other operators are Rust ports, and each is tested against the library. The tests under
  `crates/*/tests` of the [application's repository](https://github.com/sacridini/terraflux) read references stored in
  `tests/data/reference`, which scripts named `tools/make_*_reference.py` write by running the library (most of the 61
  of them; a few take their references from numpy, scipy or R directly). The comparison is to the bit wherever the
  library's arithmetic can be reproduced, and to a rounding stated in the test where the library takes its result from
  numpy's BLAS, SIMD exponentials or LAPACK (the habitat graphs' tests say which columns, the circuit tests compare
  the maps to the bit). The library, in turn, is checked against the original tools, as above.

The scripts are in the application's [`tools`](https://github.com/sacridini/terraflux/tree/main/tools) folder; each
one starts with a comment that says what it writes and how to run it.

## Reproduce

The comparisons are part of the test suite:

```bash
pip install -e ".[test]"
pytest                              # every test, stored references included
pytest -m live_reference            # also run the originals that are installed
```

The live runs look for the originals in these environment variables:

| Variable | Original |
|---|---|
| `TERRAFLUX_MSPA_EXE` | GuidosToolbox's `mspa` executable (`MSPAstandalone/mspa_win64.exe`) |
| `TERRAFLUX_GDL_EXE` | GDL, to run GuidosToolbox's IDL code |
| `TERRAFLUX_SPATCON_EXE`, `TERRAFLUX_GRAYSPATCON_EXE`, `TERRAFLUX_GGEO_EXE` | GuidosToolbox's helper programs |
| `TERRAFLUX_RSCRIPT`, `TERRAFLUX_R_LIBS` | R with landscapemetrics and Makurhini |
| `TERRAFLUX_JULIA`, `TERRAFLUX_JULIA_PROJECT` | Julia with Circuitscape and Omniscape |

[`tools/make_reference*.py`](https://github.com/sacridini/terraflux-py/tree/main/tools) regenerate the stored references from the
originals, and `python tools/benchmark.py --exe <mspa executable>` repeats
the MSPA timings. [`tools/make_reference_optimize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_optimize.py) and [`tools/benchmark_optimize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_optimize.py) use R
with prioritizr, prioriactions and gurobi (`TERRAFLUX_RSCRIPT`,
`TERRAFLUX_R_LIBS`); Marxan's references come from its executable in WSL.
[`tools/make_reference_prioritize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_prioritize.py) and
[`tools/benchmark_prioritize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_prioritize.py) look for Zonation 5's `z5.exe` and
`z5_16bit.exe` in `TERRAFLUX_Z5_DIR`.
[`tools/make_reference_sdm.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_sdm.py) and [`tools/benchmark_sdm.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_sdm.py) use R with the
packages listed above in `TERRAFLUX_SDM_R_LIBS` (default
`%LOCALAPPDATA%/R/landschaft-sdm-lib`).
[`tools/make_reference_simulate.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_simulate.py) and [`tools/benchmark_simulate.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_simulate.py) use R
with lulcc, OpenLand and diffeR (`TERRAFLUX_SIM_R_LIBS`, default
`%LOCALAPPDATA%/R/landschaft-sim-lib`), CLUMondo (`TERRAFLUX_CLUMONDO`,
`TERRAFLUX_CLUMONDO_FIXED`), GRASS 8 with the `r.futures` addons in WSL,
Dinamica EGO's console (`TERRAFLUX_DINAMICA`) and MOLUSCE's sources
(`TERRAFLUX_MOLUSCE`, `TERRAFLUX_MOLUSCE_FIXED`);
[`tools/benchmark_dinamica.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_dinamica.py) times the Dinamica EGO functors added in 1.46
on 4 cores.
