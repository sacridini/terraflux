---
title: Land change simulation
description: "Land use change: transitions, potentials, CLUE / Dinamica / PLUS allocation, validation."
order: 200
---

Land use and land cover change models: analysing the changes between maps,
estimating where change is likely, allocating future change and validating
the simulated maps: the `tf.simulate` module. It brings the tools of the
field together as interchangeable pieces of one model (CLUE-S, CLUMondo,
Dinamica EGO, PLUS, FUTURES), each reproducing its reference. The module is
being built phase by phase; this page grows with it.

<!-- no-run -->
```python
import terraflux as tf

lu = tf.load_raster(tf.simulate.example_data("pie_landuse"))   # 1985, 1991, 1999
ch = tf.simulate.changes(lu)                                   # transitions, areas, change counts
ia = ch.intensity(gain=2, loss=1)                              # intensity analysis
tf.simulate.difference_components(ch.matrix())                 # quantity, exchange, shift
mk = ch.markov(steps=14)                                       # transition rates per year

ef = tf.load_raster(tf.simulate.example_data("pie_drivers"))
woe = tf.simulate.weights_of_evidence(lu[0], lu[2], ef, {"dist_built_1985": 100})
p = woe.probability(lu[2], ef)                                 # Dinamica EGO's transition probabilities

forms = {1: ["elevation", "slope"], 2: ["elevation", "slope", "dist_built_1985"], 3: ["elevation", "slope"]}
model = tf.simulate.Model(                                     # CLUE-S, as lulcc
    potential=tf.simulate.LogisticRegression(forms).fit(lu[0], ef),
    demand=tf.simulate.AreaDemand.from_maps(lu, times=[0, 6, 14]),
    allocation=tf.simulate.CLUE(jitter=0.0002, scale=1e-6, max_diff=50, mean_diff=50),
    rules=tf.simulate.Rules(elasticity=[0.2, 0.2, 0.2]))
sim = model.run(lu[0], ef, seed=1)                             # 14 yearly maps
```

Land systems that supply goods and services (CLUMondo), given as overlapping
binary layers:

<!-- no-run -->
```python
S = tf.simulate
st = S.LandSystemStack({"forest": f, "extensive": pe, "intensive": pi, "soy": soy})
st.overlaps()                                                  # the combinations (mosaics) found
roles = S.Roles(natural=["forest"], families={"cattle": ["extensive", "intensive"], "crops": ["soy"]})
model = S.Model(
    potential=st.suitability({"forest": sf, "extensive": se, "intensive": si, "soy": ss}),
    demand=S.ServiceDemand(demand_df),                         # heads of cattle, tons of soy per year
    allocation=S.CLUMondo(st.supply({"extensive": {"cattle": 0.6}, "intensive": {"cattle": 2.5},
                                     "soy": {"soy": 3.4}})),
    rules=S.Rules(elasticity=0.5, roles=roles, conversion_order=["intensification", "expansion"],
                  caps=[S.Cap("expansion", cells=500)], restrictions=parks))
sim = model.run(st, seed=1)
sim.layers["forest"]; sim.shortfall; sim.trajectory()
```

Urban growth by patches (FUTURES):

<!-- no-run -->
```python
demand = S.DevelopmentDemand.from_population(observed_maps, counties, observed_pop, projected_pop,
                                             range(2020, 2051))
model = S.Model(
    potential=S.DevelopmentPotential(coefficients_df),         # intercept, devpressure, predictors per county
    demand=demand,
    allocation=S.PatchGrowing(patch_library, subregions=counties, neighborhood=10,
                              compactness_mean=0.4, compactness_range=0.05, incentive_power=2))
sim = model.run(developed_2020, {"slope": slope, "road_dist": road_dist}, seed=1)
sim.development                                                # step in which each cell was developed
```

## Change between maps

::: terraflux.simulate.changes

::: terraflux.simulate.Changes

::: terraflux.simulate.IntensityAnalysis

::: terraflux.simulate.crosstab

## Components of difference

::: terraflux.simulate.difference_components

::: terraflux.simulate.overall_difference

::: terraflux.simulate.difference_resolutions

::: terraflux.simulate.composite_matrix

::: terraflux.simulate.memberships

::: terraflux.simulate.mad

::: terraflux.simulate.sample_to_population

## Markov chains

::: terraflux.simulate.Markov

## Transition potential

::: terraflux.simulate.WeightsOfEvidence

::: terraflux.simulate.weights_of_evidence

::: terraflux.simulate.LogisticRegression

::: terraflux.simulate.Estimator

::: terraflux.simulate.MultiCriteria

::: terraflux.simulate.RasterPotential

## Allocation

::: terraflux.simulate.Model

::: terraflux.simulate.Simulation

::: terraflux.simulate.AreaDemand

::: terraflux.simulate.Rules

::: terraflux.simulate.Neighbourhood

::: terraflux.simulate.CLUE

::: terraflux.simulate.Ordered

## Dynamic variables and regions

::: terraflux.simulate.distance_to

::: terraflux.simulate.share_of

::: terraflux.simulate.gravity_of

::: terraflux.simulate.Regional

## Land systems (CLUMondo)

::: terraflux.simulate.CLUMondo

::: terraflux.simulate.ServiceDemand

::: terraflux.simulate.Supply

::: terraflux.simulate.NeighbourhoodEffect

::: terraflux.simulate.LandSystemStack

::: terraflux.simulate.Roles

::: terraflux.simulate.Cap

::: terraflux.simulate.Succession

## Expander and Patcher (Dinamica)

::: terraflux.simulate.ExpanderPatcher

## Stock taken by patches (Dinamica's continuous Patcher)

::: terraflux.simulate.ContinuousPatcher

::: terraflux.simulate.StockAllocation

::: terraflux.simulate.StockSimulation

## Roads built by the model (Dinamica's road constructor)

::: terraflux.simulate.RoadBuilder

::: terraflux.simulate.RoadDistance

::: terraflux.simulate.friction_cost

::: terraflux.simulate.destinations

::: terraflux.simulate.pathways

## Multi-type random patch seeds (PLUS)

::: terraflux.simulate.CARS

## MOLUSCE

::: terraflux.simulate.MolusceWoE

::: terraflux.simulate.MolusceCA

## Urban growth (FUTURES)

::: terraflux.simulate.PatchGrowing

::: terraflux.simulate.DevelopmentDemand

::: terraflux.simulate.DevelopmentPotential

::: terraflux.simulate.development_pressure

::: terraflux.simulate.Flooding

::: terraflux.simulate.patch_library

::: terraflux.simulate.calibrate_patches

## Validation

::: terraflux.simulate.three_map_comparison

::: terraflux.simulate.ThreeMapComparison

::: terraflux.simulate.compare_maps

::: terraflux.simulate.error_budget

::: terraflux.simulate.futures_validation

::: terraflux.simulate.futures_validation_grid

::: terraflux.simulate.fuzzy_similarity

::: terraflux.simulate.FuzzySimilarity

::: terraflux.simulate.differences

::: terraflux.simulate.similarity_of_differences

::: terraflux.simulate.validate

::: terraflux.simulate.Validation

## Calibration

::: terraflux.simulate.calibrate

::: terraflux.simulate.Calibration

## Presets and one-call projection

::: terraflux.simulate.run

::: terraflux.simulate.Projection

::: terraflux.simulate.agreement

::: terraflux.simulate.presets

## Running models with Terraflux

[Terraflux](https://github.com/sacridini/terraflux) runs land change models as graphs of operators, with the
same C++ kernels: `Model.save` writes a fitted model as its graph (with the data it runs on), `Model.load` reads
it back, and `terraflux` runs it with the `terraflux` command, giving the same maps as `Model.run` with the same
seed.

::: terraflux.simulate.terraflux

## Scenarios and replicates

::: terraflux.simulate.scenarios

::: terraflux.simulate.Ensemble

## Example data

::: terraflux.simulate.example_data
