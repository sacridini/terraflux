---
title: Conservation planning and habitat networks
description: "The measures that judge a conservation plan and its habitat: a prioritization's cost, boundary, representation, targets and the importance of each unit (prioritizr), Zonation's top fractions, comparisons and impacts, ecological intactness (plangea), and GuidosToolbox's habitat networks and zones of influence."
order: 140
---

::: metrics conservation-indices

## The examples {#example}

The values on the cards were computed on four small examples of the library, made in one script: the imaginary region
of Serra Verde (60 x 90 cells of 1 km, a cost and eight species) with a minimum-set problem, its optimal solution and two
Zonation rankings; a toy impact assessment of a road on two random features; a toy human footprint of 5 x 5 cells; and
GuidosToolbox's example habitat map with its MSPA:

```python
import numpy as np
import pandas as pd
import terraflux as tf

P = tf.prioritize
# Serra Verde (imaginary, 60 x 90 cells of 1 km): a cost and eight species; a minimum-set problem with 20 % targets
cost = tf.load_raster(P.example_data("cost"))
features = tf.load_raster(P.example_data("features"))
problem = P.problem(cost, features).add_min_set_objective().add_relative_targets(0.2).add_binary_decisions()
solution = problem.solve()
caz = P.zonation(features, rule="CAZ1")    # two Zonation rankings of the same species
abf = P.zonation(features, rule="ABF")

# a toy impact assessment: two random features on 30 x 40 cells, a road planned along row 15
species = np.random.default_rng(0).random((2, 30, 40))
road = np.zeros((30, 40), np.uint16)
road[15, :] = 1
scenario = P.ImpactScenario(road, [P.ImpactSource(group=1, source=1, footprint=100, buffers=((3, 50),))])

# a toy human footprint, 5 x 5 cells of 1 km: wild on the left, a town on the right
hfi = np.array([[0, 0, 0, 10, 40]] * 5, dtype=float)

# GuidosToolbox's example habitat map (1000 x 1000 cells) and its MSPA
habitat = tf.load_raster(tf.example_data("input"))
mspa = tf.mspa(habitat)

print(P.eval_n_summary(problem, solution).to_string(index=False))
print(P.eval_cost_summary(problem, solution).round(1).to_string(index=False))
print(caz, abf, sep="\n")
print(tf.gtn_components(mspa))
```

```text
summary     n
overall 534.0
summary    cost
overall 23593.9
ZonationResult(rule=CAZ1, features=8, cells=4825, iterations=21, ranking_error=0.007513%, auc=0.7898)
ZonationResult(rule=ABF 0.25, features=8, cells=4825, iterations=15, ranking_error=0.007525%, auc=0.7933)
<GTNComponentsResult: 418 components (135 networks, 283 isolated), ECA=182473.03 pixels, DOC=56.71%>
```

A minimum-set solution meets every target at the lowest cost, so most species end near their 20 % target and a few above
it (the harpy eagle at 30 %, the araucaria forest at 43 %), carried along by the cells the others need. The importance of
the units (Ferrier, rarity-weighted richness, replacement cost, rank) says which of the 534 cells the plan could least do
without. The importance maps have no value on the 575 cells of the ocean, which are not planning units. The toy examples
are there because no example data of the library has a planned land use or a human footprint; use your own the same way.
