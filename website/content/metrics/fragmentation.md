---
title: Fragmentation
description: "The moving-window measures of habitat fragmentation as GuidosToolbox computes them: foreground area density (FAD) and its classes, the fixed observation scale (FOS), contagion and clustering, spatial entropy, GraySpatCon's statistics of a percentage map and the landscape mosaic."
order: 100
---

::: metrics fragmentation

## The example maps {#example}

The values on the cards were computed on the three maps of the fragmentation tutorials: the forest of GuidosToolbox's
sample (1000 x 1000 cells of 25 m), GraySpatCon's sample of the percentage of tree cover of the conterminous United
States (1289 x 1990 cells of 2.43 km), and the CORINE land cover around Hannover grouped into agriculture, natural and
developed land for the landscape mosaic (the ranges of [`tf.reclassify`][terraflux.reclassify] are `low <= value < high`;
the water, 40 to 44, is in no group and so missing). A window of 27 cells of the forest map is 675 m wide:

```python
import terraflux as tf

# forest of GuidosToolbox's sample (1000 x 1000 cells of 25 m): 0 missing, 1 background, 2 forest
forest = tf.load_raster(tf.example_data("input"))
# % tree cover of the conterminous United States (GraySpatCon's sample): 0-100, 255 missing
cover = tf.load_raster(tf.example_data("gscinput"))
# CORINE around Hannover in the landscape mosaic's three groups: 1 agriculture, 2 natural, 3 developed, 0 missing
three = tf.reclassify(tf.load_raster(tf.example_data("clc")), [(1, 12, 3), (12, 23, 1), (23, 40, 2)], others=0)

fad = tf.fad(forest, window_size=27)
print(fad.stats.to_string(index=False))
fos = tf.fos(forest, window_size=27)
print(f"{fos.n_patches} patches, ECA {fos.eca:.0f} cells, coherence {fos.coherence:.2f}%")
```

```text
 scale  intact_%  interior_%  dominant_%  transitional_%  patchy_%  rare_%  average_fad
    27      7.35       32.72       32.18           13.99     12.21    1.55        73.54
2850 patches, ECA 221293 cells, coherence 51.64%
```

The forest is 42.9% of the map, in 2 850 patches. In windows of 675 m, nearly two thirds of it is dominant or interior,
and an average forest cell has 73.5% forest around it. The equivalent connected area is about half of the forest's
area: that half is the coherence.
