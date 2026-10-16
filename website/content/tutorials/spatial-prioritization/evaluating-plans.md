---
title: Evaluating, comparing and reporting plans
short: 6. Evaluating plans
description: Check any plan with prioritize.evaluate - targets met, the importance of every place by replacement cost, irreplaceability, rarity and rank, a gap analysis of existing reserves - calibrate trade-offs with prioritize.calibrate, compare a Zonation ranking with an optimized plan, and report a prioritization in a paper.
order: 60
level: intermediate
time: 50 min
template: Irreplaceability of a plan
ops: [prioritize.evaluate, prioritize.calibrate, prioritize.optimize, prioritize.zonation, raster.read, map.calc]
series: Systematic conservation planning
part: 6
---

A plan is a proposal to check, not an answer to accept. Does it meet its targets, and by how much? Which of its places
are essential, and which could be swapped? How much would a more compact plan cost? Would a ranking have told the same
story? In this part you will evaluate Serra Verde's 30 % plan with [[prioritize.evaluate]], measure the importance of
each of its cells four ways, run a gap analysis of the existing protected areas, calibrate the boundary penalty and
draw what each budget buys with [[prioritize.calibrate]], and put Zonation's best cells beside the optimized plan. The
part ends with a checklist for reporting a prioritization in a paper.

## The idea

### Evaluation works on any plan

Every measure of this part takes a problem (features, costs, targets) and a **solution**: a map of 0 and 1 (or shares).
The solution can come from [[prioritize.optimize]], from Marxan, from a threshold of a Zonation ranking, from the
existing reserves, or from a map drawn by hand in a workshop. The same yardstick for all is what makes plans
comparable. [[prioritize.evaluate]] reproduces prioritizr's `eval_*` functions, number for number (Hanson et al.,
2025).

### Which places matter most

A plan of 800 cells is a long list. Stakeholders ask which cells are essential and which could be swapped. Four
**importance** scores rank the selected cells; they answer slightly different questions:

| Measure | The question | How |
|---|---|---|
| **Replacement cost** (Cabeza & Moilanen, 2006) | how much dearer does the plan get if this cell is lost and the rest re-optimised? | one solve per selected cell |
| **Ferrier's irreplaceability** (Ferrier, Pressey & Barrett, 2000) | how likely is this cell to be needed to meet the targets? | a formula over the features |
| **Rarity-weighted richness** (Albuquerque & Beier, 2015) | how much of the rare features does this cell hold? | the sum of each feature's share in the cell |
| **Rank importance** (Jung et al., 2021) | how early does this cell enter the plan as the budget grows? | a few solves at growing budgets |

!!! science "Key idea: replacement cost"
    With \(C^*\) the cost of the optimal plan and \(C^*_{-i}\) the cost of the optimal plan when cell \(i\) is locked
    out, the replacement cost of \(i\) is

    \[ R_i = C^*_{-i} - C^* \]

    Zero means an equally good plan exists without the cell: it is interchangeable. A large value means the targets
    depend on it, and losing it (a landowner who refuses, a road) would cost that much more. [[prioritize.evaluate]]
    rescales the scores from 0.01 to 1 (`rescale`, true by default).

### Trade-offs

Every plan is a compromise: cost against compactness, money against representation, one stakeholder against another.
A single map hides the compromises; a **trade-off curve** shows them. [[prioritize.calibrate]] builds the curves of
prioritizr's calibration guide (Hanson et al., 2025):

- **blended**: the problem with a range of penalty weights, each giving a plan with a cost and a boundary (or a
  connectivity), as in [part 5](../spatial-design/);
- **hierarchical**: the least boundary within cost caps above the least cost (1 %, 2 %, 5 % more...): a clearer
  question ("how compact for 2 % more?") but harder problems;
- **pareto**: the least total shortfall of the targets within growing budgets: what each budget buys;
- **cohon**: a starting penalty from the two extreme plans (Cohon, Church & Sheer, 1979).

Two reproducible rules choose a candidate: the **knee** of the curve, the point farthest from the line between its two
ends (Satopää et al., 2011), where compactness stops being cheap; and **TOPSIS** (Hwang & Yoon, 1981), the candidate
closest to the ideal (least cost and least boundary) and farthest from the anti-ideal, both scaled.

## When to use it

- **After every solve**: the coverage of the targets, the cost, the shape.
- **Before a workshop**: the importance of places, to know where there is room to negotiate.
- **Gap analysis**: what the existing reserves already hold (a solution of the protected areas).
- **Choosing a penalty or a budget**: a calibration curve instead of a guess.
- **Comparing methods**: Zonation, Marxan and prioritizr on the same problem.

## What you need

The features and costs of the series
([serra-verde.zip](/data/tutorials/spatial-prioritization/systematic-conservation-planning/serra-verde.zip)), a plan
or any other solution map, and the targets. The template **Templates ▸ Conservation and restoration ▸ Irreplaceability
of a plan** evaluates a plan for four random features and draws a portfolio's selection frequency.

## Build the model

### Evaluate a plan

```tfgraph
#! caption: The 30 % minimum set (node 3) and its evaluation (node 4): coverage, cost and boundary, the importance of every selected cell (replacement cost, Ferrier's irreplaceability, rarity, rank importance) and the extinction risk of the features.
#! numbered: true
#! highlight: evaluation, ferrier_total
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
id = "plan"
op = "prioritize.optimize"
inputs = { features = "features.raster", cost = "cost.raster" }
params = { objective = "min_set", relative_targets = 0.3, gap = 0.001 }

[[nodes]]
id = "evaluation"
op = "prioritize.evaluate"
inputs = { features = "features.raster", cost = "cost.raster", solution = "plan.solution" }
params = { relative_targets = 0.3, replacement = true, rank = 10, extinction_z = 0.25, gap = 0.001 }

[[nodes]]
id = "ferrier_total"
op = "map.calc"
inputs = { f = "evaluation.ferrier" }
params = { expression = "f.total" }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "evaluation.replacement" }
params = { title = "Replacement cost" }

[outputs.coverage]
from = "evaluation.coverage"
```

<div class="steps" markdown>

1. **The problem** (nodes 1 and 2) and **a plan** (node 3). Any solution map can take node 3's place on the `solution`
   input: a [[raster.read]] of the protected areas, of Marxan's best run, of a hand-drawn plan.
2. **The evaluation** (node 4), with the same targets as the plan. Its settings: `relative_targets` or
   `absolute_targets` (what coverage is measured against); `replacement = true` (one solve per selected cell, in
   parallel; ask for a small `gap`, or the solver's tolerance swamps the small replacement costs); `rank`, the number
   of budgets of the rank importance; `extinction_z`, the exponent of the species-area risk.
3. **Its outputs**: `summary` (cells, cost, boundary), `representation` (per feature: its total, the amount held and
   the share), `coverage` (per target: met, the shortfall), `units` (per selected cell: every score), and the maps
   `replacement`, `rank`, `rarity` and `ferrier` (a stack: a layer per feature and `total`; node 5 takes the total
   with [[map.calc]]'s `f.total`), and `extinction_risk`.

</div>

### Calibrate

```tfgraph
#! caption: Calibration of the boundary penalty of the 30 % plan: seven penalties solved in parallel (node 3), the knee and TOPSIS marked in its table, and the plan TOPSIS chooses on its best output.
#! numbered: true
#! highlight: calibration
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
id = "calibration"
op = "prioritize.calibrate"
inputs = { features = "features.raster", cost = "cost.raster" }
params = { approach = "blended", penalty = "boundary", objective = "min_set", relative_targets = 0.3, values = [0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1], gap = 0.01 }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "calibration.best" }

[outputs.table]
from = "calibration.table"
```

[[prioritize.calibrate]] takes the problem as [[prioritize.optimize]] does, **without** the penalty it trades off, and
its own settings: `approach`, `penalty` (`boundary`, `connectivity`, `asym_connectivity` or `linear`), `values` (the
penalties, cost caps or budgets; without them it chooses `n` values around Cohon's penalty), `inflation` (for the
hierarchical approach: the cost caps as shares above the least cost), `gap` and `time_limit` per solve (no limit by
default, as in prioritizr: with a strong penalty one solve can take hours, so set one on a large problem; a candidate
stopped by it is marked not optimal, with its gap). Its `table` has
a row per candidate (its value, cost, criterion, gap, TOPSIS score, and `knee` and `topsis` set to 1 for the chosen),
`solutions` a stack of the candidate plans, and `best` the plan TOPSIS chooses.

!!! app "In the application"
    `prioritize.evaluate` and `prioritize.calibrate` are in the library's *Spatial prioritization* subject. Connect a
    plan's `solution` to the evaluation's `solution` input, run it, and click its output ports under the canvas:
    `coverage` and `units` are tables you can sort, `replacement` and `ferrier` maps.

## Read the results

### Coverage

The `coverage` of the plan: all eight targets met, six of them within 0.2 % of 30 %, the harpy eagle at 43.7 % and the
araucaria forest at 60.2 %, "for free" in the mountains the plan needs anyway. Its `summary`: 812 cells, 37.1 million
USD, a boundary of 480.5 km. With `extinction_z = 0.25`, the `extinction_risk` table sums the features' risks
\(1 - \text{held}^{z}\): 1.86 (of 8).

### The importance of each place

![Four measures of importance.](/img/tutorials/spatial-prioritization/evaluating-plans/importance.png "Four measures of the importance of the plan's 812 cells, each rescaled from 0 to 1 among them. Replacement cost is highest on the mangrove coast and in the tamarin's fragments, which have no substitutes. Ferrier's irreplaceability and rarity-weighted richness also pick out the coast and the fragments, and the rivers. Rank importance adds the core of the mountain blocks, which the plan buys first at every budget.")

```tftable
data: /data/tutorials/spatial-prioritization/evaluating-plans/importance-correlations.csv
labels: { measure: "Spearman correlation" }
digits: 2
heat: true
caption: "How the four measures rank the 812 selected cells: their Spearman correlations. Ferrier's irreplaceability, rarity and rank importance agree closely (0.73 to 0.86); replacement cost agrees with them only moderately (0.44 to 0.55)."
```

The measures agree on the essential places and disagree on the ordinary ones, which is as it should be: ordinary cells
are, by definition, interchangeable. Of the fifteen cells of highest replacement cost, fourteen are on the mangrove coast and in
the tamarin's fragments (the fifteenth is a river cell of the otter); the dearest to lose, 1.0 on the rescaled scale,
is a mangrove cell. Replacement cost is the most direct (it is money) and the slowest; Ferrier's and rarity are instant and work on
any plan; rank importance says where to start when money arrives in instalments.

A **portfolio's selection frequency** ([part 4](../prioritizr/#alternatives-portfolios)) is a fifth view; on Serra
Verde it separates only a few interchangeable cells, since the plans near the optimum differ by a pair of cells.

### A gap analysis

```tfchart
type: bar
data: /data/tutorials/spatial-prioritization/evaluating-plans/gap-analysis.csv
x: feature
y: [Held by the protected areas, Target]
colors: { "Held by the protected areas": "#785ef0", Target: "#d9e0d9" }
ylabel: "% of the feature"
height: 280
caption: "A gap analysis: the coverage of the three existing protected areas (prioritize.evaluate with the protected areas as the solution) against targets of 30 %. They cover 175 cells and 10.6 million USD of land, and hold under 10 % of every feature: nothing of the tamarin and the mangrove, 0.2 % of the tapir."
```

Evaluating the existing reserves as a solution answers the first question of systematic conservation planning
(Margules & Pressey, 2000): what do we have? Serra Verde's protected areas are in the mountains and on an island of
forest; the species of the lowlands and the coast are not in them. [Part 4](../prioritizr/) locked them into a plan and
measured what completing the network costs.

### Calibrating the boundary penalty

```tfchart
type: scatter
data: /data/tutorials/spatial-prioritization/evaluating-plans/blended.csv
x: Boundary (km)
y: [Cost (million USD)]
colors: { "Cost (million USD)": "#2e7d4f" }
xlabel: Boundary of the plan (km)
ylabel: Cost (million USD)
height: 270
caption: "The blended calibration of the 30 % plan: six penalties from 0.001 (right) to 0.05 (left), solved in parallel in 50 s (gap 2 %, a time limit of 90 s each). The knee is at a penalty of 0.005 (368 km, 37.4 million USD); TOPSIS chooses 0.05 (238 km, 39.7 million)."
```

```tfgallery
columns: 2
caption: "The plans the two rules choose. The knee (left) keeps most of the cheap plan's shape for 0.7 % more than the optimum; TOPSIS (right), weighing cost and boundary equally after scaling, takes the most compact candidate: a third less boundary for 6.9 % more cost."
images:
  - { src: /img/tutorials/spatial-prioritization/evaluating-plans/blended_knee.png, caption: "Knee: penalty 0.005" }
  - { src: /img/tutorials/spatial-prioritization/evaluating-plans/blended_topsis.png, caption: "TOPSIS: penalty 0.05" }
```

The knee answers "up to where is compactness a bargain?"; TOPSIS "which plan is the best all-rounder, if cost and
compactness matter equally?". Neither is *the* answer; both are reproducible starting points, better than a value
copied from another study. Note that TOPSIS here picks the end of the range given: with wider values it could pick
another point, so give a range that covers the bend and beyond.

The **hierarchical** approach asks the clearer question: how short can the boundary be if the plan may cost 0.5, 1, 2,
4, 7 or 10 % more than the cheapest? Its candidates are *minimum penalty* problems, harder to solve: with a time limit
of 120 s each, four of the six stopped before proving their plan (gaps of 9 to 33 %), and the curve is ragged. Allowing
4 % more cost (38.6 million) shortens the boundary to 278 km, 58 % of the cheapest plan's; 10 % more, to 229 km. Run it
on coarser planning units, or with longer time limits, before relying on it.

### What each budget buys

```tfchart
type: line
data: /data/tutorials/spatial-prioritization/evaluating-plans/pareto.csv
x: budget
y: [Shortfall, Targets met]
colors: { Shortfall: "#b4508a", "Targets met": "#2e7d4f" }
xlabel: Budget (million USD)
ylabel: Shortfall, or targets met
height: 270
caption: "The Pareto front of budget and representation (approach = pareto): for each budget, the least total shortfall of the eight 30 % targets (the sum, over the features, of the share of each target missing). It falls fast at first and slowly at the end; the knee is at 15 million USD. The green line counts the targets fully met."
```

Every point of the front is **efficient**: no plan is both cheaper and closer to the targets. The number of targets
met does not always rise with the budget (4 at 20 million, 3 at 25), because minimum shortfall reduces the total gap,
not the count of targets fully met. Choosing a point on the front is a political decision; the front makes it an
informed one. (The pareto approach takes the problem with its targets and any objective that uses them, `min_set` here or
`min_shortfall`: whatever the objective, it solves the minimum shortfall problem within each budget itself.)

### A ranking against a plan of the same cost

Zonation ranks every cell ([part 2](../zonation/)); a plan meets targets. To compare them fairly, take Zonation's best
cells (CAZ2, with costs) until they cost what the 30 % plan costs (37.1 million USD), and evaluate both against the
same targets:

![Zonation's best cells and the optimized plan, for the same money.](/img/tutorials/spatial-prioritization/evaluating-plans/zonation-vs-plan.png "Zonation's best cells for 37.1 million USD (715 cells) and the optimized plan for the same money (812 cells): 536 cells in both (green), 179 only in Zonation's (orange: the whole rivers, the coast, rings around the tamarin's fragments) and 276 only in the plan's (blue: more of the mountains and the lowland forests of the jaguar and the tapir).")

```tfchart
type: bar
data: /data/tutorials/spatial-prioritization/evaluating-plans/zonation-vs-plan.csv
x: feature
y: [Zonation's best cells, Optimized plan]
colors: { "Zonation's best cells": "#e8901f", "Optimized plan": "#2e7d4f" }
ylabel: "% of the feature held"
height: 300
caption: "What each holds of every feature. Zonation's best cells hold more of the rare features (76 % of the tamarin, 86 % of the mangrove, 53 % of the otter) and less of the widespread ones (22 % of the jaguar, 20 % of the tapir): five targets of 30 % met of eight, a mean of 46 % held. The plan meets all eight, most exactly, a mean of 36 %."
```

| | Zonation's best cells | Optimized plan |
|---|---|---|
| Cost | 37.1 million USD | 37.1 million USD |
| Cells | 715 | 812 |
| Targets of 30 % met | 5 of 8 | 8 of 8 |
| Mean share held | 46 % | 36 % |
| Sum of extinction risks (z = 0.25) | 1.57 | 1.86 |

Neither is wrong; they answer different questions. CAZ2 keeps as much as possible of **every** feature at every
fraction, weighted towards the rarest: it knows no targets and goes on adding the tamarin's habitat long after 30 %.
The plan meets **exactly** what was asked, at the least cost, and nothing more. By the species-area yardstick, the
ranking does better (a lower sum of risks); by the targets, the plan does. A good workflow uses both: the ranking to
explore before targets are agreed, the plan once they are, and this comparison to show what the targets leave out.

## How to report a prioritization

A plan is only as useful as its documentation. Whatever the method, a paper or a report should let a reader repeat the
analysis and judge its choices. A checklist, drawing on the practice of the field (Ardron et al., 2010; Hanson et al.,
2025):

**Data**

- The study area, the planning units (cells, hexagons, polygons; their size and number) and the coordinate system.
- Every feature: what it is (species, ecosystem, service), the data (ranges, distribution models with their scores,
  maps), units and date; how features were weighted.
- The cost: what it measures (land price, opportunity cost, area), its source, units and year.
- Existing reserves and excluded areas (locked in and out), and why.

**The problem**

- The method (ranking or optimisation) and the software with its version (here Terraflux, and the model it reproduces:
  Zonation 5, Marxan 4, prioritizr 8, prioriactions 0.5).
- The targets and their justification (policy, ecology, stakeholders), or the budget.
- The objective; the decisions (binary, proportion); penalties and constraints with their values and how they were
  calibrated (the curve, the knee or TOPSIS).
- The solver and its optimality gap (asked and reached), or for heuristics the runs, iterations and settings.

**The results**

- A map of the plan, and of the importance of its places.
- The cost, the area, the number of units, the boundary.
- A table of every feature: the target, the amount held, met or not (the `coverage` of [[prioritize.evaluate]]).
- The trade-offs explored (cost against boundary, budgets against shortfall) and the alternatives (portfolios,
  scenarios).
- The sensitivity of the plan to its main choices: targets, costs, weights, data.

**What it is not**

- The limits of the data and the assumptions (that features persist in what is selected, that costs are right), and
  that the plan is a support for decisions that also depend on what no map holds (Game et al., 2013).

Put the model file (the TOML, or the Terraflux project) and the data in the supplementary material: with them, anyone
can run the same plan again.

## Settings that change the answer

- **The targets the evaluation uses.** Coverage is measured against them; evaluate every plan against the same.
- **The gap of the replacement costs.** Each is a difference between two optimised costs: with a gap of 1 % on a plan of
  37 million USD, noise of 370 thousand swamps the small values.
- **The rescaling** (`rescale`): scores from 0.01 to 1 compare places within a plan, not across plans.
- **The candidates of a calibration**: the knee and TOPSIS choose among the values you give; a curve with too few
  points or too narrow a range can miss the bend.

## Common pitfalls

- **Presenting a plan without its coverage table.** It is the first thing a reader needs.
- **Comparing plans evaluated against different targets or costs.**
- **Treating importance as a ranking of the whole landscape.** It ranks the selected cells of one plan; for the whole
  landscape, use a [Zonation ranking](../zonation/).
- **A calibration chosen by eye on a map.** Use the curve, and say how the point was chosen.
- **Reporting the objective as the cost** when penalties are in the objective.

## Going further

- Part 7, [Multi-action planning](../multi-action-planning/): actions against threats.
- The template **Irreplaceability of a plan** for a quick start.
- The operators: [[prioritize.evaluate]], [[prioritize.calibrate]].

## References

- Albuquerque, F., & Beier, P. (2015). Rarity-weighted richness: a simple and reliable alternative to integer programming and heuristic algorithms for minimum set and maximum coverage problems in conservation planning. *PLoS ONE*, 10(3), e0119905. https://doi.org/10.1371/journal.pone.0119905
- Ardron, J. A., Possingham, H. P., & Klein, C. J. (Eds.) (2010). *Marxan Good Practices Handbook*, Version 2. Pacific Marine Analysis and Research Association, Victoria, Canada.
- Cabeza, M., & Moilanen, A. (2006). Replacement cost: a practical measure of site value for cost-effective reserve planning. *Biological Conservation*, 132(3), 336–342. https://doi.org/10.1016/j.biocon.2006.04.025
- Cohon, J. L., Church, R. L., & Sheer, D. P. (1979). Generating multiobjective trade-offs: an algorithm for bicriterion problems. *Water Resources Research*, 15(5), 1001–1010. https://doi.org/10.1029/WR015i005p01001
- Ferrier, S., Pressey, R. L., & Barrett, T. W. (2000). A new predictor of the irreplaceability of areas for achieving a conservation goal, its application to real-world planning, and a research agenda for further refinement. *Biological Conservation*, 93(3), 303–325. https://doi.org/10.1016/S0006-3207(99)00149-4
- Game, E. T., Kareiva, P., & Possingham, H. P. (2013). Six common mistakes in conservation priority setting. *Conservation Biology*, 27(3), 480–485. https://doi.org/10.1111/cobi.12051
- Hanson, J. O., Schuster, R., Strimas-Mackey, M., Morrell, N., Edwards, B. P. M., Arcese, P., Bennett, J. R., & Possingham, H. P. (2025). Systematic conservation prioritization with the prioritizr R package. *Conservation Biology*, 39(1), e14376. https://doi.org/10.1111/cobi.14376
- Hwang, C.-L., & Yoon, K. (1981). *Multiple Attribute Decision Making: Methods and Applications*. Springer. https://doi.org/10.1007/978-3-642-48318-9
- Jung, M., Arnell, A., de Lamo, X., García-Rangel, S., Lewis, M., Mark, J., et al. (2021). Areas of global importance for conserving terrestrial biodiversity, carbon and water. *Nature Ecology & Evolution*, 5(11), 1499–1509. https://doi.org/10.1038/s41559-021-01528-7
- Margules, C. R., & Pressey, R. L. (2000). Systematic conservation planning. *Nature*, 405(6783), 243–253. https://doi.org/10.1038/35012251
- Satopää, V., Albrecht, J., Irwin, D., & Raghavan, B. (2011). Finding a "kneedle" in a haystack: detecting knee points in system behavior. In *31st International Conference on Distributed Computing Systems Workshops* (pp. 166–171). IEEE. https://doi.org/10.1109/ICDCSW.2011.20
