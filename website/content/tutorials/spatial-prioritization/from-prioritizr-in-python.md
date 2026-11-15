---
title: "From prioritizr and prioriactions to Python"
short: "From prioritizr to Python"
description: "tf.prioritize builds the same optimization models as the R packages prioritizr (8.0.6) and prioriactions (0.5.0), row by row, and keeps their vocabulary, so most scripts translate line by line."
order: 918
level: intermediate
time: 20 min
---

`tf.prioritize` builds the same optimization models as the R packages
[prioritizr](https://prioritizr.net) (8.0.6) and
[prioriactions](https://prioriactions.github.io/prioriactions/) (0.5.0), row by row, and keeps their
vocabulary, so most scripts translate line by line. This page is the dictionary: what changes in the
syntax, the equivalent of every function, and what to expect when you compare the results. The ideas behind each function are in the
[spatial prioritization tutorials](/tutorials/spatial-prioritization/); the full reference is the
[Spatial prioritization](/python/api/prioritization/) API page.

## What changes, what stays

| | prioritizr / prioriactions (R) | `tf.prioritize` |
|---|---|---|
| Building a problem | pipes: `problem(...) %>% add_...()` | method chains: `P.problem(...).add_...()` |
| Order of the calls | targets are resolved when the problem is built | any order: everything is resolved by `compile()` and `solve()` |
| A call changes the problem? | returns a new problem | returns a new problem (a `Problem` is immutable) |
| Rasters, polygons, tables | `terra` `SpatRaster`, `sf`, `data.frame` | `xarray.DataArray` ([`tf.load_raster`](/python/api/data/)), `GeoDataFrame`, `pandas.DataFrame` |
| Features | layers or columns | bands of a stack, a list of rasters, or columns; always by name |
| Default solver | the best one installed (Gurobi, CPLEX, CBC, HiGHS...) | HiGHS, built into Terraflux (nothing to install) |
| Threads | `threads = 1` | `n_threads`: most of the cores by default |
| Result of `solve` | the input's class with a `solution_1` layer or column | a [`Solution`](/python/api/prioritization/#terraflux.prioritize.Solution): `selection` (raster or table), `values`, `coverage`, `cost`, `objective`, `gap`, `bound`, `status`, `seconds` |
| Evaluations | `tibble` | `pandas.DataFrame`, the same columns |
| Plots | `plot(s)` | `tf.plot(s)` |

## A script, side by side

=== "R (prioritizr)"

    ```r
    library(prioritizr)
    library(terra)

    cost <- rast("cost.tif")
    features <- rast("features.tif")
    protected <- rast("protected.tif")

    p <- problem(cost, features) %>%
      add_min_set_objective() %>%
      add_relative_targets(0.3) %>%
      add_boundary_penalties(penalty = 0.003, edge_factor = 0.5) %>%
      add_locked_in_constraints(protected) %>%
      add_binary_decisions() %>%
      add_highs_solver(gap = 0.01)

    s <- solve(p)
    attr(s, "status"); attr(s, "objective")
    eval_target_coverage_summary(p, s)
    eval_cost_summary(p, s)
    writeRaster(s, "solution.tif")
    ```

=== "Python (tf.prioritize)"

    ```python
    import terraflux as tf

    P = tf.prioritize
    cost = tf.load_raster(P.example_data("cost"))
    features = tf.load_raster(P.example_data("features"))
    protected = tf.load_raster(P.example_data("protected"))

    p = (P.problem(cost, features)
           .add_min_set_objective()
           .add_relative_targets(0.3)
           .add_boundary_penalties(penalty=0.003, edge_factor=0.5)
           .add_locked_in_constraints(protected)
           .add_binary_decisions()
           .add_highs_solver(gap=0.01))

    s = p.solve()
    s.status, s.objective
    P.eval_target_coverage_summary(p, s)
    P.eval_cost_summary(p, s)
    tf.write_raster(s.selection, "solution.tif")
    ```

The importance of the selected units works the same way:

```python
ranks, steps = P.eval_rank_importance(p, s, n=4)     # prioritizr: eval_rank_importance(p, s, n = 4)
tf.plot(ranks)
```

## prioritizr, function by function

### Problems and data

| prioritizr | `tf.prioritize` |
|---|---|
| `problem(cost_raster, features)` | `P.problem(cost_raster, features)` |
| `problem(sf, features, cost_column = "cost")` | `P.problem(gdf, features, cost_column="cost")` (features: rasters summed over the polygons as exactextractr does, or column names) |
| `problem(pu, features, rij = rij, cost_column = "cost")` | `P.problem(pu, features, rij=rij, cost_column="cost")` (Marxan-style tables: `pu` with `id`, `features` with `id`, `name`, `rij` with `pu`, `species`, `amount`) |
| `feature_names(p)`, `number_of_features(p)` | `p.units.feature_names`, `len(p.units.feature_names)` |
| `number_of_planning_units(p)` | `p.units.n` |
| `compile(p)` | `p.compile()`: `A`, `obj`, `rhs`, `sense`, `lb`, `ub`, `vtype`, `modelsense`, `row_ids`, `col_ids` (R's `o$A()`, `o$obj()`, ...) |
| `get_sim_pu_raster()`, `get_sim_features()` | `P.example_data("cost")`, `P.example_data("features")` (Serra Verde) |
| Planning units as hexagons | `P.h3_units(cost, resolution)` |

### Objectives

| prioritizr | `tf.prioritize` |
|---|---|
| `add_min_set_objective()` | `.add_min_set_objective()` |
| `add_min_shortfall_objective(budget)` | `.add_min_shortfall_objective(budget)` |
| `add_min_largest_shortfall_objective(budget)` | `.add_min_largest_shortfall_objective(budget)` |
| `add_max_utility_objective(budget)` (9: `add_max_wtd_sum_objective`) | `.add_max_utility_objective(budget)`, or `.add_max_wtd_sum_objective(budget)` |
| `add_max_features_objective(budget)` (9: `add_max_n_targets_met_objective`) | `.add_max_features_objective(budget)`, or `.add_max_n_targets_met_objective(budget)` |
| `add_max_cover_objective(budget)` | `.add_max_cover_objective(budget)` |
| `add_max_phylo_div_objective(budget, tree)` | `.add_max_phylo_div_objective(budget, tree)` (an `tf.bio` tree or a Newick string) |
| `add_max_phylo_end_objective(budget, tree)` | `.add_max_phylo_end_objective(budget, tree)` |
| `add_min_penalties_objective(budget)` | `.add_min_penalties_objective(budget)` |
| — | `.add_min_extinction_risk_objective(budget, z=0.25)` (species-area extinction risk) |

### Targets and weights

| prioritizr | `tf.prioritize` |
|---|---|
| `add_relative_targets(0.3)` | `.add_relative_targets(0.3)` (one value or one per feature) |
| `add_absolute_targets(x)` | `.add_absolute_targets(x)` |
| `add_loglinear_targets(...)` | `.add_loglinear_targets(...)` (the same arguments) |
| `add_manual_targets(df)` | `.add_manual_targets(df)` (`feature`, `type`, `sense`, `target`) |
| `add_auto_targets()`, `add_group_targets()`, `spec_*_targets()` (8.1) | compute the targets in Python and pass them to `.add_absolute_targets()` or `.add_relative_targets()` |
| — | `.add_subregional_targets(regions, targets)`: targets within regions |
| `add_feature_weights(w)` | `.add_feature_weights(w)` |

### Constraints

| prioritizr | `tf.prioritize` |
|---|---|
| `add_locked_in_constraints(x)` | `.add_locked_in_constraints(x)` (a raster with 1, a column name, booleans or indices) |
| `add_locked_out_constraints(x)` | `.add_locked_out_constraints(x)` |
| `add_manual_locked_constraints(data)` | `.add_manual_locked_constraints(data)` (the same table: `pu`, `status`), or `status=` as a number, raster, column or values |
| `add_manual_bounded_constraints(data)` | `.add_manual_bounded_constraints(data)` (`pu`, `lower`, `upper`), or `lower=` / `upper=` |
| `add_linear_constraints(threshold, sense, data)` | `.add_linear_constraints(threshold, sense, data)` |
| `add_cost_constraints()` (9) | `.add_linear_constraints(threshold, "<=", cost)` |
| `add_contiguity_constraints(data)` | `.add_contiguity_constraints(data)` (an exact model: see below) |
| `add_feature_contiguity_constraints(data)` | `.add_feature_contiguity_constraints(data)` (exact) |
| `add_neighbors_constraints(k, clamp)` (newer: `add_neighbor_constraints`) | `.add_neighbors_constraints(k, clamp)`, or `.add_neighbor_constraints(k, clamp)` |
| `add_corridor_constraints()` (removed from prioritizr) | `.add_corridor_constraints(terminals, resistance, penalty)`: a different, working formulation (flows between terminals) |

### Penalties

| prioritizr | `tf.prioritize` |
|---|---|
| `add_boundary_penalties(penalty, edge_factor, data)` | `.add_boundary_penalties(penalty, edge_factor, data=None)` |
| `add_connectivity_penalties(penalty, data = m)` | `.add_connectivity_penalties(penalty, m)` |
| `add_asym_connectivity_penalties(penalty, data = m)` | `.add_asym_connectivity_penalties(penalty, m)` |
| `add_neighbor_penalties(penalty, zones, data)` (9) | `.add_neighbor_penalties(penalty, data=None, zones=None)` |
| `add_linear_penalties(penalty, data)` | `.add_linear_penalties(penalty, data)` |
| `add_cost_penalties()` (9) | `.add_linear_penalties(penalty, cost)` |

### Decisions, solvers and portfolios

| prioritizr | `tf.prioritize` |
|---|---|
| `add_binary_decisions()` | `.add_binary_decisions()` |
| `add_proportion_decisions()` | `.add_proportion_decisions()` |
| `add_semicontinuous_decisions(upper_limit)` | `.add_semicontinuous_decisions(upper_limit)` |
| `add_highs_solver(gap, time_limit, threads, presolve, verbose)` | `.add_highs_solver(gap, time_limit, n_threads, presolve, verbose)` |
| `add_gurobi_solver(gap, time_limit, threads, verbose)` | `.add_gurobi_solver(gap, time_limit, n_threads, verbose)` (needs `gurobipy`) |
| `add_default_solver()`, `add_cbc_solver()`, `add_cplex_solver()`, `add_lpsymphony_solver()`, `add_rsymphony_solver()` | `.add_highs_solver()` |
| — | `.add_anneal_solver(...)`: Marxan's simulated annealing on the same model |
| `add_default_portfolio()`, `add_single_portfolio()` | the default |
| `add_cuts_portfolio(n)` | `.add_cuts_portfolio(n)` |
| `add_shuffle_portfolio(n, remove_duplicates)` | `.add_shuffle_portfolio(n, remove_duplicates)` |
| `add_top_portfolio(n)` | `.add_top_portfolio(n)` (also on HiGHS) |
| `add_gap_portfolio(n, pool_gap)`, `add_extra_portfolio()`, `add_pool_portfolio()` | `.add_gap_portfolio(n, pool_gap)` (also on HiGHS) |
| `solve(p)` | `p.solve()`; `p.solve(start=...)` from a solution or a Zonation ranking |
| `attr(s, "objective")`, `"status"`, `"gap"`, `"runtime"` | `s.objective`, `s.status`, `s.gap`, `s.seconds` |
| `eval_objective_summary(p, s)` (9) | `s.objective` |

A portfolio solves to a [`Portfolio`](/python/api/prioritization/#terraflux.prioritize.Portfolio): `selection` (a
band or a column per solution, named `solution_1`, `solution_2`... as in prioritizr), `summary` and
`frequency` (the share of the solutions that select every unit).

### Evaluation

Every `eval_*` function has the same name and arguments: `P.eval_n_summary(p, s)`,
`eval_cost_summary`, `eval_feature_representation_summary`, `eval_target_coverage_summary`,
`eval_boundary_summary`, `eval_connectivity_summary`, `eval_asym_connectivity_summary`,
`eval_ferrier_importance`, `eval_rank_importance`, `eval_rare_richness_importance` and
`eval_replacement_importance` (in parallel). The old names map to them: `replacement_cost` →
`eval_replacement_importance`, `rarity_weighted_richness` → `eval_rare_richness_importance`,
`ferrier_score` → `eval_ferrier_importance`, `feature_representation` →
`eval_feature_representation_summary`.

Beyond prioritizr: `P.eval_extinction_risk`, `P.eval_landscape_metrics` and `P.eval_mspa` (the
solution's pattern), `P.compare_zonation` (a Zonation ranking cut at the solution's cost) and
`P.report(p, s)`, everything in one object that `tf.plot` draws and `.save("report.html")` writes.

### Spatial data and calibration

| prioritizr | `tf.prioritize` |
|---|---|
| `boundary_matrix(x)` | `P.boundary_matrix(x)` |
| `adjacency_matrix(x, directions)` | `P.adjacency_matrix(x, directions)` |
| `connectivity_matrix(x, y)` | `P.connectivity_matrix(x, y)` |
| `proximity_matrix(x, distance)` | `P.proximity_matrix(x, distance)` |
| `rescale_matrix(m)` | `m / m.max()` |
| `calibrate_cohon_penalty(p)` | `P.cohon_penalty(p)` |
| the blended approach of the calibration vignette | `P.calibrate_penalty(p, "boundary", values=...)` |
| the hierarchical approach of the calibration vignette, `multi_problem()` with `add_hier_approach()` (9) | `P.calibrate_budget(p, "boundary", ...)` |
| a loop over budgets | `P.pareto_front(p, n=10)` |
| any other parameter | `P.calibrate(make, values, criterion)`, then `.best("knee")` or `.best("topsis")` |

### Management zones

| prioritizr | `tf.prioritize` |
|---|---|
| `problem(costs, zones(f_a, f_b, zone_names = c("a", "b")))` | `P.problem({"a": cost_a, "b": cost_b}, features={"a": f_a, "b": f_b})` |
| `zones(f, f * 0.5, ...)` (the same features scaled per zone) | `P.problem({"a": cost_a, "b": cost_b}, f, contribution={"b": 0.5})` (a number, one per feature, or a raster) |
| `problem(x, zones(c("f1", ...), c("g1", ...)), cost_column = c("ca", "cb"))` | `P.problem(x, features={"a": ["f1", ...], "b": ["g1", ...]}, cost_column={"a": "ca", "b": "cb"})` |
| `problem(x, spec, rij, cost_column, zones = z)` (`rij$zone`) | `P.problem(x, spec, rij=rij, zones=z, cost_column=[...])` |
| `add_relative_targets(matrix(...))` (features x zones) | the same matrix, or `add_relative_targets(0.3)` (summed over the zones) and `add_relative_targets(0.1, zone="a")` |
| `add_manual_targets()` with `zone = list(c("a", "b"))` | the same table (a zone or a list per row; no column: every zone) |
| `add_mandatory_allocation_constraints()` | `p.add_mandatory_allocation_constraints()` |
| `add_locked_in_constraints(matrix / stack)` | `p.add_locked_in_constraints({"a": raster_a, ...})` (or the stack, or `P.binary_stack(map)`) |
| budgets `c(b_a, b_b)` | `[b_a, b_b]` or `{"a": b_a, "b": b_b}` |
| `add_feature_weights(matrix(...))` | the same matrix (or one weight per feature, the same in every zone) |
| `add_boundary_penalties(p, edge_factor, zones = m)` | `p.add_boundary_penalties(p, edge_factor=[...], zones=m)` (`"same"` is the identity) |
| `add_connectivity_penalties(p, zones = m, data)`, `add_asym_connectivity_penalties` | the same with `zones=m` |
| `add_linear_penalties(c(...), data)` | `p.add_linear_penalties([...], {"a": data_a, ...})` |
| `add_neighbor_constraints(k = c(...), zones = m)`, `add_contiguity_constraints(zones = m)` | the same with `k=[...]` and `zones=m` |
| `category_layer(s)`, `binary_stack(x)` | `P.category_layer(s)`, `P.binary_stack(x)`; a solution's `zones` is already the map |
| `eval_*_summary(p, s)` with zones | the same functions: a row `overall` and one per zone |

Not with zones: arrays of connectivity or neighbours by pairs of zones (4-D), feature contiguity and the
corridors (an extension of Terraflux), Ferrier and rarity-weighted importance (as in prioritizr).

### Not available

The
weighted-sum and reference-point approaches of `multi_problem()` (9), `presolve_check()` and the target
methods of 8.1 (`spec_*_targets()`). Open an issue if you need one of them.

## What to expect when you compare the results

- **The models are prioritizr 8.0.6's.** `p.compile()` gives the same matrix, objective and bounds as
  R's `compile(p)`, row by row (the test suite checks every objective, target, constraint, penalty and
  decision against it), with one exception below.
- **Contiguity is exact here.** prioritizr's contiguity constraints (and feature contiguity) build a
  tree whose edges always run from a unit to one of a lower index. That misses every patch in which a
  unit has no selected neighbour of a higher index: three units in a line with the middle one first
  have no feasible patch at all, and on larger problems the "optimal" patch can cost more than the best
  one. Terraflux uses an exact model (a spanning arborescence with a flow), so its patches are as cheap
  or cheaper, and a problem prioritizr calls infeasible may have a solution here. The restricted model
  is still used, as a quick first patch.
- **Zones: targets summed over the zones are shares of the features' totals.** With the easy form
  (`contribution=`), `add_relative_targets(0.3)` asks for 30% of every feature's total, its amounts in
  every zone counted by the zone's contribution: prioritizr's manual absolute target with the list of
  every zone. A relative target of one zone is a share of the feature's total in that zone, as in
  prioritizr; with every zone's own features (as `zones()`), a target over several zones is a share of
  the sum of their totals, as in prioritizr.
- **Three prioritizr bugs with zones are fixed here.** Manual bounds or locks no longer open a zone where
  a unit has no cost (prioritizr gives it cost 0 there); the default adjacency (neighbour and contiguity
  constraints) takes the cells with a cost in any zone (prioritizr's takes those with a cost in every
  zone, so a unit missing from one zone has no neighbours); linear penalties keep the other zones' when
  one zone's is 0 (prioritizr drops them all).
- **prioritizr 9 changed three things.** It rescales the boundary data it computes so that their largest
  value is 1, so a penalty from a prioritizr 9 script corresponds here to that penalty divided by
  `P.boundary_matrix(cost).max()`. It dropped the small cost term that breaks ties in the maximum
  utility, maximum features and phylogenetic objectives. And it renamed two objectives (both names work
  here).
- **Optimal is not unique.** Two solvers, or two versions of one, can return different selections with
  the same objective. Compare the objective (and the gap and bound), not the cells.
- **The gap means the same.** `gap=0.1` stops within 10 % of the optimum, as in prioritizr; for a final
  plan use a small gap.
- **Faster, not different.** Terraflux's own engines (covering, cuts, closure) reach the solutions faster
  than the solver alone and prove the same bounds; the [benchmarks](/about/validation/) list the times.
- **Top fractions count cells by position.** Landscape identification and comparison follow Zonation,
  not a quantile of the surface (where ties can add units): the cells are ranked by position (ties: the
  earlier cell in row order ranks lower) and the top fraction is the cells ranked at or above
  1 − fraction, which on a Zonation ranking gives Zonation's own areas. The areas are 8-connected by
  default and keep Zonation's numbers.

To check a migrated analysis, export R's compiled model and compare:

<!-- no-run -->
```python
import numpy as np
import pandas as pd

m = p.compile()
r_obj = pd.read_csv("obj.csv")["x"].to_numpy()       # R: write.csv(data.frame(x = o$obj()), "obj.csv")
np.allclose(m.obj, r_obj), m.A.shape                  # R: dim(o$A())
```

## prioriactions

| prioriactions | `tf.prioritize` |
|---|---|
| `inputData(pu, features, dist_features, threats, dist_threats, sensitivity, boundary)` | `P.actions_data(pu, features, dist_features, threats, dist_threats, sensitivity, boundary)` (the same tables and columns) |
| — | `P.actions_raster_data(monitoring_cost, features, threats, action_cost, ...)`: the same from rasters |
| `getPotentialBenefit(d)` | `d.potential_benefit()` |
| `problem(d, model_type = "minimizeCosts", budget, blm, curve, segments)` | `d.problem(model="minimizeCosts", budget=..., blm=..., curve=..., segments=...)` |
| `getModelInfo(m)` | `m.compile()` |
| `solve(m, solver = "gurobi", gap_limit, time_limit, cores, verbose)` | `m.add_gurobi_solver(gap, time_limit, n_threads, verbose).solve()`, or `add_highs_solver` (the default) |
| `getActions(s)`, `getActions(s, format = "large")` | `s.action_summary()`, `s.action_summary(format="long")` |
| `getCost(s)` | `s.cost()` |
| `getSolutionBenefit(s)`, `getSolutionBenefit(s, type = "local")` | `s.benefit()`, `s.local_benefit` |
| `getConnectivityPenalty(s)` | `s.connectivity_penalty()` |
| `s$data$objval`, `s$data$gap`, `s$data$sol` | `s.objective`, `s.gap`, `s.x` |
| — | portfolios (`add_cuts_portfolio` and the others), `add_directional_dependencies(flow)`, `s.maps()` with raster inputs |

The example of prioriactions's documentation, in both:

=== "R (prioriactions)"

    ```r
    library(prioriactions)
    d <- inputData(pu = sim_pu_data, features = sim_features_data,
                   dist_features = sim_dist_features_data, threats = sim_threats_data,
                   dist_threats = sim_dist_threats_data, sensitivity = sim_sensitivity_data,
                   boundary = sim_boundary_data)
    m <- problem(d, model_type = "minimizeCosts")
    s <- solve(m, solver = "gurobi", gap_limit = 0.01, output_file = FALSE)
    getActions(s)
    ```

=== "Python (tf.prioritize)"

    ```python
    import pandas as pd

    pu = pd.DataFrame({"id": [1, 2, 3, 4], "monitoring_cost": [2.0, 1.0, 3.0, 1.0]})
    feats = pd.DataFrame({"id": [1, 2], "target_recovery": [1.0, 0.5]})
    dist_features = pd.DataFrame({"pu": [1, 2, 3, 3, 4], "feature": [1, 1, 1, 2, 2], "amount": 1.0})
    threats = pd.DataFrame({"id": [1]})
    dist_threats = pd.DataFrame({"pu": [1, 2, 3, 4], "threat": 1, "amount": 1.0,
                                 "action_cost": [1.0, 4.0, 2.0, 1.0]})

    d = P.actions_data(pu, feats, dist_features, threats, dist_threats)
    m = d.problem(model="minimizeCosts")
    sa = m.add_highs_solver(gap=0.01).solve()
    sa.action_summary()
    ```

## See also

- [Spatial prioritization](/python/api/prioritization/): the API reference.
- [Optimized conservation plans](/tutorials/spatial-prioritization/conservation-plans-in-python/): a complete plan, step by step.
- [Systematic conservation planning: the ideas](/tutorials/spatial-prioritization/systematic-conservation-planning/): where the software fits.
