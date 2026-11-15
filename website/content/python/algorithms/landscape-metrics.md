---
title: Landscape metrics
description: "The FRAGSTATS metrics as landscapemetrics implements them: how the engine works, the options, the 133 metrics, the extras and the exact arithmetic."
order: 80
---

The same in the app: [What are landscape metrics?](/tutorials/landscape-metrics/what-are-landscape-metrics/), [Patch and class metrics](/tutorials/landscape-metrics/patch-and-class-metrics/) and [Landscape level and diversity](/tutorials/landscape-metrics/landscape-level-and-diversity/). The same as a worked tutorial in Python: [Landscape metrics in Python](/tutorials/landscape-metrics/landscape-metrics-in-python/). API: [`tf.Landscape`][terraflux.Landscape], [`tf.landscape_metrics`][terraflux.landscape_metrics] and [`tf.list_metrics`][terraflux.list_metrics] on the [landscape metrics](/python/api/landscape-metrics/) page. Plots, windows, time series and signatures are on [the next page](/python/algorithms/landscape-metrics-extras/); how it is checked, with timings, is in [how each tool is checked](/about/validation/#landscape-metrics-in-detail).

The examples use a crop of the CORINE land-cover map that ships with the library, written to a file as `landcover.tif`:

```python
import numpy as np
import terraflux as tf

landcover = tf.load_raster(tf.example_data("clc"))[:600, :600]
tf.write_raster(landcover, "landcover.tif")
tf.write_raster(landcover, "t1.tif")          # two dates for the series below (the same map, to run it)
tf.write_raster(landcover, "t2.tif")
tf.write_raster(landcover, "huge_landcover.tif")   # stands in for a raster larger than memory
```

The patch-, class- and landscape-level metrics of FRAGSTATS as implemented by the R
package [landscapemetrics](https://r-spatialecology.github.io/landscapemetrics/):
same metric names (`lsm_p_area` → `"area"` at the patch level, `lsm_c_ca` →
`"ca"`, …), same options, same patch ids and the same values: the same bits for all
metrics but `circle`, `gyrate` and rare `enn` ties, which agree to a relative
1e-10 (see *Bit-identical arithmetic* below). The FRAGSTATS statistics that
landscapemetrics lacks (`_am`, `_md`, `_ra`) are available as extras.

## How it works

`Landscape(data, ...)` maps the class values to dense indices once. On first use the
C++ core then

1. labels the patches of **all classes in one pass**: the map is cut into strips of
   64 rows labelled in parallel (union-find), and the pieces are joined across the
   strip seams;
2. accumulates, in the same pass, every per-patch statistic the metrics need: cells,
   cell sides towards other cells (left/right and top/bottom separately, for
   non-square cells), like adjacencies (orthogonal and diagonal), coordinate sums;
3. orders the patches like landscapemetrics (by class, then by first cell in
   column-major order), so patch `id`s are the same;
4. counts, in one more parallel pass, the 4-neighbour adjacencies between classes,
   NA and the landscape boundary (plus the diagonal ones when `neighbourhood=8`).

Every metric is a function of these tables, so asking for many metrics costs little
more than asking for one. The results are cached in the object.

## Usage

```python
import terraflux as tf

lc = tf.Landscape("landcover.tif")          # path, DataArray, numpy or masked array
lc                                           # Landscape(shape=..., resolution=..., classes=[...])

lc.patch_metrics(["area", "perim", "shape"])  # class, id, area, perim, shape
lc.class_metrics()                            # every class-level metric
lc.landscape_metrics(["shdi", "contag", "ed", "lsi"])
lc.metrics(level=["class", "landscape"], type="aggregation metric")   # long format
lc.metrics(what=["lsm_c_ca", "lsm_l_ta"])     # landscapemetrics function names

lc.labels()          # patch ids 1..n (0 = NA), same grid as the input
lc.to_raster("area") # patch metric painted on the patches (spatialize_lsm)

tf.landscape_metrics(["t1.tif", "t2.tif"], level="landscape")   # adds a `layer` column
tf.list_metrics(level="class", type="-core area metric")        # "-" excludes
```

Wide tables (`patch_metrics`, `class_metrics`, `landscape_metrics`) have one column
per metric; the long table (`metrics`, `landscape_metrics()`) has the columns of
`calculate_lsm`: `level`, `class`, `id`, `metric`, `value` (plus `layer`), sorted the
same way. Both are pandas DataFrames, ready for `to_csv` / `to_parquet`.

## Input

* Cell size from the raster (`res=` for numpy arrays, default 1); areas are in
  hectares and lengths in map units, as in landscapemetrics. A geographic CRS gives
  a warning (degrees).
* NA: the raster's nodata value, `nodata=`, masked cells and NaN.
* Float maps are truncated to integers (like `terra::as.int`), with a warning when
  values are not whole numbers.

## Options (landscapemetrics arguments)

| option | default | used by |
|---|---|---|
| `directions` | 8 | patch connectivity (all patch-based metrics) |
| `count_boundary` | `False` | `te`, `ed`: landscape boundary and NA count as edge |
| `consider_boundary`, `edge_depth` | `False`, 1 | core-area metrics |
| `cell_center` | `False` | `gyrate`: centroid moved to the nearest cell |
| `classes_max` | `None` | `rpr` (NA without it) |
| `neighbourhood`, `ordered`, `base` | 4, `True`, `"log2"` | `ent`, `condent`, `joinent`, `mutinf`, `relmutinf` |

`calculate_lsm` also uses `neighbourhood` for `te`, `ed`, `iji` and `contag`,
unlike the functions `lsm_l_te`, `lsm_c_iji`, `lsm_l_contag`, … (always 4
neighbours). terraflux follows the individual functions; with the default
`neighbourhood = 4` both agree.

## Available metrics

| level | metrics |
|---|---|
| patch | `area`, `perim`, `para`, `shape`, `frac`, `gyrate`, `contig`, `circle`, `enn`, `core`, `ncore`, `cai` |
| class | `ca`, `pland`, `np`, `pd`, `lpi`, `te`, `ed`, `ai`, `clumpy`, `cohesion`, `division`, `mesh`, `split`, `iji`, `lsi`, `nlsi`, `pladj`, `pafrac`, `tca`, `cpland`, `ndca`, `dcad`, and `_mn`/`_sd`/`_cv` of `area`, `para`, `shape`, `frac`, `gyrate`, `contig`, `circle`, `enn`, `core`, `cai`, `dcore` |
| landscape | `ta`, `np`, `pd`, `lpi`, `te`, `ed`, `ai`, `cohesion`, `contag`, `division`, `mesh`, `split`, `iji`, `lsi`, `pladj`, `pafrac`, `pr`, `prd`, `rpr`, `shdi`, `shei`, `sidi`, `siei`, `msidi`, `msiei`, `ent`, `condent`, `joinent`, `mutinf`, `relmutinf`, `tca`, `ndca`, `dcad`, and the same `_mn`/`_sd`/`_cv` |

`_sd` is the sample standard deviation and `_cv` = `_sd / _mn · 100`, as in
landscapemetrics (NA for a single patch).

`enn` is the distance between the centres of the nearest cells of the patch and of
another patch of the same class; it is NA when the class has a single patch, and
so are the `enn_mn/sd/cv` of that class and of the landscape (NA propagates, as
in landscapemetrics). When every class has a single patch, landscapemetrics
returns one class-level row without class for each `enn_*`; `metrics()` does the
same, while `class_metrics()` keeps one NA row per class. The engine sorts the
patch edge cells of each class in Morton order, builds one quadtree-like tree per
class whose nodes know when all their cells belong to one patch, and searches the
nearest cell of another patch leaf by leaf, in parallel; a 5×5 raster look-up
and the smallest possible distance between two patches settle most cells without
a search. The distances are computed as in landscapemetrics and are identical.

`circle` is `1 - area / area of the smallest circumscribing circle`. As in
landscapemetrics the circle encloses the corners of the patch cells in its first
and last rows and columns, found with Welzl's algorithm. landscapemetrics shuffles
the points at random, so its values can change in the last bit between runs;
terraflux uses a fixed shuffle per patch (repeatable results, same values within
1e-15). landscapemetrics refuses non-square cells; terraflux uses the real cell
width and height.

Core areas follow landscapemetrics' `get_boundaries`: a cell is core when its
city-block distance to the nearest cell of another class or NA (and to the map edge,
unless `consider_boundary=True`) exceeds `edge_depth`; `edge_depth=0` behaves like 1.
The core engine computes that distance in two parallel separable passes instead of
repeated erosions. As in landscapemetrics, `dcore_mn/sd/cv` summarise the *number* of
core areas per patch (`ncore`), not their sizes.

## FRAGSTATS extras, metric names and threads

```python
lc.class_metrics(["area_am", "gyrate_am", "shape_md", "enn_ra"])   # FRAGSTATS statistics
lc.metrics(level="class", metric="shape", extras=True)              # shape_mn ... shape_ra
tf.list_metrics(extras=True)                                        # 211 metrics ("extra" column)
lc.class_metrics(["CA", "AREA_MN", "total_area", "shape_index_am"]) # FRAGSTATS / pylandstats names
lc.landscape_metrics(["SHDI", "shannon_diversity_index", "contagion"])
tf.Landscape("landcover.tif", n_threads=4)                          # limit the cores
```

**Extras.** For every patch metric with summary statistics (`area`, `cai`, `circle`,
`contig`, `core`, `dcore`, `enn`, `frac`, `gyrate`, `para`, `shape`) the class and
landscape levels also have the area-weighted mean `_am` (Σ x·a / Σ a, as R's
`weighted.mean`), the median `_md` and the range `_ra` of FRAGSTATS and pylandstats, and
`perim` gets `_mn`, `_sd`, `_cv`, `_am`, `_md`, `_ra` (pylandstats). `gyrate_am` is
FRAGSTATS' correlation length. `dcore` follows landscapemetrics (number of core areas per
patch), weighted by patch area. As for `_mn`, NA propagates (pylandstats skips NA at the
landscape level). These 78 metrics are not part of landscapemetrics, so they are only
selected with `extras=True` (or by name, or in `what`); without it every selection gives
the same rows as `calculate_lsm`. They were checked against numpy and against
pylandstats' `_mn`/`_am`/`_md`/`_ra` of `area`, `perimeter`, `fractal_dimension` (and
`euclidean_nearest_neighbor`, `core_area` where pylandstats' patch values agree).

**Names.** Wide tables and `to_raster` accept FRAGSTATS abbreviations in capitals
(`"SHDI"`, `"AREA_MN"`, `"ENN_AM"`), pylandstats' method names (`"total_area"` → `ca` at
the class level and `ta` at the landscape level, `"proportion_of_landscape"`,
`"shape_index_am"`, `"disjunct_core_area_mn"`, `"continguity_index"`, …) and FRAGSTATS'
full names (`"shannon_diversity_index"`, `"effective_mesh_size"`, …); the columns keep
the names as asked. The `metric=` filter accepts family aliases (`"shape_index"`).

**Threads.** `n_threads` (in `Landscape`, `landscape_metrics`, `sample_metrics`,
`window_metrics`, ...) limits the C++ core (default: most of the cores); the results do
not depend on it.

**FRAGSTATS-only metrics** (extras; landscapemetrics has none of them):

```python
contrast = {(1, 2): 0.2, (1, 3): 0.7, (2, 3): 0.5}        # or a class x class DataFrame
lc = tf.Landscape("landcover.tif", search_radius=500, connect_threshold=100,
                  contrast=contrast, boundary_contrast=0.0, similarity={(1, 2): 0.5})
lc.patch_metrics(["prox", "simi", "econ"])
lc.class_metrics(["connect", "cwed", "teci", "prox_mn", "econ_am"])
```

| metric | level | definition |
|---|---|---|
| `prox` | patch (+ stats) | Σ over the other patches of the class within `search_radius`: area (m²) / distance² |
| `simi` | patch (+ stats) | the same over the patches of all classes, weighted by the `similarity` of the classes (1 within a class; without a table `simi = prox`) |
| `connect` | class, landscape | % of the pairs of patches of a class within `connect_threshold` |
| `econ` | patch (+ stats) | Σ edge length × contrast / perimeter × 100 (edges to NA and the map edge weigh `boundary_contrast`) |
| `cwed` | class, landscape | Σ edge length × contrast / total area (m/ha); NA and map-edge edges count with `count_boundary` |
| `teci` | class, landscape | Σ edge length × contrast / Σ edge length × 100, all edges (FRAGSTATS' E*) |

Distances between patches are the shortest distances between their cell centres, as
for `enn` and in FRAGSTATS; the neighbour's whole area counts in `prox`/`simi`. Without
`search_radius`, `connect_threshold` or a `contrast` table the metrics are NaN; contrast
pairs missing from the table give NaN too. The pairs of patches within reach come from a
range search over the boundary cells, one focal patch at a time (2000×2000 with 223,000
patches: `prox` 0.2 s with a 3-cell radius, 2 s with 30 cells). These metrics need the
raster, except `cwed` and `teci`, which also work out of core. There is no FRAGSTATS to
compare with here: they are tested against direct implementations of the definitions.

**`convention="fragstats"`** changes five landscapemetrics conventions, all checked
against pylandstats or brute force:

- with non-square cells the sides between left/right neighbours are as long as the cell
  *height* (the physical perimeter; landscapemetrics multiplies them by the width), for
  `perim`, `te`, `ed` and everything built on them;
- `dcore_*` summarise the sizes (ha) of the disjunct core areas of the class or landscape
  (`dcore_am` weighted by those sizes), not the number of core areas per patch;
- `shape` is FRAGSTATS' raster SHAPE, the perimeter over the smallest perimeter of as many
  cells (in cell sides), instead of .25 p / sqrt(a); cells that are not square keep
  .25 p / sqrt(a), and `lsi` becomes .25 E / sqrt(A) with the physical edge lengths
  (pylandstats; FRAGSTATS needs square cells);
- `_sd` and `_cv` use the standard deviation over n, FRAGSTATS' (landscapemetrics uses
  R's `sd()`, over n - 1; one patch gives 0 instead of NA);
- the `circle` encloses the whole patch: landscapemetrics' circle only encloses the
  corners of the cells in the first and last rows and columns, which can miss the
  corners that decide it. Here the first and last cell of the patch in every row are
  gathered in parallel, reduced to their convex hull on the integer grid (exact), and
  Welzl's algorithm runs on the hull (also out of core, with the same values;
  `circumscribing_circles` follows the convention too).

## Bit-identical arithmetic

How each platform gets R's bits (all checked by the same tests):

| Build | 80-bit sums (`sum`, `mean`, `var`) | `log`, vector `sqrt` |
| --- | --- | --- |
| Windows, MSVC x64 (the wheels) | x87 unit, MASM (`x87_msvc.asm`) | x87 unit, MASM |
| Linux / macOS on x86-64 (gcc, clang) | native `long double` | x87 inline assembly |
| Windows, MinGW gcc | emulated (`SoftExtended`): Windows runs the x87 at 53 bits | x87 inline assembly |
| ARM64 (Apple M1–M6, Linux aarch64) | emulated (`SoftExtended`): the same bits | the system library: may differ in the last bit |

On ARM64 the sums still give R's bits (R's reference values come from R on x86-64
Windows); only metrics that take a logarithm or a square root may differ in the last bit
there. The MASM file is only assembled by MSVC on x86-64; every other build uses the C++
code. `terraflux._core.x87_math` tells which kind of build is running: the reference
tests compare bit for bit where it is true and allow a few units in the last place
elsewhere (`tests/conftest.py`).

R computes `sum()`, `mean()` and `var()` in `long double`, the 80-bit x87 type with a
64-bit mantissa (R on Windows is built with gcc/mingw-w64). MSVC has no such type, so
`src/cpp/lsm/extended.hpp` emulates it with integer arithmetic, correctly rounded (half to
even); it was checked against gcc's native type on 40 million random operations, and
gcc/clang builds use the native type. The metrics then follow landscapemetrics' R code
operation by operation: `mean()` sums, divides and adds the mean residual; `sd()` squares
the deviations from the mean rounded to double; `sum()` inside `aggregate()` runs per
class in patch order; `weighted.mean`, `median` and the range are computed the same way
for the extras.

R's `log()` and the `sqrt()` of a vector come from mingw-w64's math library, which runs
them on the x87 unit (glibc's `fyl2x`/`fyl2xp1` code for `log`) and rounds twice, so they
can differ in the last bit from a correctly rounded result; `rmath.hpp` does the same
(with a small MASM routine under MSVC, which has no x64 inline assembly). R's byte-code
interpreter computes the `sqrt` of a single number inline and correctly rounded (the one
inside `sd()`, `cohesion`'s factor, the shape of a single patch), and `x^-1` is `powl`.
`pafrac` is 2 over the slope of `lm(log(area) ~ log(perimeter))`: `src/cpp/lsm/rlm.cpp`
runs what `lm()` runs, LINPACK's `dqrdc2` (Householder QR with R's limited pivoting,
tolerance 1e-7) and `dqrsl` over R's reference BLAS (`ddot`, `daxpy` and `dscal` unrolled
as in R, LAPACK 3.12's `dnrm2`), so the slope is R's to the last bit and NA exactly when R
drops the perimeter as collinear.

Values that stay within 1e-10 instead of identical: `circle` (landscapemetrics shuffles
Welzl's points at random, so its own values change between runs), `gyrate` (R sums the
coordinates and distances cell by cell in extended precision) and the `enn` of rasters
whose cell sides differ by rounding only (R's pruned search can settle on another pair of
cells at the same distance).

landscapemetrics' own quirks are reproduced: patch perimeters treat cell sides that
differ by rounding only as equal (`all.equal`), and `window_metrics` computes `shdi` and
`shei` from 8-connected patch areas whatever `directions` is, as `window_lsm` does (their
functions hard-code it; `calculate_lsm` hides it).

## Rasters larger than memory (`block_rows`)

```python
lc = tf.Landscape("huge_landcover.tif", block_rows="auto")   # or a number of rows
lc.metrics(level=["class", "landscape"])
tf.landscape_metrics("huge_landcover.tif", block_rows=2048, what=["lsm_c_ca", "lsm_l_shdi"])
```

With `block_rows` the file is read by blocks of rows (rounded up to a multiple of 64;
`"auto"`: 64 rows per thread, at least 4 and at most 32 million cells) and only one
block, the patch tables and the class adjacencies are in memory. The results are
**identical to the in-memory ones** (all 133 metrics, every option), so they match
landscapemetrics as well.

1. Pass 0 finds the class values (a count of the raw values for 8/16-bit files).
2. Pass 1 labels every block with the in-memory engine. The pieces of the patches cut by
   the block seams are joined with a union-find, and what the block labelling got wrong
   at the seams is corrected: cell sides towards the next block, cell pairs across the
   seam and, with 4 directions, diagonal pairs whose pieces turn out to be one patch.
   Class adjacencies are counted with one row of context above and below. This gives
   the patch tables (same patches, same ids) and every metric but the next four.
3. A detail pass, run only when they are asked for, computes core areas (distance to
   the patch edges with `edge_depth` rows of context; core areas joined across seams
   like the patches), `gyrate` (plus a pass for the ties with `cell_center`), `circle`
   (cells on the edges of each patch's bounding box) and `enn`.

Blocks start at multiples of 64 rows, the height of the strips the in-memory engine
labels in parallel, so even the floating-point sums (`gyrate`) are made in the same
order. Memory grows with the number of patches (about 70 bytes per patch piece while
reading, plus the per-patch values the metrics keep) and, for `enn` only, with the
number of patch edge cells of the classes with several patches (about 24 bytes each):
enn needs every edge cell to find nearest neighbours. `labels()` and `to_raster()` are
not available out of core. GeoTIFF blocks are decoded on the same threads
(`GDAL_NUM_THREADS`).

## References

- Hesselbarth, M. H. K., Sciaini, M., With, K. A., Wiegand, K., & Nowosad, J. (2019). landscapemetrics: an open-source R tool to calculate landscape metrics. *Ecography*, 42(10), 1648–1657. https://doi.org/10.1111/ecog.04617

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
