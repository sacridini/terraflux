---
title: "Landscape metrics: plots, windows and signatures"
description: "Sample plots and moving windows, several rasters at once, time series and spatial signatures, the plots and the utilities of landscapemetrics."
order: 85
---

The same in the app: [Sampling and change](/tutorials/landscape-metrics/sampling-and-change/), [Moving windows](/tutorials/landscape-metrics/moving-windows/), [Landscape signatures](/tutorials/landscape-metrics/landscape-signatures/) and [Large maps](/tutorials/landscape-metrics/large-maps/). The same as a worked tutorial in Python: [Landscape metrics in Python](/tutorials/landscape-metrics/landscape-metrics-in-python/). API: the [landscape metrics](/python/api/landscape-metrics/) page. This page continues [landscape metrics](/python/algorithms/landscape-metrics/): the same `Landscape` object, its options and its metrics.

The examples use a crop of the CORINE land-cover map that ships with the library, written to a few files, points and polygons to sample, and a `Landscape` of it:

```python
import geopandas as gpd
import numpy as np
import terraflux as tf
from shapely.geometry import box

landcover = tf.load_raster(tf.example_data("clc"))[:600, :600]
for i, name in enumerate(["landcover", "t1", "t2", "t3", "a", "b", "lc2000", "lc2010", "lc2020", "map"]):
    later = landcover.copy()
    later.values[: 40 * i] = later.values[0, 0]    # a changing strip, so that the dates differ
    tf.write_raster(later, f"{name}.tif")
files, dates = ["lc2000.tif", "lc2010.tif", "lc2020.tif"], [2000, 2010, 2020]

xs, ys = landcover.x.values, landcover.y.values
points_xy = np.array([[xs[150], ys[150]], [xs[300], ys[350]], [xs[450], ys[200]]])
xy = points_xy
gdf_polygons = gpd.GeoDataFrame({"name": ["north", "south"]}, crs=landcover.rio.crs,
                                geometry=[box(xs[50], ys[250], xs[250], ys[50]), box(xs[300], ys[550], xs[550], ys[300])])
gdf = gdf_polygons

lc = tf.Landscape("landcover.tif")
tf.write_raster(lc.labels(), "patch_ids.tif")
```

## Sample plots, moving windows and several rasters

```python
# metrics of sample plots (sample_lsm): squares or circles around points, or polygons
tf.sample_metrics("landcover.tif", points_xy, size=500, shape="circle", level="landscape")
tf.sample_metrics("landcover.tif", gdf_polygons, plot_id=gdf_polygons["name"], all_classes=True)

# landscape-level metrics in a moving window (window_lsm): {metric: raster}
maps = tf.window_metrics("landcover.tif", window=21, what=["lsm_l_shdi", "lsm_l_ed"])
disk = np.hypot(*np.mgrid[-5:6, -5:6]) <= 5          # any odd-sized shape
maps = tf.window_metrics("landcover.tif", window=disk, metric="contag")

# several rasters, computed side by side
tf.landscape_metrics(["t1.tif", "t2.tif", "t3.tif"])
```

**`sample_metrics`** cuts every plot out of the raster as `terra::crop(mask = TRUE)`
does in `sample_lsm`: the plot's bounding box is snapped to the nearest cell edges
(one cell if it collapses), every cell the plot touches is kept (GDAL's
all-touched rasterization, ported to C++ with GDAL's rules and epsilons, so no GDAL
call per plot) and the others become NA. Point buffers are
landscapemetrics' squares (half side `size`) or 100-vertex circles (radius `size`).
Each plot is then a landscape of its own; all plots go to the C++ core at once and are
computed in parallel. The table has `calculate_lsm`'s columns plus `plot_id` and
`percentage_inside` (area of the plot's cells over the plot area, in %; the plot area
is geodesic when `y` has a CRS, like a GeoDataFrame, and planar otherwise, as
`terra::expanse(transform = TRUE)` does), ordered like `sample_lsm`. `all_classes`
adds NA rows for the classes of the whole raster missing in a plot, including a row
for class NA when the raster has NA cells, as landscapemetrics does. A plot outside
the raster gives NA metrics and 0 % (landscapemetrics stops with an error).

**`window_metrics`** computes landscape-level metrics for the window centred on every
cell (cells outside the raster or outside the window shape are NA), in parallel over
the rows; each window is a small landscape with the class values mapped to dense
indices once for the whole raster. `terra::focal` hands the window values row by
row but landscapemetrics fills its matrix column by column, so its windows are
transposed; terraflux transposes them too, with the cell width and height swapped,
so the patches come in R's order (which `pafrac`'s regression rounds by) and the
values are R's. Where `window_lsm` breaks the geometry terraflux keeps it:
rectangular windows (R reshapes them), cells that are not square (R keeps the cell
sides unswapped) and window shapes that are not symmetric (R masks the transposed
values with the untransposed shape). And
`window_lsm` fails for `gyrate_*` (it does not pass `cell_center`), which work here.

**`scale_sample`** runs `sample_metrics` for several buffer sizes (a `size` column),
ordered like landscapemetrics' `scale_sample` (plot, level, metric, class, id, size);
points only.

**`full_name=True`** (in `landscape_metrics`, `Landscape.metrics`, `sample_metrics`,
`scale_sample` and `extract_metrics`) adds `calculate_lsm`'s columns `name`, `type`
and `function_name` after `value` (categoricals; the FRAGSTATS extras have their own).
The names are landscapemetrics' own, typo included (`siei` is the "simspon's evenness
index"; `list_metrics(name=...)` takes both spellings). landscapemetrics'
`sample_lsm(full_name = TRUE)` loses `function_name` by dropping a column by position;
it is kept here.

**`return_raster=True`** in `sample_metrics` adds `raster_sample_plots`: each plot's
raster as `terra::crop(mask = TRUE)` returns it (a DataArray on the snapped window,
NaN outside the plot and on NA cells, the raster's CRS; `None` for a plot outside the
raster). The window's extent is terra's (`align` through terra's origin, then
`setWindow`), the same bits as a crop of the intact raster. terra's crop also rewrites
the extent of the raster it crops (its `removeWindow` rebuilds it with the window's
cell size), so in `sample_lsm` the plots after the first see an extent moved by a few
ulps; that side effect is not reproduced (the cells and values are the same).

landscapemetrics' public helpers:

```python
tf.construct_buffer(points_xy, "circle", 500)                    # GeoDataFrame of plots
tf.construct_buffer(points_xy, "square", 500, return_vec=False)  # R's vertex matrix
padded = tf.pad_raster("landcover.tif", -999, 2)   # 2 rows / columns of -999 around it
tf.unpad_raster(padded, 2)                         # and back
tf.raster_to_points("landcover.tif", return_NA=False)   # layer, x, y, z
tf.get_unique_values(["a.tif", "b.tif"])  # {"layer_1": values, "layer_2": values}
```

`construct_buffer` gives the plots `sample_metrics` builds (squares of half side
`size`, 100-vertex circles), with R's bits: the circle's `sin`/`cos` are the x87
`fsin`/`fcos` R runs through mingw-w64 (`np.sin` differs in the last bit at 5 of the 100
angles). `pad_raster` / `unpad_raster` return a DataArray for georeferenced input
(extent grown or shrunk by the cells, as `ext(x) + res(x) * n`; landscapemetrics fails
when the cells are not square, since terra recycles `c(xres, yres)` over the four
sides; terraflux grows each axis by its own cell size) and a numpy array otherwise;
`global_` is R's `global` (NA cells take the pad value too). `raster_to_points` gives
terra's cell centres and the values in row order. `get_unique_values` returns the sorted
distinct values of each layer (NA left out, doubles truncated), in parallel in C++.

**Zonal results and export.** `zonal_metrics(data, zones)` cuts the polygons out like
`sample_metrics` and returns a GeoDataFrame: the zones' geometry and attributes, one
column per landscape metric and one per class metric and class (`ca_11`, `pland_42`...),
and `percentage_inside`. `write_table(table, path)` writes any metric table by extension:
`.csv`, `.csv.gz`, `.parquet` (pyarrow; keeps the categorical and nullable columns,
compact for patch tables with millions of rows) or, for GeoDataFrames, `.gpkg`,
`.geojson`, `.shp`, `.fgb`. For maps with tens of millions of patches,
`Landscape.write_metrics(path, ...)` writes the table of `metrics(...)` (same rows and
types) one metric at a time and the patch metrics `chunk_rows` rows at a time, and
`Landscape.write_patch_metrics(path, metrics)` the wide patch table the same way, to
`.parquet` (one row group per chunk), `.csv` or `.csv.gz`, never building the whole
table: the 9 patch metrics of 9 M patches (81 M rows) go to Parquet in 8.0 s with
1.0 GB of extra memory, instead of 10.3 s and 6.1 GB through a DataFrame. The CSV
text is the same as `write_table`'s. `Landscape.to_raster(metric)` paints a patch metric on the
patches or a class metric on the cells of each class, several at once with a list, and
writes GeoTIFFs with `path=`. `batch("landscape_metrics", inputs, out, **options)` writes
one long table per raster (`<name>_landscape_metrics.csv`, or `.parquet` with
`table_format="parquet"`, streamed by `write_metrics`) plus the JSON record; with
`block_rows` the files are read by
blocks; `batch("window_metrics", ...)` writes one GeoTIFF per metric.

**Several rasters** (`landscape_metrics(list)`, and lists in `sample_metrics` /
`window_metrics`) are computed side by side when they are small and one after the
other, each in parallel, when they are large.

## Time series and spatial signatures (pylandstats-style)

```python
sta = tf.SpatioTemporalAnalysis(["lc2000.tif", "lc2010.tif", "lc2020.tif"], dates=[2000, 2010, 2020])
sta.landscape_metrics(["shdi", "ed", "np"])        # one row per date
sta.class_metrics(["pland", "np"])                  # (class, date) rows; absent class: 0 for area/count
sta.plot_metric("pland", classes="all")             # one line per class
tf.SpatioTemporalAnalysis(files, dates, zones=gdf)                   # per polygon: (zone, date) rows
tf.SpatioTemporalAnalysis(files, dates, points=xy, size=500)         # buffers around points
tf.SpatioTemporalAnalysis(files, dates, grid=1000)                   # 1 km grid (tf.grid_zones)

sig = tf.SpatialSignatureAnalysis("map.tif", grid=1000, landscape_metrics=["shdi", "ed"], class_metrics=["pland"])   # a grid of 1 km zones; or the files of several landscapes
sig.table                     # the signature matrix: rows = landscapes (or zones), columns = metrics
res = sig.pca()               # scores, loadings, explained_variance_ratio
labels = sig.cluster(3)       # k-means (k-means++, best of 10)
sig.cluster(3, method="ward") # hierarchical: ward, average, complete, single, centroid, median, weighted
nmf = sig.nmf(3)              # non-negative factors: scores (W), components (H), reconstruction_error
cg = sig.clustergram(range(1, 9), method="ward")   # labels and cluster means for each k
sig.plot_clustergram(cg)      # Schonlau's clustergram, as pylandstats' get_cgram draws it
sig.plot_pca(labels); sig.plot_cluster_metrics(labels)
tf.SpatialSignatureAnalysis(files, landscape_metrics=["shdi", "ed"])   # one landscape per file
```

The series are computed with `landscape_metrics` / `sample_metrics` (so they are the same
values). The signature matrix has one column per landscape metric and one per class
metric and class (`pland_11`); the PCA works on z-scores (population standard deviation,
as scikit-learn's `StandardScaler`), leaves out constant metrics and rows with NaN (with a
warning), and orients each component so that its largest loading is positive; k-means and
the hierarchical linkages (scipy, Euclidean, the tree cut into k clusters) run on the
z-scores or on the first principal components, and clusters are numbered by first
appearance. `nmf` scales each metric to [0, 1] (min-max) and factorises it into two
non-negative matrices with scikit-learn's method (NNDSVDa start, coordinate-descent/HALS
sweeps, Frobenius loss): the same loss as `sklearn.decomposition.NMF(init="nndsvda",
solver="cd")` to the stopping tolerance. `clustergram` gives, for each k, every row's
cluster and the mean of that cluster: its centre projected on the first principal axis of
the clustered data (`pca_weighted=True`) or averaged over the columns, the values the
clustergram package draws (pylandstats' `get_cgram`); `plot_clustergram` draws them the
same way. All agree with scikit-learn / clustergram, which are not needed.

## Plots

With matplotlib, like landscapemetrics' `show_*` (they
return a `matplotlib.figure.Figure`; the maps use the raster's coordinates):

```python
lc.show_patches()                     # all patches, each in its own colour
lc.show_patches("all", labels=True)   # one map per class, with patch ids
lc.show_cores(edge_depth=2)           # core (dark) and edge (light) cells per class
lc.show_lsm("shape", "all")           # a patch metric on the patches, with a colour bar
tf.show_correlation(lc.metrics(level=["patch", "class"]), method="spearman", labels=True)
tf.show_patches("landcover.tif", directions=4)    # rasters work too (Landscape options)
```

`classes` is `"global"` (one map), `"all"` (one per class) or class values;
`show_cores` defaults to `consider_boundary=True` as in landscapemetrics' plot.

## Utilities (landscapemetrics' `get_*`, `extract_lsm`, `check_landscape`, `calculate_correlation`)

```python
lc = tf.Landscape("landcover.tif")
lc.centroids()                          # class, id, x, y (get_centroids)
lc.centroids(cell_center=True)          # the patch cell nearest to it (ties: several rows)
lc.nearest_neighbours(return_id=True)   # class, id, dist, id_neighbour (get_nearestneighbour)
lc.circumscribing_circles("class")      # diameter and centre (get_circumscribingcircle)
lc.adjacencies(8, what="unlike")        # class x class cell pairs (get_adjacencies)
lc.boundaries(edge_depth=2, patch_id=True)   # get_boundaries of the patches
lc.patches()                            # {class: raster of patch ids} (get_patches)
lc.extract_metrics(points_xy, what=["lsm_p_area", "lsm_p_shape"])   # extract_lsm
tf.raster_boundaries("patch_ids.tif")   # get_boundaries on the values of any raster
tf.check_landscape(["a.tif", "b.tif"])  # crs, units, class, n_classes, OK
tf.calculate_correlation(tf.landscape_metrics(["t1.tif", "t2.tif", "t3.tif"]), method="spearman")
```

All give landscapemetrics' values, bit for bit (except the circles, randomised in
landscapemetrics), and follow its conventions:

* **Coordinates** are terra's: cell centres `xmin + (col + 0.5) · (xmax − xmin) / ncol`,
  the extent from the geotransform (`xmax = xmin + res · ncol`). A point on a cell edge
  belongs to the cell right of / below it, except on the last column / row (terra's
  `cellFromXY`); points outside or on NA give no rows in `extract_metrics`.
* **Centroids** are R's `mean()` of the cell centres in raster order (80-bit sums). With
  `cell_center`, the nearest cells are those at the smallest distance (x87 `sqrt`, as R
  computes a vector); ties come in the order R's `merge()` leaves them (its Shell sort).
* **Nearest neighbours**: the distance of each patch to the nearest patch of its class is
  the `enn` metric. With `return_id`, landscapemetrics gives each boundary cell (4
  neighbours) the first cell of another patch at its nearest distance that its sweep over
  the cells sorted by x meets; the patch lists the patches its cells meet at the patch's
  distance. terraflux applies the same rule to the candidates of a range search around
  each cell, so it does not slow down on long columns of cells.
* **Circles** enclose the corners of the cells in the first and last rows and columns
  of the patch (or class), as the `circle` metric; centres in map coordinates. Non-square
  cells work (landscapemetrics refuses them).
* **Adjacencies** count directed pairs (each pair twice) with 4, 8 or any neighbourhood
  matrix (1 = neighbour, 0 = focal cell; read transposed, as landscapemetrics does); NA
  cells are left out. `what="like"/"unlike"/"triangle"` keep the diagonal / the lower (or
  `upper`) triangle, NaN elsewhere.
* **Boundaries**: `Landscape.boundaries` marks, for every class separately, the cells
  within `edge_depth` cells (city-block) of another class, NA or the map edge (unless
  `consider_boundary`): the complement of the core cells. `raster_boundaries` runs
  landscapemetrics' `rcpp_get_boundaries` rule on the values as given: a cell is a
  boundary when the cell *below* differs or the cell above, left or right is NA, so on a
  multi-class raster only one side of a change is marked, as in R; `patch_id` is refused
  with a warning when a value is 0.
* **check_landscape** reads the CRS like terra, which assumes lon/lat (`OGC:CRS84`) for a
  raster file without CRS whose extent fits longitudes and latitudes.
* **calculate_correlation** builds the tables like R's `xtabs` (values summed per id /
  class / layer, missing combinations 0, NA values NA, rows without id left out) and
  computes R's `cor()`: two-pass means and 80-bit cross products, Spearman on average
  ranks, Kendall from sign products; NA or constant metrics have NA correlations (left
  out) and a diagonal of 1.

These need the raster, so they are not available out of core (`block_rows`).

## References

- Hesselbarth, M. H. K., Sciaini, M., With, K. A., Wiegand, K., & Nowosad, J. (2019). landscapemetrics: an open-source R tool to calculate landscape metrics. *Ecography*, 42(10), 1648–1657. https://doi.org/10.1111/ecog.04617

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
