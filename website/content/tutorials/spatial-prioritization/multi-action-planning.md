---
title: Multi-action planning
short: 7. Multi-action planning
description: Decide which actions against which threats to take where, as a mixed integer program - threats, sensitivity, recovery and conservation targets, the cheapest plan and the most benefit for a budget, on Serra Verde's maps and on tables of any planning units.
order: 70
level: advanced
time: 50 min
template: Multi-action planning
ops: [prioritize.actions_raster, prioritize.actions, table.of, raster.read, view.plot]
series: Systematic conservation planning
part: 7
---

The plans of this series so far choose places to protect. But biodiversity is mostly lost to **threats**:
deforestation, fire, invasive species, overgrazing, pollution. Fighting the right threat in the right place can do
more than buying land where nothing threatens anything. In this last part you will plan **actions against threats**
for Serra Verde with [[prioritize.actions_raster]], the model of Salgado-Rojas et al. (2020): where to fight
deforestation, where to fight fire, and where managing the land is enough, so that every feature recovers to its
target at the least cost. Then you will spend a fixed budget for the most benefit, see why that objective leaves the
rarest features behind, tell the model which features each threat really harms, and run the same model on tables of
any planning units.

## The idea

### From places to actions

A multi-action problem chooses **actions in places**. Its ingredients (Cattarino et al., 2015; Salgado-Rojas et
al., 2020):

| Ingredient | Meaning | In Serra Verde |
|---|---|---|
| **planning units** | the places, each with a cost of **monitoring** (managing it at all) | the 4 825 cells, with the cost of land |
| **features** | what should benefit | the eight species and ecosystems |
| **threats** | what harms the features, with an intensity in every unit | deforestation along the highway and around the cities; fire in a band across the west |
| **actions** | one per threat: removing it from a unit, at a cost | fighting deforestation (30 thousand USD per cell) and fire (10) |
| **sensitivity** | which features each threat harms, and how much | by default every feature to every threat |

![The two threats of Serra Verde.](/img/tutorials/spatial-prioritization/multi-action-planning/threats.png "The intensity of the two threats: deforestation (1 555 cells) along the highway, the coast and around the three cities, and fire (1 283 cells) in a band across the western mountains.")

!!! science "Key idea: conservation and recovery"
    The benefit of a plan for a feature has two parts (Salgado-Rojas et al., 2020):

    - **conservation**: the amount of the feature in monitored units where **no threat** harms it. Being managed
      keeps it;
    - **recovery**: the amount that comes back where the threats that harm it are **removed by actions**.

    A unit can only take actions if it is monitored, and its monitoring cost is paid once whatever actions it takes.
    Targets can be set for each part: a share of each feature's threatened amount to recover, a share of its
    unthreatened amount to conserve. With several threats in a unit, the benefit grows with the share of them
    removed, linearly (`curve = 1`) or more steeply (`curve = 2` or `3`: half the threats removed gives a quarter or an
    eighth of the benefit).

It is the logic of a doctor with a budget: some patients only need a check-up (monitoring), some need a specific
treatment (an action against their threat), and treating the wrong illness, or patients who would recover anyway,
wastes the budget. Carwardine et al. (2012) ranked strategies of threat management by the species they secure per
dollar, for the Kimberley of northern Australia; Wilson et al. (2007) made the case for asking "what to do, where,
and when" across the world's regions.

### Two objectives

- **`minimizeCosts`** (the default): the cheapest monitoring and actions that meet every target.
- **`maximizeBenefits`**: the most total benefit within a `budget`.

Both are integer linear programs, solved exactly to the gap you ask for. [[prioritize.actions]] builds the model of
Salgado-Rojas et al. (2020), its checks and defaults included (the authors' code was its reference, and its results
were checked against it); Terraflux solves it with landschaft's engine and HiGHS.

## When to use it

- **Threat management plans**: weeding, fencing, fire management, pest control, restoration of riparian strips;
  where to do each.
- **Freshwater plans**, where threats spread down rivers and actions upstream protect reaches downstream.
- **When protection is not the action**: on land that will stay private or used, the decision is what to do there,
  not whether to buy it.

When the decision is the **use** of each place (strict protection, sustainable use), use management zones ([part
5](../spatial-design/#zones)): a unit takes one zone. When it is **how much** of each converted place to restore,
use the restoration plans of [Distance and restoration](/tutorials/distance-restoration/restoration-plan/).

## What you need

On rasters ([[prioritize.actions_raster]]), all on one grid:

- `monitoring_cost`: the cost of managing every cell (no value: not a planning unit);
- `features`: a stack, a layer per feature (its amount in every cell);
- `threats`: a stack, a layer per threat (its intensity, above 0 where it occurs; an action there removes it);
- optionally `locked_in` and `locked_out` maps, a `maps` stack of the actions' costs, a `sensitivity` table.

On tables of any planning units ([[prioritize.actions]]): six tables (`pu`, `features`,
`dist_features`, `threats`, `dist_threats`, optionally `sensitivity` and `boundary`).

Serra Verde's threats are in `threats.tif` of
[serra-verde.zip](/data/tutorials/spatial-prioritization/systematic-conservation-planning/serra-verde.zip) (two
bands, `deforestation` and `fire`). The template **Templates ▸ Conservation and restoration ▸ Multi-action planning**
is a small problem in tables.

## Build the model

### On maps

```tfgraph
#! caption: The cheapest monitoring and actions against deforestation and fire that recover 30 % of every feature's threatened amount and conserve 10 % of its unthreatened amount.
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
note = "The monitoring cost: thousand USD per cell."

[[nodes]]
id = "threats"
op = "raster.read"
params = { path = "threats.tif" }
note = "Two bands: deforestation and fire, 0 to 1."

[[nodes]]
id = "plan"
op = "prioritize.actions_raster"
inputs = { monitoring_cost = "cost.raster", features = "features.raster", threats = "threats.raster" }
params = { action_cost = [30, 10], target_recovery = 0.3, target_conservation = 0.1, model = "minimizeCosts", gap = 0.01 }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "plan.maps" }
```

<div class="steps" markdown>

1. **The inputs** (nodes 1 to 3): the features' `stack`, the cost's `map` as `monitoring_cost`, the threats' `stack`.
   Every cell with a monitoring cost is a planning unit; every cell where a threat is above 0 is a possible action
   against it.
2. **The plan** (node 4), in the Node pane:
    - `action_cost`: the cost of acting against each threat, a number for all, a list in the threats' order (here 30
      for deforestation, 10 for fire), `{threat: cost}`, or the name of a layer of the `maps` input for costs that
      vary;
    - `target_recovery` and `target_conservation`: a number for all features, a list, or `{feature: target}`. With
      `relative = true` (the default) they are shares: of the amount each feature has where a threat harms it, and
      where none does;
    - `model`: `minimizeCosts` or `maximizeBenefits` with a `budget`;
    - `blm` (the monitored units' boundary) and `blm_actions` (each threat's actions' boundary), with `boundary =
      true` to use the cells' shared edges, for compact plans as in part 5;
    - `curve` and `segments` (the response, above), `gap`, `time_limit`.
3. **Outputs**: `maps`, a stack of 0/1 layers, `monitoring` and one per threat by its name; tables of the plan
   (`monitoring`, `actions`, `summary` per unit), of the costs (`cost`: monitoring and each threat's actions), of the
   benefits (`benefit`: conservation, recovery and total per feature; `local` per unit and feature; `potential`, the
   most each feature could get) and the solver's `result` (objective, gap, optimal).

</div>

!!! app "In the application"
    Add `prioritize.actions_raster` from the *Spatial prioritization* subject, connect the three maps, set the
    targets and press ++ctrl+enter++. The `maps` port shows the monitored cells and the actions as layers of a
    stack; the `benefit` table says what every feature gets.

### On tables

For properties, watersheds or any planning units, [[prioritize.actions]] takes the problem as tables. The template
is the smallest example: six units, two species, one threat in five of them.

```tfgraph
#! caption: The template Multi-action planning: the problem's tables written with table.of (in their place, tables read from files) and the cheapest plan.
#! numbered: true
#! highlight: plan
format = "terraflux/1"

[[nodes]]
id = "pu"
op = "table.of"
params = { columns = ["id", "monitoring_cost"], rows = [[1, 2], [2, 1], [3, 3], [4, 1], [5, 2], [6, 2]] }

[[nodes]]
id = "features"
op = "table.of"
params = { columns = ["id", "target_recovery"], rows = [[1, 1], [2, 1]] }

[[nodes]]
id = "dist_features"
op = "table.of"
params = { columns = ["pu", "feature", "amount"], rows = [[1, 1, 1], [2, 1, 1], [3, 1, 1], [3, 2, 1], [4, 2, 1], [5, 2, 1], [6, 1, 1]] }

[[nodes]]
id = "threats"
op = "table.of"
params = { columns = ["id"], rows = [[1]] }

[[nodes]]
id = "dist_threats"
op = "table.of"
params = { columns = ["pu", "threat", "amount", "action_cost"], rows = [[1, 1, 1, 1], [2, 1, 1, 4], [3, 1, 1, 2], [4, 1, 1, 1], [5, 1, 1, 2]] }

[[nodes]]
id = "plan"
op = "prioritize.actions"
inputs = { pu = "pu.table", features = "features.table", dist_features = "dist_features.table", threats = "threats.table", dist_threats = "dist_threats.table" }
params = { model = "minimizeCosts" }
```

The tables:

| Table | Columns | Here |
|---|---|---|
| `pu` | `id`, `monitoring_cost`, `status` (2 locked in, 3 out) | six units |
| `features` | `id`, `target_recovery`, `target_conservation` | two species, each to recover by 1 (an absolute amount) |
| `dist_features` | `pu`, `feature`, `amount` | where each species lives |
| `threats` | `id`, `blm_actions` | one threat |
| `dist_threats` | `pu`, `threat`, `amount`, `action_cost`, `status` | where it is, and what acting there costs |
| `sensitivity` | `feature`, `threat`, `delta1` to `delta4` | (optional) which features each threat harms |
| `boundary` | `id1`, `id2`, `boundary` | (optional) shared lengths, for the blms |

The answer: monitor units 1 and 4 (cost 3) and act against the threat in both (cost 2), a total of 5. Unit 1 holds
species 1 and unit 4 species 2, and acting there is the cheapest way to recover one unit of each. Unit 6, which holds
species 1 with no threat, would not help: its benefit is conservation, and the targets ask for recovery.

## Read the results

![The cheapest plan of monitoring and actions.](/img/tutorials/spatial-prioritization/multi-action-planning/plan.png "The cheapest plan that recovers 30 % of every feature's threatened amount and conserves 10 % of its unthreatened amount: fire is fought in large blocks of the western forests (578 cells), deforestation along the coast, the rivers and around the tamarin's fragments (186 cells), both in 5 cells, and 109 cells are only monitored. 57.6 million USD, within 0.9 % of the optimum.")

The `cost` table: monitoring 46.0 million USD, actions against deforestation 5.7 million, against fire 5.8 million.
Monitoring dominates, since every unit with an action must be managed. The pattern follows the threats and the
features they harm: fire in the west, where it threatens the jaguar, the harpy eagle, the maned wolf and the araucaria
forest; deforestation on the coast and in the lowlands, where it threatens the mangrove, the tamarin and the otter.
The cells only monitored hold features where no threat is: they meet the conservation targets.

The `benefit` table against the `potential` (the most each feature could get if every unit were managed and every
threat removed):

| Feature | Recovery | Conservation | Total | Most possible |
|---|---|---|---|---|
| Jaguar | 323.8 | 82.8 | 406.6 | 1 534.1 |
| Tapir | 76.5 | 34.3 | 110.8 | 465.6 |
| Harpy eagle | 267.6 | 56.0 | 323.6 | 825.1 |
| Maned wolf | 282.7 | 41.7 | 324.5 | 972.5 |
| Tamarin | 18.4 | 1.3 | 19.7 | 47.6 |
| Otter | 49.5 | 20.7 | 70.2 | 217.3 |
| Araucaria forest | 114.8 | 44.9 | 159.7 | 425.4 |
| Mangrove | 19.9 | 0 | 19.9 | 26.9 |

The mangrove has no conservation benefit at all: deforestation reaches every cell of it, so all of its benefit must
come from recovery. Its recovery target forces deforestation actions along the coast.

### Reporting a multi-action plan

Report the threats and how their intensity was mapped, the actions and their costs, the sensitivity of each feature
to each threat (or that every feature was assumed to suffer from every threat), the targets of recovery and
conservation, the objective, the solver's gap, and the plan's costs by action and benefits by feature. Show the map of
actions. Like every plan of this series, it is a decision-support tool: the costs and the sensitivities are estimates,
and the people who will carry out the actions should see them.

## Settings that change the answer

### A budget instead of targets

```tfslider
label: Budget (million USD)
caption: "The plan with the most total benefit (maximizeBenefits) for budgets from 10 to 60 million USD, and the share of its most possible benefit each feature gets. Monitoring takes nearly all the money, in large blocks of cheap mountain land; fire is fought there from 20 million on. Deforestation is never fought, and the tamarin and the mangrove get nothing at any budget."
frames:
  - { value: 10, src: /img/tutorials/spatial-prioritization/multi-action-planning/budget_10.png, stats: { "Monitored": "286 cells", "Against fire": "0", "Against deforestation": "0" } }
  - { value: 20, src: /img/tutorials/spatial-prioritization/multi-action-planning/budget_20.png, stats: { "Monitored": "516 cells", "Against fire": "70", "Against deforestation": "0" } }
  - { value: 30, src: /img/tutorials/spatial-prioritization/multi-action-planning/budget_30.png, stats: { "Monitored": "721 cells", "Against fire": "225", "Against deforestation": "0" } }
  - { value: 40, src: /img/tutorials/spatial-prioritization/multi-action-planning/budget_40.png, stats: { "Monitored": "914 cells", "Against fire": "370", "Against deforestation": "0" } }
  - { value: 60, src: /img/tutorials/spatial-prioritization/multi-action-planning/budget_60.png, stats: { "Monitored": "1 279 cells", "Against fire": "647", "Against deforestation": "0" } }
start: 2
chart:
  type: line
  data: /data/tutorials/spatial-prioritization/multi-action-planning/budgets.csv
  x: budget
  y: [Jaguar, Tapir, Harpy eagle, Maned wolf, Tamarin, Otter, Araucaria, Mangrove]
  colors: { Jaguar: "#0072b2", Tapir: "#e69f00", "Harpy eagle": "#009e73", "Maned wolf": "#cc79a7", Tamarin: "#56b4e9", Otter: "#d55e00", Araucaria: "#f0e442", Mangrove: "#785ef0" }
  xlabel: Budget (million USD)
  ylabel: "% of the most possible benefit"
```

Maximising the **total** benefit adds up every feature in its own units, so a widespread feature in cheap land always
offers more benefit per dollar: the jaguar, the harpy eagle, the maned wolf and the araucaria forest win, and the
rare tamarin and mangrove, on dear land and threatened by the dearer action, get nothing even with 60 million, more
than the cheapest plan that meets every target. This is the same lesson as maximum utility in [part 4](../linear-programming/),
and the remedies are the same: targets (with `minimizeCosts`), or features scaled to comparable totals before
planning.

### Which features each threat harms

By default every threat harms every feature. A `sensitivity` table says otherwise: a row per (feature, threat) pair
that is harmed, with `delta1` to `delta4` shaping the response (the intensity from which the feature starts to suffer
and at which it is lost, and the least and most probability of persistence; 0, 1, 0, 1 is a straight line). Only the
pairs listed count:

```tfgraph
#! caption: Only the tamarin (feature 5) and the mangrove (feature 8) are harmed, and only by deforestation (threat 1).
#! fragment: true
#! highlight: sensitivity
[[nodes]]
id = "sensitivity"
op = "table.of"
params = { columns = ["feature", "threat", "delta1", "delta2", "delta3", "delta4"], rows = [[5, 1, 0, 1, 0, 1], [8, 1, 0, 1, 0, 1]] }

[[nodes]]
id = "plan"
op = "prioritize.actions_raster"
inputs = { monitoring_cost = "cost.map", features = "features.stack", threats = "threats.stack", sensitivity = "sensitivity.table" }
params = { action_cost = [30, 10], target_recovery = 0.3, target_conservation = 0.1, gap = 0.01 }
```

![The plan when only the tamarin and the mangrove suffer from deforestation.](/img/tutorials/spatial-prioritization/multi-action-planning/sensitivity.png "With only the tamarin and the mangrove harmed, and only by deforestation: 113 cells of actions against deforestation (the coast and the tamarin's fragments), no action against fire, and 247 cells only monitored for the conservation targets of the other features. 23.5 million USD instead of 57.6.")

The plan costs 23.5 million instead of 57.6: fire no longer harms anything, so no money goes to it, and the other
six features need only conservation. Sensitivity is often the least known part of the problem, and the one that
moves the plan most: base it on studies or experts, and test alternatives.

### The response curve

`curve` matters where several threats meet in a unit: with `curve = 3`, removing one of two threats gives an eighth
of the benefit, and the plan prefers to remove all the threats from fewer units. In Serra Verde the two threats
barely overlap (the plan acts against both in only 5 cells), and `curve = 3` gives the same plan as `curve = 1`.
With overlapping threats (fire, grazing and weeds in the same paddocks) it changes a lot.

## Common pitfalls

- **Maximising total benefit without thinking about units.** It favours widespread features, as the slider shows.
- **Relative targets of a feature that no threat reaches.** Its recovery target is then a share of nothing; its
  benefit can only be conservation. Read the `potential` table first.
- **Threat maps that are not intensities.** A threat layer above 0 everywhere makes every cell a possible action;
  set cells without the threat to 0.
- **Sensitivity left at its default** when it is known that some threats spare some features.
- **Costs in different units** for monitoring and actions. They are added: use the same currency and period.

## Going further

- The series from the start: [1. The ideas](../systematic-conservation-planning/), [2. Zonation](../zonation/),
  [3. Marxan](../marxan/), [4. Linear programming](../linear-programming/), [5. Spatial design](../spatial-design/),
  [6. Evaluating plans](../evaluating-plans/).
- Restoring converted land: [Distance and restoration](/tutorials/distance-restoration/restoration-plan/).
- The operators: [[prioritize.actions_raster]], [[prioritize.actions]].

## References

- Carwardine, J., O'Connor, T., Legge, S., Mackey, B., Possingham, H. P., & Martin, T. G. (2012). Prioritizing threat management for biodiversity conservation. *Conservation Letters*, 5(3), 196–204. https://doi.org/10.1111/j.1755-263X.2012.00228.x
- Cattarino, L., Hermoso, V., Carwardine, J., Kennard, M. J., & Linke, S. (2015). Multi-action planning for threat management: a novel approach for the spatial prioritization of conservation actions. *PLoS ONE*, 10(5), e0128027. https://doi.org/10.1371/journal.pone.0128027
- Salgado-Rojas, J., Álvarez-Miranda, E., Hermoso, V., Garcia-Gonzalo, J., & Weintraub, A. (2020). A mixed integer programming approach for multi-action planning for threat management. *Ecological Modelling*, 418, 108901. https://doi.org/10.1016/j.ecolmodel.2019.108901
- Wilson, K. A., Underwood, E. C., Morrison, S. A., Klausmeyer, K. R., Murdoch, W. W., Reyers, B., et al. (2007). Conserving biodiversity efficiently: what to do, where, and when. *PLoS Biology*, 5(9), e223. https://doi.org/10.1371/journal.pbio.0050223
