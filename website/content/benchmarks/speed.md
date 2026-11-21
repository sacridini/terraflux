---
title: Speed
short: Speed
description: How much faster Terraflux's methods run than the tools they reproduce, area by area, with the machine, the method and every case.
order: 10
---

How long does a method take in Terraflux, and how long in the tool it reproduces? This page answers with one chart
and one table for each area, and ends with Terraflux's own times on much larger problems.

!!! note "Whose timings these are"
    The timings are those of **the Python library** ([terraflux-py](https://github.com/sacridini/terraflux-py), version
    {{pyversion}}), measured as each section says. The application runs the library's C++ for land change simulation,
    landscape metrics, neutral landscape models and the GuidosToolbox analyses, and Rust ports for the other methods
    ([how it is checked](/benchmarks/application/)). **The application has no timing table of its own yet**, so none of
    these numbers is a measured run time of the application.

Every chart shows the **speed-up**: the original's time divided by Terraflux's, on a logarithmic scale. A bar of 10 means
ten times faster; the dashed line at 1 is the same speed. The original's time is its own run, including its input and
output where noted. Terraflux is timed in memory. Across the 121 cases with a reference time, the median speed-up is 10
times, the smallest 1.2 times (FD8 flow accumulation) and the largest more than 6,000 times (a neutral landscape model
and a texture image, both of which R computes cell by cell).

!!! info "How these were measured"
    - **MSPA** was measured with [`tools/benchmark.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark.py)
      (version 0.53), and **spatial prioritization** with
      [`tools/benchmark_prioritize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_prioritize.py), on a
      desktop with an Intel Core i5-13600K (14 cores, 20 threads), 64 GB of memory and Windows 11. MSPA is the fastest of 10
      alternating runs; prioritization is one run of Zonation 5 and the fastest of 3 of Terraflux. Both programs were kept
      on the performance cores.
    - **Species distributions, zonal statistics and plotting** were measured on the same performance cores: the best of 3
      runs in R (the fastest of 3 for the others) and of 5 in Terraflux.
    - **Contiguity constraints** were measured on a Ryzen 7 7730U laptop with 4 threads, a limit of 120 s and a gap of 1 %.
    - **Terrain hydrology** and **Dinamica EGO's functors** were measured on 4 cores on both sides. **Optimized plans** used
      12 threads on the performance cores, a gap of 1 % and a limit of 120 s unless stated. **Circuits** used 20 threads.
    - **The other timings** were recorded in the library's [changelog](/python/changelog/) when each tool was written, with the
      version given in the column "Since". Their machine is not restated.

    Times depend on the computer, the map and the options: treat them as orders of magnitude. Nothing was measured on Linux
    or macOS: those systems give the same results, not timings.

!!! note "Hybrid processors"
    On processors with performance and efficiency cores (Intel 12th generation and later), Windows may move a busy
    process to the slower efficiency cores, especially when it runs in the background; any program then takes up to
    twice as long. Keep the terminal in the foreground for timings, and compare runs made side by side.

## Habitat patterns (MSPA)

Against GuidosToolbox's `mspa` executable (version 2.3, whose time includes reading and writing its TIFF files), on its
sample map and on the CORINE forest map. Terraflux was run with 1 thread and with the default 19. Every output was
compared pixel by pixel and is identical. **On one thread Terraflux is 8.0 to 13.7 times faster; with 19 threads, 9.8 to
16.7 times.** GuidosToolbox's executable runs on one thread, so the one-thread column is the like-for-like one.

```tfchart
type: dot
log: true
format: "{v} s"
labels_at_end: true
data: /data/benchmarks/view/speed_mspa.csv
x: short
y: [tf19, tf1, gtb]
labels: { tf19: "Terraflux, 19 threads", tf1: "Terraflux, 1 thread", gtb: GuidosToolbox mspa }
xlabel: Seconds for one map (logarithmic scale)
caption: "MSPA of two maps with three settings each (connectivity, edge width in cells): the time of GuidosToolbox's executable and of the Python library on 1 and on 19 threads. The number at the end of a row is GuidosToolbox's time. Rows read: the map and its size in cells per side (1000² is 1000 × 1000), then connectivity / edge width."
```

```tftable
data: /data/benchmarks/view/speed_mspa.csv
columns: [case, gtb, tf1, tf19, sp1, sp19, identical]
sortable: true
align: { case: left }
digits: 4
format: { sp1: "{v}×", sp19: "{v}×" }
labels: { case: "Map · connectivity, edge width", gtb: GuidosToolbox, tf1: "Terraflux, 1 thread", tf19: "Terraflux, 19 threads", sp1: "Speed-up, 1 thread", sp19: "Speed-up, 19 threads", identical: Result }
badge: { identical: { identical: good } }
caption: "Times are in seconds. Settings are the connectivity (4 or 8) and the edge width, in cells."
```

At scale, the CORINE map repeated 3 × 3 (6000 × 6000, 36 million pixels) takes GuidosToolbox 20 to 26 s and Terraflux 1.4
to 2.3 s, again with identical outputs.

MSPA's longest steps (geodesic distances, thinning, watershed) are ordered propagations that must run in sequence to stay
bit-exact, so one map takes only 10 to 30 % less time with more threads. Many maps gain fully: run them side by side with
[`mspa_batch`](/python/api/batch/#terraflux.mspa_batch) or [`batch`](/python/api/batch/#terraflux.batch).

## Landscape metrics

Against the R package landscapemetrics 2.2.2.

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
data: /data/benchmarks/view/speed_lsm_chart.csv
x: short
y: [speedup]
labels: { speedup: Speed-up }
xlabel: Times faster than landscapemetrics
caption: "Landscape metrics on `augusta_nlcd` (440 × 678 cells) and on a 1000 × 1000 map: how many times faster the library is than landscapemetrics. 1000² is 1000 × 1000 cells; window_lsm runs a 5 × 5 window on a map of 9 × 11 cells."
```

```tftable
data: /data/benchmarks/view/speed_lsm.csv
columns: [case, reference_s, terraflux_s, speedup, since, note]
sortable: true
align: { case: left, note: left }
digits: 4
format: { speedup: "{v}×" }
labels: { case: Task, reference_s: landscapemetrics (s), terraflux_s: Terraflux (s), speedup: Speed-up, since: Since, note: Note }
caption: "\"Since\" is the version of the library that brought the method. The results of every row are identical to landscapemetrics' bits ([accuracy](/benchmarks/accuracy/#results-by-tool))."
```

At scale, a 10,000 × 10,000 map with 7.7 million patches gets all 133 metrics (92 million values) in about 10 s, and maps
larger than memory are read in blocks with the same results ([larger problems](#larger-problems)).

## Habitat networks

Against the R package Makurhini (and, through it, gdistance).

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
data: /data/benchmarks/view/speed_habitat_chart.csv
x: short
y: [speedup]
labels: { speedup: Speed-up }
xlabel: Times faster than Makurhini
caption: "Habitat graphs and distances on Makurhini's example data (404 patches; 398 patches over 222 thousand cells). IIC is the integral index of connectivity, PC the probability of connectivity."
```

```tftable
data: /data/benchmarks/view/speed_habitat.csv
columns: [case, reference_s, terraflux_s, speedup, since]
sortable: true
align: { case: left }
digits: 4
format: { speedup: "{v}×" }
labels: { case: Task, reference_s: Makurhini (s), terraflux_s: Terraflux (s), speedup: Speed-up, since: Since }
caption: "The commute-time row is 13.5 times by the detailed timing (50 s against 3.7 s); the library's first table says \"13 times\"."
```

## Circuit connectivity

Against Omniscape 0.7 and Circuitscape 6.1 (Julia), both with 20 threads.

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
color_by: tool
data: /data/benchmarks/view/speed_circuit_chart.csv
x: short
y: [speedup]
labels: { speedup: Speed-up }
xlabel: Times faster than the Julia package
caption: "Omniscape and Circuitscape runs on synthetic maps, with 20 threads on both sides. The last two rows are the same network problem (1 million nodes) against Circuitscape's exact solver and against its default iterative one. Omniscape rows read: the map in cells per side, then the moving-window radius; the Circuitscape rows use a 1000 × 1000 map."
```

```tftable
data: /data/benchmarks/view/speed_circuit.csv
columns: [case, reference_s, terraflux_s, speedup, since]
sortable: true
align: { case: left }
digits: 4
format: { speedup: "{v}×" }
labels: { case: Task, reference_s: Original (s), terraflux_s: Terraflux (s), speedup: Speed-up, since: Since }
caption: "Every window and every circuit is solved exactly (a sparse Cholesky factorisation), and the result does not depend on the number of threads."
```

For the network mode, the original's time is 27.7 s with its exact solver (CHOLMOD) and 182 s with its default solver
(conjugate gradients with an algebraic multigrid). The default solver is itself only about 2e-7 away from the exact solution,
so Terraflux is compared with the exact runs ([accuracy](/benchmarks/accuracy/#results-by-tool)).

## Grid distances

Against the R package gdistance 1.6.5 on a 300 × 300 grid of random resistances (8 directions; best of 3 runs in R, of 5 in
Terraflux).

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
data: /data/benchmarks/view/speed_grid_chart.csv
x: short
y: [speedup]
labels: { speedup: Speed-up }
xlabel: Times faster than gdistance
caption: "Least-cost, random-walk and randomised-shortest-path tasks on a 300 × 300 grid. One more task is not in the chart: gdistance fails on the 3 × 3 points case because it needs a dense matrix of 60 GiB. 60² is 60 × 60 cells."
```

```tftable
data: /data/benchmarks/view/speed_grid.csv
columns: [case, reference_s, terraflux_s, speedup, since, note]
sortable: true
align: { case: left, note: left }
digits: 4
format: { speedup: "{v}×" }
labels: { case: Task, reference_s: gdistance (s), terraflux_s: Terraflux (s), speedup: Speed-up, since: Since, note: Note }
caption: "Terraflux's randomised-shortest-path distances between 3 × 3 points take 0.15 s where gdistance cannot run."
```

At scale, on 2000 × 2000 (4 million cells) a transition takes 0.15 s, a cost surface 0.6 s, a random-walk passage map 4.5 s, a
randomised-shortest-path passage map 4.6 s (7.8 s for an asymmetric transition) ([larger problems](#larger-problems)).

## Neutral landscape models

The same landscapes as NLMR (same seeds, identical values), one model call each (best of 5, default threads).

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
data: /data/benchmarks/view/speed_nlm_chart.csv
x: short
y: [speedup]
labels: { speedup: Speed-up }
xlabel: Times faster than NLMR
caption: "Eighteen neutral landscape models. The gain is largest for the models NLMR builds cell by cell (random clusters, mosaic fields) and smallest for the simplest gradients. Sizes are cells per side (2000² is 2000 × 2000)."
```

```tftable
data: /data/benchmarks/view/speed_nlm.csv
columns: [case, reference_s, terraflux_s, speedup, since]
sortable: true
align: { case: left }
digits: 4
format: { speedup: "{v}×" }
labels: { case: Model and size, reference_s: NLMR (s), terraflux_s: Terraflux (s), speedup: Speed-up, since: Since }
caption: "Every landscape has the same values as NLMR's with the same seed."
```

## Surface metrics and terrain indices

Against geodiv 1.1.1 and spatialEco 2.0-5 (R 4.4.2, terra 1.9-50) on geodiv's example rasters (371 × 371 cells: `normforest`,
`orforest` for the trend, `orelevation` for terrain), measured with
[`tools/benchmark_surface.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_surface.py): the best of 3 runs
in R (fast tasks repeated to 0.5 s), of 5 in Terraflux with the default threads. Where Terraflux corrects geodiv or
spatialEco the work is the same. (The page of the library's tables says only "R" for each row; the first ten rows are
geodiv's and the rest spatialEco's and terra's, by the functions they name.)

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
color_by: tool
data: /data/benchmarks/view/speed_surface_chart.csv
x: short
y: [speedup]
labels: { speedup: Speed-up }
xlabel: Times faster than the R package
caption: "Surface metrics (geodiv) and terrain indices (spatialEco, terra) on 371 × 371 cells; the two texture images are on 100² and 30² cells (5 × 5 windows)."
```

```tftable
data: /data/benchmarks/view/speed_surface.csv
columns: [case, tool, reference_s, terraflux_s, speedup, since]
sortable: true
align: { case: left, tool: left }
digits: 4
format: { speedup: "{v}×" }
labels: { case: Task, tool: R package, reference_s: R (s), terraflux_s: Terraflux (s), speedup: Speed-up, since: Since }
caption: "The R times are in seconds: 0.029 s is 29 ms."
```

The gains are largest where the R packages compute cell by cell in R (texture images, peaks, curvature, TRI) and smallest
where terra's focal functions do the work in C++ with running sums (TPI, HSP, VRM). Terraflux's focal means, sums and
standard deviations use compensated prefix sums along the rows (within about 1e-13 of summing each window), so a window
costs one step per row whatever its size.

## Terrain hydrology

Against WhiteboxTools 2.4 on a rough terrain of 3000 × 3000 cells, both on 4 cores
([`tools/benchmark_hydro.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_hydro.py), best of 3).
WhiteboxTools' times include reading and writing its GeoTIFFs, so Terraflux's include writing its result. This is the area
with the smallest gains: WhiteboxTools is already compiled code.

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
data: /data/benchmarks/view/speed_hydro_chart.csv
x: short
y: [speedup]
labels: { speedup: Speed-up }
xlabel: Times faster than WhiteboxTools
caption: "Five hydrology tasks on a 3000 × 3000 terrain, 4 cores on both sides."
```

```tftable
data: /data/benchmarks/view/speed_hydro.csv
columns: [case, reference_s, terraflux_s, speedup, since]
sortable: true
align: { case: left }
digits: 4
format: { speedup: "{v}×" }
labels: { case: Task, reference_s: WhiteboxTools (s), terraflux_s: Terraflux (s), speedup: Speed-up, since: Since }
caption: "The outputs are identical bit for bit."
```

## Spatial prioritization

Against Zonation 5 v2.4 (`z5.exe`, and `z5_16bit.exe` for 16 bits) on synthetic species maps: seeded Gaussian random fields
above a threshold, each feature on 60 % of the cells (10 % in the 200-feature case), written as GeoTIFFs. Measured with
[`tools/benchmark_prioritize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_prioritize.py) (version
0.95) on the performance cores, one run of z5 and the fastest of 3 of Terraflux; both times include reading the GeoTIFFs,
and z5's writing its outputs. Every rank map is the same, bit for bit, as z5's on one thread.

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
data: /data/benchmarks/view/speed_prioritize.csv
x: short
y: [sp1, sp12]
labels: { sp1: "1 thread", sp12: "12 threads" }
xlabel: Times faster than Zonation 5 on the same number of threads
caption: "Terraflux against z5 on the same number of threads, on five synthetic problems. Rows read: cells per side (1000² is 1000 × 1000), the number of features and the rule or setting (CAZ2 and ABF are Zonation's cell-removal rules)."
```

```tftable
data: /data/benchmarks/view/speed_prioritize.csv
columns: [case, z5_1, z5_12, tf1, tf12, sp1, sp12, since]
sortable: true
align: { case: left }
digits: 4
format: { sp1: "{v}×", sp12: "{v}×" }
labels: { case: "Map, features, rule and iterations", z5_1: "z5, 1 thread (s)", z5_12: "z5, 12 threads (s)", tf1: "Terraflux, 1 thread (s)", tf12: "Terraflux, 12 threads (s)", sp1: "Speed-up, 1 thread", sp12: "Speed-up, 12 threads", since: Since }
caption: "Rules: CAZ2 (core-area zonation) and ABF (additive benefit function). \"16 bits\" is z5's 16-bit build."
```

On one thread Terraflux is 1.5 to 7 times faster than z5, most with many sparse features: each feature is walked along its
own occurrences, where z5 visits every feature in every cell. With 12 threads the gap grows to 4 to 11 times: z5 gains 1.7 to
3 times from its 12 threads, Terraflux 4.7 to 6.3. Memory: about 130 bytes per cell with data during the ranking, plus 1 to 2
bytes per value that is not zero; `storage=` puts the large arrays in memory-mapped files. z5 on several threads can give
results different from its own one-thread run; Terraflux's do not depend on the threads ([known differences](/benchmarks/accuracy/#known-differences)).

### Optimized plans

Against prioritizr 8.0.6's models solved by HiGHS 1.15 alone (the solver prioritizr calls) and by Gurobi 12, on synthetic
problems of [`tools/benchmark_optimize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_optimize.py): 30
species-like features with targets of 30 % and costs from 1 to 10 (multi-action: 10 features, 3 threats), a gap of 1 %, 12
threads on the performance cores. "No answer" means no plan within the time limit (120 s unless stated). The same models and
the same optima as prioritizr's; Terraflux's engines start HiGHS from near-optimal plans, fix the units whose choice is
proven, and solve the problems general solvers find hard on their own.

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
data: /data/benchmarks/view/speed_optimize.csv
x: short
y: [sp_highs, sp_gurobi]
labels: { sp_highs: HiGHS alone, sp_gurobi: Gurobi }
xlabel: Times faster than the solver
caption: "Optimized plans against the solvers prioritizr uses. A star marks a problem where HiGHS gave no plan in its time limit: the bar is a lower bound. Some problems were not run with one of the solvers. k is thousand and M million planning units; gap 0 asks for the exact optimum."
```

```tftable
data: /data/benchmarks/view/speed_optimize.csv
columns: [label, highs_s, gurobi_s, terraflux_s, sp_highs, sp_gurobi, since, note]
sortable: true
align: { label: left, note: left }
digits: 4
format: { sp_highs: "{v}×", sp_gurobi: "{v}×" }
labels: { label: Problem, highs_s: HiGHS alone (s), gurobi_s: Gurobi (s), terraflux_s: Terraflux (s), sp_highs: "Speed-up over HiGHS", sp_gurobi: "Speed-up over Gurobi", since: Since, note: Note }
caption: "Where HiGHS gave no answer, its column shows the time limit and the speed-up is a lower bound. A minimum set of 10 million units (5 features) is solved by Terraflux in 60 s, proven optimal."
```

#### Contiguity constraints

Contiguity constraints are the exception. prioritizr's contiguity model is a restriction (it misses some patches, see the
[migration guide](/tutorials/spatial-prioritization/from-prioritizr-in-python/#what-to-expect-when-you-compare-the-results));
Terraflux solves the exact model: a patch heuristic, a first patch of the restricted model, directed cuts at the root, then
HiGHS's branch and bound with the same cuts separated at its nodes (HiGHS is patched for it). This table compares quality, not
time: [`tools/benchmark_connect.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_connect.py) (version
1.52; 120 s, gap 1 %, 4 threads, on a Ryzen 7 7730U laptop) against Gurobi 13 on the same exact model, alone and with
Terraflux's cuts added at its nodes by a callback.

```tftable
data: /data/benchmarks/quality_contiguity.csv
sortable: true
align: { case: left, note: left }
digits: 2
labels: { case: Problem, terraflux_best_patch: "Terraflux: best patch", terraflux_gap_pct: "Terraflux: gap (%)", gurobi_best_patch: "Gurobi: best patch", gurobi_gap_pct: "Gurobi: gap (%)", gurobi_plus_cuts_best_patch: "Gurobi + cuts: best patch", gurobi_plus_cuts_gap_pct: "Gurobi + cuts: gap (%)", note: Note }
caption: "Best patch found (a lower number is a cheaper network) and the proven gap to the optimum, after 120 s."
```

Terraflux's patches are as cheap as or cheaper than Gurobi's, and its proven gaps match Gurobi's own except with feature
contiguity; Gurobi's branch and bound does more with the same cuts. Before version 1.52, the same runs gave Serra Verde a
network of 33,981 with a gap of 24 % (in 60 s).

## Species distributions

Against the R packages each step reproduces (R 4.4.2, terra 1.9-50): CoordinateCleaner 3.0.1, spThin 0.2.0, usdm 2.1-7, dismo
1.3-16, blockCV 3.2-0, predicts 0.1-19, ecospat 4.1.3 and maxnet 0.1.4 with glmnet 4.1-10. The inputs are the example climate
(9 bioclimatic layers, 192 × 186 cells of 0.5°), the *Bradypus* records with 10,000 background points, 5,000 synthetic records
over the same area and 2,000 clustered ones. Measured with
[`tools/benchmark_sdm.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_sdm.py) (version 1.0) on the
performance cores: the best of 3 runs in R (fast tasks repeated to 0.5 s), of 5 in Terraflux with the default threads. Every
result is the same as R's, the random steps included (same seed), except where the
[known differences](/benchmarks/accuracy/#known-differences) say otherwise.

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
data: /data/benchmarks/view/speed_sdm_chart.csv
x: short
y: [speedup]
labels: { speedup: Speed-up }
xlabel: Times faster than the R package
caption: "Eleven steps of a species distribution workflow, each against the R package it reproduces."
```

```tftable
data: /data/benchmarks/view/speed_sdm.csv
columns: [case, reference_s, terraflux_s, speedup, since]
sortable: true
align: { case: left }
digits: 4
format: { speedup: "{v}×" }
labels: { case: Task, reference_s: R (s), terraflux_s: Terraflux (s), speedup: Speed-up, since: Since }
caption: "Package of each row: CoordinateCleaner, spThin, usdm, dismo, blockCV (three rows), predicts (two rows), ecospat, maxnet."
```

The largest gains are where R works on whole matrices: spThin computes the distance between every pair of records and rescans
them after each removal (Terraflux keeps only the neighbours, in a grid over the sphere, and the counts in a tree: a million
records take seconds, where R runs out of memory), and blockCV's blocks and predicts' evaluation go through data frames row by
row. MaxNet keeps glmnet's arithmetic and order of operations, so the gain comes from screening (most gradient checks are
settled by a bound, without the exact dot product) and from parallel work around the coordinate descent, which itself stays
serial; with 20 predictors and hinge features (10,000 background points) it fits in 0.47 s against R's 6.4 s. Weighted
sampling without replacement, which R does in time proportional to the cells times the draws, runs in closed form over runs of
equal weights (1 million cells and 30,000 draws: 0.24 s against 16 to 20 s with R's algorithm).

## Land change simulation

Against the land change tools `tf.simulate` reproduces, on the cases of its reference tests. Measured with
[`tools/benchmark_simulate.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_simulate.py) (version 1.20):
lulcc 1.0.4 in R 4.4.2 (with the corrections of
[`tools/lulcc_fixed.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/lulcc_fixed.R), so that both sides make the same
allocation; only the timed call is measured), MOLUSCE 5.3.0's algorithms in Python, Dinamica EGO 8.13's console (the time of a
script that only loads the maps is subtracted); the best of 3 runs, the default threads. CLUMondo and FUTURES were timed when
they were written.

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
color_by: tool
data: /data/benchmarks/view/speed_landchange_chart.csv
x: short
y: [speedup]
labels: { speedup: Speed-up }
xlabel: Times faster than the original tool
caption: "Ten land change tasks, each against its original: lulcc, CLUMondo, GRASS's r.futures, MOLUSCE and Dinamica EGO."
```

```tftable
data: /data/benchmarks/view/speed_landchange.csv
columns: [case, tool, reference_s, terraflux_s, speedup, since]
sortable: true
align: { case: left, tool: left }
digits: 4
format: { speedup: "{v}×" }
labels: { case: Task, tool: Original, reference_s: Original (s), terraflux_s: Terraflux (s), speedup: Speed-up, since: Since }
caption: "Dinamica EGO's functors of version 1.46 (the last three rows) were timed on 4 cores on both sides."
```

The Dinamica EGO functors of version 1.46 were timed with
[`tools/benchmark_dinamica.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_dinamica.py) on 4 cores on
both sides (Dinamica with `-processors 4`), the time of the functor in Dinamica's log. Its cost surface sweeps the map in four
directions until no cost changes (fewer passes give an approximation); Terraflux's runs Dijkstra's algorithm once, with the same
sums.

CLUE-S draws a random number per cell and class at each of its thousands of passes, from R's generator, one after the other:
Terraflux draws the same numbers, so the gain is in the rest of the pass. The three-map comparison runs lulcc's equations per
block in one pass of C++ (R builds a raster per class and equation), and MOLUSCE loops over the pixels in Python.

## Zonal statistics and extraction

Not a reproduction of another tool: the functions that read a raster in zones and at points (`zonal_stats`, `extract`),
against the Python packages commonly used for it, rasterstats 0.21 and exactextract 0.3 (both single-threaded). A 5000 × 5000
float32 raster in memory; the statistics mean, min, max, standard deviation and median; random polygons of 16 vertices.
Measured with [`tools/benchmark_zonal.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_zonal.py)
(version 0.98) on the performance cores, the fastest of 3 alternating runs. The speed-ups below are for Terraflux on 1 thread.

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
color_by: tool
data: /data/benchmarks/view/speed_zonal_chart.csv
x: short
y: [speedup]
labels: { speedup: Speed-up }
xlabel: Times faster than the package (Terraflux on 1 thread)
caption: "Zonal statistics and values at points, against rasterstats and exactextract: 2000 small polygons (16 million cells), 20 large polygons (22 million cells) and 1 million points."
```

```tftable
data: /data/benchmarks/speed_zonal.csv
sortable: true
align: { case: left }
digits: 4
format: { speedup_vs_rasterstats_1_thread: "{v}×", speedup_vs_exactextract_1_thread: "{v}×" }
labels: { case: Case, rasterstats_s: rasterstats (s), exactextract_s: exactextract (s), terraflux_1_thread_s: "Terraflux, 1 thread (s)", terraflux_12_threads_s: "Terraflux, 12 threads (s)", speedup_vs_rasterstats_1_thread: "Speed-up over rasterstats", speedup_vs_exactextract_1_thread: "Speed-up over exactextract", since_version: Since }
caption: "Values at 1 million points: rasterstats took about 119 s; exactextract was not run."
```

The cells of every polygon are those GDAL burns (the centre rule, or `all_touched`), as rasterstats takes them: the statistics
agree with its own, which adds up in single precision, to 1e-7, and with NumPy in double precision to 1e-15. exactextract weights
every cell by the share of it the polygon covers, so its means differ slightly. The median and the percentiles take a
linear-time selection, not a sort; the result does not depend on the number of threads.

## Animal movement

The dynamic Brownian bridges of move2utils 0.4.6 in R 4.4.2 on one thread (`OMP_NUM_THREADS=1`; the C kernels of the
`move` package), against the library's
`tf.move.dbbmm_variance`, `dbbmm_ud`, `dbgb_variance` and `dbgb_ud`, which run the same method (the same numbers bit for bit,
the same on any number of threads). The tracks are the fishers of move2's example data (F1, F2 and M1) and a
simulated track of 100,000 locations; the UDs are on cells of 100 m or 30 m, or on a grid of 1669 × 2000 cells. The times
are the library's, with the same C++ as the application's; **the application's own times were not measured**. These
comparisons are not counted in the 121 cases, the median or the areas of the summary at the top of the page.

```tfchart
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
data: /data/benchmarks/view/speed_movement.csv
x: short
y: [speedup_1_thread, speedup_8_threads]
labels: { speedup_1_thread: "1 thread", speedup_8_threads: "8 threads" }
xlabel: Times faster than move2utils in R
caption: "Terraflux's library on 1 and 8 threads against move2utils 0.4.6 in R, on the fishers of move2 and on a track of 100,000 locations. A bar below the dashed line is slower than R."
```

```tftable
data: /data/benchmarks/speed_movement.csv
columns: [case, reference_s, terraflux_1_thread_s, terraflux_8_threads_s, speedup_1_thread, speedup_8_threads]
sortable: true
align: { case: left }
digits: 4
format: { speedup_1_thread: "{v}×", speedup_8_threads: "{v}×" }
labels: { case: Case, reference_s: "move2utils in R (s)", terraflux_1_thread_s: "Terraflux, 1 thread (s)", terraflux_8_threads_s: "Terraflux, 8 threads (s)", speedup_1_thread: "Speed-up, 1 thread", speedup_8_threads: "Speed-up, 8 threads" }
caption: "The last row, `corridor` on the fishers, has no R time: 0.36 s on 1 thread and 0.16 s on 4."
```

On one thread the library is about as fast as R, from 1.3 times faster to 0.8 times as fast (the dBGB's UDs, 19.5 s against
15.7 s on 100 m cells and 206 s against 164 s on 30 m cells): the kernels are ported from R's C code, and each cell adds its
time steps in the order of the track, which gives the same bits with any number of threads. The gain is the threads: 5.6 to 7.8 times
faster than R for the dBBMM and 2.6 to 4.7 for the dBGB on 8 threads. R's `move2utils` itself parallelises the cells of
each time step only; the library gives each thread a strip of rows of the grid.

## Plotting

`tf.plot`, from the data to the page a notebook or a window shows (the map's HTML, images and data included), measured with
[`tools/benchmark_plot.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_plot.py) (version 1.32.0) on the
performance cores, the fastest of 3 runs. There is no reference tool: these are Terraflux's own times. A first plot in a
session also imports folium and compiles its templates, about 0.7 s once. A page holds everything it needs (Leaflet, about
0.16 MB; plotly, 4.3 MB, in a page with charts), so it opens without a network connection.

```tfchart
type: bar
horizontal: true
format: "{v} s"
labels_at_end: true
digits: 2
data: /data/benchmarks/view/speed_plot.csv
x: short
y: [terraflux_s]
labels: { terraflux_s: Time }
xlabel: Seconds from the data to the page
caption: "Time to make the plot of `tf.plot` in the Python library, from data in memory or on disk to the page. 10 000² is 10 000 × 10 000 cells; the stack is 47 bands of 1000 × 1000."
```

```tftable
data: /data/benchmarks/view/speed_plot.csv
columns: [case, terraflux_s, page_mb]
sortable: true
align: { case: left }
digits: 4
format: { terraflux_s: "{v} s", page_mb: "{v} MB" }
labels: { case: Case, terraflux_s: Time, page_mb: Page }
```

The time hardly grows with the size of a raster: it is reduced to the resolution of the screen first (block means in C++, or
the cells of a lazy raster read in strips), and only that is reprojected and encoded.

## Larger problems

Terraflux's own times on problems much larger than the comparisons above, where no original could be run or none was timed.
There is no reference column: these are not speed-ups.

### Landscape metrics

```tfchart
type: bar
horizontal: true
format: "{v} s"
labels_at_end: true
data: /data/benchmarks/view/scale_lsm.csv
x: short
y: [terraflux_s]
labels: { terraflux_s: Time }
xlabel: Seconds
caption: "Landscape metrics on large maps. Rows marked \"extra\" are the extra time on top of the labelling above them. Sizes are cells per side (10 000² is 10 000 × 10 000); the 20 000² rows read a 400-million-cell file."
```

```tftable
data: /data/benchmarks/view/scale_lsm.csv
columns: [case, terraflux_s, note]
sortable: true
align: { case: left, note: left }
digits: 4
format: { terraflux_s: "{v} s" }
labels: { case: Case, terraflux_s: Terraflux, note: Memory }
caption: "`block_rows=\"auto\"` reads the file in blocks: less memory, a little more time, the same results."
```

### Habitat networks

```tfchart
type: bar
horizontal: true
log: true
format: "{v} s"
labels_at_end: true
data: /data/benchmarks/view/scale_habitat.csv
x: short
y: [terraflux_s]
labels: { terraflux_s: Time }
xlabel: Seconds (logarithmic scale)
caption: "Habitat graphs, distances and link importances on random patches and on a 4000 × 4000 raster. PC is the probability of connectivity; IIC the integral index of connectivity. Rows read: the patch count or the raster size in cells per side (4000² is 4000 × 4000); link rows are the importance of every link."
```

```tftable
data: /data/benchmarks/view/scale_habitat.csv
columns: [case, terraflux_s]
sortable: true
align: { case: left }
digits: 4
format: { terraflux_s: "{v} s" }
labels: { case: Case, terraflux_s: Terraflux }
```

### Grid distances

```tftable
data: /data/benchmarks/view/scale_grid.csv
columns: [case, terraflux_s]
sortable: true
align: { case: left }
digits: 4
format: { terraflux_s: "{v} s" }
labels: { case: Case on a 2000 × 2000 grid, terraflux_s: Terraflux }
```

### Other areas

- **MSPA:** the CORINE map repeated 3 × 3 (6000 × 6000, 36 million pixels), three parameter sets: 1.4 to 2.3 s, against 20 to 26 s
  for GuidosToolbox, with identical outputs.
- **Optimized plans:** a minimum set of 10 million units (5 features) in 60 s, proven optimal. No reference was run.
- **Species distributions:** thinning a million records takes seconds, where spThin runs out of memory (the library gives no
  figure). A MaxNet fit with 20 predictors and hinge features takes 0.47 s; weighted sampling without replacement of 30,000 draws
  from 1 million cells takes 0.24 s.
- **Land change simulation in the application:** 56.6 million cells, 10 transitions and 3 steps on 4 threads took 318 s in all
  (43 s of fits, about 92 s a step), recorded as a scale figure of the application's tiled engine in version 0.5.0 of the changelog.
  There is no reference run.
