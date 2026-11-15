---
title: Statistics of maps
description: "Trends of every cell, Moran's I, LISA, Gi*; kriging, IDW; cell statistics, local correlation."
order: 270
---

Trends along a series of maps and the spatial autocorrelation of a map: the
Mann-Kendall test and Sen's slope of every cell (pymannkendall's
`original_test`, the same bits), the global and the local Moran's I and Getis
and Ord's Gi* over the rook or queen contiguity of the cells (PySAL's esda, to
rounding), with pseudo p values of random permutations; and the interpolation of
points onto a map as R's gstat: sample variograms and their fit, inverse
distance weighting and ordinary kriging; statistics of a stack at every cell
(with the layer of the peak) and the correlation of two maps in a moving
window. C++ with OpenMP; the same with any number of threads. The `tf.stats`
module.

<!-- no-run -->
```python
import terraflux as tf

ndvi = tf.load_raster("ndvi_2000_2024.tif")       # a band per year
t = tf.stats.trend(ndvi)
t.slope, t.p, t.trend                              # Sen's slope, the test's p, 1 up / -1 down / 0 none

tf.stats.moran(t.slope)                            # global Moran's I and its p values
tf.stats.lisa(t.slope).clusters                    # high-high, low-low... cells
tf.stats.getis_ord(tf.load_raster("density.tif")).spots   # hot and cold spots

pts = tf.load_vector("soil_samples.gpkg")          # points with a value
v = tf.stats.variogram(pts, "carbon", model="exponential")
k = tf.stats.kriging(pts, ndvi[0], v, "carbon")    # onto the cells of a map
k.prediction, k.variance
tf.stats.idw(pts, ndvi[0], "carbon", nmax=12)

tf.stats.cell_stats(ndvi, "argmax")                # the year of the peak
tf.stats.local_correlation(ndvi[0], rain, radius=5).slope
```

## Trends

::: terraflux.stats.trend

::: terraflux.stats.Trend

::: terraflux.stats.mann_kendall

## Statistics of a stack

::: terraflux.stats.cell_stats

## Local correlation

::: terraflux.stats.local_correlation

::: terraflux.stats.LocalCorrelation

## Spatial autocorrelation

::: terraflux.stats.moran

::: terraflux.stats.Moran

::: terraflux.stats.lisa

::: terraflux.stats.LISA

::: terraflux.stats.getis_ord

::: terraflux.stats.GetisOrd

## Interpolation

::: terraflux.stats.variogram

::: terraflux.stats.Variogram

::: terraflux.stats.VariogramModel

::: terraflux.stats.idw

::: terraflux.stats.kriging

::: terraflux.stats.Kriging
