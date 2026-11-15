---
title: Accuracy
short: Accuracy
description: How the results of Terraflux's methods are compared with the tools they reproduce, the result for each tool, the details by module and every known difference.
order: 20
---

Terraflux's methods are re-implementations. This page says how each one is compared with the program it reproduces, what the
comparison found, and where the results differ on purpose or by necessity.

!!! note "Whose comparisons these are"
    The comparisons are those of **the Python library** ([terraflux-py](https://github.com/sacridini/terraflux-py), version
    {{pyversion}}), which holds the methods and their test suite. The application is compared with the library, and the library
    with the originals: see [the application's own tests](/benchmarks/application/). The machines and the versions of every
    original tool are stated with each comparison, and none was run again for this page.

## How results are compared

- **Stored references.** The outputs of the original tools on the example maps, on synthetic maps built to hit edge cases, and
  on the originals' own test cases are stored in `tests/data/reference`. Every run of the test suite compares with them.
- **Live runs.** When the original is installed, the tests also run it on freshly generated random maps and compare on the spot
  (`pytest -m live_reference`).
- **Original code, not a re-reading of it.** Where GuidosToolbox computes in IDL, its processing statements are copied
  verbatim from `guidostoolbox.pro` and run with [GDL](https://github.com/gnudatalanguage/gdl), the free IDL implementation,
  with GuidosToolbox's own `spatcon`, `grayspatcon`, `recode` and `ggeo` programs where it calls them.
- **Bits, then tolerances.** Integer maps and counts must be identical. Floating-point results are compared bit for bit where
  the original's arithmetic can be reproduced (landscapemetrics: R's 80-bit sums and x87 `log`/`sqrt`), and otherwise to the
  tolerance given below, typically the precision of the original itself.

Four words are used for the result of a comparison:

- **identical**: every map, count and value has the same bits as the original's;
- **identical, some values to a tolerance**: maps and counts are identical, and some decimal values agree to a stated
  tolerance;
- **within a tolerance**: the values agree to a stated tolerance, for example because the original sums in single precision or
  inverts a matrix with LAPACK;
- **partly different**: the original is closed or approximate in part, and the difference is stated.

```tfchart
type: bar
horizontal: true
stacked: true
labels_at_end: true
data: /data/benchmarks/view/accuracy_by_domain.csv
x: domain
y: [c0, c1, c2, c3]
labels: { c0: identical, c1: "identical, some values to a tolerance", c2: within a tolerance, c3: partly different }
xlabel: Comparison groups
caption: "The 47 comparison groups of the table below, by area and by how they agree with the original. A group is a set of methods compared with one original on a set of cases."
```

## Results by tool

Each row is one comparison group: the methods of Terraflux, what they were compared with, on how many reference cases (where
the library counts them), and the result. Click a header to sort; the table is long, so sorting by "Result" or by "Area"
groups what you look for.

```tftable
data: /data/benchmarks/view/accuracy_view.csv
columns: [domain, methods, reference, cases, class, tolerance, result]
sortable: true
align: { domain: left, methods: left, reference: left, class: left, tolerance: left, result: left, cases: right }
digits: 0
labels: { domain: Area, methods: Methods, reference: Compared with, cases: Cases, class: Result, tolerance: Loosest tolerance, result: Agreement }
badge:
  class:
    identical: good
    "identical, some values to a tolerance": info
    "within a tolerance": info
    "partly different": warning
caption: "\"Cases\" is left empty where the library gives no count. Where an original errs, the comparison is with the original with its errors corrected, listed [below](#where-the-original-errs)."
```

The default solver of Omniscape and Circuitscape (conjugate gradients with an algebraic multigrid) is itself only about 2e-7
away from the exact solution; Terraflux's solves are exact, so it is compared with their exact (CHOLMOD) runs.

### Where the original errs

Some originals have errors, and a method that reproduced them would be wrong. In those cases the comparison is with the
original **with its errors corrected**, by a small script or patch kept in the library's repository, and the page says so:

- R packages: `tools/cc_fixed.R` (CoordinateCleaner), `tools/ecospat_fixed.R` (ecospat), `tools/lulcc_fixed.R` (lulcc),
  `tools/surface_fixed.R` (geodiv and spatialEco), `tools/differ_fixed.R` (lulcc, OpenLand and diffeR), `tools/move2_fixed.R`
  (move2's datum code) and `tools/amt_fixed.R` (amt), each in the
  [library's `tools` folder](https://github.com/sacridini/terraflux-py/tree/main/tools).
- Programs: `tools/clumondo_fixed.patch` (CLUMondo's age error), `tools/futures_fixed.patch` (the `r.futures` addons) and
  `tools/molusce_fixed.patch` (MOLUSCE's confidence).
- Also corrected, as the table says: prioritizr 8.0.6 (phylogenetic tip order, portfolio cuts), prioriactions 0.5.0, Marxan
  4.0.6, dsmextra's covariate index and, for Dinamica EGO's destinations, its quadrant and draw errors (see
  [known differences](#known-differences)).

## GuidosToolbox: the reference programs and the cases

The methods of GuidosToolbox are compared with its own programs and code, as the table above says. The cases behind each
comparison, as the library's older guide listed them:

- **MSPA:** the `mspa` executable (`mspa_win64.exe`) on 1,200+ random configurations and on full CORINE datasets: 0 differing
  pixels.
- **FAD and contagion:** the original JRC `spatcon64` program: the maps are bit-exact.
- **Landscape mosaic:** the JRC `recode64` program: bit-exact.
- **FAD-MS, FAD multiscale change, the landscape mosaic heatmap and its change:** GuidosToolbox's own IDL code run in GDL
  together with its `spatcon`: FAD-MS maps and 103-class maps identical, change tables identical, heatmap vectors identical bit
  for bit, CSV reports identical character for character.
- **MSPA Tiling:** GuidosToolbox's `mspatile` IDL code run in GDL with its gdal tools and `mspa`: 10 stored cases covering all
  the tiling layouts and tile shortcuts, plus live random cases; the maps are identical. GDL needs two shims for GuidosToolbox's
  file handling (`tests/gtb_idl/read_tiff.pro` and `updategeotagsinimage.pro`).
- **FOS** (FAD, FED and FAC, 5 and 6 classes, binary and grayscale): GuidosToolbox's batch FOS IDL code run in GDL with its
  `spatcon` and `grayspatcon`: 13 stored cases plus live random scenes; the maps are identical and the statistics are identical
  to IDL's float or double precision.
- **Accounting** (IDL-only in GuidosToolbox): its own IDL code (`tests/gtb_idl`, the statements copied verbatim from
  `guidostoolbox.pro`) run in GDL: maps and integer statistics identical. The stored digests
  (`tests/data/reference/idl_reference.json`) cover 7 maps with 5 parameter sets each, and random scenes run live when GDL is
  available.
- **GraySpatCon:** the two metrics that FOS uses (the mean cover and the clustering) are compared with the `grayspatcon`
  program, to float32 precision. The guide said that every metric was compared with `grayspatcon64.exe`; the tests in the
  repository check the others on constructed cases (see [checked in other ways](#checked-in-other-ways)), so that is what this
  page says.

## Checked in other ways

These tools follow GuidosToolbox's method, or have no open original to run, and are not compared with a run of its code. Each
is checked as the right-hand column says.

| Tools | How they are checked |
|---|---|
| Network components and importance of MSPA cores and bridges | an independent step-by-step reproduction of GuidosToolbox's IDL routine |
| Spatial entropy | an independent reproduction of GuidosToolbox's `entropy_mspainp.pro` |
| Restoration status, parcellation | GuidosToolbox's values for its sample map |
| Cost distance, least-cost path, restoration corridor, contortion, simple and morphological change, FOS change, GraySpatCon metrics other than those of FOS | unit tests on constructed cases |
| Zonal statistics, extraction at points, clip, rasterize | cells against GDAL's rasterizer, statistics against NumPy and rasterstats |
| Land change: Dinamica's Expander and Patcher | Dinamica EGO 8.13's functors crash for patches of more than one cell, so their seed selection was measured with one-cell patches (acceptance by neighbours, draw by probability) and the patch growth follows the publications; statistical comparison |
| Land change: Dinamica's continuous Patcher (`ContinuousPatcher`) and road destinations (`destinations`) | Dinamica EGO 8.13's continuous Patcher crashes for patches of more than one cell, as its Patcher does: the candidates, the pruning, the quota and the draw by probability were measured with one-cell patches (exact on the cases that do not depend on the draw, statistical on 32 draws), the growth is the Patcher's; destinations compared statistically on 100 draws (Dinamica's draw errors corrected, see [known differences](#known-differences)) |
| Hilltop protection areas (`surface.hilltop`) | written from the Forest Code (Lei 12.651/2012): Dinamica EGO's `CalcHilltop` follows the older CONAMA 303/2002 rule on a DEM cut into elevation bands, with its own definitions (its slope criterion measures to the map's edge or to other features: 5° on an isolated cone of 21.8°), so only the heights are compared (to 0.05 m on cones); checked against an independent computation of the landforms by connected components on random DEMs, and on cones, ranges and lakes of known geometry |
| Land change: PLUS's CA with multi-type random patch seeds (CARS) | PLUS is distributed without source code or licence: written from its paper, checked by its properties (demand met, growth from patches and seeds, the threshold, the rules, the same maps with any number of threads) |
| Land change: dynamic variables, regional demands, presets, projections, scenarios, calibration | unit tests: the pieces they assemble are compared above |

## Details by module

Where the old guide kept a verification section per module, they are kept here. The wording is the library's; the scripts and
tests it names are in the [library's repository](https://github.com/sacridini/terraflux-py). The timings that went with them
are on the [speed page](/benchmarks/speed/).

### Landscape metrics in detail

[`tools/make_reference_lsm.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_lsm.py) runs
landscapemetrics ([`tools/lsm_reference.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/lsm_reference.R)) on 14
maps (its `landscape`, crops of `augusta_nlcd` and `podlasie_ccilc`, synthetic maps with NA cells, negative and large class
values, non-square cells, salt-and-pepper patterns, few distant patches with a non-integer cell size, a checkerboard, single
rows and columns, a single class) with 5 option sets, and stores every value in `tests/data/reference/lsm`.
`tests/test_lsm_reference.py` compares all of them; with `Rscript` available (`TERRAFLUX_RSCRIPT`, `TERRAFLUX_R_LIBS` for a
specific landscapemetrics library) it also runs landscapemetrics live on random maps with other option combinations.
`make_reference_lsm.py sample window` also stores `sample_lsm` (square, circle and polygon plots, with and without
`all_classes`, a CRS and boundary options) and `window_lsm` (square and circular windows) results, which
`tests/test_lsm_sampling.py` compares, plus live runs on random maps; sample plots and windows are also checked against
`Landscape` objects built on the same cells. `tests/test_lsm_tiled.py` checks that out-of-core results (`block_rows`) are
identical to the in-memory ones on every stored reference map and on files with many blocks, patches across every seam, deep
`edge_depth` context, several data types and nodata values. All comparisons with landscapemetrics require the same bits, except
for the three families above (relative 1e-10); `tests/test_lsm_rmath.py` checks the arithmetic itself against R on 600 stored
vectors and the `lm()` slope on 400 regressions, 166 of them collinear (`make_reference_lsm.py rmath`).
`tests/test_lsm_extras.py` checks the extras against numpy and pylandstats, the names and the threads.
`make_reference_lsm.py utils` stores the results of the utilities
([`tools/lsm_utils_reference.R`](https://github.com/sacridini/terraflux-py/blob/main/tools/lsm_utils_reference.R)) on 8 maps and
option sets, compared by `tests/test_lsm_utils.py`, which also runs them live on random maps. `tests/test_lsm.py` checks values
computed by hand, the labels against SciPy, the parallel seams, input handling and metric selection; `enn` against SciPy's exact
Euclidean distance transform and `circle` against an exhaustive search over the circles through two or three hull points.

### Habitat graphs in detail

* [`tools/make_reference_conn.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_conn.py) runs
  Makurhini's `MK_dPCIIC` (Makurhini 3.0) on its example `habitat_nodes` (404 patches, centroid distances) and three synthetic
  scenes (random points, non-metric costs, separate clusters) with IIC and PC, several distances, probabilities, `pij_min` and
  landscape areas (`tests/data/reference/conn/makurhini.csv.gz`). `tests/test_conn_reference.py` compares the unrounded values
  of its internal steps to 1e-12 and reproduces its rounded output exactly.
* The same script builds Conefor Sensinode 2.2's own index code (`landscape.cpp`, with
  [`tools/conefor/conefor_harness.cpp`](https://github.com/sacridini/terraflux-py/blob/main/tools/conefor/conefor_harness.cpp)
  replaying its node-removal loop) with g++ and stores every index and node importance of the synthetic scenes
  (`conefor.csv.gz`). Conefor computes in single precision: the values agree to its precision (1e-5 relative for the indices).
* `make_reference_conn.py spatial` stores `nodesfile`, `distancefile` and `MK_dPCIIC` on Makurhini's example raster
  (`habitat_nodes_raster`, 398 patches) and on 53 synthetic polygons (multipart, with holes, U-shaped, touching and
  overlapping) in `tests/data/reference/conn/spatial`. `tests/test_conn_spatial.py` finds the same bits for the node attributes
  and the centroid distances (rasters and polygons) and for the polygon edge distances; raster edge distances agree to the last
  bit (GEOS measures between terra's polygon vertices: 1 in 80,000 values differs by 1 ulp). It also checks raster edge
  distances against brute force (non-square cells too) and GEOS on the cell polygons.
* Least-cost distances (`resistance_matrix`, 8 and 4 directions, `1/mean(x)`; the polygons over a resistance with NA walls,
  whose unreachable pairs take the centroid distance) are gdistance's bits, and `MK_dPCIIC` with them is reproduced exactly;
  `tests/test_conn_spatial.py` also checks them against SciPy's Dijkstra on the graph built from gdistance's formulas.
* `make_reference_conn.py commute` stores `distancefile(type = "commute-time")` on the polygons (8 and 4 directions, both cost
  functions) and on the example raster over `resistance_matrix`; `tests/test_conn_spatial.py` matches them to 1e-11 (the same
  pairs) and checks commute times against the Laplacian's pseudo-inverse.
* `make_reference_conn.py links` runs `MK_dPCIIC_links` (removal, and changes of 40 pairs brought closer or moved apart) and
  `MK_dPCIIC` with `restoration` on the three synthetic scenes (`makurhini_links.csv.gz`): the same rounded values, restoration
  gains to 1e-12. The Conefor driver also replays its "adding nodes" mode (the last five nodes of each scene), matched to its
  precision for every index.
* `make_reference_conn.py deca` runs `MK_dECA` on three times of the polygons (PC and IIC, two distances) and of the example
  raster (`deca_steps.csv`, `makurhini_deca.csv.gz`): `dA` has the same bits and the other values agree to 1e-13.
* `make_reference_conn.py protconn` runs `MK_ProtConn` on 36 synthetic protected areas and two regions (without and with
  transboundary zones of both types, and ProtConn[Bound]) with its inner steps: fed Makurhini's own nodes and distances,
  `protconn_indicators` gives `get_protconn_grid`'s bits; terraflux's own nodes have the same areas (1e-14) and its rounded
  ProtConn table is Makurhini's, value for value.
* `make_reference_conn.py grids` stores `make_grid` (hexagons and squares: the same coordinates), `MK_Connect_grid` (ProtConn
  with its default and with `pij_min = 0`, and PC), `MK_ProtConn_raster` (three transboundary settings) and `MK_dECA_grid`; all
  are reproduced after Makurhini's rounding (the dECA grid to 1e-12).
* `make_reference_conn.py centrality` runs `MK_RMCentrality` (binary and weighted, with and without a probability) on the
  polygons and the example raster (`makurhini_centrality.csv.gz`): degree, BWC and cluster are the same, the other values agree
  to 1e-12 (eigenvectors: ARPACK against LAPACK).
* `make_reference_conn.py betweenness` runs the command line of Conefor 2.7.1 (`CONEFOR_EXE`) for BC, BC(IIC) and BC(PC) on the
  synthetic scenes (`conefor_betweenness.csv.gz`), matched to the 6 digits it prints. Conefor keeps the probabilities in single
  precision: where a path is shorter than the direct link by less than float precision (near-collinear points, 1e-10 to 8e-8
  relative) it counts the path; on the Euclidean scenes BC(PC) is 0 in exact arithmetic, and the tests check that every
  difference is such a tie.
* `make_reference_conn.py focal` runs `MK_Focal_nodes` (IIC and PC, one and two distances and search buffers, `pij_min`) on the
  polygons and the ProtConn areas (`makurhini_focal.csv.gz`): every neighbourhood is the same, the indices agree to 1e-12 and the
  importances after Makurhini's 7-digit rounding. (Makurhini fails with a `threshold` there.)
* `make_reference_conn.py samc` runs `MK_dPC_SAMC` on the polygons over their resistance (with an absorption raster written from
  them), 4 and 8 directions, sum and mean, 10 or 3 vertices or a fifth of them, and stores samc's settlement probabilities: they
  agree to 1e-14, and the PC importances are Makurhini's after its rounding.
* `tests/test_conn.py` checks every index and importance against a brute-force recomputation of the graph without each node (and
  with its attribute set to 0), under both conventions, and the input kinds, pair tables, Conefor files and thread independence.

### Circuit connectivity in detail

`python tools/make_reference_omniscape.py advanced` (Julia with Circuitscape 6.1, see the script) runs
`compute_omniscape_current` with the `cholmod` and the default `cg+amg` solvers on synthetic rasters: NoData islands, finite and
direct grounds, components without a source or a ground, 4 and 8 neighbours, an Omniscape window and conductances over six
orders of magnitude (`tests/data/reference/circuit`). The same cells carry current and the values agree with CHOLMOD to 1.2e-13
of the largest current; the iterative `cg+amg` itself is 2e-7 away from both.

## Known differences

!!! warning "Where the results differ"
    - **Euclidean distance, distance to resistance, influence zones and proximity** depend on IDL's `MORPH_DISTANCE`, which
      does not always pick the nearest object near the boundaries between the zones of different objects (about 1 % of the
      pixels of the test cases). Terraflux's distances are exact Euclidean distances, so these maps can differ from
      GuidosToolbox's in detail; influence zones and proximity are marked experimental.
    - **MSPA Tiling** reproduces GuidosToolbox's tiling exactly, including its approximation near the tile seams when objects
      are larger than the overlap. A map that fits in one tile gets the MSPA of the whole map (GuidosToolbox drops the first
      `overlap / 2` columns).
    - **Zonation 5's curves** (performance curves, AUC, `features_info`, histograms and flexibility maps) come from the order of
      its second-to-last sort, while its rank map comes from the last one. Terraflux's describe the rank map it returns; they
      are compared with z5 evaluating that rank map (`LOAD` mode). Flexibility maps are identical where the two orders give the
      same curves, and otherwise compared with a direct reproduction of Zonation's algorithm.
    - **Zonation 5 on several threads** adds up the layers per thread, and its results can differ from its run on one thread;
      Terraflux's do not depend on the threads and equal z5's on one thread.
    - **Species distributions** keep the R packages' results except where they err (each case listed in
      [IDEAS.md](https://github.com/sacridini/terraflux-py/blob/main/IDEAS.md) of the library and in the functions' notes). Three
      of the corrections change values people compare with R: the continuous Boyce index drops ecospat's "+1" on the last
      window (which made the index depend on the scale of the predictions), country checks use Natural Earth's `iso_a3_eh` (with
      `iso_a3`, France and Norway are "-99"), and MOP's Mahalanobis distance uses the reference's covariance (mop used that of
      the cells tested).
    - **Dinamica EGO's destinations** (`destinations`, of its road constructor): Dinamica gives each quadrant `n // q` rows or
      columns, so the last `n % q` never get a destination, and rounds its draw to an integer of the summed attraction, so the
      first cell of a quadrant gets half its share and a draw past the end takes the quadrant's last cell, even of no
      attraction. Terraflux's quadrants cover the map and its draw follows the attraction exactly.
    - **Off Windows**, the arctangent of the slopes of `surface.terrain` is the C library's, as terra's is on Windows: R takes
      it from Windows' UCRT, whose code is not public, and other C libraries (glibc) differ from it in the last bit. The slopes
      then differ from R's by one unit in the last place (two in degrees). The SWAT+ port calls the C library's exp, log and
      powers as SWAT+ does, and carries their last bits through years of daily state: off Windows its results are within 1e-4 of
      each quantity's scale of SWAT+'s on Windows (2.4e-5 at most in the test cases). Everything else gives the same bits on
      Windows and Linux: Terraflux carries its own versions of the functions whose bits matter (log, sqrt, sin, cos, atan2, exp
      and powers as mingw-w64 computes them on the x87 unit, and the UCRT's hypot).

Three more limits of what these pages claim:

- **Corrected originals.** Where an original errs, Terraflux is compared with the original with its errors corrected, not with
  the original as released ([where the original errs](#where-the-original-errs)).
- **Not run against the original.** The tools in [checked in other ways](#checked-in-other-ways) are not compared with a run of
  the original: GuidosToolbox's network components, entropy and restoration status, cost distance and corridors, zonal
  statistics and clipping, Dinamica's Expander, Patcher and continuous Patcher, hilltops, PLUS's CARS, and the dynamic
  variables, projections and scenarios of land change.
- **Two hops for the application.** The application is compared with the library, which is compared with the original, so the
  application's agreement with an original is the library's, passed through the application's tests
  ([the application's own tests](/benchmarks/application/)).
