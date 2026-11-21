---
title: Spatial prioritization
description: "Zonation's ranking of the cells for conservation: rules, weights, costs, curves."
order: 170
---

Two ways of setting conservation priorities, side by side: ranking every cell of a landscape
(below), and selecting the best set of planning units by optimization, with targets, budgets,
spatial penalties and constraints, portfolios, multi-action plans and the evaluation of the
solutions ([Optimization](#optimization) onwards). Both read the same rasters and share their
pre-processing, and each can start or check the other.

Ranking the cells of a landscape from the least to the most important for
conservation, given where a set of features (species, habitats, ecosystem
services, carbon...) occur: the core of systematic conservation planning. The
ranking balances all features at once, weighted, with costs and hierarchical
masks (for example, protected areas kept to the end), and its performance curves
tell how much of every feature the best 10 %, 20 %... of the landscape hold.
It lives in the `tf.prioritize` module.

The method is Zonation 5 (Moilanen et al. 2022): the cells are sorted by their
marginal loss, the loss to the features of removing a cell at the point where it
is removed, with the core-area (CAZ, driven by the features that lose the largest
fraction) or additive-benefit (ABF) rules, and sorted again until the order is
consistent. Terraflux gives the rank maps and curves of the z5 program (v2.4)
bit for bit, faster, with the curves of the ranking it returns (Zonation's own
report those of the order before its last sort). The
[tutorial](/tutorials/spatial-prioritization/zonation/#in-python) walks through a project, and the
[spatial prioritization tutorials](/tutorials/spatial-prioritization/) explain
the method.

<!-- no-run -->
```python
import terraflux as tf

res = tf.prioritize.zonation(["sp1.tif", "sp2.tif", "sp3.tif"], weights=[2, 1, 1],
                             cost="land_price.tif", hierarchic_mask="protected.tif")
res.rank                      # 0 = removed first ... ~1 = highest priority
res.feature_curves            # fraction of every feature kept, by fraction removed
res.write("zonation_output")  # rankmap.tif and Zonation's CSV files
```

## Ranking

::: terraflux.prioritize.zonation

::: terraflux.prioritize.ZonationResult

## Large maps

The features are read in blocks of rows (files, NumPy, xarray or dask arrays;
dask stacks are computed block by block, never whole) and quantized to one or two
bytes per value, kept as runs of NoData, zeros and values: a feature costs about
its occurrences, however many cells it is zero in, so thousands of species ranges
fit where Zonation stores every feature in every cell. During the ranking every
cell's occurrences are a compact row (a bitmap or layer gaps, whichever is
smaller) and the work arrays take about 130 bytes per cell with data.

For maps larger than the memory, `storage` puts the large arrays in memory-mapped
temporary files in a directory (on a fast disk), which the operating system pages
as needed; the reads and writes of the ranking are in sequence or nearly so. The
result is the same bit for bit; the files are deleted when no longer needed.

<!-- no-run -->
```python
import dask.array as da

stack = da.stack([da.from_zarr(f"ranges.zarr/{s}") for s in species])  # (features, y, x)
res = tf.prioritize.zonation(stack, storage="D:/scratch", n_threads=16)
res.write("global_ranking")
```

The ranking is limited to 2^32 - 1 cells with data and 65 535 features.

## Zonation projects

Existing Zonation 5 projects run as they are: the settings file, its feature
list and link files (weight groups, conditions, retention), with the switches of
the z5 command line.

::: terraflux.prioritize.run_project

::: terraflux.prioritize.read_project

## Connectivity

Dispersal kernels for the connectivity transforms of the features (the
``connectivity``, ``interaction`` and ``matrix_connectivity`` arguments of
`zonation`): a feature smoothed by the dispersal of its species, weighted by the
connectivity of a resource, or mixed with others by a matrix.

::: terraflux.prioritize.Kernel

::: terraflux.prioritize.Interaction

::: terraflux.prioritize.convolve

## Post-processing

Analyses of a ranking (methods of `ZonationResult`): statistics of areas
(external zones, the top fraction, separate high-priority areas), comparison with
another solution, histograms of the features' coverage and the flexibility map;
`ZonationResult.postprocess` runs Zonation's post-processing files.

::: terraflux.prioritize.AreaStatistics

## Impact assessment

Scenarios of land-use change and pressures, evaluated for the features and
applied to them before the ranking (the ``impact``, ``cumulative_impact``,
``impact_groups`` and ``impact_bins`` arguments of `zonation`): the loss around
impact sources, per footprint and buffer zone, and the direct habitat loss and
cumulative pressures of every impact group.

::: terraflux.prioritize.ImpactScenario

::: terraflux.prioritize.ImpactSource

::: terraflux.prioritize.CumulativeScenario

::: terraflux.prioritize.Pressure

## Optimization

A problem of planning units (raster cells, polygons, H3 cells or table rows), features, an
objective, targets, penalties and constraints, compiled to a mixed-integer linear model and solved
to a guaranteed gap. The grammar and the models are prioritizr's (Hanson et al.): `compile()` gives
prioritizr 8.0.6's model bit for bit and the solvers reach the same optima as prioritizr on Gurobi.
HiGHS is built in, with Terraflux's own engines in front of it for the problems it finds hard
(covering problems of millions of units, budgeted objectives, boundary penalties by minimum cuts,
multi-action plans); Gurobi is used when installed and asked for. Coming from R? The
[migration guide](/tutorials/spatial-prioritization/from-prioritizr/) lists the equivalent of every function.

```python
import terraflux as tf

P = tf.prioritize
cost = tf.load_raster(P.example_data("cost"))
features = tf.load_raster(P.example_data("features"))
protected = tf.load_raster(P.example_data("protected"))
p = (P.problem(cost, features)
       .add_min_set_objective()
       .add_relative_targets(0.2)
       .add_locked_in_constraints(protected)
       .add_boundary_penalties(0.0005)
       .add_binary_decisions()
       .add_highs_solver(gap=0.05))
sol = p.solve()
sol.status, round(sol.gap, 3)
```

::: terraflux.prioritize.problem

::: terraflux.prioritize.Problem

::: terraflux.prioritize.Solution

::: terraflux.prioritize.Portfolio

::: terraflux.prioritize.OptimizationProblem

::: terraflux.prioritize.h3_units

::: terraflux.prioritize.example_data

### Management zones

With zones (prioritizr's zones), every planning unit goes to at most one of several zones, each with
its own cost and its own contribution to the features: give `problem` the costs of the zones (a mapping
zone -> raster, or cost columns) and the contributions, or every zone's own features. Targets are summed
over the zones unless a `zone` is given. The solution's `zones` is the zone of every unit; these two
helpers convert between such maps and stacks of zone layers.

::: terraflux.prioritize.category_layer

::: terraflux.prioritize.binary_stack

## Spatial relations

The matrices that penalties and constraints read: the length of the boundary units share, which
units are adjacent, the connectivity between them from a raster or from distances.

::: terraflux.prioritize.boundary_matrix

::: terraflux.prioritize.adjacency_matrix

::: terraflux.prioritize.connectivity_matrix

::: terraflux.prioritize.proximity_matrix

::: terraflux.prioritize.decay_connectivity

::: terraflux.prioritize.unit_points

## Evaluating solutions

The figures of a solution (prioritizr's `eval_*`, equal bit for bit) and the importance of every
selected unit; the landscape the solution makes (patches, MSPA) and its species-area extinction risk.
They take a `Solution`, its selection or plain values per planning unit.

::: terraflux.prioritize.eval_n_summary

::: terraflux.prioritize.eval_cost_summary

::: terraflux.prioritize.eval_boundary_summary

::: terraflux.prioritize.eval_connectivity_summary

::: terraflux.prioritize.eval_asym_connectivity_summary

::: terraflux.prioritize.eval_feature_representation_summary

::: terraflux.prioritize.eval_target_coverage_summary

::: terraflux.prioritize.eval_replacement_importance

::: terraflux.prioritize.eval_ferrier_importance

::: terraflux.prioritize.eval_rare_richness_importance

::: terraflux.prioritize.eval_rank_importance

::: terraflux.prioritize.eval_extinction_risk

::: terraflux.prioritize.eval_landscape_metrics

::: terraflux.prioritize.eval_mspa

### Areas of a priority surface

The separate areas of the top fraction of any priority surface (a Zonation ranking, an importance, a
selection frequency) and what each holds, and the overlap of the top fractions of two surfaces
(Zonation's landscape identification and comparison; on a Zonation ranking, the same areas as
`ZonationResult.area_statistics_separate_areas`).

::: terraflux.prioritize.eval_landscape_identification

::: terraflux.prioritize.eval_landscape_comparison

::: terraflux.prioritize.position_rank

::: terraflux.prioritize.LandscapeAreas

::: terraflux.prioritize.LandscapeComparison

### Irreplaceability (ProxIRR)

ProxIRR (Baisero et al. 2022): for every target, the share of the feature's spare amount a unit
holds (alpha), and per unit the chance it is needed for at least one feature (beta) and the number of
features it is essential for plus beta (gamma). The bits are those of the CRAN package proxirr.

::: terraflux.prioritize.eval_proxirr

::: terraflux.prioritize.eval_proxirr_marxan

::: terraflux.prioritize.ProxIRR

## Calibration and trade-offs

Candidate solutions over a trade-off (a penalty's weight, cost thresholds, budgets, any parameter),
the choice of one by TOPSIS or the knee of the curve, and solving again as a problem changes.

::: terraflux.prioritize.calibrate_penalty

::: terraflux.prioritize.calibrate_budget

::: terraflux.prioritize.cohon_penalty

::: terraflux.prioritize.pareto_front

::: terraflux.prioritize.calibrate

::: terraflux.prioritize.TradeOffs

::: terraflux.prioritize.topsis

::: terraflux.prioritize.knee_point

::: terraflux.prioritize.mean_sd

::: terraflux.prioritize.iterative_solve

::: terraflux.prioritize.protected_cells

## Ranking and optimization together

Zonation's feature transforms as the features of a problem, the two paradigms compared at the same
cost, features from species distribution models, a report of a solution, and problems solved on
coarser blocks.

::: terraflux.prioritize.zonation_features

::: terraflux.prioritize.compare_zonation

::: terraflux.prioritize.ZonationComparison

::: terraflux.prioritize.sdm_features

::: terraflux.prioritize.report

::: terraflux.prioritize.PrioritizationReport

::: terraflux.prioritize.coarsen

::: terraflux.prioritize.refine

## Multi-action planning

Which units to monitor and which actions to take against which threats in them, so that the
features recover (or are kept) to their targets at the least cost, or recover the most within a
budget: prioriactions's models (Salgado-Rojas et al. 2020), bit for bit, from tables or rasters.

::: terraflux.prioritize.actions_data

::: terraflux.prioritize.actions_raster_data

::: terraflux.prioritize.ActionsData

::: terraflux.prioritize.ActionsProblem

::: terraflux.prioritize.ActionsSolution

::: terraflux.prioritize.ActionsPortfolio

## Restoration planning

How much of every converted cell to restore, for the most reduction of the species' extinction risk
(and of the ecosystems' risk of collapse), carbon and ecological intactness within area targets and a
budget, or the cheapest plan for a benefit: the restoration problem of plangea (Strassburg et al. 2020),
with its conservation (`conservable=`) and conversion (`convertible=`) forms. The species-area curves are
concave, so the optimum is found and proven, by simplicial decomposition (linear programs over the cells,
mixed by a small concave problem), from thousands of cells to millions; plangea's own procedure (benefit
per cost per cell, the target in steps) is `method="steps"`. Targets per region come from projections of
land demand (`add_projected_change`), shares of the land (`relative=True`), other values
(`add_linear_target`) or the ecosystems' risk of collapse (`ecosystem_targets`); `tradeoff` gives the plans
of a growing target and the gradient of priorities, `compare_weights` ranks sets of weights. The inputs
come from `tf.original_vegetation`, `tf.sdm.area_of_habitat` and `tf.intactness`. Tutorial:
[restoration priorities](/tutorials/distance-restoration/restoration-plan/#in-python).

::: terraflux.prioritize.restoration_problem

::: terraflux.prioritize.RestorationProblem

::: terraflux.prioritize.RestorationSolution

::: terraflux.prioritize.RestorationTradeOff

::: terraflux.prioritize.ecosystem_areas

::: terraflux.prioritize.ecosystem_targets

::: terraflux.prioritize.eval_risk_reduction

::: terraflux.prioritize.RiskReduction

## Simulated annealing

Marxan 4's runs, unit by unit as the program, and its annealing as a solver of any problem
(`Problem.add_anneal_solver`) for scales where a gap matters less than an answer.

::: terraflux.prioritize.marxan

::: terraflux.prioritize.MarxanResult
