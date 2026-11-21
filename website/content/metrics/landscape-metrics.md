---
title: Landscape metrics
description: The patch, class and landscape metrics of a categorical map, as landscapemetrics and FRAGSTATS compute them.
order: 10
---

::: metrics landscape-metrics

## The example map {#example}

Every value on the cards above was computed on one small map, made by a neutral landscape model: a Gaussian random
field cut into three classes, 30 x 30 cells of 100 m (one cell is one hectare). The same lines draw it again, and one
[`tf.Landscape`][terraflux.Landscape] answers the three levels, finding the patches once:

```python
import terraflux as tf

field = tf.nlm.gaussian_field(30, 30, resolution=100, autocorr_range=8, nug=0.0, seed=7)
landscape = tf.nlm.classify(field, 3)    # three classes; the cells are 100 m, so one cell is one hectare

lc = tf.Landscape(landscape)             # the patches are found once, for every metric
print(lc.landscape_metrics(["ta", "np", "shdi", "contag"]).round(3).to_string(index=False))
print(lc.class_metrics(["ca", "pland", "ed", "ai"]).round(2).to_string(index=False))
print(lc.patch_metrics(["area", "shape"]).head(4).round(2).to_string(index=False))
```

```text
   ta   np  shdi  contag
900.0 37.0 1.086  17.173
 class    ca  pland    ed    ai
     1 251.0  27.89 26.78 75.96
     2 366.0  40.67 57.00 67.10
     3 283.0  31.44 32.89 73.87
 class  id  area  shape
     1   1 142.0   2.69
     1   2   1.0   1.00
     1   3   1.0   1.00
     1   4   1.0   1.00
```

The map has 900 cells (`ta`, in hectares) in 37 patches. The tables are *wide*, a column per metric: `landscape_metrics`
gives one row, `class_metrics` a row per class and `patch_metrics` a row per patch. The call on a card,
[`tf.landscape_metrics`][terraflux.landscape_metrics], is the one-call way: a long table with the columns `level`,
`class`, `id`, `metric` and `value`. To compute the metrics of a real map, read it with `tf.load_raster` and give it to
either.
