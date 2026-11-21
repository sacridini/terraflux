---
title: Grid distances
description: "Transitions, cost surfaces, least-cost and random-walk distances and paths."
order: 110
---

Distances and routes over a raster seen as a graph of neighbouring cells: each
connection between two cells carries a conductance (how easily something
moves from one to the other), from which come cost surfaces, least-cost
distances and paths, and the distances and flows of random walks. They live in
the `tf.cost` module.

The results are those of the R package gdistance, value for value: the
connections are built with R's arithmetic, and on longitude/latitude grids the
distances between cells are the ellipsoidal ones of the R package raster, to
the last bit. Tutorial: [routes on grids](/tutorials/grid-distances/).

<!-- no-run -->
```python
import numpy as np
import terraflux as tf

dem = tf.load_raster(tf.cost.example_data("maungawhau"))
slope = tf.cost.geo_correction(tf.cost.transition(dem, "diff", 8, symmetric=False))
speed = slope.map(lambda s: 6 * np.exp(-3.5 * np.abs(s + 0.05)), pairs="all")  # Tobler, m/s
conductance = tf.cost.geo_correction(speed)                                    # 1 / seconds per step
```

## Transitions

::: terraflux.cost.transition

::: terraflux.cost.Transition

::: terraflux.cost.geo_correction

::: terraflux.cost.normalize

::: terraflux.cost.sum_reciprocal

## Least-cost surfaces, distances and paths

The cost of a step between neighbouring cells is the reciprocal of the
transition's value; among equally cheap paths, `shortest_path` returns the one
gdistance returns.

::: terraflux.cost.acc_cost

::: terraflux.cost.cost_distance

::: terraflux.cost.shortest_path

## Random walks

A random walk moves from a cell to a neighbour with a probability proportional
to the conductance of the connection. Its commute distances and passages are
those of an electrical circuit (the transition's conductances as resistors),
computed with the same sparse Cholesky factorisation as the
[circuit tools](/python/api/circuits/).

::: terraflux.cost.commute_distance

::: terraflux.cost.passage

## Randomised shortest paths

Between the random walk and the least-cost path: walks that prefer cheap
routes more and more as `theta` grows (Saerens et al. 2009). `passage` with a
`theta` gives their passages; `rsp_distance` their expected costs.

::: terraflux.cost.rsp_distance

## Route overlap, barriers and areas

How much the routes from a common origin to two locations share (van Etten
and Hijmans 2010), optionally weighted by barriers or areas.

::: terraflux.cost.path_inc

::: terraflux.cost.overlap

::: terraflux.cost.nonoverlap

::: terraflux.cost.barriers

::: terraflux.cost.areas

## Example data

::: terraflux.cost.example_data
