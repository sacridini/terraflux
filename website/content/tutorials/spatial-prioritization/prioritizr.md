---
title: Optimal plans with prioritizr
short: 4. prioritizr
description: Conservation plans as integer programs, solved with a proof of their quality - objectives, targets and budgets, locked areas, the optimality gap and the HiGHS solver, proportion decisions and portfolios of near-optimal plans, with prioritize.optimize.
order: 40
level: intermediate
time: 60 min
template: Conservation planning (prioritizr)
ops: [prioritize.optimize, prioritize.evaluate, raster.read, view.plot]
series: Systematic conservation planning
part: 4
---

Marxan searched for good plans; this part finds the best one and proves it. You will write Serra Verde's planning
problem as an integer program with [[prioritize.optimize]], the model of the R package prioritizr, solve it with the
open-source solver HiGHS, and read what the solver tells you about the quality of its answer. Then you will change
the question: what to do with half the money, under six different meanings of "the most"; how the existing
protected areas and the cities change the plan; and how many other plans are almost as good. At the end you will
know how to choose an objective, what an optimality gap is, and what to report.

## The idea

### Planning problems are integer programs

In the 1990s, operations researchers pointed out that reserve selection is a classic problem of their field: choose
a set of sites, each in or out, that meets every target at the least cost (Underhill, 1994; Church, Stoms & Davis,
1996). Such problems are **integer linear programs** (ILP): an objective and constraints that are sums of decisions
times numbers, with decisions that must be whole (0 or 1). For years they were thought too slow for real planning,
and heuristics such as Marxan's annealing took over (Rodrigues & Gaston, 2002). Solvers then became thousands of
times faster. Beyer et al. (2016) showed that commercial and open-source solvers find **optimal** plans for
realistic problems in seconds to minutes, and Schuster et al. (2020) that they beat simulated annealing in both
cost and time. prioritizr, the R package of Hanson et al. (2025), made the approach the standard: a problem is built
from parts (an objective, targets, decisions, penalties, constraints) and handed to a solver.

!!! science "Key idea: the minimum set as an integer program"
    With \(x_i \in \{0, 1\}\) the decision for planning unit \(i\), \(c_i\) its cost, \(r_{ij}\) the amount of feature
    \(j\) in it and \(T_j\) the target of feature \(j\):

    \[ \min \sum_i c_i x_i \quad \text{subject to} \quad \sum_i r_{ij} x_i \geq T_j \;\; \text{for every feature } j \]

    One variable per planning unit, one row per target. Every other objective of prioritizr changes the sum that is
    minimised or maximised and adds a budget, \(\sum_i c_i x_i \leq B\).

[[prioritize.optimize]] builds exactly the model prioritizr 8 compiles, row by row, and solves it with **HiGHS**
(Huangfu & Hall, 2018), the open-source solver that prioritizr also supports, with landschaft's own engines in front
of it for the problems general solvers find slow.

### Bounds, gaps and proofs

Serra Verde has \(2^{4825}\) possible plans. A solver does not try them: it solves the problem with **fractions** of
units allowed (the *linear relaxation*, fast to solve exactly), whose cost is a **bound** no real plan can beat, then
**branches** on a fractional unit (in, or out) and discards every branch whose bound is already worse than the best
plan found. The relative difference between the best plan found and the best bound is the **optimality gap**: a gap of
1 % is a proof that no plan is more than 1 % cheaper.

![The bound, the optimum and a plan found with a loose gap.](/img/tutorials/spatial-prioritization/prioritizr/bound.png "The 30 % minimum set of Serra Verde. With fractions of cells allowed (decisions = proportion) the cheapest solution costs 37.115 million USD: no plan of whole cells can cost less. The proven optimum (gap 0) is 37.141. Asked for a gap of 10 %, the solver stops at a plan of 37.196, which it proves within 0.22 % of the bound.")

The gap you ask for is when the solver may **stop**; the gap it reports is how far its plan **is** at most from the
best. Marxan reports neither.

## When to use it

- **Firm targets, unknown cost**: the minimum set, "what does 30 % of every ecosystem cost?"
- **A fixed budget**: the budget objectives, "what is the best we can do with 20 million?"
- **Rules that must hold**: locked-in reserves, locked-out cities, a budget, a cap on farmland taken: constraints are
  kept exactly, not traded off.
- **A proof of quality** for a plan that will be defended in public or in court.
- **Alternatives**: portfolios of plans within a known distance of the optimum.

When targets are not yet agreed or the map is huge, a [Zonation ranking](../zonation/) is the better start. When the
plan must be compact or connected, add penalties and constraints ([part 5](../spatial-design/)). When actions other
than protection matter, use [multi-action planning](../multi-action-planning/).

## What you need

- **Features** as a stack and a **cost** map on the same grid (cells with no cost are not planning units), as in the
  previous parts: [serra-verde.zip](/data/tutorials/spatial-prioritization/systematic-conservation-planning/serra-verde.zip).
- Optionally maps of units to lock in (existing reserves) or out (cities), tables of targets, polygons as planning
  units ([part 5](../spatial-design/)).
- The template **Templates ▸ Conservation and restoration ▸ Conservation planning (prioritizr)**: the minimum set on
  four features drawn by chance.

## Build the model

### The minimum set, with what is protected and what cannot be

```tfgraph
#! caption: The cheapest plan that holds 30 % of every feature, the three protected areas locked in and the three cities locked out, solved within 0.1 % of the optimum.
#! numbered: true
#! highlight: plan
format = "terraflux/1"

[[nodes]]
id = "features"
op = "raster.read"
params = { path = "features.tif" }

[[nodes]]
id = "cost"
op = "raster.read"
params = { path = "cost.tif" }

[[nodes]]
id = "protected"
op = "raster.read"
params = { path = "protected.tif" }
note = "1 in the protected areas."

[[nodes]]
id = "urban"
op = "raster.read"
params = { path = "urban.tif" }
note = "1 in the cities."

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { features = "features.raster", cost = "cost.raster", locked_in = "protected.raster", locked_out = "urban.raster" }
params = { objective = "min_set", relative_targets = 0.3, gap = 0.001 }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "plan.solution" }

[outputs.summary]
from = "plan.summary"

[outputs.coverage]
from = "plan.coverage"
```

<div class="steps" markdown>

1. **Features and cost** (nodes 1 and 2) to the `features` and `cost` inputs, as in part 1.
2. **Locked areas** (nodes 3 and 4): any map whose cells are not 0. `locked_in` cells are in every plan (the
   protected areas, a reserve already bought); `locked_out` cells in none (the cities, land already committed).
3. **The problem** (node 5), in the Node pane:
    - `objective`: `min_set` here; the budget objectives below;
    - the **targets**: `relative_targets` (a share of each feature's total, one number or a list in the order of
      the layers), `absolute_targets` (amounts), `loglinear_targets` (a share that falls as a feature's total
      grows, as in the global gap analysis of Rodrigues et al., 2004), or a table on the `targets` input (feature, relative or absolute, sense,
      target);
    - `decisions`: `binary` (in or out, the default), `proportion` (any share of a unit) or `semicontinuous` (up to
      `upper_limit`);
    - `gap` (0.1 by default, as in prioritizr) and `time_limit` in seconds.
4. **Outputs**: `solution` (a map: 1 selected, 0 not), `selection` (a table per unit), `summary` (the objective,
   cost, units selected, the gap reached and whether the solve was optimal), and `coverage` (per target: the amount
   held, the target, the total and whether it is met).

</div>

!!! app "In the application"
    **Templates ▸ Conservation and restoration ▸ Conservation planning (prioritizr)** has the node ready; replace
    its random maps with [[raster.read]] nodes of yours. Connect the locked areas' `map` ports to `locked_in` and
    `locked_out`, select `plan` and press ++ctrl+enter++. Under the canvas, the `summary` table says how the solve
    ended: read it before the map.

From the command line, `terraflux run plan.toml --out output` writes `output/summary.csv` and
`output/coverage.csv`, the outputs listed in the model.

### Read the plan

![The plan with the protected areas locked in and the cities locked out.](/img/tutorials/spatial-prioritization/prioritizr/locked.png "The minimum set for 30 % of every feature with the three protected areas (purple) in the plan and the three cities (pink) out of it: 889 cells, of which 175 are the protected areas, for 44.0 million USD, within 0.003 % of the optimum.")

The `summary`: 889 cells, a cost of 44.0 million USD, a gap of 0.003 %, `optimal` 1. Without the locks the plan
costs 37.1 million (part 1). The protected areas cost 10.6 million and hold little of what the targets ask: 4 % of
the jaguar's habitat, 0.2 % of the tapir's, nothing of the tamarin or the mangrove. Locking them in raises the cost of
the plan by 6.9 million: they count towards the targets, but they are not where the targets are cheapest. This is the
price of the reserves the region already has, worth reporting beside the plan.

The `coverage` table: every target met, most exactly (30.0 %), with the harpy eagle (43 %) and the araucaria forest
(58 %) far above, "for free" in the western mountains the plan needs anyway.

## Spending a budget: six meanings of "the most"

With a fixed budget, the solver needs to know what "doing the most" means. Six objectives of
[[prioritize.optimize]] on the same budget, half of what the 30 % minimum set costs (18.6 million USD):

| Objective | What it maximises (or minimises) within the budget |
|---|---|
| `max_utility` | the total amount of all features held (with `feature_weights`, weighted) |
| `min_shortfall` | minus the sum of the shortfalls, each as a share of its target |
| `min_largest_shortfall` | minus the largest shortfall of any feature |
| `max_features` | the number of targets met in full |
| `min_extinction_risk` | minus the sum of the features' risk, \(1 - (\text{held}/\text{total})^z\) (landschaft's) |
| `max_cover` | the number of features held at all (any amount) |

`max_phylo_div` and `max_phylo_end` add a phylogeny (`tree`, in Newick) and value evolutionary history (Faith's
phylogenetic diversity) rather than species.

![The plans of six objectives with the same budget.](/img/tutorials/spatial-prioritization/prioritizr/objectives.png "Six objectives, one budget of 18.6 million USD. Max utility buys the cheap western mountains; weighted by the inverse of each feature's total, it buys the coast and the tamarin's fragment instead. The shortfall objectives, max features and min extinction risk spread the money between the mountains, the rivers, the coast and the fragments.")

```tfchart
type: bar
data: /data/tutorials/spatial-prioritization/prioritizr/objectives-by-feature.csv
x: feature
y: [Max utility, "Max utility, weighted", Min shortfall, Min largest shortfall, Max features, Min extinction risk]
colors: { "Max utility": "#e8901f", "Max utility, weighted": "#b4508a", "Min shortfall": "#2e7d4f", "Min largest shortfall": "#2f6f9f", "Max features": "#785ef0", "Min extinction risk": "#1c2420" }
ylabel: "% of the feature held"
height: 340
caption: "What each objective holds of every feature with the same budget (the targets of the shortfall objectives and of max features: 30 %). Toggle objectives in the legend."
```

| Objective | Targets of 30 % met | Mean held | Least held | Sum of risks (z = 0.25) |
|---|---|---|---|---|
| Max utility | 2 | 21.1 % | 0 % (tamarin, mangrove) | 4.02 |
| Max utility, weighted | 2 | 29.1 % | 2.1 % (tapir) | 2.64 |
| Min shortfall | 5 | 25.4 % | 5.9 % (tapir) | 2.44 |
| Min largest shortfall | 1 | 19.7 % | 16.1 % (jaguar, tapir) | 2.71 |
| Max features | 5 | 24.1 % | 1.2 % (tapir) | 2.64 |
| Min extinction risk | 4 | 26.5 % | 7.7 % (tapir) | 2.39 |

Lessons from the table:

- **Max utility favours the common and the cheap.** Without weights, a km² of jaguar habitat counts as much as a km²
  of mangrove, and the mountains are cheap: the tamarin and the mangrove get nothing. Weights of one over each
  feature's total (`feature_weights`) put every feature on the same scale and swing the plan to the rare ones.
- **Min shortfall** is a balanced default when there are targets: five met, the rest partly.
- **Min largest shortfall** lifts the worst-off feature (no feature below 16 %) at the price of meeting only one
  target: it suits plans where no feature may be left far behind.
- **Max features** is all or nothing: it secures five targets, then stops spending (17.1 of 18.6 million), since no
  sixth target is within reach; the tapir gets 1 %.
- **Min extinction risk** needs no targets: the species-area curve (Pereira & Daily, 2006) makes the first km² of a
  feature worth far more than the last, so the money spreads. It has the lowest sum of risks, as it should.

There is no best objective, only the one that matches the question. Say which you used and why.

### What each budget buys

The slider runs the minimum shortfall objective from 5 to 40 million USD; the chart follows every feature as a share
of its target:

```tfslider
label: Budget (million USD)
caption: "The minimum shortfall plan for every budget. The first millions go to the tamarin and the mangrove, whose targets are small and on cheap enough land; then the otter's rivers and the mountain features; the jaguar and the tapir, widespread and partly on dear land, come last. At 40 million every target is met (the minimum set costs 37.1)."
frames:
  - { value: 5, src: /img/tutorials/spatial-prioritization/prioritizr/budget_5.png, stats: { "Targets met": "0 of 8", "Mean, % of target": "32 %", "Worst off": "tapir, 2 %" } }
  - { value: 10, src: /img/tutorials/spatial-prioritization/prioritizr/budget_10.png, stats: { "Targets met": "1 of 8", "Mean, % of target": "55 %", "Worst off": "tapir, 6 %" } }
  - { value: 15, src: /img/tutorials/spatial-prioritization/prioritizr/budget_15.png, stats: { "Targets met": "4 of 8", "Mean, % of target": "73 %", "Worst off": "tapir, 12 %" } }
  - { value: 20, src: /img/tutorials/spatial-prioritization/prioritizr/budget_20.png, stats: { "Targets met": "5 of 8", "Mean, % of target": "83 %", "Worst off": "tapir, 30 %" } }
  - { value: 25, src: /img/tutorials/spatial-prioritization/prioritizr/budget_25.png, stats: { "Targets met": "5 of 8", "Mean, % of target": "89 %", "Worst off": "tapir, 61 %" } }
  - { value: 30, src: /img/tutorials/spatial-prioritization/prioritizr/budget_30.png, stats: { "Targets met": "6 of 8", "Mean, % of target": "95 %", "Worst off": "jaguar, 77 %" } }
  - { value: 35, src: /img/tutorials/spatial-prioritization/prioritizr/budget_35.png, stats: { "Targets met": "7 of 8", "Mean, % of target": "99 %", "Worst off": "jaguar, 92 %" } }
  - { value: 40, src: /img/tutorials/spatial-prioritization/prioritizr/budget_40.png, stats: { "Targets met": "8 of 8", "Mean, % of target": "100 %", "Worst off": "none" } }
start: 3
chart:
  type: line
  data: /data/tutorials/spatial-prioritization/prioritizr/budget.csv
  x: budget
  y: [Jaguar, Tapir, Harpy eagle, Maned wolf, Tamarin, Otter, Araucaria, Mangrove]
  colors: { Jaguar: "#0072b2", Tapir: "#e69f00", "Harpy eagle": "#009e73", "Maned wolf": "#cc79a7", Tamarin: "#56b4e9", Otter: "#d55e00", Araucaria: "#f0e442", Mangrove: "#785ef0" }
  xlabel: Budget (million USD)
  ylabel: "% of the target held"
```

("Targets met" counts those held to within 0.5 % of their target.) Such a curve, budget against what it buys, is often
the most useful figure for a decision maker; [part 6](../evaluating-plans/) draws it formally as a Pareto front with
[[prioritize.calibrate]].

## Read the results: the solver's report

Every solve ends with a `summary` row; read it first.

| Column | Meaning | What to look for |
|---|---|---|
| `objective` | the value of the objective (the cost for `min_set`) | |
| `cost` | the cost of the units selected | in the units of the cost map |
| `selected` | the units selected (with proportion decisions, their sum) | |
| `gap` | the gap the solver proved | at most the `gap` asked |
| `optimal` | 1 if the solver reached the gap asked | 0: it stopped at the time limit, or the problem is infeasible |

A problem with targets that cannot be met (more than exists outside the locked-out units) is **infeasible**: no plan
exists. Check the features' totals and the locked areas before raising targets.

### Reporting a plan

In the methods: the planning units, features and their data, the cost and its units, the targets (and why those),
the objective, the decisions, locked areas, the solver (HiGHS, through Terraflux's prioritizr model) and the gap
asked. In the results: the cost, the number and area of the units, the gap proved, the coverage of every target, and
the map. A sentence such as "the plan costs 44.0 million USD, within 0.003 % of the cheapest possible" is something
Marxan's results cannot say. [Part 6](../evaluating-plans/) adds the importance of each place and a checklist.

## Settings that change the answer

### The gap

| Gap asked | Cost (million USD) | Gap proved | Cells | Time |
|---|---|---|---|---|
| 0.1 (the default) | 37.196 | 0.22 % | 813 | 0.4 s |
| 0.01 | 37.196 | 0.22 % | 813 | 0.4 s |
| 0.001 | 37.143 | 0.01 % | 812 | 5 s |
| 0 | 37.141 | 0 | 812 | 98 s |
| proportion decisions | 37.115 | (a bound) | 809 whole + 5 fractions | 0.4 s |

(Times on a desktop computer; yours will differ.) The default gap of 10 % gives a plan within 0.22 % of the best, in
under a second: the solver stopped as soon as it could prove 10 % and its first good plan was much better than that.
The last fraction of a percent takes most of the time: proving the optimum took 98 s for 0.15 % of the cost. For
exploring, 1 % is plenty; for a final plan, ask for 0.1 % or less and report the gap proved.

### Decisions

`decisions = "proportion"` lets the solver take any share of a unit. On rasters, a share of a cell rarely makes sense
on the ground, but proportions are right when units are large and partly protectable (a share of a property under an
easement), and they solve fast: here the relaxation is the bound of the figure above, and only 5 cells come out
fractional. `semicontinuous` with `upper_limit` caps the share of every unit (no more than 50 % of any farm).

![The plan with proportion decisions.](/img/tutorials/spatial-prioritization/prioritizr/proportion.png "With proportion decisions, the 30 % minimum set takes 809 cells whole and five in part (the shares between 0 and 1), for 37.115 million USD.")

### Alternatives: portfolios

The optimum is rarely the only good plan, and stakeholders may reject a place for reasons no map holds. A
**portfolio** returns several plans:

| `portfolio` | What it returns |
|---|---|
| `gap` | up to `number_solutions` distinct plans within `pool_gap` of the optimum, from the solver's pool |
| `cuts` | the best plan, then the best that differs from it, and so on |
| `top` | as cuts, each solved to optimality |
| `shuffle` | the problem solved with its units in random orders (`seed`): plans of equal quality when there are ties |

Its `solutions` output is a stack of the plans, `frequency` the share of the plans selecting each unit, and the
`summary` has a row per plan.

![Twenty plans within 2 % of the optimum.](/img/tutorials/spatial-prioritization/prioritizr/portfolio.png "A gap portfolio of 20 plans within 2 % of the optimum (pool_gap = 0.02): 794 cells are in all of them; 20 cells (orange) swap in and out, two at a time, in the river valleys and on the edges of the mountain blocks. The plans cost from 37.196 to 37.203 million USD.")

Here the plans near the optimum are almost the same: each differs from the first by swapping two cells. That is
information too: the 30 % targets leave little freedom where they are cheapest, and the places that matter (the
tamarin's fragments, the mangrove coast, the rivers) are in every plan. A `cuts` portfolio of ten plans tells the same
story (804 cells in all). Wider choice comes from relaxing the question (other targets, costs or penalties), not from
the solver. [Part 6](../evaluating-plans/) measures each place's importance directly.

## Common pitfalls

- **Not reading the `summary`.** A plan with `optimal = 0` may be far from the best, or the problem infeasible.
- **The default gap in a final plan.** It is 10 %, as in prioritizr; the plan is usually much closer, but only a
  small gap proves it. Report the gap proved.
- **Max utility without weights,** which abandons the rare features. Weight them, or use targets.
- **Zero instead of no value in the cost.** A cell with cost 0 is a free planning unit, which every objective will
  take. Cells outside the problem must have no value.
- **Locked-in areas that are too dear**, or locked-out areas that hold a feature's only habitat: the cost jumps, or
  the problem becomes infeasible. Look at the coverage of the locked areas alone first ([[prioritize.evaluate]]).
- **Features with very different totals** in budget objectives: weight them, or use relative targets.

## Going further

- Part 5, [Compact and connected plans, and zones](../spatial-design/): boundary and connectivity penalties,
  neighbours and contiguity, H3 hexagons, management zones.
- Part 6, [Evaluating, comparing and reporting plans](../evaluating-plans/): importance of places, trade-off curves,
  Zonation against an optimized plan.
- Part 7, [Multi-action planning](../multi-action-planning/): actions against threats instead of protection.
- The operators: [[prioritize.optimize]], [[prioritize.evaluate]].

## References

- Beyer, H. L., Dujardin, Y., Watts, M. E., & Possingham, H. P. (2016). Solving conservation planning problems with integer linear programming. *Ecological Modelling*, 328, 14–22. https://doi.org/10.1016/j.ecolmodel.2016.02.005
- Church, R. L., Stoms, D. M., & Davis, F. W. (1996). Reserve selection as a maximal covering location problem. *Biological Conservation*, 76(2), 105–112. https://doi.org/10.1016/0006-3207(95)00102-6
- Hanson, J. O., Schuster, R., Strimas-Mackey, M., Morrell, N., Edwards, B. P. M., Arcese, P., Bennett, J. R., & Possingham, H. P. (2025). Systematic conservation prioritization with the prioritizr R package. *Conservation Biology*, 39(1), e14376. https://doi.org/10.1111/cobi.14376
- Huangfu, Q., & Hall, J. A. J. (2018). Parallelizing the dual revised simplex method. *Mathematical Programming Computation*, 10(1), 119–142. https://doi.org/10.1007/s12532-017-0130-5
- Pereira, H. M., & Daily, G. C. (2006). Modeling biodiversity dynamics in countryside landscapes. *Ecology*, 87(8), 1877–1885. https://doi.org/10.1890/0012-9658(2006)87[1877:MBDICL]2.0.CO;2
- Rodrigues, A. S. L., Andelman, S. J., Bakarr, M. I., Boitani, L., Brooks, T. M., Cowling, R. M., et al. (2004). Effectiveness of the global protected area network in representing species diversity. *Nature*, 428(6983), 640–643. https://doi.org/10.1038/nature02422
- Rodrigues, A. S. L., & Gaston, K. J. (2002). Optimisation in reserve selection procedures: why not? *Biological Conservation*, 107(1), 123–129. https://doi.org/10.1016/S0006-3207(02)00042-3
- Schuster, R., Hanson, J. O., Strimas-Mackey, M., & Bennett, J. R. (2020). Exact integer linear programming solvers outperform simulated annealing for solving conservation planning problems. *PeerJ*, 8, e9258. https://doi.org/10.7717/peerj.9258
- Underhill, L. G. (1994). Optimal and suboptimal reserve selection algorithms. *Biological Conservation*, 70(1), 85–87. https://doi.org/10.1016/0006-3207(94)90302-6
