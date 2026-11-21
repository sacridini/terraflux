---
title: Surface metrics
description: The texture of a continuous raster (an NDVI, an elevation, a canopy height), as geodiv computes it from ISO 25178.
order: 20
---

::: metrics surface-metrics

## The example surface {#example}

The values on the cards were computed on the NDVI of a part of south-western Oregon that ships with the library
(371 x 371 cells of about 30 m, the mean NDVI of June to August 2000), with its best-fitting trend surface already
removed, which is the form the metrics expect. Each metric has a function of its own, and
[`tf.surface.metrics`][terraflux.surface.metrics] gives any set of them in one table:

```python
import terraflux as tf

# NDVI of a part of south-western Oregon (371 x 371 cells), the best-fitting trend already removed
z = tf.load_raster(tf.surface.example_data("normforest"))

print(round(tf.surface.sa(z), 4))
print(tf.surface.metrics(z, metrics=["sa", "sq", "ssk"]).round(4).to_string())
```

```text
0.0443
sa     0.0443
sq     0.0648
ssk   -1.9341
```

A negative skewness says that the values have a longer tail below the mean than above it: a few cells with a much lower
NDVI than the rest. The metrics are meant for a surface without a trend. The same NDVI before the trend was taken out is
`tf.surface.example_data("orforest")`, and [`tf.surface.remove_plane`][terraflux.surface.remove_plane] takes the
best-fitting trend out of any surface.
