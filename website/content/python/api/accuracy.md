---
title: Map accuracy and area
description: "Stratified samples of a map, its accuracy and the area of its classes (Olofsson et al. 2014)."
order: 210
---

The accuracy of a classified map and the area of its classes from a
stratified random sample, by the good practices of Olofsson et al. (2014,
*Remote Sensing of Environment* 148, 42-57): the size of the sample and its
share of each stratum, the points drawn, and, once each point has its
reference class (from imagery or the field), the error matrix in shares of
the area, the user's, producer's and overall accuracies and each class's area
estimated from the sample instead of counted on the map, with standard errors
and confidence intervals. The estimates are those of R's mapaccuracy
(`olofsson`, `stehman2014`) to 1e-12, and the points those numpy's
`default_rng(seed).choice` draws. The `tf.accuracy` module.

<!-- no-run -->
```python
import terraflux as tf

lulc = tf.load_raster("map_2020.tif")
design = tf.accuracy.sample_size(lulc, user_accuracy={3: 0.7, 4: 0.6}, minimum=75)
points = tf.accuracy.sample(lulc, design, seed=1)
tf.write_vector(points, "to_label.gpkg")                   # fill the column "reference"
acc = tf.accuracy.assess(lulc, tf.load_vector("labelled.gpkg"))
acc.table[["class", "area_ha", "area_ha_ci", "user_accuracy", "producer_accuracy"]]
```

## Designing the sample

::: terraflux.accuracy.sample_size

::: terraflux.accuracy.SampleSize

::: terraflux.accuracy.allocate

::: terraflux.accuracy.sample

## Accuracy and area

::: terraflux.accuracy.assess

::: terraflux.accuracy.Accuracy

::: terraflux.accuracy.qnorm
