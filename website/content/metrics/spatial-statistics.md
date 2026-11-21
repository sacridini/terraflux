---
title: Spatial autocorrelation
description: "How much the neighbouring cells of a map are alike, as PySAL's esda computes it: the global Moran's I with its z scores and permutation p, the local Moran's I (LISA) with its quadrants and clusters, and the Getis-Ord Gi* hot and cold spots."
order: 130
---

::: metrics spatial-statistics

## The example map {#example}

The values on the cards were computed on the elevation of a part of south-western Oregon that ships with the library
(371 x 371 cells of about 30 m, no cell without a value). Each statistic is one call, and its result holds the map or the
numbers the cards show:

```python
import numpy as np
import pandas as pd
import terraflux as tf

# elevation of a part of south-western Oregon (371 x 371 cells), every cell with a value
elev = tf.load_raster(tf.example_data("orelevation"))

print(tf.stats.moran(elev, permutations=99))
print(tf.stats.lisa(elev, permutations=99))
print(tf.stats.getis_ord(elev))
noise = elev.copy(data=np.random.default_rng(1).random(elev.shape))   # random values on the same grid
print(tf.stats.moran(noise, permutations=99))
```

```text
Moran(I=0.9990, z_rand=739.127, p_rand=0, p_sim=0.01)
LISA(38800 high-high, 40866 low-low, 0 low-high, 0 high-low cells)
GetisOrd(7569 hot, 4934 cold cells)
Moran(I=0.0011, z_rand=0.804, p_rand=0.421, p_sim=0.23)
```

An elevation is about as autocorrelated as a map can be: Moran's I is almost 1, and every p is the smallest the test
allows. The last line is the same statistic on random values laid on the same grid, for comparison: I near its
expectation and no pattern. On real maps of a few hundred thousand cells, the z scores are always large; compare the size
of I between maps, at the same cell size and contiguity, rather than its significance.
