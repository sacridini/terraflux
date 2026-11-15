---
title: Neutral landscape models
description: "Simulated landscapes as null models: random, gradients, clusters, fractals."
order: 150
---

Simulated landscapes that follow a known rule (complete randomness, a
gradient, clustering, fractal roughness) and nothing else: the null models of
landscape ecology, to test whether a pattern or a process differs from what
chance alone would give. They live in the `tf.nlm` module. Tutorial:
[neutral landscapes](/tutorials/neutral-landscapes/neutral-landscapes-in-python/).

With the same `seed` every model gives the same landscape as the R package
NLMR, value for value: the random number generators of R and of the C++
library NLMR is built with are reproduced, so studies done in R can be
repeated and extended here.

<!-- no-run -->
```python
import terraflux as tf

r = tf.nlm.random(200, 200, seed=42)             # a DataArray of values in 0-1
habitat = tf.nlm.classify(r, weighting=[0.7, 0.3])
```

## Random and percolation

::: terraflux.nlm.random

::: terraflux.nlm.percolation

## Gradients

::: terraflux.nlm.planar_gradient

::: terraflux.nlm.edge_gradient

::: terraflux.nlm.distance_gradient

## Fractal surfaces and random fields

::: terraflux.nlm.mpd

::: terraflux.nlm.fbm

::: terraflux.nlm.gaussian_field

## Mosaics

::: terraflux.nlm.mosaic_field

::: terraflux.nlm.mosaic_tess

::: terraflux.nlm.mosaic_gibbs

!!! note "Voronoi mosaics and shapely"
    NLMR gives the tile values in the order in which GEOS returns the Voronoi
    cells. Terraflux asks GEOS for that order through shapely (installed with
    it); without shapely the models warn and give each germ its own value. GEOS 3.11 or later is needed for NLMR's order (older versions
    return the cells in another order, with a warning).

## Clusters

::: terraflux.nlm.random_cluster

::: terraflux.nlm.neigh

::: terraflux.nlm.random_rectangular_cluster

::: terraflux.nlm.curds

## Classification and rescaling

::: terraflux.nlm.classify

::: terraflux.nlm.binarize

::: terraflux.nlm.rescale

::: terraflux.nlm.merge
