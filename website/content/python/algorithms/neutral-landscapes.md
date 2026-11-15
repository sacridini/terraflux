---
title: Neutral landscape models
description: "The neutral landscape models of NLMR, their R and C++ random numbers reproduced bit for bit, and the landscapetools utilities."
order: 110
---

The same in the app: [Neutral landscape models](/tutorials/neutral-landscapes/neutral-landscape-models/) and [Neutral landscapes in a study](/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/). The same as a worked tutorial in Python: [Neutral landscapes in Python](/tutorials/neutral-landscapes/neutral-landscapes-in-python/) (a gallery, habitat maps, a null-model test of the example forest's aggregation). API: the [neutral landscape models](/python/api/nlm/) page. Checked against NLMR 1.4.0 with the same seeds: identical bits (see [how each tool is checked](/about/validation/#results-by-tool)).

`tf.nlm` simulates neutral landscapes, the null models of landscape ecology, as
the R package NLMR does, and returns `xarray.DataArray`s on NLMR's grid (origin
0, 0; `resolution` cell size). With the same `seed` (NLMR's `user_seed`) each
model gives NLMR's landscape bit for bit: R's generator (`set.seed`, `runif`,
`rnorm` by inversion, `sample`) and the libstdc++ distributions of NLMR's C++
code (`std::mt19937`) are reproduced. `seed=None` draws a seed and stores it in
`attrs["seed"]`.

```python
import terraflux as tf

r = tf.nlm.random(200, 150, seed=42)                     # uniform values, rescaled to 0-1
p = tf.nlm.percolation(200, 150, prob=0.4, seed=1)        # 0/1 (uint8)
g = tf.nlm.planar_gradient(200, 150, direction=45)        # direction=None: drawn from the seed
e = tf.nlm.edge_gradient(200, 150, seed=3)
d = tf.nlm.distance_gradient(200, 150, origin=(10, 20, 30, 40))
classes = tf.nlm.classify(r, weighting=[0.5, 0.3, 0.2])   # int32 classes 1..3 (0 = missing)
jenks = tf.nlm.classify(r, n=4)                           # Jenks natural breaks
x = tf.nlm.rescale(r.values * 10)                           # NLMR's util_rescale
m = tf.nlm.mpd(257, 257, roughness=0.5, seed=1)           # diamond-square (NLMR's grid size)
n = tf.nlm.neigh(200, 150, p_neigh=0.7, p_empty=0.1, categories=4, seed=2)
rc = tf.nlm.random_rectangular_cluster(200, 150, minl=5, maxl=20, seed=3)
cl = tf.nlm.random_cluster(200, 150, p=0.4, ai=[0.3, 0.3, 0.4], seed=4)
cu = tf.nlm.curds([0.5, 0.4, 0.6], [8, 8, 8], wheyes=[0.1, 0.1, 0.1], seed=5)  # 512 x 512
gf = tf.nlm.gaussian_field(200, 150, autocorr_range=20, mag_var=5, nug=0.2, seed=6)
fb = tf.nlm.fbm(200, 150, fract_dim=1.2, seed=7)
mf = tf.nlm.mosaic_field(200, 150, n=20, seed=8)          # dict with collect=/infinit=
mt = tf.nlm.mosaic_tess(200, 150, germs=100, seed=9)
mg = tf.nlm.mosaic_gibbs(200, 150, germs=80, R=8, patch_classes=4, seed=10)
blend = tf.nlm.merge(g, [fb, gf], scaling=0.5)             # landscapetools' util_merge
habitat = tf.nlm.binarize(fb, [0.2, 0.4])                  # util_binarize: 1 matrix, 2 habitat
```

NLMR behaviours kept with `nlmr_compat=True` (the default where they apply):

- `distance_gradient`: `origin` is applied as `(first row, last row, first column,
  last column)`, as NLMR indexes the raster; `terra::distance` treats rasters of at
  most 360 x 90 cells as longitude/latitude (terra's default CRS for such an extent)
  and returns haversine distances in metres; larger rasters get GDAL's proximity
  algorithm (cell units, float32). `nlmr_compat=False` gives exact Euclidean
  distances times `resolution`, with `origin = (first column, last column, first
  row, last row)`.
- `mpd`: NLMR's grid is a square of side `max(nrow, ncol) + 1` (made odd) minus one
  cell on each side, so the result is square (a warning says when it differs from
  `nrow x ncol`); `nlmr_compat=False` builds a `2**k + 1` grid and crops it to the
  size asked for. The normal deviates are drawn in NLMR's order (a fresh
  `std::normal_distribution` per cell), their log/sqrt and each step's cells are
  computed in parallel (in order on a torus, where cells of one step can meet).
- `neigh`: categories are filled from `categories - 1` down to 0, a neighbour
  "holds a category" when it is non-zero, the edges wrap, `p_neigh`/`p_empty` are
  rounded to float as in NLMR's C++.
- `random_rectangular_cluster`: a count of empty cells replaces NLMR's scan of the
  whole matrix after every rectangle (same result).
- `random_cluster`: clusters numbered as `terra::patches` does (in the order of
  their first cell), the empty cells filled from the clustered map (not from the
  cells filled before them), ties by `sample()`, cells without typed neighbours by
  `sample(prob = ai)`; neighbours are counted in parallel, the draws follow in order.
- `curds`: `dplyr::sample_frac` = `sample.int(n, round(n * share))` over the cells in
  row-major order; 1 is habitat (curds), 0 matrix.
- `gaussian_field`, `fbm`: NLMR 1.4 disabled `nlm_gaussianfield` and `nlm_fbm` (only a
  message) and ships RandomFields-free methods as the internal `simulate_gaussian_field`
  (circulant embedding) and `simulate_fbm` (1/f spectrum); these are reproduced and
  rescaled as the previous `nlm_*` did. That takes R's `fft` (Singleton's algorithm,
  ported statement by statement; the columns' and rows' transforms run in parallel
  with the same bits), R's `exp` and `x^y` on Windows (mingw-w64's x87 `exp` and
  `powl`), R's vector `sqrt` and the 80-bit `mean`/`sd`.
- `mosaic_field`: spatstat's `rpoislinetess(4)` (R's `rpois`, `runif`, x87 `sin`/`cos`)
  and `rMosaicField`: the tiles are the cells of the line arrangement in the unit
  square in `chop.tess` order (below before above each line, lines in order), each
  pixel centre (`as.mask`'s grid) takes its tile's `rnorm` value; `infinit` is
  `log(exp(Z))` of `rLGCP`'s circulant embedding (`grid.prep`, `covariance.prep`,
  `generate.normals` with R's `fft`). Kept NLMR quirks: `n = 1` sums three fields
  (`for (i in 2:n)`), `collect` divides the i-th field by `sqrt(i)` and rescales all
  steps with the first step's range.
- `mosaic_tess`, `mosaic_gibbs`: the germs are `sf::st_sample`'s `runif` draws and
  spatstat's `rSSI` (blocks of proposals, rejection within `R`); NLMR gives the k-th
  value to the k-th Voronoi polygon in GEOS's order (`st_voronoi(st_union(...))`,
  `dTolerance = 0.1` for `mosaicgibbs`, where the value is the k-th germ's class, not
  the class of the germ inside the polygon). Terraflux gets that order from GEOS
  through shapely (GEOS 3.11 or later: older versions order the cells differently and
  warn; without shapely: a warning, each germ its own value), rasterises by
  nearest germ in C++ (the polygon containing each cell centre) and rounds to float32
  as `terra::rasterize`.
- `classify`: `sum`/`cumsum` of the weights in 80-bit precision as R; when all
  class boundaries are equal every cell becomes class 1; Jenks' breaks cost
  O(classes x cells^2) as in NLMR (parallel over the rows of each class column).

Reference: [`tools/make_reference_nlm.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_nlm.py) (NLMR 1.4.0 installed from a clone into a
private R library, R 4.4.2; the libstdc++ distributions checked against a probe
built with the Rtools g++), tests in `tests/test_nlm.py` and
`tests/test_nlm_rng.py`.

## References

- Sciaini, M., Fritsch, M., Scherer, C., & Simpkins, C. E. (2018). NLMR and landscapetools: an integrated environment for simulating and modifying neutral landscape models in R. *Methods in Ecology and Evolution*, 9(11), 2240–2248. https://doi.org/10.1111/2041-210X.13076

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
